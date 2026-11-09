import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../api/api_exception.dart';
import '../../data/auth_controller.dart';
import '../../data/providers.dart';
import 'auth_layout.dart';
import 'oidc.dart';

/// Sign in / register. Shows "Create admin account" on a fresh server.
class LoginScreen extends ConsumerStatefulWidget {
  const LoginScreen({super.key, this.migrate = false});

  /// Connect-later flow: continue to the migration screen after sign-in.
  final bool migrate;

  @override
  ConsumerState<LoginScreen> createState() => _LoginScreenState();
}

class _LoginScreenState extends ConsumerState<LoginScreen> {
  final _formKey = GlobalKey<FormState>();
  final _username = TextEditingController();
  final _password = TextEditingController();
  final _displayName = TextEditingController();
  final _email = TextEditingController();
  final _invite = TextEditingController();
  bool _register = false;
  bool _busy = false;
  bool _obscure = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => ref.read(authProvider.notifier).refresh());
  }

  @override
  void dispose() {
    for (final c in [_username, _password, _displayName, _email, _invite]) {
      c.dispose();
    }
    super.dispose();
  }

  Future<void> _submit(bool register) async {
    if (!_formKey.currentState!.validate()) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    final auth = ref.read(authProvider.notifier);
    try {
      if (register) {
        await auth.register(
          username: _username.text,
          password: _password.text,
          displayName: _displayName.text,
          email: _email.text,
          inviteCode: _invite.text,
        );
      } else {
        await auth.login(_username.text, _password.text);
      }
      if (widget.migrate && mounted) context.go('/connect/migrate');
    } catch (e) {
      setState(
        () => _error = e is ApiException && e.statusCode == 401
            ? 'Wrong username or password.'
            : e is ApiException && e.statusCode == 403 && register
            ? 'Registration is closed. Ask the household owner for an invite code.'
            : errorMessage(e),
      );
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _oidc() async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await startOidcLogin(ref, inviteCode: _invite.text);
      if (widget.migrate && mounted && ref.read(authProvider).status == AuthStatus.signedIn) {
        context.go('/connect/migrate');
      }
    } catch (e) {
      if (mounted) setState(() => _error = errorMessage(e));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authProvider);
    final cfg = auth.config;
    final setup = cfg?.setupRequired ?? false;
    final register = setup || _register;
    final theme = Theme.of(context);
    final server = ref.watch(apiClientProvider).baseUrl;

    return AuthLayout(
      title: setup ? 'Create admin account' : (register ? 'Create an account' : 'Welcome back'),
      subtitle: setup
          ? 'This server has no users yet. The first account becomes the server administrator.'
          : register
          ? 'Join a household with an invite code${(cfg?.registrationOpen ?? false) ? ', or start your own' : ''}.'
          : 'Sign in to your recipe collection.',
      children: [
        Form(
          key: _formKey,
          child: AutofillGroup(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                TextFormField(
                  key: const Key('username'),
                  controller: _username,
                  enabled: !_busy,
                  autofillHints: const [AutofillHints.username],
                  autocorrect: false,
                  decoration: InputDecoration(
                    labelText: register ? 'Username' : 'Username or email',
                    prefixIcon: const Icon(Icons.person_outline),
                  ),
                  validator: (v) => (v == null || v.trim().isEmpty) ? 'Required' : null,
                ),
                const SizedBox(height: 12),
                TextFormField(
                  key: const Key('password'),
                  controller: _password,
                  enabled: !_busy,
                  obscureText: _obscure,
                  autofillHints: [register ? AutofillHints.newPassword : AutofillHints.password],
                  decoration: InputDecoration(
                    labelText: 'Password',
                    prefixIcon: const Icon(Icons.lock_outline),
                    suffixIcon: IconButton(
                      tooltip: _obscure ? 'Show password' : 'Hide password',
                      icon: Icon(_obscure ? Icons.visibility_outlined : Icons.visibility_off_outlined),
                      onPressed: () => setState(() => _obscure = !_obscure),
                    ),
                  ),
                  validator: (v) {
                    if (v == null || v.isEmpty) return 'Required';
                    if (register && v.length < 8) return 'At least 8 characters';
                    return null;
                  },
                  onFieldSubmitted: (_) => _submit(register),
                ),
                if (register) ...[
                  const SizedBox(height: 12),
                  TextFormField(
                    controller: _displayName,
                    enabled: !_busy,
                    autofillHints: const [AutofillHints.name],
                    decoration: const InputDecoration(
                      labelText: 'Display name',
                      prefixIcon: Icon(Icons.badge_outlined),
                    ),
                  ),
                  const SizedBox(height: 12),
                  TextFormField(
                    controller: _email,
                    enabled: !_busy,
                    keyboardType: TextInputType.emailAddress,
                    autofillHints: const [AutofillHints.email],
                    decoration: const InputDecoration(
                      labelText: 'Email (optional)',
                      prefixIcon: Icon(Icons.mail_outline),
                    ),
                  ),
                  if (!setup) ...[
                    const SizedBox(height: 12),
                    TextFormField(
                      controller: _invite,
                      enabled: !_busy,
                      autocorrect: false,
                      decoration: const InputDecoration(
                        labelText: 'Invite code',
                        prefixIcon: Icon(Icons.confirmation_number_outlined),
                      ),
                    ),
                  ],
                ],
              ],
            ),
          ),
        ),
        if (_error != null) ...[
          const SizedBox(height: 16),
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(color: theme.colorScheme.errorContainer, borderRadius: BorderRadius.circular(12)),
            child: Text(_error!, style: TextStyle(color: theme.colorScheme.onErrorContainer)),
          ),
        ],
        const SizedBox(height: 20),
        FilledButton(
          key: const Key('submit'),
          onPressed: _busy ? null : () => _submit(register),
          child: _busy
              ? const SizedBox(height: 20, width: 20, child: CircularProgressIndicator(strokeWidth: 2))
              : Text(setup ? 'Create admin account' : (register ? 'Create account' : 'Sign in')),
        ),
        if ((cfg?.oidcEnabled ?? false) && !setup) ...[
          const SizedBox(height: 12),
          OutlinedButton.icon(
            onPressed: _busy ? null : _oidc,
            icon: const Icon(Icons.key_rounded),
            label: Text('Sign in with ${cfg!.oidcName}'),
          ),
        ],
        if (!setup) ...[
          const SizedBox(height: 8),
          TextButton(
            onPressed: _busy ? null : () => setState(() => _register = !_register),
            child: Text(register ? 'I already have an account' : 'Create an account'),
          ),
        ],
        const Divider(height: 32),
        Row(
          children: [
            Icon(Icons.dns_outlined, size: 16, color: theme.colorScheme.onSurfaceVariant),
            const SizedBox(width: 6),
            Expanded(
              child: Text(
                '$server${cfg != null && cfg.version.isNotEmpty ? '  ·  v${cfg.version}' : ''}',
                style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                overflow: TextOverflow.ellipsis,
              ),
            ),
            TextButton(
              onPressed: _busy ? null : () => context.go(widget.migrate ? '/connect' : '/setup-server'),
              child: Text(kIsWeb ? 'Use another server' : 'Change'),
            ),
          ],
        ),
      ],
    );
  }
}
