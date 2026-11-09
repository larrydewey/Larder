import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:larder/api/api_client.dart';
import 'package:larder/api/api_exception.dart';
import 'package:larder/data/local_store.dart';
import 'package:larder/data/migration.dart';
import 'package:larder/models/meal_plan.dart';
import 'package:larder/models/recipe.dart';
import 'package:larder/models/shopping.dart';

import '../support/fakes.dart';

void main() {
  late LocalStore store;
  late FakeServer server;
  late Migrator migrator;

  setUp(() async {
    store = await memoryStore();
    server = FakeServer();
    final api = ApiClient(baseUrl: 'http://srv', httpClient: server.client, accessToken: 'a', refreshToken: 'r');
    migrator = Migrator(store: store, api: api, readFile: (p) async => Uint8List.fromList([1, 2, 3]));
    await store.putRecipe(Recipe(id: 'r1', name: 'Soup', isFavorite: true, localImages: const {'full': '/img/r1.jpg'}));
    await store.putRecipe(Recipe(id: 'r2', name: 'Bread'));
    await store.putMealPlanEntry(const MealPlanEntry(id: 'm1', date: '2026-11-10', recipeId: 'r2'));
    await store.putShoppingList(
      const ShoppingList(
        id: 'l1',
        name: 'Weekly',
        items: [ShoppingItem(id: 'i1', listId: 'l1', food: 'milk', display: 'milk', checked: true)],
      ),
    );
    server.on(
      'GET',
      '/api/sync',
      (r) => json({
        'cursor': 'c',
        'full': true,
        'recipes': [
          {'id': 'server-bread', 'name': 'bread'},
        ],
      }),
    );
    server.on('POST', RegExp(r'.*'), (r) => json(r.body ?? {}, 201));
    server.on('PUT', RegExp(r'.*'), (r) => json({}, 200));
    server.on('PATCH', RegExp(r'.*'), (r) => json({}, 200));
  });

  test('uploads everything with existing ids, remapping skipped duplicates', () async {
    final progress = <String>[];
    final report = await migrator.run(skipExistingNames: true, onProgress: (p) => progress.add(p.phase));
    expect(report.recipes, 1);
    expect(report.skipped, 1);
    expect(report.images, 1);
    expect(server.log, [
      'GET /api/sync',
      'POST /api/recipes',
      'PUT /api/recipes/r1/favorite',
      'PUT /api/recipes/r1/image',
      'POST /api/mealplan',
      'POST /api/shopping-lists',
      'POST /api/shopping-lists/l1/items',
      'PATCH /api/shopping-lists/l1/items/i1',
    ]);
    final recipeBody = server.requests[1].body as Map;
    expect(recipeBody['id'], 'r1');
    expect(recipeBody.containsKey('local_images'), isFalse);
    expect((server.requests[4].body as Map)['recipe_id'], 'server-bread');
    expect((server.requests[4].body as Map)['id'], 'm1');
    expect((server.requests[6].body as Map)['id'], 'i1');
    expect(progress.last, 'Done');
    expect(await store.getMeta<List>(MetaKeys.migratedIds), isNull);
  });

  test('skip-existing uses the server duplicate rule: source URL or folded name', () async {
    await store.putRecipe(Recipe(id: 'r3', name: 'Crème Brûlée'));
    await store.putRecipe(Recipe(id: 'r4', name: 'My stew', sourceUrl: 'https://ex.com/stew'));
    await store.putRecipe(Recipe(id: 'r5', name: 'Pie', sourceUrl: 'https://ex.com/pie'));
    server.handlers.insert(
      0,
      (r) => r.method == 'GET' && r.path == '/api/sync'
          ? json({
              'cursor': 'c',
              'full': true,
              'recipes': [
                {'id': 's-creme', 'name': '  CREME   brulee'},
                {'id': 's-stew', 'name': 'Beef stew', 'source_url': 'https://ex.com/stew'},
                {'id': 's-pie', 'name': 'Apple pie', 'source_url': 'https://ex.com/apple-pie'},
                // Uploaded by an earlier, interrupted run: not a duplicate.
                {'id': 'r1', 'name': 'Soup'},
              ],
            })
          : null,
    );
    final report = await migrator.run(skipExistingNames: true);
    final posted = server.requests
        .where((r) => r.method == 'POST' && r.path == '/api/recipes')
        .map((r) => (r.body as Map)['id'])
        .toList();
    expect(posted, unorderedEquals(['r1', 'r2', 'r5']));
    expect(report.skipped, 2);
  });

  test('resumes after a network failure without re-sending finished steps', () async {
    var failMealPlan = true;
    server.handlers.insert(0, (r) {
      if (r.path == '/api/mealplan' && failMealPlan) throw http.ClientException('down');
      return null;
    });
    await expectLater(migrator.run(), throwsA(isA<NetworkException>()));
    expect(await store.getMeta<List>(MetaKeys.migratedIds), containsAll(['r:r1', 'r:r2']));

    failMealPlan = false;
    server.requests.clear();
    await migrator.run();
    expect(server.log.where((l) => l == 'POST /api/recipes'), isEmpty);
    expect(server.log, contains('POST /api/mealplan'));
  });

  test('4xx on one item becomes a warning', () async {
    server.handlers.insert(
      0,
      (r) => r.path == '/api/shopping-lists/l1/items'
          ? http.Response(
              '{"error":"bad item","code":"bad_request"}',
              400,
              headers: {'content-type': 'application/json'},
            )
          : null,
    );
    final report = await migrator.run();
    expect(report.warnings.single, contains('bad item'));
  });
}
