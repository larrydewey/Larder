import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/app_mode.dart';
import '../../standalone/standalone.dart';
import '../auth/auth_layout.dart';

/// Use the engine on this device after checking that it loads.
Future<bool> enableStandalone(BuildContext context, WidgetRef ref) async {
  final support = ref.read(standaloneSupportProvider);
  final problem = support == null ? 'Not supported on this platform.' : await support.checkEngine();
  if (problem != null) {
    if (context.mounted) {
      await showDialog<void>(
        context: context,
        builder: (context) => AlertDialog(
          title: const Text('On-device mode unavailable'),
          content: Text('This build cannot run without a server.\n\n$problem'),
          actions: [TextButton(onPressed: () => Navigator.pop(context), child: const Text('OK'))],
        ),
      );
    }
    return false;
  }
  await ref.read(appModeProvider.notifier).set(AppMode.standalone);
  return true;
}

/// First launch on Android: standalone or server.
class WelcomeScreen extends ConsumerStatefulWidget {
  const WelcomeScreen({super.key});

  @override
  ConsumerState<WelcomeScreen> createState() => _WelcomeScreenState();
}

class _WelcomeScreenState extends ConsumerState<WelcomeScreen> {
  bool _busy = false;

  @override
  Widget build(BuildContext context) {
    return AuthLayout(
      title: 'Welcome to your larder',
      subtitle: 'Keep recipes, plan meals and make shopping lists. You can always connect to a server later.',
      children: [
        _Choice(
          icon: Icons.phone_android_rounded,
          title: 'Use on this device',
          body: 'No account or server needed. Everything stays on this phone.',
          onTap: _busy
              ? null
              : () async {
                  setState(() => _busy = true);
                  await enableStandalone(context, ref);
                  if (mounted) setState(() => _busy = false);
                },
        ),
        const SizedBox(height: 12),
        _Choice(
          icon: Icons.dns_rounded,
          title: 'Connect to a server',
          body: 'Sync with your self-hosted Larder server and share with your household.',
          onTap: _busy ? null : () => ref.read(appModeProvider.notifier).set(AppMode.connected),
        ),
        if (_busy) const Padding(padding: EdgeInsets.only(top: 16), child: LinearProgressIndicator()),
      ],
    );
  }
}

class _Choice extends StatelessWidget {
  const _Choice({required this.icon, required this.title, required this.body, required this.onTap});
  final IconData icon;
  final String title;
  final String body;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Material(
      color: theme.colorScheme.surfaceContainerHigh,
      borderRadius: BorderRadius.circular(16),
      child: InkWell(
        borderRadius: BorderRadius.circular(16),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Row(
            children: [
              CircleAvatar(
                radius: 24,
                backgroundColor: theme.colorScheme.primaryContainer,
                child: Icon(icon, color: theme.colorScheme.onPrimaryContainer),
              ),
              const SizedBox(width: 16),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(title, style: theme.textTheme.titleMedium),
                    const SizedBox(height: 4),
                    Text(body, style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
                  ],
                ),
              ),
              const Icon(Icons.chevron_right_rounded),
            ],
          ),
        ),
      ),
    );
  }
}
