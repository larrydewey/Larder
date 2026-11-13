import 'dart:convert';
import 'dart:typed_data';

import 'package:http/http.dart' as http;

import '../models/ai_settings.dart';
import '../models/cookbook.dart';
import '../models/household.dart';
import '../models/import_job.dart';
import '../models/json.dart';
import '../models/meal_plan.dart';
import '../models/organize.dart';
import '../models/recipe.dart';
import '../models/share.dart';
import '../models/shopping.dart';
import '../models/sync.dart';
import '../models/user.dart';
import 'api_client.dart';
import 'api_exception.dart';

/// Typed wrappers for the endpoints in docs/API.md.
extension LarderApi on ApiClient {
  // ---- config & auth -----------------------------------------------------
  Future<ServerConfig> config() async => ServerConfig.fromJson(asJson(await get('/api/config', auth: false)));

  Future<AuthResponse> login(String username, String password) async => AuthResponse.fromJson(
    asJson(await sendJson('POST', '/api/auth/login', body: {'username': username, 'password': password}, auth: false)),
  );

  Future<AuthResponse> register({
    required String username,
    required String password,
    String? displayName,
    String? email,
    String? inviteCode,
  }) async => AuthResponse.fromJson(
    asJson(
      await sendJson(
        'POST',
        '/api/auth/register',
        body: {
          'username': username,
          'password': password,
          if (displayName != null && displayName.isNotEmpty) 'display_name': displayName,
          if (email != null && email.isNotEmpty) 'email': email,
          if (inviteCode != null && inviteCode.isNotEmpty) 'invite_code': inviteCode,
        },
        auth: false,
      ),
    ),
  );

  Future<void> logout() async {
    final rt = refreshToken;
    if (rt != null) await sendJson('POST', '/api/auth/logout', body: {'refresh_token': rt});
  }

  // ---- users ------------------------------------------------------------
  Future<User> me() async => User.fromJson(asJson(await get('/api/users/me')));
  Future<User> updateMe(Json body) async => User.fromJson(asJson(await patch('/api/users/me', body)));

  /// Updates the profile. An empty [email] is sent as an explicit `null`,
  /// which the server treats as "clear the email".
  Future<User> updateProfile({required String displayName, String? email}) {
    final e = email?.trim();
    return updateMe({'display_name': displayName.trim(), 'email': e == null || e.isEmpty ? null : e});
  }

  /// Changes the password. Sends this device's refresh token so the server
  /// keeps this session (and revokes every other one); the stored tokens stay
  /// valid. The body is built per attempt: if the access token expires and
  /// the request is retried after a refresh, the rotated token is sent.
  Future<User> changePassword({String? currentPassword, required String newPassword}) async {
    final res = await send(() {
      final rt = refreshToken;
      return http.Request('PATCH', uri('/api/users/me'))
        ..headers['Accept'] = 'application/json'
        ..headers['Content-Type'] = 'application/json'
        ..body = jsonEncode({
          if (currentPassword != null && currentPassword.isNotEmpty) 'current_password': currentPassword,
          'new_password': newPassword,
          if (rt != null && rt.isNotEmpty) 'refresh_token': rt,
        });
    });
    return User.fromJson(asJson(ApiClient.decodeBody(res)));
  }

  // ---- household --------------------------------------------------------
  Future<Household> household() async => Household.fromJson(asJson(await get('/api/household')));
  Future<Household> renameHousehold(String name) async =>
      Household.fromJson(asJson(await patch('/api/household', {'name': name})));
  Future<List<Invite>> invites() async => asJsonList(await get('/api/household/invites')).map(Invite.fromJson).toList();
  Future<Invite> createInvite(int days) async =>
      Invite.fromJson(asJson(await post('/api/household/invites', {'expires_in_days': days})));
  Future<void> revokeInvite(String code) => delete('/api/household/invites/${Uri.encodeComponent(code)}');
  Future<Household> setMemberRole(String userId, String role) async =>
      Household.fromJson(asJson(await patch('/api/household/members/$userId', {'role': role})));
  Future<void> removeMember(String userId) => delete('/api/household/members/$userId');

  // ---- admin -----------------------------------------------------------
  Future<List<User>> adminUsers() async => asJsonList(await get('/api/admin/users')).map(User.fromJson).toList();
  Future<User> adminUpdateUser(String id, Json body) async =>
      User.fromJson(asJson(await patch('/api/admin/users/$id', body)));
  Future<void> adminDeleteUser(String id) => delete('/api/admin/users/$id');

  // --- server settings (admins) ---
  Future<AiSettings> aiSettings() async => AiSettings.fromJson(asJson(await get('/api/admin/settings/ai')));

  /// Saves the AI assistant settings. Leave [apiKey] null to keep the saved key (the server
  /// only keeps it for the same provider and address).
  Future<AiSettings> saveAiSettings({required String provider, String? apiKey, String? model, String? baseUrl}) async =>
      AiSettings.fromJson(
        asJson(
          await put('/api/admin/settings/ai', {
            'provider': provider,
            if (apiKey != null && apiKey.trim().isNotEmpty) 'api_key': apiKey.trim(),
            if (model != null && model.trim().isNotEmpty) 'model': model.trim(),
            if (baseUrl != null && baseUrl.trim().isNotEmpty) 'base_url': baseUrl.trim(),
          }),
        ),
      );
  Future<void> deleteAiSettings() => delete('/api/admin/settings/ai');

