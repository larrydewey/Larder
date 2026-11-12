import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';
import 'package:share_plus/share_plus.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../api/endpoints.dart';
import '../../data/app_mode.dart';
import '../../data/preferences.dart';
import '../../data/providers.dart';
import '../../data/sharing.dart';
import '../../data/backend.dart';
import '../../models/recipe.dart';
import '../../util/scaling.dart';
import '../../util/share.dart';
import '../../util/timers.dart';
import '../../util/units.dart';
import '../../widgets/rating.dart';
import '../../widgets/recipe_image.dart';
import '../../widgets/snack.dart';
import '../../widgets/states.dart';
import '../trial/trial_bar.dart';
import 'ingredients_view.dart';
import 'recipe_actions.dart';

class RecipeDetailScreen extends ConsumerStatefulWidget {
  const RecipeDetailScreen({super.key, required this.id, this.base = '/recipes'});
  final String id;

  /// The list this page belongs to (`/recipes` or `/trial`); cook mode,
  /// the editor and "back" stay below it.
  final String base;

  @override
  ConsumerState<RecipeDetailScreen> createState() => _RecipeDetailScreenState();
}

class _RecipeDetailScreenState extends ConsumerState<RecipeDetailScreen> {
  double? _servings;
  UnitSystem? _system;
  final Set<int> _checked = {};
  bool _fetching = false;
  Object? _fetchError;

