import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/app_mode.dart';
import '../../data/auth_controller.dart';
import '../../data/sync_controller.dart';
import '../../widgets/snack.dart';
import 'offline_banner.dart';

class _Dest {
  const _Dest(this.label, this.icon, this.selectedIcon);
  final String label;
  final IconData icon;
  final IconData selectedIcon;
}

const _destinations = [
  _Dest('Recipes', Icons.menu_book_outlined, Icons.menu_book_rounded),
  _Dest('Meal Plan', Icons.calendar_month_outlined, Icons.calendar_month_rounded),
  _Dest('Shopping', Icons.shopping_basket_outlined, Icons.shopping_basket_rounded),
  _Dest('Import', Icons.download_outlined, Icons.download_rounded),
  _Dest('Settings', Icons.settings_outlined, Icons.settings_rounded),
];

/// Adaptive scaffold: NavigationBar on phones, NavigationRail on wide screens.
/// Also owns app-wide sync triggers (start, resume, notices).
class AppShell extends ConsumerStatefulWidget {
  const AppShell({super.key, required this.shell});
  final StatefulNavigationShell shell;

  @override
  ConsumerState<AppShell> createState() => _AppShellState();
}

class _AppShellState extends ConsumerState<AppShell> {
  AppLifecycleListener? _lifecycle;
  StreamSubscription<String>? _messages;

  @override
  void initState() {
    super.initState();
    final sync = ref.read(syncProvider.notifier);
    _messages = sync.messages.listen((m) => showSnack(m));
    _lifecycle = AppLifecycleListener(onResume: () => unawaited(sync.sync()));
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (ref.read(appModeProvider) == AppMode.connected) {
        unawaited(sync.sync());
        unawaited(ref.read(authProvider.notifier).refresh());
      }
    });
  }

  @override
  void dispose() {
    _lifecycle?.dispose();
    _messages?.cancel();
    super.dispose();
  }

  void _go(int index) => widget.shell.goBranch(index, initialLocation: index == widget.shell.currentIndex);

  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.sizeOf(context).width;
    final wide = width >= 720;
    final body = OfflineBanner(child: widget.shell);
    if (!wide) {
      return Scaffold(
        body: body,
        bottomNavigationBar: NavigationBar(
          selectedIndex: widget.shell.currentIndex,
          onDestinationSelected: _go,
          labelBehavior: NavigationDestinationLabelBehavior.alwaysShow,
          destinations: [
            for (final d in _destinations)
              NavigationDestination(icon: Icon(d.icon), selectedIcon: Icon(d.selectedIcon), label: d.label),
          ],
        ),
      );
    }
    final extended = width >= 1180;
    return Scaffold(
      body: Row(
        children: [
          SafeArea(
            child: NavigationRail(
              extended: extended,
              selectedIndex: widget.shell.currentIndex,
              onDestinationSelected: _go,
              labelType: extended ? NavigationRailLabelType.none : NavigationRailLabelType.all,
              leading: Padding(
                padding: const EdgeInsets.symmetric(vertical: 12),
                child: _Brand(extended: extended),
              ),
              destinations: [
                for (final d in _destinations)
                  NavigationRailDestination(
                    icon: Icon(d.icon),
                    selectedIcon: Icon(d.selectedIcon),
                    label: Text(d.label),
                  ),
              ],
            ),
          ),
          const VerticalDivider(width: 1),
          Expanded(child: body),
        ],
      ),
    );
  }
}

class _Brand extends StatelessWidget {
  const _Brand({required this.extended});
  final bool extended;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    final logo = CircleAvatar(
      radius: 20,
      backgroundColor: cs.primary,
      child: Icon(Icons.soup_kitchen_rounded, color: cs.onPrimary),
    );
    if (!extended) return logo;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        logo,
        const SizedBox(width: 12),
        Text('Larder', style: Theme.of(context).textTheme.titleLarge),
      ],
    );
  }
}
