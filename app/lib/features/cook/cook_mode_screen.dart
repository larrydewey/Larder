import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

import '../../data/preferences.dart';
import '../../data/providers.dart';
import '../../models/recipe.dart';
import '../../util/scaling.dart';
import '../../util/timers.dart';
import '../../widgets/states.dart';
import '../recipes/ingredients_view.dart';
import 'cook_timers.dart';

/// Full-screen step-by-step cooking view.
class CookModeScreen extends ConsumerStatefulWidget {
  const CookModeScreen({super.key, required this.id, this.servings});
  final String id;
  final double? servings;

  @override
  ConsumerState<CookModeScreen> createState() => _CookModeScreenState();
}

class _CookModeScreenState extends ConsumerState<CookModeScreen> {
  final _pages = PageController();
  int _page = 0;
  final Set<int> _checked = {};

  @override
  void initState() {
    super.initState();
    WakelockPlus.enable().catchError((_) {});
  }

  @override
  void dispose() {
    WakelockPlus.disable().catchError((_) {});
    _pages.dispose();
    super.dispose();
  }

  void _goTo(int page, int count) {
    if (page < 0 || page >= count) return;
    _pages.animateToPage(page, duration: const Duration(milliseconds: 280), curve: Curves.easeOutCubic);
  }

  void _close() => context.canPop() ? context.pop() : context.go('/recipes/${widget.id}');

