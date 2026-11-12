import 'json.dart';

class Ingredient {
  const Ingredient({
    this.section,
    this.quantity,
    this.quantityMax,
    this.unit,
    this.food,
    this.note,
    required this.original,
    this.parsed = true,
  });

  /// A line that only carries the text as entered; the server will parse it.
  const Ingredient.unparsed(this.original, {this.section})
    : quantity = null,
      quantityMax = null,
      unit = null,
      food = null,
      note = null,
      parsed = false;

  final String? section;
  final double? quantity;
  final double? quantityMax;
  final String? unit;
  final String? food;
  final String? note;
  final String original;

  /// False for lines edited locally that have not been parsed by the server
  /// yet. Such lines are sent with only `original` (+ `section`).
  final bool parsed;

  factory Ingredient.fromJson(Json j) {
    final original = asStr(j['original']);
    final hasParsed = j.containsKey('food') || j.containsKey('quantity') || j.containsKey('unit');
    return Ingredient(
      section: _nullIfEmpty(asString(j['section'])),
      quantity: asDouble(j['quantity']),
      quantityMax: asDouble(j['quantity_max']),
      unit: _nullIfEmpty(asString(j['unit'])),
      food: _nullIfEmpty(asString(j['food'])),
      note: _nullIfEmpty(asString(j['note'])),
      original: original.isEmpty ? asStr(j['food']) : original,
      parsed: j['_parsed'] is bool ? j['_parsed'] as bool : hasParsed,
    );
  }

  /// Full JSON (used for the local cache and for unchanged lines).
  Json toJson() => {
    'section': section,
    'quantity': quantity,
    'quantity_max': quantityMax,
    'unit': unit,
    'food': food,
    'note': note,
    'original': original,
    if (!parsed) '_parsed': false,
  };

  /// JSON for the API: unparsed lines only send `original` and `section`.
  Json toApiJson() => parsed
      ? {
          'section': section,
          'quantity': jsonNum(quantity),
          'quantity_max': jsonNum(quantityMax),
          'unit': unit,
          'food': food,
          'note': note,
          'original': original,
        }
      : {'original': original, if (section != null) 'section': section};

  Ingredient withSection(String? s) => Ingredient(
    section: s,
    quantity: quantity,
    quantityMax: quantityMax,
    unit: unit,
    food: food,
    note: note,
    original: original,
    parsed: parsed,
  );
}

class Instruction {
  const Instruction({this.section, required this.text});
  final String? section;
  final String text;

  factory Instruction.fromJson(Object? raw) {
    if (raw is String) return Instruction(text: raw);
    final j = asJson(raw);
    return Instruction(section: _nullIfEmpty(asString(j['section'])), text: asStr(j['text']));
  }

  Json toJson() => {'section': section, 'text': text};
}

String? _nullIfEmpty(String? s) => (s == null || s.trim().isEmpty) ? null : s;

/// Where a recipe lives: the main cookbook, or the trial list of recipes the
/// household wants to try before they join the cookbook.
abstract final class RecipeStatus {
  static const cookbook = 'cookbook';
  static const trial = 'trial';

  /// Unknown values (and null) mean the cookbook.
  static String parse(Object? v) => asString(v)?.trim().toLowerCase() == trial ? trial : cookbook;
}

class Recipe {
  const Recipe({
    required this.id,
    this.householdId = '',
    this.slug = '',
    required this.name,
    this.description = '',
    this.yieldQuantity,
    this.yieldUnit = 'servings',
    this.prepTimeMinutes,
    this.cookTimeMinutes,
    this.totalTimeMinutes,
    this.sourceUrl,
    this.sourceName,
    this.notes = '',
    this.rating,
    this.ingredients = const [],
    this.instructions = const [],
    this.tags = const [],
    this.categories = const [],
    this.nutrition = const {},
    this.imageUrl,
    this.isFavorite = false,
    this.createdBy,
    this.createdAt,
    this.updatedAt,
    this.version,
    this.status = RecipeStatus.cookbook,
    this.triedCount = 0,
    this.lastTriedAt,
    this.localImages = const {},
    this.searchText,
  });

  final String id;
  final String householdId;
  final String slug;
  final String name;
  final String description;
  final double? yieldQuantity;
  final String yieldUnit;
  final int? prepTimeMinutes;
  final int? cookTimeMinutes;
  final int? totalTimeMinutes;
  final String? sourceUrl;
  final String? sourceName;
  final String notes;
  final int? rating;
  final List<Ingredient> ingredients;
  final List<Instruction> instructions;
  final List<String> tags;
  final List<String> categories;
  final Map<String, String> nutrition;
  final String? imageUrl;
  final bool isFavorite;
  final String? createdBy;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final int? version;

