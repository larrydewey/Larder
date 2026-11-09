import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/app_mode.dart';
import '../../data/backend.dart';
import '../../data/sync_controller.dart';
import '../../models/organize.dart';
import '../../widgets/snack.dart';
import '../../widgets/states.dart';
import '../recipes/recipe_actions.dart';

/// Tidies ingredient text and fills in missing categories for all recipes
/// (server: `POST /api/recipes/organize`; standalone: the on-device engine).
class OrganizeScreen extends ConsumerStatefulWidget {
  const OrganizeScreen({super.key});

  @override
  ConsumerState<OrganizeScreen> createState() => _OrganizeScreenState();
}

class _OrganizeScreenState extends ConsumerState<OrganizeScreen> {
  CategorizeMode _mode = CategorizeMode.uncategorized;
  bool _normalize = true;
  bool _busy = false;
  OrganizeResult? _preview;

  void _setOptions(VoidCallback change) => setState(() {
    change();
    _preview = null; // a preview only describes the options it was made with
  });

  Future<void> _run({required bool dryRun}) async {
    setState(() => _busy = true);
    final result = await runOnline(
      () => ref.read(recipeRepositoryProvider).organize(categorize: _mode, normalize: _normalize, dryRun: dryRun),
    );
    if (!mounted) return;
    setState(() {
      _busy = false;
      if (result != null) _preview = dryRun ? result : null;
    });
    if (result != null && !dryRun) showSnack(result.summary);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final connected = ref.watch(appModeProvider) == AppMode.connected;
    final offline = connected && !ref.watch(syncProvider).online;
    final nothingToDo = _mode == CategorizeMode.none && !_normalize;
    final enabled = !_busy && !offline && !nothingToDo;
    final preview = _preview;

    return Scaffold(
      appBar: AppBar(
        leading: BackButton(onPressed: () => context.canPop() ? context.pop() : context.go('/settings')),
        title: const Text('Organize recipes'),
      ),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 32),
        children: [
          MaxWidth(
            maxWidth: 760,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  'Tidies the ingredient text of your recipes, so every unit is spelled one way and the same food '
                  'always has the same name, and fills in missing categories.',
                  style: theme.textTheme.bodyMedium?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                ),
                const SectionHeader('Categories'),
                RadioGroup<CategorizeMode>(
                  groupValue: _mode,
                  onChanged: (v) => _setOptions(() => _mode = v ?? _mode),
                  child: Column(
                    children: [
                      for (final m in CategorizeMode.values)
                        RadioListTile<CategorizeMode>(
                          key: Key('organize-mode-${m.name}'),
                          contentPadding: EdgeInsets.zero,
                          value: m,
                          title: Text(m.label),
                        ),
                    ],
                  ),
                ),
                const SectionHeader('Ingredients'),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  value: _normalize,
                  onChanged: (v) => _setOptions(() => _normalize = v),
                  title: const Text('Tidy ingredient text'),
                  subtitle: const Text('e.g. "2 Tablespoons Butter, melted" becomes "2 tbsp butter, melted"'),
                ),
                if (offline)
                  Padding(
                    padding: const EdgeInsets.only(top: 8),
                    child: Card(
                      color: theme.colorScheme.errorContainer,
                      child: ListTile(
                        leading: Icon(Icons.cloud_off_rounded, color: theme.colorScheme.onErrorContainer),
                        title: Text(
                          'Organizing runs on the server and needs a connection. Try again when you are back online.',
                          style: TextStyle(color: theme.colorScheme.onErrorContainer),
                        ),
                      ),
                    ),
                  ),
                const SizedBox(height: 16),
                Wrap(
                  spacing: 12,
                  runSpacing: 8,
                  children: [
                    OutlinedButton.icon(
                      key: const Key('organize-preview'),
                      onPressed: enabled ? () => _run(dryRun: true) : null,
                      icon: const Icon(Icons.preview_outlined),
                      label: const Text('Preview'),
                    ),
                    FilledButton.icon(
                      key: const Key('organize-apply'),
                      onPressed: enabled ? () => _run(dryRun: false) : null,
                      icon: _busy
                          ? const SizedBox(width: 18, height: 18, child: CircularProgressIndicator(strokeWidth: 2))
                          : const Icon(Icons.auto_fix_high_rounded),
                      label: const Text('Apply'),
                    ),
                  ],
                ),
                if (preview != null) ...[
                  const SectionHeader('Preview'),
                  Text(preview.summary, key: const Key('organize-summary'), style: theme.textTheme.titleSmall),
                  const SizedBox(height: 8),
                  for (final group in preview.byCategory) ...[
                    Padding(
                      padding: const EdgeInsets.fromLTRB(4, 12, 4, 4),
                      child: Text(
                        group.key ?? 'No category',
                        style: theme.textTheme.titleSmall?.copyWith(color: theme.colorScheme.primary),
                      ),
                    ),
                    Card(
                      child: Column(children: [for (final c in group.value) _ChangeTile(change: c)]),
                    ),
                  ],
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _ChangeTile extends StatelessWidget {
  const _ChangeTile({required this.change});
  final OrganizeChange change;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final c = change;
    final category = c.categoryChanged
        ? '${c.categoryBefore ?? 'No category'} → ${c.categoryAfter ?? 'No category'}'
        : null;
    return ListTile(
      title: Text(c.name),
      subtitle: category == null ? null : Text(category),
      trailing: c.ingredientsChanged
          ? Chip(
              label: const Text('ingredients tidied'),
              labelStyle: theme.textTheme.labelSmall,
              visualDensity: VisualDensity.compact,
              padding: EdgeInsets.zero,
            )
          : null,
    );
  }
}
