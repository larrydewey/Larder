import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:image_picker/image_picker.dart';
import 'package:material_ui/material_ui.dart';

import '../../api/endpoints.dart';
import '../../data/outbox_op.dart';
import '../../data/providers.dart';
import '../../data/backend.dart';
import '../../models/json.dart';
import '../../models/recipe.dart';
import '../../util/categories.dart';
import '../../util/fractions.dart';
import '../../util/platform/platform.dart';
import '../../widgets/rating.dart';
import '../../widgets/recipe_image.dart';
import '../../widgets/snack.dart';
import '../../widgets/states.dart';
import '../recipes/recipe_actions.dart';
import 'chips_input.dart';
import 'editor_lines.dart';
import 'line_list_editor.dart';

/// Tag / category suggestions: cached recipes + server counts (best effort).
final _serverTagsProvider = FutureProvider<(List<String>, List<String>)>((ref) async {
  final api = ref.watch(apiClientProvider);
  try {
    final t = await api.tags();
    final c = await api.categories();
    return (t.map((e) => e.name).toList(), c.map((e) => e.name).toList());
  } catch (_) {
    return (const <String>[], const <String>[]);
  }
});

class RecipeEditorScreen extends ConsumerStatefulWidget {
  const RecipeEditorScreen({super.key, this.id, this.draft, this.draftImagePath, this.base = '/recipes'});

  /// Existing recipe to edit.
  final String? id;

  /// Prefilled new recipe (e.g. from an import preview).
  final Recipe? draft;

  /// On-device image that came with an import preview (standalone).
  final String? draftImagePath;

  /// The list the editor was opened from (`/recipes` or `/trial`).
  final String base;

  @override
  ConsumerState<RecipeEditorScreen> createState() => _RecipeEditorScreenState();
}

class _RecipeEditorScreenState extends ConsumerState<RecipeEditorScreen> {
  final _formKey = GlobalKey<FormState>();
  Recipe? _base;
  bool _isNew = true;
  bool _loading = true;
  bool _saving = false;
  bool _dirty = false;

  final _name = TextEditingController();
  final _description = TextEditingController();
  final _yieldQty = TextEditingController();
  final _yieldUnit = TextEditingController(text: 'servings');
  final _prep = TextEditingController();
  final _cook = TextEditingController();
  final _total = TextEditingController();
  final _sourceUrl = TextEditingController();
  final _sourceName = TextEditingController();
  final _notes = TextEditingController();
  int? _rating;
  String _status = RecipeStatus.cookbook;
  List<String> _tags = [];
  List<String> _categories = [];
  final _ingredients = LineListController();
  final _instructions = LineListController();
  final List<(TextEditingController, TextEditingController)> _nutrition = [];

  Uint8List? _newImage;
  String? _newImageName;
  bool _removeImage = false;