  /// [RecipeStatus.cookbook] or [RecipeStatus.trial] (household-wide).
  final String status;

  /// How often the household said "I made this" (changed only by verdicts).
  final int triedCount;
  final DateTime? lastTriedAt;

  bool get isTrial => status == RecipeStatus.trial;

  /// On-device processed images (`thumb`/`medium`/`full` -> file path).
  /// Local-only: never sent to the server.
  final Map<String, String> localImages;

  /// Lower-cased, accent-folded search text (from the engine's
  /// `normalize_recipe`). Local-only.
  final String? searchText;

  bool get hasImage => imageUrl != null || localImages.isNotEmpty;

  /// Best local file for [size] (`thumb`/`medium`/`full`), if any.
  String? localImage(String size) {
    if (localImages.isEmpty) return null;
    return localImages[size] ?? localImages['medium'] ?? localImages['full'] ?? localImages.values.first;
  }

  /// Total time, falling back to prep + cook.
  int? get effectiveTotalMinutes {
    if (totalTimeMinutes != null && totalTimeMinutes! > 0) return totalTimeMinutes;
    final s = (prepTimeMinutes ?? 0) + (cookTimeMinutes ?? 0);
    return s > 0 ? s : null;
  }

  factory Recipe.fromJson(Json j) => Recipe(
    id: asStr(j['id']),
    householdId: asStr(j['household_id']),
    slug: asStr(j['slug']),
    name: asStr(j['name']),
    description: asStr(j['description']),
    yieldQuantity: asDouble(j['yield_quantity']),
    yieldUnit: asStr(j['yield_unit'], 'servings').isEmpty ? 'servings' : asStr(j['yield_unit'], 'servings'),
    prepTimeMinutes: asInt(j['prep_time_minutes']),
    cookTimeMinutes: asInt(j['cook_time_minutes']),
    totalTimeMinutes: asInt(j['total_time_minutes']),
    sourceUrl: _nullIfEmpty(asString(j['source_url'])),
    sourceName: _nullIfEmpty(asString(j['source_name'])),
    notes: asStr(j['notes']),
    rating: asInt(j['rating']),
    ingredients: asJsonList(j['ingredients']).map(Ingredient.fromJson).toList(),
    instructions: (j['instructions'] is List ? j['instructions'] as List : const []).map(Instruction.fromJson).toList(),
    tags: asStringList(j['tags']),
    categories: asStringList(j['categories']),
    nutrition: asStringMap(j['nutrition']),
    imageUrl: _nullIfEmpty(asString(j['image_url'])),
    isFavorite: asBool(j['is_favorite']),
    createdBy: asString(j['created_by']),
    createdAt: asDate(j['created_at']),
    updatedAt: asDate(j['updated_at']),
    version: asInt(j['version']),
    status: RecipeStatus.parse(j['status']),
    triedCount: asInt(j['tried_count']) ?? 0,
    lastTriedAt: asDate(j['last_tried_at']),
    localImages: asStringMap(j['local_images']),
    searchText: asString(j['search_text']),
  );

  /// Full representation (local cache).
  Json toJson() => {
    'id': id,
    'household_id': householdId,
    'slug': slug,
    'name': name,
    'description': description,
    'yield_quantity': yieldQuantity,
    'yield_unit': yieldUnit,
    'prep_time_minutes': prepTimeMinutes,
    'cook_time_minutes': cookTimeMinutes,
    'total_time_minutes': totalTimeMinutes,
    'source_url': sourceUrl,
    'source_name': sourceName,
    'notes': notes,
    'rating': rating,
    'ingredients': ingredients.map((e) => e.toJson()).toList(),
    'instructions': instructions.map((e) => e.toJson()).toList(),
    'tags': tags,
    'categories': categories,
    'nutrition': nutrition,
    'image_url': imageUrl,
    'is_favorite': isFavorite,
    'created_by': createdBy,
    'created_at': createdAt?.toUtc().toIso8601String(),
    'updated_at': updatedAt?.toUtc().toIso8601String(),
    'version': version,
    'status': status,
    'tried_count': triedCount,
    'last_tried_at': lastTriedAt?.toUtc().toIso8601String(),
    if (localImages.isNotEmpty) 'local_images': localImages,
    if (searchText != null) 'search_text': searchText,
  };

