import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../api/api_exception.dart';
import '../../data/app_mode.dart';
import '../../data/auth_controller.dart';
import '../../data/local_store.dart';
import '../../data/migration.dart';
import '../../data/providers.dart';
import '../../data/sync_controller.dart';
import '../../standalone/standalone.dart';
import '../../widgets/snack.dart';
import '../auth/auth_layout.dart';

/// Connect-later: copies on-device data to the server, then switches to
/// connected mode. Resumable.
class MigrationScreen extends ConsumerStatefulWidget {
  const MigrationScreen({super.key});

  @override
  ConsumerState<MigrationScreen> createState() => _MigrationScreenState();
}

class _MigrationScreenState extends ConsumerState<MigrationScreen> {
  bool _skipExisting = true;
  bool _running = false;
  MigrationProgress? _progress;
  MigrationReport? _report;
  String? _error;
  bool _resuming = false;

  @override
  void initState() {
    super.initState();
    ref.read(localStoreProvider).getMeta<List>(MetaKeys.migratedIds).then((v) {
      if (mounted && v != null && v.isNotEmpty) setState(() => _resuming = true);
    });
  }

  Future<void> _start() async {
    final support = ref.read(standaloneSupportProvider);
    setState(() {
      _running = true;
      _error = null;
    });
    try {
      final report =
          await Migrator(
            store: ref.read(localStoreProvider),
            api: ref.read(apiClientProvider),
            readFile: (path) async => support?.readFile(path),
          ).run(
            skipExistingNames: _skipExisting,
            onProgress: (p) {
              if (mounted) setState(() => _progress = p);
            },
          );
      setState(() => _report = report);
    } on NetworkException {
      setState(() => _error = 'Lost the connection to the server. Your progress is saved; try again to resume.');
    } catch (e) {
      setState(() => _error = '${errorMessage(e)}\nYour progress is saved; try again to resume.');
    } finally {
      if (mounted) setState(() => _running = false);
    }
  }

  Future<void> _cancel() async {
    await ref.read(authProvider.notifier).logout(keepData: true);
    if (mounted) context.go('/settings');
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final report = _report;
    if (report != null) {
      return AuthLayout(
        title: 'All set!',
        subtitle: 'Your data is now on the server.',
        children: [
          Text(
            '${report.recipes} recipes, ${report.images} photos, ${report.mealPlan} meal plan entries, '
            '${report.lists} lists and ${report.items} items copied'
            '${report.skipped > 0 ? '; ${report.skipped} recipes already existed' : ''}.',
          ),
          if (report.warnings.isNotEmpty)
            ExpansionTile(
              tilePadding: EdgeInsets.zero,
              title: Text('${report.warnings.length} items could not be copied'),
              children: [
                for (final w in report.warnings.take(50)) Align(alignment: Alignment.centerLeft, child: Text('• $w')),
              ],
            ),
          const SizedBox(height: 20),
          FilledButton(
            onPressed: () async {
              // Switch to connected; a full sync replaces the local copy
              // with the server's (which now includes everything).
              await ref.read(localStoreProvider).setMeta(MetaKeys.syncCursor, null);
              await ref.read(appModeProvider.notifier).set(AppMode.connected);
              unawaited(ref.read(syncProvider.notifier).sync(full: true));
              unawaited(ref.read(authProvider.notifier).refresh());
              showSnack('Connected to ${ref.read(apiClientProvider).baseUrl}');
            },
            child: const Text('Open my recipes'),
          ),
        ],
      );
    }
    final p = _progress;
    return AuthLayout(
      title: 'Copy your data to the server',
      subtitle:
          'Recipes (with photos), your meal plan and shopping lists will be uploaded. '
          'Afterwards this device stays in sync with the server.',
      children: [
        if (_resuming && !_running)
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: Text(
              'A previous copy was interrupted; it will continue where it stopped.',
              style: theme.textTheme.bodySmall,
            ),
          ),
        CheckboxListTile(
          contentPadding: EdgeInsets.zero,
          value: _skipExisting,
          onChanged: _running ? null : (v) => setState(() => _skipExisting = v ?? true),
          title: const Text('Skip recipes whose name already exists on the server'),
        ),
        if (p != null) ...[
          const SizedBox(height: 8),
          Text('${p.phase} · ${p.done} / ${p.total}', style: theme.textTheme.labelLarge),
          const SizedBox(height: 6),
          LinearProgressIndicator(value: p.fraction),
        ],
        if (_error != null) ...[
          const SizedBox(height: 12),
          Text(_error!, style: TextStyle(color: theme.colorScheme.error)),
        ],
        const SizedBox(height: 20),
        FilledButton(
          onPressed: _running ? null : _start,
          child: _running
              ? const SizedBox(width: 18, height: 18, child: CircularProgressIndicator(strokeWidth: 2))
              : Text(_error != null || _resuming ? 'Resume' : 'Start'),
        ),
        TextButton(onPressed: _running ? null : _cancel, child: const Text('Cancel and stay on this device')),
      ],
    );
  }
}
