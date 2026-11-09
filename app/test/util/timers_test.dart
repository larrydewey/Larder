import 'package:flutter_test/flutter_test.dart';
import 'package:larder/util/timers.dart';

List<Duration> d(String s) => detectTimers(s).map((t) => t.duration).toList();

void main() {
  test('ordinal "a second" is not a timer', () {
    expect(detectTimers('In a second bowl whisk the eggs.'), isEmpty);
    expect(detectTimers('Wait a minute, then stir.').single.duration, const Duration(minutes: 1));
    expect(detectTimers('Stir for 30 seconds.').single.duration, const Duration(seconds: 30));
  });

  test('minutes and hours', () {
    expect(d('Bake for 25 minutes until golden.'), [const Duration(minutes: 25)]);
    expect(d('Simmer 1 hour.'), [const Duration(hours: 1)]);
    expect(d('Rest for 30 secs'), [const Duration(seconds: 30)]);
    expect(d('Cook 5 mins'), [const Duration(minutes: 5)]);
    expect(d('Cook 5min'), [const Duration(minutes: 5)]);
  });

  test('ranges take the upper bound', () {
    expect(d('Bake 10-12 minutes'), [const Duration(minutes: 12)]);
    expect(d('Bake 10 to 15 minutes'), [const Duration(minutes: 15)]);
    expect(d('Roast 1–2 hours'), [const Duration(hours: 2)]);
  });

  test('compound and fractional durations', () {
    expect(d('Braise for 1 hour 30 minutes'), [const Duration(minutes: 90)]);
    expect(d('Braise for 2 hours and 15 minutes'), [const Duration(minutes: 135)]);
    expect(d('Chill 1 1/2 hours'), [const Duration(minutes: 90)]);
    expect(d('Chill 1½ hours'), [const Duration(minutes: 90)]);
    expect(d('Bake for an hour and a half'), [const Duration(minutes: 90)]);
    expect(d('Wait half an hour'), [const Duration(minutes: 30)]);
  });

  test('word numbers', () {
    expect(d('Boil for ten minutes'), [const Duration(minutes: 10)]);
    expect(d('Stir for a minute'), [const Duration(minutes: 1)]);
  });

  test('multiple timers in one step', () {
    expect(d('Bake 20 minutes, then broil 3 minutes.'), [const Duration(minutes: 20), const Duration(minutes: 3)]);
  });

  test('no false positives', () {
    expect(d('Add 2 cups of flour and a pinch of salt.'), isEmpty);
    expect(d('Use a 9x13 pan and mix the mint.'), isEmpty);
    expect(d('Preheat oven to 350 F.'), isEmpty);
  });

  test('labels', () {
    expect(detectTimers('Bake for 25 minutes.').single.label, '25 minutes');
  });

  test('formatClock / formatMinutes', () {
    expect(formatClock(const Duration(minutes: 4, seconds: 5)), '4:05');
    expect(formatClock(const Duration(hours: 1, minutes: 2, seconds: 3)), '1:02:03');
    expect(formatMinutes(80), '1 h 20 min');
    expect(formatMinutes(45), '45 min');
  });
}
