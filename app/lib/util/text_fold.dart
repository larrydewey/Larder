/// Text folding shared with the server: a faithful port of
/// `larder_core::text::{fold, collapse_ws}` (server/crates/core/src/text.rs).
///
/// The server uses `fold` for search text, duplicate detection
/// (`recipes.name_lower`) and cookbook sort keys, so the app must produce
/// exactly the same strings.
library;

/// Characters Rust's `char::is_whitespace` accepts (Unicode `White_Space`).
const _whitespace = <int>{
  0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x20, 0x85, 0xA0, 0x1680, //
  0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200A,
  0x2028, 0x2029, 0x202F, 0x205F, 0x3000,
};

/// Rust `char::is_whitespace`.
bool isRustWhitespace(int rune) => _whitespace.contains(rune);

/// Rust `collapse_ws`: collapses runs of whitespace to one space and trims.
String collapseWs(String s) {
  final words = <String>[];
  final b = StringBuffer();
  for (final r in s.runes) {
    if (isRustWhitespace(r)) {
      if (b.isNotEmpty) {
        words.add(b.toString());
        b.clear();
      }
    } else {
      b.writeCharCode(r);
    }
  }
  if (b.isNotEmpty) words.add(b.toString());
  return words.join(' ');
}

/// The single-character mappings of Rust `fold` (after lower-casing).
const _map = {
  'àáâãäåā': 'a',
  'çčć': 'c',
  'èéêëēę': 'e',
  'ìíîïī': 'i',
  'ñń': 'n',
  'òóôõöøō': 'o',
  'ùúûüū': 'u',
  'ýÿ': 'y',
  'śš': 's',
  'źżž': 'z',
  'ł': 'l',
  'ß': 'ss',
  'æ': 'ae',
  'œ': 'oe',
};

final Map<int, String> _table = {
  for (final e in _map.entries)
    for (final c in e.key.runes) c: e.value,
};

/// Lower-cases rune by rune like Rust's `chars().flat_map(char::to_lowercase)`
/// (no context-sensitive rules such as the Greek final sigma, which
/// `String.toLowerCase` applies on the web).
Iterable<int> _lowerRunes(String s) sync* {
  for (final r in s.runes) {
    if (r < 0x80) {
      yield r >= 0x41 && r <= 0x5A ? r + 0x20 : r;
    } else {
      yield* String.fromCharCode(r).toLowerCase().runes;
    }
  }
}

/// Rust `fold`: lower-cases, strips common Latin diacritics and collapses
/// whitespace, so "  Crème   Brûlée " becomes "creme brulee".
String foldText(String input) {
  final b = StringBuffer();
  for (final r in _lowerRunes(input)) {
    final rep = _table[r];
    if (rep != null) {
      b.write(rep);
    } else if (isRustWhitespace(r)) {
      b.write(' ');
    } else {
      b.writeCharCode(r);
    }
  }
  return collapseWs(b.toString());
}

/// The server's duplicate rule (`find_duplicate` in
/// server/crates/server/src/recipes.rs): same `source_url`, or the same name
/// after [foldText].
class DuplicateIndex {
  final Map<String, String> _byUrl = {};
  final Map<String, String> _byName = {};

  /// Records a recipe. The first recipe seen for a URL/name wins.
  void add(String id, {required String name, String? sourceUrl}) {
    if (sourceUrl != null) _byUrl.putIfAbsent(sourceUrl, () => id);
    _byName.putIfAbsent(foldText(name), () => id);
  }

  /// Id of the existing recipe [name]/[sourceUrl] duplicates, if any.
  /// Like the server, a matching URL takes precedence over a matching name.
  String? find({required String name, String? sourceUrl}) {
    if (sourceUrl != null) {
      final byUrl = _byUrl[sourceUrl];
      if (byUrl != null) return byUrl;
    }
    return _byName[foldText(name)];
  }
}
