/// Unit catalogue, alias matching and metric/imperial conversion.
///
/// A faithful port of `server/crates/core/src/units.rs`; keep the two in sync
/// (test/util/units_test.dart holds the shared expectations). The printed
/// cookbook is rendered by the Rust code, so any difference here shows up as
/// different amounts on the recipe page and in the book.
library;

import 'dart:convert';

import 'text_fold.dart';

enum UnitSystem {
  original,
  metric,
  imperial;

  /// Rust `UnitSystem::parse`.
  static UnitSystem parse(String? s) => switch (s?.toLowerCase()) {
    'metric' => UnitSystem.metric,
    'imperial' || 'us' => UnitSystem.imperial,
    _ => UnitSystem.original,
  };

  String get label => switch (this) {
    UnitSystem.original => 'Original',
    UnitSystem.metric => 'Metric',
    UnitSystem.imperial => 'Imperial',
  };
}

enum Dimension { volume, mass, count }

class UnitDef {
  const UnitDef({
    required this.key,
    required this.singular,
    required this.plural,
    required this.dim,
    required this.factor,
    required this.metric,
    required this.aliases,
    this.csAliases = const [],
  });

  final String key;
  final String singular;
  final String plural;
  final Dimension dim;

  /// Millilitres for volume, grams for mass, 1 for counts.
  final double factor;
  final bool metric;

  /// Lower-case aliases (matched case-insensitively) …
  final List<String> aliases;

  /// … and case-sensitive aliases (e.g. "T" = tablespoon, "t" = teaspoon).
  final List<String> csAliases;
}

UnitDef _count(String key, String plural, List<String> aliases) =>
    UnitDef(key: key, singular: key, plural: plural, dim: Dimension.count, factor: 1, metric: false, aliases: aliases);

