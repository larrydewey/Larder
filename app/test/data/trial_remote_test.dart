import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:larder/api/api_client.dart';
import 'package:larder/data/app_mode.dart';
import 'package:larder/data/backend.dart';
import 'package:larder/data/local_store.dart';
import 'package:larder/data/outbox_op.dart';
import 'package:larder/data/providers.dart';
import 'package:larder/data/sync_controller.dart';
import 'package:larder/models/meal_plan.dart';
import 'package:larder/models/recipe.dart';
import 'package:larder/models/trial.dart';

import '../support/fakes.dart';

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
  });

  tearDown(() => c.dispose());

  final trial = Recipe(id: 'r1', name: 'Miso Aubergine', status: RecipeStatus.trial, version: 2);

  test('a verdict is applied optimistically and replayed from the outbox when back online', () async {
    await store.putRecipe(trial);
    server.offline = true;
    final verdict = TrialVerdict(keep: false, rating: 4, note: 'too salty', date: DateTime(2026, 11, 12));
    final o = await c.read(recipeRepositoryProvider).recordTried(trial, verdict);
    expect(o.kind, OutcomeKind.queued);
    final local = (await store.recipe('r1'))!;
    expect((local.triedCount, local.rating, local.status, local.version), (1, 4, RecipeStatus.trial, 3));
    expect(local.notes, 'Nov 12: too salty');
    final op = (await store.ops()).single;
    expect((op.method, op.path), ('POST', '/api/recipes/r1/tried'));
    expect(op.body, verdict.toJson());

    // Back online: the server's counted copy replaces the optimistic one.
    server.offline = false;
    server.on(
      'POST',
      '/api/recipes/r1/tried',
      (r) => json({
        ...trial.toJson(),
        'tried_count': 3,
        'last_tried_at': (r.body as Map)['tried_at'],
        'rating': 4,
        'version': 5,
      }),
    );
    await c.read(syncProvider.notifier).flush();
    expect(await store.opCount(), 0);
    expect((await store.recipe('r1'))!.triedCount, 3, reason: 'verdicts from other devices count too');
    expect(server.log, ['POST /api/recipes/r1/tried']);
  });

  test('"Keep it" and "Move to cookbook" change the status; imports send the chosen status', () async {
    await store.putRecipe(trial);
    server.on(
      'POST',
      '/api/recipes/r1/tried',
      (r) => json({...trial.toJson(), 'status': 'cookbook', 'tried_count': 1}),
    );
    await c.read(recipeRepositoryProvider).recordTried(trial, TrialVerdict(keep: true));
    expect((await store.recipe('r1'))!.status, RecipeStatus.cookbook);
    expect((server.requests.single.body as Map)['outcome'], 'keep');

    server.on('PUT', '/api/recipes/r1', (r) => json({...trial.toJson(), ...(r.body as Map), 'id': 'r1'}));
    await c.read(recipeRepositoryProvider).save(trial.copyWith(status: RecipeStatus.cookbook), isNew: false);
    expect((server.requests.last.body as Map)['status'], 'cookbook');
    expect((server.requests.last.body as Map).containsKey('tried_count'), isFalse);

    server.on('POST', '/api/import/url', (r) => json({...trial.toJson(), 'id': 'r9'}));
    await c.read(importServiceProvider).importUrl('https://example.com/r');
    expect((server.requests.last.body as Map)['status'], 'trial', reason: 'single imports default to trial');
    await c.read(importServiceProvider).importUrl('https://example.com/r', status: RecipeStatus.cookbook);
    expect((server.requests.last.body as Map)['status'], 'cookbook');
  });

  test('answering "Did you make it?" sends trial_outcome with the meal plan entry', () async {
    final entry = MealPlanEntry(id: 'm1', date: '2026-11-10', recipeId: 'r1');
    expect(entry.toInput().containsKey('trial_outcome'), isFalse, reason: 'omitted = unchanged on the server');
    server.on('PUT', '/api/mealplan/m1', (r) => json({...entry.toJson(), ...(r.body as Map), 'id': 'm1'}));
    await c.read(mealPlanRepositoryProvider).update(entry.copyWith(trialOutcome: TrialOutcome.skipped));
    expect((server.requests.single.body as Map)['trial_outcome'], 'skipped');
    expect((await store.mealPlanEntry('m1'))!.trialOutcome, 'skipped');
  });
}
