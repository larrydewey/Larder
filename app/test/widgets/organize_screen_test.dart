import 'package:flutter_test/flutter_test.dart';
import 'package:larder/features/organize/organize_screen.dart';
import 'package:material_ui/material_ui.dart';

import '../data/organize_remote_test.dart' show organizeResponse;
import '../support/app_harness.dart';
import '../support/fakes.dart';

void main() {
  testWidgets('preview lists changes by new category; offline disables the buttons', (tester) async {
    tester.view.physicalSize = const Size(900, 2400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final store = await tester.runAsync(memoryStore);
    final server = FakeServer()..on('POST', '/api/recipes/organize', (r) => json(organizeResponse));
    await tester.pumpWidget(wrap(const OrganizeScreen(), testOverrides(store!, server)));
    await tester.pump();

    expect(find.textContaining('fills in missing categories'), findsOneWidget);
    expect(find.text('Only recipes without a category'), findsOneWidget);
    expect(find.text('Tidy ingredient text'), findsOneWidget);

    await tester.tap(find.byKey(const Key('organize-mode-all')));
    await tester.pump();
    await tester.tap(find.byKey(const Key('organize-preview')));
    await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 50)));
    await tester.pump();
    expect(server.requests.single.body, {'categorize': 'all', 'normalize': true, 'dry_run': true});
    expect(find.text('Soups & Stews'), findsOneWidget);
    expect(find.text('No category → Soups & Stews'), findsOneWidget);
    expect(find.text('ingredients tidied'), findsNWidgets(2));

    // Changing an option discards the stale preview.
    await tester.tap(find.text('Tidy ingredient text'));
    await tester.pump();
    expect(find.text('No category → Soups & Stews'), findsNothing);

    server.offline = true;
    await tester.tap(find.byKey(const Key('organize-preview')));
    await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 50)));
    await tester.pump();
    expect(find.textContaining('needs a connection'), findsWidgets);
    expect(tester.widget<FilledButton>(find.byKey(const Key('organize-apply'))).onPressed, isNull);
  });
}
