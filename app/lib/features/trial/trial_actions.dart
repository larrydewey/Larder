import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/backend.dart';
import '../../data/outbox_op.dart';
import '../../data/providers.dart';
import '../../models/meal_plan.dart';
import '../../models/recipe.dart';
import '../../models/trial.dart';
import '../../widgets/snack.dart';
import 'verdict_sheet.dart';

/// How long "Undo" is offered before a discarded recipe is deleted.
const discardUndoWindow = Duration(seconds: 5);

/// Recipes discarded but still inside their Undo window. They are hidden
/// from lists; nothing is deleted (or synced) until the window ends.
final pendingDiscardsProvider = NotifierProvider<PendingDiscards, Set<String>>(PendingDiscards.new);

class PendingDiscards extends Notifier<Set<String>> {
  @override
  Set<String> build() => const {};
  void add(String id) => state = {...state, id};
  void remove(String id) => state = {...state}..remove(id);
}

/// The household's trial recipes (without pending discards).
final trialRecipesProvider = Provider<List<Recipe>>((ref) {
  final pending = ref.watch(pendingDiscardsProvider);
  return [
    for (final r in ref.watch(recipesProvider).value ?? const <Recipe>[])
      if (r.isTrial && !pending.contains(r.id)) r,
  ];
});

/// "Move to cookbook": a plain recipe update, so it works offline.
Future<void> moveToCookbook(WidgetRef ref, Recipe recipe) async {
  final o = await ref.read(recipeRepositoryProvider).save(recipe.copyWith(status: RecipeStatus.cookbook), isNew: false);
  reportOutcome(o, success: '"${recipe.name}" is in your cookbook now');
}

/// "Discard" / "Not for us": shows a snackbar with Undo and deletes the
/// recipe only when it closes without Undo. [beforeDelete] runs first (e.g.
/// marking the meal plan entry answered).
void discardWithUndo(BuildContext context, WidgetRef ref, Recipe recipe, {Future<void> Function()? beforeDelete}) {
  // Everything is captured now: the calling screen is usually gone when the
  // Undo window ends.
  final repo = ref.read(recipeRepositoryProvider);
  final pending = ref.read(pendingDiscardsProvider.notifier);
  final messenger = scaffoldMessengerKey.currentState ?? ScaffoldMessenger.maybeOf(context);
  Future<void> commit() async {
    try {
      await beforeDelete?.call();
      reportOutcome(await repo.delete(recipe));
    } finally {
      pending.remove(recipe.id);
    }
  }

  pending.add(recipe.id);
  if (messenger == null) {
    unawaited(commit());
    return;
  }
  messenger.hideCurrentSnackBar();
  final bar = messenger.showSnackBar(
    SnackBar(
      content: Text('"${recipe.name}" discarded'),
      duration: discardUndoWindow,
      persist: false,
      action: SnackBarAction(label: 'Undo', onPressed: () {}),
    ),
  );
  unawaited(
    bar.closed.then((reason) {
      if (reason == SnackBarClosedReason.action) {
        pending.remove(recipe.id);
        return null;
      }
      return commit();
    }),
  );
}

/// Carries out an answer from the verdict sheet. [entry] is the meal plan
/// entry that asked "Did you make it?"; it is marked answered so it asks
/// only once.
Future<void> applyVerdictResult(
  BuildContext context,
  WidgetRef ref,
  Recipe recipe,
  VerdictResult result, {
  MealPlanEntry? entry,
}) async {
  final recipes = ref.read(recipeRepositoryProvider);
  final mealPlan = ref.read(mealPlanRepositoryProvider);
  Future<void> resolve(String outcome) async {
    if (entry == null) return;
    final o = await mealPlan.update(entry.copyWith(trialOutcome: outcome, trialResolvedAt: DateTime.now().toUtc()));
    if (o.kind == OutcomeKind.rejected) reportOutcome(o);
  }

  switch (result.action) {
    case VerdictAction.keep:
    case VerdictAction.again:
      final keep = result.action == VerdictAction.keep;
      final o = await recipes.recordTried(recipe, TrialVerdict(keep: keep, rating: result.rating, note: result.note));
      final ok = reportOutcome(
        o,
        success: keep ? '"${recipe.name}" is in your cookbook now' : '"${recipe.name}" stays on your trial list',
      );
      if (ok) await resolve(keep ? TrialOutcome.keep : TrialOutcome.again);
    case VerdictAction.discard:
      discardWithUndo(context, ref, recipe, beforeDelete: entry == null ? null : () => resolve(TrialOutcome.discard));
    case VerdictAction.skipped:
      await resolve(TrialOutcome.skipped);
  }
}
