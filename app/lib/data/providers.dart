import 'package:material_ui/material_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_client.dart';
import '../models/meal_plan.dart';
import '../models/recipe.dart';
import '../models/shopping.dart';
import '../models/user.dart';
import 'app_mode.dart';
import 'local_store.dart';

/// Values read from the local database before the app starts.
class BootstrapData {
  const BootstrapData({
    this.serverUrl,
    this.user,
    this.config,
    this.themeMode = ThemeMode.system,
    this.mode = AppMode.unset,
    this.localUnits = 'original',
    this.llm,
  });
  final AppMode mode;
  final String localUnits;
  final Map<String, Object?>? llm;
  final String? serverUrl;
  final User? user;
  final ServerConfig? config;
  final ThemeMode themeMode;
}

/// Overridden in main() / tests.
final localStoreProvider = Provider<LocalStore>((ref) => throw UnimplementedError('localStoreProvider'));
final apiClientProvider = Provider<ApiClient>((ref) => throw UnimplementedError('apiClientProvider'));
final bootstrapProvider = Provider<BootstrapData>((ref) => const BootstrapData());

/// Whether network/connectivity plugins should be used (disabled in tests).
final platformServicesEnabledProvider = Provider<bool>((ref) => true);

// ---- cached data streams ---------------------------------------------------

final recipesProvider = StreamProvider<List<Recipe>>((ref) => ref.watch(localStoreProvider).watchRecipes());

final recipeProvider = Provider.family<AsyncValue<Recipe?>, String>((ref, id) {
  return ref.watch(recipesProvider).whenData((list) {
    for (final r in list) {
      if (r.id == id || (r.slug.isNotEmpty && r.slug == id)) return r;
    }
    return null;
  });
});

final mealPlanProvider = StreamProvider<List<MealPlanEntry>>((ref) => ref.watch(localStoreProvider).watchMealPlan());

final shoppingListsProvider = StreamProvider<List<ShoppingList>>(
  (ref) => ref.watch(localStoreProvider).watchShoppingLists(),
);

final shoppingListProvider = Provider.family<AsyncValue<ShoppingList?>, String>((ref, id) {
  return ref.watch(shoppingListsProvider).whenData((lists) {
    for (final l in lists) {
      if (l.id == id) return l;
    }
    return null;
  });
});

final pendingOpsProvider = StreamProvider<int>((ref) => ref.watch(localStoreProvider).watchOpCount());

/// All tags / categories found in the cached recipes, most used first.
final localTagsProvider = Provider<List<String>>((ref) {
  final recipes = ref.watch(recipesProvider).value ?? const [];
  return _rank(recipes.expand((r) => r.tags));
});

final localCategoriesProvider = Provider<List<String>>((ref) {
  final recipes = ref.watch(recipesProvider).value ?? const [];
  return _rank(recipes.expand((r) => r.categories));
});

List<String> _rank(Iterable<String> names) {
  final counts = <String, int>{};
  final display = <String, String>{};
  for (final n in names) {
    final k = n.toLowerCase();
    counts[k] = (counts[k] ?? 0) + 1;
    display.putIfAbsent(k, () => n);
  }
  final keys = counts.keys.toList()
    ..sort((a, b) {
      final c = counts[b]!.compareTo(counts[a]!);
      return c != 0 ? c : a.compareTo(b);
    });
  return keys.map((k) => display[k]!).toList();
}
