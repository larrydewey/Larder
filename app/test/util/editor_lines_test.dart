import 'package:flutter_test/flutter_test.dart';
import 'package:larder/features/editor/editor_lines.dart';
import 'package:larder/models/recipe.dart';

void main() {
  const flour = Ingredient(quantity: 1, unit: 'cup', food: 'flour', original: '1 cup flour');
  const sugar = Ingredient(section: 'Sauce', quantity: 2, unit: 'tbsp', food: 'sugar', original: '2 tbsp sugar');

  test('ingredients <-> lines with sections', () {
    final lines = ingredientsToLines([flour, sugar]);
    expect(lines, ['1 cup flour', '## Sauce', '2 tbsp sugar']);
  });

  test('unchanged lines keep parse; changed lines become unparsed', () {
    final out = linesToIngredients(['1 cup flour', '## Sauce', '2 tbsp sugar', '3 eggs'], [flour, sugar]);
    expect(out[0].parsed, isTrue);
    expect(out[1].food, 'sugar');
    expect(out[1].section, 'Sauce');
    expect(out[2].parsed, isFalse);
    expect(out[2].section, 'Sauce');
    expect(out[2].toApiJson(), {'original': '3 eggs', 'section': 'Sauce'});
  });

  test('moving a line to another section keeps parse', () {
    final out = linesToIngredients(['## Dough', '1 cup flour'], [flour]);
    expect(out.single.parsed, isTrue);
    expect(out.single.section, 'Dough');
  });

  test('instructions with sections', () {
    final steps = linesToInstructions(['Mix', '# Bake', 'Bake it', '']);
    expect(steps.map((s) => s.section), [null, 'Bake']);
    expect(instructionsToLines(steps), ['Mix', '## Bake', 'Bake it']);
  });
}
