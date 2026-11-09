import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path/path.dart' as p;

import '../api/api_exception.dart';
import '../data/backend.dart';
import '../data/local_store.dart';
import '../data/outbox_op.dart';
import '../data/preferences.dart';
import '../data/providers.dart';
import '../engine/engine.dart';
import '../models/cookbook.dart';
import '../models/import_job.dart';
import '../models/json.dart';
import '../models/meal_plan.dart';
import '../models/organize.dart';
import '../models/recipe.dart';
import '../models/shopping.dart';
import '../util/categories.dart';
import '../util/text_fold.dart';

/// Directories used in standalone mode (under the app documents dir).
class LocalPaths {
  LocalPaths(this._root);
  final Future<Directory> Function() _root;
  Directory? _cached;

  Future<Directory> _sub(String name) async {
    final root = _cached ??= await _root();
    final d = Directory(p.join(root.path, name));
    if (!d.existsSync()) await d.create(recursive: true);
    return d;
  }

  /// Processed recipe photos (source of truth in standalone mode).
  Future<Directory> images() => _sub('images');

  /// Scratch space for imports / renders.
  Future<Directory> tmp() => _sub('tmp');

  Future<String> tmpFile(String name) async => p.join((await tmp()).path, '${newId()}-$name');
}

const _applied = OpOutcome(OutcomeKind.applied);

/// Standalone mode: the local store is the source of truth; parsing,
/// normalising, merging, imports and rendering run in the engine.
Backend createLocalBackend(Ref ref, Engine engine, LocalPaths paths) {
  final recipes = LocalRecipeRepository(ref, engine, paths);
  return Backend(
    recipes: recipes,
    mealPlan: LocalMealPlanRepository(ref),
    shopping: LocalShoppingRepository(ref, engine),
    imports: LocalImportService(ref, engine, paths, recipes),
    cookbook: LocalCookbookService(ref, engine, paths),
  );
}

class LocalRecipeRepository implements RecipeRepository {
  LocalRecipeRepository(this.ref, this.engine, this.paths);
  final Ref ref;
  final Engine engine;
  final LocalPaths paths;
  LocalStore get store => ref.read(localStoreProvider);

  /// Normalises [recipe] with the engine (parses changed ingredient lines,
  /// computes slug + search text) and stores it.
  Future<Recipe> normalizeAndStore(Recipe recipe, {Recipe? existing}) async {
    final norm = await engine.normalizeRecipe(recipe.toInput(includeVersion: false));
    final now = DateTime.now().toUtc();
    final merged = Recipe.fromJson({
      ...recipe.toJson(),
      ...norm,
      'id': recipe.id,
      'household_id': 'local',
      'image_url': null,
      'is_favorite': recipe.isFavorite,
      'created_at': (existing?.createdAt ?? recipe.createdAt ?? now).toIso8601String(),
      'updated_at': now.toIso8601String(),
      'version': (existing?.version ?? 0) + 1,
      'local_images': existing?.localImages ?? recipe.localImages,
    });
    await store.putRecipe(merged);
    return merged;
  }

  @override
  Future<OpOutcome> save(Recipe recipe, {required bool isNew, bool force = false}) async {
    try {
      final existing = await store.recipe(recipe.id);
      final r = await normalizeAndStore(recipe, existing: existing);
      return OpOutcome(OutcomeKind.applied, data: r.toJson());
    } on ApiException catch (e) {
      return OpOutcome(OutcomeKind.rejected, message: e.message);
    }
  }

  @override
  Future<OpOutcome> delete(Recipe recipe) async {
    final existing = await store.recipe(recipe.id);
    await store.deleteRecipe(recipe.id);
    await _deleteFiles(existing?.localImages ?? recipe.localImages);
    return _applied;
  }

  @override
  Future<OpOutcome> setFavorite(Recipe recipe, bool favorite) async {
    await store.putRecipe(recipe.copyWith(isFavorite: favorite));
    return _applied;
  }

