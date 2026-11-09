import 'dart:async';

import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_client.dart';
import '../api/api_exception.dart';
import '../api/endpoints.dart';
import '../models/meal_plan.dart';
import 'app_mode.dart';
import 'local_store.dart';
import 'outbox.dart';
import 'outbox_op.dart';
import 'providers.dart';

@immutable
class SyncStatus {
  const SyncStatus({this.online = true, this.syncing = false, this.lastSync, this.lastError});
  final bool online;
  final bool syncing;
  final DateTime? lastSync;
  final String? lastError;

  SyncStatus copyWith({bool? online, bool? syncing, DateTime? lastSync, String? Function()? lastError}) => SyncStatus(
    online: online ?? this.online,
    syncing: syncing ?? this.syncing,
    lastSync: lastSync ?? this.lastSync,
    lastError: lastError != null ? lastError() : this.lastError,
  );
}

final syncProvider = NotifierProvider<SyncController, SyncStatus>(SyncController.new);

/// Pulls server state into the local store and pushes the outbox.
///
/// Triggers: app start, resume, connectivity regained, pull-to-refresh and a
/// periodic retry while changes are pending.
class SyncController extends Notifier<SyncStatus> {
  late LocalStore _store;
  late ApiClient _api;
  final _messages = StreamController<String>.broadcast();
  Future<FlushResult>? _flushing;
  Future<void>? _syncing;
  StreamSubscription<List<ConnectivityResult>>? _connSub;
  Timer? _retry;

  /// Notices to show as snackbars (conflicts, rejected offline changes).
  Stream<String> get messages => _messages.stream;

  @override
  SyncStatus build() {
    _store = ref.watch(localStoreProvider);
    _api = ref.watch(apiClientProvider);
    if (ref.watch(platformServicesEnabledProvider)) {
      try {
        _connSub = Connectivity().onConnectivityChanged.listen((results) {
          final hasNet = results.any((r) => r != ConnectivityResult.none);
          if (!hasNet) {
            _setOnline(false);
          } else if (!state.online) {
            unawaited(sync());
          }
        }, onError: (_) {});
      } catch (_) {}
      _retry = Timer.periodic(const Duration(seconds: 45), (_) async {
        if (await _store.opCount() > 0) unawaited(sync());
      });
    }
    ref.onDispose(() {
      _connSub?.cancel();
      _retry?.cancel();
    });
    return const SyncStatus();
  }

  void _setOnline(bool v) {
    if (state.online != v) state = state.copyWith(online: v);
  }

  void notify(String message) => _messages.add(message);

  /// Pushes pending changes, then pulls updates. Safe to call repeatedly.
  Future<void> sync({bool full = false}) {
    return _syncing ??= _doSync(full: full).whenComplete(() => _syncing = null);
  }

  Future<void> _doSync({bool full = false}) async {
    if (!_api.hasSession || ref.read(appModeProvider) != AppMode.connected) return;
    state = state.copyWith(syncing: true);
    try {
      final fr = await flush();
      if (fr.stoppedOffline || fr.stoppedUnauthorized) return;
      if (await _store.opCount() > 0) return; // new local changes; next round.
      final cursor = full ? null : await _store.getMeta<String>(MetaKeys.syncCursor);
      final res = await _api.sync(since: cursor);
      // Something may have been queued while we were waiting: don't let the
      // snapshot clobber optimistic changes; they'll be reconciled next time.
      if (await _store.opCount() > 0) {
        unawaited(Future<void>.delayed(const Duration(milliseconds: 200), sync));
        return;
      }
      await _store.applyRecipes(res.recipes, res.deletedRecipeIds, full: res.full || cursor == null);
      final today = DateTime.now();
      await _store.replaceMealPlan(
        res.mealPlan,
        start: dateKey(today.subtract(const Duration(days: 30))),
        end: dateKey(today.add(const Duration(days: 60))),
      );
      await _store.replaceShoppingLists(res.shoppingLists);
      await _store.setMeta(MetaKeys.syncCursor, res.cursor);
      final now = DateTime.now();
      await _store.setMeta(MetaKeys.lastSync, now.toIso8601String());
      state = state.copyWith(online: true, lastSync: now, lastError: () => null);
    } on NetworkException {
      _setOnline(false);
    } on ApiException catch (e) {
      state = state.copyWith(online: true, lastError: () => e.message);
    } catch (e) {
      state = state.copyWith(lastError: () => e.toString());
    } finally {
      state = state.copyWith(syncing: false);
    }
  }

  /// Replays the outbox (single-flight).
  Future<FlushResult> flush() async {
    if (_flushing != null) {
      await _flushing;
    }
    return _flushing ??= _doFlush().whenComplete(() => _flushing = null);
  }

  Future<FlushResult> _doFlush() async {
    final result = await OutboxProcessor(_store, _api).flush();
    if (result.stoppedOffline) {
      _setOnline(false);
    } else if (result.sent > 0 || result.outcomes.isNotEmpty) {
      _setOnline(true);
    }
    for (final m in result.messages) {
      _messages.add(m);
    }
    if (result.needsSync && !result.stoppedOffline) {
      unawaited(Future<void>.delayed(const Duration(milliseconds: 100), sync));
    }
    return result;
  }

  /// Queues [op] and tries to send it right away. Returns the outcome for
  /// this op ([OpOutcome.queued] when offline).
  Future<OpOutcome> submit(OutboxOp op) async {
    await _store.addOp(op);
    if (!_api.hasSession) return OpOutcome.queued;
    final result = await flush();
    final outcome = result.outcomes[op.id];
    if (outcome != null) return outcome;
    // Our op may have been sent by a flush that was already running.
    if (!result.stoppedOffline && await _store.opCount() > 0) {
      final again = await flush();
      return again.outcomes[op.id] ?? OpOutcome.queued;
    }
    return OpOutcome.queued;
  }

  /// Runs [action] directly against the server, tracking online state.
  Future<T> online<T>(Future<T> Function(ApiClient api) action) async {
    try {
      final r = await action(_api);
      _setOnline(true);
      return r;
    } on NetworkException {
      _setOnline(false);
      rethrow;
    }
  }
}
