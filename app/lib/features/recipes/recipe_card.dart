import 'package:material_ui/material_ui.dart';

import '../../models/recipe.dart';
import '../../util/timers.dart';
import '../../widgets/recipe_image.dart';
import '../../widgets/trial_badge.dart';

class RecipeCard extends StatelessWidget {
  const RecipeCard({super.key, required this.recipe, required this.onTap});
  final Recipe recipe;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    return Card(
      child: InkWell(
        onTap: onTap,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Expanded(
              child: Stack(
                fit: StackFit.expand,
                children: [
                  Hero(tag: 'recipe-image-${recipe.id}', child: RecipeImage.of(recipe)),
                  if (recipe.isFavorite)
                    Positioned(
                      top: 8,
                      right: 8,
                      child: CircleAvatar(
                        radius: 14,
                        backgroundColor: cs.surface.withValues(alpha: 0.9),
                        child: Icon(Icons.favorite_rounded, size: 16, color: cs.primary),
                      ),
                    ),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 10, 12, 12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    recipe.name,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.titleMedium?.copyWith(height: 1.2),
                  ),
                  const SizedBox(height: 6),
                  RecipeMetaRow(recipe: recipe),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class RecipeMetaRow extends StatelessWidget {
  const RecipeMetaRow({super.key, required this.recipe});
  final Recipe recipe;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final style = theme.textTheme.labelMedium?.copyWith(color: theme.colorScheme.onSurfaceVariant);
    final iconColor = theme.colorScheme.onSurfaceVariant;
    final time = recipe.effectiveTotalMinutes;
    return Wrap(
      spacing: 10,
      runSpacing: 4,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        if (recipe.isTrial) const TrialBadge(dense: true),
        if (time != null)
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.schedule_rounded, size: 14, color: iconColor),
              const SizedBox(width: 3),
              Text(formatMinutes(time), style: style),
            ],
          ),
        if (recipe.rating != null && recipe.rating! > 0)
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.star_rounded, size: 14, color: theme.colorScheme.primary),
              const SizedBox(width: 2),
              Text('${recipe.rating}', style: style),
            ],
          ),
        if (recipe.categories.isNotEmpty) Text(recipe.categories.first, style: style),
      ],
    );
  }
}

class RecipeListTile extends StatelessWidget {
  const RecipeListTile({super.key, required this.recipe, required this.onTap});
  final Recipe recipe;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: InkWell(
        onTap: onTap,
        child: Row(
          children: [
            SizedBox(
              width: 96,
              height: 80,
              child: Hero(tag: 'recipe-image-${recipe.id}', child: RecipeImage.of(recipe, iconSize: 26)),
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
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                    const SizedBox(height: 4),
                    RecipeMetaRow(recipe: recipe),
                  ],
                ),
              ),
            ),
            if (recipe.isFavorite)
              Padding(
                padding: const EdgeInsets.only(right: 14),
                child: Icon(Icons.favorite_rounded, size: 18, color: Theme.of(context).colorScheme.primary),
              ),
          ],
        ),
      ),
    );
  }
}
