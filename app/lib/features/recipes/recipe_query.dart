import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../models/recipe.dart';
import '../../util/text_fold.dart';

enum RecipeSort {
  name('name', 'Name (A–Z)'),
  nameDesc('-name', 'Name (Z–A)'),
  updated('-updated', 'Recently updated'),
  created('-created', 'Recently added'),
  rating('-rating', 'Highest rated'),
  quickest('time', 'Quickest');

  const RecipeSort(this.key, this.label);
  final String key;
  final String label;
}

@immutable
class RecipeQuery {
  const RecipeQuery({
    this.text = '',
    this.tags = const {},
    this.categories = const {},
    this.favoritesOnly = false,
    this.sort = RecipeSort.name,
  });

  final String text;
  final Set<String> tags;
  final Set<String> categories;
  final bool favoritesOnly;
  final RecipeSort sort;

  bool get isFiltered => text.trim().isNotEmpty || tags.isNotEmpty || categories.isNotEmpty || favoritesOnly;

  RecipeQuery copyWith({
    String? text,
    Set<String>? tags,
    Set<String>? categories,
    bool? favoritesOnly,
    RecipeSort? sort,
  }) => RecipeQuery(
    text: text ?? this.text,
    tags: tags ?? this.tags,
    categories: categories ?? this.categories,
    favoritesOnly: favoritesOnly ?? this.favoritesOnly,
    sort: sort ?? this.sort,
  );
}

final recipeQueryProvider = NotifierProvider<RecipeQueryController, RecipeQuery>(RecipeQueryController.new);

class RecipeQueryController extends Notifier<RecipeQuery> {
  @override
  RecipeQuery build() => const RecipeQuery();

  void update(RecipeQuery Function(RecipeQuery q) f) => state = f(state);
  void clear() => state = RecipeQuery(sort: state.sort);
}

/// Search haystack: the engine's `search_text` when present (standalone),
/// else a Dart accent-fold of the searchable fields.
String recipeHaystack(Recipe r) {
  final st = r.searchText;
  if (st != null && st.isNotEmpty) return '$st\n${foldText(r.name)}';
  return foldText(
    [
      r.name,
      r.description,
      ...r.tags,
      ...r.categories,
      for (final i in r.ingredients) ...[i.original, i.food ?? ''],
    ].join('\n'),
  );
}

bool _containsCi(Iterable<String> values, String wanted) => values.any((v) => v.toLowerCase() == wanted.toLowerCase());

/// Search/filter/sort done locally on the cached recipes (works offline).
/// Mirrors the server semantics: every whitespace-separated term must match
/// name, description, ingredients, tags or categories; all selected tags and
/// categories must be present.
List<Recipe> applyRecipeQuery(List<Recipe> all, RecipeQuery q) {
  final terms = foldText(q.text).split(RegExp(r'\s+')).where((t) => t.isNotEmpty).toList();
  final out = all.where((r) {
    if (q.favoritesOnly && !r.isFavorite) return false;
    for (final t in q.tags) {
      if (!_containsCi(r.tags, t)) return false;
    }
    for (final c in q.categories) {
      if (!_containsCi(r.categories, c)) return false;
    }
    if (terms.isNotEmpty) {
      final h = recipeHaystack(r);
      for (final t in terms) {
        if (!h.contains(t)) return false;
      }
    }
    return true;
  }).toList();

  int byName(Recipe a, Recipe b) => a.name.toLowerCase().compareTo(b.name.toLowerCase());
  final epoch = DateTime.fromMillisecondsSinceEpoch(0);
  switch (q.sort) {
    case RecipeSort.name:
      out.sort(byName);
    case RecipeSort.nameDesc:
      out.sort((a, b) => byName(b, a));
    case RecipeSort.updated:
      out.sort((a, b) => (b.updatedAt ?? epoch).compareTo(a.updatedAt ?? epoch));
    case RecipeSort.created:
      out.sort((a, b) => (b.createdAt ?? b.updatedAt ?? epoch).compareTo(a.createdAt ?? a.updatedAt ?? epoch));
    case RecipeSort.rating:
      out.sort((a, b) {
        final c = (b.rating ?? -1).compareTo(a.rating ?? -1);
        return c != 0 ? c : byName(a, b);
      });
    case RecipeSort.quickest:
      out.sort((a, b) {
        final c = (a.effectiveTotalMinutes ?? 1 << 30).compareTo(b.effectiveTotalMinutes ?? 1 << 30);
        return c != 0 ? c : byName(a, b);
      });
  }
  return out;
}
