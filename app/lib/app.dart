import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import 'data/local_store.dart';
import 'data/providers.dart';
import 'features/cook/timer_alerts.dart';
import 'router.dart';
import 'theme.dart';
import 'widgets/snack.dart';

final themeModeProvider = NotifierProvider<ThemeModeController, ThemeMode>(ThemeModeController.new);

class ThemeModeController extends Notifier<ThemeMode> {
  @override
  ThemeMode build() => ref.watch(bootstrapProvider).themeMode;

  void set(ThemeMode mode) {
    state = mode;
    unawaited(ref.read(localStoreProvider).setMeta(MetaKeys.themeMode, mode.name));
  }
}

class LarderApp extends ConsumerWidget {
  const LarderApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final router = ref.watch(routerProvider);
    return MaterialApp.router(
      title: 'Larder',
      debugShowCheckedModeBanner: false,
      theme: buildTheme(Brightness.light),
      darkTheme: buildTheme(Brightness.dark),
      themeMode: ref.watch(themeModeProvider),
      scaffoldMessengerKey: scaffoldMessengerKey,
      routerConfig: router,
      builder: (context, child) => TimerAlertHost(child: child ?? const SizedBox.shrink()),
    );
  }
}
