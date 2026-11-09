import 'dart:async';
import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/endpoints.dart';
import '../models/cookbook.dart';
import '../models/import_job.dart';
import '../models/json.dart';
import '../models/meal_plan.dart';
import '../models/organize.dart';
import '../models/recipe.dart';
import '../models/shopping.dart';
import 'backend.dart';
import 'local_store.dart';
import 'outbox_op.dart';
import 'providers.dart';
import 'sync_controller.dart';

/// Connected mode: optimistic local writes + outbox + server.
Backend createRemoteBackend(Ref ref) => Backend(
  recipes: RemoteRecipeRepository(ref),
  mealPlan: RemoteMealPlanRepository(ref),
  shopping: RemoteShoppingRepository(ref),
  imports: RemoteImportService(ref),
  cookbook: RemoteCookbookService(ref),
);

abstract class _Repo {
  _Repo(this.ref);
  final Ref ref;
  LocalStore get store => ref.read(localStoreProvider);
  SyncController get sync => ref.read(syncProvider.notifier);

  Future<OpOutcome> submit(
    String method,
    String path,
    Object? body, {
    String apply = ApplyKind.none,
    String label = '',
  }) => sync.submit(OutboxOp(id: newId(), method: method, path: path, body: body, apply: apply, label: label));
}

class RemoteRecipeRepository extends _Repo implements RecipeRepository {
  RemoteRecipeRepository(super.ref);

  @override
  Future<OpOutcome> save(Recipe recipe, {required bool isNew, bool force = false}) async {
    final now = DateTime.now().toUtc();
    final optimistic = recipe.copyWith(
      updatedAt: now,
      version: () => isNew ? recipe.version : (recipe.version == null ? null : recipe.version! + 1),
    );
    await store.putRecipe(optimistic);
    if (isNew) {
      return submit(
        'POST',
        '/api/recipes',
        recipe.toInput(includeVersion: false),
        apply: ApplyKind.recipe,
        label: 'Creating "${recipe.name}"',
      );
    }
    return submit(
      'PUT',
      '/api/recipes/${recipe.id}',
      recipe.toInput(includeVersion: !force),
      apply: ApplyKind.recipe,
      label: 'Saving "${recipe.name}"',
    );
  }

  @override
  Future<OpOutcome> delete(Recipe recipe) async {
    await store.deleteRecipe(recipe.id);
    return submit('DELETE', '/api/recipes/${recipe.id}', null, label: 'Deleting "${recipe.name}"');
  }

  @override
  Future<OpOutcome> setFavorite(Recipe recipe, bool favorite) async {
    await store.putRecipe(recipe.copyWith(isFavorite: favorite));
    return submit(
      favorite ? 'PUT' : 'DELETE',
      '/api/recipes/${recipe.id}/favorite',
      null,
      label: favorite ? 'Favoriting "${recipe.name}"' : 'Unfavoriting "${recipe.name}"',
    );
  }

  @override
  Future<Recipe> duplicate(String id) async {
    final r = await sync.online((api) => api.duplicateRecipe(id));
    await store.putRecipe(r);
    return r;
  }

  /// Requires connectivity. Pending changes are flushed first so the recipe
  /// exists on the server.
  @override
  Future<Recipe> setImage(String id, Uint8List bytes, String filename) async {
    await sync.flush();
    final r = await sync.online((api) => api.uploadRecipeImage(id, bytes, filename));
    await store.putRecipe(r);
    return r;
  }

  @override
  Future<Recipe> removeImage(String id) async {
    await sync.flush();
    final r = await sync.online((api) => api.deleteRecipeImage(id));
    await store.putRecipe(r);
    return r;
  }

  @override
  Future<Recipe?> setImageFromPath(String id, String path) async => null;

  @override
  Future<Recipe> refresh(String id) async {
    final r = await sync.online((api) => api.recipe(id));
    await store.putRecipe(r);
    return r;
  }

  @override
  Future<OrganizeResult> organize({
    required CategorizeMode categorize,
    required bool normalize,
    required bool dryRun,
  }) async {
    // Send pending edits first so the server works on (and the preview shows)
    // the latest recipes, and queued saves don't conflict with the result.
    await sync.flush();
    final result = await sync.online(
      (api) => api.organizeRecipes(categorize: categorize, normalize: normalize, dryRun: dryRun),
    );
    if (!dryRun && result.recipesChanged > 0) await sync.sync();
    return result;
  }
}

