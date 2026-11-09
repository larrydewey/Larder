import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

@immutable
class CookTimer {
  const CookTimer({
    required this.id,
    required this.label,
    required this.total,
    required this.remaining,
    this.recipeName = '',
    this.endsAt,
    this.done = false,
  });

  final int id;
  final String label;
  final String recipeName;
  final Duration total;

  /// Remaining time (frozen while paused).
  final Duration remaining;

  /// Set while running.
  final DateTime? endsAt;
  final bool done;

  bool get running => endsAt != null && !done;
  double get progress => total.inMilliseconds == 0 ? 1 : 1 - remaining.inMilliseconds / total.inMilliseconds;

  CookTimer copyWith({Duration? remaining, DateTime? Function()? endsAt, bool? done}) => CookTimer(
    id: id,
    label: label,
    recipeName: recipeName,
    total: total,
    remaining: remaining ?? this.remaining,
    endsAt: endsAt != null ? endsAt() : this.endsAt,
    done: done ?? this.done,
  );
}

final cookTimersProvider = NotifierProvider<CookTimersController, List<CookTimer>>(CookTimersController.new);

/// Multiple concurrent kitchen timers; they keep running while navigating.
class CookTimersController extends Notifier<List<CookTimer>> {
  Timer? _ticker;
  int _nextId = 1;

  /// Overridable clock (tests).
  DateTime Function() now = DateTime.now;

  @override
  List<CookTimer> build() {
    ref.onDispose(() => _ticker?.cancel());
    return const [];
  }

  int start(String label, Duration duration, {String recipeName = ''}) {
    final id = _nextId++;
    state = [
      ...state,
      CookTimer(
        id: id,
        label: label,
        recipeName: recipeName,
        total: duration,
        remaining: duration,
        endsAt: now().add(duration),
      ),
    ];
    _ensureTicker();
    return id;
  }

  void pause(int id) => _update(id, (t) => t.running ? t.copyWith(remaining: _left(t), endsAt: () => null) : t);

  void resume(int id) =>
      _update(id, (t) => (!t.running && !t.done) ? t.copyWith(endsAt: () => now().add(t.remaining)) : t);

  void cancel(int id) {
    state = state.where((t) => t.id != id).toList();
    if (state.every((t) => !t.running)) _stopTicker();
  }

  void _update(int id, CookTimer Function(CookTimer) f) {
    state = [for (final t in state) t.id == id ? f(t) : t];
    if (state.any((t) => t.running)) {
      _ensureTicker();
    } else {
      _stopTicker();
    }
  }

  Duration _left(CookTimer t) {
    final l = t.endsAt!.difference(now());
    return l.isNegative ? Duration.zero : l;
  }

  void _ensureTicker() => _ticker ??= Timer.periodic(const Duration(milliseconds: 250), (_) => tick());

  void _stopTicker() {
    _ticker?.cancel();
    _ticker = null;
  }

  @visibleForTesting
  void tick() {
    var changed = false;
    final next = <CookTimer>[];
    for (final t in state) {
      if (!t.running) {
        next.add(t);
        continue;
      }
      final left = _left(t);
      changed = true;
      next.add(
        left == Duration.zero
            ? t.copyWith(remaining: Duration.zero, done: true, endsAt: () => null)
            : t.copyWith(remaining: left),
      );
    }
    if (changed) state = next;
    if (!state.any((t) => t.running)) _stopTicker();
  }
}
