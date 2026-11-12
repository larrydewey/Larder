/// Trial recipes: verdicts ("I made this") and the rules shared by the
/// server (`larder_core::trial`), the on-device backend and the screens.
library;

import 'json.dart';
import 'meal_plan.dart';
import 'recipe.dart';

/// Answers stored on a meal plan entry (`trial_outcome`).
abstract final class TrialOutcome {
  static const keep = 'keep';
  static const again = 'again';
  static const discard = 'discard';
  static const skipped = 'skipped';
}

/// "I made this": what the household thought of a trial recipe.
class TrialVerdict {
  TrialVerdict({required this.keep, this.rating, this.note = '', DateTime? triedAt, DateTime? date})
    : triedAt = (triedAt ?? DateTime.now()).toUtc(),
      date = date ?? DateTime.now();

  /// "Keep it" (move to the cookbook); otherwise "Try again".
  final bool keep;

  /// 1-5 stars, or null to keep the current rating.
  final int? rating;
  final String note;

  /// When it was made. Also identifies the verdict, so a replayed request
  /// (offline outbox retry) is not counted twice.
  final DateTime triedAt;

  /// The cook's local calendar date, used for the note line.
  final DateTime date;

  /// Body for `POST /api/recipes/{id}/tried`.
  Json toJson() => {
    'outcome': keep ? TrialOutcome.keep : TrialOutcome.again,
    'tried_at': triedAt.toIso8601String(),
    'date': dateKey(date),
    if (rating != null) 'rating': rating,
    if (note.trim().isNotEmpty) 'note': note.trim(),
  };
}

const _months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/// The dated line a verdict note adds to the recipe notes
/// (`Nov 12: too salty, cut soy by half`), or null for an empty note.
/// Same format as the server's `note_line`.
String? trialNoteLine(DateTime date, String note) {
  final text = note.trim().split(RegExp(r'\s+')).where((w) => w.isNotEmpty).join(' ');
  if (text.isEmpty) return null;
  return '${_months[date.month - 1]} ${date.day}: $text';
}

/// Appends [line] to [notes] on a line of its own.
String appendNoteLine(String notes, String line) {
  final n = notes.trimRight();
  return n.isEmpty ? line : '$n\n$line';
}

/// [recipe] after [v]: counted, `lastTriedAt` moved forward, rating and note
/// line applied and, for "Keep it", moved to the cookbook. Mirrors the
/// server's `record_tried` (the version and timestamps are the caller's).
Recipe applyTrialVerdict(Recipe recipe, TrialVerdict v) {
  final line = trialNoteLine(v.date, v.note);
  final last = recipe.lastTriedAt;
  return recipe.copyWith(
    status: v.keep ? RecipeStatus.cookbook : recipe.status,
    triedCount: recipe.triedCount + 1,
    lastTriedAt: () => (last == null || last.isBefore(v.triedAt)) ? v.triedAt : last,
    rating: v.rating == null ? null : () => v.rating,
    notes: line == null ? null : appendNoteLine(recipe.notes, line),
  );
}

/// Whether a meal plan entry should ask "Did you make it?": its recipe is
/// on trial, the planned day has passed, nobody answered for this entry yet
/// and the recipe was not marked as made on or after that day.
bool asksDidYouMakeIt(MealPlanEntry entry, Recipe? recipe, {DateTime? now}) {
  if (recipe == null || !recipe.isTrial || entry.trialOutcome != null) return false;
  final n = now ?? DateTime.now();
  final day = parseDateKey(entry.date);
  if (!day.isBefore(DateTime(n.year, n.month, n.day))) return false;
  final last = recipe.lastTriedAt?.toLocal();
  return last == null || DateTime(last.year, last.month, last.day).isBefore(day);
}
