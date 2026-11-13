import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/app_mode.dart';
import '../../data/providers.dart';
import '../../data/sync_controller.dart';
import '../../models/recipe.dart';
import '../../widgets/states.dart';
import '../trial/trial_actions.dart';
import 'recipe_card.dart';
import 'recipe_query.dart';

final _gridViewProvider = NotifierProvider<_GridView, bool>(_GridView.new);

class _GridView extends Notifier<bool> {
  @override
  bool build() => true;
  void toggle() => state = !state;
}

class RecipeListScreen extends ConsumerStatefulWidget {
  const RecipeListScreen({super.key});

  @override
  ConsumerState<RecipeListScreen> createState() => _RecipeListScreenState();
}

class _RecipeListScreenState extends ConsumerState<RecipeListScreen> {
  static const _pageSize = 48;
  late final TextEditingController _search = TextEditingController(text: ref.read(recipeQueryProvider).text);
  final _scroll = ScrollController();
  Timer? _debounce;
  int _limit = _pageSize;
  int _resultCount = 0;

  @override
  void initState() {
    super.initState();
    _scroll.addListener(() {
      if (_scroll.position.extentAfter < 600 && _limit < _resultCount) setState(() => _limit += _pageSize);
    });
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _search.dispose();
    _scroll.dispose();
    super.dispose();
  }

