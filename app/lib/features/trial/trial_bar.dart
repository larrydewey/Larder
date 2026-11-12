import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../models/recipe.dart';
import 'trial_actions.dart';
import 'trial_text.dart';
import 'verdict_sheet.dart';

/// The slim bar under a trial recipe's title: `TRIAL · not made yet`,
/// "I made this", and an overflow with "Move to cookbook" and "Discard".
/// Nothing here changes the recipe without an explicit tap.
class TrialBar extends ConsumerWidget {
  const TrialBar({super.key, required this.recipe, required this.onDiscarded});
  final Recipe recipe;

  /// Called after "Discard" (leave the recipe's page).
  final VoidCallback onDiscarded;

  Future<void> _madeThis(BuildContext context, WidgetRef ref) async {
    final result = await showVerdictSheet(context, recipe, mode: VerdictMode.madeThis);
    if (result == null || !context.mounted) return;
    final done = applyVerdictResult(context, ref, recipe, result);
    if (result.action == VerdictAction.discard) onDiscarded();
    await done;
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    return Container(
      key: const Key('trial-bar'),
      padding: const EdgeInsets.fromLTRB(12, 4, 0, 4),
      decoration: BoxDecoration(color: cs.tertiaryContainer, borderRadius: BorderRadius.circular(12)),
      child: Row(
        children: [
          Icon(Icons.science_outlined, size: 18, color: cs.onTertiaryContainer),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              'TRIAL · ${trialStatusLine(recipe)}',
              style: theme.textTheme.labelLarge?.copyWith(color: cs.onTertiaryContainer, letterSpacing: 0.6),
            ),
          ),
          TextButton.icon(
            key: const Key('made-this'),
            style: TextButton.styleFrom(foregroundColor: cs.onTertiaryContainer),
            onPressed: () => _madeThis(context, ref),
            icon: const Icon(Icons.check_rounded, size: 18),
            label: const Text('I made this'),
          ),
          PopupMenuButton<String>(
            tooltip: 'Trial options',
            icon: Icon(Icons.more_horiz_rounded, color: cs.onTertiaryContainer),
            onSelected: (v) {
              switch (v) {
                case 'cookbook':
                  moveToCookbook(ref, recipe);
                case 'discard':
                  discardWithUndo(context, ref, recipe);
                  onDiscarded();
              }
            },
            itemBuilder: (_) => [
              const PopupMenuItem(
                value: 'cookbook',
                child: ListTile(leading: Icon(Icons.bookmark_added_outlined), title: Text('Move to cookbook')),
              ),
              PopupMenuItem(
                value: 'discard',
                child: ListTile(
                  leading: Icon(Icons.delete_outline_rounded, color: cs.error),
                  title: Text('Discard', style: TextStyle(color: cs.error)),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
