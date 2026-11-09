import 'package:flutter_test/flutter_test.dart';
import 'package:larder/features/auth/login_screen.dart';
import 'package:material_ui/material_ui.dart';

import '../support/app_harness.dart';
import '../support/fakes.dart';

void main() {
  Future<FakeServer> pumpLogin(WidgetTester tester, {bool setup = false}) async {
    tester.view.physicalSize = const Size(800, 1800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final server = FakeServer();
    server.on(
      'GET',
      '/api/config',
      (r) => json({
        'version': '0.1.0',
        'setup_required': setup,
        'registration_open': false,
        'oidc': {'enabled': true, 'name': 'Authentik'},
        'llm_enabled': false,
        'database': 'sqlite',
      }),
    );
    server.on('POST', '/api/auth/login', (r) => json({'error': 'bad credentials', 'code': 'unauthorized'}, 401));
    final store = await tester.runAsync(memoryStore);
    await tester.pumpWidget(wrap(const LoginScreen(), testOverrides(store!, server)));
    for (var i = 0; i < 4; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 50)));
      await tester.pump(const Duration(milliseconds: 50));
    }
    return server;
  }

  testWidgets('validates and shows server errors; offers OIDC', (tester) async {
    final server = await pumpLogin(tester);
    expect(find.text('Welcome back'), findsOneWidget);
    expect(find.text('Sign in with Authentik'), findsOneWidget);

    await tester.tap(find.byKey(const Key('submit')));
    await tester.pump();
    expect(find.text('Required'), findsNWidgets(2));

    await tester.enterText(find.byKey(const Key('username')), 'alice');
    await tester.enterText(find.byKey(const Key('password')), 'wrongpass');
    await tester.tap(find.byKey(const Key('submit')));
    for (var i = 0; i < 4; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 50)));
      await tester.pump(const Duration(milliseconds: 50));
    }
    expect(find.text('Wrong username or password.'), findsOneWidget);
    expect(server.log, contains('POST /api/auth/login'));
  });

  testWidgets('fresh server shows create admin account', (tester) async {
    await pumpLogin(tester, setup: true);
    expect(find.text('Create admin account'), findsNWidgets(2));
    expect(find.text('Display name'), findsOneWidget);
    expect(find.text('Sign in with Authentik'), findsNothing);
  });
}
