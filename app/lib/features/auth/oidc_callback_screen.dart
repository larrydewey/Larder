import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../api/api_exception.dart';
import '../../data/auth_controller.dart';
import '../../util/platform/platform.dart';
import '../../widgets/states.dart';
import 'oidc.dart';

/// Web landing page for `/oidc-callback#access_token=...`.
class OidcCallbackScreen extends ConsumerStatefulWidget {
  const OidcCallbackScreen({super.key});

  @override
  ConsumerState<OidcCallbackScreen> createState() => _OidcCallbackScreenState();
}

class _OidcCallbackScreenState extends ConsumerState<OidcCallbackScreen> {
  String? _error;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _complete());
  }

  Future<void> _complete() async {
    final result = OidcResult.parse(currentUrlFragment());
    clearUrlFragment(); // don't leave tokens in the address bar / history
    if (!result.ok) {
      setState(() => _error = result.error ?? 'Sign-in failed');
      return;
    }
    try {
      await ref.read(authProvider.notifier).completeWithTokens(result.accessToken!, result.refreshToken!);
      if (mounted) context.go('/recipes');
    } catch (e) {
      setState(() => _error = errorMessage(e));
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: _error == null
          ? const Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [CircularProgressIndicator(), SizedBox(height: 16), Text('Signing you in…')],
              ),
            )
          : EmptyState(
              icon: Icons.lock_outline_rounded,
              title: 'Sign-in failed',
              message: _error,
              action: FilledButton(onPressed: () => context.go('/login'), child: const Text('Back to sign in')),
            ),
    );
  }
}
