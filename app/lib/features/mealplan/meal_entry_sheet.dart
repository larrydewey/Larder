import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/providers.dart';
import '../../data/backend.dart';
import '../../models/meal_plan.dart';
import '../../models/recipe.dart';
import '../../util/fractions.dart';
import '../../widgets/recipe_image.dart';
import '../../widgets/snack.dart';
import '../../widgets/trial_badge.dart';
import '../recipes/recipe_query.dart';

/// Bottom sheet to add or edit a meal plan entry (recipe or free text).
Future<void> showMealEntrySheet(BuildContext context, WidgetRef ref, {DateTime? date, MealPlanEntry? entry}) async {
  final result = await showModalBottomSheet<MealPlanEntry>(
    context: context,
    isScrollControlled: true,
    showDragHandle: true,
    builder: (_) => _MealEntrySheet(date: date, entry: entry),
  );
  if (result == null) return;
  final repo = ref.read(mealPlanRepositoryProvider);
  final o = entry == null ? await repo.create(result) : await repo.update(result);
  reportOutcome(o);
}

class _MealEntrySheet extends ConsumerStatefulWidget {
  const _MealEntrySheet({this.date, this.entry});
  final DateTime? date;
  final MealPlanEntry? entry;

  @override
  ConsumerState<_MealEntrySheet> createState() => _MealEntrySheetState();
}

class _MealEntrySheetState extends ConsumerState<_MealEntrySheet> {
  late DateTime _date = widget.entry != null ? parseDateKey(widget.entry!.date) : (widget.date ?? DateTime.now());
  late String _meal = widget.entry?.meal ?? 'dinner';
  late bool _freeText = widget.entry != null && widget.entry!.recipeId == null;
  Recipe? _recipe;
  late final _title = TextEditingController(text: widget.entry?.title ?? '');
  late final _note = TextEditingController(text: widget.entry?.note ?? '');
  late final _servings = TextEditingController(
    text: widget.entry?.servings == null ? '' : formatDecimal(widget.entry!.servings!),
  );
  final _search = TextEditingController();

  @override
  void dispose() {
    for (final c in [_title, _note, _servings, _search]) {
      c.dispose();
    }
    super.dispose();
  }

  void _submit() {
    final e = widget.entry;
    final recipeId = _freeText ? null : (_recipe?.id ?? e?.recipeId);
    if (!_freeText && recipeId == null) {
      showSnack('Pick a recipe or switch to a free-text entry');
      return;
    }
    if (_freeText && _title.text.trim().isEmpty) {
      showSnack('Enter what you are having');
      return;
    }
    Navigator.pop(
      context,
      MealPlanEntry(
        id: e?.id ?? newId(),
        date: dateKey(_date),
        meal: _meal,
        recipeId: recipeId,
        recipeName: _freeText ? null : (_recipe?.name ?? e?.recipeName),
        recipeImageUrl: _freeText ? null : (_recipe?.imageUrl ?? e?.recipeImageUrl),
        title: _freeText ? _title.text.trim() : null,
        note: _note.text.trim().isEmpty ? null : _note.text.trim(),
        servings: parseQuantity(_servings.text),
        createdBy: e?.createdBy,
        updatedAt: DateTime.now().toUtc(),
        trialOutcome: e?.trialOutcome,
        trialResolvedAt: e?.trialResolvedAt,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final recipes = ref.watch(recipesProvider).value ?? const <Recipe>[];
    // Trial recipes can be planned like any other.
    final matches = applyRecipeQuery(recipes, RecipeQuery(text: _search.text), includeTrial: true).take(30).toList();
    final selectedId = _recipe?.id ?? widget.entry?.recipeId;
    return Padding(
      padding: EdgeInsets.only(bottom: MediaQuery.viewInsetsOf(context).bottom),
      child: DraggableScrollableSheet(
        expand: false,
        initialChildSize: 0.85,
        maxChildSize: 0.95,
        builder: (context, scroll) => ListView(
          controller: scroll,
          padding: const EdgeInsets.fromLTRB(20, 0, 20, 24),
          children: [
            Text(widget.entry == null ? 'Plan a meal' : 'Edit meal', style: theme.textTheme.headlineSmall),
            const SizedBox(height: 12),
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(Icons.event_rounded),
              title: Text(DateFormat.yMMMMEEEEd().format(_date)),
              trailing: const Icon(Icons.edit_calendar_rounded),
              onTap: () async {
                final d = await showDatePicker(
                  context: context,
                  initialDate: _date,
                  firstDate: DateTime(2000),
                  lastDate: DateTime(2100),
                );
                if (d != null) setState(() => _date = d);
              },
            ),
            SegmentedButton<String>(
              showSelectedIcon: false,
              segments: [for (final m in mealTypes) ButtonSegment(value: m, label: Text(mealLabel(m)))],
              selected: {_meal},
              onSelectionChanged: (s) => setState(() => _meal = s.first),
            ),
            const SizedBox(height: 12),
            SegmentedButton<bool>(
              segments: const [
                ButtonSegment(value: false, icon: Icon(Icons.menu_book_outlined), label: Text('Recipe')),
                ButtonSegment(value: true, icon: Icon(Icons.edit_note_rounded), label: Text('Free text')),
              ],
              selected: {_freeText},
              onSelectionChanged: (s) => setState(() => _freeText = s.first),
            ),
            const SizedBox(height: 12),
            if (_freeText)
              TextField(
                controller: _title,
                decoration: const InputDecoration(
                  labelText: 'What are you having?',
                  hintText: 'Leftovers, eating out…',
                ),
              )
            else ...[
              TextField(
                controller: _search,
                decoration: const InputDecoration(prefixIcon: Icon(Icons.search_rounded), hintText: 'Search recipes'),
                onChanged: (_) => setState(() {}),
              ),
              const SizedBox(height: 8),
              SizedBox(
                height: 260,
                child: matches.isEmpty
                    ? const Center(child: Text('No recipes found'))
                    : ListView.builder(
                        itemCount: matches.length,
                        itemBuilder: (_, i) {
                          final r = matches[i];
                          final selected = r.id == selectedId;
                          return ListTile(
                            selected: selected,
                            leading: ClipRRect(
                              borderRadius: BorderRadius.circular(8),
                              child: SizedBox(width: 44, height: 44, child: RecipeImage.of(r, iconSize: 18)),
                            ),
                            title: Text(r.name),
                            subtitle: r.isTrial
                                ? const Align(alignment: Alignment.centerLeft, child: TrialBadge(dense: true))
                                : null,
                            trailing: selected ? const Icon(Icons.check_circle_rounded) : null,
                            onTap: () => setState(() {
                              _recipe = r;
                              if (_servings.text.isEmpty && r.yieldQuantity != null) {
                                _servings.text = formatDecimal(r.yieldQuantity!);
                              }
                            }),
                          );
                        },
                      ),
              ),
            ],
            const SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _servings,
                    keyboardType: const TextInputType.numberWithOptions(decimal: true),
                    decoration: const InputDecoration(labelText: 'Servings'),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  flex: 2,
                  child: TextField(
                    controller: _note,
                    decoration: const InputDecoration(labelText: 'Note'),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 20),
            FilledButton(onPressed: _submit, child: Text(widget.entry == null ? 'Add to plan' : 'Save')),
          ],
        ),
      ),
    );
  }
}
