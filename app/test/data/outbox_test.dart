import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:larder/api/api_client.dart';
import 'package:larder/data/app_mode.dart';
import 'package:larder/data/backend.dart';
import 'package:larder/data/local_store.dart';
import 'package:larder/data/outbox.dart';
import 'package:larder/data/outbox_op.dart';
import 'package:larder/data/providers.dart';
import 'package:larder/data/sync_controller.dart';
import 'package:larder/models/recipe.dart';

import '../support/fakes.dart';

OutboxOp op(String method, String path, [Object? body, String apply = ApplyKind.none]) =>
    OutboxOp(id: newId(), method: method, path: path, body: body, apply: apply);

Map<String, Object?> recipeJson(String id, String name, {int version = 1}) =>
    Recipe(id: id, name: name, version: version).toJson();

void main() {
  late LocalStore store;
  late FakeServer server;
  late ApiClient api;

  setUp(() async {
    store = await memoryStore();
    server = FakeServer();
    api = ApiClient(baseUrl: 'http://srv', httpClient: server.client, accessToken: 'a', refreshToken: 'r');
  });

  test('replays in order and empties the outbox; applies responses', () async {
    server.on('POST', '/api/recipes', (r) => json(recipeJson('r1', 'Soup'), 201));
    server.on('PUT', '/api/recipes/r1', (r) => json(recipeJson('r1', 'Soup 2', version: 2)));
    server.on('PUT', '/api/recipes/r1/favorite', (r) => http.Response('', 204));
    await store.addOp(op('POST', '/api/recipes', {'id': 'r1', 'name': 'Soup'}, ApplyKind.recipe));
    await store.addOp(op('PUT', '/api/recipes/r1', {'name': 'Soup 2', 'version': 1}, ApplyKind.recipe));
    await store.addOp(op('PUT', '/api/recipes/r1/favorite'));

    final res = await OutboxProcessor(store, api).flush();
    expect(server.log, ['POST /api/recipes', 'PUT /api/recipes/r1', 'PUT /api/recipes/r1/favorite']);
    expect(res.sent, 3);
    expect(await store.opCount(), 0);
    expect((await store.recipe('r1'))!.name, 'Soup 2');
    expect(server.requests.first.headers['authorization'], 'Bearer a');
  });

  test('stops on network error and keeps remaining ops in order', () async {
    var n = 0;
    server.on('POST', RegExp(r'^/api/mealplan$'), (r) {
      if (++n == 2) throw http.ClientException('connection reset');
      return json({'id': 'm$n', 'date': '2026-11-09', 'meal': 'dinner'}, 201);
    });
    for (var i = 0; i < 3; i++) {
      await store.addOp(op('POST', '/api/mealplan', {'id': 'm$i', 'date': '2026-11-09'}, ApplyKind.mealPlan));
    }
    final res = await OutboxProcessor(store, api).flush();
    expect(res.stoppedOffline, isTrue);
    expect(res.sent, 1);
    final left = await store.ops();
    expect(left.map((o) => (o.body as Map)['id']), ['m1', 'm2']);
  });

  test('replaying a create is idempotent (server returns 200 with existing)', () async {
    server.on('POST', '/api/shopping-lists', (r) => json({'id': 'l1', 'name': 'Weekly'}, 200));
    await store.addOp(op('POST', '/api/shopping-lists', {'id': 'l1', 'name': 'Weekly'}, ApplyKind.shoppingList));
    await store.addOp(op('POST', '/api/shopping-lists', {'id': 'l1', 'name': 'Weekly'}, ApplyKind.shoppingList));
    final res = await OutboxProcessor(store, api).flush();
    expect(res.sent, 2);
    expect(res.messages, isEmpty);
    expect((await store.shoppingList('l1'))!.name, 'Weekly');
  });

  test('4xx drops the op, surfaces a message and continues', () async {
    server.on('PUT', '/api/mealplan/bad', (r) => json({'error': 'invalid meal', 'code': 'bad_request'}, 400));
    server.on('DELETE', '/api/mealplan/gone', (r) => json({'error': 'nope', 'code': 'not_found'}, 404));
    server.on('DELETE', '/api/mealplan/ok', (r) => http.Response('', 204));
    await store.addOp(
      OutboxOp(id: 'x', method: 'PUT', path: '/api/mealplan/bad', body: {}, label: 'Updating meal plan'),
    );
    await store.addOp(op('DELETE', '/api/mealplan/gone'));
    await store.addOp(op('DELETE', '/api/mealplan/ok'));
    final res = await OutboxProcessor(store, api).flush();
    expect(res.outcomes['x']!.kind, OutcomeKind.rejected);
    expect(res.messages.single, contains('invalid meal'));
    expect(server.log.length, 3);
    expect(await store.opCount(), 0);
  });

  test('409 re-fetches the recipe (server wins)', () async {
    await store.putRecipe(Recipe(id: 'r1', name: 'Mine', version: 2));
    server.on('PUT', '/api/recipes/r1', (r) => json({'error': 'conflict', 'code': 'conflict'}, 409));
    server.on('GET', '/api/recipes/r1', (r) => json(recipeJson('r1', 'Theirs', version: 5)));
    await store.addOp(
      OutboxOp(id: 'c', method: 'PUT', path: '/api/recipes/r1', body: {'version': 1}, apply: ApplyKind.recipe),
    );
    final res = await OutboxProcessor(store, api).flush();
    expect(res.outcomes['c']!.kind, OutcomeKind.conflict);
    expect((await store.recipe('r1'))!.name, 'Theirs');
    expect(res.messages.single, contains('changed by someone else'));
  });

  test('409 body with "current" is used directly', () async {
    server.on(
      'PUT',
      '/api/recipes/r1',
      (r) => json({'error': 'conflict', 'code': 'conflict', 'current': recipeJson('r1', 'Current', version: 9)}, 409),
    );
    await store.addOp(op('PUT', '/api/recipes/r1', {'version': 1}, ApplyKind.recipe));
    await OutboxProcessor(store, api).flush();
    expect((await store.recipe('r1'))!.version, 9);
    expect(server.log, ['PUT /api/recipes/r1']);
  });

  group('repository + sync controller', () {
    late ProviderContainer c;
    setUp(() {
      c = ProviderContainer(
        overrides: [
          localStoreProvider.overrideWithValue(store),
          apiClientProvider.overrideWithValue(api),
          platformServicesEnabledProvider.overrideWithValue(false),
          bootstrapProvider.overrideWithValue(const BootstrapData(mode: AppMode.connected)),
        ],
      );
    });
    tearDown(() => c.dispose());

    test('offline save is optimistic + queued, then flushed when online', () async {
      server.offline = true;
      final repo = c.read(recipeRepositoryProvider);
      final o = await repo.save(Recipe(id: 'n1', name: 'Bread'), isNew: true);
      expect(o.kind, OutcomeKind.queued);
      expect((await store.recipe('n1'))!.name, 'Bread');
      expect(await store.opCount(), 1);
      expect(c.read(syncProvider).online, isFalse);

      server.offline = false;
      server.on('POST', '/api/recipes', (r) => json({...recipeJson('n1', 'Bread'), 'slug': 'bread'}, 201));
      await c.read(syncProvider.notifier).flush();
      expect(await store.opCount(), 0);
      expect((await store.recipe('n1'))!.slug, 'bread');
      expect((server.requests.single.body as Map)['id'], 'n1');
    });

    test('second offline edit carries the bumped version', () async {
      server.offline = true;
      await store.putRecipe(Recipe(id: 'r', name: 'A', version: 3));
      final repo = c.read(recipeRepositoryProvider);
      await repo.save((await store.recipe('r'))!.copyWith(name: 'B', version: () => 3), isNew: false);
      final after = (await store.recipe('r'))!;
      expect(after.version, 4);
      await repo.save(after.copyWith(name: 'C'), isNew: false);
      final ops = await store.ops();
      expect(ops.map((o) => (o.body as Map)['version']), [3, 4]);
    });
  });
}