class RemoteMealPlanRepository extends _Repo implements MealPlanRepository {
  RemoteMealPlanRepository(super.ref);

  @override
  Future<OpOutcome> create(MealPlanEntry entry) async {
    await store.putMealPlanEntry(entry);
    return submit('POST', '/api/mealplan', entry.toInput(), apply: ApplyKind.mealPlan, label: 'Adding to meal plan');
  }

  @override
  Future<OpOutcome> update(MealPlanEntry entry) async {
    await store.putMealPlanEntry(entry);
    return submit(
      'PUT',
      '/api/mealplan/${entry.id}',
      entry.toInput(includeId: false),
      apply: ApplyKind.mealPlan,
      label: 'Updating meal plan',
    );
  }

  @override
  Future<OpOutcome> delete(MealPlanEntry entry) async {
    await store.deleteMealPlanEntry(entry.id);
    return submit('DELETE', '/api/mealplan/${entry.id}', null, label: 'Removing meal plan entry');
  }

  /// Loads a date range from the server (for weeks outside the sync window).
  @override
  Future<void> fetchRange(DateTime start, DateTime end) async {
    if (await store.opCount() > 0) return;
    final s = dateKey(start), e = dateKey(end);
    final entries = await sync.online((api) => api.mealPlan(s, e));
    await store.replaceMealPlan(entries, start: s, end: e);
  }
}

class RemoteShoppingRepository extends _Repo implements ShoppingRepository {
  RemoteShoppingRepository(super.ref);

  @override
  Future<ShoppingList> createList(String name) async {
    final now = DateTime.now().toUtc();
    final list = ShoppingList(id: newId(), name: name, createdAt: now, updatedAt: now, items: const []);
    await store.putShoppingList(list);
    await submit(
      'POST',
      '/api/shopping-lists',
      {'id': list.id, 'name': name},
      apply: ApplyKind.shoppingList,
      label: 'Creating list "$name"',
    );
    return list;
  }

  @override
  Future<OpOutcome> renameList(ShoppingList list, String name) async {
    await store.putShoppingList(list.rename(name));
    return submit(
      'PATCH',
      '/api/shopping-lists/${list.id}',
      {'name': name},
      apply: ApplyKind.shoppingList,
      label: 'Renaming list',
    );
  }

  @override
  Future<OpOutcome> deleteList(ShoppingList list) async {
    await store.deleteShoppingList(list.id);
    return submit('DELETE', '/api/shopping-lists/${list.id}', null, label: 'Deleting list "${list.name}"');
  }

  @override
  Future<OpOutcome> addItem(ShoppingList list, String text) async {
    final items = list.items ?? const [];
    final pos = items.isEmpty ? 0 : items.map((e) => e.position).reduce((a, b) => a > b ? a : b) + 1;
    final item = ShoppingItem(
      id: newId(),
      listId: list.id,
      food: text,
      display: text,
      position: pos,
      updatedAt: DateTime.now().toUtc(),
    );
    await store.putShoppingItem(item);
    return submit(
      'POST',
      '/api/shopping-lists/${list.id}/items',
      {'id': item.id, 'text': text},
      apply: ApplyKind.shoppingItem,
      label: 'Adding "$text"',
    );
  }

  /// Applies [updated] locally and PATCHes only [fields].
  @override
  Future<OpOutcome> updateItem(ShoppingItem updated, Map<String, Object?> fields) async {
    await store.putShoppingItem(updated);
    return submit(
      'PATCH',
      '/api/shopping-lists/${updated.listId}/items/${updated.id}',
      fields,
      apply: ApplyKind.shoppingItem,
      label: 'Updating "${updated.label}"',
    );
  }

  @override
  Future<OpOutcome> setChecked(ShoppingItem item, bool checked) =>
      updateItem(item.copyWith(checked: checked), {'checked': checked});

  @override
  Future<OpOutcome> deleteItem(ShoppingItem item) async {
    await store.deleteShoppingItem(item.listId, item.id);
    return submit(
      'DELETE',
      '/api/shopping-lists/${item.listId}/items/${item.id}',
      null,
      label: 'Removing "${item.label}"',
    );
  }