  Future<void> _fetch() async {
    setState(() {
      _fetching = true;
      _fetchError = null;
    });
    try {
      await ref.read(recipeRepositoryProvider).refresh(widget.id);
    } catch (e) {
      _fetchError = e;
    } finally {
      if (mounted) setState(() => _fetching = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final async = ref.watch(recipeProvider(widget.id));
    return async.when(
      loading: () => const Scaffold(body: LoadingView()),
      error: (e, _) => Scaffold(
        appBar: AppBar(),
        body: ErrorState(error: e),
      ),
      data: (recipe) {
        if (recipe == null) {
          if (!_fetching && _fetchError == null) {
            WidgetsBinding.instance.addPostFrameCallback((_) => _fetch());
          }
          return Scaffold(
            appBar: AppBar(),
            body: _fetchError != null ? ErrorState(error: _fetchError!, onRetry: _fetch) : const LoadingView(),
          );
        }
        return _buildRecipe(context, recipe);
      },
    );
  }

  Widget _buildRecipe(BuildContext context, Recipe r) {
    final theme = Theme.of(context);
    final UnitSystem system = _system ?? ref.watch(unitsPreferenceProvider);
    final base = r.yieldQuantity ?? 1;
    final servings = _servings ?? base;
    final factor = r.yieldQuantity == null ? servings : scaleFactor(r.yieldQuantity, servings);
    final width = MediaQuery.sizeOf(context).width;
    final wide = width >= 960;

    final ingredients = _Panel(
      title: 'Ingredients',
      trailing: null,
      children: [
        Wrap(
          spacing: 12,
          runSpacing: 8,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            ServingsStepper(
              servings: servings,
              unit: r.yieldQuantity == null ? '×' : r.yieldUnit,
              onChanged: (v) => setState(() => _servings = v),
            ),
            UnitToggle(value: system, onChanged: (v) => setState(() => _system = v)),
          ],
        ),
        const SizedBox(height: 8),
        if (r.ingredients.isEmpty) const Text('No ingredients listed.'),
        IngredientsList(
          ingredients: r.ingredients,
          factor: factor,
          system: system,
          checked: _checked,
          onToggle: (i) => setState(() => _checked.contains(i) ? _checked.remove(i) : _checked.add(i)),
        ),
      ],
    );

    final instructions = _Panel(
      title: 'Instructions',
      children: [if (r.instructions.isEmpty) const Text('No instructions yet.'), ..._instructionWidgets(context, r)],
    );

    final extras = [
      if (r.notes.trim().isNotEmpty)
        _Panel(
          title: 'Notes',
          children: [SelectableText(r.notes, style: theme.textTheme.bodyLarge)],
        ),
      if (r.nutrition.isNotEmpty) _Panel(title: 'Nutrition', children: [_NutritionTable(r.nutrition)]),
    ];

    return Scaffold(
      floatingActionButton: FloatingActionButton.extended(
        heroTag: 'cook',
        onPressed: () => context.push('${widget.base}/${r.id}/cook?servings=$servings'),
        icon: const Icon(Icons.play_arrow_rounded),
        label: const Text('Cook'),
      ),
      body: CustomScrollView(
        slivers: [
          SliverAppBar(
            pinned: true,
            stretch: true,
            expandedHeight: !r.hasImage ? null : (wide ? 380 : 280),
            flexibleSpace: !r.hasImage
                ? null
                : FlexibleSpaceBar(
                    background: Hero(
                      tag: 'recipe-image-${r.id}',
                      child: RecipeImage.of(r, size: 'medium', iconSize: 64),
                    ),
                  ),
            actions: [
              _CircleAction(
                tooltip: r.isFavorite ? 'Remove from favorites' : 'Add to favorites',
                icon: r.isFavorite ? Icons.favorite_rounded : Icons.favorite_border_rounded,
                onPressed: () => ref.read(recipeRepositoryProvider).setFavorite(r, !r.isFavorite),
              ),
              _CircleAction(
                tooltip: 'Edit',
                icon: Icons.edit_outlined,
                onPressed: () => context.push('${widget.base}/${r.id}/edit'),
              ),
              _menu(context, r, servings),
              const SizedBox(width: 4),
            ],
          ),
          SliverToBoxAdapter(
            child: MaxWidth(
              maxWidth: 1200,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(20, 16, 20, 120),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    _Header(
                      recipe: r,
                      belowTitle: r.isTrial ? TrialBar(recipe: r, onDiscarded: () => context.go(widget.base)) : null,
                    ),
                    const SizedBox(height: 8),
                    if (wide)
                      Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Expanded(flex: 2, child: ingredients),
                          const SizedBox(width: 24),
                          Expanded(flex: 3, child: Column(children: [instructions, ...extras])),
                        ],
                      )
                    else ...[
                      ingredients,
                      instructions,
                      ...extras,
                    ],
                  ],
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  List<Widget> _instructionWidgets(BuildContext context, Recipe r) {
    final theme = Theme.of(context);
    final out = <Widget>[];
    String? section;
    var n = 0;
    for (final step in r.instructions) {
      if (step.section != null && step.section != section) {
        out.add(
          Padding(
            padding: const EdgeInsets.fromLTRB(0, 16, 0, 4),
            child: Text(step.section!, style: theme.textTheme.titleSmall?.copyWith(color: theme.colorScheme.primary)),
          ),
        );
      }
      section = step.section;
      n++;
      final timers = detectTimers(step.text);
      out.add(
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 8),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              CircleAvatar(
                radius: 15,
                backgroundColor: theme.colorScheme.primaryContainer,
                child: Text(
                  '$n',
                  style: theme.textTheme.labelLarge?.copyWith(color: theme.colorScheme.onPrimaryContainer),
                ),
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SelectableText(step.text, style: theme.textTheme.bodyLarge?.copyWith(height: 1.5)),
                    if (timers.isNotEmpty)
                      Padding(
                        padding: const EdgeInsets.only(top: 6),
                        child: Wrap(
                          spacing: 6,
                          children: [
                            for (final t in timers)
                              Chip(
                                visualDensity: VisualDensity.compact,
                                avatar: const Icon(Icons.timer_outlined, size: 16),
                                label: Text(t.label),
                              ),
                          ],
                        ),
                      ),
                  ],
                ),
              ),
            ],
          ),
        ),
      );
    }
    return out;
  }

  Widget _menu(BuildContext context, Recipe r, double servings) {
    final shared = ref.watch(publicSharingProvider) && ref.watch(recipeShareProvider(r.id)).value != null;
    return PopupMenuButton<String>(
      tooltip: 'More',
      icon: const _CircleIcon(Icons.more_vert_rounded),
      onSelected: (v) async {
        final repo = ref.read(recipeRepositoryProvider);
        switch (v) {
          case 'mealplan':
            await showAddToMealPlan(context, ref, r, servings: servings);
          case 'shopping':
            await showAddToShoppingList(context, ref, r, servings: servings);
          case 'duplicate':
            final copy = await runOnline(() => repo.duplicate(r.id));
            if (copy != null && context.mounted) context.go('${widget.base}/${copy.id}');
          case 'share':
            Uri? link;
            if (ref.read(publicSharingProvider)) {
              // A public link anyone can open; sharing again reuses it.
              final share = await runOnline(() => ref.read(apiClientProvider).createRecipeShare(r.id));
              if (share == null) return;
              ref.invalidate(recipeShareProvider(r.id));
              link = Uri.tryParse(share.url);
            } else if (ref.read(appModeProvider) == AppMode.connected) {
              // The server does not allow public links: share the recipe's page for household members.
              final base = ref.read(apiClientProvider).baseUrl;
              link = recipeLink(base.isEmpty ? Uri.base.origin : base, r);
            }
            await SharePlus.instance.share(
              ShareParams(
                text: recipeShareText(r, link: link),
                subject: r.name,
              ),
            );
          case 'unshare':
            if (await confirm(
              context,
              title: 'Turn off the public link?',
              message:
                  'People with the link will no longer be able to open "${r.name}". Sharing again makes a new link.',
            )) {
              final done = await runOnline(() async {
                await ref.read(apiClientProvider).deleteRecipeShare(r.id);
                return true;
              });
              if (done == true) {
                ref.invalidate(recipeShareProvider(r.id));
                showSnack('Public link turned off');
              }
            }
          case 'source':
            final uri = Uri.tryParse(r.sourceUrl ?? '');
            if (uri != null) await launchUrl(uri, mode: LaunchMode.externalApplication);
          case 'delete':
            if (await confirm(
              context,
              title: 'Delete recipe?',
              message: '"${r.name}" will be removed for everyone in your household.',
            )) {
              unawaited(repo.delete(r).then((o) => reportOutcome(o, success: 'Recipe deleted')));
              if (context.mounted) context.go(widget.base);
            }
        }
      },
      itemBuilder: (_) => [
        const PopupMenuItem(
          value: 'mealplan',
          child: ListTile(leading: Icon(Icons.calendar_month_outlined), title: Text('Add to meal plan')),
        ),
        const PopupMenuItem(
          value: 'shopping',
          child: ListTile(leading: Icon(Icons.add_shopping_cart_rounded), title: Text('Add to shopping list')),
        ),
        const PopupMenuItem(
          value: 'duplicate',
          child: ListTile(leading: Icon(Icons.copy_rounded), title: Text('Duplicate')),
        ),
        const PopupMenuItem(
          value: 'share',
          child: ListTile(leading: Icon(Icons.share_outlined), title: Text('Share')),
        ),
        if (shared)
          const PopupMenuItem(
            value: 'unshare',
            child: ListTile(leading: Icon(Icons.link_off_rounded), title: Text('Turn off public link')),
          ),
        if (r.sourceUrl != null)
          const PopupMenuItem(
            value: 'source',
            child: ListTile(leading: Icon(Icons.open_in_new_rounded), title: Text('Open source')),
          ),
        const PopupMenuDivider(),
        PopupMenuItem(
          value: 'delete',
          child: ListTile(
            leading: Icon(Icons.delete_outline_rounded, color: Theme.of(context).colorScheme.error),
            title: Text('Delete', style: TextStyle(color: Theme.of(context).colorScheme.error)),
          ),
        ),
      ],
    );
  }
}