  void _onSearch(String text) {
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 300), () {
      _limit = _pageSize;
      ref.read(recipeQueryProvider.notifier).update((q) => q.copyWith(text: text));
    });
  }

  void _update(RecipeQuery Function(RecipeQuery) f) {
    _limit = _pageSize;
    ref.read(recipeQueryProvider.notifier).update(f);
  }

  void _open(Recipe r) => context.go('/recipes/${r.id}');

  @override
  Widget build(BuildContext context) {
    final recipesAsync = ref.watch(recipesProvider);
    final query = ref.watch(recipeQueryProvider);
    final grid = ref.watch(_gridViewProvider);
    final sync = ref.watch(syncProvider);
    final discarding = ref.watch(pendingDiscardsProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Recipes'),
        actions: [
          IconButton(
            tooltip: 'Import recipes',
            icon: const Icon(Icons.download_outlined),
            onPressed: () => context.go('/recipes/import'),
          ),
          PopupMenuButton<RecipeSort>(
            tooltip: 'Sort',
            icon: const Icon(Icons.sort_rounded),
            initialValue: query.sort,
            onSelected: (s) => _update((q) => q.copyWith(sort: s)),
            itemBuilder: (_) => [
              for (final s in RecipeSort.values)
                CheckedPopupMenuItem(value: s, checked: s == query.sort, child: Text(s.label)),
            ],
          ),
          IconButton(
            tooltip: grid ? 'List view' : 'Grid view',
            icon: Icon(grid ? Icons.view_list_rounded : Icons.grid_view_rounded),
            onPressed: () => ref.read(_gridViewProvider.notifier).toggle(),
          ),
          PopupMenuButton<String>(
            onSelected: (v) {
              switch (v) {
                case 'cookbook':
                  context.push('/cookbook');
                case 'organize':
                  context.push('/organize');
                case 'sync':
                  ref.read(syncProvider.notifier).sync();
              }
            },
            itemBuilder: (_) => [
              const PopupMenuItem(
                value: 'cookbook',
                child: ListTile(leading: Icon(Icons.picture_as_pdf_outlined), title: Text('Export cookbook')),
              ),
              const PopupMenuItem(
                value: 'organize',
                child: ListTile(leading: Icon(Icons.auto_fix_high_rounded), title: Text('Organize recipes')),
              ),
              if (ref.read(appModeProvider) == AppMode.connected)
                const PopupMenuItem(
                  value: 'sync',
                  child: ListTile(leading: Icon(Icons.sync_rounded), title: Text('Sync now')),
                ),
            ],
          ),
        ],
        bottom: PreferredSize(
          preferredSize: const Size.fromHeight(112),
          child: _SearchAndFilters(controller: _search, onSearch: _onSearch, query: query, onUpdate: _update),
        ),
      ),
      floatingActionButton: FloatingActionButton.extended(
        heroTag: 'new-recipe',
        onPressed: () => context.push('/recipes/new'),
        icon: const Icon(Icons.add_rounded),
        label: const Text('New recipe'),
      ),
      body: recipesAsync.when(
        loading: () => _Skeleton(grid: grid),
        error: (e, _) => ErrorState(error: e, onRetry: () => ref.invalidate(recipesProvider)),
        data: (all) {
          // Trial recipes are listed under Trial (a search still finds them).
          final trials = all.where((r) => r.isTrial).length;
          if (all.length == trials && !query.isFiltered) {
            if (all.isEmpty && sync.syncing && sync.lastSync == null) return _Skeleton(grid: grid);
            return EmptyState(
              icon: Icons.menu_book_rounded,
              title: 'Your cookbook is empty',
              message: trials == 0
                  ? 'Add your first recipe, or import a collection from another app or website.'
                  : 'Add your first recipe, or import a collection from another app or website. '
                        '$trials recipe${trials == 1 ? ' is' : 's are'} waiting on your trial list.',
              action: Wrap(
                spacing: 12,
                runSpacing: 8,
                alignment: WrapAlignment.center,
                children: [
                  FilledButton.icon(
                    onPressed: () => context.push('/recipes/new'),
                    icon: const Icon(Icons.add),
                    label: const Text('New recipe'),
                  ),
                  OutlinedButton.icon(
                    onPressed: () => context.go('/recipes/import'),
                    icon: const Icon(Icons.download),
                    label: const Text('Import'),
                  ),
                ],
              ),
            );
          }
          final results = applyRecipeQuery([
            for (final r in all)
              if (!discarding.contains(r.id)) r,
          ], query);
          if (results.isEmpty) {
            return EmptyState(
              icon: Icons.search_off_rounded,
              title: 'No matching recipes',
              message: 'Try a different search or clear the filters.',
              action: TextButton(
                onPressed: () {
                  _search.clear();
                  ref.read(recipeQueryProvider.notifier).clear();
                },
                child: const Text('Clear filters'),
              ),
            );
          }
          _resultCount = results.length;
          final shown = results.take(_limit).toList();
          return RefreshIndicator(
            onRefresh: () => ref.read(syncProvider.notifier).sync(),
            child: CustomScrollView(
              controller: _scroll,
              physics: const AlwaysScrollableScrollPhysics(),
              slivers: [
                SliverPadding(
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
                  sliver: SliverToBoxAdapter(
                    child: Text(
                      '${results.length} recipe${results.length == 1 ? '' : 's'}',
                      style: Theme.of(context).textTheme.labelLarge
                          ?.copyWith(color: Theme.of(context).colorScheme.onSurfaceVariant),
                    ),
                  ),
                ),
                SliverPadding(
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 96),
                  sliver: grid
                      ? SliverGrid.builder(
                          gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
                            maxCrossAxisExtent: 260,
                            mainAxisSpacing: 14,
                            crossAxisSpacing: 14,
                            childAspectRatio: 0.78,
                          ),
                          itemCount: shown.length,
                          itemBuilder: (_, i) => RecipeCard(recipe: shown[i], onTap: () => _open(shown[i])),
                        )
                      : SliverList.separated(
                          itemCount: shown.length,
                          separatorBuilder: (_, _) => const SizedBox(height: 10),
                          itemBuilder: (_, i) => RecipeListTile(recipe: shown[i], onTap: () => _open(shown[i])),
                        ),
                ),
                if (shown.length < results.length)
                  const SliverToBoxAdapter(
                    child: Padding(
                      padding: EdgeInsets.only(bottom: 96),
                      child: Center(child: CircularProgressIndicator()),
                    ),
                  ),
              ],
            ),
          );
        },
      ),
    );
  }
}

