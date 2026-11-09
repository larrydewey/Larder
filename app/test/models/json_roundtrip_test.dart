import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:larder/models/cookbook.dart';
import 'package:larder/models/import_job.dart';
import 'package:larder/models/meal_plan.dart';
import 'package:larder/models/recipe.dart';
import 'package:larder/models/shopping.dart';
import 'package:larder/models/sync.dart';
import 'package:larder/models/user.dart';

const recipeJson = {
  'id': '6f1c1d4e-0000-4000-8000-000000000001',
  'household_id': 'h1',
  'slug': 'classic-pancakes',
  'name': 'Classic Pancakes',
  'description': '',
  'yield_quantity': 4,
  'yield_unit': 'servings',
  'prep_time_minutes': 10,
  'cook_time_minutes': 15,
  'total_time_minutes': 25,
  'source_url': null,
  'source_name': null,
  'notes': '',
  'rating': 4,
  'ingredients': [
    {
      'section': null,
      'quantity': 1.5,
      'quantity_max': null,
      'unit': 'cup',
      'food': 'flour',
      'note': 'sifted',
      'original': '1 1/2 cups flour, sifted',
    },
  ],
  'instructions': [
    {'section': null, 'text': 'Mix.'},
  ],
  'tags': ['breakfast', 'quick'],
  'categories': ['Breakfast'],
  'nutrition': {'calories': '250 kcal'},
  'image_url': '/api/images/abc',
  'is_favorite': false,
  'created_by': 'u1',
  'created_at': '2026-11-09T12:34:56.123456Z',
  'updated_at': '2026-11-09T12:34:56.123456Z',
  'version': 3,
  'some_future_field': {'x': 1},
};

void main() {
  test('Recipe round-trip', () {
    final r = Recipe.fromJson(Map<String, dynamic>.from(recipeJson));
    expect(r.name, 'Classic Pancakes');
    expect(r.yieldQuantity, 4);
    expect(r.ingredients.single.quantity, 1.5);
    expect(r.ingredients.single.parsed, isTrue);
    expect(r.version, 3);
    final again = Recipe.fromJson(jsonDecode(jsonEncode(r.toJson())) as Map<String, dynamic>);
    expect(again.toJson(), r.toJson());
  });

  test('Recipe tolerates missing / mistyped fields', () {
    final r = Recipe.fromJson({
      'id': 'x',
      'name': 'Bare',
      'yield_quantity': '2',
      'rating': 4.0,
      'instructions': ['Step'],
    });
    expect(r.yieldQuantity, 2);
    expect(r.rating, 4);
    expect(r.yieldUnit, 'servings');
    expect(r.instructions.single.text, 'Step');
    expect(r.tags, isEmpty);
  });

  test('RecipeInput: unparsed lines send only original + section; ints stay ints', () {
    final r = Recipe(
      id: 'id',
      name: 'X',
      yieldQuantity: 4,
      version: 7,
      ingredients: const [Ingredient.unparsed('2 eggs', section: 'Batter')],
      localImages: const {'thumb': '/x.jpg'},
      searchText: 'x',
    );
    final input = r.toInput();
    expect(input['ingredients'], [
      {'original': '2 eggs', 'section': 'Batter'},
    ]);
    expect(jsonEncode(input['yield_quantity']), '4');
    expect(input['version'], 7);
    expect(input.containsKey('local_images'), isFalse);
    expect(input.containsKey('search_text'), isFalse);
    expect(r.toInput(includeVersion: false).containsKey('version'), isFalse);
    // Local-only fields survive the cache round-trip.
    final cached = Recipe.fromJson(r.toJson());
    expect(cached.localImages['thumb'], '/x.jpg');
    expect(cached.ingredients.single.parsed, isFalse);
    expect(cached.localImage('medium'), '/x.jpg');
  });

  test('MealPlanEntry / ShoppingList / ImportJob / User round-trips', () {
    final e = MealPlanEntry.fromJson({
      'id': 'm1',
      'date': '2026-11-09',
      'meal': 'dinner',
      'recipe_id': 'r1',
      'recipe_name': 'Lasagna',
      'servings': 4,
      'updated_at': '2026-11-09T00:00:00Z',
    });
    expect(MealPlanEntry.fromJson(e.toJson()).toJson(), e.toJson());
    expect(e.toInput()['servings'], 4);

    final l = ShoppingList.fromJson({
      'id': 'l1',
      'name': 'Weekly',
      'item_count': 1,
      'checked_count': 0,
      'items': [
        {
          'id': 'i1',
          'list_id': 'l1',
          'food': 'flour',
          'quantity': 2.5,
          'unit': 'cup',
          'display': '2 1/2 cups flour',
          'aisle': 'Baking',
          'checked': false,
          'position': 0,
          'recipe_ids': ['r1'],
        },
      ],
    });
    expect(ShoppingList.fromJson(l.toJson()).toJson(), l.toJson());
    expect(l.items!.single.label, '2 1/2 cups flour');
    expect(ShoppingList.fromJson({'id': 'l2', 'name': 'x'}).items, isNull);

    final j = ImportJob.fromJson({
      'id': 'j',
      'source': 'mealie',
      'status': 'running',
      'total': 10,
      'imported': 4,
      'skipped': 1,
      'failed': 0,
    });
    expect(j.progress, 0.5);
    expect(ImportJob.fromJson(j.toJson()).toJson(), j.toJson());

    final u = User.fromJson({
      'id': 'u',
      'username': 'alice',
      'display_name': 'Alice',
      'preferences': {'units': 'metric'},
      'household_role': 'owner',
    });
    expect(u.units, 'metric');
    expect(u.isOwner, isTrue);
    expect(User.fromJson(u.toJson()).toJson(), u.toJson());

    final cfg = ServerConfig.fromJson({
      'version': '0.1.0',
      'setup_required': true,
      'oidc': {'enabled': true, 'name': 'Authentik'},
    });
    expect(cfg.oidcName, 'Authentik');
    expect(ServerConfig.fromJson(cfg.toJson()).toJson(), cfg.toJson());
  });

  test('SyncResponse parses', () {
    final s = SyncResponse.fromJson({
      'cursor': 'c1',
      'full': true,
      'recipes': [recipeJson],
      'deleted_recipe_ids': [],
      'meal_plan': [],
      'shopping_lists': [],
    });
    expect(s.full, isTrue);
    expect(s.recipes.single.slug, 'classic-pancakes');
  });

  test('CookbookOptions omits empty optionals', () {
    final o = CookbookOptions(title: 'Book', subtitle: ' ');
    final j = o.toJson();
    expect(j.containsKey('subtitle'), isFalse);
    expect(j['recipe_ids'], isNull);
    expect(j['theme'], 'classic');
  });
}
