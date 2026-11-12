import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import 'data/app_mode.dart';
import 'data/auth_controller.dart';
import 'data/backend.dart';
import 'features/auth/login_screen.dart';
import 'features/auth/oidc_callback_screen.dart';
import 'features/cook/cook_mode_screen.dart';
import 'features/cookbook/cookbook_screen.dart';
import 'features/editor/recipe_editor_screen.dart';
import 'features/import/import_screen.dart';
import 'features/mealplan/meal_plan_screen.dart';
import 'features/organize/organize_screen.dart';
import 'features/recipes/recipe_detail_screen.dart';
import 'features/recipes/recipe_list_screen.dart';
import 'features/settings/admin_users_screen.dart';
import 'features/settings/ai_settings_screen.dart';
import 'features/settings/household_screen.dart';
import 'features/settings/settings_screen.dart';
import 'features/setup/migration_screen.dart';
import 'features/setup/server_setup_screen.dart';
import 'features/setup/welcome_screen.dart';
import 'features/shell/app_shell.dart';
import 'features/shopping/shopping_list_screen.dart';
import 'features/shopping/shopping_lists_screen.dart';
import 'features/trial/trial_screen.dart';
import 'models/recipe.dart';
import 'widgets/snack.dart';

/// Bridges auth state changes to go_router's refreshListenable.
class _AuthListenable extends ChangeNotifier {
  void notify() => notifyListeners();
}

final routerProvider = Provider<GoRouter>((ref) {
  final listenable = _AuthListenable();
  ref.listen(authProvider.select((a) => a.status), (_, _) => listenable.notify());
  ref.listen(appModeProvider, (_, _) => listenable.notify());
  ref.onDispose(listenable.dispose);

  final router = GoRouter(
    navigatorKey: rootNavigatorKey,
    initialLocation: '/recipes',
    refreshListenable: listenable,
    redirect: (context, state) =>
        appRedirect(ref.read(appModeProvider), ref.read(authProvider).status, state.matchedLocation),
    routes: [
      GoRoute(path: '/', redirect: (_, _) => '/recipes'),
      GoRoute(path: '/welcome', builder: (_, _) => const WelcomeScreen()),
      GoRoute(path: '/setup-server', builder: (_, _) => const ServerSetupScreen()),
      // Connect-later flow (standalone -> connected).
      GoRoute(
        path: '/connect',
        builder: (_, _) => const ServerSetupScreen(migrate: true),
        routes: [
          GoRoute(path: 'login', builder: (_, _) => const LoginScreen(migrate: true)),
          GoRoute(path: 'migrate', builder: (_, _) => const MigrationScreen()),
        ],
      ),
      GoRoute(path: '/login', builder: (_, _) => const LoginScreen()),
      GoRoute(path: '/oidc-callback', builder: (_, _) => const OidcCallbackScreen()),
      GoRoute(path: '/cookbook', builder: (_, _) => const CookbookScreen()),
      GoRoute(path: '/organize', builder: (_, _) => const OrganizeScreen()),
      StatefulShellRoute.indexedStack(
        builder: (context, state, shell) => AppShell(shell: shell),
        branches: [
          StatefulShellBranch(
            routes: [
              GoRoute(path: '/recipes', builder: (_, _) => const RecipeListScreen(), routes: _recipeRoutes('/recipes')),
            ],
          ),
          StatefulShellBranch(
            routes: [GoRoute(path: '/trial', builder: (_, _) => const TrialScreen(), routes: _recipeRoutes('/trial'))],
          ),
          StatefulShellBranch(
            routes: [GoRoute(path: '/mealplan', builder: (_, _) => const MealPlanScreen())],
          ),
          StatefulShellBranch(
            routes: [
              GoRoute(
                path: '/shopping',
                builder: (_, _) => const ShoppingListsScreen(),
                routes: [
                  GoRoute(
                    path: ':id',
                    builder: (_, state) => ShoppingListScreen(id: state.pathParameters['id']!),
                  ),
                ],
              ),
            ],
          ),
          StatefulShellBranch(
            routes: [GoRoute(path: '/import', builder: (_, _) => const ImportScreen())],
          ),
          StatefulShellBranch(
            routes: [
              GoRoute(
                path: '/settings',
                builder: (_, _) => const SettingsScreen(),
                routes: [
                  GoRoute(path: 'household', builder: (_, _) => const HouseholdScreen()),
                  GoRoute(path: 'admin', builder: (_, _) => const AdminUsersScreen()),
                  GoRoute(path: 'ai', builder: (_, _) => const AiSettingsScreen()),
                ],
              ),
            ],
          ),
        ],
      ),
    ],
  );
  ref.onDispose(router.dispose);
  return router;
});

/// New / detail / cook / edit routes below a list ([base] is `/recipes` or
/// `/trial`), so a recipe opened from the trial list stays in that tab.
List<RouteBase> _recipeRoutes(String base) => [
  GoRoute(
    path: 'new',
    parentNavigatorKey: rootNavigatorKey,
    builder: (_, state) {
      final extra = state.extra;
      if (extra is ImportPreview) {
        return RecipeEditorScreen(draft: extra.recipe, draftImagePath: extra.imagePath, base: base);
      }
      // A recipe started from the trial list is a recipe to try.
      final draft = extra is Recipe
          ? extra
          : (base == '/trial' ? const Recipe(id: '', name: '', status: RecipeStatus.trial) : null);
      return RecipeEditorScreen(draft: draft, base: base);
    },
  ),
  GoRoute(
    path: ':id',
    builder: (_, state) => RecipeDetailScreen(id: state.pathParameters['id']!, base: base),
    routes: [
      GoRoute(
        path: 'cook',
        parentNavigatorKey: rootNavigatorKey,
        builder: (_, state) => CookModeScreen(
          id: state.pathParameters['id']!,
          servings: double.tryParse(state.uri.queryParameters['servings'] ?? ''),
          base: base,
        ),
      ),
      GoRoute(
        path: 'edit',
        parentNavigatorKey: rootNavigatorKey,
        builder: (_, state) => RecipeEditorScreen(id: state.pathParameters['id'], base: base),
      ),
    ],
  ),
];

/// Route guard (pure, unit tested).
String? appRedirect(AppMode mode, AuthStatus status, String location) {
  if (location == '/oidc-callback') return null;
  const entry = {'/welcome', '/login', '/setup-server'};
  final connectFlow = location == '/connect' || location.startsWith('/connect/');
  switch (mode) {
    case AppMode.unset:
      return location == '/welcome' ? null : '/welcome';
    case AppMode.standalone:
      return entry.contains(location) ? '/recipes' : null;
    case AppMode.connected:
      if (connectFlow || location == '/welcome') return status == AuthStatus.signedIn ? '/recipes' : '/login';
  }
  switch (status) {
    case AuthStatus.needsServer:
      return location == '/setup-server' ? null : '/setup-server';
    case AuthStatus.signedOut:
      return (location == '/login' || location == '/setup-server') ? null : '/login';
    case AuthStatus.signedIn:
      return (location == '/login' || location == '/setup-server') ? '/recipes' : null;
  }
}
