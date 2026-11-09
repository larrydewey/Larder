/// Tolerant JSON readers: never throw on missing / mistyped fields.
library;

typedef Json = Map<String, dynamic>;

String? asString(Object? v) {
  if (v == null) return null;
  if (v is String) return v;
  return v.toString();
}

String asStr(Object? v, [String fallback = '']) => asString(v) ?? fallback;

int? asInt(Object? v) {
  if (v is int) return v;
  if (v is num) return v.round();
  if (v is String) return int.tryParse(v) ?? double.tryParse(v)?.round();
  return null;
}

double? asDouble(Object? v) {
  if (v is num) return v.toDouble();
  if (v is String) return double.tryParse(v);
  return null;
}

bool asBool(Object? v, [bool fallback = false]) {
  if (v is bool) return v;
  if (v is String) return v == 'true' || v == '1';
  if (v is num) return v != 0;
  return fallback;
}

DateTime? asDate(Object? v) {
  if (v is String) return DateTime.tryParse(v);
  return null;
}

List<String> asStringList(Object? v) {
  if (v is List) return v.where((e) => e != null).map((e) => e.toString()).toList();
  return const [];
}

Json asJson(Object? v) {
  if (v is Map<String, dynamic>) return v;
  if (v is Map) return v.map((k, val) => MapEntry(k.toString(), val));
  return <String, dynamic>{};
}

List<Json> asJsonList(Object? v) {
  if (v is List) return v.whereType<Map>().map(asJson).toList();
  return const [];
}

Map<String, String> asStringMap(Object? v) {
  if (v is Map) {
    return {
      for (final e in v.entries)
        if (e.value != null) e.key.toString(): e.value.toString(),
    };
  }
  return const {};
}

/// Converts any (possibly nested, possibly `Map<dynamic,dynamic>`) JSON value
/// into plain JSON-compatible structures.
Object? deepJson(Object? v) {
  if (v is Map) return {for (final e in v.entries) e.key.toString(): deepJson(e.value)};
  if (v is List) return v.map(deepJson).toList();
  return v;
}

/// Encodes whole doubles as ints (`6` instead of `6.0`) so integer fields on
/// the server deserialize cleanly.
num? jsonNum(double? v) {
  if (v == null || v.isNaN || v.isInfinite) return null;
  return v == v.roundToDouble() ? v.toInt() : v;
}
