import '../models/recipe.dart';

/// The address of a recipe in the Larder web app on [serverBase]
/// (for example `https://recipes.example.com`), or null if there is no usable server address.
Uri? recipeLink(String serverBase, Recipe r) {
  final base = Uri.tryParse(serverBase.trim());
  if (base == null || !base.hasScheme || base.host.isEmpty) return null;
  final path = base.path.endsWith('/') ? base.path.substring(0, base.path.length - 1) : base.path;
  return Uri(
    scheme: base.scheme,
    host: base.host,
    port: base.hasPort ? base.port : null,
    path: '$path/recipes/${Uri.encodeComponent(r.id)}',
  );
}

/// What the Share action sends.
///
/// With a server, this is the recipe's name and its Larder link. Without one (standalone mode),
/// it is the recipe as saved here. The source URL is never shared: it is kept as attribution, and
/// the saved recipe may differ from the original.
String recipeShareText(Recipe r, {Uri? link}) {
  if (link != null) return '${r.name}\n\n$link';
  final out = <String>[r.name];
  if (r.description.trim().isNotEmpty) out.add(r.description.trim());
  final ingredients = [
    for (final i in r.ingredients)
      if (i.original.trim().isNotEmpty) '- ${i.original.trim()}',
  ];
  if (ingredients.isNotEmpty) out.add('Ingredients\n${ingredients.join('\n')}');
  final steps = [
    for (final s in r.instructions)
      if (s.text.trim().isNotEmpty) s.text.trim(),
  ];
  if (steps.isNotEmpty) {
    out.add('Instructions\n${[for (var k = 0; k < steps.length; k++) '${k + 1}. ${steps[k]}'].join('\n')}');
  }
  if (r.notes.trim().isNotEmpty) out.add('Notes\n${r.notes.trim()}');
  return out.join('\n\n');
}