  @override
  Future<Recipe> duplicate(String id) async {
    final src = await refresh(id);
    final newRecipeId = newId();
    final images = <String, String>{};
    final dir = await paths.images();
    for (final e in src.localImages.entries) {
      final f = File(e.value);
      if (f.existsSync()) {
        final dest = p.join(dir.path, '$newRecipeId-${e.key}${p.extension(e.value)}');
        await f.copy(dest);
        images[e.key] = dest;
      }
    }
    final copy = src.copyWith(
      id: newRecipeId,
      name: '${src.name} (copy)',
      isFavorite: false,
      localImages: images,
      createdAt: DateTime.now().toUtc(),
      version: () => null,
    );
    return normalizeAndStore(copy);
  }

  @override
  Future<Recipe> setImage(String id, Uint8List bytes, String filename) async {
    final path = await paths.tmpFile(p.basename(filename.isEmpty ? 'photo.jpg' : filename));
    await File(path).writeAsBytes(bytes, flush: true);
    try {
      return (await setImageFromPath(id, path))!;
    } finally {
      unawaited(File(path).delete().then((_) {}, onError: (_) {}));
    }
  }

  @override
  Future<Recipe?> setImageFromPath(String id, String path) async {
    final r = await refresh(id);
    final out = await engine.processImage(path, (await paths.images()).path, newId());
    await _deleteFiles(r.localImages);
    final updated = r.copyWith(localImages: out, imageUrl: () => null, updatedAt: DateTime.now().toUtc());
    await store.putRecipe(updated);
    return updated;
  }

  @override
  Future<Recipe> removeImage(String id) async {
    final r = await refresh(id);
    await _deleteFiles(r.localImages);
    final updated = r.copyWith(localImages: const {}, imageUrl: () => null);
    await store.putRecipe(updated);
    return updated;
  }

  @override
  Future<Recipe> refresh(String id) async {
    final r = await store.recipe(id);
    if (r == null) throw ApiException(404, 'Recipe not found', code: 'not_found');
    return r;
  }

  Future<void> _deleteFiles(Map<String, String> files) async {
    for (final path in files.values.toSet()) {
      try {
        final f = File(path);
        if (f.existsSync()) await f.delete();
      } catch (_) {}
    }
  }

  @override
  Future<OrganizeResult> organize({
    required CategorizeMode categorize,
    required bool normalize,
    required bool dryRun,
  }) async {
    // Same rules as the server's `organize` (routes/recipes.rs).
    final all = await store.recipes();
    final staged = <(Recipe, Json)>[];
    for (final r in all) {
      var data = r.toInput(includeVersion: false);
      if (normalize) data = {...data, ...await engine.normalizeRecipe(data), 'id': r.id};
      staged.add((r, data));
    }
    final suggestions = categorize == CategorizeMode.none
        ? const <String, String?>{}
        : await engine.suggestCategories([
            for (final (_, data) in staged)
              if (categorize == CategorizeMode.all || asStringList(data['categories']).isEmpty) data,
          ]);
    final changes = <OrganizeChange>[];
    var categorized = 0, normalized = 0;
    for (final (r, data) in staged) {
      final ingredients = asJsonList(data['ingredients']).map(Ingredient.fromJson).toList();
      final ingredientsChanged = !sameIngredients(ingredients, r.ingredients);
      final before = r.categories.firstOrNull;
      final categories = asStringList(data['categories']).toList();
      var categoryChanged = false;
      final c = suggestions[r.id];
      if (c != null && before != c) {
        categories
          ..removeWhere((x) => eqIgnoreAsciiCase(x, c))
          ..insert(0, c);
        categoryChanged = true;
      }
      if (!ingredientsChanged && !categoryChanged) continue;
      if (ingredientsChanged) normalized++;
      if (categoryChanged) categorized++;
      changes.add(
        OrganizeChange(
          id: r.id,
          name: r.name,
          categoryBefore: before,
          categoryAfter: categories.firstOrNull,
          ingredientsChanged: ingredientsChanged,
        ),
      );
      if (!dryRun) {
        await normalizeAndStore(
          r.copyWith(ingredients: ingredients, categories: categories),
          existing: r,
        );
      }
    }
    return OrganizeResult(
      dryRun: dryRun,
      recipesChanged: changes.length,
      categorized: categorized,
      normalized: normalized,
      changes: changes,
    );
  }

