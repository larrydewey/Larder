/// Cooking-friendly number formatting: `1.5` -> `1 ½`, `0.333` -> `⅓`.
library;

const _glyphList = <(double, String)>[
  (1 / 8, '⅛'),
  (1 / 4, '¼'),
  (1 / 3, '⅓'),
  (3 / 8, '⅜'),
  (1 / 2, '½'),
  (5 / 8, '⅝'),
  (2 / 3, '⅔'),
  (3 / 4, '¾'),
  (7 / 8, '⅞'),
];

/// Formats [value] using unicode vulgar fractions when the fractional part
/// is close to a common cooking fraction (eighths and thirds).
///
/// When [snap] is true the fractional part is always rounded to the nearest
/// eighth/third (used for scaled or converted imperial amounts). Otherwise a
/// value that is not close to a common fraction is shown as a decimal.
String formatFraction(double value, {bool snap = false, double tolerance = 0.02}) {
  if (value.isNaN || value.isInfinite) return '';
  if (value < 0) return '-${formatFraction(-value, snap: snap, tolerance: tolerance)}';
  // Large amounts: fractions stop being useful.
  if (value >= 100) return value.round().toString();
  var whole = value.floor();
  final frac = value - whole;
  if (frac < 1e-9) return whole.toString();

  // Candidate fractions including 0 and 1 for rounding.
  double best = 0;
  var bestDiff = frac;
  if ((1 - frac) < bestDiff) {
    best = 1;
    bestDiff = 1 - frac;
  }
  for (final (f, _) in _glyphList) {
    final d = (frac - f).abs();
    if (d < bestDiff) {
      best = f;
      bestDiff = d;
    }
  }

  if (!snap && bestDiff > tolerance) {
    return formatDecimal(value);
  }
  if (snap && whole == 0 && best == 0) {
    // Too small to snap to zero; show a decimal instead of "0".
    return formatDecimal(value, maxDecimals: 2);
  }
  if (best == 1) {
    whole += 1;
    return whole.toString();
  }
  if (best == 0) return whole.toString();
  final glyph = _glyphList.firstWhere((g) => g.$1 == best).$2;
  return whole == 0 ? glyph : '$whole $glyph';
}

/// Formats a decimal with up to [maxDecimals] places, trimming zeros.
String formatDecimal(double value, {int maxDecimals = 2}) {
  var s = value.toStringAsFixed(maxDecimals);
  if (s.contains('.')) {
    s = s.replaceAll(RegExp(r'0+$'), '');
    if (s.endsWith('.')) s = s.substring(0, s.length - 1);
  }
  if (s == '-0') s = '0';
  return s;
}

/// Parses user-entered quantities such as `1 1/2`, `1½`, `0.5`, `3/4`.
double? parseQuantity(String input) {
  var s = input.trim();
  if (s.isEmpty) return null;
  double total = 0;
  for (final (value, glyph) in _glyphList) {
    if (s.contains(glyph)) {
      total += value;
      s = s.replaceAll(glyph, ' ');
    }
  }
  s = s.trim();
  if (s.isEmpty) return total;
  final parts = s.split(RegExp(r'\s+'));
  for (final p in parts) {
    if (p.contains('/')) {
      final nd = p.split('/');
      if (nd.length != 2) return null;
      final n = double.tryParse(nd[0]);
      final d = double.tryParse(nd[1]);
      if (n == null || d == null || d == 0) return null;
      total += n / d;
    } else {
      final v = double.tryParse(p.replaceAll(',', '.'));
      if (v == null) return null;
      total += v;
    }
  }
  return total;
}
