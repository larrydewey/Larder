import 'package:intl/intl.dart';

import '../../models/recipe.dart';

/// `not made yet` or `tried 2× · last Nov 12` (the trial bar and list).
String trialStatusLine(Recipe r) {
  if (r.triedCount <= 0) return 'not made yet';
  final last = r.lastTriedAt;
  final tried = 'tried ${r.triedCount}×';
  return last == null ? tried : '$tried · last ${DateFormat.MMMd().format(last.toLocal())}';
}

/// `added 4 months ago` and friends, for recipes not made yet.
String addedAgo(DateTime? created, {DateTime? now}) {
  if (created == null) return 'not made yet';
  final n = now ?? DateTime.now();
  final c = created.toLocal();
  final days = DateTime(n.year, n.month, n.day).difference(DateTime(c.year, c.month, c.day)).inDays;
  String ago(int count, String unit) => 'added $count $unit${count == 1 ? '' : 's'} ago';
  if (days <= 0) return 'added today';
  if (days == 1) return 'added yesterday';
  if (days < 14) return ago(days, 'day');
  if (days < 60) return ago(days ~/ 7, 'week');
  if (days < 365) return ago(days ~/ 30, 'month');
  return ago(days ~/ 365, 'year');
}
