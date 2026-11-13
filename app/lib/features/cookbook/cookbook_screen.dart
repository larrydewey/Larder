import 'package:flutter/foundation.dart' show listEquals;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/backend.dart';
import '../../data/preferences.dart';
import '../../data/providers.dart';
import '../../models/cookbook.dart';
import '../../models/recipe.dart';
import '../../util/platform/platform.dart';
import '../../widgets/recipe_image.dart';
import '../../widgets/snack.dart';
import '../../widgets/states.dart';
import '../recipes/recipe_actions.dart';
import '../recipes/recipe_query.dart';
import 'cookbook_chapters.dart';

final cookbookThemesProvider = FutureProvider.autoDispose<List<CookbookTheme>>(
  (ref) => ref.watch(cookbookServiceProvider).themes(),
);

/// Saves the backup zip (server export or on-device engine).
Future<void> exportJsonBackup(WidgetRef ref) async {
  showSnack('Preparing backup…');
  await runOnline(() async {
    final bytes = await ref.read(cookbookServiceProvider).exportBackup();
    final stamp = DateTime.now().toIso8601String().substring(0, 10);
    await saveAndOpenFile(bytes, 'larder-backup-$stamp.zip', 'application/zip');
    showSnack('Backup ready');
  });
}

class CookbookScreen extends ConsumerStatefulWidget {
  const CookbookScreen({super.key});

  @override
  ConsumerState<CookbookScreen> createState() => _CookbookScreenState();
}

