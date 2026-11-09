import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';
import 'package:material_ui/material_ui.dart';

import '../../api/api_exception.dart';
import '../../data/providers.dart';
import '../../data/backend.dart';
import '../../models/meal_plan.dart';
import '../../models/recipe.dart';
import '../../models/shopping.dart';
import '../../util/fractions.dart';
import '../../widgets/snack.dart';

/// Dialog: pick a date + meal (+ servings) and add [recipe] to the plan.
Future<void> showAddToMealPlan(BuildContext context, WidgetRef ref, Recipe recipe, {double? servings}) async {
  final result = await showDialog<MealPlanEntry>(
    context: context,
    builder: (_) => _AddToMealPlanDialog(recipe: recipe, servings: servings ?? recipe.yieldQuantity),
  );
  if (result == null) return;
  final outcome = await ref.read(mealPlanRepositoryProvider).create(result);
  reportOutcome(outcome, success: 'Added to ${DateFormat.MMMEd().format(parseDateKey(result.date))}');
}

class _AddToMealPlanDialog extends StatefulWidget {
  const _AddToMealPlanDialog({required this.recipe, this.servings});
  final Recipe recipe;
  final double? servings;

  @override
  State<_AddToMealPlanDialog> createState() => _AddToMealPlanDialogState();
}

class _AddToMealPlanDialogState extends State<_AddToMealPlanDialog> {
  DateTime _date = DateTime.now();
  String _meal = 'dinner';
  late final _servings = TextEditingController(text: widget.servings == null ? '' : formatDecimal(widget.servings!));

  @override
  void dispose() {
    _servings.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Add to meal plan'),
      content: SizedBox(
        width: 360,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(Icons.event_rounded),
              title: Text(DateFormat.yMMMMEEEEd().format(_date)),
              trailing: const Icon(Icons.edit_calendar_rounded),
              onTap: () async {
                final d = await showDatePicker(
                  context: context,
                  initialDate: _date,
                  firstDate: DateTime.now().subtract(const Duration(days: 365)),
                  lastDate: DateTime.now().add(const Duration(days: 365 * 2)),
                );
                if (d != null) setState(() => _date = d);
              },
            ),
            const SizedBox(height: 8),
            SegmentedButton<String>(
              showSelectedIcon: false,
              segments: [for (final m in mealTypes) ButtonSegment(value: m, label: Text(mealLabel(m)))],
              selected: {_meal},
              onSelectionChanged: (s) => setState(() => _meal = s.first),
            ),
            const SizedBox(height: 16),
            TextField(
              controller: _servings,
              keyboardType: const TextInputType.numberWithOptions(decimal: true),
              decoration: const InputDecoration(labelText: 'Servings', prefixIcon: Icon(Icons.people_outline)),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancel')),
        FilledButton(
          onPressed: () => Navigator.pop(
            context,
            MealPlanEntry(
              id: newId(),
              date: dateKey(_date),
              meal: _meal,
              recipeId: widget.recipe.id,
              recipeName: widget.recipe.name,
              recipeImageUrl: widget.recipe.imageUrl,
              servings: parseQuantity(_servings.text),
              updatedAt: DateTime.now().toUtc(),
            ),
          ),
          child: const Text('Add'),
        ),
      ],
    );
  }
}

/// Picks a shopping list (or creates one). Returns the list id.
Future<String?> pickShoppingList(BuildContext context, WidgetRef ref, {String title = 'Choose a list'}) async {
  final lists = ref.read(shoppingListsProvider).value ?? const <ShoppingList>[];
  final picked = await showDialog<String>(
    context: context,
    builder: (context) => SimpleDialog(
      title: Text(title),
      children: [
        for (final l in lists)
          SimpleDialogOption(
            onPressed: () => Navigator.pop(context, l.id),
            child: ListTile(leading: const Icon(Icons.shopping_basket_outlined), title: Text(l.name)),
          ),
        SimpleDialogOption(
          onPressed: () => Navigator.pop(context, '__new__'),
          child: const ListTile(leading: Icon(Icons.add_rounded), title: Text('New list…')),
        ),
      ],
    ),
  );
  if (picked != '__new__' || !context.mounted) return picked;
  final name = await promptText(context, title: 'New shopping list', label: 'Name', initial: 'Groceries');
  if (name == null || name.trim().isEmpty) return null;
  final list = await ref.read(shoppingRepositoryProvider).createList(name.trim());
  return list.id;
}

