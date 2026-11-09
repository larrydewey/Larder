import 'json.dart';

class ImportJob {
  const ImportJob({
    required this.id,
    this.source = '',
    this.status = 'pending',
    this.total = 0,
    this.imported = 0,
    this.skipped = 0,
    this.failed = 0,
    this.errors = const [],
    this.recipeIds = const [],
    this.createdAt,
    this.finishedAt,
  });

  final String id;
  final String source;
  final String status;
  final int total;
  final int imported;
  final int skipped;
  final int failed;
  final List<String> errors;
  final List<String> recipeIds;
  final DateTime? createdAt;
  final DateTime? finishedAt;

  bool get isFinished => status == 'done' || status == 'failed';
  int get processed => imported + skipped + failed;
  double? get progress => total > 0 ? (processed / total).clamp(0, 1).toDouble() : null;

  factory ImportJob.fromJson(Json j) => ImportJob(
    id: asStr(j['id']),
    source: asStr(j['source']),
    status: asStr(j['status'], 'pending'),
    total: asInt(j['total']) ?? 0,
    imported: asInt(j['imported']) ?? 0,
    skipped: asInt(j['skipped']) ?? 0,
    failed: asInt(j['failed']) ?? 0,
    errors: asStringList(j['errors']),
    recipeIds: asStringList(j['recipe_ids']),
    createdAt: asDate(j['created_at']),
    finishedAt: asDate(j['finished_at']),
  );

  Json toJson() => {
    'id': id,
    'source': source,
    'status': status,
    'total': total,
    'imported': imported,
    'skipped': skipped,
    'failed': failed,
    'errors': errors,
    'recipe_ids': recipeIds,
    'created_at': createdAt?.toUtc().toIso8601String(),
    'finished_at': finishedAt?.toUtc().toIso8601String(),
  };
}