  /// Gives [data] (a RecipeInput) the suggested category when it has none,
  /// like the server's `auto_categorize`.
  Future<Json> autoCategorize(Json data) async {
    if (asStringList(data['categories']).isNotEmpty) return data;
    final c = (await engine.suggestCategories([
      {...data, 'id': 'x'},
    ]))['x'];
    return c == null
        ? data
        : {
            ...data,
            'categories': [c],
          };
  }

  /// Saves an engine `ImportedRecipe` as a new recipe (new id).
  Future<Recipe> saveImported(Json imported, {bool categorize = true}) async {
    var data = asJson(imported['data']);
    if (categorize) data = await autoCategorize(data);
    final createdAt = asDate(imported['created_at']);
    final draft = Recipe.fromJson({...data, 'id': newId()})
        .copyWith(isFavorite: asBool(imported['favorite']), createdAt: createdAt);
    var saved = await normalizeAndStore(draft);
    final image = asString(imported['image_path']);
    if (image != null && File(image).existsSync()) {
      try {
        saved = (await setImageFromPath(saved.id, image)) ?? saved;
      } catch (_) {
        // Keep the recipe even if its photo could not be processed.
      }
      unawaited(File(image).delete().then((_) {}, onError: (_) {}));
    }
    return saved;
  }
}

class LocalMealPlanRepository implements MealPlanRepository {
  LocalMealPlanRepository(this.ref);
  final Ref ref;
  LocalStore get store => ref.read(localStoreProvider);

  @override
  Future<OpOutcome> create(MealPlanEntry entry) async {
    await store.putMealPlanEntry(entry);
    return _applied;
  }

  @override
  Future<OpOutcome> update(MealPlanEntry entry) async {
    await store.putMealPlanEntry(entry);
    return _applied;
  }

  @override
  Future<OpOutcome> delete(MealPlanEntry entry) async {
    await store.deleteMealPlanEntry(entry.id);
    return _applied;
  }

  @override
  Future<void> fetchRange(DateTime start, DateTime end) async {}
}

class LocalShoppingRepository implements ShoppingRepository {
  LocalShoppingRepository(this.ref, this.engine);
  final Ref ref;
  final Engine engine;
  LocalStore get store => ref.read(localStoreProvider);

  Future<ShoppingList> _list(String id) async {
    final l = await store.shoppingList(id);
    if (l == null) throw ApiException(404, 'Shopping list not found', code: 'not_found');
    return l;
  }

  @override
  Future<ShoppingList> createList(String name) async {
    final now = DateTime.now().toUtc();
    final list = ShoppingList(id: newId(), name: name, createdAt: now, updatedAt: now, items: const []);
    await store.putShoppingList(list);
    return list;
  }

  @override
  Future<OpOutcome> renameList(ShoppingList list, String name) async {
    await store.putShoppingList(list.rename(name));
    return _applied;
  }

  @override
  Future<OpOutcome> deleteList(ShoppingList list) async {
    await store.deleteShoppingList(list.id);
    return _applied;
  }

