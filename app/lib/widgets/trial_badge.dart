import 'package:material_ui/material_ui.dart';

/// Small "Trial" pill shown wherever a trial recipe appears outside the
/// trial list (search results, meal plan, pickers).
class TrialBadge extends StatelessWidget {
  const TrialBadge({super.key, this.dense = false});
  final bool dense;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    return Container(
      padding: EdgeInsets.symmetric(horizontal: dense ? 6 : 8, vertical: dense ? 1 : 2),
      decoration: BoxDecoration(color: cs.tertiaryContainer, borderRadius: BorderRadius.circular(8)),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.science_outlined, size: dense ? 12 : 14, color: cs.onTertiaryContainer),
          const SizedBox(width: 3),
          Text('Trial', style: theme.textTheme.labelSmall?.copyWith(color: cs.onTertiaryContainer)),
        ],
      ),
    );
  }
}
