import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../util/platform/platform.dart';
import '../../widgets/snack.dart';
import 'cook_timers.dart';

/// Rings (sound + vibration, repeated) and shows a dialog when a timer ends.
class TimerAlertHost extends ConsumerStatefulWidget {
  const TimerAlertHost({super.key, required this.child});
  final Widget child;

  @override
  ConsumerState<TimerAlertHost> createState() => _TimerAlertHostState();
}

class _TimerAlertHostState extends ConsumerState<TimerAlertHost> {
  final Set<int> _alerted = {};
  Timer? _ring;

  @override
  void dispose() {
    _ring?.cancel();
    super.dispose();
  }

  void _startRinging() {
    if (_ring != null) return;
    unawaited(alertBeep());
    var count = 0;
    _ring = Timer.periodic(const Duration(seconds: 2), (_) {
      if (++count > 30) return _stopRinging();
      unawaited(alertBeep());
    });
  }

  void _stopRinging() {
    _ring?.cancel();
    _ring = null;
  }

  Future<void> _show(CookTimer t) async {
    _startRinging();
    final ctx = rootNavigatorKey.currentContext;
    if (ctx == null) return;
    await showDialog<void>(
      context: ctx,
      useRootNavigator: true,
      builder: (context) => AlertDialog(
        icon: const Icon(Icons.alarm_rounded, size: 40),
        title: const Text("Time's up!"),
        content: Text(
          [t.label, if (t.recipeName.isNotEmpty) t.recipeName].join(' · '),
          textAlign: TextAlign.center,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        actions: [FilledButton(onPressed: () => Navigator.pop(context), child: const Text('Dismiss'))],
      ),
    );
    ref.read(cookTimersProvider.notifier).cancel(t.id);
    if (!ref.read(cookTimersProvider).any((x) => x.done)) _stopRinging();
  }

  @override
  Widget build(BuildContext context) {
    ref.listen<List<CookTimer>>(cookTimersProvider, (_, timers) {
      for (final t in timers) {
        if (t.done && _alerted.add(t.id)) unawaited(_show(t));
      }
      _alerted.removeWhere((id) => !timers.any((t) => t.id == id));
    });
    return widget.child;
  }
}