  @override
  Future<OpOutcome> addItem(ShoppingList list, String text) async {
    final current = await _list(list.id);
    final items = current.items ?? const [];
    final pos = items.isEmpty ? 0 : items.map((e) => e.position).reduce((a, b) => a > b ? a : b) + 1;
    Json parsed;
    try {
      parsed = await engine.parseShoppingItem(text);
    } catch (_) {
      parsed = {'food': text, 'display': text};
    }
    final item = ShoppingItem.fromJson({
      ...parsed,
      'id': newId(),
      'list_id': list.id,
      'position': pos,
      'checked': false,
      'display': asString(parsed['display']) ?? text,
      'updated_at': DateTime.now().toUtc().toIso8601String(),
    });
    await store.putShoppingItem(item);
    return OpOutcome(OutcomeKind.applied, data: item.toJson());
  }

  @override
  Future<OpOutcome> updateItem(ShoppingItem updated, Map<String, Object?> fields) async {
    await store.putShoppingItem(updated);
    return _applied;
  }

  @override
  Future<OpOutcome> setChecked(ShoppingItem item, bool checked) =>
      updateItem(item.copyWith(checked: checked), {'checked': checked});

  @override
  Future<OpOutcome> deleteItem(ShoppingItem item) async {
    await store.deleteShoppingItem(item.listId, item.id);
    return _applied;
  }

  @override
  Future<OpOutcome> clearChecked(ShoppingList list) async {
    final current = await _list(list.id);
    await store.putShoppingList(current.withItems((current.items ?? const []).where((e) => !e.checked).toList()));
    return _applied;
  }

  Future<OpOutcome> _merge(String listId, List<Json> add) async {
    if (add.isEmpty) return const OpOutcome(OutcomeKind.applied);
    try {
      final list = await _list(listId);
      final merged = await engine.mergeShopping((list.items ?? const []).map((e) => e.toJson()).toList(), add);
      final items = [
        for (final j in merged)
          ShoppingItem.fromJson({...j, 'list_id': listId, if (asString(j['id']) == null) 'id': newId()}),
      ];
      final updated = list.withItems(items);
      await store.putShoppingList(updated);
      return OpOutcome(OutcomeKind.applied, data: updated.toJson());
    } on ApiException catch (e) {
      return OpOutcome(OutcomeKind.rejected, message: e.message);
    }
  }

  @override
  Future<OpOutcome> addRecipes(String listId, List<({String recipeId, double? servings})> recipes) async {
    final add = <Json>[];
    for (final r in recipes) {
      final recipe = await store.recipe(r.recipeId);
      if (recipe != null) add.add({'recipe': recipe.toJson(), 'servings': jsonNum(r.servings)});
    }
    return _merge(listId, add);
  }

  @override
  Future<OpOutcome> addMealPlan(String listId, DateTime start, DateTime end) async {
    final s = dateKey(start), e = dateKey(end);
    final entries = (await store.mealPlanEntries()).where(
      (m) => m.recipeId != null && m.date.compareTo(s) >= 0 && m.date.compareTo(e) <= 0,
    );
    return addRecipes(listId, [for (final m in entries) (recipeId: m.recipeId!, servings: m.servings)]);
  }

  @override
  Future<void> refreshList(String id) async {}
}

class LocalImportService implements ImportService {
  LocalImportService(this.ref, this.engine, this.paths, this.recipes);
  final Ref ref;
  final Engine engine;
  final LocalPaths paths;
  final LocalRecipeRepository recipes;
  LocalStore get store => ref.read(localStoreProvider);

  Json _llm() {
    final llm = ref.read(localPrefsProvider).llm;
    if (llm == null) {
      throw EngineException('Set up an AI provider in Settings to use AI imports.', code: 'llm_disabled');
    }
    return llm.toJson();
  }

  /// Like the server, previews always carry the suggested category (the
  /// user can change it in the editor).
  Future<ImportPreview> _preview(Json recipeInput, {String? imagePath}) async =>
      ImportPreview(Recipe.fromJson({...await recipes.autoCategorize(recipeInput), 'id': ''}), imagePath: imagePath);

  @override
  Future<ImportPreview> previewUrl(String url) async {
    final imported = await engine.importUrl(url, (await paths.tmp()).path);
    return _preview(asJson(imported['data']), imagePath: asString(imported['image_path']));
  }

