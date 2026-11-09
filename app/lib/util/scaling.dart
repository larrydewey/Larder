/// Ingredient scaling + display formatting.
library;

import '../models/recipe.dart';
import 'units.dart';

/// Scale factor for showing [targetServings] of a recipe yielding [yieldQuantity].
double scaleFactor(double? yieldQuantity, double targetServings) {
  if (yieldQuantity == null || yieldQuantity <= 0) return targetServings <= 0 ? 1 : targetServings;
  return targetServings / yieldQuantity;
}

/// Rust `scaled_parts`: applies scale + conversion and returns
/// (quantity, quantity_max, unit).
(double?, double?, String?) scaledParts(Ingredient ing, double scale, UnitSystem system) {
  final q = ing.quantity == null ? null : ing.quantity! * scale;
  final qmax = ing.quantityMax == null ? null : ing.quantityMax! * scale;
  final unit = ing.unit;
  if (q != null && unit != null) {
    final c = convert(q, unit, system);
    if (c != null) {
      final (cq, cu) = c;
      final ratio = q != 0.0 ? cq / q : 1.0;
      double round(double x) => system == UnitSystem.metric ? roundMetric(x, cu) : x;
      return (round(cq), qmax == null ? null : round(qmax * ratio), cu);
    }
  }
  return (q, qmax, unit);
}

/// The quantity + unit part of [ingredientDisplay] ("1 1/2 cups"), or ''.
String _amountText(double? q, double? qmax, String? unit) {
  final parts = <String>[];
  if (q != null) {
    parts.add(qmax != null && qmax > q ? '${formatQuantity(q)}–${formatQuantity(qmax)}' : formatQuantity(q));
  }
  if (unit != null) parts.add(unitLabel(unit, qmax ?? q));
  return parts.join(' ');
}

/// Rust `ingredient_display`: renders an ingredient as text, e.g.
/// "1 1/2 cups flour, sifted". When no structured data is available the
/// original line is returned.
String ingredientDisplay(Ingredient ing, double scale, UnitSystem system) {
  if (ing.food == null && ing.quantity == null) return ing.original;
  final (q, qmax, unit) = scaledParts(ing, scale, system);
  var s = [if (q != null || unit != null) _amountText(q, qmax, unit), ?ing.food].join(' ');
  final note = ing.note;
  if (note != null && note.isNotEmpty) s = s.isEmpty ? note : '$s, $note';
  return s;
}

class IngredientDisplay {
  const IngredientDisplay(this.amount, this.text, this.full);

  /// Quantity + unit as the printed cookbook shows it ("1½ cups"); may be empty.
  final String amount;

  /// The rest of the line ("flour, sifted"), or the whole line when there is
  /// no separate amount.
  final String text;

  /// The whole line exactly as the server/engine renders it
  /// (Rust `ingredient_display`).
  final String full;

  @override
  String toString() => full;
}

/// Formats [ing] scaled by [factor] and converted to [system], producing the
/// same amounts as the server and the printed cookbook.
///
/// When nothing changes (factor 1, original units) the original text is shown
/// verbatim, as typed.
IngredientDisplay displayIngredient(Ingredient ing, {double factor = 1, UnitSystem system = UnitSystem.original}) {
  final unchanged = (factor - 1).abs() < 1e-9 && system == UnitSystem.original;
  if (unchanged) return IngredientDisplay('', ing.original, ing.original);
  final full = ingredientDisplay(ing, factor, system);
  // Split off the amount like the cookbook's `ingredient_line`.
  var amount = '';
  if (ing.quantity != null && (ing.food != null || ing.unit != null)) {
    final (q, qmax, unit) = scaledParts(ing, factor, system);
    final candidate = _amountText(q, qmax, unit);
    if (candidate.isNotEmpty && full.startsWith(candidate)) amount = candidate;
  }
  var item = full.substring(amount.length).trim();
  while (item.startsWith(',')) {
    item = item.substring(1);
  }
  item = item.trim();
  if (item.isEmpty) return IngredientDisplay('', full, full);
  return IngredientDisplay(unicodeFractions(amount), item, full);
}

/// Groups items by their (nullable) section preserving first-seen order.
List<MapEntry<String?, List<T>>> groupBySection<T>(List<T> items, String? Function(T) sectionOf) {
  final out = <MapEntry<String?, List<T>>>[];
  for (final it in items) {
    final s = sectionOf(it);
    if (out.isNotEmpty && out.last.key == s) {
      out.last.value.add(it);
    } else {
      out.add(MapEntry(s, [it]));
    }
  }
  return out;
}
