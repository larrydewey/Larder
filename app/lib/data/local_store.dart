import 'dart:async';

import 'package:sembast/sembast.dart';

import '../models/json.dart';
import '../models/meal_plan.dart';
import '../models/recipe.dart';
import '../models/shopping.dart';
import 'outbox_op.dart';

/// Keys in the meta store.
abstract final class MetaKeys {
  static const serverUrl = 'server_url';
  static const accessToken = 'access_token';
  static const refreshToken = 'refresh_token';
  static const user = 'user';
  static const config = 'config';
  static const syncCursor = 'sync_cursor';
  static const lastSync = 'last_sync';
  static const themeMode = 'theme_mode';
  static const appMode = 'app_mode';
  static const localUnits = 'local_units';
  static const llmConfig = 'llm_config';
  static const importHistory = 'import_history';
  static const migratedIds = 'migrated_ids';
}

/// Local cache (sembast) for everything the app shows, plus the outbox.
class LocalStore {
  LocalStore(this.db);

  final Database db;

  static final _recipes = stringMapStoreFactory.store('recipes');
  static final _mealPlan = stringMapStoreFactory.store('meal_plan');
  static final _lists = stringMapStoreFactory.store('shopping_lists');
  static final _outbox = intMapStoreFactory.store('outbox');
  static final _meta = StoreRef<String, Object?>('meta');

  static Map<String, Object?> _plain(Json j) => deepJson(j) as Map<String, Object?>;

  // ---- meta ------------------------------------------------------------------
  Future<T?> getMeta<T>(String key) async {
    final v = await _meta.record(key).get(db);
    return v is T ? v : null;
  }

  Future<void> setMeta(String key, Object? value) async {
    if (value == null) {
      await _meta.record(key).delete(db);
    } else {
      await _meta.record(key).put(db, value is Map ? deepJson(value) : value);
    }
  }

  // ---- recipes -----------------------------------------------------------
  Stream<List<Recipe>> watchRecipes() =>
      _recipes.query().onSnapshots(db).map((snaps) => snaps.map((s) => Recipe.fromJson(asJson(s.value))).toList());

  Future<List<Recipe>> recipes() async =>
      (await _recipes.find(db)).map((s) => Recipe.fromJson(asJson(s.value))).toList();

  Future<Recipe?> recipe(String id) async {
    final v = await _recipes.record(id).get(db);
    return v == null ? null : Recipe.fromJson(asJson(v));
  }

  Future<void> putRecipe(Recipe r) => _recipes.record(r.id).put(db, _plain(r.toJson()));

  Future<void> deleteRecipe(String id) => _recipes.record(id).delete(db);

  /// Applies a sync payload: full replaces everything, otherwise upserts +
  /// deletes.
  Future<void> applyRecipes(List<Recipe> recipes, List<String> deletedIds, {required bool full}) async {
    await db.transaction((txn) async {
      if (full) await _recipes.delete(txn);
      for (final r in recipes) {
        await _recipes.record(r.id).put(txn, _plain(r.toJson()));
      }
      for (final id in deletedIds) {
        await _recipes.record(id).delete(txn);
      }
    });
  }

  // ---- meal plan ---------------------------------------------------------
  Stream<List<MealPlanEntry>> watchMealPlan() => _mealPlan
      .query()
      .onSnapshots(db)
      .map((snaps) => snaps.map((s) => MealPlanEntry.fromJson(asJson(s.value))).toList());

  Future<List<MealPlanEntry>> mealPlanEntries() async =>
      (await _mealPlan.find(db)).map((s) => MealPlanEntry.fromJson(asJson(s.value))).toList();

  Future<List<ShoppingList>> shoppingLists() async =>
      (await _lists.find(db)).map((s) => ShoppingList.fromJson(asJson(s.value))).toList();

  Future<MealPlanEntry?> mealPlanEntry(String id) async {
    final v = await _mealPlan.record(id).get(db);
    return v == null ? null : MealPlanEntry.fromJson(asJson(v));
  }

  Future<void> putMealPlanEntry(MealPlanEntry e) => _mealPlan.record(e.id).put(db, _plain(e.toJson()));

  Future<void> deleteMealPlanEntry(String id) => _mealPlan.record(id).delete(db);

