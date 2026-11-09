import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/outbox_op.dart';
import '../../data/providers.dart';
import '../../data/backend.dart';
import '../../models/json.dart';
import '../../models/shopping.dart';
import '../../util/fractions.dart';
import '../../widgets/snack.dart';
import '../../widgets/states.dart';
import '../recipes/recipe_actions.dart';

class ShoppingListScreen extends ConsumerStatefulWidget {
  const ShoppingListScreen({super.key, required this.id});
  final String id;

  @override
  ConsumerState<ShoppingListScreen> createState() => _ShoppingListScreenState();
}

class _ShoppingListScreenState extends ConsumerState<ShoppingListScreen> {
  final _add = TextEditingController();
  final _addFocus = FocusNode();

  @override
  void initState() {
    super.initState();
    unawaited(ref.read(shoppingRepositoryProvider).refreshList(widget.id).catchError((_) {}));
  }

  @override
  void dispose() {
    _add.dispose();
    _addFocus.dispose();
    super.dispose();
  }

  Future<void> _addItem(ShoppingList list) async {
    final text = _add.text.trim();
    if (text.isEmpty) return;
    _add.clear();
    _addFocus.requestFocus();
    final o = await ref.read(shoppingRepositoryProvider).addItem(list, text);
    if (o.kind == OutcomeKind.rejected) reportOutcome(o);
  }

  @override
  Widget build(BuildContext context) {
    final async = ref.watch(shoppingListProvider(widget.id));
    return async.when(
      loading: () => const Scaffold(body: LoadingView()),
      error: (e, _) => Scaffold(
        appBar: AppBar(),
        body: ErrorState(error: e),
      ),
      data: (list) {
        if (list == null) {
          return Scaffold(
            appBar: AppBar(),
            body: EmptyState(
              icon: Icons.shopping_basket_outlined,
              title: 'List not found',
              message: 'It may have been deleted.',
              action: TextButton(onPressed: () => context.go('/shopping'), child: const Text('All lists')),
            ),
          );
        }
        return _build(context, list);
      },
    );
  }

