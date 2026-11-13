import 'package:material_ui/material_ui.dart';

import '../../models/recipe.dart';
import '../../widgets/rating.dart';

/// Where the "How did it go?" sheet was opened from.
enum VerdictMode {
  /// Leaving cook mode: the last button is "Just looking".
  cookMode,

  /// The "I made this" button: the last button is a plain "Cancel".
  madeThis,

  /// "Did you make it?" on a past meal plan entry: the last button is "Skipped".
  mealPlan,
}

enum VerdictAction { keep, again, discard, skipped }

class VerdictResult {
  const VerdictResult(this.action, {this.rating, this.note = ''});
  final VerdictAction action;

  /// Stars picked in the sheet, or null to leave the rating alone.
  final int? rating;
  final String note;
}

/// Asks how a trial recipe went. Returns null for "Just looking" / "Cancel"
/// and for every dismissal (swipe down, tap outside, back): nothing is
/// recorded then. With [finished] false (cook mode left before the last
/// step), "Just looking" is the highlighted action.
Future<VerdictResult?> showVerdictSheet(
  BuildContext context,
  Recipe recipe, {
  required VerdictMode mode,
  bool finished = true,
}) => showModalBottomSheet<VerdictResult>(
  context: context,
  isScrollControlled: true,
  showDragHandle: true,
  builder: (_) => VerdictSheet(recipe: recipe, mode: mode, finished: finished),
);

class VerdictSheet extends StatefulWidget {
  const VerdictSheet({super.key, required this.recipe, required this.mode, this.finished = true});
  final Recipe recipe;
  final VerdictMode mode;
  final bool finished;

  @override
  State<VerdictSheet> createState() => _VerdictSheetState();
}

class _VerdictSheetState extends State<VerdictSheet> {
  int? _rating;
  final _note = TextEditingController();

  @override
  void dispose() {
    _note.dispose();
    super.dispose();
  }

  void _done(VerdictAction a) => Navigator.pop(context, VerdictResult(a, rating: _rating, note: _note.text.trim()));

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    final lookingFirst = widget.mode == VerdictMode.cookMode && !widget.finished;

    Widget button({
      required Key key,
      required IconData icon,
      required String label,
      required String hint,
      required VoidCallback onPressed,
      bool highlighted = false,
      Color? color,
    }) {
      final child = Row(
        children: [
          Icon(icon, size: 20),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(label),
                Text(
                  hint,
                  style: theme.textTheme.bodySmall?.copyWith(color: highlighted ? cs.onPrimary : cs.onSurfaceVariant),
                ),
              ],
            ),
          ),
        ],
      );
      final padding = const EdgeInsets.symmetric(horizontal: 16, vertical: 10);
      return Padding(
        padding: const EdgeInsets.only(bottom: 8),
        child: highlighted
            ? FilledButton(
                key: key,
                style: FilledButton.styleFrom(padding: padding),
                onPressed: onPressed,
                child: child,
              )
            : OutlinedButton(
                key: key,
                style: OutlinedButton.styleFrom(padding: padding, foregroundColor: color),
                onPressed: onPressed,
                child: child,
              ),
      );
    }

    final (lastLabel, lastHint, lastIcon) = switch (widget.mode) {
      VerdictMode.cookMode => ('Just looking', 'Nothing is recorded', Icons.visibility_outlined),
      VerdictMode.madeThis => ('Cancel', 'Nothing is recorded', Icons.close_rounded),
      VerdictMode.mealPlan => ('Skipped', 'We did not make it', Icons.event_busy_outlined),
    };
    final last = button(
      key: const Key('verdict-last'),
      icon: lastIcon,
      label: lastLabel,
      hint: lastHint,
      highlighted: lookingFirst,
      onPressed: () => widget.mode == VerdictMode.mealPlan ? _done(VerdictAction.skipped) : Navigator.pop(context),
    );

    return Padding(
      padding: EdgeInsets.only(bottom: MediaQuery.viewInsetsOf(context).bottom),
      child: SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(20, 0, 20, 20),
        child: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 560),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  widget.mode == VerdictMode.mealPlan ? 'Did you make it?' : 'How did it go?',
                  style: theme.textTheme.headlineSmall,
                ),
                const SizedBox(height: 2),
                Text(widget.recipe.name, style: theme.textTheme.bodyMedium?.copyWith(color: cs.onSurfaceVariant)),
                const SizedBox(height: 12),
                Row(
                  children: [
                    const Text('Rating'),
                    const SizedBox(width: 8),
                    StarRating(rating: _rating, size: 28, onChanged: (v) => setState(() => _rating = v)),
                  ],
                ),
                const SizedBox(height: 8),
                TextField(
                  key: const Key('verdict-note'),
                  controller: _note,
                  maxLines: 2,
                  minLines: 1,
                  textCapitalization: TextCapitalization.sentences,
                  decoration: const InputDecoration(
                    labelText: 'Note (optional)',
                    hintText: 'e.g. too salty, cut soy by half',
                  ),
                ),
                const SizedBox(height: 16),
                if (lookingFirst) last,
                button(
                  key: const Key('verdict-keep'),
                  icon: Icons.bookmark_added_outlined,
                  label: 'Keep it',
                  hint: 'Move it to the cookbook',
                  highlighted: !lookingFirst,
                  onPressed: () => _done(VerdictAction.keep),
                ),
                button(
                  key: const Key('verdict-again'),
                  icon: Icons.replay_rounded,
                  label: 'Try again',
                  hint: 'It stays on the trial list',
                  onPressed: () => _done(VerdictAction.again),
                ),
                button(
                  key: const Key('verdict-discard'),
                  icon: Icons.thumb_down_alt_outlined,
                  label: 'Not for us',
                  hint: 'Remove the recipe',
                  color: cs.error,
                  onPressed: () => _done(VerdictAction.discard),
                ),
                if (!lookingFirst) last,
              ],
            ),
          ),
        ),
      ),
    );
  }
}
