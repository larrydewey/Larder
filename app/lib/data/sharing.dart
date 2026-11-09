import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/endpoints.dart';
import '../models/share.dart';
import 'app_mode.dart';
import 'auth_controller.dart';
import 'providers.dart';

/// Whether recipes can be published at public share links: connected to a
/// server that allows it.
final publicSharingProvider = Provider<bool>((ref) {
  if (ref.watch(appModeProvider) != AppMode.connected) return false;
  return ref.watch(authProvider).config?.publicSharing ?? false;
});

/// The recipe's current public share link, or null if it has none (or
/// public sharing is unavailable). Refresh with `ref.invalidate` after
/// creating or removing a link.
final recipeShareProvider = FutureProvider.autoDispose.family<RecipeShare?, String>((ref, id) async {
  if (!ref.watch(publicSharingProvider)) return null;
  return ref.watch(apiClientProvider).recipeShare(id);
});