  @override
  Widget build(BuildContext context) {
    final recipe = ref.watch(recipeProvider(widget.id)).value;
    if (recipe == null) return Scaffold(appBar: AppBar(), body: const LoadingView());
    final steps = recipe.instructions.isEmpty
        ? [const Instruction(text: 'No instructions for this recipe.')]
        : recipe.instructions;
    final count = steps.length;
    final theme = Theme.of(context);

    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.arrowRight): () => _goTo(_page + 1, count),
        const SingleActivator(LogicalKeyboardKey.arrowLeft): () => _goTo(_page - 1, count),
        const SingleActivator(LogicalKeyboardKey.escape): _close,
      },
      child: Focus(
        autofocus: true,
        child: Scaffold(
          appBar: AppBar(
            leading: IconButton(tooltip: 'Exit cook mode', icon: const Icon(Icons.close_rounded), onPressed: _close),
            title: Text(recipe.name, overflow: TextOverflow.ellipsis),
            actions: [
              TextButton.icon(
                onPressed: () => _showIngredients(recipe),
                icon: const Icon(Icons.format_list_bulleted_rounded),
                label: const Text('Ingredients'),
              ),
              const SizedBox(width: 8),
            ],
            bottom: PreferredSize(
              preferredSize: const Size.fromHeight(4),
              child: LinearProgressIndicator(value: (_page + 1) / count, minHeight: 4),
            ),
          ),
          body: Column(
            children: [
              Expanded(
                child: PageView.builder(
                  controller: _pages,
                  itemCount: count,
                  onPageChanged: (p) => setState(() => _page = p),
                  itemBuilder: (_, i) => _StepPage(
                    index: i,
                    count: count,
                    step: steps[i],
                    onStartTimer: (t) =>
                        ref.read(cookTimersProvider.notifier).start(t.label, t.duration, recipeName: recipe.name),
                  ),
                ),
              ),
              const _ActiveTimers(),
              SafeArea(
                top: false,
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 12),
                  child: Row(
                    children: [
                      OutlinedButton.icon(
                        onPressed: _page > 0 ? () => _goTo(_page - 1, count) : null,
                        icon: const Icon(Icons.arrow_back_rounded),
                        label: const Text('Back'),
                      ),
                      Expanded(
                        child: Text(
                          'Step ${_page + 1} of $count',
                          textAlign: TextAlign.center,
                          style: theme.textTheme.labelLarge,
                        ),
                      ),
                      _page < count - 1
                          ? FilledButton.icon(
                              onPressed: () => _goTo(_page + 1, count),
                              icon: const Icon(Icons.arrow_forward_rounded),
                              label: const Text('Next'),
                            )
                          : FilledButton.icon(
                              onPressed: _close,
                              icon: const Icon(Icons.check_rounded),
                              label: const Text('Done'),
                            ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  void _showIngredients(Recipe recipe) {
    final servings = widget.servings ?? recipe.yieldQuantity ?? 1;
    final factor = scaleFactor(recipe.yieldQuantity, servings);
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (context) => StatefulBuilder(
        builder: (context, setSheet) => DraggableScrollableSheet(
          expand: false,
          initialChildSize: 0.7,
          maxChildSize: 0.95,
          builder: (context, scroll) => ListView(
            controller: scroll,
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
            children: [
              Text('Ingredients', style: Theme.of(context).textTheme.headlineSmall),
              IngredientsList(
                ingredients: recipe.ingredients,
                factor: factor,
                system: ref.read(unitsPreferenceProvider),
                checked: _checked,
                large: true,
                onToggle: (i) => setSheet(() => _checked.contains(i) ? _checked.remove(i) : _checked.add(i)),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _StepPage extends StatelessWidget {
  const _StepPage({required this.index, required this.count, required this.step, required this.onStartTimer});
  final int index;
  final int count;
  final Instruction step;
  final ValueChanged<DetectedTimer> onStartTimer;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final timers = detectTimers(step.text);
    final wide = MediaQuery.sizeOf(context).width > 700;
    return SingleChildScrollView(
      padding: EdgeInsets.symmetric(horizontal: wide ? 64 : 24, vertical: 28),
      child: MaxWidth(
        maxWidth: 900,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (step.section != null)
              Text(
                step.section!.toUpperCase(),
                style: theme.textTheme.labelLarge?.copyWith(color: theme.colorScheme.primary, letterSpacing: 1.4),
              ),
            Text(
              'Step ${index + 1}',
              style: theme.textTheme.headlineMedium?.copyWith(color: theme.colorScheme.primary),
            ),
            const SizedBox(height: 16),
            Text(
              step.text,
              style: theme.textTheme.headlineSmall?.copyWith(
                fontFamily: 'Inter',
                fontSize: wide ? 30 : 24,
                height: 1.45,
              ),
            ),
            if (timers.isNotEmpty) ...[
              const SizedBox(height: 28),
              Wrap(
                spacing: 12,
                runSpacing: 12,
                children: [
                  for (final t in timers)
                    FilledButton.tonalIcon(
                      style: FilledButton.styleFrom(padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16)),
                      onPressed: () => onStartTimer(t),
                      icon: const Icon(Icons.timer_outlined),
                      label: Text('Start ${formatClock(t.duration)} timer'),
                    ),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _ActiveTimers extends ConsumerWidget {
  const _ActiveTimers();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final timers = ref.watch(cookTimersProvider);
    if (timers.isEmpty) return const SizedBox.shrink();
    final ctrl = ref.read(cookTimersProvider.notifier);
    final cs = Theme.of(context).colorScheme;
    return Container(
      color: cs.surfaceContainerHigh,
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 12),
        child: Row(
          children: [
            for (final t in timers)
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 4),
                child: Container(
                  padding: const EdgeInsets.fromLTRB(14, 6, 4, 6),
                  decoration: BoxDecoration(
                    color: t.done ? cs.errorContainer : cs.primaryContainer,
                    borderRadius: BorderRadius.circular(28),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      SizedBox(
                        width: 22,
                        height: 22,
                        child: CircularProgressIndicator(value: t.progress, strokeWidth: 3),
                      ),
                      const SizedBox(width: 10),
                      Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text(
                            formatClock(t.remaining),
                            style: Theme.of(context).textTheme.titleMedium
                                ?.copyWith(fontFeatures: const [FontFeature.tabularFigures()]),
                          ),
                          Text(t.label, style: Theme.of(context).textTheme.labelSmall),
                        ],
                      ),
                      const SizedBox(width: 4),
                      if (!t.done)
                        IconButton(
                          tooltip: t.running ? 'Pause' : 'Resume',
                          icon: Icon(t.running ? Icons.pause_rounded : Icons.play_arrow_rounded),
                          onPressed: () => t.running ? ctrl.pause(t.id) : ctrl.resume(t.id),
                        ),
                      IconButton(
                        tooltip: 'Cancel timer',
                        icon: const Icon(Icons.close_rounded),
                        onPressed: () => ctrl.cancel(t.id),
                      ),
                    ],
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}
