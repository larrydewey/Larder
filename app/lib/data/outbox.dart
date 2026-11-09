import 'dart:async';

import '../api/api_client.dart';
import '../api/api_exception.dart';
import '../models/json.dart';
import '../models/meal_plan.dart';
import '../models/recipe.dart';
import '../models/shopping.dart';
import 'local_store.dart';
import 'outbox_op.dart';

/// Result of one outbox flush.
class FlushResult {
  final Map<String, OpOutcome> outcomes = {};

  /// User-facing notices (conflicts, rejected changes).
  final List<String> messages = [];

  /// True when flushing stopped because the server was unreachable / failing.
  bool stoppedOffline = false;

  /// True when flushing stopped because the session expired.
  bool stoppedUnauthorized = false;

  /// True when a conflict happened on a resource we could not refetch
  /// individually, so a sync is needed.
  bool needsSync = false;

  int sent = 0;
}

/// Replays queued mutations in order.
class OutboxProcessor {
  OutboxProcessor(this.store, this.api, {this.maxServerErrorAttempts = 5});

  final LocalStore store;
  final ApiClient api;
  final int maxServerErrorAttempts;

  static final _recipePath = RegExp(r'^/api/recipes/([^/]+)(?:/.*)?$');

  Future<FlushResult> flush() async {
    final result = FlushResult();
    final ops = await store.ops();
    for (final op in ops) {
      try {
        final res = await api.sendJson(op.method, op.path, body: op.body);
        result.sent++;
        await applyResponse(op, res);
        await store.removeOp(op.key!);
        result.outcomes[op.id] = OpOutcome(OutcomeKind.applied, data: res);
      } on NetworkException {
        result.stoppedOffline = true;
        break;
      } on ApiException catch (e) {
        if (e.statusCode == 401) {
          result.stoppedUnauthorized = true;
          break;
        }
        if (e.statusCode == 404 && op.method == 'DELETE') {
          // Already gone on the server: that's what we wanted.
          await store.removeOp(op.key!);
          result.outcomes[op.id] = const OpOutcome(OutcomeKind.applied);
          continue;
        }
        if (e.statusCode == 409) {
          await store.removeOp(op.key!);
          final current = await _refetch(op, e, result);
          final msg = '${_name(op)} was changed by someone else; the server version was kept.';
          result.messages.add(msg);
          result.outcomes[op.id] = OpOutcome(OutcomeKind.conflict, data: current, message: msg);
          continue;
        }
        if (e.statusCode >= 500) {
          final attempts = op.attempts + 1;
          if (attempts < maxServerErrorAttempts) {
            await store.updateOp(op.withAttempts(attempts));
            result.stoppedOffline = true;
            break;
          }
        }
        // Other 4xx (or repeated 5xx): drop and surface.
        await store.removeOp(op.key!);
        final msg = '${_name(op)} failed: ${e.message}';
        result.messages.add(msg);
        result.needsSync = true;
        result.outcomes[op.id] = OpOutcome(OutcomeKind.rejected, message: e.message);
      }
    }
    return result;
  }

  String _name(OutboxOp op) => op.label.isNotEmpty ? op.label : '${op.method} ${op.path}';

  /// Server wins: re-fetch the conflicting resource into the local store.
  Future<Json?> _refetch(OutboxOp op, ApiException e, FlushResult result) async {
    final m = _recipePath.firstMatch(op.path);
    final current = e.body?['current'];
    if (current is Map) {
      final r = Recipe.fromJson(asJson(current));
      if (r.id.isNotEmpty) await store.putRecipe(r);
      return asJson(current);
    }
    if (m != null) {
      try {
        final fresh = asJson(await api.get('/api/recipes/${m.group(1)}'));
        await store.putRecipe(Recipe.fromJson(fresh));
        return fresh;
      } catch (_) {
        result.needsSync = true;
        return null;
      }
    }
    result.needsSync = true;
    return null;
  }

  /// Applies a successful response to the local store.
  Future<void> applyResponse(OutboxOp op, Object? res) async {
    if (res is! Map) return;
    final j = asJson(res);
    switch (op.apply) {
      case ApplyKind.recipe:
        final r = Recipe.fromJson(j);
        if (r.id.isNotEmpty) await store.putRecipe(r);
      case ApplyKind.mealPlan:
        final e = MealPlanEntry.fromJson(j);
        if (e.id.isNotEmpty) await store.putMealPlanEntry(e);
      case ApplyKind.shoppingList:
        final l = ShoppingList.fromJson(j);
        if (l.id.isNotEmpty) await store.putShoppingList(l);
      case ApplyKind.shoppingItem:
        final i = ShoppingItem.fromJson(j);
        if (i.id.isNotEmpty && i.listId.isNotEmpty) await store.putShoppingItem(i);
    }
  }
}
