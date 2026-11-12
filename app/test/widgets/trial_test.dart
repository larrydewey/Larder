import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:larder/data/local_store.dart';
import 'package:larder/features/cook/cook_mode_screen.dart';
import 'package:larder/features/recipes/recipe_detail_screen.dart';
import 'package:larder/features/trial/verdict_sheet.dart';
import 'package:larder/models/recipe.dart';
import 'package:larder/theme.dart';
import 'package:material_ui/material_ui.dart';

import '../support/app_harness.dart';
import '../support/fakes.dart';

final trial = Recipe(
  id: 'r1',
  name: 'Miso Aubergine',
  status: RecipeStatus.trial,
  version: 2,
  instructions: const [
    Instruction(text: 'Roast.'),
    Instruction(text: 'Glaze.'),
    Instruction(text: 'Serve.'),
  ],
);

Future<void> settle(WidgetTester tester) async {
  for (var i = 0; i < 6; i++) {
    await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 40)));
    await tester.pump(const Duration(milliseconds: 100));
  }
}

void useTallView(WidgetTester tester) {
  tester.view.physicalSize = const Size(900, 2000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
}

/// Opens the sheet from a button and returns what it resolved to.
Future<List<VerdictResult?>> openSheet(WidgetTester tester, VerdictMode mode, {bool finished = true}) async {
  final results = <VerdictResult?>[];
  await tester.pumpWidget(
    MaterialApp(
      theme: buildTheme(Brightness.light),
      home: Builder(
        builder: (context) => Scaffold(
          body: TextButton(
            onPressed: () async => results.add(await showVerdictSheet(context, trial, mode: mode, finished: finished)),
            child: const Text('open'),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
  return results;
}

bool highlighted(WidgetTester tester, String key) => tester.widget(find.byKey(Key(key))) is FilledButton;

void main() {
  group('verdict sheet', () {
    testWidgets('leaving cook mode early: "Just looking" is the default; tapping outside records nothing', (
      tester,
    ) async {
      useTallView(tester);
      final results = await openSheet(tester, VerdictMode.cookMode, finished: false);
      expect(find.text('How did it go?'), findsOneWidget);
      for (final label in ['Keep it', 'Try again', 'Not for us', 'Just looking']) {
        expect(find.text(label), findsOneWidget);
      }
      expect(highlighted(tester, 'verdict-last'), isTrue);
      expect(highlighted(tester, 'verdict-keep'), isFalse);
      await tester.tapAt(const Offset(10, 10)); // the barrier above the sheet
      await tester.pumpAndSettle();
      expect(results, [null]);
    });

    testWidgets('after the last step "Keep it" leads; answers carry rating and note', (tester) async {
      useTallView(tester);
      final results = await openSheet(tester, VerdictMode.cookMode);
      expect(highlighted(tester, 'verdict-keep'), isTrue);
      expect(highlighted(tester, 'verdict-last'), isFalse);
      await tester.tap(find.byTooltip('4 stars'));
      await tester.enterText(find.byKey(const Key('verdict-note')), 'too salty, cut soy by half');
      await tester.tap(find.byKey(const Key('verdict-again')));
      await tester.pumpAndSettle();
      final r = results.single!;
      expect((r.action, r.rating, r.note), (VerdictAction.again, 4, 'too salty, cut soy by half'));
    });

    testWidgets('"I made this" offers Cancel instead of "Just looking"; swiping down records nothing', (tester) async {
      useTallView(tester);
      final results = await openSheet(tester, VerdictMode.madeThis);
      expect(find.text('Just looking'), findsNothing);
      expect(find.text('Cancel'), findsOneWidget);
      await tester.fling(find.text('How did it go?'), const Offset(0, 800), 2000);
      await tester.pumpAndSettle();
      expect(results, [null]);
    });

    testWidgets('meal plan offers "Skipped", which is an answer', (tester) async {
      useTallView(tester);
      final results = await openSheet(tester, VerdictMode.mealPlan);
      expect(find.text('Did you make it?'), findsOneWidget);
      expect(find.text('Just looking'), findsNothing);
      await tester.tap(find.text('Skipped'));
      await tester.pumpAndSettle();
      expect(results.single!.action, VerdictAction.skipped);
    });
  });

  group('recipe page', () {
    Future<LocalStore> open(WidgetTester tester, FakeServer server) async {
      useTallView(tester);
      final store = (await tester.runAsync(memoryStore))!;
      await tester.runAsync(() => store.putRecipe(trial));
      await tester.pumpWidget(wrap(const RecipeDetailScreen(id: 'r1', base: '/trial'), testOverrides(store, server)));
      await settle(tester);
      return store;
    }

    testWidgets('viewing a trial recipe shows the trial bar and changes nothing', (tester) async {
      final server = FakeServer();
      final store = await open(tester, server);
      expect(find.byKey(const Key('trial-bar')), findsOneWidget);
      expect(find.text('TRIAL · not made yet'), findsOneWidget);
      // Scroll around and leave: still untouched, nothing queued or sent.
      await tester.drag(find.byType(CustomScrollView), const Offset(0, -400));
      await settle(tester);
      await tester.pumpWidget(const SizedBox());
      final after = (await tester.runAsync(() => store.recipe('r1')))!;
      expect(after.toJson(), trial.toJson());
      expect(await tester.runAsync(store.opCount), 0);
      expect(server.requests, isEmpty);
    });

    testWidgets('"I made this" → "Try again" counts it once and queues the verdict', (tester) async {
      final server = FakeServer();
      final store = await open(tester, server);
      await tester.tap(find.byKey(const Key('made-this')));
      await tester.pumpAndSettle();
      // Cancel first: nothing happens.
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
      expect((await tester.runAsync(() => store.recipe('r1')))!.triedCount, 0);

      await tester.tap(find.byKey(const Key('made-this')));
      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const Key('verdict-note')), 'needs more ginger');
      await tester.tap(find.byKey(const Key('verdict-again')));
      await settle(tester);
      final after = (await tester.runAsync(() => store.recipe('r1')))!;
      expect(after.triedCount, 1);
      expect(after.status, RecipeStatus.trial);
      expect(after.notes, endsWith(': needs more ginger'));
      final ops = (await tester.runAsync(store.ops))!;
      expect(ops.single.path, '/api/recipes/r1/tried');
      expect(find.textContaining('TRIAL · tried 1×'), findsOneWidget);
    });

    testWidgets('a cookbook recipe has no trial bar', (tester) async {
      useTallView(tester);
      final store = (await tester.runAsync(memoryStore))!;
      await tester.runAsync(() => store.putRecipe(trial.copyWith(status: RecipeStatus.cookbook)));
      await tester.pumpWidget(wrap(const RecipeDetailScreen(id: 'r1'), testOverrides(store, FakeServer())));
      await settle(tester);
      expect(find.byKey(const Key('trial-bar')), findsNothing);
    });
  });

  testWidgets('cook mode: starting asks nothing; leaving early defaults to "Just looking"', (tester) async {
    useTallView(tester);
    final store = (await tester.runAsync(memoryStore))!;
    await tester.runAsync(() => store.putRecipe(trial));
    final router = GoRouter(
      initialLocation: '/trial/r1',
      routes: [
        GoRoute(
          path: '/trial/:id',
          builder: (_, s) => Scaffold(body: Text('page ${s.pathParameters['id']}')),
          routes: [
            GoRoute(
              path: 'cook',
              builder: (_, s) => CookModeScreen(id: s.pathParameters['id']!, base: '/trial'),
            ),
          ],
        ),
      ],
    );
    addTearDown(router.dispose);
    await tester.pumpWidget(
      ProviderScope(
        overrides: testOverrides(store, FakeServer()),
        retry: (_, _) => null,
        child: MaterialApp.router(theme: buildTheme(Brightness.light), routerConfig: router),
      ),
    );
    await settle(tester);
    router.push('/trial/r1/cook');
    await settle(tester);
    expect(find.text('Step 1 of 3'), findsOneWidget);
    expect(find.text('How did it go?'), findsNothing);

    await tester.tap(find.byTooltip('Exit cook mode'));
    await settle(tester);
    expect(find.text('How did it go?'), findsOneWidget);
    expect(tester.widget(find.byKey(const Key('verdict-last'))), isA<FilledButton>());
    await tester.tap(find.text('Just looking'));
    await settle(tester);
    expect(find.text('page r1'), findsOneWidget, reason: 'back on the recipe page');
    final after = (await tester.runAsync(() => store.recipe('r1')))!;
    expect(after.toJson(), trial.toJson());
    expect(await tester.runAsync(store.opCount), 0);
  });
}
