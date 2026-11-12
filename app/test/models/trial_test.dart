import 'package:flutter_test/flutter_test.dart';
import 'package:larder/features/trial/trial_text.dart';
import 'package:larder/models/meal_plan.dart';
import 'package:larder/models/recipe.dart';
import 'package:larder/models/trial.dart';

void main() {
  final trial = Recipe(
    id: 'r1',
    name: 'Miso Aubergine',
    status: RecipeStatus.trial,
    notes: 'From the market.',
    rating: 3,
  );

  test('note lines match the server format', () {
    expect(
      trialNoteLine(DateTime(2026, 11, 2), '  too salty,\n cut soy by half '),
      'Nov 2: too salty, cut soy by half',
    );
    expect(trialNoteLine(DateTime(2026, 1, 12), '   '), isNull);
    expect(appendNoteLine('', 'Nov 2: good'), 'Nov 2: good');
    expect(appendNoteLine('Use fresh basil.\n\n', 'Nov 2: good'), 'Use fresh basil.\nNov 2: good');
  });

  test('a verdict body carries the outcome, an idempotency time and the local date', () {
    final v = TrialVerdict(
      keep: false,
      rating: 4,
      note: ' too salty ',
      triedAt: DateTime.utc(2026, 11, 12, 18, 30),
      date: DateTime(2026, 11, 12),
    );
    expect(v.toJson(), {
      'outcome': 'again',
      'tried_at': '2026-11-12T18:30:00.000Z',
      'date': '2026-11-12',
      'rating': 4,
      'note': 'too salty',
    });
    expect(TrialVerdict(keep: true).toJson().keys, unorderedEquals(['outcome', 'tried_at', 'date']));
  });

  test('applying a verdict counts, rates, notes and (for keep) promotes', () {
    final again = applyTrialVerdict(
      trial,
      TrialVerdict(
        keep: false,
        rating: 5,
        note: 'great',
        triedAt: DateTime.utc(2026, 11, 12),
        date: DateTime(2026, 11, 12),
      ),
    );
    expect(again.status, RecipeStatus.trial);
    expect(again.triedCount, 1);
    expect(again.lastTriedAt, DateTime.utc(2026, 11, 12));
    expect(again.rating, 5);
    expect(again.notes, 'From the market.\nNov 12: great');

    // An older verdict arriving late does not move last_tried_at back; no rating/note keeps them.
    final late = applyTrialVerdict(again, TrialVerdict(keep: true, triedAt: DateTime.utc(2026, 11, 1)));
    expect(late.status, RecipeStatus.cookbook);
    expect(late.triedCount, 2);
    expect(late.lastTriedAt, DateTime.utc(2026, 11, 12));
    expect(late.rating, 5);
    expect(late.notes, again.notes);
  });

  test('"Did you make it?" is asked once, for past entries of trial recipes', () {
    final now = DateTime(2026, 11, 12, 9);
    MealPlanEntry entry(String date, {String? outcome}) =>
        MealPlanEntry(id: 'm', date: date, recipeId: 'r1', trialOutcome: outcome);
    expect(asksDidYouMakeIt(entry('2026-11-10'), trial, now: now), isTrue);
    expect(asksDidYouMakeIt(entry('2026-11-12'), trial, now: now), isFalse, reason: 'not past yet');
    expect(asksDidYouMakeIt(entry('2026-11-13'), trial, now: now), isFalse);
    expect(
      asksDidYouMakeIt(entry('2026-11-10', outcome: 'skipped'), trial, now: now),
      isFalse,
      reason: 'answered',
    );
    expect(asksDidYouMakeIt(entry('2026-11-10'), trial.copyWith(status: RecipeStatus.cookbook), now: now), isFalse);
    expect(asksDidYouMakeIt(entry('2026-11-10'), null, now: now), isFalse);
    // Marked as made on/after the planned day (e.g. via "I made this"): no need to ask.
    final made = trial.copyWith(triedCount: 1, lastTriedAt: () => DateTime(2026, 11, 10, 20));
    expect(asksDidYouMakeIt(entry('2026-11-10'), made, now: now), isFalse);
    expect(asksDidYouMakeIt(entry('2026-11-11'), made, now: now), isTrue);
  });

  test('status line and "added … ago"', () {
    expect(trialStatusLine(trial), 'not made yet');
    expect(
      trialStatusLine(trial.copyWith(triedCount: 2, lastTriedAt: () => DateTime(2026, 11, 12, 12))),
      'tried 2× · last Nov 12',
    );
    final now = DateTime(2026, 11, 12, 10);
    expect(addedAgo(DateTime(2026, 11, 12, 8), now: now), 'added today');
    expect(addedAgo(DateTime(2026, 11, 11), now: now), 'added yesterday');
    expect(addedAgo(DateTime(2026, 11, 2), now: now), 'added 10 days ago');
    expect(addedAgo(DateTime(2026, 10, 20), now: now), 'added 3 weeks ago');
    expect(addedAgo(DateTime(2026, 7, 10), now: now), 'added 4 months ago');
    expect(addedAgo(DateTime(2024, 11, 1), now: now), 'added 2 years ago');
  });
}
