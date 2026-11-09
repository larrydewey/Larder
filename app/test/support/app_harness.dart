import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:larder/api/api_client.dart';
import 'package:larder/data/app_mode.dart';
import 'package:larder/data/local_store.dart';
import 'package:larder/data/providers.dart';
import 'package:material_ui/material_ui.dart';
import 'package:larder/theme.dart';

import 'fakes.dart';

List<Override> testOverrides(LocalStore store, FakeServer server, {AppMode mode = AppMode.connected}) => [
  localStoreProvider.overrideWithValue(store),
  apiClientProvider.overrideWithValue(ApiClient(baseUrl: 'http://srv', httpClient: server.client)),
  platformServicesEnabledProvider.overrideWithValue(false),
  bootstrapProvider.overrideWithValue(BootstrapData(mode: mode)),
];

Widget wrap(Widget child, List<Override> overrides) => ProviderScope(
  overrides: overrides,
  retry: (_, _) => null,
  child: MaterialApp(theme: buildTheme(Brightness.light), home: child),
);