/// Rust `UNITS` (same order: lookups return the first match).
final List<UnitDef> units = List.unmodifiable(<UnitDef>[
  const UnitDef(
    key: 'tsp',
    singular: 'tsp',
    plural: 'tsp',
    dim: Dimension.volume,
    factor: 4.92892,
    metric: false,
    // "cc" moved to ml on the server (cc = cubic centimetre).
    aliases: ['tsp', 'tsps', 'teaspoon', 'teaspoons', 'tea spoon', 'tea spoons', 'tspn', 'c. à c.'],
    csAliases: ['t'],
  ),
  const UnitDef(
    key: 'tbsp',
    singular: 'tbsp',
    plural: 'tbsp',
    dim: Dimension.volume,
    factor: 14.7868,
    metric: false,
    aliases: [
      'tbsp',
      'tbsps',
      'tbs',
      'tbl',
      'tbls',
      'tablespoon',
      'tablespoons',
      'table spoon',
      'table spoons',
      'tblsp',
      'c. à s.',
      'el',
    ],
    csAliases: ['T', 'Tb'],
  ),
  const UnitDef(
    key: 'fl oz',
    singular: 'fl oz',
    plural: 'fl oz',
    dim: Dimension.volume,
    factor: 29.5735,
    metric: false,
    aliases: ['fl oz', 'fl. oz', 'fl.oz', 'floz', 'fluid ounce', 'fluid ounces', 'fl ounce', 'fl ounces'],
  ),
  const UnitDef(
    key: 'cup',
    singular: 'cup',
    plural: 'cups',
    dim: Dimension.volume,
    factor: 236.588,
    metric: false,
    aliases: ['cup', 'cups', 'c', 'cp', 'tasse', 'tassen'],
    csAliases: ['C'],
  ),
  const UnitDef(
    key: 'pint',
    singular: 'pint',
    plural: 'pints',
    dim: Dimension.volume,
    factor: 473.176,
    metric: false,
    aliases: ['pint', 'pints', 'pt', 'pts'],
  ),
  const UnitDef(
    key: 'quart',
    singular: 'quart',
    plural: 'quarts',
    dim: Dimension.volume,
    factor: 946.353,
    metric: false,
    aliases: ['quart', 'quarts', 'qt', 'qts'],
  ),
  const UnitDef(
    key: 'gallon',
    singular: 'gallon',
    plural: 'gallons',
    dim: Dimension.volume,
    factor: 3785.41,
    metric: false,
    aliases: ['gallon', 'gallons', 'gal', 'gals'],
  ),
  const UnitDef(
    key: 'ml',
    singular: 'ml',
    plural: 'ml',
    dim: Dimension.volume,
    factor: 1,
    metric: true,
    aliases: ['ml', 'mls', 'milliliter', 'milliliters', 'millilitre', 'millilitres', 'cc', 'cc.'],
    csAliases: ['mL'],
  ),
  const UnitDef(
    key: 'cl',
    singular: 'cl',
    plural: 'cl',
    dim: Dimension.volume,
    factor: 10,
    metric: true,
    aliases: ['cl', 'centiliter', 'centiliters', 'centilitre', 'centilitres'],
  ),
  const UnitDef(
    key: 'dl',
    singular: 'dl',
    plural: 'dl',
    dim: Dimension.volume,
    factor: 100,
    metric: true,
    aliases: ['dl', 'deciliter', 'deciliters', 'decilitre', 'decilitres'],
  ),
  const UnitDef(
    key: 'l',
    singular: 'l',
    plural: 'l',
    dim: Dimension.volume,
    factor: 1000,
    metric: true,
    aliases: ['l', 'liter', 'liters', 'litre', 'litres', 'ltr', 'ltrs', 'lt'],
    csAliases: ['L'],
  ),
  const UnitDef(
    key: 'oz',
    singular: 'oz',
    plural: 'oz',
    dim: Dimension.mass,
    factor: 28.3495,
    metric: false,
    aliases: ['oz', 'ozs', 'ounce', 'ounces', 'oz.'],
  ),
  const UnitDef(
    key: 'lb',
    singular: 'lb',
    plural: 'lb',
    dim: Dimension.mass,
    factor: 453.592,
    metric: false,
    aliases: ['lb', 'lbs', 'pound', 'pounds', 'lb.', 'lbs.'],
  ),
  const UnitDef(
    key: 'mg',
    singular: 'mg',
    plural: 'mg',
    dim: Dimension.mass,
    factor: 0.001,
    metric: true,
    aliases: ['mg', 'milligram', 'milligrams', 'milligramme', 'milligrammes'],
  ),
  const UnitDef(
    key: 'g',
    singular: 'g',
    plural: 'g',
    dim: Dimension.mass,
    factor: 1,
    metric: true,
    aliases: ['g', 'gr', 'grs', 'gram', 'grams', 'gramme', 'grammes', 'gm', 'gms'],
  ),
  const UnitDef(
    key: 'kg',
    singular: 'kg',
    plural: 'kg',
    dim: Dimension.mass,
    factor: 1000,
    metric: true,
    aliases: ['kg', 'kgs', 'kilogram', 'kilograms', 'kilo', 'kilos', 'kilogramme', 'kilogrammes'],
  ),
  _count('pinch', 'pinches', ['pinch', 'pinches', 'pn']),
  _count('dash', 'dashes', ['dash', 'dashes']),
  _count('clove', 'cloves', ['clove', 'cloves']),
  _count('can', 'cans', ['can', 'cans', 'tin', 'tins']),
  _count('jar', 'jars', ['jar', 'jars']),
  _count('package', 'packages', [
    'package',
    'packages',
    'pkg',
    'pkgs',
    'packet',
    'packets',
    'pack',
    'packs',
    'box',
    'boxes',
    'bag',
    'bags',
    'envelope',
    'envelopes',
  ]),
  _count('slice', 'slices', ['slice', 'slices']),
  _count('piece', 'pieces', ['piece', 'pieces', 'pc', 'pcs']),
  _count('stick', 'sticks', ['stick', 'sticks']),
  _count('bunch', 'bunches', ['bunch', 'bunches', 'bn']),
  _count('sprig', 'sprigs', ['sprig', 'sprigs']),
  _count('handful', 'handfuls', ['handful', 'handfuls']),
  _count('head', 'heads', ['head', 'heads']),
  _count('drop', 'drops', ['drop', 'drops']),
]);

