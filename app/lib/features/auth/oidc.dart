import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_web_auth_2/flutter_web_auth_2.dart';

import '../../data/auth_controller.dart';
import '../../data/providers.dart';
import '../../util/platform/platform.dart';

/// Tokens (or an error) parsed from an OIDC redirect fragment:
/// `access_token=...&refresh_token=...&expires_in=900` or `error=...`.
class OidcResult {
  const OidcResult({this.accessToken, this.refreshToken, this.error});
  final String? accessToken;
  final String? refreshToken;
  final String? error;

  bool get ok => accessToken != null && accessToken!.isNotEmpty && refreshToken != null && refreshToken!.isNotEmpty;

  factory OidcResult.parse(String fragment) {
    var f = fragment.startsWith('#') ? fragment.substring(1) : fragment;
    // Tolerate a hash-route prefix like "/oidc-callback?access_token=..."
    final q = f.indexOf('?');
    if (q >= 0 && !f.substring(0, q).contains('=')) f = f.substring(q + 1);
    Map<String, String> p;
    try {
      p = Uri.splitQueryString(f);
    } catch (_) {
      p = const {};
    }
    return OidcResult(
      accessToken: p['access_token'],
      refreshToken: p['refresh_token'],
      error: p['error'] ?? (p.isEmpty ? 'No login response received' : null),
    );
  }
}

/// Starts the OIDC flow. On web this navigates away (the app is reloaded at
/// /oidc-callback). On Android a browser tab is opened and the tokens come
/// back through the `larder://auth` redirect.
Future<void> startOidcLogin(WidgetRef ref, {String? inviteCode}) async {
  final api = ref.read(apiClientProvider);
  final invite = (inviteCode == null || inviteCode.trim().isEmpty) ? null : inviteCode.trim();
  if (kIsWeb) {
    final origin = pageOrigin() ?? Uri.base.origin;
    final url = api.uri('/api/auth/oidc/login', {'redirect_uri': '$origin/oidc-callback', 'invite_code': invite});
    navigateTo(url.toString());
    return;
  }
  final url = api.uri('/api/auth/oidc/login', {'redirect_uri': 'larder://auth', 'invite_code': invite});
  final result = await FlutterWebAuth2.authenticate(url: url.toString(), callbackUrlScheme: 'larder');
  final uri = Uri.parse(result);
  final parsed = OidcResult.parse(uri.fragment.isNotEmpty ? uri.fragment : uri.query);
  if (!parsed.ok) throw Exception(parsed.error ?? 'Sign-in failed');
  await ref.read(authProvider.notifier).completeWithTokens(parsed.accessToken!, parsed.refreshToken!);
}
