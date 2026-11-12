import 'package:flutter_test/flutter_test.dart';
import 'package:larder/features/recipes/recipe_query.dart';
import 'package:larder/models/recipe.dart';
import 'package:larder/util/text_fold.dart';

void main() {
  final recipes = [
    Recipe(
      id: '1',
      name: 'Crème Brûlée',
      tags: const ['dessert'],
      categories: const ['Desserts'],
      rating: 5,
      isFavorite: true,
    ),
    Recipe(
      id: '2',
      name: 'Pancakes',
      tags: const ['breakfast', 'quick'],
      categories: const ['Breakfast'],
      rating: 3,
      totalTimeMinutes: 20,
      ingredients: const [Ingredient(food: 'flour', original: '1 cup flour')],
    ),
    Recipe(id: '3', name: 'Apple pie', tags: const ['dessert'], categories: const ['Desserts'], totalTimeMinutes: 90),
  ];

  List<String> ids(RecipeQuery q) => applyRecipeQuery(recipes, q).map((r) => r.id).toList();

  test('fold', () => expect(foldText('Crème BRÛLÉE Ærø'), 'creme brulee aero'));

  test('accent-insensitive multi-term search', () {
    expect(ids(const RecipeQuery(text: 'creme brulee')), ['1']);
    expect(ids(const RecipeQuery(text: 'flour pan')), ['2']);
    expect(ids(const RecipeQuery(text: 'dessert')), ['3', '1']);
  });

  test('engine search_text is used when present', () {
    final r = Recipe(id: '9', name: 'X', searchText: 'secret ingredient');
    expect(applyRecipeQuery([r], const RecipeQuery(text: 'secret')), hasLength(1));
  });

  test('trial recipes stay out of the list but show up in searches', () {
    final all = [
      ...recipes,
      Recipe(id: 't', name: 'Miso Aubergine', tags: const ['dessert'], status: RecipeStatus.trial),
    ];
    List<String> q(RecipeQuery query, {bool? includeTrial}) =>
        applyRecipeQuery(all, query, includeTrial: includeTrial).map((r) => r.id).toList();
    expect(q(const RecipeQuery()), isNot(contains('t')));
    expect(q(const RecipeQuery(tags: {'dessert'})), isNot(contains('t')));
    expect(q(const RecipeQuery(text: 'aubergine')), ['t']);
    expect(q(const RecipeQuery(), includeTrial: true), contains('t'));
    expect(q(const RecipeQuery(text: 'aubergine'), includeTrial: false), isEmpty);
  });

  test('filters and sort', () {
    expect(ids(const RecipeQuery(favoritesOnly: true)), ['1']);
    expect(ids(const RecipeQuery(tags: {'Dessert'}, sort: RecipeSort.nameDesc)), ['1', '3']);
    expect(ids(const RecipeQuery(categories: {'breakfast'})), ['2']);
    expect(ids(const RecipeQuery(sort: RecipeSort.rating)), ['1', '2', '3']);
    expect(ids(const RecipeQuery(sort: RecipeSort.quickest)), ['2', '3', '1']);
  });
}
