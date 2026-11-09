// Runs against the real engine only when LARDER_ENGINE_LIB points to a built
// liblarder_engine.so (tool/build_engine.sh --host). Skipped otherwise.
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:larder/engine/engine.dart';
import 'package:larder/engine/ffi_engine.dart';
import 'package:larder/models/recipe.dart';
import 'package:larder/util/categories.dart';
import 'package:larder/util/scaling.dart';
import 'package:larder/util/units.dart';

import '../util/units_test.dart' show displayCases;

void main() {
  final lib = Platform.environment['LARDER_ENGINE_LIB'];
  final skip = (lib == null || !File(lib).existsSync()) ? 'LARDER_ENGINE_LIB not set' : null;

  group('real engine', () {
    late FfiEngine engine;
    setUp(() => engine = FfiEngine(libraryPath: lib));

    test('version', () async => expect(await engine.version(), isNotEmpty));

    test('parse + normalize', () async {
      final parsed = await engine.parseIngredients(['2 cups flour']);
      expect(parsed.single['food'], isNotNull);
      final norm = await engine.normalizeRecipe({
        'name': ' Crème Brûlée ',
        'ingredients': [
          {'original': '1 cup cream'},
        ],
        'instructions': [],
      });
      expect(norm['slug'], isNotEmpty);
      expect(norm['search_text'], contains('creme'));
    });

    test('Dart ingredient formatting is identical to the engine (Rust)', () async {
      for (final (q, qmax, unit, food, note, original, scale, system, _) in displayCases) {
        final ing = Ingredient(quantity: q, quantityMax: qmax, unit: unit, food: food, note: note, original: original);
        final res = await engine.call('format_ingredient', {
          'ingredient': ing.toJson(),
          'scale': scale,
          'units': system.name,
        });
        expect(ingredientDisplay(ing, scale, system), (res as Map)['text'], reason: '$original x$scale ${system.name}');
      }
      // Parsed lines too, including aliases such as "cc" (now millilitres).
      final lines = [
        '1 1/2 cups flour, sifted',
        '2 T sugar',
        '1 t salt',
        '250 cc milk',
        '5 cl rum',
        '2 dl cream',
        '500 mg saffron',
        '3 lbs. potatoes',
        '1 C. stock',
        '2-3 cloves garlic',
        '1 fl. oz lime juice',
      ];
      final parsed = await engine.parseIngredients(lines);
      for (final p in parsed) {
        final ing = Ingredient.fromJson(p);
        for (final system in UnitSystem.values) {
          for (final scale in [1.0, 2.0, 0.5, 1.75]) {
            final res = await engine.call('format_ingredient', {'ingredient': p, 'scale': scale, 'units': system.name});
            expect(ingredientDisplay(ing, scale, system), (res as Map)['text'], reason: '${ing.original} x$scale');
          }
        }
      }
      expect(canonicalUnit('cc'), 'ml');
      expect(Ingredient.fromJson(parsed[3]).unit, 'ml');
    });

    test('suggest_categories: same category list as the app, suggestions from it', () async {
      final res = await engine.call('suggest_categories', {
        'recipes': [
          {'id': 'a', 'name': 'Chocolate Chip Cookies', 'ingredients': [], 'instructions': []},
          {'id': 'b', 'name': 'Tomato Soup', 'ingredients': [], 'instructions': []},
          {'id': 'c', 'name': 'Qwxz', 'ingredients': [], 'instructions': []},
        ],
      });
      expect((res as Map)['categories'], larderCategories);
      final byId = await engine.suggestCategories([
        {'id': 'a', 'name': 'Chocolate Chip Cookies'},
        {'id': 'b', 'name': 'Tomato Soup'},
      ]);
      expect(byId['a'], 'Desserts');
      expect(byId['b'], 'Soups & Stews');
    });

    test('errors become EngineException', () async {
      await expectLater(engine.call('no_such_method', {}), throwsA(isA<EngineException>()));
    });
  }, skip: skip);
}
