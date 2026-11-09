import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_web_plugins/url_strategy.dart';
import 'package:material_ui/material_ui.dart';

import 'api/api_client.dart';
import 'app.dart';
import 'data/app_mode.dart';
import 'data/db/db_factory.dart';
import 'data/local_store.dart';
import 'data/providers.dart';
import 'models/json.dart';
import 'models/user.dart';
import 'util/platform/platform.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // Clean URLs (/recipes/123) so OIDC can redirect to /oidc-callback#tokens.
  usePathUrlStrategy();
  _registerFontLicenses();

  final db = await openAppDatabase();
  final store = LocalStore(db);
  final storedUrl = await store.getMeta<String>(MetaKeys.serverUrl);
  final serverUrl = storedUrl ?? (kIsWeb ? pageOrigin() : null);
  final userJson = await store.getMeta<Map>(MetaKeys.user);
  final configJson = await store.getMeta<Map>(MetaKeys.config);
  final theme = await store.getMeta<String>(MetaKeys.themeMode);
  final storedMode = await store.getMeta<String>(MetaKeys.appMode);
  final mode = kIsWeb
      ? AppMode.connected
      : storedMode != null
      ? AppMode.parse(storedMode)
      : (storedUrl != null ? AppMode.connected : AppMode.unset);
  final llm = await store.getMeta<Map>(MetaKeys.llmConfig);

  final api = ApiClient(
    baseUrl: serverUrl,
    accessToken: await store.getMeta<String>(MetaKeys.accessToken),
    refreshToken: await store.getMeta<String>(MetaKeys.refreshToken),
  );

  runApp(
    ProviderScope(
      retry: (_, _) => null,
      overrides: [
        localStoreProvider.overrideWithValue(store),
        apiClientProvider.overrideWithValue(api),
        bootstrapProvider.overrideWithValue(
          BootstrapData(
            serverUrl: serverUrl,
            user: userJson == null ? null : User.fromJson(asJson(userJson)),
            config: configJson == null ? null : ServerConfig.fromJson(asJson(configJson)),
            themeMode: ThemeMode.values.firstWhere((m) => m.name == theme, orElse: () => ThemeMode.system),
            mode: mode,
            localUnits: await store.getMeta<String>(MetaKeys.localUnits) ?? 'original',
            llm: llm == null ? null : asJson(llm),
          ),
        ),
      ],
      child: const LarderApp(),
    ),
  );
}

void _registerFontLicenses() {
  LicenseRegistry.addLicense(() async* {
    for (final (pkg, file) in [('Inter', 'Inter-OFL.txt'), ('DM Serif Display', 'DMSerifDisplay-OFL.txt')]) {
      final text = await rootBundle.loadString('assets/fonts/$file');
      yield LicenseEntryWithLineBreaks([pkg], text);
    }
  });
}