  Widget _build(BuildContext context, ShoppingList list) {
    final theme = Theme.of(context);
    final items = [...?list.items];
    final hasChecked = items.any((e) => e.checked);
    final groups = <String, List<ShoppingItem>>{};
    for (final it in items) {
      groups.putIfAbsent(it.aisle ?? 'Other', () => []).add(it);
    }
    final aisleNames = groups.keys.toList()
      ..sort((a, b) {
        final c = aisleOrder(a).compareTo(aisleOrder(b));
        return c != 0 ? c : a.compareTo(b);
      });
    for (final g in groups.values) {
      g.sort((a, b) {
        if (a.checked != b.checked) return a.checked ? 1 : -1;
        return a.position.compareTo(b.position);
      });
    }
    final repo = ref.read(shoppingRepositoryProvider);

    return Scaffold(
      appBar: AppBar(
        title: Text(list.name),
        actions: [
          if (hasChecked)
            TextButton.icon(
              onPressed: () async => reportOutcome(await repo.clearChecked(list)),
              icon: const Icon(Icons.playlist_remove_rounded),
              label: const Text('Clear checked'),
            ),
          PopupMenuButton<String>(
            onSelected: (v) async {
              if (v == 'rename') {
                final name = await promptText(context, title: 'Rename list', initial: list.name, confirm: 'Rename');
                if (name != null && name.trim().isNotEmpty) reportOutcome(await repo.renameList(list, name.trim()));
              } else if (v == 'delete' && context.mounted) {
                if (await confirm(context, title: 'Delete "${list.name}"?')) {
                  unawaited(repo.deleteList(list).then(reportOutcome));
                  if (context.mounted) context.go('/shopping');
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
                child: ListTile(leading: Icon(Icons.delete_outline), title: Text('Delete list')),
              ),
            ],
          ),
        ],
      ),
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
            child: MaxWidth(
              child: TextField(
                key: const Key('quick-add'),
                controller: _add,
                focusNode: _addFocus,
                textInputAction: TextInputAction.done,
                decoration: InputDecoration(
                  hintText: 'Add an item, e.g. "2 lb chicken thighs"',
                  prefixIcon: const Icon(Icons.add_rounded),
                  suffixIcon: IconButton(
                    tooltip: 'Add',
                    icon: const Icon(Icons.send_rounded),
                    onPressed: () => _addItem(list),
                  ),
                ),
                onSubmitted: (_) => _addItem(list),
              ),
            ),
          ),
          Expanded(
            child: items.isEmpty
                ? const EmptyState(
                    icon: Icons.checklist_rounded,
                    title: 'Nothing on this list',
                    message: 'Type above to add items, or add a recipe from its page.',
                  )
                : RefreshIndicator(
                    onRefresh: () => repo.refreshList(list.id).catchError((_) {}),
                    child: ListView(
                      physics: const AlwaysScrollableScrollPhysics(),
                      padding: const EdgeInsets.fromLTRB(16, 0, 16, 48),
                      children: [
                        for (final aisle in aisleNames)
                          MaxWidth(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.stretch,
                              children: [
                                Padding(
                                  padding: const EdgeInsets.fromLTRB(4, 16, 4, 6),
                                  child: Text(
                                    aisle.toUpperCase(),
                                    style: theme.textTheme.labelLarge?.copyWith(
                                      color: theme.colorScheme.primary,
                                      letterSpacing: 1.2,
                                    ),
                                  ),
                                ),
                                Card(
                                  child: Column(children: [for (final it in groups[aisle]!) _ItemTile(item: it)]),
                                ),
                              ],
                            ),
                          ),
                      ],
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}

class _ItemTile extends ConsumerWidget {
  const _ItemTile({required this.item});
  final ShoppingItem item;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final repo = ref.read(shoppingRepositoryProvider);
    return Dismissible(
      key: ValueKey(item.id),
      direction: DismissDirection.endToStart,
      background: Container(
        color: theme.colorScheme.errorContainer,
        alignment: Alignment.centerRight,
        padding: const EdgeInsets.only(right: 20),
        child: Icon(Icons.delete_outline_rounded, color: theme.colorScheme.onErrorContainer),
      ),
      onDismissed: (_) => repo.deleteItem(item),
      child: CheckboxListTile(
        controlAffinity: ListTileControlAffinity.leading,
        value: item.checked,
        onChanged: (v) => repo.setChecked(item, v ?? false),
        title: Text(
          item.label,
          style: theme.textTheme.bodyLarge?.copyWith(
            decoration: item.checked ? TextDecoration.lineThrough : null,
            color: item.checked ? theme.colorScheme.onSurfaceVariant.withValues(alpha: 0.6) : null,
          ),
        ),
        subtitle: item.note == null || item.note!.isEmpty ? null : Text(item.note!),
        secondary: IconButton(
          tooltip: 'Edit item',
          icon: const Icon(Icons.edit_outlined, size: 20),
          onPressed: () => _edit(context, ref),
        ),
      ),
    );
  }

  Future<void> _edit(BuildContext context, WidgetRef ref) async {
    final result = await showDialog<Map<String, Object?>>(
      context: context,
      builder: (_) => _EditItemDialog(item: item),
    );
    if (result == null) return;
    if (result.containsKey('__delete')) {
      await ref.read(shoppingRepositoryProvider).deleteItem(item);
      return;
    }
    final q = result['quantity'] as double?;
    final unit = result['unit'] as String?;
    final food = result['food'] as String?;
    final updated = item.copyWith(
      food: food,
      quantity: () => q,
      unit: () => unit,
      note: () => result['note'] as String?,
      aisle: result['aisle'] as String?,
      display: [if (q != null) formatFraction(q), ?unit, ?food].join(' '),
    );
    reportOutcome(await ref.read(shoppingRepositoryProvider).updateItem(updated, {...result, 'quantity': jsonNum(q)}));
  }
}

class _EditItemDialog extends StatefulWidget {
  const _EditItemDialog({required this.item});
  final ShoppingItem item;

  @override
  State<_EditItemDialog> createState() => _EditItemDialogState();
}

class _EditItemDialogState extends State<_EditItemDialog> {
  late final _food = TextEditingController(text: widget.item.food ?? widget.item.display);
  late final _qty = TextEditingController(
    text: widget.item.quantity == null ? '' : formatDecimal(widget.item.quantity!),
  );
  late final _unit = TextEditingController(text: widget.item.unit ?? '');
  late final _note = TextEditingController(text: widget.item.note ?? '');
  late String _aisle = widget.item.aisle ?? 'Other';

  @override
  void dispose() {
    for (final c in [_food, _qty, _unit, _note]) {
      c.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    String? opt(TextEditingController c) => c.text.trim().isEmpty ? null : c.text.trim();
    final aisleOptions = {...aisles, _aisle}.toList();
    return AlertDialog(
      title: const Text('Edit item'),
      content: SizedBox(
        width: 380,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(
                controller: _food,
                decoration: const InputDecoration(labelText: 'Item'),
              ),
              const SizedBox(height: 12),
              Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: _qty,
                      keyboardType: const TextInputType.numberWithOptions(decimal: true),
                      decoration: const InputDecoration(labelText: 'Quantity'),
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: TextField(
                      controller: _unit,
                      decoration: const InputDecoration(labelText: 'Unit'),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              TextField(
                controller: _note,
                decoration: const InputDecoration(labelText: 'Note'),
              ),
              const SizedBox(height: 12),
              DropdownButtonFormField<String>(
                initialValue: _aisle,
                decoration: const InputDecoration(labelText: 'Aisle'),
                items: [for (final a in aisleOptions) DropdownMenuItem(value: a, child: Text(a))],
                onChanged: (v) => setState(() => _aisle = v ?? _aisle),
              ),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context, {'__delete': true}),
          child: Text('Delete', style: TextStyle(color: Theme.of(context).colorScheme.error)),
        ),
        TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancel')),
        FilledButton(
          onPressed: () => Navigator.pop(context, <String, Object?>{
            'food': opt(_food),
            'quantity': parseQuantity(_qty.text),
            'unit': opt(_unit),
            'note': opt(_note),
            'aisle': _aisle,
          }),
          child: const Text('Save'),
        ),
      ],
    );
  }
}
