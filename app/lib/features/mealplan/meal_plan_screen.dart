import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:intl/intl.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/providers.dart';
import '../../data/backend.dart';
import '../../models/meal_plan.dart';
import '../../util/fractions.dart';
import '../../widgets/recipe_image.dart';
import '../../widgets/snack.dart';
import '../recipes/recipe_actions.dart';
import 'meal_entry_sheet.dart';

DateTime _monday(DateTime d) {
  final day = DateTime(d.year, d.month, d.day);
  return day.subtract(Duration(days: day.weekday - DateTime.monday));
}

class MealPlanScreen extends ConsumerStatefulWidget {
  const MealPlanScreen({super.key});

  @override
  ConsumerState<MealPlanScreen> createState() => _MealPlanScreenState();
}

class _MealPlanScreenState extends ConsumerState<MealPlanScreen> {
  static const _origin = 1000;
  final _pages = PageController(initialPage: _origin);
  final _thisWeek = _monday(DateTime.now());
  int _page = _origin;

  DateTime _weekStart(int page) {
    final d = _thisWeek.add(Duration(days: 7 * (page - _origin)));
    return DateTime(d.year, d.month, d.day);
  }

  @override
  void dispose() {
    _pages.dispose();
    super.dispose();
  }

  void _onPage(int p) {
    setState(() => _page = p);
    final start = _weekStart(p);
    final now = DateTime.now();
    // The sync window covers today-30d..today+60d; fetch other weeks on demand.
    if (start.isBefore(now.subtract(const Duration(days: 30))) ||
        start.add(const Duration(days: 6)).isAfter(now.add(const Duration(days: 60)))) {
      unawaited(
        ref.read(mealPlanRepositoryProvider).fetchRange(start, start.add(const Duration(days: 6))).catchError((_) {}),
      );
    }
  }

  Future<void> _addWeekToList() async {
    final start = _weekStart(_page);
    final listId = await pickShoppingList(context, ref, title: 'Add this week to…');
    if (listId == null) return;
    final outcome = await ref
        .read(shoppingRepositoryProvider)
        .addMealPlan(listId, start, start.add(const Duration(days: 6)));
    reportOutcome(outcome, success: "This week's ingredients were added to your list");
  }

  @override
  Widget build(BuildContext context) {
    final start = _weekStart(_page);
    final end = start.add(const Duration(days: 6));
    final sameMonth = start.month == end.month;
    final range = sameMonth
        ? '${DateFormat.MMMd().format(start)} – ${DateFormat.d().format(end)}'
        : '${DateFormat.MMMd().format(start)} – ${DateFormat.MMMd().format(end)}';
    return Scaffold(
      appBar: AppBar(
        title: const Text('Meal Plan'),
        actions: [
          IconButton(
            tooltip: 'Add week to shopping list',
            icon: const Icon(Icons.add_shopping_cart_rounded),
            onPressed: _addWeekToList,
          ),
        ],
        bottom: PreferredSize(
          preferredSize: const Size.fromHeight(52),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8),
            child: Row(
              children: [
                IconButton(
                  tooltip: 'Previous week',
                  icon: const Icon(Icons.chevron_left_rounded),
                  onPressed: () =>
                      _pages.previousPage(duration: const Duration(milliseconds: 250), curve: Curves.easeOut),
                ),
                Expanded(
                  child: Text(range, textAlign: TextAlign.center, style: Theme.of(context).textTheme.titleMedium),
                ),
                if (_page != _origin)
                  TextButton(
                    onPressed: () => _pages.animateToPage(
                      _origin,
                      duration: const Duration(milliseconds: 300),
                      curve: Curves.easeOut,
                    ),
                    child: const Text('Today'),
                  ),
                IconButton(
                  tooltip: 'Next week',
                  icon: const Icon(Icons.chevron_right_rounded),
                  onPressed: () => _pages.nextPage(duration: const Duration(milliseconds: 250), curve: Curves.easeOut),
                ),
              ],
            ),
          ),
        ),
      ),
      body: PageView.builder(
        controller: _pages,
        onPageChanged: _onPage,
        itemBuilder: (_, page) => _WeekView(start: _weekStart(page)),
      ),
    );
  }
}

class _WeekView extends ConsumerWidget {
  const _WeekView({required this.start});
  final DateTime start;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final entries = ref.watch(mealPlanProvider).value ?? const <MealPlanEntry>[];
    final days = [for (var i = 0; i < 7; i++) start.add(Duration(days: i))];
    final byDay = <String, List<MealPlanEntry>>{};
    for (final e in entries) {
      byDay.putIfAbsent(e.date, () => []).add(e);
    }
    for (final l in byDay.values) {
      l.sort((a, b) {
        final c = mealTypes.indexOf(a.meal).compareTo(mealTypes.indexOf(b.meal));
        return c != 0 ? c : a.displayTitle.compareTo(b.displayTitle);
      });
    }
    return RefreshIndicator(
      onRefresh: () async {
        await ref
            .read(mealPlanRepositoryProvider)
            .fetchRange(start, start.add(const Duration(days: 6)))
            .catchError((_) {});
      },
      child: ListView.builder(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 32),
        itemCount: days.length,
        itemBuilder: (context, i) => MaxWidthDay(
          child: _DayCard(day: days[i], entries: byDay[dateKey(days[i])] ?? const []),
        ),
      ),
    );
  }
}

