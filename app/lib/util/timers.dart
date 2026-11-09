/// Detects cooking durations ("bake for 25 minutes", "1 hour 30 min",
/// "10-12 minutes") in instruction text. Ranges use the upper bound.
library;

class DetectedTimer {
  const DetectedTimer({required this.duration, required this.label, required this.start, required this.end});
  final Duration duration;

  /// The matched text, e.g. "25 minutes".
  final String label;
  final int start;
  final int end;

  @override
  String toString() => 'DetectedTimer($label, $duration)';
}

const _words = <String, double>{
  'a': 1,
  'an': 1,
  'one': 1,
  'two': 2,
  'three': 3,
  'four': 4,
  'five': 5,
  'six': 6,
  'seven': 7,
  'eight': 8,
  'nine': 9,
  'ten': 10,
  'eleven': 11,
  'twelve': 12,
  'fifteen': 15,
  'twenty': 20,
  'thirty': 30,
  'forty': 40,
  'forty-five': 45,
  'fifty': 50,
  'sixty': 60,
  'ninety': 90,
  'half': 0.5,
  'half an': 0.5,
  'half a': 0.5,
};

const _vulgar = {'½': 0.5, '¼': 0.25, '¾': 0.75, '⅓': 1 / 3, '⅔': 2 / 3};

const _num =
    r'(?:\d+(?:[.,]\d+)?(?:\s*[½¼¾⅓⅔]|\s+\d/\d)?|\d/\d|[½¼¾⅓⅔]|'
    r'half an?|forty-five|an?|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|fifteen|twenty|thirty|forty|fifty|sixty|ninety)';
const _range = '($_num)(?:\\s*(?:-|–|—|to|or)\\s*($_num))?';
const _unit = r'(hours?|hrs?|h|minutes?|mins?|m|seconds?|secs?)';

final _pattern = RegExp(
  '(?<![\\w½¼¾⅓⅔])$_range\\s*$_unit(?![a-z])'
  '(?:\\s*(?:and\\s+(?:a\\s+half\\b)?)?\\s*(?:($_num)\\s*(minutes?|mins?)(?![a-z]))?)?',
  caseSensitive: false,
);

double? _parseNum(String? raw) {
  if (raw == null) return null;
  final s = raw.trim().toLowerCase();
  if (_words.containsKey(s)) return _words[s];
  double total = 0;
  var rest = s;
  for (final e in _vulgar.entries) {
    if (rest.contains(e.key)) {
      total += e.value;
      rest = rest.replaceAll(e.key, ' ');
    }
  }
  for (final part in rest.trim().split(RegExp(r'\s+'))) {
    if (part.isEmpty) continue;
    if (part.contains('/')) {
      final nd = part.split('/');
      final n = double.tryParse(nd[0]);
      final d = double.tryParse(nd[1]);
      if (n == null || d == null || d == 0) return null;
      total += n / d;
    } else {
      final v = double.tryParse(part.replaceAll(',', '.'));
      if (v == null) return null;
      total += v;
    }
  }
  return total;
}

int _unitSeconds(String unit) {
  final u = unit.toLowerCase();
  if (u.startsWith('h')) return 3600;
  if (u.startsWith('s')) return 1;
  return 60;
}

/// Finds all durations in [text], in order of appearance.
List<DetectedTimer> detectTimers(String text) {
  final out = <DetectedTimer>[];
  for (final m in _pattern.allMatches(text)) {
    final low = _parseNum(m.group(1));
    final high = _parseNum(m.group(2));
    final unit = m.group(3)!;
    // Single-letter "m" only counts when attached to the number ("5m").
    if (unit.toLowerCase() == 'm' || unit.toLowerCase() == 'h') {
      final between = text.substring(m.start, m.end);
      if (RegExp(r'\s[mh](?![a-z])', caseSensitive: false).hasMatch(between)) continue;
    }
    final value = high ?? low;
    if (value == null || value <= 0) continue;
    var seconds = value * _unitSeconds(unit);
    final whole = m.group(0)!;
    var label = whole.trimRight();
    var end = m.start + label.length;
    if (RegExp(r'and\s+a\s+half', caseSensitive: false).hasMatch(whole)) {
      seconds += 0.5 * _unitSeconds(unit);
    }
    // "In a second bowl…" is not a timer; nothing in a kitchen needs one this short.
    if (seconds < 10) continue;
    final extra = _parseNum(m.group(4));
    if (extra != null && m.group(5) != null && _unitSeconds(unit) == 3600) {
      seconds += extra * 60;
    } else if (m.group(4) != null) {
      // Trailing number belonged to something else; trim it from the label.
      final cut = whole.toLowerCase().lastIndexOf(m.group(4)!.toLowerCase());
      if (cut > 0) {
        label = whole.substring(0, cut).trimRight();
        if (label.toLowerCase().endsWith(' and')) label = label.substring(0, label.length - 4);
        end = m.start + label.length;
      }
    }
    // Strip a dangling "and" if nothing followed it.
    if (label.toLowerCase().endsWith(' and')) {
      label = label.substring(0, label.length - 4);
      end = m.start + label.length;
    }
    out.add(
      DetectedTimer(
        duration: Duration(seconds: seconds.round()),
        label: label,
        start: m.start,
        end: end,
      ),
    );
  }
  return out;
}

/// Formats a duration as `1:05:00` / `4:30`.
String formatClock(Duration d) {
  final neg = d.isNegative;
  final t = d.abs();
  final h = t.inHours;
  final m = t.inMinutes.remainder(60);
  final s = t.inSeconds.remainder(60);
  final mm = h > 0 ? m.toString().padLeft(2, '0') : m.toString();
  final body = h > 0 ? '$h:$mm:${s.toString().padLeft(2, '0')}' : '$mm:${s.toString().padLeft(2, '0')}';
  return neg ? '-$body' : body;
}

/// Formats minutes as `1 h 20 min`.
String formatMinutes(int minutes) {
  if (minutes < 60) return '$minutes min';
  final h = minutes ~/ 60;
  final m = minutes % 60;
  return m == 0 ? '$h h' : '$h h $m min';
}
