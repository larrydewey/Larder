/// Larder's standard recipe categories, in meal order.
///
/// Mirrors `larder_core::categorize::CATEGORIES`
/// (server/crates/core/src/categorize.rs). The server's category suggestions
/// only ever produce these names, and the printed cookbook orders chapters
/// with these names first, in this order.
library;

const larderCategories = <String>[
  'Breakfast',
  'Appetizers & Snacks',
  'Soups & Stews',
  'Salads',
  'Main Dishes',
  'Side Dishes',
  'Breads',
  'Desserts',
  'Drinks',
  'Sauces & Condiments',
  'Seasonings & Spice Mixes',
  'Household & DIY',
];

/// Lower-cases ASCII letters only, like Rust's `eq_ignore_ascii_case`.
String asciiLower(String s) => String.fromCharCodes(s.codeUnits.map((c) => c >= 0x41 && c <= 0x5A ? c + 0x20 : c));

/// Rust `a.eq_ignore_ascii_case(b)`.
bool eqIgnoreAsciiCase(String a, String b) => a.length == b.length && asciiLower(a) == asciiLower(b);

/// Position of [title] in [larderCategories] (ASCII case-insensitive), or
/// null for any other name. Used for the cookbook's meal-order sort.
int? categoryRank(String title) {
  final t = asciiLower(title);
  for (var i = 0; i < larderCategories.length; i++) {
    if (asciiLower(larderCategories[i]) == t) return i;
  }
  return null;
}

/// Category suggestions for an editor: the household's own categories first,
/// then the standard ones that are not already there (case-insensitive).
List<String> categorySuggestions(Iterable<String> existing) {
  final seen = <String>{};
  final out = <String>[];
  for (final c in [...existing, ...larderCategories]) {
    final t = c.trim();
    if (t.isNotEmpty && seen.add(t.toLowerCase())) out.add(t);
  }
  return out;
}