  List<TextEditingController> get _allFields => [
    _name,
    _description,
    _yieldQty,
    _yieldUnit,
    _prep,
    _cook,
    _total,
    _sourceUrl,
    _sourceName,
    _notes,
  ];

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    Recipe? r;
    if (widget.id != null) {
      r = await ref.read(localStoreProvider).recipe(widget.id!);
      if (r == null) {
        try {
          r = await ref.read(recipeRepositoryProvider).refresh(widget.id!);
        } catch (_) {}
      }
      _isNew = false;
    } else {
      r = widget.draft;
      _isNew = true;
    }
    if (!mounted) return;
    setState(() {
      _fill(r);
      _loading = false;
    });
  }

  void _fill(Recipe? r) {
    _base = r;
    _name.text = r?.name ?? '';
    _description.text = r?.description ?? '';
    _yieldQty.text = r?.yieldQuantity == null ? '' : formatDecimal(r!.yieldQuantity!);
    _yieldUnit.text = r?.yieldUnit ?? 'servings';
    _prep.text = r?.prepTimeMinutes?.toString() ?? '';
    _cook.text = r?.cookTimeMinutes?.toString() ?? '';
    _total.text = r?.totalTimeMinutes?.toString() ?? '';
    _sourceUrl.text = r?.sourceUrl ?? '';
    _sourceName.text = r?.sourceName ?? '';
    _notes.text = r?.notes ?? '';
    _rating = r?.rating;
    _status = r?.status ?? RecipeStatus.cookbook;
    _tags = [...?r?.tags];
    _categories = [...?r?.categories];
    _ingredients.setLines(ingredientsToLines(r?.ingredients ?? const []));
    _instructions.setLines(instructionsToLines(r?.instructions ?? const []));
    for (final (k, v) in _nutrition) {
      k.dispose();
      v.dispose();
    }
    _nutrition
      ..clear()
      ..addAll([
        for (final e in (r?.nutrition ?? const <String, String>{}).entries)
          (TextEditingController(text: e.key), TextEditingController(text: e.value)),
      ]);
    _newImage = null;
    _removeImage = false;
    _dirty = false;
  }

  @override
  void dispose() {
    for (final c in _allFields) {
      c.dispose();
    }
    _ingredients.dispose();
    _instructions.dispose();
    for (final (k, v) in _nutrition) {
      k.dispose();
      v.dispose();
    }
    super.dispose();
  }

  void _markDirty() {
    if (!_dirty) setState(() => _dirty = true);
  }

  int? _int(TextEditingController c) => int.tryParse(c.text.trim());
  String? _opt(TextEditingController c) => c.text.trim().isEmpty ? null : c.text.trim();

  Recipe _build() {
    final base = _base;
    final id = (base == null || base.id.isEmpty) ? newId() : base.id;
    final recipe = Recipe(
      id: id,
      householdId: base?.householdId ?? '',
      slug: base?.slug ?? '',
      name: _name.text.trim(),
      description: _description.text.trim(),
      yieldQuantity: parseQuantity(_yieldQty.text),
      yieldUnit: _yieldUnit.text.trim().isEmpty ? 'servings' : _yieldUnit.text.trim(),
      prepTimeMinutes: _int(_prep),
      cookTimeMinutes: _int(_cook),
      totalTimeMinutes: _int(_total),
      sourceUrl: _opt(_sourceUrl),
      sourceName: _opt(_sourceName),
      notes: _notes.text.trim(),
      rating: _rating,
      ingredients: linesToIngredients(_ingredients.lines, base?.ingredients ?? const []),
      instructions: linesToInstructions(_instructions.lines),
      tags: _tags,
      categories: _categories,
      nutrition: {
        for (final (k, v) in _nutrition)
          if (k.text.trim().isNotEmpty && v.text.trim().isNotEmpty) k.text.trim(): v.text.trim(),
      },
      imageUrl: base?.imageUrl,
      isFavorite: base?.isFavorite ?? false,
      createdBy: base?.createdBy,
      createdAt: base?.createdAt ?? DateTime.now().toUtc(),
      updatedAt: DateTime.now().toUtc(),
      version: base?.version,
      status: _status,
      // Only verdicts change these; the editor passes them through.
      triedCount: base?.triedCount ?? 0,
      lastTriedAt: base?.lastTriedAt,
    );
    return recipe;
  }

  Future<void> _save({bool force = false}) async {
    if (!_formKey.currentState!.validate()) return;
    setState(() => _saving = true);
    final recipe = _build();
    final repo = ref.read(recipeRepositoryProvider);
    try {
      final outcome = await repo.save(recipe, isNew: _isNew, force: force);
      if (!mounted) return;
      switch (outcome.kind) {
        case OutcomeKind.conflict:
          await _handleConflict(recipe, outcome);
          return;
        case OutcomeKind.rejected:
          showSnack(outcome.message ?? 'Could not save', error: true);
          if (_isNew) await ref.read(localStoreProvider).deleteRecipe(recipe.id);
          return;
        case OutcomeKind.queued:
          showSnack('Saved offline. It will sync when you are back online.');
          if (_newImage != null) showSnack('Saved offline. Add the photo again once you are online.');
        case OutcomeKind.applied:
          await _syncImage(recipe.id);
      }
      _dirty = false;
      if (mounted) context.go('${widget.base}/${recipe.id}');
    } catch (e) {
      showError(e);
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Future<void> _syncImage(String id) async {
    final repo = ref.read(recipeRepositoryProvider);
    if (_newImage != null) {
      await runOnline(() => repo.setImage(id, _newImage!, _newImageName ?? 'photo.jpg'));
    } else if (_isNew && !_removeImage && widget.draftImagePath != null) {
      await runOnline(() => repo.setImageFromPath(id, widget.draftImagePath!));
    } else if (_removeImage && !_isNew) {
      await runOnline(() => repo.removeImage(id));
    }
  }

  Future<void> _handleConflict(Recipe mine, OpOutcome outcome) async {
    final choice = await showDialog<String>(
      context: context,
      barrierDismissible: false,
      builder: (context) => AlertDialog(
        icon: const Icon(Icons.merge_type_rounded),
        title: const Text('Recipe changed elsewhere'),
        content: const Text(
          'Someone else saved this recipe while you were editing. You can reload their version (your edits are discarded) or overwrite it with yours.',
        ),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context, 'reload'), child: const Text('Reload')),
          FilledButton(onPressed: () => Navigator.pop(context, 'overwrite'), child: const Text('Overwrite with mine')),
        ],
      ),
    );
    if (!mounted) return;
    if (choice == 'overwrite') {
      _base = mine;
      await _save(force: true);
    } else {
      final current = outcome.data is Map
          ? Recipe.fromJson(asJson(outcome.data))
          : await ref.read(localStoreProvider).recipe(mine.id);
      setState(() {
        _fill(current);
        _saving = false;
      });
      showSnack('Loaded the latest version');
    }
  }

  Future<void> _pickImage(ImageSource source) async {
    try {
      final file = await ImagePicker().pickImage(source: source, maxWidth: 2400, maxHeight: 2400, imageQuality: 88);
      if (file == null) return;
      final bytes = await file.readAsBytes();
      setState(() {
        _newImage = bytes;
        _newImageName = file.name.isEmpty ? 'photo.jpg' : file.name;
        _removeImage = false;
        _dirty = true;
      });
    } catch (e) {
      showError(e);
    }
  }

  Future<bool> _confirmDiscard() async {
    if (!_dirty) return true;
    return confirm(context, title: 'Discard changes?', message: 'Your unsaved edits will be lost.', action: 'Discard');
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) return Scaffold(appBar: AppBar(), body: const LoadingView());
    final tagsLocal = ref.watch(localTagsProvider);
    final catsLocal = ref.watch(localCategoriesProvider);
    final server = ref.watch(_serverTagsProvider).value;
    final tagSuggestions = {...tagsLocal, ...?server?.$1}.toList();
    final catSuggestions = categorySuggestions([...catsLocal, ...?server?.$2]);
    final theme = Theme.of(context);

    return PopScope(
      canPop: !_dirty,
      onPopInvokedWithResult: (didPop, _) async {
        if (didPop) return;
        if (await _confirmDiscard() && context.mounted) {
          setState(() => _dirty = false);
          context.pop();
        }
      },
      child: Scaffold(
        appBar: AppBar(
          title: Text(_isNew ? 'New recipe' : 'Edit recipe'),
          leading: IconButton(
            tooltip: 'Close',
            icon: const Icon(Icons.close_rounded),
            onPressed: () async {
              if (await _confirmDiscard() && context.mounted) {
                _dirty = false;
                context.canPop() ? context.pop() : context.go(_isNew ? widget.base : '${widget.base}/${widget.id}');
              }
            },
          ),
          actions: [
            Padding(
              padding: const EdgeInsets.only(right: 12),
              child: FilledButton.icon(
                onPressed: _saving ? null : () => _save(),
                icon: _saving
                    ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                    : const Icon(Icons.check_rounded),
                label: const Text('Save'),
              ),
            ),
          ],
        ),
        body: Form(
          key: _formKey,
          onChanged: _markDirty,
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 120),
            children: [
              MaxWidth(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    _imageSection(theme),
                    const SizedBox(height: 16),
                    TextFormField(
                      controller: _name,
                      style: theme.textTheme.titleLarge,
                      decoration: const InputDecoration(labelText: 'Recipe name'),
                      validator: (v) => (v == null || v.trim().isEmpty) ? 'Give your recipe a name' : null,
                    ),
                    const SizedBox(height: 12),
                    TextFormField(
                      controller: _description,
                      minLines: 2,
                      maxLines: 5,
                      decoration: const InputDecoration(labelText: 'Description'),
                    ),
                    const SizedBox(height: 12),
                    Row(
                      children: [
                        const Text('Rating'),
                        const SizedBox(width: 8),
                        StarRating(
                          rating: _rating,
                          size: 26,
                          onChanged: (v) => setState(() {
                            _rating = v;
                            _dirty = true;
                          }),
                        ),
                      ],
                    ),
                    const SizedBox(height: 8),
                    Wrap(
                      spacing: 12,
                      runSpacing: 8,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: [
                        const Text('Save to'),
                        SegmentedButton<String>(
                          key: const Key('status-choice'),
                          showSelectedIcon: false,
                          segments: const [
                            ButtonSegment(
                              value: RecipeStatus.cookbook,
                              icon: Icon(Icons.menu_book_outlined),
                              label: Text('Cookbook'),
                            ),
                            ButtonSegment(
                              value: RecipeStatus.trial,
                              icon: Icon(Icons.science_outlined),
                              label: Text('Trial'),
                            ),
                          ],
                          selected: {_status},
                          onSelectionChanged: (s) => setState(() {
                            _status = s.first;
                            _dirty = true;
                          }),
                        ),
                        if (_status == RecipeStatus.trial)
                          Text(
                            'Kept apart until you decide it belongs in the cookbook.',
                            style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                          ),
                      ],
                    ),
                    const SectionHeader('Details'),
                    Wrap(
                      spacing: 12,
                      runSpacing: 12,
                      children: [
                        _small(_yieldQty, 'Yield', number: true, decimal: true),
                        _small(_yieldUnit, 'Yield unit', width: 160),
                        _small(_prep, 'Prep (min)', number: true),
                        _small(_cook, 'Cook (min)', number: true),
                        _small(_total, 'Total (min)', number: true),
                      ],
                    ),
                    const SizedBox(height: 12),
                    TextFormField(
                      controller: _sourceUrl,
                      keyboardType: TextInputType.url,
                      decoration: const InputDecoration(labelText: 'Source URL', prefixIcon: Icon(Icons.link_rounded)),
                    ),
                    const SizedBox(height: 12),
                    TextFormField(
                      controller: _sourceName,
                      decoration: const InputDecoration(
                        labelText: 'Source name',
                        hintText: 'e.g. Grandma',
                        prefixIcon: Icon(Icons.person_outline),
                      ),
                    ),
                    const SectionHeader('Categories & tags'),
                    ChipsInput(
                      label: 'Category',
                      icon: Icons.folder_outlined,
                      values: _categories,
                      suggestions: catSuggestions,
                      onChanged: (v) => setState(() {
                        _categories = v;
                        _dirty = true;
                      }),
                    ),
                    const SizedBox(height: 12),
                    ChipsInput(
                      label: 'Tag',
                      icon: Icons.tag_rounded,
                      values: _tags,
                      suggestions: tagSuggestions,
                      onChanged: (v) => setState(() {
                        _tags = v;
                        _dirty = true;
                      }),
                    ),
                    SectionHeader(
                      'Ingredients',
                      trailing: TextButton.icon(
                        onPressed: () => _bulkEdit(_ingredients, 'Ingredients', 'One ingredient per line'),
                        icon: const Icon(Icons.notes_rounded),
                        label: const Text('Bulk edit'),
                      ),
                    ),
                    Text(
                      'One ingredient per line, e.g. "1 ½ cups flour, sifted". Start a line with ## for a section heading.',
                      style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                    ),
                    const SizedBox(height: 8),
                    LineListEditor(
                      controller: _ingredients,
                      hint: 'Ingredient',
                      addLabel: 'Add ingredient',
                      onChanged: _markDirty,
                    ),
                    SectionHeader(
                      'Instructions',
                      trailing: TextButton.icon(
                        onPressed: () => _bulkEdit(_instructions, 'Instructions', 'One step per line'),
                        icon: const Icon(Icons.notes_rounded),
                        label: const Text('Bulk edit'),
                      ),
                    ),
                    LineListEditor(
                      controller: _instructions,
                      hint: 'Describe this step',
                      addLabel: 'Add step',
                      numbered: true,
                      multiline: true,
                      onChanged: _markDirty,
                    ),
                    const SectionHeader('Notes'),
                    TextFormField(
                      controller: _notes,
                      minLines: 3,
                      maxLines: 10,
                      decoration: const InputDecoration(hintText: 'Tips, substitutions, storage…'),
                    ),
                    SectionHeader(
                      'Nutrition',
                      trailing: TextButton.icon(
                        onPressed: () => setState(() {
                          _nutrition.add((TextEditingController(), TextEditingController()));
                          _dirty = true;
                        }),
                        icon: const Icon(Icons.add_rounded),
                        label: const Text('Add'),
                      ),
                    ),
                    if (_nutrition.isEmpty)
                      Text(
                        'e.g. calories → 250 kcal',
                        style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                      ),
                    for (var i = 0; i < _nutrition.length; i++)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 8),
                        child: Row(
                          children: [
                            Expanded(
                              child: TextField(
                                controller: _nutrition[i].$1,
                                decoration: const InputDecoration(hintText: 'calories'),
                              ),
                            ),
                            const SizedBox(width: 8),
                            Expanded(
                              child: TextField(
                                controller: _nutrition[i].$2,
                                decoration: const InputDecoration(hintText: '250 kcal'),
                              ),
                            ),
                            IconButton(
                              tooltip: 'Remove',
                              icon: const Icon(Icons.remove_circle_outline),
                              onPressed: () => setState(() {
                                final (k, v) = _nutrition.removeAt(i);
                                k.dispose();
                                v.dispose();
                                _dirty = true;
                              }),
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
      ),
    );
  }

  Widget _small(
    TextEditingController c,
    String label, {
    bool number = false,
    bool decimal = false,
    double width = 120,
  }) => SizedBox(
    width: width,
    child: TextFormField(
      controller: c,
      keyboardType: number ? TextInputType.numberWithOptions(decimal: decimal) : null,
      decoration: InputDecoration(labelText: label),
      validator: (v) {
        if (!number || v == null || v.trim().isEmpty) return null;
        final ok = decimal ? parseQuantity(v) != null : int.tryParse(v.trim()) != null;
        return ok ? null : 'Number';
      },
    ),
  );

  Widget _imageSection(ThemeData theme) {
    final draftImage = (_isNew && !_removeImage && widget.draftImagePath != null)
        ? localImageProvider(widget.draftImagePath!)
        : null;
    final hasExisting = ((_base?.hasImage ?? false) || draftImage != null) && !_removeImage;
    Widget preview;
    if (_newImage != null) {
      preview = Image.memory(_newImage!, fit: BoxFit.cover);
    } else if (draftImage != null) {
      preview = Image(image: draftImage, fit: BoxFit.cover);
    } else if (hasExisting) {
      preview = RecipeImage.of(_base!, size: 'medium');
    } else {
      preview = const ImagePlaceholder(iconSize: 48);
    }
    return ClipRRect(
      borderRadius: BorderRadius.circular(16),
      child: AspectRatio(
        aspectRatio: 16 / 9,
        child: Stack(
          fit: StackFit.expand,
          children: [
            preview,
            Positioned(
              right: 12,
              bottom: 12,
              child: Wrap(
                spacing: 8,
                children: [
                  FilledButton.tonalIcon(
                    onPressed: () => _pickImage(ImageSource.gallery),
                    icon: const Icon(Icons.photo_library_outlined),
                    label: Text(_newImage != null || hasExisting ? 'Change photo' : 'Add photo'),
                  ),
                  FilledButton.tonalIcon(
                    onPressed: () => _pickImage(ImageSource.camera),
                    icon: const Icon(Icons.photo_camera_outlined),
                    label: const Text('Camera'),
                  ),
                  if (_newImage != null || hasExisting)
                    IconButton.filledTonal(
                      tooltip: 'Remove photo',
                      onPressed: () => setState(() {
                        _newImage = null;
                        _removeImage = true;
                        _dirty = true;
                      }),
                      icon: const Icon(Icons.delete_outline_rounded),
                    ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _bulkEdit(LineListController c, String title, String hint) async {
    final text = await promptText(
      context,
      title: title,
      label: hint,
      initial: c.lines.where((l) => l.trim().isNotEmpty).join('\n'),
      maxLines: 14,
      keyboardType: TextInputType.multiline,
    );
    if (text == null) return;
    setState(() {
      c.setLines(text.split('\n').where((l) => l.trim().isNotEmpty).toList());
      _dirty = true;
    });
  }
}