class _Header extends StatelessWidget {
  const _Header({required this.recipe, this.belowTitle});
  final Recipe recipe;

  /// Shown right under the name (the trial bar).
  final Widget? belowTitle;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    final r = recipe;
    Widget meta(IconData icon, String label, String value) => Container(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(color: cs.surfaceContainerHigh, borderRadius: BorderRadius.circular(14)),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 20, color: cs.primary),
          const SizedBox(width: 8),
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(label, style: theme.textTheme.labelSmall?.copyWith(color: cs.onSurfaceVariant)),
              Text(value, style: theme.textTheme.labelLarge),
            ],
          ),
        ],
      ),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (r.categories.isNotEmpty)
          Text(
            r.categories.join(' · ').toUpperCase(),
            style: theme.textTheme.labelMedium?.copyWith(color: cs.primary, letterSpacing: 1.2),
          ),
        const SizedBox(height: 4),
        SelectableText(r.name, style: theme.textTheme.displaySmall?.copyWith(height: 1.1)),
        if (belowTitle != null) ...[const SizedBox(height: 10), belowTitle!],
        if (r.rating != null && r.rating! > 0) ...[const SizedBox(height: 8), StarRating(rating: r.rating)],
        if (r.description.trim().isNotEmpty) ...[
          const SizedBox(height: 12),
          Text(r.description, style: theme.textTheme.bodyLarge?.copyWith(color: cs.onSurfaceVariant, height: 1.5)),
        ],
        const SizedBox(height: 16),
        Wrap(
          spacing: 10,
          runSpacing: 10,
          children: [
            if (r.prepTimeMinutes != null && r.prepTimeMinutes! > 0)
              meta(Icons.content_cut_rounded, 'Prep', formatMinutes(r.prepTimeMinutes!)),
            if (r.cookTimeMinutes != null && r.cookTimeMinutes! > 0)
              meta(Icons.local_fire_department_outlined, 'Cook', formatMinutes(r.cookTimeMinutes!)),
            if (r.effectiveTotalMinutes != null)
              meta(Icons.schedule_rounded, 'Total', formatMinutes(r.effectiveTotalMinutes!)),
            if (r.yieldQuantity != null)
              meta(Icons.restaurant_rounded, 'Makes', '${_num(r.yieldQuantity!)} ${r.yieldUnit}'),
          ],
        ),
        if (r.sourceUrl != null || r.sourceName != null) ...[
          const SizedBox(height: 12),
          InkWell(
            onTap: r.sourceUrl == null
                ? null
                : () => launchUrl(Uri.parse(r.sourceUrl!), mode: LaunchMode.externalApplication),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(Icons.link_rounded, size: 18, color: cs.onSurfaceVariant),
                const SizedBox(width: 6),
                Flexible(
                  child: Text(
                    r.sourceName ?? Uri.tryParse(r.sourceUrl!)?.host ?? r.sourceUrl!,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.bodyMedium?.copyWith(
                      color: r.sourceUrl == null ? cs.onSurfaceVariant : cs.primary,
                      decoration: r.sourceUrl == null ? null : TextDecoration.underline,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
        if (r.tags.isNotEmpty) ...[
          const SizedBox(height: 12),
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: [
              for (final t in r.tags)
                Chip(
                  visualDensity: VisualDensity.compact,
                  avatar: const Icon(Icons.tag_rounded, size: 14),
                  label: Text(t),
                ),
            ],
          ),
        ],
      ],
    );
  }

  static String _num(double v) => v == v.roundToDouble() ? v.toInt().toString() : v.toString();
}

