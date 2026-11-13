import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/providers.dart';
import '../../data/sync_controller.dart';
import '../add/add_sheet.dart';
import '../../models/recipe.dart';
import '../../widgets/recipe_image.dart';
import '../../widgets/states.dart';
import 'trial_actions.dart';
import 'trial_text.dart';

/// Recipes the household wants to try, kept apart from the cookbook:
/// "Not made yet" (newest first) and "Tried, undecided".
class TrialScreen extends ConsumerWidget {
  const TrialScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final loading = ref.watch(recipesProvider).isLoading;
    final trials = ref.watch(trialRecipesProvider);
    final epoch = DateTime.fromMillisecondsSinceEpoch(0);
    final notMade = trials.where((r) => r.triedCount <= 0).toList()
      ..sort((a, b) => (b.createdAt ?? b.updatedAt ?? epoch).compareTo(a.createdAt ?? a.updatedAt ?? epoch));
    final tried = trials.where((r) => r.triedCount > 0).toList()
      ..sort((a, b) => (b.lastTriedAt ?? epoch).compareTo(a.lastTriedAt ?? epoch));

    void open(Recipe r) => context.go('/trial/${r.id}');

    return Scaffold(
      appBar: AppBar(title: Text(trials.isEmpty ? 'Trial' : 'Trial · ${trials.length}')),
      floatingActionButton: FloatingActionButton.extended(
        key: const Key('add-trial-recipe'),
        heroTag: 'new-trial-recipe',
        onPressed: () => openAdd(context, ref, base: '/trial'),
        icon: const Icon(Icons.add_rounded),
        label: const Text('Recipe to try'),
      ),
      body: loading && trials.isEmpty
          ? const LoadingView()
          : trials.isEmpty
          ? const EmptyState(
              icon: Icons.science_outlined,
              title: 'Nothing to try yet',
              message:
                  'Recipes you want to try before they join your cookbook live here. '
                  'Recipes imported from a website, text or a photo start here too.',
            )
          : RefreshIndicator(
              onRefresh: () => ref.read(syncProvider.notifier).sync(),
              child: ListView(
                padding: const EdgeInsets.fromLTRB(16, 0, 16, 96),
                children: [
                  if (notMade.isNotEmpty) ...[
                    const MaxWidth(child: SectionHeader('Not made yet', padding: EdgeInsets.fromLTRB(0, 12, 0, 8))),
                    for (final r in notMade)
                      MaxWidth(
                        child: _TrialTile(recipe: r, subtitle: addedAgo(r.createdAt), onTap: () => open(r)),
                      ),
                  ],
                  if (tried.isNotEmpty) ...[
                    const MaxWidth(child: SectionHeader('Tried, undecided', padding: EdgeInsets.fromLTRB(0, 20, 0, 8))),
                    for (final r in tried)
                      MaxWidth(
                        child: _TrialTile(recipe: r, subtitle: trialStatusLine(r), onTap: () => open(r)),
                      ),
                  ],
                ],
              ),
            ),
    );
  }
}

class _TrialTile extends StatelessWidget {
  const _TrialTile({required this.recipe, required this.subtitle, required this.onTap});
  final Recipe recipe;
  final String subtitle;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: Card(
        child: InkWell(
          onTap: onTap,
          child: Row(
            children: [
              SizedBox(
                width: 84,
                height: 72,
                child: Hero(tag: 'recipe-image-${recipe.id}', child: RecipeImage.of(recipe, iconSize: 24)),
              ),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        recipe.name,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.titleMedium,
                      ),
                      const SizedBox(height: 4),
                      Text(
                        subtitle,
                        style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                      ),
                    ],
                  ),
                ),
              ),
              const Padding(padding: EdgeInsets.only(right: 12), child: Icon(Icons.chevron_right_rounded)),
            ],
          ),
        ),
      ),
    );
  }
}
