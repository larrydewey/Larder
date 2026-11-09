import 'package:material_ui/material_ui.dart';

import '../../models/recipe.dart';
import '../../util/fractions.dart';
import '../../util/scaling.dart';
import '../../util/units.dart';

/// Ingredient list grouped by section, with checkboxes.
class IngredientsList extends StatelessWidget {
  const IngredientsList({
    super.key,
    required this.ingredients,
    required this.factor,
    required this.system,
    required this.checked,
    required this.onToggle,
    this.large = false,
  });

  final List<Ingredient> ingredients;
  final double factor;
  final UnitSystem system;
  final Set<int> checked;
  final ValueChanged<int> onToggle;
  final bool large;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final indexed = [for (var i = 0; i < ingredients.length; i++) (i, ingredients[i])];
    final groups = groupBySection(indexed, (e) => e.$2.section);
    final textStyle = large ? theme.textTheme.titleMedium : theme.textTheme.bodyLarge;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final g in groups) ...[
          if (g.key != null)
            Padding(
              padding: const EdgeInsets.fromLTRB(4, 16, 4, 4),
              child: Text(
                g.key!,
                style: theme.textTheme.titleSmall?.copyWith(color: theme.colorScheme.primary, letterSpacing: 0.4),
              ),
            ),
          for (final (i, ing) in g.value)
            _IngredientRow(
              display: displayIngredient(ing, factor: factor, system: system),
              checked: checked.contains(i),
              onTap: () => onToggle(i),
              style: textStyle,
            ),
        ],
      ],
    );
  }
}

class _IngredientRow extends StatelessWidget {
  const _IngredientRow({required this.display, required this.checked, required this.onTap, this.style});
  final IngredientDisplay display;
  final bool checked;
  final VoidCallback onTap;
  final TextStyle? style;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    final base = style?.copyWith(
      decoration: checked ? TextDecoration.lineThrough : null,
      color: checked ? cs.onSurfaceVariant.withValues(alpha: 0.6) : null,
    );
    return InkWell(
      borderRadius: BorderRadius.circular(8),
      onTap: onTap,
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 2),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Checkbox(value: checked, onChanged: (_) => onTap(), visualDensity: VisualDensity.compact),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.only(top: 10, right: 4),
                child: Text.rich(
                  TextSpan(
                    style: base,
                    children: [
                      if (display.amount.isNotEmpty)
                        TextSpan(
                          text: '${display.amount} ',
                          style: const TextStyle(fontWeight: FontWeight.w700),
                        ),
                      TextSpan(text: display.text),
                    ],
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// `- 4 servings +` control.
class ServingsStepper extends StatelessWidget {
  const ServingsStepper({super.key, required this.servings, required this.unit, required this.onChanged});
  final double servings;
  final String unit;
  final ValueChanged<double> onChanged;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    final step = servings <= 1 ? 0.5 : 1.0;
    return Container(
      decoration: BoxDecoration(color: cs.surfaceContainerHigh, borderRadius: BorderRadius.circular(24)),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          IconButton(
            key: const Key('servings-minus'),
            tooltip: 'Fewer',
            visualDensity: VisualDensity.compact,
            onPressed: servings > 0.5 ? () => onChanged((servings - step).clamp(0.5, 999)) : null,
            icon: const Icon(Icons.remove_rounded),
          ),
          Text(
            '${formatFraction(servings)} $unit',
            key: const Key('servings-label'),
            style: Theme.of(context).textTheme.labelLarge,
          ),
          IconButton(
            key: const Key('servings-plus'),
            tooltip: 'More',
            visualDensity: VisualDensity.compact,
            onPressed: () => onChanged(servings < 1 ? servings + 0.5 : servings + 1),
            icon: const Icon(Icons.add_rounded),
          ),
        ],
      ),
    );
  }
}

class UnitToggle extends StatelessWidget {
  const UnitToggle({super.key, required this.value, required this.onChanged});
  final UnitSystem value;
  final ValueChanged<UnitSystem> onChanged;

  @override
  Widget build(BuildContext context) {
    return SegmentedButton<UnitSystem>(
      showSelectedIcon: false,
      style: const ButtonStyle(visualDensity: VisualDensity.compact),
      segments: [for (final u in UnitSystem.values) ButtonSegment(value: u, label: Text(u.label))],
      selected: {value},
      onSelectionChanged: (s) => onChanged(s.first),
    );
  }
}
