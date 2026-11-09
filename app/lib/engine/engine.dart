/// Interface to the on-device Rust engine (docs/ENGINE.md).
///
/// Platform neutral: the FFI implementation lives in `ffi_engine.dart` and
/// is only imported on native platforms; tests use a fake.
library;

import '../api/api_exception.dart';
import '../models/json.dart';

/// An engine call failed (`{"error": ..., "code": ...}`).
///
/// Extends [ApiException] so UI error handling is shared with server mode.
class EngineException extends ApiException {
  EngineException(String message, {String code = 'internal'}) : super(_status(code), message, code: code);

  static int _status(String code) => switch (code) {
    'bad_request' || 'llm_disabled' => 400,
    'unprocessable' => 422,
    _ => 500,
  };
}

/// The engine library could not be loaded (not bundled / wrong ABI).
class EngineUnavailableException implements Exception {
  EngineUnavailableException(this.cause);
  final Object cause;

  @override
  String toString() => 'The on-device engine is not available in this build ($cause).';
}

abstract class Engine {
  /// Calls [method] with [args]; returns the decoded `ok` value or throws
  /// [EngineException].
  Future<Object?> call(String method, Map<String, Object?> args);
}

/// Unwraps a raw engine response envelope.
Object? unwrapEngineResponse(Object? decoded) {
  final j = asJson(decoded);
  if (j.containsKey('ok')) return j['ok'];
  throw EngineException(asStr(j['error'], 'Engine error'), code: asStr(j['code'], 'internal'));
}

/// Typed helpers for every engine method the app uses.
extension EngineMethods on Engine {
  Future<String> version() async => asStr(asJson(await call('version', {}))['version']);

  Future<List<Json>> parseIngredients(List<String> lines) async =>
      asJsonList(await call('parse_ingredients', {'lines': lines}));

  /// Returns the normalised RecipeInput plus `slug` and `search_text`.
  Future<Json> normalizeRecipe(Json recipeInput) async => asJson(await call('normalize_recipe', recipeInput));

  /// Suggested category per recipe (Rust `categorize::suggest_category`):
  /// recipe id -> category, or null when nothing fits. Each recipe is a
  /// RecipeInput that carries its `id`.
  Future<Map<String, String?>> suggestCategories(List<Json> recipes) async {
    if (recipes.isEmpty) return const {};
    final res = asJson(await call('suggest_categories', {'recipes': recipes}));
    return {for (final s in asJsonList(res['suggestions'])) asStr(s['id']): asString(s['category'])};
  }

  Future<Json> parseShoppingItem(String text) async => asJson(await call('parse_shopping_item', {'text': text}));

  Future<String> classifyAisle(String food) async =>
      asStr(asJson(await call('classify_aisle', {'food': food}))['aisle'], 'Other');

  /// [add] entries: `{"recipe": Recipe, "servings": n|null}`.
  Future<List<Json>> mergeShopping(List<Json> items, List<Json> add) async =>
      asJsonList(asJson(await call('merge_shopping', {'items': items, 'add': add}))['items']);

  /// Returns `{thumb, medium, full}` JPEG paths.
  Future<Map<String, String>> processImage(String inputPath, String outputDir, String id) async =>
      asStringMap(await call('process_image', {'input_path': inputPath, 'output_dir': outputDir, 'id': id}));

  Future<Json> importFile({
    required String source,
    required String path,
    required String filename,
    required String imageDir,
  }) async =>
      asJson(await call('import_file', {'source': source, 'path': path, 'filename': filename, 'image_dir': imageDir}));

  Future<Json> importUrl(String url, String imageDir) async =>
      asJson(await call('import_url', {'url': url, 'image_dir': imageDir}));

  Future<Json> importText(String text) async => asJson(await call('import_text', {'text': text}));

  Future<List<String>> mealieList(String baseUrl, String token) async =>
      asStringList(asJson(await call('mealie_list', {'base_url': baseUrl, 'token': token}))['slugs']);

  Future<Json> mealieFetch(String baseUrl, String token, String slug, String imageDir) async =>
      asJson(await call('mealie_fetch', {'base_url': baseUrl, 'token': token, 'slug': slug, 'image_dir': imageDir}));

  Future<Json> llmRecipeFromText(Json llm, String text) async =>
      asJson(await call('llm_recipe_from_text', {'llm': llm, 'text': text}));

  Future<Json> llmRecipeFromImage(Json llm, String imagePath) async =>
      asJson(await call('llm_recipe_from_image', {'llm': llm, 'image_path': imagePath}));

  Future<List<Json>> cookbookThemes() async => asJsonList(await call('cookbook_themes', {}));

  Future<Json> renderCookbook(Json options, List<Json> recipes, String outPath) async =>
      asJson(await call('render_cookbook', {'options': options, 'recipes': recipes, 'out_path': outPath}));

  Future<Json> exportBackup(List<Json> recipes, String outPath) async =>
      asJson(await call('export_backup', {'recipes': recipes, 'out_path': outPath}));
}
