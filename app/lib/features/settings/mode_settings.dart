import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/app_mode.dart';
import '../../data/auth_controller.dart';
import '../../data/local_store.dart';
import '../../data/preferences.dart';
import '../../data/providers.dart';
import '../../data/sync_controller.dart';
import '../../standalone/standalone.dart';
import '../../widgets/snack.dart';

/// Settings card shown in standalone mode.
class StandaloneCard extends StatelessWidget {
  const StandaloneCard({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                CircleAvatar(
                  radius: 28,
                  backgroundColor: theme.colorScheme.primary,
                  child: Icon(Icons.phone_android_rounded, color: theme.colorScheme.onPrimary),
                ),
                const SizedBox(width: 16),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('On this device', style: theme.textTheme.titleLarge),
                      Text(
                        'Everything is stored on this phone. No account needed.',
                        style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                      ),
                    ],
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            FilledButton.tonalIcon(
              onPressed: () => context.go('/connect'),
              icon: const Icon(Icons.dns_outlined),
              label: const Text('Connect to a server…'),
            ),
          ],
        ),
      ),
    );
  }
}

/// AI assistant (LLM) settings for standalone imports.
Future<void> showLlmSettings(BuildContext context, WidgetRef ref) async {
  final current = ref.read(localPrefsProvider).llm;
  final result = await showDialog<(bool, LlmConfig?)>(
    context: context,
    builder: (_) => _LlmDialog(current: current),
  );
  if (result == null) return;
  ref.read(localPrefsProvider.notifier).setLlm(result.$1 ? result.$2 : null);
  showSnack(result.$1 ? 'AI assistant enabled' : 'AI assistant turned off');
}

class _LlmDialog extends StatefulWidget {
  const _LlmDialog({this.current});
  final LlmConfig? current;

  @override
  State<_LlmDialog> createState() => _LlmDialogState();
}

class _LlmDialogState extends State<_LlmDialog> {
  late String _provider = widget.current?.provider ?? 'anthropic';
  late final _key = TextEditingController(text: widget.current?.apiKey ?? '');
  late final _model = TextEditingController(text: widget.current?.model ?? '');
  late final _baseUrl = TextEditingController(text: widget.current?.baseUrl ?? '');

  @override
  void dispose() {
    _key.dispose();
    _model.dispose();
    _baseUrl.dispose();
    super.dispose();
  }

  String? _opt(TextEditingController c) => c.text.trim().isEmpty ? null : c.text.trim();

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: const Text('AI assistant'),
    content: SizedBox(
      width: 400,
      child: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Text('Used to read recipe photos and messy text. Your API key is stored only on this device.'),
            const SizedBox(height: 12),
            DropdownButtonFormField<String>(
              initialValue: _provider,
              decoration: const InputDecoration(labelText: 'Provider'),
              items: const [
                DropdownMenuItem(value: 'anthropic', child: Text('Anthropic')),
                DropdownMenuItem(value: 'openai', child: Text('OpenAI-compatible')),
                DropdownMenuItem(value: 'ollama', child: Text('Ollama (self-hosted)')),
              ],
              onChanged: (v) => setState(() => _provider = v ?? _provider),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _key,
              obscureText: true,
              decoration: const InputDecoration(labelText: 'API key'),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _model,
              decoration: const InputDecoration(labelText: 'Model (optional)'),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _baseUrl,
              keyboardType: TextInputType.url,
              decoration: const InputDecoration(labelText: 'Base URL (optional)'),
            ),
          ],
        ),
      ),
    ),
    actions: [
      if (widget.current != null)
        TextButton(onPressed: () => Navigator.pop(context, (false, null)), child: const Text('Turn off')),
      TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancel')),
      FilledButton(
        onPressed: () => Navigator.pop(context, (
          true,
          LlmConfig(provider: _provider, apiKey: _opt(_key), model: _opt(_model), baseUrl: _opt(_baseUrl)),
        )),
        child: const Text('Save'),
      ),
    ],
  );
}

/// Connected -> standalone (Android): keep the data, download photos.
Future<void> showDisconnect(BuildContext context, WidgetRef ref) async {
  final support = ref.read(standaloneSupportProvider);
  final problem = support == null ? 'Not supported here.' : await support.checkEngine();
  if (problem != null) {
    showSnack('On-device mode is unavailable: $problem', error: true);
    return;
  }
  if (!context.mounted) return;
  await showDialog<void>(context: context, barrierDismissible: false, builder: (_) => const _DisconnectDialog());
}

class _DisconnectDialog extends ConsumerStatefulWidget {
  const _DisconnectDialog();

  @override
  ConsumerState<_DisconnectDialog> createState() => _DisconnectDialogState();
}

class _DisconnectDialogState extends ConsumerState<_DisconnectDialog> {
  bool _running = false;
  int _done = 0;
  int _total = 0;
  String _status = '';
  int _pending = 0;

  Future<void> _run({bool discardPending = false}) async {
    setState(() {
      _running = true;
      _status = 'Saving pending changes';
    });
    final store = ref.read(localStoreProvider);
    await ref.read(syncProvider.notifier).flush();
    final pending = await store.opCount();
    if (pending > 0 && !discardPending) {
      setState(() {
        _running = false;
        _pending = pending;
      });
      return;
    }
    await store.clearOutbox();
    await ref.read(standaloneSupportProvider)!.downloadImages(store, ref.read(apiClientProvider), (d, t, m) {
      if (mounted) {
        setState(() {
          _done = d;
          _total = t;
          _status = m;
        });
      }
    });
    await store.setMeta(MetaKeys.syncCursor, null);
    // Switch mode first so the router does not bounce to the login screen.
    await ref.read(appModeProvider.notifier).set(AppMode.standalone);
    await ref.read(authProvider.notifier).logout(keepData: true);
    if (mounted) Navigator.pop(context);
    showSnack('Disconnected. Your recipes now live on this device.');
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: const Text('Use on this device only?'),
    content: SizedBox(
      width: 380,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const Text(
            'Your recipes, meal plan and shopping lists stay on this phone and photos are downloaded for '
            'offline use. Changes will no longer sync with the server or your household.',
          ),
          if (_pending > 0) ...[
            const SizedBox(height: 12),
            Text(
              '$_pending change${_pending == 1 ? '' : 's'} could not be sent to the server.',
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          ],
          if (_running) ...[
            const SizedBox(height: 16),
            Text(_total > 0 ? '$_status · $_done / $_total' : _status),
            const SizedBox(height: 6),
            LinearProgressIndicator(value: _total > 0 ? _done / _total : null),
          ],
        ],
      ),
    ),
    actions: _running
        ? const []
        : [
            TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancel')),
            if (_pending > 0)
              TextButton(onPressed: () => _run(discardPending: true), child: const Text('Discard them and continue'))
            else
              FilledButton(onPressed: _run, child: const Text('Disconnect')),
          ],
  );
}