  @override
  Future<OpOutcome> clearChecked(ShoppingList list) async {
    await store.putShoppingList(list.withItems((list.items ?? const []).where((e) => !e.checked).toList()));
    return submit(
      'POST',
      '/api/shopping-lists/${list.id}/clear-checked',
      null,
      apply: ApplyKind.shoppingList,
      label: 'Clearing checked items',
    );
  }

  /// Server merges & scales; applied when online, queued otherwise.
  @override
  Future<OpOutcome> addRecipes(String listId, List<({String recipeId, double? servings})> recipes) => submit(
    'POST',
    '/api/shopping-lists/$listId/add-recipes',
    {
      'recipes': [
        for (final r in recipes) {'recipe_id': r.recipeId, if (r.servings != null) 'servings': jsonNum(r.servings)},
      ],
    },
    apply: ApplyKind.shoppingList,
    label: 'Adding recipe ingredients',
  );

  @override
  Future<OpOutcome> addMealPlan(String listId, DateTime start, DateTime end) => submit(
    'POST',
    '/api/shopping-lists/$listId/add-mealplan',
    {'start': dateKey(start), 'end': dateKey(end)},
    apply: ApplyKind.shoppingList,
    label: 'Adding meal plan to list',
  );

  @override
  Future<void> refreshList(String id) async {
    if (await store.opCount() > 0) return;
    final l = await sync.online((api) => api.shoppingList(id));
    await store.putShoppingList(l);
  }
}

class RemoteImportService extends _Repo implements ImportService {
  RemoteImportService(super.ref);

  ImportPreview _preview(Json j) => ImportPreview(Recipe.fromJson({...j, 'id': ''}));

  @override
  Future<ImportPreview> previewUrl(String url) async =>
      _preview(await sync.online((api) => api.importUrl(url, save: false)));

  @override
  Future<Recipe> importUrl(String url) async {
    final r = Recipe.fromJson(await sync.online((api) => api.importUrl(url, save: true)));
    await store.putRecipe(r);
    return r;
  }

  @override
  Future<ImportPreview> previewText(String text, {bool useLlm = false}) async =>
      _preview(await sync.online((api) => api.importText(text, save: false, useLlm: useLlm)));

  @override
  Future<ImportPreview> previewImage(Uint8List bytes, String filename) async =>
      _preview(await sync.online((api) => api.importImage(bytes, filename, save: false)));

  @override
  Stream<ImportJob> importFile(
    Uint8List bytes,
    String filename, {
    required String source,
    bool skipDuplicates = true,
    bool categorize = true,
  }) => _track(
    () => sync.online(
      (api) => api.importFile(bytes, filename, source: source, skipDuplicates: skipDuplicates, categorize: categorize),
    ),
  );

  @override
  Stream<ImportJob> importMealie(String baseUrl, String token, {bool skipDuplicates = true, bool categorize = true}) =>
      _track(
        () => sync.online(
          (api) => api.importMealie(baseUrl, token, skipDuplicates: skipDuplicates, categorize: categorize),
        ),
      );

  /// Starts a server job and polls GET /api/import/jobs/{id} until finished.
  Stream<ImportJob> _track(Future<ImportJob> Function() start) async* {
    var job = await start();
    yield job;
    var failures = 0;
    while (!job.isFinished) {
      await Future<void>.delayed(const Duration(milliseconds: 1500));
      try {
        job = await sync.online((api) => api.importJob(job.id));
        failures = 0;
        yield job;
      } catch (e) {
        if (++failures > 20) rethrow;
      }
    }
    unawaited(sync.sync());
  }

  @override
  Future<List<ImportJob>> history() => sync.online((api) => api.importJobs());
}

class RemoteCookbookService extends _Repo implements CookbookService {
  RemoteCookbookService(super.ref);

  @override
  Future<List<CookbookTheme>> themes() => sync.online((api) => api.cookbookThemes());

  @override
  Future<Uint8List> renderCookbook(CookbookOptions options) => sync.online((api) => api.exportCookbook(options));

  @override
  Future<Uint8List> exportBackup() => sync.online((api) => api.exportJson());
}
