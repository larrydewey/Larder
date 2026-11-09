/// The chapters the server will produce for a cookbook: a port of
/// `group_recipes` / `chapter_label` in server/crates/cookbook/src/book.rs.
library;

import '../../models/recipe.dart';
import '../../util/categories.dart';
import '../../util/text_fold.dart';

/// Catch-all chapter for recipes without a category/tag (Rust `OTHER_CHAPTER`).
const otherChapter = 'Other Recipes';

/// The single chapter when grouping is off or nothing has a category/tag
/// (Rust `ALL_CHAPTER`).
const allChapter = 'Recipes';

final _upper = RegExp(r'\p{Uppercase}', unicode: true);
final _alnum = RegExp(r'[\p{Alphabetic}\p{N}]', unicode: true);

/// Rust `capitalize_first`.
String _capitalizeFirst(String s) {
  if (s.isEmpty) return s;
  final first = s.runes.first;
  final head = String.fromCharCode(first);
  return '${head.toUpperCase()}${s.substring(head.length)}';
}

/// Rust `chapter_label`: collapses whitespace and capitalises the first
/// letter of an all-lower-case name ("main dishes" → "Main dishes").
String chapterLabel(String s) {
  final c = collapseWs(s);
  return _upper.hasMatch(c) ? c : _capitalizeFirst(c);
}

/// Rust `larder_cookbook::text::sort_key`.
String chapterSortKey(String s) {
  final folded = foldText(s);
  var i = 0;
  final runes = folded.runes.toList();
  while (i < runes.length && !_alnum.hasMatch(String.fromCharCode(runes[i]))) {
    i++;
  }
  return String.fromCharCodes(runes.sublist(i));
}

/// Compares like Rust `str::cmp` (by code point).
int _cmp(String a, String b) {
  final x = a.runes.toList(), y = b.runes.toList();
  for (var i = 0; i < x.length && i < y.length; i++) {
    if (x[i] != y[i]) return x[i].compareTo(y[i]);
  }
  return x.length.compareTo(y.length);
}

/// The chapter titles the server will produce for [recipes] (the selected
/// ones) grouped by [groupBy] ('category', 'tag' or 'none'), in the server's
/// default order: Larder's standard categories in meal order, then other
/// names alphabetically, then [otherChapter] last.
///
/// Each recipe lands in its FIRST non-empty category (or tag); names merge
/// case-insensitively keeping the first-seen spelling.
List<String> cookbookChapters(Iterable<Recipe> recipes, String groupBy) {
  if (recipes.isEmpty) return const [];
  if (groupBy != 'category' && groupBy != 'tag') return const [allChapter];
  // lower-cased key -> display title
  final groups = <String, String>{};
  var other = false;
  for (final r in recipes) {
    final names = groupBy == 'tag' ? r.tags : r.categories;
    final first = names.where((c) => c.trim().isNotEmpty).firstOrNull;
    if (first == null) {
      other = true;
      continue;
    }
    final title = chapterLabel(first);
    groups.putIfAbsent(title.toLowerCase(), () => title);
  }
  if (groups.isEmpty) return const [allChapter];
  // A category literally named "Other Recipes" absorbs the catch-all and,
  // like the catch-all, is listed last.
  final catchAll = groups.remove(otherChapter.toLowerCase()) ?? (other ? otherChapter : null);
  // Larder's standard categories come first, in meal order; any other names
  // follow alphabetically (book.rs `meal_rank`, then sort key, then title).
  int rank(String t) => categoryRank(t) ?? larderCategories.length;
  final rest = groups.values.toList()
    ..sort((a, b) {
      var c = rank(a).compareTo(rank(b));
      if (c != 0) return c;
      c = _cmp(chapterSortKey(a), chapterSortKey(b));
      return c != 0 ? c : _cmp(a, b);
    });
  return [...rest, ?catchAll];
}
