import '../../models/recipe.dart';

/// Converts between the editor's text lines and ingredient/instruction lists.
///
/// A line starting with `#` (e.g. `## For the sauce`) is a section heading
/// that applies to the following lines.
bool isHeading(String line) => line.trimLeft().startsWith('#');
String headingText(String line) => line.trimLeft().replaceFirst(RegExp(r'^#+\s*'), '').trim();

List<String> ingredientsToLines(List<Ingredient> ingredients) {
  final out = <String>[];
  String? section;
  for (final i in ingredients) {
    if (i.section != section && i.section != null) out.add('## ${i.section}');
    section = i.section;
    out.add(i.original);
  }
  return out;
}

/// Builds ingredients from [lines]. Lines whose text and section match an
/// ingredient in [previous] keep its parsed fields; changed lines are sent
/// with only `original` (+ `section`) so the server parses them.
List<Ingredient> linesToIngredients(List<String> lines, List<Ingredient> previous) {
  final pool = [...previous];
  final out = <Ingredient>[];
  String? section;
  for (final raw in lines) {
    final line = raw.trim();
    if (line.isEmpty) continue;
    if (isHeading(line)) {
      final h = headingText(line);
      section = h.isEmpty ? null : h;
      continue;
    }
    final idx = pool.indexWhere((p) => p.original.trim() == line && p.section == section);
    if (idx >= 0) {
      out.add(pool.removeAt(idx));
    } else {
      final sameText = pool.indexWhere((p) => p.original.trim() == line);
      if (sameText >= 0) {
        // Moved to another section: keep the parse, change the section.
        out.add(pool.removeAt(sameText).withSection(section));
      } else {
        out.add(Ingredient.unparsed(line, section: section));
      }
    }
  }
  return out;
}

List<String> instructionsToLines(List<Instruction> steps) {
  final out = <String>[];
  String? section;
  for (final s in steps) {
    if (s.section != section && s.section != null) out.add('## ${s.section}');
    section = s.section;
    out.add(s.text);
  }
  return out;
}

List<Instruction> linesToInstructions(List<String> lines) {
  final out = <Instruction>[];
  String? section;
  for (final raw in lines) {
    final line = raw.trim();
    if (line.isEmpty) continue;
    if (isHeading(line)) {
      final h = headingText(line);
      section = h.isEmpty ? null : h;
      continue;
    }
    out.add(Instruction(section: section, text: line));
  }
  return out;
}
