import 'json.dart';
import 'meal_plan.dart';
import 'recipe.dart';
import 'shopping.dart';

class SyncResponse {
  const SyncResponse({
    required this.cursor,
    required this.full,
    this.recipes = const [],
    this.deletedRecipeIds = const [],
    this.mealPlan = const [],
    this.shoppingLists = const [],
  });

  final String cursor;
  final bool full;
  final List<Recipe> recipes;
  final List<String> deletedRecipeIds;
  final List<MealPlanEntry> mealPlan;
  final List<ShoppingList> shoppingLists;

  factory SyncResponse.fromJson(Json j) => SyncResponse(
    cursor: asStr(j['cursor']),
    full: asBool(j['full']),
    recipes: asJsonList(j['recipes']).map(Recipe.fromJson).toList(),
    deletedRecipeIds: asStringList(j['deleted_recipe_ids']),
    mealPlan: asJsonList(j['meal_plan']).map(MealPlanEntry.fromJson).toList(),
    shoppingLists: asJsonList(j['shopping_lists']).map(ShoppingList.fromJson).toList(),
  );
}