class MaxWidthDay extends StatelessWidget {
  const MaxWidthDay({super.key, required this.child});
  final Widget child;

  @override
  Widget build(BuildContext context) => Center(
    child: ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 860),
      child: Padding(padding: const EdgeInsets.only(bottom: 12), child: child),
    ),
  );
}

class _DayCard extends ConsumerWidget {
  const _DayCard({required this.day, required this.entries});
  final DateTime day;
  final List<MealPlanEntry> entries;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    final now = DateTime.now();
    final isToday = day.year == now.year && day.month == now.month && day.day == now.day;
    return Card(
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(
          color: isToday ? cs.primary : cs.outlineVariant.withValues(alpha: 0.6),
          width: isToday ? 1.5 : 1,
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 10, 8, 10),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Text(DateFormat.EEEE().format(day), style: theme.textTheme.titleLarge),
                const SizedBox(width: 8),
                Text(
                  DateFormat.MMMd().format(day),
                  style: theme.textTheme.labelLarge?.copyWith(color: cs.onSurfaceVariant),
                ),
                if (isToday) ...[
                  const SizedBox(width: 8),
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                    decoration: BoxDecoration(color: cs.primary, borderRadius: BorderRadius.circular(8)),
                    child: Text('Today', style: theme.textTheme.labelSmall?.copyWith(color: cs.onPrimary)),
                  ),
                ],
                const Spacer(),
                IconButton(
                  tooltip: 'Add meal',
                  icon: const Icon(Icons.add_circle_outline_rounded),
                  onPressed: () => showMealEntrySheet(context, ref, date: day),
                ),
              ],
            ),
            if (entries.isEmpty)
              Padding(
                padding: const EdgeInsets.only(bottom: 6),
                child: Text('Nothing planned', style: theme.textTheme.bodyMedium?.copyWith(color: cs.onSurfaceVariant)),
              ),
            for (final e in entries) _EntryTile(entry: e),
          ],
        ),
      ),
    );
  }
}

class _EntryTile extends ConsumerWidget {
  const _EntryTile({required this.entry});
  final MealPlanEntry entry;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final recipe = entry.recipeId == null ? null : ref.watch(recipeProvider(entry.recipeId!)).value;
    final title = entry.recipeName ?? recipe?.name ?? entry.title ?? 'Untitled';
    final image = entry.recipeImageUrl ?? recipe?.imageUrl;
    final subtitle = [
      mealLabel(entry.meal),
      if (entry.servings != null) '${formatFraction(entry.servings!)} servings',
      if (entry.note != null && entry.note!.isNotEmpty) entry.note!,
    ].join(' · ');
    return ListTile(
      contentPadding: const EdgeInsets.only(left: 0, right: 0),
      leading: ClipRRect(
        borderRadius: BorderRadius.circular(10),
        child: SizedBox(
          width: 52,
          height: 52,
          child: entry.recipeId == null
              ? Container(
                  color: Theme.of(context).colorScheme.secondaryContainer,
                  child: Icon(Icons.edit_note_rounded, color: Theme.of(context).colorScheme.onSecondaryContainer),
                )
              : RecipeImage(url: image, localPath: recipe?.localImage('thumb'), iconSize: 20),
        ),
      ),
      title: Text(title, maxLines: 1, overflow: TextOverflow.ellipsis),
      subtitle: Text(subtitle, maxLines: 1, overflow: TextOverflow.ellipsis),
      onTap: entry.recipeId != null
          ? () => context.go('/recipes/${entry.recipeId}')
          : () => showMealEntrySheet(context, ref, entry: entry),
      trailing: PopupMenuButton<String>(
        onSelected: (v) async {
          switch (v) {
            case 'edit':
              await showMealEntrySheet(context, ref, entry: entry);
            case 'delete':
              final o = await ref.read(mealPlanRepositoryProvider).delete(entry);
              reportOutcome(o);
          }
        },
        itemBuilder: (_) => const [
          PopupMenuItem(
            value: 'edit',
            child: ListTile(leading: Icon(Icons.edit_outlined), title: Text('Edit / move')),
          ),
          PopupMenuItem(
            value: 'delete',
            child: ListTile(leading: Icon(Icons.delete_outline), title: Text('Remove')),
          ),
        ],
      ),
    );
  }
}
