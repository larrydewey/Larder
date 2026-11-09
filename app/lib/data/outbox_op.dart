import '../models/json.dart';

/// How a successful response for an op is applied to the local store.
abstract final class ApplyKind {
  static const none = 'none';
  static const recipe = 'recipe';
  static const mealPlan = 'meal_plan';
  static const shoppingList = 'shopping_list';
  static const shoppingItem = 'shopping_item';
}

/// A queued mutating request, persisted until the server accepted it.
class OutboxOp {
  OutboxOp({
    this.key,
    required this.id,
    required this.method,
    required this.path,
    this.body,
    DateTime? createdAt,
    this.apply = ApplyKind.none,
    this.label = '',
    this.attempts = 0,
  }) : createdAt = createdAt ?? DateTime.now().toUtc();

  /// Auto-increment store key (defines replay order).
  final int? key;

  /// Client generated id (used to match outcomes to submitters).
  final String id;
  final String method;
  final String path;
  final Object? body;
  final DateTime createdAt;
  final String apply;

  /// Human readable description for error messages ("Update Pancakes").
  final String label;
  final int attempts;

  factory OutboxOp.fromJson(Json j, {int? key}) => OutboxOp(
    key: key,
    id: asStr(j['id']),
    method: asStr(j['method'], 'POST'),
    path: asStr(j['path']),
    body: j['body'],
    createdAt: asDate(j['created_at']),
    apply: asStr(j['apply'], ApplyKind.none),
    label: asStr(j['label']),
    attempts: asInt(j['attempts']) ?? 0,
  );

  Json toJson() => {
    'id': id,
    'method': method,
    'path': path,
    'body': deepJson(body),
    'created_at': createdAt.toIso8601String(),
    'apply': apply,
    'label': label,
    'attempts': attempts,
  };

  OutboxOp withAttempts(int n) => OutboxOp(
    key: key,
    id: id,
    method: method,
    path: path,
    body: body,
    createdAt: createdAt,
    apply: apply,
    label: label,
    attempts: n,
  );
}

enum OutcomeKind { applied, queued, conflict, rejected }

class OpOutcome {
  const OpOutcome(this.kind, {this.data, this.message});
  final OutcomeKind kind;

  /// Response body (applied) or the server's current resource (conflict).
  final Object? data;
  final String? message;

  bool get ok => kind == OutcomeKind.applied || kind == OutcomeKind.queued;

  static const queued = OpOutcome(OutcomeKind.queued);
}