/// Looks up a unit definition by canonical key (Rust `unit_def`).
UnitDef? unitDef(String? key) {
  if (key == null) return null;
  for (final u in units) {
    if (u.key == key) return u;
  }
  return null;
}

/// Rust `str::trim` (Unicode `White_Space`).
String _rustTrim(String s) {
  final r = s.runes.toList();
  var a = 0, b = r.length;
  while (a < b && isRustWhitespace(r[a])) {
    a++;
  }
  while (b > a && isRustWhitespace(r[b - 1])) {
    b--;
  }
  return String.fromCharCodes(r.sublist(a, b));
}

String _trimEndDots(String s) {
  var e = s.length;
  while (e > 0 && s.codeUnitAt(e - 1) == 0x2E) {
    e--;
  }
  return s.substring(0, e);
}

/// Maps any alias (e.g. "Tablespoons", "T", "lbs.") to its canonical key, or
/// null when the unit is unknown (Rust `canonical_unit`).
String? canonicalUnit(String? s) {
  if (s == null) return null;
  final trimmed = _rustTrim(s);
  for (final u in units) {
    if (u.csAliases.contains(trimmed)) return u.key;
  }
  final lower = _trimEndDots(trimmed.toLowerCase());
  for (final u in units) {
    if (u.key == lower || u.aliases.any((a) => _trimEndDots(a) == lower)) return u.key;
  }
  return null;
}

/// Every alias with its canonical key and whether it is case-sensitive,
/// longest (in UTF-8 bytes, like Rust) first; ties keep table order.
List<(String alias, String key, bool caseSensitive)> aliasTable() {
  final v = <(String, String, bool)>[];
  for (final u in units) {
    v.add((u.key, u.key, false));
    for (final a in u.aliases) {
      v.add((a, u.key, false));
    }
    for (final a in u.csAliases) {
      v.add((a, u.key, true));
    }
  }
  // Rust's sort_by_key is stable; Dart's List.sort is not, so sort indices.
  final len = [for (final e in v) utf8.encode(e.$1).length];
  final idx = List<int>.generate(v.length, (i) => i)
    ..sort((a, b) {
      final c = len[b].compareTo(len[a]);
      return c != 0 ? c : a.compareTo(b);
    });
  return [for (final i in idx) v[i]];
}

/// The physical dimension of the canonical unit [key], or null if unknown.
Dimension? dimensionOf(String? key) => unitDef(key)?.dim;

/// Quantity in base units (ml or g) if [unit] is a measurable one (Rust `to_base`).
(double, Dimension)? toBase(double qty, String unit) {
  final def = unitDef(unit);
  if (def == null || def.dim == Dimension.count) return null;
  return (qty * def.factor, def.dim);
}

/// Converts a quantity to the requested unit system, picking a sensible unit.
/// Returns null when no conversion applies (counts, free-text units, or
/// already in the target system). Rust `convert`.
(double, String)? convert(double qty, String unit, UnitSystem system) {
  final def = unitDef(unit);
  if (def == null || def.dim == Dimension.count) return null;
  switch (system) {
    case UnitSystem.original:
      return null;
    case UnitSystem.metric:
      if (def.metric) return null;
      final base = qty * def.factor;
      if (def.dim == Dimension.volume) return base >= 1000.0 ? (base / 1000.0, 'l') : (base, 'ml');
      return base >= 1000.0 ? (base / 1000.0, 'kg') : (base, 'g');
    case UnitSystem.imperial:
      if (!def.metric) return null;
      final base = qty * def.factor;
      if (def.dim == Dimension.volume) {
        final cups = base / 236.588;
        if (cups >= 16.0) return (base / 3785.41, 'gallon');
        if (cups >= 4.0) return (base / 946.353, 'quart');
        if (cups >= 0.25) return (cups, 'cup');
        if (base >= 14.7868 - 0.5) return (base / 14.7868, 'tbsp');
        return (base / 4.92892, 'tsp');
      }
      final oz = base / 28.3495;
      return oz >= 16.0 ? (base / 453.592, 'lb') : (oz, 'oz');
  }
}