  /// Replaces all entries whose date is within [start]..[end] (inclusive,
  /// `YYYY-MM-DD`) with [entries]. With null bounds everything is replaced.
  Future<void> replaceMealPlan(List<MealPlanEntry> entries, {String? start, String? end}) async {
    await db.transaction((txn) async {
      if (start == null || end == null) {
        await _mealPlan.delete(txn);
      } else {
        await _mealPlan.delete(
          txn,
          finder: Finder(
            filter: Filter.and([Filter.greaterThanOrEquals('date', start), Filter.lessThanOrEquals('date', end)]),
          ),
        );
      }
      for (final e in entries) {
        await _mealPlan.record(e.id).put(txn, _plain(e.toJson()));
      }
    });
  }

  // ---- shopping lists ------------------------------------------------------
  Stream<List<ShoppingList>> watchShoppingLists() =>
      _lists.query().onSnapshots(db).map((snaps) => snaps.map((s) => ShoppingList.fromJson(asJson(s.value))).toList());

  Future<ShoppingList?> shoppingList(String id) async {
    final v = await _lists.record(id).get(db);
    return v == null ? null : ShoppingList.fromJson(asJson(v));
  }

  /// Upserts a list. When [list] has no items, existing cached items are kept.
  Future<void> putShoppingList(ShoppingList list) async {
    await db.transaction((txn) async {
      var toStore = list;
      if (list.items == null) {
        final existing = await _lists.record(list.id).get(txn);
        if (existing != null) {
          final old = ShoppingList.fromJson(asJson(existing));
          if (old.items != null) {
            toStore = ShoppingList(
              id: list.id,
              name: list.name,
              createdAt: list.createdAt,
              updatedAt: list.updatedAt,
              itemCount: list.itemCount,
              checkedCount: list.checkedCount,
              items: old.items,
            );
          }
        }
      }
      await _lists.record(list.id).put(txn, _plain(toStore.toJson()));
    });
  }

  Future<void> deleteShoppingList(String id) => _lists.record(id).delete(db);

  Future<void> replaceShoppingLists(List<ShoppingList> lists) async {
    await db.transaction((txn) async {
      await _lists.delete(txn);
      for (final l in lists) {
        await _lists.record(l.id).put(txn, _plain(l.toJson()));
      }
    });
  }

  /// Inserts or replaces one item inside its list (recomputing counts).
  Future<void> putShoppingItem(ShoppingItem item) => _updateItems(item.listId, (items) {
    final i = items.indexWhere((e) => e.id == item.id);
    if (i >= 0) {
      items[i] = item;
    } else {
      items.add(item);
    }
    return items;
  });

  Future<void> deleteShoppingItem(String listId, String itemId) =>
      _updateItems(listId, (items) => items..removeWhere((e) => e.id == itemId));

  Future<void> _updateItems(String listId, List<ShoppingItem> Function(List<ShoppingItem>) change) async {
    await db.transaction((txn) async {
      final raw = await _lists.record(listId).get(txn);
      if (raw == null) return;
      final list = ShoppingList.fromJson(asJson(raw));
      final items = change([...?list.items]);
      await _lists.record(listId).put(txn, _plain(list.withItems(items).toJson()));
    });
  }

  // ---- outbox --------------------------------------------------------------
  Future<int> addOp(OutboxOp op) => _outbox.add(db, _plain(op.toJson()));

  Future<List<OutboxOp>> ops() async {
    final snaps = await _outbox.find(db, finder: Finder(sortOrders: [SortOrder(Field.key)]));
    return snaps.map((s) => OutboxOp.fromJson(asJson(s.value), key: s.key)).toList();
  }

  Future<void> removeOp(int key) => _outbox.record(key).delete(db);

  Future<void> updateOp(OutboxOp op) => _outbox.record(op.key!).put(db, _plain(op.toJson()));

  Future<int> opCount() => _outbox.count(db);

  Future<void> clearOutbox() => _outbox.delete(db);

  Stream<int> watchOpCount() => _outbox.query().onCount(db);

  // ---- wipe ----------------------------------------------------------------
  /// Removes all cached data (used on logout / server change). Keeps theme.
  Future<void> clearData() async {
    await db.transaction((txn) async {
      await _recipes.delete(txn);
      await _mealPlan.delete(txn);
      await _lists.delete(txn);
      await _outbox.delete(txn);
      for (final k in [MetaKeys.syncCursor, MetaKeys.lastSync, MetaKeys.user]) {
        await _meta.record(k).delete(txn);
      }
    });
  }
}
