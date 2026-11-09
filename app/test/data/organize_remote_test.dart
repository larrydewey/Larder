import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:larder/api/api_client.dart';
import 'package:larder/data/app_mode.dart';
import 'package:larder/data/backend.dart';
import 'package:larder/data/local_store.dart';
import 'package:larder/data/providers.dart';
import 'package:larder/models/organize.dart';
import 'package:larder/models/recipe.dart';

import '../support/fakes.dart';

final organizeResponse = {
  'dry_run': true,
  'recipes_changed': 2,
  'categorized': 1,
  'normalized': 2,
  'changes': [
    {
      'id': 'r1',
      'name': 'Soup',
      'category_before': null,
      'category_after': 'Soups & Stews',
      'ingredients_changed': true,
    },
    {
      'id': 'r2',
      'name': 'Toast',
      'category_before': 'Breakfast',
      'category_after': 'Breakfast',
      'ingredients_changed': true,
    },
  ],
};

void main() {
  late LocalStore store;
  late FakeServer server;
  late ProviderContainer c;

  setUp(() async {
    store = await memoryStore();
    server = FakeServer();
    final api = ApiClient(baseUrl: 'http://srv', httpClient: server.client, accessToken: 'a', refreshToken: 'r');
    c = ProviderContainer(
      overrides: [
        localStoreProvider.overrideWithValue(store),
        apiClientProvider.overrideWithValue(api),
        platformServicesEnabledProvider.overrideWithValue(false),
        bootstrapProvider.overrideWithValue(const BootstrapData(mode: AppMode.connected)),
      ],
    );
    server.on(
      'POST',
      '/api/recipes/organize',
      (r) => json({...organizeResponse, 'dry_run': (r.body as Map)['dry_run']}),
    );
    server.on(
      'GET',
      '/api/sync',
      (r) => json({
        'cursor': 'c1',
        'full': true,
        'recipes': [
          Recipe(id: 'r1', name: 'Soup', categories: const ['Soups & Stews'], version: 2).toJson(),
        ],
      }),
    );
  });
  tearDown(() => c.dispose());

  test('preview posts dry_run and does not sync', () async {
    final res = await c
        .read(recipeRepositoryProvider)
        .organize(categorize: CategorizeMode.uncategorized, normalize: true, dryRun: true);
    expect(server.requests.single.body, {'categorize': 'uncategorized', 'normalize': true, 'dry_run': true});
    expect((res.recipesChanged, res.categorized, res.normalized), (2, 1, 2));
    expect(res.byCategory.map((g) => g.key), ['Soups & Stews', 'Breakfast']);
    expect(res.changes[1].categoryChanged, isFalse);
    expect(res.summary, '2 recipes changed: 1 recipe categorized, 2 recipes with tidied ingredients');
  });

  test('apply posts dry_run false, then syncs', () async {
    await c.read(recipeRepositoryProvider).organize(categorize: CategorizeMode.all, normalize: false, dryRun: false);
    expect(server.log, ['POST /api/recipes/organize', 'GET /api/sync']);
    expect(server.requests.first.body, {'categorize': 'all', 'normalize': false, 'dry_run': false});
    expect((await store.recipe('r1'))!.categories, ['Soups & Stews']);
  });

  test('imports send the categorize option', () async {
    server.on(
      'POST',
      '/api/import/file',
      (r) => json({'id': 'j1', 'source': 'auto', 'status': 'done', 'finished_at': '2026-01-01T00:00:00Z'}),
    );
    server.on(
      'POST',
      '/api/import/mealie',
      (r) => json({'id': 'j2', 'source': 'mealie', 'status': 'done', 'finished_at': '2026-01-01T00:00:00Z'}),
    );
    final imports = c.read(importServiceProvider);
    await imports.importFile(Uint8List.fromList([1]), 'x.zip', source: 'auto', categorize: false).first;
    final multipart = utf8.decode(server.requests.last.body! as List<int>, allowMalformed: true);
    expect(multipart, contains('name="categorize"\r\n\r\nfalse'));
    await imports.importFile(Uint8List.fromList([1]), 'x.zip', source: 'auto').first;
    expect(
      utf8.decode(server.requests.last.body! as List<int>, allowMalformed: true),
      contains('name="categorize"\r\n\r\ntrue'),
    );
    await imports.importMealie('https://m', 't', categorize: false).first;
    expect((server.requests.last.body as Map)['categorize'], false);
    await imports.importMealie('https://m', 't').first;
    expect((server.requests.last.body as Map)['categorize'], true);
  });
}