/// Rounds converted metric values to kitchen-friendly precision (Rust `round_metric`).
double roundMetric(double qty, String unit) {
  switch (unit) {
    case 'ml' || 'g':
      if (qty >= 100.0) return (qty / 5.0).roundToDouble() * 5.0;
      if (qty >= 10.0) return qty.roundToDouble();
      return (qty * 10.0).roundToDouble() / 10.0;
    case 'l' || 'kg':
      return (qty * 100.0).roundToDouble() / 100.0;
    default:
      return qty;
  }
}

/// Converts [qty] of [from] into [to] if both share a measurable dimension
/// (Rust `convert_between`).
double? convertBetween(double qty, String from, String to) {
  if (from == to) return qty;
  final a = unitDef(from), b = unitDef(to);
  if (a == null || b == null || a.dim != b.dim || a.dim == Dimension.count) return null;
  return qty * a.factor / b.factor;
}

// ---- formatting (server/crates/core/src/format.rs) --------------------------

const _fractions = <(double, String)>[
  (0.125, '1/8'),
  (0.25, '1/4'),
  (1.0 / 3.0, '1/3'),
  (0.375, '3/8'),
  (0.5, '1/2'),
  (0.625, '5/8'),
  (2.0 / 3.0, '2/3'),
  (0.75, '3/4'),
  (0.875, '7/8'),
];

const _vulgar = <(String, String)>[
  ('1/8', '⅛'),
  ('1/4', '¼'),
  ('1/3', '⅓'),
  ('3/8', '⅜'),
  ('1/2', '½'),
  ('5/8', '⅝'),
  ('2/3', '⅔'),
  ('3/4', '¾'),
  ('7/8', '⅞'),
];

String _trimDecimal(double v) {
  var s = v.toStringAsFixed(2);
  var e = s.length;
  while (e > 0 && s[e - 1] == '0') {
    e--;
  }
  while (e > 0 && s[e - 1] == '.') {
    e--;
  }
  s = s.substring(0, e);
  return s == '-0' ? '0' : s;
}

/// Formats a number as a mixed fraction where that reads naturally
/// ("1 1/2", "3/4"), otherwise as a trimmed decimal (Rust `format_quantity`).
String formatQuantity(double q) {
  if (!q.isFinite) return '';
  if (q >= 20.0) return _trimDecimal((q * 10.0).roundToDouble() / 10.0);
  final whole = q.truncateToDouble();
  final frac = q - whole;
  if (frac < 0.03) return _trimDecimal(whole);
  if (frac > 0.97) return _trimDecimal(whole + 1.0);
  for (final (v, s) in _fractions) {
    if ((frac - v).abs() < 0.02) return whole == 0.0 ? s : '${whole.toInt()} $s';
  }
  return _trimDecimal((q * 100.0).roundToDouble() / 100.0);
}

/// Like [formatQuantity] but with Unicode vulgar fractions ("1½")
/// (Rust `format_quantity_unicode`).
String formatQuantityUnicode(double q) {
  final s = formatQuantity(q);
  for (final (a, b) in _vulgar) {
    if (s.endsWith(a)) return '${s.substring(0, s.length - a.length).trimRight()}$b';
  }
  return s;
}

final _fractionRe = RegExp(r'(?:\b(\d+) )?\b(1/8|1/4|1/3|3/8|1/2|5/8|2/3|3/4|7/8)\b');

/// Replaces ASCII kitchen fractions with Unicode ones, as the cookbook does:
/// "1 1/2 cups" → "1½ cups" (Rust `larder_cookbook::text::unicode_fractions`).
String unicodeFractions(String s) => s.replaceAllMapped(_fractionRe, (m) {
  final f = _vulgar.firstWhere((v) => v.$1 == m[2]).$2;
  return m[1] == null ? f : '${m[1]}$f';
});

/// Unit label for display, pluralised where appropriate (Rust `unit_label`).
String unitLabel(String unit, [double? qty]) {
  final def = unitDef(unit);
  if (def == null) return unit;
  return qty != null && qty > 1.0 + 1e-9 ? def.plural : def.singular;
}
