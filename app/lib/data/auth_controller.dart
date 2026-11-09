import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_client.dart';
import '../api/endpoints.dart';
import '../models/json.dart';
import '../models/user.dart';
import 'app_mode.dart';
import 'local_store.dart';
import 'providers.dart';
import 'sync_controller.dart';

enum AuthStatus { needsServer, signedOut, signedIn }

@immutable
class AuthState {
  const AuthState({required this.status, this.user, this.config});
  final AuthStatus status;
  final User? user;
  final ServerConfig? config;

  AuthState copyWith({AuthStatus? status, User? Function()? user, ServerConfig? config}) =>
      AuthState(status: status ?? this.status, user: user != null ? user() : this.user, config: config ?? this.config);
}

final authProvider = NotifierProvider<AuthController, AuthState>(AuthController.new);

class AuthController extends Notifier<AuthState> {
  late LocalStore _store;
  late ApiClient _api;

  @override
  AuthState build() {
    _store = ref.watch(localStoreProvider);
    _api = ref.watch(apiClientProvider);
    final boot = ref.watch(bootstrapProvider);
    _api.onTokens = (access, refresh) async {
      await _store.setMeta(MetaKeys.accessToken, access);
      await _store.setMeta(MetaKeys.refreshToken, refresh);
    };
    _api.onAuthFailure = () {
      if (state.status == AuthStatus.signedIn) {
        state = state.copyWith(status: AuthStatus.signedOut, user: () => null);
      }
    };
    if (_api.baseUrl.isEmpty) return AuthState(status: AuthStatus.needsServer, config: boot.config);
    if (_api.hasSession) return AuthState(status: AuthStatus.signedIn, user: boot.user, config: boot.config);
    return AuthState(status: AuthStatus.signedOut, config: boot.config);
  }

  ServerConfig get config => state.config ?? const ServerConfig();

  /// In standalone mode the local data is the source of truth and must
  /// survive server changes / sign-in (it is migrated afterwards).
  bool get _standalone => ref.read(appModeProvider) != AppMode.connected;

  /// Validates [url] via GET /api/config and makes it the active server.
  Future<ServerConfig> connect(String url) async {
    final normalized = ApiClient.normalizeBaseUrl(url);
    final previous = _api.baseUrl;
    _api.baseUrl = normalized;
    final ServerConfig cfg;
    try {
      cfg = await _api.config();
    } catch (_) {
      _api.baseUrl = previous;
      rethrow;
    }
    if (previous != normalized) {
      await _api.setTokens(null, null);
      if (!_standalone) await _store.clearData();
    }
    await _store.setMeta(MetaKeys.serverUrl, normalized);
    await _store.setMeta(MetaKeys.config, cfg.toJson());
    state = AuthState(
      status: _api.hasSession ? AuthStatus.signedIn : AuthStatus.signedOut,
      user: _api.hasSession ? state.user : null,
      config: cfg,
    );
    return cfg;
  }

  /// Refreshes server config (and the user when signed in) in the background.
  Future<void> refresh() async {
    if (_api.baseUrl.isEmpty) return;
    try {
      final cfg = await _api.config();
      await _store.setMeta(MetaKeys.config, cfg.toJson());
      state = state.copyWith(config: cfg);
      if (state.status == AuthStatus.signedIn) {
        final me = await _api.me();
        await _store.setMeta(MetaKeys.user, me.toJson());
        state = state.copyWith(user: () => me);
      }
    } catch (_) {
      // Offline: keep cached values.
    }
  }

  Future<void> login(String username, String password) async {
    final res = await _api.login(username.trim(), password);
    await _completeAuth(res.accessToken, res.refreshToken, res.user);
  }

  Future<void> register({
    required String username,
    required String password,
    String? displayName,
    String? email,
    String? inviteCode,
  }) async {
    final res = await _api.register(
      username: username.trim(),
      password: password,
      displayName: displayName?.trim(),
      email: email?.trim(),
      inviteCode: inviteCode?.trim(),
    );
    await _completeAuth(res.accessToken, res.refreshToken, res.user);
  }

  /// Completes an OIDC login using tokens from the redirect fragment.
  Future<void> completeWithTokens(String access, String refresh) => _completeAuth(access, refresh, null);

  Future<void> _completeAuth(String access, String refresh, User? user) async {
    await _api.setTokens(access, refresh);
    final me = user ?? await _api.me();
    if (_standalone) {
      // Connect-later: keep local data; the migration screen takes over.
      await _store.setMeta(MetaKeys.user, me.toJson());
      state = AuthState(status: AuthStatus.signedIn, user: me, config: state.config);
      return;
    }
    final previousUser = await _store.getMeta<Map>(MetaKeys.user);
    if (previousUser != null && asJson(previousUser)['id'] != me.id) {
      await _store.clearData();
    }
    await _store.setMeta(MetaKeys.user, me.toJson());
    state = AuthState(status: AuthStatus.signedIn, user: me, config: state.config);
    unawaited(ref.read(syncProvider.notifier).sync(full: true));
  }

  void setUser(User user) {
    unawaited(_store.setMeta(MetaKeys.user, user.toJson()));
    state = state.copyWith(user: () => user);
  }

  /// Signs out. In connected mode the cached data is removed; with
  /// [keepData] (or in standalone mode) it is kept.
  Future<void> logout({bool keepData = false}) async {
    try {
      await _api.logout();
    } catch (_) {}
    await _api.setTokens(null, null);
    if (keepData || _standalone) {
      await _store.setMeta(MetaKeys.user, null);
    } else {
      await _store.clearData();
    }
    state = AuthState(
      status: _api.baseUrl.isEmpty ? AuthStatus.needsServer : AuthStatus.signedOut,
      config: state.config,
    );
  }

  /// Forget the server (Android "change server").
  Future<void> forgetServer() async {
    await logout();
    await _store.setMeta(MetaKeys.serverUrl, null);
    _api.baseUrl = '';
    state = const AuthState(status: AuthStatus.needsServer);
  }
}
