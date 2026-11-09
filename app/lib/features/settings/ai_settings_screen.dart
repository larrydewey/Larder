import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../api/endpoints.dart';
import '../../data/auth_controller.dart';
import '../../data/providers.dart';
import '../../models/ai_settings.dart';
import '../../widgets/snack.dart';
import '../../widgets/states.dart';
import '../recipes/recipe_actions.dart';

final aiSettingsProvider = FutureProvider.autoDispose<AiSettings>((ref) => ref.watch(apiClientProvider).aiSettings());

const _providers = {'anthropic': 'Anthropic (Claude)', 'openai': 'OpenAI-compatible', 'ollama': 'Ollama (self-hosted)'};

String _providerLabel(String? p) => _providers[p] ?? p ?? '';

/// Server admins: the AI assistant that reads recipe photos and messy text for everyone on this server.
class AiSettingsScreen extends ConsumerWidget {
  const AiSettingsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final me = ref.watch(authProvider).user;
    if (!(me?.isAdmin ?? false)) {
      return Scaffold(
        appBar: AppBar(title: const Text('AI assistant')),
        body: const EmptyState(icon: Icons.lock_outline_rounded, title: 'Admins only'),
      );
    }
    final async = ref.watch(aiSettingsProvider);
    return Scaffold(
      appBar: AppBar(title: const Text('AI assistant')),
      body: async.when(
        loading: () => const LoadingView(),
        error: (e, _) => ErrorState(error: e, onRetry: () => ref.invalidate(aiSettingsProvider)),
        data: (s) => Align(
          alignment: Alignment.topCenter,
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 640),
            child: ListView(
              padding: const EdgeInsets.all(16),
              children: [
                const Text(
                  'Reads recipe photos and messy text for everyone on this server. Each photo or text you import '
                  'with AI is sent to the provider you choose, which bills your account per request.',
                ),
                const SizedBox(height: 16),
                _StatusCard(settings: s),
                const SizedBox(height: 16),
                if (s.fromEnvironment)
                  const Text(
                    'These settings come from the LARDER_LLM_* environment variables on the server, so they '
                    'can only be changed there.',
                  )
                else
                  _AiForm(key: ValueKey('${s.provider}|${s.model}|${s.baseUrl}|${s.keyHint}'), settings: s),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _StatusCard extends ConsumerStatefulWidget {
  const _StatusCard({required this.settings});
  final AiSettings settings;

  @override
  ConsumerState<_StatusCard> createState() => _StatusCardState();
}

class _StatusCardState extends ConsumerState<_StatusCard> {
  bool _testing = false;

  Future<void> _test() async {
    setState(() => _testing = true);
    final model = await runOnline(() => ref.read(apiClientProvider).testAiSettings());
    if (mounted) setState(() => _testing = false);
    if (model != null) showSnack('Working: $model answered');
  }

  @override
  Widget build(BuildContext context) {
    final s = widget.settings;
    final theme = Theme.of(context);
    final lines = <String>[
      if (s.enabled) ...[
        _providerLabel(s.provider),
        if (s.model != null) 'Model: ${s.model}',
        if (s.hasKey) 'Key ending in ${s.keyHint ?? '…'}',
      ] else if (s.keyUnreadable)
        'The saved key can no longer be read (the server\'s signing secret changed). Enter it again.'
      else
        'Not set up. Photo import and the AI option for pasted text are hidden until it is.',
    ];
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(
                  s.enabled ? Icons.check_circle_rounded : Icons.do_not_disturb_on_outlined,
                  color: s.enabled ? theme.colorScheme.primary : theme.colorScheme.outline,
                ),
                const SizedBox(width: 8),
                Text(s.enabled ? 'On' : 'Off', style: theme.textTheme.titleMedium),
                const Spacer(),
                if (s.enabled)
                  OutlinedButton.icon(
                    onPressed: _testing ? null : _test,
                    icon: _testing
                        ? const SizedBox.square(dimension: 16, child: CircularProgressIndicator(strokeWidth: 2))
                        : const Icon(Icons.bolt_rounded),
                    label: const Text('Test'),
                  ),
              ],
            ),
            const SizedBox(height: 8),
            for (final l in lines) Text(l),
          ],
        ),
      ),
    );
  }
}

class _AiForm extends ConsumerStatefulWidget {
  const _AiForm({super.key, required this.settings});
  final AiSettings settings;

  @override
  ConsumerState<_AiForm> createState() => _AiFormState();
}

