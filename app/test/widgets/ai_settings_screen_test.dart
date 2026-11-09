import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:larder/api/api_client.dart';
import 'package:larder/data/app_mode.dart';
import 'package:larder/data/providers.dart';
import 'package:larder/features/settings/ai_settings_screen.dart';
import 'package:larder/models/user.dart';
import 'package:larder/theme.dart';
import 'package:material_ui/material_ui.dart';

import '../support/fakes.dart';

const _defaults = {
  'anthropic': {'model': 'claude-opus-5-5', 'base_url': 'https://api.anthropic.com', 'needs_key': true},
  'openai': {'model': 'gpt-4o-mini', 'base_url': 'https://api.openai.com/v1', 'needs_key': true},
  'ollama': {'model': 'llama3.2-vision', 'base_url': 'http://localhost:11434/v1', 'needs_key': false},
};

Future<void> _settle(WidgetTester tester) async {
  await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 50)));
  await tester.pump();
}

void main() {
  testWidgets('an admin adds a key; the key is sent once and only its hint is shown', (tester) async {
    tester.view.physicalSize = const Size(900, 2000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final store = await tester.runAsync(memoryStore);
    Map<String, Object?> status = {'source': 'off', 'enabled': false, 'has_key': false, 'defaults': _defaults};
    final server = FakeServer()
      ..on('GET', '/api/admin/settings/ai', (r) => json(status))
      ..on('PUT', '/api/admin/settings/ai', (r) {
        status = {
          'source': 'server',
          'enabled': true,
          'provider': 'anthropic',
          'model': 'claude-opus-5-5',
          'base_url': 'https://api.anthropic.com',
          'has_key': true,
          'key_hint': '0123',
          'defaults': _defaults,
        };
        return json(status);
      })
      ..on('GET', '/api/config', (r) => json({'version': '0.1.4', 'llm_enabled': true}))
      ..on('GET', '/api/users/me', (r) => json({'id': 'u1', 'username': 'admin', 'is_admin': true}));
    final admin = User.fromJson({'id': 'u1', 'username': 'admin', 'is_admin': true});
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          localStoreProvider.overrideWithValue(store!),
          apiClientProvider.overrideWithValue(
            ApiClient(baseUrl: 'http://srv', httpClient: server.client, accessToken: 'a', refreshToken: 'r'),
          ),
          platformServicesEnabledProvider.overrideWithValue(false),
          bootstrapProvider.overrideWithValue(
            BootstrapData(mode: AppMode.connected, serverUrl: 'http://srv', user: admin, config: const ServerConfig()),
          ),
        ],
        retry: (_, _) => null,
        child: MaterialApp(theme: buildTheme(Brightness.light), home: const AiSettingsScreen()),
      ),
    );
    await _settle(tester);

    expect(find.text('Off'), findsOneWidget);
    expect(find.textContaining('console.anthropic.com'), findsOneWidget);
    expect(find.text('Turn off'), findsNothing);

    await tester.enterText(find.widgetWithText(TextField, 'API key'), 'sk-ant-test-key-0123');
    await tester.tap(find.text('Save'));
    await _settle(tester);
    await _settle(tester);

    final put = server.requests.singleWhere((r) => r.method == 'PUT');
    expect(put.body, {'provider': 'anthropic', 'api_key': 'sk-ant-test-key-0123'});
    expect(find.text('On'), findsOneWidget);
    expect(find.text('Key ending in 0123'), findsOneWidget);
    expect(find.textContaining('Saved key ends in 0123'), findsOneWidget);
    expect(find.text('sk-ant-test-key-0123'), findsNothing, reason: 'the key field is cleared after saving');
    expect(find.text('Test'), findsOneWidget);
    expect(find.text('Turn off'), findsOneWidget);
    // Let the confirmation message time out.
    await tester.pump(const Duration(seconds: 10));
  });

  testWidgets('settings from environment variables are read-only', (tester) async {
    final store = await tester.runAsync(memoryStore);
    final server = FakeServer()
      ..on(
        'GET',
        '/api/admin/settings/ai',
        (r) => json({
          'source': 'environment',
          'enabled': true,
          'provider': 'anthropic',
          'model': 'claude-opus-5-5',
          'has_key': true,
          'key_hint': 'WXYZ',
          'defaults': _defaults,
        }),
      );
    final admin = User.fromJson({'id': 'u1', 'username': 'admin', 'is_admin': true});
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          localStoreProvider.overrideWithValue(store!),
          apiClientProvider.overrideWithValue(
            ApiClient(baseUrl: 'http://srv', httpClient: server.client, accessToken: 'a', refreshToken: 'r'),
          ),
          platformServicesEnabledProvider.overrideWithValue(false),
          bootstrapProvider.overrideWithValue(
            BootstrapData(mode: AppMode.connected, serverUrl: 'http://srv', user: admin, config: const ServerConfig()),
          ),
        ],
        retry: (_, _) => null,
        child: MaterialApp(theme: buildTheme(Brightness.light), home: const AiSettingsScreen()),
      ),
    );
    await _settle(tester);
    expect(find.textContaining('LARDER_LLM_'), findsOneWidget);
    expect(find.text('Key ending in WXYZ'), findsOneWidget);
    expect(find.text('Save'), findsNothing);
  });
}
