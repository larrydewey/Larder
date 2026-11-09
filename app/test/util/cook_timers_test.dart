import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:larder/features/cook/cook_timers.dart';

void main() {
  test('concurrent timers: pause, resume, finish, cancel', () {
    final c = ProviderContainer();
    addTearDown(c.dispose);
    var now = DateTime(2026, 1, 1, 12);
    final ctrl = c.read(cookTimersProvider.notifier)..now = () => now;
    final a = ctrl.start('Bake', const Duration(minutes: 25));
    final b = ctrl.start('Rest', const Duration(minutes: 5));

    now = now.add(const Duration(minutes: 2));
    ctrl.tick();
    ctrl.pause(a);
    expect(c.read(cookTimersProvider).firstWhere((t) => t.id == a).remaining, const Duration(minutes: 23));

    now = now.add(const Duration(minutes: 3));
    ctrl.tick();
    final timers = c.read(cookTimersProvider);
    expect(timers.firstWhere((t) => t.id == b).done, isTrue);
    expect(timers.firstWhere((t) => t.id == a).remaining, const Duration(minutes: 23));

    ctrl.resume(a);
    now = now.add(const Duration(minutes: 23));
    ctrl.tick();
    expect(c.read(cookTimersProvider).every((t) => t.done), isTrue);
    ctrl.cancel(a);
    ctrl.cancel(b);
    expect(c.read(cookTimersProvider), isEmpty);
  });
}