  /// `RecipeInput` for POST/PUT. [includeVersion] enables optimistic
  /// concurrency on update. The trial counters are not part of it: the
  /// server only changes them through verdicts (`POST /recipes/{id}/tried`).
  Json toInput({bool includeId = true, bool includeVersion = true}) => {
    if (includeId) 'id': id,
    'name': name,
    'description': description,
    'yield_quantity': jsonNum(yieldQuantity),
    'yield_unit': yieldUnit,
    'prep_time_minutes': prepTimeMinutes,
    'cook_time_minutes': cookTimeMinutes,
    'total_time_minutes': totalTimeMinutes,
    'source_url': sourceUrl,
    'source_name': sourceName,
    'notes': notes,
    'rating': rating,
    'ingredients': ingredients.map((e) => e.toApiJson()).toList(),
    'instructions': instructions.map((e) => e.toJson()).toList(),
    'tags': tags,
    'categories': categories,
    'nutrition': nutrition,
    'status': status,
    if (includeVersion && version != null) 'version': version,
  };

  Recipe copyWith({
    String? id,
    String? name,
    String? description,
    double? Function()? yieldQuantity,
    String? yieldUnit,
    int? Function()? prepTimeMinutes,
    int? Function()? cookTimeMinutes,
    int? Function()? totalTimeMinutes,
    String? Function()? sourceUrl,
    String? Function()? sourceName,
    String? notes,
    int? Function()? rating,
    List<Ingredient>? ingredients,
    List<Instruction>? instructions,
    List<String>? tags,
    List<String>? categories,
    Map<String, String>? nutrition,
    String? Function()? imageUrl,
    bool? isFavorite,
    DateTime? updatedAt,
    int? Function()? version,
    Map<String, String>? localImages,
    String? Function()? searchText,
    String? slug,
    String? householdId,
    String? createdBy,
    DateTime? createdAt,
    String? status,
    int? triedCount,
    DateTime? Function()? lastTriedAt,
  }) => Recipe(
    id: id ?? this.id,
    householdId: householdId ?? this.householdId,
    slug: slug ?? this.slug,
    name: name ?? this.name,
    description: description ?? this.description,
    yieldQuantity: yieldQuantity != null ? yieldQuantity() : this.yieldQuantity,
    yieldUnit: yieldUnit ?? this.yieldUnit,
    prepTimeMinutes: prepTimeMinutes != null ? prepTimeMinutes() : this.prepTimeMinutes,
    cookTimeMinutes: cookTimeMinutes != null ? cookTimeMinutes() : this.cookTimeMinutes,
    totalTimeMinutes: totalTimeMinutes != null ? totalTimeMinutes() : this.totalTimeMinutes,
    sourceUrl: sourceUrl != null ? sourceUrl() : this.sourceUrl,
    sourceName: sourceName != null ? sourceName() : this.sourceName,
    notes: notes ?? this.notes,
    rating: rating != null ? rating() : this.rating,
    ingredients: ingredients ?? this.ingredients,
    instructions: instructions ?? this.instructions,
    tags: tags ?? this.tags,
    categories: categories ?? this.categories,
    nutrition: nutrition ?? this.nutrition,
    imageUrl: imageUrl != null ? imageUrl() : this.imageUrl,
    isFavorite: isFavorite ?? this.isFavorite,
    createdBy: createdBy ?? this.createdBy,
    createdAt: createdAt ?? this.createdAt,
    updatedAt: updatedAt ?? this.updatedAt,
    version: version != null ? version() : this.version,
    status: status ?? this.status,
    triedCount: triedCount ?? this.triedCount,
    lastTriedAt: lastTriedAt != null ? lastTriedAt() : this.lastTriedAt,
    localImages: localImages ?? this.localImages,
    searchText: searchText != null ? searchText() : this.searchText,
  );
}

/// Field-by-field equality of two ingredient lists (the fields the server
/// compares: section, quantities, unit, food, note and original text).
bool sameIngredients(List<Ingredient> a, List<Ingredient> b) {
  if (a.length != b.length) return false;
  for (var i = 0; i < a.length; i++) {
    final x = a[i], y = b[i];
    if (x.section != y.section ||
        x.quantity != y.quantity ||
        x.quantityMax != y.quantityMax ||
        x.unit != y.unit ||
        x.food != y.food ||
        x.note != y.note ||
        x.original != y.original) {
      return false;
    }
  }
  return true;
}
