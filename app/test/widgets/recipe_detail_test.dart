import 'package:flutter_test/flutter_test.dart';
import 'package:larder/features/recipes/recipe_detail_screen.dart';
import 'package:larder/models/recipe.dart';
import 'package:material_ui/material_ui.dart';

import '../support/app_harness.dart';
import '../support/fakes.dart';

String? label(WidgetTester tester) => tester.widget<Text>(find.byKey(const Key('servings-label'))).data;

void main() {
  testWidgets('servings scaler scales quantities and units toggle converts', (tester) async {
    tester.view.physicalSize = const Size(800, 2400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final store = await tester.runAsync(memoryStore);
    await tester.runAsync(
      () => store!.putRecipe(
        Recipe(
          id: 'r1',
          name: 'Pancakes',
          yieldQuantity: 4,
          ingredients: const [
            Ingredient(quantity: 1.5, unit: 'cup', food: 'flour', original: '1 1/2 cups flour'),
            Ingredient(food: 'salt', note: 'to taste', original: 'salt, to taste'),
          ],
          instructions: const [Instruction(text: 'Rest the batter for 10 minutes.')],
        ),
      ),
    );
    await tester.pumpWidget(wrap(const RecipeDetailScreen(id: 'r1'), testOverrides(store!, FakeServer())));
    for (var i = 0; i < 5; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 50)));
      await tester.pump(const Duration(milliseconds: 50));
    }

    expect(find.text('Pancakes'), findsWidgets);
    expect(label(tester), '4 servings');
    expect(find.text('1 1/2 cups flour', findRichText: true), findsOneWidget);
    expect(find.text('10 minutes'), findsOneWidget); // detected timer chip

    await tester.tap(find.byKey(const Key('servings-plus')));
    await tester.pump();
    await tester.tap(find.byKey(const Key('servings-plus')));
    await tester.pump();
    await tester.tap(find.byKey(const Key('servings-plus')));
    await tester.pump();
    await tester.tap(find.byKey(const Key('servings-plus')));
    await tester.pump();
    expect(label(tester), '8 servings');
    expect(find.text('3 cups flour', findRichText: true), findsOneWidget);
    expect(find.text('salt, to taste', findRichText: true), findsOneWidget);

    await tester.tap(find.byKey(const Key('servings-minus')));
    await tester.pump();
    expect(find.text('2⅝ cups flour', findRichText: true), findsOneWidget); // as in the printed cookbook

    await tester.tap(find.text('Metric'));
    await tester.pump();
    expect(find.text('620 ml flour', findRichText: true), findsOneWidget); // Rust round_metric
  });
}
