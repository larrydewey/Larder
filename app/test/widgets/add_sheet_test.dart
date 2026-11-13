import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:larder/features/add/add_sheet.dart';
import 'package:larder/features/import/import_screen.dart';
import 'package:larder/theme.dart';
import 'package:material_ui/material_ui.dart';

import '../support/app_harness.dart';
import '../support/fakes.dart';

void main() {
  test('choices: "From a photo" only with AI assist', () {
    expect(addChoices(llm: false), isNot(contains(AddChoice.photo)));
    expect(addChoices(llm: true), contains(AddChoice.photo));
  });

  test('each choice leads into the list it was opened from', () {
    expect(addLocation(AddChoice.write, '/recipes'), '/recipes/new');
    expect(addLocation(AddChoice.write, '/trial'), '/trial/new');
    expect(addLocation(AddChoice.website, '/recipes'), '/recipes/import?tab=website');
    expect(addLocation(AddChoice.text, '/trial'), '/trial/import?tab=text');
    expect(addLocation(AddChoice.photo, '/trial'), '/trial/import?tab=photo');
    expect(addLocation(AddChoice.collection, '/recipes'), '/recipes/import?tab=file');
  });

  testWidgets('the sheet lists the ways to add and returns the one picked', (tester) async {
    AddChoice? picked;
    await tester.pumpWidget(
      MaterialApp(
        theme: buildTheme(Brightness.light),
        home: Builder(
          builder: (context) => TextButton(
            onPressed: () async => picked = await showAddSheet(context, llm: false, trial: true),
            child: const Text('open'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();

    expect(find.text('Add a recipe to try'), findsOneWidget);
    for (final label in ['From a website', 'From text', 'Write it myself', 'Import a collection']) {
      expect(find.text(label), findsOneWidget);
    }
    expect(find.text('From a photo'), findsNothing);

    await tester.tap(find.byKey(const Key('add-website')));
    await tester.pumpAndSettle();
    expect(picked, AddChoice.website);
  });

  // Regression: opened from Recipes, "Add to Trial" started off, so website
  // imports landed in the cookbook.
  for (final base in ['/recipes', '/trial']) {
    testWidgets('a website import from $base lands on the trial list', (tester) async {
      tester.view.physicalSize = const Size(900, 2000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final store = await tester.runAsync(memoryStore);
      final server = FakeServer()
        ..on(
          'POST',
          '/api/import/url',
          (r) => json({'id': 'r9', 'name': 'Miso Aubergine', 'status': (r.body as Map)['status']}, 201),
        );
      final router = GoRouter(
        initialLocation: '$base/import?tab=website',
        routes: [
          for (final b in ['/recipes', '/trial'])
            GoRoute(
              path: b,
              builder: (_, _) => const SizedBox(),
              routes: [
                GoRoute(
                  path: 'import',
                  builder: (_, s) => ImportScreen(initialTab: s.uri.queryParameters['tab'], trial: b == '/trial'),
                ),
                GoRoute(path: ':id', builder: (_, s) => Text('opened ${s.uri.path}')),
              ],
            ),
        ],
      );
      await tester.pumpWidget(
        ProviderScope(
          overrides: testOverrides(store!, server),
          retry: (_, _) => null,
          child: MaterialApp.router(theme: buildTheme(Brightness.light), routerConfig: router),
        ),
      );
      await tester.pump();

      expect(tester.widget<SwitchListTile>(find.byKey(const Key('import-to-trial'))).value, isTrue);
      await tester.enterText(find.byType(TextField).first, 'https://example.com/miso-aubergine');
      await tester.tap(find.text('Import directly'));
      for (var i = 0; i < 5; i++) {
        await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 40)));
        await tester.pump(const Duration(milliseconds: 100));
      }

      expect((server.requests.single.body as Map)['status'], 'trial');
      expect(find.text('opened /trial/r9'), findsOneWidget);
    });
  }
}
