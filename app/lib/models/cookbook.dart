import 'json.dart';

class CookbookTheme {
  const CookbookTheme({required this.id, required this.name, this.description = ''});
  final String id;
  final String name;
  final String description;

  factory CookbookTheme.fromJson(Json j) =>
      CookbookTheme(id: asStr(j['id']), name: asStr(j['name']), description: asStr(j['description']));
}

class CookbookOptions {
  CookbookOptions({
    this.title = 'Family Cookbook',
    this.subtitle = '',
    this.author = '',
    this.dedication = '',
    this.theme = 'classic',
    this.paper = 'letter',
    this.recipeIds,
    this.groupBy = 'category',
    this.chapterOrder,
    this.includeImages = true,
    this.includeToc = true,
    this.includeIndex = true,
    this.includeNotes = true,
    this.includeNutrition = false,
    this.units = 'original',
  });

  String title;
  String subtitle;
  String author;
  String dedication;
  String theme;
  String paper;
  List<String>? recipeIds;
  String groupBy;
  List<String>? chapterOrder;
  bool includeImages;
  bool includeToc;
  bool includeIndex;
  bool includeNotes;
  bool includeNutrition;
  String units;

  Json toJson() {
    String? opt(String s) => s.trim().isEmpty ? null : s.trim();
    return {
      'title': title.trim().isEmpty ? 'Cookbook' : title.trim(),
      if (opt(subtitle) != null) 'subtitle': opt(subtitle),
      if (opt(author) != null) 'author': opt(author),
      if (opt(dedication) != null) 'dedication': opt(dedication),
      'theme': theme,
      'paper': paper,
      'recipe_ids': recipeIds,
      'group_by': groupBy,
      if (chapterOrder != null && chapterOrder!.isNotEmpty) 'chapter_order': chapterOrder,
      'include_images': includeImages,
      'include_toc': includeToc,
      'include_index': includeIndex,
      'include_notes': includeNotes,
      'include_nutrition': includeNutrition,
      'units': units,
    };
  }
}
