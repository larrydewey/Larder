import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/providers.dart';
import '../../data/backend.dart';
import '../../data/sync_controller.dart';
import '../../models/shopping.dart';
import '../../widgets/snack.dart';
import '../../widgets/states.dart';
import '../recipes/recipe_actions.dart';

class ShoppingListsScreen extends ConsumerWidget {
  const ShoppingListsScreen({super.key});

  Future<void> _create(BuildContext context, WidgetRef ref) async {
    final name = await promptText(
      context,
      title: 'New shopping list',
      label: 'Name',
      initial: 'Groceries',
      confirm: 'Create',
    );
    if (name == null || name.trim().isEmpty) return;
    final list = await ref.read(shoppingRepositoryProvider).createList(name.trim());
    if (context.mounted) context.go('/shopping/${list.id}');
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(shoppingListsProvider);
    return Scaffold(
      appBar: AppBar(title: const Text('Shopping')),
      floatingActionButton: FloatingActionButton.extended(
        heroTag: 'new-list',
        onPressed: () => _create(context, ref),
        icon: const Icon(Icons.add_rounded),
        label: const Text('New list'),
      ),
      body: async.when(
        loading: () => const LoadingView(),
        error: (e, _) => ErrorState(error: e),
        data: (lists) {
          if (lists.isEmpty) {
            return EmptyState(
              icon: Icons.shopping_basket_rounded,
              title: 'No shopping lists yet',
              message: 'Create a list, then add items or whole recipes to it.',
              action: FilledButton.icon(
                onPressed: () => _create(context, ref),
                icon: const Icon(Icons.add),
                label: const Text('New list'),
              ),
            );
          }
          final sorted = [...lists]..sort((a, b) => (b.updatedAt ?? DateTime(0)).compareTo(a.updatedAt ?? DateTime(0)));
          return RefreshIndicator(
            onRefresh: () => ref.read(syncProvider.notifier).sync(),
            child: ListView.separated(
              physics: const AlwaysScrollableScrollPhysics(),
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 96),
              itemCount: sorted.length,
              separatorBuilder: (_, _) => const SizedBox(height: 10),
              itemBuilder: (_, i) => MaxWidth(child: _ListCard(list: sorted[i])),
            ),
          );
        },
      ),
    );
  }
}

class _ListCard extends ConsumerWidget {
  const _ListCard({required this.list});
  final ShoppingList list;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final total = list.items?.length ?? list.itemCount;
    final checked = list.items?.where((e) => e.checked).length ?? list.checkedCount;
    final remaining = total - checked;
    return Card(
      child: InkWell(
        onTap: () => context.go('/shopping/${list.id}'),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 14, 4, 14),
          child: Row(
            children: [
              CircleAvatar(
                backgroundColor: theme.colorScheme.primaryContainer,
                child: Icon(Icons.shopping_basket_outlined, color: theme.colorScheme.onPrimaryContainer),
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(list.name, style: theme.textTheme.titleMedium),
                    const SizedBox(height: 4),
                    Text(
                      total == 0 ? 'Empty' : (remaining == 0 ? 'All done!' : '$remaining of $total left'),
                      style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                    ),
                    if (total > 0) ...[
                      const SizedBox(height: 8),
                      ClipRRect(
                        borderRadius: BorderRadius.circular(4),
                        child: LinearProgressIndicator(value: checked / total, minHeight: 6),
                      ),
                    ],
                  ],
                ),
              ),
              PopupMenuButton<String>(
                onSelected: (v) async {
                  final repo = ref.read(shoppingRepositoryProvider);
                  if (v == 'rename') {
                    final name = await promptText(context, title: 'Rename list', initial: list.name, confirm: 'Rename');
                    if (name != null && name.trim().isNotEmpty) reportOutcome(await repo.renameList(list, name.trim()));
                  } else if (v == 'delete' && context.mounted) {
                    if (await confirm(context, title: 'Delete "${list.name}"?')) {
                      reportOutcome(await repo.deleteList(list));
                    }
                  }
                },
                itemBuilder: (_) => const [
                  PopupMenuItem(
                    value: 'rename',
                    child: ListTile(leading: Icon(Icons.edit_outlined), title: Text('Rename')),
                  ),
                  PopupMenuItem(
                    value: 'delete',
                    child: ListTile(leading: Icon(Icons.delete_outline), title: Text('Delete')),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
