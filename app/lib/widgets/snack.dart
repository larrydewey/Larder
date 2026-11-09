import 'package:material_ui/material_ui.dart';

import '../api/api_exception.dart';
import '../data/outbox_op.dart';

final scaffoldMessengerKey = GlobalKey<ScaffoldMessengerState>();
final rootNavigatorKey = GlobalKey<NavigatorState>(debugLabel: 'root');

void showSnack(String message, {bool error = false, SnackBarAction? action}) {
  final m = scaffoldMessengerKey.currentState;
  if (m == null) return;
  m.hideCurrentSnackBar();
  m.showSnackBar(
    SnackBar(
      content: Text(message),
      action: action,
      backgroundColor: error ? Theme.of(m.context).colorScheme.error : null,
    ),
  );
}

void showError(Object e) => showSnack(errorMessage(e), error: true);

/// Shows feedback for a submitted change. Returns true if it was applied or
/// queued (i.e. not rejected).
bool reportOutcome(OpOutcome o, {String? success}) {
  switch (o.kind) {
    case OutcomeKind.applied:
      if (success != null) showSnack(success);
      return true;
    case OutcomeKind.queued:
      showSnack('Saved offline. It will sync when you are back online.');
      return true;
    case OutcomeKind.conflict:
      // The sync controller already surfaced a notice.
      return false;
    case OutcomeKind.rejected:
      showSnack(o.message ?? 'The server rejected the change', error: true);
      return false;
  }
}