class _Panel extends StatelessWidget {
  const _Panel({required this.title, required this.children, this.trailing});
  final String title;
  final List<Widget> children;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      SectionHeader(title, trailing: trailing),
      ...children,
    ],
  );
}

class _NutritionTable extends StatelessWidget {
  const _NutritionTable(this.nutrition);
  final Map<String, String> nutrition;

  String _label(String key) {
    final s = key.replaceAll('_', ' ').replaceAll(RegExp(r'content$'), '').trim();
    return s.isEmpty ? key : s[0].toUpperCase() + s.substring(1);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Card(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
        child: Column(
          children: [
            for (final e in nutrition.entries)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 6),
                child: Row(
                  children: [
                    Expanded(child: Text(_label(e.key), style: theme.textTheme.bodyMedium)),
                    Text(e.value, style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w600)),
                  ],
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _CircleIcon extends StatelessWidget {
  const _CircleIcon(this.icon);
  final IconData icon;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    return CircleAvatar(
      radius: 18,
      backgroundColor: cs.surface.withValues(alpha: 0.85),
      child: Icon(icon, size: 20, color: cs.onSurface),
    );
  }
}

class _CircleAction extends StatelessWidget {
  const _CircleAction({required this.tooltip, required this.icon, required this.onPressed});
  final String tooltip;
  final IconData icon;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) => IconButton(tooltip: tooltip, onPressed: onPressed, icon: _CircleIcon(icon));
}
