import 'package:flutter_test/flutter_test.dart';
import 'package:larder/models/recipe.dart';
import 'package:larder/util/scaling.dart';
import 'package:larder/util/units.dart';

void main() {
  const flour = Ingredient(
    quantity: 1.5,
    unit: 'cup',
    food: 'flour',
    note: 'sifted',
    original: '1 1/2 cups flour, sifted',
  );
  const garlic = Ingredient(quantity: 2, quantityMax: 3, unit: 'clove', food: 'garlic', original: '2-3 cloves garlic');
  const salt = Ingredient(food: 'salt', original: 'salt to taste');

  test('scale factor', () {
    expect(scaleFactor(4, 8), 2);
    expect(scaleFactor(4, 2), 0.5);
    expect(scaleFactor(null, 3), 3);
  });

  test('unchanged shows original text', () {
    expect(displayIngredient(flour).toString(), '1 1/2 cups flour, sifted');
  });

  test('scaling formats nice fractions', () {
    expect(displayIngredient(flour, factor: 2).toString(), '3 cups flour, sifted');
    expect(displayIngredient(flour, factor: 0.5).toString(), '3/4 cup flour, sifted');
    expect(displayIngredient(garlic, factor: 2).toString(), '4–6 cloves garlic');
  });

  test('scaling + conversion', () {
    expect(displayIngredient(flour, factor: 2, system: UnitSystem.metric).toString(), '710 ml flour, sifted');
  });

  test('amount is split off like the printed cookbook (unicode fractions)', () {
    final d = displayIngredient(flour, factor: 1.75);
    expect(d.amount, '2⅝ cups');
    expect(d.text, 'flour, sifted');
    expect(d.toString(), '2 5/8 cups flour, sifted');
    final r = displayIngredient(garlic, factor: 0.5);
    expect((r.amount, r.text), ('1–1½ cloves', 'garlic'));
  });

  test('ingredients without quantity: Rust shows food + note, unparsed lines verbatim', () {
    expect(displayIngredient(salt, factor: 3).toString(), 'salt');
    expect(displayIngredient(salt, factor: 3).amount, '');
    const unparsed = Ingredient.unparsed('a pinch of love');
    expect(displayIngredient(unparsed, factor: 3, system: UnitSystem.metric).toString(), 'a pinch of love');
    const noFood = Ingredient(quantity: 2, unit: 'cup', note: 'divided', original: '2 cups, divided');
    final d = displayIngredient(noFood, system: UnitSystem.metric);
    expect(d.toString(), '475 ml, divided');
    expect((d.amount, d.text), ('475 ml', 'divided'));
  });

  test('groupBySection keeps order', () {
    final groups = groupBySection(['a', 'b', 'c'], (s) => s == 'a' ? null : 'X');
    expect(groups.map((e) => e.key), [null, 'X']);
    expect(groups[1].value, ['b', 'c']);
  });
}
