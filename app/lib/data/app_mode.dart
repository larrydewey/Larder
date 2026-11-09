import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'local_store.dart';
import 'providers.dart';

/// How the app gets its data.
enum AppMode {
  /// First launch on Android: the user has not chosen yet.
  unset,

  /// Android only: everything on device (local store + engine), no server.
  standalone,

  /// Server-backed: local cache + outbox + /api/sync. Always on web.
  connected;

  static AppMode parse(String? s) => AppMode.values.firstWhere((m) => m.name == s, orElse: () => AppMode.unset);
}

final appModeProvider = NotifierProvider<AppModeController, AppMode>(AppModeController.new);

class AppModeController extends Notifier<AppMode> {
  @override
  AppMode build() => kIsWeb ? AppMode.connected : ref.watch(bootstrapProvider).mode;

  Future<void> set(AppMode mode) async {
    if (kIsWeb) return;
    await ref.read(localStoreProvider).setMeta(MetaKeys.appMode, mode.name);
    state = mode;
  }
}