class _CookbookScreenState extends ConsumerState<CookbookScreen> {
  final _o = CookbookOptions();
  late final _title = TextEditingController(text: _o.title);
  final _subtitle = TextEditingController();
  final _author = TextEditingController();
  final _dedication = TextEditingController();
  bool _allRecipes = true;
  Set<String> _selected = {};
  bool _customOrder = false;
  List<String> _chapters = [];
  List<String> _chaptersFor = const [];
  String? _chaptersGroupBy;
  bool _reordered = false;
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    _o.units = ref.read(unitsPreferenceProvider).name;
  }

  @override
  void dispose() {
    for (final c in [_title, _subtitle, _author, _dedication]) {
      c.dispose();
    }
    super.dispose();
  }

  /// Exactly the chapters the server will produce for the current selection
  /// and grouping (see [cookbookChapters]).
  List<String> _availableChapters(List<Recipe> recipes) {
    final pool = _allRecipes ? recipes : recipes.where((r) => _selected.contains(r.id));
    return cookbookChapters(pool, _o.groupBy);
  }

  Future<void> _generate() async {
    _o
      ..title = _title.text
      ..subtitle = _subtitle.text
      ..author = _author.text
      ..dedication = _dedication.text
      ..recipeIds = _allRecipes ? null : _selected.toList()
      ..chapterOrder = (_customOrder && _o.groupBy != 'none') ? _chapters : null;
    if (!_allRecipes && _selected.isEmpty) {
      showSnack('Pick at least one recipe');
      return;
    }
    setState(() => _busy = true);
    await runOnline(() async {
      final bytes = await ref.read(cookbookServiceProvider).renderCookbook(_o);
      final name = '${_o.title.trim().isEmpty ? 'cookbook' : _o.title.trim()}.pdf'
          .replaceAll(RegExp(r'[^\w\s.-]'), '')
          .replaceAll(RegExp(r'\s+'), '-');
      await saveAndOpenFile(bytes, name, 'application/pdf');
      showSnack('Cookbook ready');
    });
    if (mounted) setState(() => _busy = false);
  }

  Future<void> _pickRecipes(List<Recipe> recipes) async {
    final result = await showDialog<Set<String>>(
      context: context,
      builder: (_) => _RecipePickerDialog(recipes: recipes, initial: _selected),
    );
    if (result != null) setState(() => _selected = result);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    // Trial recipes are not vetted yet and never go into the printed book.
    final recipes = [
      for (final r in ref.watch(recipesProvider).value ?? const <Recipe>[])
        if (!r.isTrial) r,
    ];
    final themes = ref.watch(cookbookThemesProvider);
    // Recomputed on every build, so it follows the selection, the grouping
    // and edits to the recipes. Until the user drags a chapter the list is the
    // server's default order; after that their order is kept for chapters that
    // still exist and new ones are appended.
    final available = _availableChapters(recipes);
    if (_o.groupBy != _chaptersGroupBy) {
      _chaptersGroupBy = _o.groupBy;
      _reordered = false;
    }
    if (!listEquals(available, _chaptersFor)) {
      _chaptersFor = available;
      if (_reordered) {
        final kept = _chapters.where(available.contains).toList();
        _chapters = [...kept, ...available.where((c) => !kept.contains(c))];
      } else {
        _chapters = [...available];
      }
    }

    Widget toggle(String title, bool value, ValueChanged<bool> onChanged, {String? subtitle}) => SwitchListTile(
      contentPadding: EdgeInsets.zero,
      title: Text(title),
      subtitle: subtitle == null ? null : Text(subtitle),
      value: value,
      onChanged: (v) => setState(() => onChanged(v)),
    );

    return Scaffold(
      appBar: AppBar(
        leading: BackButton(onPressed: () => context.canPop() ? context.pop() : context.go('/recipes')),
        title: const Text('Export cookbook'),
      ),
      bottomNavigationBar: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 12),
          child: FilledButton.icon(
            style: FilledButton.styleFrom(minimumSize: const Size.fromHeight(52)),
            onPressed: _busy ? null : _generate,
            icon: _busy
                ? const SizedBox(width: 18, height: 18, child: CircularProgressIndicator(strokeWidth: 2))
                : const Icon(Icons.picture_as_pdf_rounded),
            label: Text(_busy ? 'Generating PDF…' : 'Create PDF cookbook'),
          ),
        ),
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
                  'Turn your recipes into a printable book, with chapters, a table of contents and an index.',
                  style: theme.textTheme.bodyMedium?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                ),
                const SectionHeader('Cover'),
                TextField(
                  controller: _title,
                  decoration: const InputDecoration(labelText: 'Title'),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: _subtitle,
                  decoration: const InputDecoration(labelText: 'Subtitle (optional)'),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: _author,
                  decoration: const InputDecoration(labelText: 'Author (optional)'),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: _dedication,
                  maxLines: 3,
                  decoration: const InputDecoration(labelText: 'Dedication (optional)'),
                ),
                const SectionHeader('Style'),
                themes.when(
                  loading: () => const LinearProgressIndicator(),
                  error: (e, _) => Text('Could not load themes: $e'),
                  data: (list) => RadioGroup<String>(
                    groupValue: _o.theme,
                    onChanged: (v) => setState(() => _o.theme = v ?? _o.theme),
                    child: Column(
                      children: [
                        for (final t in list)
                          Padding(
                            padding: const EdgeInsets.only(bottom: 8),
                            child: Card(
                              color: _o.theme == t.id ? theme.colorScheme.primaryContainer : null,
                              child: RadioListTile<String>(
                                value: t.id,
                                title: Text(t.name),
                                subtitle: t.description.isEmpty ? null : Text(t.description),
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(height: 12),
                Text('Paper size', style: theme.textTheme.labelLarge),
                const SizedBox(height: 6),
                SegmentedButton<String>(
                  segments: const [
                    ButtonSegment(value: 'letter', label: Text('Letter')),
                    ButtonSegment(value: 'a4', label: Text('A4')),
                    ButtonSegment(value: 'a5', label: Text('A5')),
                  ],
                  selected: {_o.paper},
                  onSelectionChanged: (s) => setState(() => _o.paper = s.first),
                ),
                const SizedBox(height: 12),
                Text('Units', style: theme.textTheme.labelLarge),
                const SizedBox(height: 6),
                SegmentedButton<String>(
                  segments: const [
                    ButtonSegment(value: 'original', label: Text('Original')),
                    ButtonSegment(value: 'metric', label: Text('Metric')),
                    ButtonSegment(value: 'imperial', label: Text('Imperial')),
                  ],
                  selected: {_o.units},
                  onSelectionChanged: (s) => setState(() => _o.units = s.first),
                ),
                const SectionHeader('Recipes'),
                SegmentedButton<bool>(
                  segments: [
                    ButtonSegment(value: true, label: Text('All (${recipes.length})')),
                    ButtonSegment(value: false, label: Text('Selected (${_selected.length})')),
                  ],
                  selected: {_allRecipes},
                  onSelectionChanged: (s) => setState(() => _allRecipes = s.first),
                ),
                if (!_allRecipes) ...[
                  const SizedBox(height: 8),
                  OutlinedButton.icon(
                    onPressed: () => _pickRecipes(recipes),
                    icon: const Icon(Icons.checklist_rounded),
                    label: Text(_selected.isEmpty ? 'Choose recipes' : 'Change selection (${_selected.length})'),
                  ),
                ],
                const SectionHeader('Chapters'),
                SegmentedButton<String>(
                  segments: const [
                    ButtonSegment(value: 'category', label: Text('By category')),
                    ButtonSegment(value: 'tag', label: Text('By tag')),
                    ButtonSegment(value: 'none', label: Text('No chapters')),
                  ],
                  selected: {_o.groupBy},
                  onSelectionChanged: (s) => setState(() => _o.groupBy = s.first),
                ),
                if (_o.groupBy != 'none') ...[
                  toggle(
                    'Custom chapter order',
                    _customOrder,
                    (v) => _customOrder = v,
                    subtitle: _customOrder ? 'Drag to reorder' : 'Alphabetical',
                  ),
                  if (_customOrder)
                    Card(
                      child: ReorderableListView(
                        shrinkWrap: true,
                        physics: const NeverScrollableScrollPhysics(),
                        buildDefaultDragHandles: false,
                        onReorderItem: (a, b) => setState(() {
                          _chapters.insert(b, _chapters.removeAt(a));
                          _reordered = true;
                        }),
                        children: [
                          for (var i = 0; i < _chapters.length; i++)
                            ListTile(
                              key: ValueKey(_chapters[i]),
                              leading: CircleAvatar(
                                radius: 14,
                                child: Text('${i + 1}', style: theme.textTheme.labelSmall),
                              ),
                              title: Text(_chapters[i]),
                              trailing: ReorderableDragStartListener(
                                index: i,
                                child: const Icon(Icons.drag_handle_rounded),
                              ),
                            ),
                        ],
                      ),
                    ),
                ],
                const SectionHeader('Include'),
                toggle('Photos', _o.includeImages, (v) => _o.includeImages = v),
                toggle('Table of contents', _o.includeToc, (v) => _o.includeToc = v),
                toggle('Index', _o.includeIndex, (v) => _o.includeIndex = v),
                toggle('Notes', _o.includeNotes, (v) => _o.includeNotes = v),
                toggle('Nutrition', _o.includeNutrition, (v) => _o.includeNutrition = v),
                const Divider(height: 40),
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  leading: const Icon(Icons.archive_outlined),
                  title: const Text('Export all as JSON backup'),
                  subtitle: const Text(
                    'A zip with every recipe and photo; re-importable into Larder (source "Larder backup").',
                  ),
                  trailing: const Icon(Icons.download_rounded),
                  onTap: () => exportJsonBackup(ref),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _RecipePickerDialog extends StatefulWidget {
  const _RecipePickerDialog({required this.recipes, required this.initial});
  final List<Recipe> recipes;
  final Set<String> initial;

  @override
  State<_RecipePickerDialog> createState() => _RecipePickerDialogState();
}

class _RecipePickerDialogState extends State<_RecipePickerDialog> {
  late final Set<String> _sel = {...widget.initial};
  String _q = '';

  @override
  Widget build(BuildContext context) {
    final shown = applyRecipeQuery(widget.recipes, RecipeQuery(text: _q));
    return AlertDialog(
      title: Text('Choose recipes (${_sel.length})'),
      contentPadding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
      content: SizedBox(
        width: 480,
        height: 520,
        child: Column(
          children: [
            TextField(
              decoration: const InputDecoration(prefixIcon: Icon(Icons.search_rounded), hintText: 'Search'),
              onChanged: (v) => setState(() => _q = v),
            ),
            Row(
              children: [
                TextButton(
                  onPressed: () => setState(() => _sel.addAll(shown.map((r) => r.id))),
                  child: const Text('Select shown'),
                ),
                TextButton(onPressed: () => setState(_sel.clear), child: const Text('Clear')),
              ],
            ),
            Expanded(
              child: ListView.builder(
                itemCount: shown.length,
                itemBuilder: (_, i) {
                  final r = shown[i];
                  return CheckboxListTile(
                    value: _sel.contains(r.id),
                    onChanged: (v) => setState(() => v == true ? _sel.add(r.id) : _sel.remove(r.id)),
                    secondary: ClipRRect(
                      borderRadius: BorderRadius.circular(8),
                      child: SizedBox(width: 40, height: 40, child: RecipeImage.of(r, iconSize: 16)),
                    ),
                    title: Text(r.name),
                    subtitle: r.categories.isEmpty ? null : Text(r.categories.join(', ')),
                  );
                },
              ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancel')),
        FilledButton(onPressed: () => Navigator.pop(context, _sel), child: const Text('Done')),
      ],
    );
  }
}