  /// Sends one small request with the saved settings; returns the model description.
  Future<String> testAiSettings() async => asStr(asJson(await post('/api/admin/settings/ai/test'))['model']);

  // ---- recipes ------------------------------------------------------------
  Future<Recipe> recipe(String idOrSlug) async => Recipe.fromJson(asJson(await get('/api/recipes/$idOrSlug')));
  Future<Recipe> duplicateRecipe(String id) async => Recipe.fromJson(asJson(await post('/api/recipes/$id/duplicate')));

  /// The recipe's public share link, or null if it has none.
  Future<RecipeShare?> recipeShare(String id) async {
    try {
      return RecipeShare.fromJson(asJson(await get('/api/recipes/$id/share')));
    } on ApiException catch (e) {
      if (e.isNotFound) return null;
      rethrow;
    }
  }

  /// Creates the public share link, or returns the existing one.
  Future<RecipeShare> createRecipeShare(String id) async =>
      RecipeShare.fromJson(asJson(await post('/api/recipes/$id/share')));
  Future<void> deleteRecipeShare(String id) => delete('/api/recipes/$id/share');
  Future<Recipe> uploadRecipeImage(String id, Uint8List bytes, String filename) async => Recipe.fromJson(
    asJson(
      await multipart(
        'PUT',
        '/api/recipes/$id/image',
        files: [UploadFile(field: 'file', bytes: bytes, filename: filename)],
      ),
    ),
  );
  Future<Recipe> deleteRecipeImage(String id) async => Recipe.fromJson(asJson(await delete('/api/recipes/$id/image')));
  Future<List<NameCount>> tags() async => asJsonList(await get('/api/tags')).map(NameCount.fromJson).toList();
  Future<List<NameCount>> categories() async =>
      asJsonList(await get('/api/categories')).map(NameCount.fromJson).toList();
  Future<List<Ingredient>> parseIngredients(List<String> lines, {bool useLlm = false}) async =>
      asJsonList(await post('/api/parse/ingredients', {'lines': lines, 'use_llm': useLlm}))
          .map(Ingredient.fromJson)
          .toList();

  /// Tidies ingredients and fills in categories (`POST /api/recipes/organize`).
  Future<OrganizeResult> organizeRecipes({
    required CategorizeMode categorize,
    required bool normalize,
    required bool dryRun,
  }) async => OrganizeResult.fromJson(
    asJson(
      await post('/api/recipes/organize', {'categorize': categorize.name, 'normalize': normalize, 'dry_run': dryRun}),
    ),
  );

  // ---- sync / meal plan / shopping -------------------------------------
  Future<SyncResponse> sync({String? since}) async =>
      SyncResponse.fromJson(asJson(await get('/api/sync', query: {'since': since})));
  Future<List<MealPlanEntry>> mealPlan(String start, String end) async =>
      asJsonList(await get('/api/mealplan', query: {'start': start, 'end': end})).map(MealPlanEntry.fromJson).toList();
  Future<ShoppingList> shoppingList(String id) async =>
      ShoppingList.fromJson(asJson(await get('/api/shopping-lists/$id')));

  // ---- import -------------------------------------------------------------
  /// Returns the raw recipe JSON (RecipeInput preview, or saved Recipe).
  Future<Json> importUrl(String url, {required bool save, String? status}) async =>
      asJson(await post('/api/import/url', {'url': url, 'save': save, 'status': ?status}));
  Future<Json> importText(String text, {required bool save, bool useLlm = false}) async =>
      asJson(await post('/api/import/text', {'text': text, 'save': save, 'use_llm': useLlm}));
  Future<Json> importImage(Uint8List bytes, String filename, {required bool save}) async => asJson(
    await multipart(
      'POST',
      '/api/import/image',
      fields: {'save': save.toString()},
      files: [UploadFile(field: 'file', bytes: bytes, filename: filename)],
    ),
  );
  Future<ImportJob> importFile(
    Uint8List bytes,
    String filename, {
    required String source,
    bool skipDuplicates = true,
    bool categorize = true,
    String? status,
  }) async => ImportJob.fromJson(
    asJson(
      await multipart(
        'POST',
        '/api/import/file',
        fields: {
          'source': source,
          'skip_duplicates': skipDuplicates.toString(),
          'categorize': categorize.toString(),
          'status': ?status,
        },
        files: [UploadFile(field: 'file', bytes: bytes, filename: filename)],
      ),
    ),
  );
  Future<ImportJob> importMealie(
    String baseUrl,
    String token, {
    bool skipDuplicates = true,
    bool categorize = true,
    String? status,
  }) async => ImportJob.fromJson(
    asJson(
      await post('/api/import/mealie', {
        'base_url': baseUrl,
        'token': token,
        'skip_duplicates': skipDuplicates,
        'categorize': categorize,
        'status': ?status,
      }),
    ),
  );
  Future<List<ImportJob>> importJobs() async =>
      asJsonList(await get('/api/import/jobs')).map(ImportJob.fromJson).toList();
  Future<ImportJob> importJob(String id) async => ImportJob.fromJson(asJson(await get('/api/import/jobs/$id')));

  // ---- export ---------------------------------------------------------
  Future<Uint8List> exportJson() => sendBytes('GET', '/api/export/json');
  Future<List<CookbookTheme>> cookbookThemes() async =>
      asJsonList(await get('/api/export/cookbook/themes')).map(CookbookTheme.fromJson).toList();
  Future<Uint8List> exportCookbook(CookbookOptions o) => sendBytes('POST', '/api/export/cookbook', body: o.toJson());
}
