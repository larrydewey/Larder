import 'json.dart';

const mealTypes = ['breakfast', 'lunch', 'dinner', 'snack'];

String mealLabel(String meal) => meal.isEmpty ? meal : meal[0].toUpperCase() + meal.substring(1);

/// `YYYY-MM-DD` for a local calendar date.
String dateKey(DateTime d) =>
    '${d.year.toString().padLeft(4, '0')}-${d.month.toString().padLeft(2, '0')}-${d.day.toString().padLeft(2, '0')}';

DateTime parseDateKey(String s) {
  final p = s.split('-');
  if (p.length != 3) return DateTime.now();
  return DateTime(int.tryParse(p[0]) ?? 1970, int.tryParse(p[1]) ?? 1, int.tryParse(p[2]) ?? 1);
}

class MealPlanEntry {
  const MealPlanEntry({
    required this.id,
    required this.date,
    this.meal = 'dinner',
    this.recipeId,
    this.recipeName,
    this.recipeImageUrl,
    this.title,
    this.note,
    this.servings,
    this.createdBy,
    this.updatedAt,
    this.recipeStatus,
    this.trialOutcome,
    this.trialResolvedAt,
  });

  final String id;

  /// `YYYY-MM-DD`
  final String date;
  final String meal;
  final String? recipeId;
  final String? recipeName;
  final String? recipeImageUrl;
  final String? title;
  final String? note;
  final double? servings;
  final String? createdBy;
  final DateTime? updatedAt;

  /// The recipe's status as the server last reported it (read-only).
  final String? recipeStatus;

  /// How "Did you make it?" was answered for this entry ([TrialOutcome]);
  /// null while unanswered. Asked once per entry.
  final String? trialOutcome;
  final DateTime? trialResolvedAt;

  String get displayTitle => recipeName ?? title ?? '';

  factory MealPlanEntry.fromJson(Json j) => MealPlanEntry(
    id: asStr(j['id']),
    date: asStr(j['date']),
    meal: asStr(j['meal'], 'dinner'),
    recipeId: asString(j['recipe_id']),
    recipeName: asString(j['recipe_name']),
    recipeImageUrl: asString(j['recipe_image_url']),
    title: asString(j['title']),
    note: asString(j['note']),
    servings: asDouble(j['servings']),
    createdBy: asString(j['created_by']),
    updatedAt: asDate(j['updated_at']),
    recipeStatus: asString(j['recipe_status']),
    trialOutcome: asString(j['trial_outcome']),
    trialResolvedAt: asDate(j['trial_resolved_at']),
  );

  Json toJson() => {
    'id': id,
    'date': date,
    'meal': meal,
    'recipe_id': recipeId,
    'recipe_name': recipeName,
    'recipe_image_url': recipeImageUrl,
    'title': title,
    'note': note,
    'servings': servings,
    'created_by': createdBy,
    'updated_at': updatedAt?.toUtc().toIso8601String(),
    'recipe_status': recipeStatus,
    'trial_outcome': trialOutcome,
    'trial_resolved_at': trialResolvedAt?.toUtc().toIso8601String(),
  };

  /// Body for POST/PUT /api/mealplan.
  Json toInput({bool includeId = true}) => {
    if (includeId) 'id': id,
    'date': date,
    'meal': meal,
    'recipe_id': recipeId,
    'title': title,
    'note': note,
    'servings': jsonNum(servings),
    // Omitted = unchanged on the server.
    if (trialOutcome != null) 'trial_outcome': trialOutcome,
  };

  MealPlanEntry copyWith({
    String? date,
    String? meal,
    String? note,
    double? servings,
    String? title,
    String? trialOutcome,
    DateTime? trialResolvedAt,
  }) => MealPlanEntry(
    id: id,
    date: date ?? this.date,
    meal: meal ?? this.meal,
    recipeId: recipeId,
    recipeName: recipeName,
    recipeImageUrl: recipeImageUrl,
    title: title ?? this.title,
    note: note ?? this.note,
    servings: servings ?? this.servings,
    createdBy: createdBy,
    updatedAt: DateTime.now().toUtc(),
    recipeStatus: recipeStatus,
    trialOutcome: trialOutcome ?? this.trialOutcome,
    trialResolvedAt: trialResolvedAt ?? this.trialResolvedAt,
  );
}