  @override
  Future<Recipe> importUrl(String url) async {
    final imported = await engine.importUrl(url, (await paths.tmp()).path);
    return recipes.saveImported(imported);
  }

  @override
  Future<ImportPreview> previewText(String text, {bool useLlm = false}) async =>
      _preview(useLlm ? await engine.llmRecipeFromText(_llm(), text) : await engine.importText(text));

  @override
  Future<ImportPreview> previewImage(Uint8List bytes, String filename) async {
    final llm = _llm();
    final path = await paths.tmpFile(p.basename(filename.isEmpty ? 'recipe.jpg' : filename));
    await File(path).writeAsBytes(bytes, flush: true);
    try {
      return await _preview(await engine.llmRecipeFromImage(llm, path), imagePath: path);
    } catch (_) {
      unawaited(File(path).delete().then((_) {}, onError: (_) {}));
      rethrow;
    }
  }

  ImportJob _job(
    String id,
    String source,
    String status, {
    int total = 0,
    int imported = 0,
    int skipped = 0,
    int failed = 0,
    List<String> errors = const [],
    List<String> ids = const [],
    DateTime? created,
    bool finished = false,
  }) => ImportJob(
    id: id,
    source: source,
    status: status,
    total: total,
    imported: imported,
    skipped: skipped,
    failed: failed,
    errors: errors,
    recipeIds: ids,
    createdAt: created,
    finishedAt: finished ? DateTime.now().toUtc() : null,
  );

  static String? _cleanUrl(String? url) {
    final u = url?.trim();
    return u == null || u.isEmpty ? null : u;
  }

  /// Saves imported recipes one by one, emitting progress.
  Stream<ImportJob> _saveAll(
    String source,
    Future<List<Object?>> Function(List<String> errors) load,
    Future<Json> Function(Object? item) resolve, {
    required bool skipDuplicates,
    required bool categorize,
  }) async* {
    final id = newId();
    final created = DateTime.now().toUtc();
    final errors = <String>[];
    final ids = <String>[];
    var imported = 0, skipped = 0, failed = 0;
    yield _job(id, source, 'running', created: created);
    ImportJob finish(String status, int total) {
      final j = _job(
        id,
        source,
        status,
        total: total,
        imported: imported,
        skipped: skipped,
        failed: failed,
        errors: errors,
        ids: ids,
        created: created,
        finished: true,
      );
      unawaited(_remember(j));
      return j;
    }

    List<Object?> items;
    try {
      items = await load(errors);
    } catch (e) {
      errors.add(errorMessage(e));
      yield finish('failed', 0);
      return;
    }
    // Same rule as the server's `find_duplicate`: same source URL, or the
    // same name after accent-folding.
    final existing = DuplicateIndex();
    for (final r in await store.recipes()) {
      existing.add(r.id, name: r.name, sourceUrl: _cleanUrl(r.sourceUrl));
    }
    for (final item in items) {
      try {
        final imported0 = await resolve(item);
        final data = asJson(imported0['data']);
        final name = asStr(data['name']);
        final url = _cleanUrl(asString(data['source_url']));
        if (skipDuplicates && existing.find(name: name, sourceUrl: url) != null) {
          skipped++;
          final img = asString(imported0['image_path']);
          if (img != null) unawaited(File(img).delete().then((_) {}, onError: (_) {}));
        } else {
          final r = await recipes.saveImported(imported0, categorize: categorize);
          existing.add(r.id, name: r.name, sourceUrl: _cleanUrl(r.sourceUrl));
          ids.add(r.id);
          imported++;
        }
      } catch (e) {
        failed++;
        errors.add(errorMessage(e));
      }
      yield _job(
        id,
        source,
        'running',
        total: items.length,
        imported: imported,
        skipped: skipped,
        failed: failed,
        errors: [...errors],
        ids: [...ids],
        created: created,
      );
    }
    yield finish('done', items.length);
  }