Future<void> showAddToShoppingList(BuildContext context, WidgetRef ref, Recipe recipe, {double? servings}) async {
  final listId = await pickShoppingList(context, ref, title: 'Add ingredients to…');
  if (listId == null || !context.mounted) return;
  final s = await promptText(
    context,
    title: 'Servings',
    label: recipe.yieldUnit,
    initial: formatDecimal(servings ?? recipe.yieldQuantity ?? 1),
    keyboardType: const TextInputType.numberWithOptions(decimal: true),
  );
  if (s == null) return;
  final outcome = await ref.read(shoppingRepositoryProvider).addRecipes(listId, [
    (recipeId: recipe.id, servings: parseQuantity(s)),
  ]);
  reportOutcome(outcome, success: 'Ingredients added to your shopping list');
}

/// Simple single-field prompt dialog.
Future<String?> promptText(
  BuildContext context, {
  required String title,
  String? label,
  String initial = '',
  String confirm = 'OK',
  TextInputType? keyboardType,
  int maxLines = 1,
}) => showDialog<String>(
  context: context,
  builder: (_) => _PromptDialog(
    title: title,
    label: label,
    initial: initial,
    confirm: confirm,
    keyboardType: keyboardType,
    maxLines: maxLines,
  ),
);

class _PromptDialog extends StatefulWidget {
  const _PromptDialog({
    required this.title,
    this.label,
    required this.initial,
    required this.confirm,
    this.keyboardType,
    required this.maxLines,
  });
  final String title;
  final String? label;
  final String initial;
  final String confirm;
  final TextInputType? keyboardType;
  final int maxLines;

  @override
  State<_PromptDialog> createState() => _PromptDialogState();
}

class _PromptDialogState extends State<_PromptDialog> {
  late final _c = TextEditingController(text: widget.initial);

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: Text(widget.title),
    content: SizedBox(
      width: 360,
      child: TextField(
        controller: _c,
        autofocus: true,
        keyboardType: widget.keyboardType,
        maxLines: widget.maxLines,
        decoration: InputDecoration(labelText: widget.label),
        onSubmitted: widget.maxLines == 1 ? (v) => Navigator.pop(context, v) : null,
      ),
    ),
    actions: [
      TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancel')),
      FilledButton(onPressed: () => Navigator.pop(context, _c.text), child: Text(widget.confirm)),
    ],
  );
}

Future<bool> confirm(
  BuildContext context, {
  required String title,
  String? message,
  String action = 'Delete',
  bool destructive = true,
}) async {
  final cs = Theme.of(context).colorScheme;
  final r = await showDialog<bool>(
    context: context,
    builder: (context) => AlertDialog(
      title: Text(title),
      content: message == null ? null : Text(message),
      actions: [
        TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('Cancel')),
        FilledButton(
          style: destructive ? FilledButton.styleFrom(backgroundColor: cs.error, foregroundColor: cs.onError) : null,
          onPressed: () => Navigator.pop(context, true),
          child: Text(action),
        ),
      ],
    ),
  );
  return r ?? false;
}

/// Runs an online-only action, showing a friendly message when offline.
Future<T?> runOnline<T>(Future<T> Function() action) async {
  try {
    return await action();
  } on NetworkException {
    showSnack('This needs a connection to the server.', error: true);
  } catch (e) {
    showError(e);
  }
  return null;
}
