import 'dart:typed_data';

import '../api/api_client.dart';
import '../api/api_exception.dart';
import '../models/json.dart';
import '../models/recipe.dart';
import '../util/text_fold.dart';
import 'local_store.dart';

class MigrationProgress {
  const MigrationProgress(this.phase, this.done, this.total);
  final String phase;
  final int done;
  final int total;
  double? get fraction => total == 0 ? null : done / total;
}

class MigrationReport {
  int recipes = 0;
  int skipped = 0;
  int images = 0;
  int mealPlan = 0;
  int lists = 0;
  int items = 0;
  final List<String> warnings = [];
}

/// One-time upload of standalone data to a server ("connect later").
///
/// Everything is created with its existing id, so replays are idempotent.
/// Finished steps are recorded in the store, so an interrupted migration
/// (network error, app killed) resumes where it stopped.
class Migrator {
  Migrator({required this.store, required this.api, required this.readFile});

  final LocalStore store;
  final ApiClient api;
  final Future<Uint8List?> Function(String path) readFile;

  late Set<String> _done;

  Future<void> _mark(String key) async {
    _done.add(key);
    await store.setMeta(MetaKeys.migratedIds, _done.toList());
  }

  /// Recipes already on the server, indexed by the server's duplicate rule
  /// (same source URL, or same accent-folded name). Recipes whose id is one of
  /// [ownIds] were uploaded by an earlier, interrupted run of this migration
  /// and are not treated as pre-existing.
  Future<DuplicateIndex> _serverRecipes(Set<String> ownIds) async {
    // The full sync carries complete recipes (the list endpoint has no source_url).
    final res = asJson(await api.get('/api/sync'));
    final index = DuplicateIndex();
    for (final r in asJsonList(res['recipes'])) {
      final id = asStr(r['id']);
      if (ownIds.contains(id)) continue;
      index.add(id, name: asStr(r['name']), sourceUrl: _cleanUrl(asString(r['source_url'])));
    }
    return index;
  }

  static String? _cleanUrl(String? url) {
    final u = url?.trim();
    return u == null || u.isEmpty ? null : u;
  }

  /// Runs (or resumes) the migration. With [skipExistingNames], local recipes
  /// that duplicate a server recipe (same source URL or same folded name, as
  /// the server's import does) are not uploaded; references to them are
  /// remapped to the server's recipe.
  ///
  /// Throws [NetworkException] when the server becomes unreachable; call
  /// again to resume.
  Future<MigrationReport> run({bool skipExistingNames = false, void Function(MigrationProgress)? onProgress}) async {
    final report = MigrationReport();
    _done = asStringList(await store.getMeta<List>(MetaKeys.migratedIds)).toSet();
    final recipes = await store.recipes();
    final entries = await store.mealPlanEntries();
    final lists = await store.shoppingLists();

    // Local recipe id -> server recipe id (differs for skipped duplicates).
    final remap = <String, String>{};
    final existing = skipExistingNames ? await _serverRecipes({for (final r in recipes) r.id}) : null;

    var n = 0;
    for (final r in recipes) {
      onProgress?.call(MigrationProgress('Recipes', n++, recipes.length));
      final dup = existing?.find(name: r.name, sourceUrl: _cleanUrl(r.sourceUrl));
      if (dup != null) {
        remap[r.id] = dup;
        report.skipped++;
        continue;
      }
      if (!_done.contains('r:${r.id}')) {
        await _uploadRecipe(r, report);
        await _mark('r:${r.id}');
      }
    }
    onProgress?.call(MigrationProgress('Recipes', recipes.length, recipes.length));

    n = 0;
    for (final e in entries) {
      onProgress?.call(MigrationProgress('Meal plan', n++, entries.length));
      if (_done.contains('m:${e.id}')) continue;
      final body = e.toInput();
      if (e.recipeId != null) body['recipe_id'] = remap[e.recipeId] ?? e.recipeId;
      await _tolerant(() => api.post('/api/mealplan', body), 'Meal plan entry ${e.date}', report);
      report.mealPlan++;
      await _mark('m:${e.id}');
    }

    n = 0;
    final totalItems = lists.fold<int>(0, (a, l) => a + (l.items?.length ?? 0)) + lists.length;
    for (final l in lists) {
      onProgress?.call(MigrationProgress('Shopping lists', n++, totalItems));
      if (!_done.contains('l:${l.id}')) {
        await _tolerant(
          () => api.post('/api/shopping-lists', {'id': l.id, 'name': l.name}),
          'List "${l.name}"',
          report,
        );
        report.lists++;
        await _mark('l:${l.id}');
      }
      for (final i in l.items ?? const []) {
        onProgress?.call(MigrationProgress('Shopping lists', n++, totalItems));
        if (_done.contains('i:${i.id}')) continue;
        await _tolerant(
          () async {
            await api.post('/api/shopping-lists/${l.id}/items', {
              'id': i.id,
              if (i.food == null) 'text': i.display,
              'food': i.food,
              'quantity': jsonNum(i.quantity),
              'unit': i.unit,
              'note': i.note,
              'aisle': i.aisle,
            });
            if (i.checked) await api.patch('/api/shopping-lists/${l.id}/items/${i.id}', {'checked': true});
            return null;
          },
          'Item "${i.label}"',
          report,
        );
        report.items++;
        await _mark('i:${i.id}');
      }
    }
    onProgress?.call(MigrationProgress('Done', totalItems, totalItems));
    await store.setMeta(MetaKeys.migratedIds, null);
    return report;
  }

  Future<void> _uploadRecipe(Recipe r, MigrationReport report) async {
    final ok = await _tolerant(
      () => api.post('/api/recipes', {
        ...r.toInput(includeVersion: false),
        // Accepted on create only, so the trial history moves with the recipe.
        'tried_count': r.triedCount,
        'last_tried_at': r.lastTriedAt?.toUtc().toIso8601String(),
      }),
      'Recipe "${r.name}"',
      report,
    );
    if (!ok) return;
    report.recipes++;
    if (r.isFavorite) await _tolerant(() => api.put('/api/recipes/${r.id}/favorite'), 'Favorite "${r.name}"', report);
    final path = r.localImage('full');
    if (path != null) {
      final bytes = await readFile(path);
      if (bytes != null) {
        final uploaded = await _tolerant(
          () => api.multipart(
            'PUT',
            '/api/recipes/${r.id}/image',
            files: [UploadFile(field: 'file', bytes: bytes, filename: '${r.id}.jpg')],
          ),
          'Photo for "${r.name}"',
          report,
        );
        if (uploaded) report.images++;
      }
    }
  }

  /// Runs [f]; 4xx errors become warnings (the item is skipped), network
  /// and 5xx errors propagate so the migration can be resumed later.
  Future<bool> _tolerant(Future<Object?> Function() f, String what, MigrationReport report) async {
    try {
      await f();
      return true;
    } on ApiException catch (e) {
      if (e.statusCode >= 500 || e.statusCode == 401) rethrow;
      report.warnings.add('$what: ${e.message}');
      return false;
    }
  }
}
