import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/preferences.dart';

/// The ways to add recipes, offered by the Add button on Recipes and Trial.
enum AddChoice {
  website('From a website', 'Paste a link to a recipe page', Icons.public_rounded),
  text('From text', 'Paste a recipe from anywhere', Icons.notes_rounded),
  photo('From a photo', 'Snap a cookbook page or a recipe card', Icons.photo_camera_outlined),
  write('Write it myself', 'Start from a blank recipe', Icons.edit_outlined),
  collection('Import a collection', 'Mealie, Paprika, Tandoor and other exports', Icons.upload_file_rounded);

  const AddChoice(this.label, this.description, this.icon);
  final String label;
  final String description;
  final IconData icon;

  /// The import screen tab this choice opens (null for "Write it myself").
  String? get importTab => switch (this) {
    AddChoice.website => 'website',
    AddChoice.text => 'text',
    AddChoice.photo => 'photo',
    AddChoice.collection => 'file',
    AddChoice.write => null,
  };
}

/// The choices on offer: "From a photo" only when AI assist is available.
List<AddChoice> addChoices({required bool llm}) => [
  for (final c in AddChoice.values)
    if (c != AddChoice.photo || llm) c,
];

/// Where [choice] leads from the list at [base] (`/recipes` or `/trial`).
/// Recipes written from Trial start as trial recipes (see the router).
String addLocation(AddChoice choice, String base) {
  final tab = choice.importTab;
  return tab == null ? '$base/new' : '$base/import?tab=$tab';
}

/// Shows the Add sheet and returns the choice, or null when dismissed.
Future<AddChoice?> showAddSheet(BuildContext context, {required bool llm, required bool trial}) =>
    showModalBottomSheet<AddChoice>(
      context: context,
      showDragHandle: true,
      isScrollControlled: true,
      builder: (context) => _AddSheet(
        choices: addChoices(llm: llm),
        trial: trial,
      ),
    );

/// Opens the Add sheet for the list at [base] and goes where the choice leads.
Future<void> openAdd(BuildContext context, WidgetRef ref, {required String base}) async {
  final choice = await showAddSheet(context, llm: ref.read(llmEnabledProvider), trial: base == '/trial');
  if (choice == null || !context.mounted) return;
  final location = addLocation(choice, base);
  if (choice == AddChoice.write) {
    context.push(location);
  } else {
    context.go(location);
  }
}

class _AddSheet extends StatelessWidget {
  const _AddSheet({required this.choices, required this.trial});
  final List<AddChoice> choices;
  final bool trial;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    Widget tile(AddChoice c) => ListTile(
      key: Key('add-${c.name}'),
      leading: Icon(c.icon),
      title: Text(c.label),
      subtitle: Text(c.description),
      onTap: () => Navigator.of(context).pop(c),
    );
    return SafeArea(
      child: SingleChildScrollView(
        padding: const EdgeInsets.only(bottom: 8),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 4),
              child: Text(trial ? 'Add a recipe to try' : 'Add a recipe', style: theme.textTheme.titleLarge),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
              child: Text(
                trial
                    ? 'Everything added here starts on your trial list.'
                    : 'Recipes from a website, text or a photo start on your trial list; '
                          'you can switch that off before importing.',
                style: theme.textTheme.bodyMedium?.copyWith(color: theme.colorScheme.onSurfaceVariant),
              ),
            ),
            for (final c in choices)
              if (c != AddChoice.collection) tile(c),
            if (choices.contains(AddChoice.collection)) ...[const Divider(height: 16), tile(AddChoice.collection)],
          ],
        ),
      ),
    );
  }
}
