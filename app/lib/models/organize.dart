import 'json.dart';

/// Which recipes get a suggested category (`categorize` in
/// `POST /api/recipes/organize`).
enum CategorizeMode {
  /// Only recipes without any category.
  uncategorized,

  /// Every recipe: the suggestion becomes the first category.
  all,

  /// Leave categories alone.
  none;

  String get label => switch (this) {
    CategorizeMode.uncategorized => 'Only recipes without a category',
    CategorizeMode.all => 'All recipes (replace first category)',
    CategorizeMode.none => "Don't change categories",
  };
}

class OrganizeChange {
  const OrganizeChange({
    required this.id,
    required this.name,
    this.categoryBefore,
    this.categoryAfter,
    this.ingredientsChanged = false,
  });

  final String id;
  final String name;
  final String? categoryBefore;
  final String? categoryAfter;
  final bool ingredientsChanged;

  bool get categoryChanged => categoryBefore != categoryAfter;

  factory OrganizeChange.fromJson(Json j) => OrganizeChange(
    id: asStr(j['id']),
    name: asStr(j['name']),
    categoryBefore: asString(j['category_before']),
    categoryAfter: asString(j['category_after']),
    ingredientsChanged: asBool(j['ingredients_changed']),
  );

  Json toJson() => {
    'id': id,
    'name': name,
    'category_before': categoryBefore,
    'category_after': categoryAfter,
    'ingredients_changed': ingredientsChanged,
  };
}

/// Result of organizing recipes (a preview when [dryRun]).
class OrganizeResult {
  const OrganizeResult({
    required this.dryRun,
    required this.recipesChanged,
    required this.categorized,
    required this.normalized,
    this.changes = const [],
  });

  final bool dryRun;
  final int recipesChanged;
  final int categorized;
  final int normalized;
  final List<OrganizeChange> changes;

  factory OrganizeResult.fromJson(Json j) {
    final changes = asJsonList(j['changes']).map(OrganizeChange.fromJson).toList();
    return OrganizeResult(
      dryRun: asBool(j['dry_run']),
      recipesChanged: asInt(j['recipes_changed']) ?? changes.length,
      categorized: asInt(j['categorized']) ?? 0,
      normalized: asInt(j['normalized']) ?? 0,
      changes: changes,
    );
  }

  /// Changes grouped by the (new) first category, in first-seen order;
  /// recipes left without a category are under null.
  List<MapEntry<String?, List<OrganizeChange>>> get byCategory {
    final groups = <String?, List<OrganizeChange>>{};
    for (final c in changes) {
      groups.putIfAbsent(c.categoryAfter, () => []).add(c);
    }
    return groups.entries.toList();
  }

  /// "3 recipes categorized, 5 tidied" style summary.
  String get summary {
    if (recipesChanged == 0) return 'Everything is already tidy';
    String n(int v, String one) => '$v ${v == 1 ? one : '${one}s'}';
    final parts = [
      if (categorized > 0) '${n(categorized, 'recipe')} categorized',
      if (normalized > 0) '${n(normalized, 'recipe')} with tidied ingredients',
    ];
    return '${n(recipesChanged, 'recipe')} changed: ${parts.join(', ')}';
  }
}