  @override
  Stream<ImportJob> importFile(
    Uint8List bytes,
    String filename, {
    required String source,
    bool skipDuplicates = true,
    bool categorize = true,
  }) {
    var detected = source;
    String? tmpPath;
    return _saveAll(
      source,
      (errors) async {
        tmpPath = await paths.tmpFile(p.basename(filename));
        await File(tmpPath!).writeAsBytes(bytes, flush: true);
        final res = await engine.importFile(
          source: source,
          path: tmpPath!,
          filename: filename,
          imageDir: (await paths.tmp()).path,
        );
        detected = asStr(res['source'], source);
        errors.addAll(asStringList(res['errors']));
        unawaited(File(tmpPath!).delete().then((_) {}, onError: (_) {}));
        return asJsonList(res['recipes']);
      },
      (item) async => asJson(item),
      skipDuplicates: skipDuplicates,
      categorize: categorize,
    ).map((j) => j.source == detected ? j : ImportJob.fromJson({...j.toJson(), 'source': detected}));
  }

  @override
  Stream<ImportJob> importMealie(String baseUrl, String token, {bool skipDuplicates = true, bool categorize = true}) =>
      _saveAll(
        'mealie',
        (errors) async => await engine.mealieList(baseUrl, token),
        (slug) async => engine.mealieFetch(baseUrl, token, slug as String, (await paths.tmp()).path),
        skipDuplicates: skipDuplicates,
        categorize: categorize,
      );

  Future<void> _remember(ImportJob job) async {
    final list = await history();
    final next = [job, ...list.where((j) => j.id != job.id)].take(50).map((j) => j.toJson()).toList();
    await store.setMeta(MetaKeys.importHistory, next);
  }

  @override
  Future<List<ImportJob>> history() async =>
      asJsonList(await store.getMeta<List>(MetaKeys.importHistory)).map(ImportJob.fromJson).toList();
}

class LocalCookbookService implements CookbookService {
  LocalCookbookService(this.ref, this.engine, this.paths);
  final Ref ref;
  final Engine engine;
  final LocalPaths paths;
  LocalStore get store => ref.read(localStoreProvider);

  @override
  Future<List<CookbookTheme>> themes() async => (await engine.cookbookThemes()).map(CookbookTheme.fromJson).toList();

  Future<Uint8List> _readAndDelete(String path) async {
    final f = File(path);
    final bytes = await f.readAsBytes();
    unawaited(f.delete().then((_) {}, onError: (_) {}));
    return bytes;
  }

  String? _image(Recipe r) {
    final path = r.localImage('full');
    return path != null && File(path).existsSync() ? path : null;
  }

  @override
  Future<Uint8List> renderCookbook(CookbookOptions options) async {
    var all = await store.recipes();
    final ids = options.recipeIds;
    if (ids != null) {
      final wanted = ids.toSet();
      all = all.where((r) => wanted.contains(r.id)).toList();
    }
    final opts = options.toJson()..remove('recipe_ids');
    final out = await paths.tmpFile('cookbook.pdf');
    final res = await engine.renderCookbook(opts, [
      for (final r in all) {'id': r.id, 'data': r.toInput(includeVersion: false), 'image_path': _image(r)},
    ], out);
    return _readAndDelete(asStr(res['path'], out));
  }

  @override
  Future<Uint8List> exportBackup() async {
    final all = await store.recipes();
    final out = await paths.tmpFile('larder-backup.zip');
    final res = await engine.exportBackup([
      for (final r in all)
        {
          'id': r.id,
          'data': r.toInput(includeVersion: false),
          'image_path': _image(r),
          'favorite': r.isFavorite,
          'created_at': r.createdAt?.toUtc().toIso8601String(),
          'updated_at': r.updatedAt?.toUtc().toIso8601String(),
        },
    ], out);
    return _readAndDelete(asStr(res['path'], out));
  }
}
