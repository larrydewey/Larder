import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../api/api_exception.dart';
import '../../data/auth_controller.dart';
import '../../data/providers.dart';
import '../../util/platform/platform.dart';
import '../auth/auth_layout.dart';
import 'welcome_screen.dart';

/// Asks for the server URL and validates it with GET /api/config.
class ServerSetupScreen extends ConsumerStatefulWidget {
  const ServerSetupScreen({super.key, this.migrate = false});

  /// Part of the connect-later flow from standalone mode.
  final bool migrate;

  @override
  ConsumerState<ServerSetupScreen> createState() => _ServerSetupScreenState();
}

class _ServerSetupScreenState extends ConsumerState<ServerSetupScreen> {
  late final TextEditingController _url;
  bool _busy = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final current = ref.read(apiClientProvider).baseUrl;
    _url = TextEditingController(text: current.isNotEmpty ? current : (kIsWeb ? pageOrigin() ?? '' : ''));
  }

  @override
  void dispose() {
    _url.dispose();
    super.dispose();
  }

  Future<void> _connect() async {
    final text = _url.text.trim();
    if (text.isEmpty) {
      setState(() => _error = 'Enter the address of your Larder server');
      return;
    }
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await ref.read(authProvider.notifier).connect(text);
      if (mounted) context.go(widget.migrate ? '/connect/login' : '/login');
    } catch (e) {
      setState(
        () => _error = e is NetworkException
            ? 'Could not reach a Larder server at that address.'
            : 'That does not look like a Larder server (${errorMessage(e)}).',
      );
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return AuthLayout(
      title: widget.migrate ? 'Connect to a server' : 'Connect to your kitchen',
      subtitle: widget.migrate
          ? 'Your recipes, meal plan and shopping lists will be copied to the server after you sign in.'
          : 'Enter the address of your self-hosted Larder server.',
      children: [
        TextField(
          controller: _url,
          keyboardType: TextInputType.url,
          autocorrect: false,
          autofocus: true,
          enabled: !_busy,
          decoration: InputDecoration(
            labelText: 'Server URL',
            hintText: 'http://192.168.1.10:8080',
            prefixIcon: const Icon(Icons.dns_outlined),
            errorText: _error,
            errorMaxLines: 3,
          ),
          onSubmitted: (_) => _connect(),
        ),
        const SizedBox(height: 8),
        Text(
          'Plain http:// works for servers on your home network.',
          style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant),
        ),
        const SizedBox(height: 24),
        FilledButton(
          onPressed: _busy ? null : _connect,
          child: _busy
              ? const SizedBox(height: 20, width: 20, child: CircularProgressIndicator(strokeWidth: 2))
              : const Text('Connect'),
        ),
        if (widget.migrate)
          TextButton(onPressed: _busy ? null : () => context.go('/settings'), child: const Text('Cancel'))
        else if (!kIsWeb)
          TextButton(
            onPressed: _busy ? null : () => enableStandalone(context, ref),
            child: const Text('Use on this device instead'),
          ),
      ],
    );
  }
}