class _SearchAndFilters extends ConsumerWidget {
  const _SearchAndFilters({
    required this.controller,
    required this.onSearch,
    required this.query,
    required this.onUpdate,
  });
  final TextEditingController controller;
  final ValueChanged<String> onSearch;
  final RecipeQuery query;
  final void Function(RecipeQuery Function(RecipeQuery)) onUpdate;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final categories = ref.watch(localCategoriesProvider);
    final quickCats = categories.take(8).where((c) => !query.categories.contains(c)).toList();
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
          child: SearchBar(
            controller: controller,
            hintText: 'Search recipes, ingredients, tags…',
            leading: const Icon(Icons.search_rounded),
            elevation: const WidgetStatePropertyAll(0),
            onChanged: onSearch,
            trailing: [
              ListenableBuilder(
                listenable: controller,
                builder: (_, _) => controller.text.isEmpty
                    ? const SizedBox.shrink()
                    : IconButton(
                        tooltip: 'Clear search',
                        icon: const Icon(Icons.close_rounded),
                        onPressed: () {
                          controller.clear();
                          onSearch('');
                        },
                      ),
              ),
            ],
          ),
        ),
        SizedBox(
          height: 48,
          child: ListView(
            scrollDirection: Axis.horizontal,
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
            children: [
              FilterChip(
                avatar: Icon(query.favoritesOnly ? Icons.favorite_rounded : Icons.favorite_border_rounded, size: 18),
                showCheckmark: false,
                label: const Text('Favorites'),
                selected: query.favoritesOnly,
                onSelected: (v) => onUpdate((q) => q.copyWith(favoritesOnly: v)),
              ),
              const SizedBox(width: 8),
              ActionChip(
                avatar: const Icon(Icons.tune_rounded, size: 18),
                label: Text(query.tags.isEmpty ? 'Tags' : 'Tags (${query.tags.length})'),
                onPressed: () => _pickTags(context, ref),
              ),
              for (final c in query.categories) ...[
                const SizedBox(width: 8),
                InputChip(
                  label: Text(c),
                  selected: true,
                  onDeleted: () => onUpdate((q) => q.copyWith(categories: {...q.categories}..remove(c))),
                ),
              ],
              for (final t in query.tags) ...[
                const SizedBox(width: 8),
                InputChip(
                  avatar: const Icon(Icons.tag_rounded, size: 16),
                  label: Text(t),
                  selected: true,
                  onDeleted: () => onUpdate((q) => q.copyWith(tags: {...q.tags}..remove(t))),
                ),
              ],
              for (final c in quickCats) ...[
                const SizedBox(width: 8),
                FilterChip(
                  label: Text(c),
                  selected: false,
                  onSelected: (_) => onUpdate((q) => q.copyWith(categories: {...q.categories, c})),
                ),
              ],
            ],
          ),
        ),
      ],
    );
  }

  Future<void> _pickTags(BuildContext context, WidgetRef ref) async {
    final tags = ref.read(localTagsProvider);
    final categories = ref.read(localCategoriesProvider);
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (context) => Consumer(
        builder: (context, ref, _) {
          final q = ref.watch(recipeQueryProvider);
          Widget section(
            String title,
            List<String> names,
            Set<String> selected,
            RecipeQuery Function(String, bool) apply,
          ) => Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SectionHeader(title, padding: const EdgeInsets.fromLTRB(0, 8, 0, 8)),
              if (names.isEmpty) const Text('None yet'),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  for (final n in names)
                    FilterChip(
                      label: Text(n),
                      selected: selected.contains(n),
                      onSelected: (v) => onUpdate((_) => apply(n, v)),
                    ),
                ],
              ),
            ],
          );
          return DraggableScrollableSheet(
            expand: false,
            initialChildSize: 0.6,
            maxChildSize: 0.92,
            builder: (context, scroll) => ListView(
              controller: scroll,
              padding: const EdgeInsets.fromLTRB(20, 0, 20, 32),
              children: [
                Row(
                  children: [
                    Expanded(child: Text('Filter', style: Theme.of(context).textTheme.headlineSmall)),
                    TextButton(
                      onPressed: () => onUpdate((q) => q.copyWith(tags: {}, categories: {})),
                      child: const Text('Clear'),
                    ),
                  ],
                ),
                section(
                  'Categories',
                  categories,
                  q.categories,
                  (n, v) => q.copyWith(categories: v ? {...q.categories, n} : ({...q.categories}..remove(n))),
                ),
                const SizedBox(height: 8),
                section(
                  'Tags',
                  tags,
                  q.tags,
                  (n, v) => q.copyWith(tags: v ? {...q.tags, n} : ({...q.tags}..remove(n))),
                ),
              ],
            ),
          );
        },
      ),
    );
  }
}

class _Skeleton extends StatelessWidget {
  const _Skeleton({required this.grid});
  final bool grid;

  @override
  Widget build(BuildContext context) {
    if (!grid) {
      return ListView.separated(
        padding: const EdgeInsets.all(16),
        itemCount: 8,
        separatorBuilder: (_, _) => const SizedBox(height: 10),
        itemBuilder: (_, _) => const SkeletonBox(height: 80),
      );
    }
    return GridView.builder(
      padding: const EdgeInsets.all(16),
      gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
        maxCrossAxisExtent: 260,
        mainAxisSpacing: 14,
        crossAxisSpacing: 14,
        childAspectRatio: 0.78,
      ),
      itemCount: 12,
      itemBuilder: (_, _) => const SkeletonBox(radius: 16),
    );
  }
}