class _AiFormState extends ConsumerState<_AiForm> {
  late String _provider = widget.settings.provider ?? 'anthropic';
  final _key = TextEditingController();
  late final _model = TextEditingController(text: _initialModel());
  late final _baseUrl = TextEditingController(text: _initialBaseUrl());
  bool _saving = false;

  AiSettings get s => widget.settings;
  AiProviderDefaults? get _defaults => s.defaults[_provider];

  // Show the saved model or address only when it differs from the default, so defaults stay current.
  String _initialModel() {
    final d = s.defaults[s.provider];
    return s.model != null && s.model != d?.model ? s.model! : '';
  }

  String _initialBaseUrl() {
    final d = s.defaults[s.provider];
    return s.baseUrl != null && s.baseUrl != d?.baseUrl ? s.baseUrl! : '';
  }

  /// The server keeps the saved key only for the same provider and address.
  bool get _keyKept => s.hasKey && s.provider == _provider && _baseUrl.text.trim() == _initialBaseUrl();

  @override
  void dispose() {
    _key.dispose();
    _model.dispose();
    _baseUrl.dispose();
    super.dispose();
  }

  Future<void> _done(String message) async {
    ref.invalidate(aiSettingsProvider);
    // Updates "AI enabled" in the app, which shows or hides the Photo import.
    await ref.read(authProvider.notifier).refresh();
    showSnack(message);
  }

  Future<void> _save() async {
    setState(() => _saving = true);
    final saved = await runOnline(
      () => ref
          .read(apiClientProvider)
          .saveAiSettings(provider: _provider, apiKey: _key.text, model: _model.text, baseUrl: _baseUrl.text),
    );
    if (mounted) setState(() => _saving = false);
    if (saved != null) {
      _key.clear();
      await _done('AI assistant saved. Use Test to check it.');
    }
  }

  Future<void> _turnOff() async {
    if (!await confirm(
      context,
      title: 'Turn off the AI assistant?',
      message: 'Photo import stops working for everyone on this server, and the saved key is deleted.',
      action: 'Turn off',
    )) {
      return;
    }
    final ok = await runOnline(() async {
      await ref.read(apiClientProvider).deleteAiSettings();
      return true;
    });
    if (ok == true) await _done('AI assistant turned off');
  }

  @override
  Widget build(BuildContext context) {
    final needsKey = _defaults?.needsKey ?? true;
    final keyHelp = _keyKept
        ? 'Saved key ends in ${s.keyHint ?? '…'}. Leave empty to keep it.'
        : switch (_provider) {
            'anthropic' => 'Create one at console.anthropic.com under API Keys.',
            'openai' => 'From your provider, for example platform.openai.com.',
            _ => 'Usually not needed for Ollama.',
          };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        DropdownButtonFormField<String>(
          initialValue: _provider,
          decoration: const InputDecoration(labelText: 'Provider'),
          items: [for (final e in _providers.entries) DropdownMenuItem(value: e.key, child: Text(e.value))],
          onChanged: (v) => setState(() => _provider = v ?? _provider),
        ),
        const SizedBox(height: 12),
        TextField(
          controller: _key,
          obscureText: true,
          autocorrect: false,
          enableSuggestions: false,
          keyboardType: TextInputType.visiblePassword,
          decoration: InputDecoration(
            labelText: needsKey ? 'API key' : 'API key (optional)',
            helperText: keyHelp,
            helperMaxLines: 2,
          ),
          onChanged: (_) => setState(() {}),
        ),
        const SizedBox(height: 12),
        TextField(
          controller: _model,
          autocorrect: false,
          decoration: InputDecoration(
            labelText: 'Model (optional)',
            hintText: _defaults?.model,
            helperText: 'Leave empty for the default${_defaults == null ? '' : ', ${_defaults!.model}'}.',
          ),
        ),
        const SizedBox(height: 12),
        TextField(
          controller: _baseUrl,
          autocorrect: false,
          keyboardType: TextInputType.url,
          decoration: InputDecoration(
            labelText: 'Base URL (optional)',
            hintText: _defaults?.baseUrl,
            helperText: 'Only for self-hosted or other compatible services.',
          ),
          onChanged: (_) => setState(() {}),
        ),
        const SizedBox(height: 20),
        Row(
          children: [
            if (s.source == 'server')
              TextButton(
                onPressed: _saving ? null : _turnOff,
                style: TextButton.styleFrom(foregroundColor: Theme.of(context).colorScheme.error),
                child: const Text('Turn off'),
              ),
            const Spacer(),
            FilledButton(onPressed: _saving ? null : _save, child: const Text('Save')),
          ],
        ),
      ],
    );
  }
}
