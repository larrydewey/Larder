import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../models/cookbook.dart';
import '../models/import_job.dart';
import '../models/meal_plan.dart';
import '../models/organize.dart';
import '../models/recipe.dart';
import '../models/shopping.dart';
import '../standalone/standalone.dart';
import 'app_mode.dart';
import 'outbox_op.dart';
import 'remote_backend.dart';

const _uuid = Uuid();
String newId() => _uuid.v4();

/// Recipe mutations. Reads always come from the local store
/// (`recipesProvider`), whatever the mode.
abstract class RecipeRepository {
  /// Creates (when [isNew]) or updates [recipe]. For updates `recipe.version`
  /// must be the version the edit was based on; [force] skips the
  /// concurrency check (overwrite after a conflict).
  Future<OpOutcome> save(Recipe recipe, {required bool isNew, bool force = false});
  Future<OpOutcome> delete(Recipe recipe);
  Future<OpOutcome> setFavorite(Recipe recipe, bool favorite);
  Future<Recipe> duplicate(String id);

  /// Replaces the recipe photo with [bytes].
  Future<Recipe> setImage(String id, Uint8List bytes, String filename);

  /// Attaches an image that is already a file on this device (e.g. one an
  /// import extracted). Returns null where unsupported.
  Future<Recipe?> setImageFromPath(String id, String path);
  Future<Recipe> removeImage(String id);

  /// Latest copy (server in connected mode) into the cache.
  Future<Recipe> refresh(String id);

  /// Tidies ingredient text and fills in categories for every recipe, with
  /// the server's rules (`POST /api/recipes/organize`). With [dryRun] nothing
  /// is saved and the result is a preview.
  Future<OrganizeResult> organize({required CategorizeMode categorize, required bool normalize, required bool dryRun});
}

abstract class MealPlanRepository {
  Future<OpOutcome> create(MealPlanEntry entry);
  Future<OpOutcome> update(MealPlanEntry entry);
  Future<OpOutcome> delete(MealPlanEntry entry);

  /// Makes sure entries for the range are cached (no-op on device).
  Future<void> fetchRange(DateTime start, DateTime end);
}

abstract class ShoppingRepository {
  Future<ShoppingList> createList(String name);
  Future<OpOutcome> renameList(ShoppingList list, String name);
  Future<OpOutcome> deleteList(ShoppingList list);

  /// Adds a free-text item (parsed by the server / engine).
  Future<OpOutcome> addItem(ShoppingList list, String text);

  /// Applies [updated] and persists only [fields].
  Future<OpOutcome> updateItem(ShoppingItem updated, Map<String, Object?> fields);
  Future<OpOutcome> setChecked(ShoppingItem item, bool checked);
  Future<OpOutcome> deleteItem(ShoppingItem item);
  Future<OpOutcome> clearChecked(ShoppingList list);

  /// Scales + merges recipe ingredients into the list.
  Future<OpOutcome> addRecipes(String listId, List<({String recipeId, double? servings})> recipes);
  Future<OpOutcome> addMealPlan(String listId, DateTime start, DateTime end);
  Future<void> refreshList(String id);
}

/// A parsed-but-unsaved recipe for the editor.
class ImportPreview {
  const ImportPreview(this.recipe, {this.imagePath});
  final Recipe recipe;

  /// Image already on this device (standalone imports).
  final String? imagePath;
}

abstract class ImportService {
  Future<ImportPreview> previewUrl(String url);
  Future<Recipe> importUrl(String url);
  Future<ImportPreview> previewText(String text, {bool useLlm = false});
  Future<ImportPreview> previewImage(Uint8List bytes, String filename);

  /// Bulk import; emits progress until the job is finished. With
  /// [categorize], recipes without a category get the suggested one.
  Stream<ImportJob> importFile(
    Uint8List bytes,
    String filename, {
    required String source,
    bool skipDuplicates = true,
    bool categorize = true,
  });
  Stream<ImportJob> importMealie(String baseUrl, String token, {bool skipDuplicates = true, bool categorize = true});
  Future<List<ImportJob>> history();
}

abstract class CookbookService {
  Future<List<CookbookTheme>> themes();

  /// Renders the PDF and returns its bytes.
  Future<Uint8List> renderCookbook(CookbookOptions options);

  /// The backup zip (same format as GET /api/export/json).
  Future<Uint8List> exportBackup();
}

class Backend {
  const Backend({
    required this.recipes,
    required this.mealPlan,
    required this.shopping,
    required this.imports,
    required this.cookbook,
  });
  final RecipeRepository recipes;
  final MealPlanRepository mealPlan;
  final ShoppingRepository shopping;
  final ImportService imports;
  final CookbookService cookbook;
}

/// The implementation for the current [AppMode].
final backendProvider = Provider<Backend>((ref) {
  final mode = ref.watch(appModeProvider);
  if (mode == AppMode.standalone) {
    final support = ref.watch(standaloneSupportProvider);
    if (support != null) return support.createBackend(ref);
  }
  return createRemoteBackend(ref);
});

final recipeRepositoryProvider = Provider<RecipeRepository>((ref) => ref.watch(backendProvider).recipes);
final mealPlanRepositoryProvider = Provider<MealPlanRepository>((ref) => ref.watch(backendProvider).mealPlan);
final shoppingRepositoryProvider = Provider<ShoppingRepository>((ref) => ref.watch(backendProvider).shopping);
final importServiceProvider = Provider<ImportService>((ref) => ref.watch(backendProvider).imports);
final cookbookServiceProvider = Provider<CookbookService>((ref) => ref.watch(backendProvider).cookbook);
