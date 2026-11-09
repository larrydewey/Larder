import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/app_mode.dart';
import '../../data/providers.dart';
import '../../data/sync_controller.dart';

/// Thin banner shown while offline and/or while changes are waiting to sync.
class OfflineBanner extends ConsumerWidget {
  const OfflineBanner({super.key, required this.child});

  /// Content below the banner; its top safe-area padding is removed while
  /// the banner (which pads itself) is visible.
  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final status = ref.watch(syncProvider);
    final pending = ref.watch(pendingOpsProvider).value ?? 0;
    final standalone = ref.watch(appModeProvider) == AppMode.standalone;
    final show = !standalone && (!status.online || pending > 0);
    final cs = Theme.of(context).colorScheme;
    final text = !status.online
        ? (pending > 0
              ? 'Offline · $pending change${pending == 1 ? '' : 's'} waiting to sync'
              : 'Offline · showing saved recipes')
        : 'Syncing $pending change${pending == 1 ? '' : 's'}…';
    return Column(
      children: [
        if (!show)
          const SizedBox.shrink()
        else
          Material(
            color: status.online ? cs.secondaryContainer : cs.tertiaryContainer,
            child: SafeArea(
              bottom: false,
              child: InkWell(
                onTap: () => ref.read(syncProvider.notifier).sync(),
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
                  child: Row(
                    children: [
                      Icon(
                        status.online ? Icons.sync_rounded : Icons.cloud_off_rounded,
                        size: 18,
                        color: status.online ? cs.onSecondaryContainer : cs.onTertiaryContainer,
                      ),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          text,
                          style: Theme.of(context).textTheme.labelMedium
                              ?.copyWith(color: status.online ? cs.onSecondaryContainer : cs.onTertiaryContainer),
                        ),
                      ),
                      Text(
                        'Retry',
                        style: Theme.of(context).textTheme.labelMedium?.copyWith(
                          color: status.online ? cs.onSecondaryContainer : cs.onTertiaryContainer,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        Expanded(
          child: MediaQuery.removePadding(context: context, removeTop: show, child: child),
        ),
      ],
    );
  }
}
