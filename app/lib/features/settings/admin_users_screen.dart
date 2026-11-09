import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../api/endpoints.dart';
import '../../data/auth_controller.dart';
import '../../data/providers.dart';
import '../../models/user.dart';
import '../../widgets/snack.dart';
import '../../widgets/states.dart';
import '../recipes/recipe_actions.dart';

final adminUsersProvider = FutureProvider.autoDispose<List<User>>((ref) => ref.watch(apiClientProvider).adminUsers());

class AdminUsersScreen extends ConsumerWidget {
  const AdminUsersScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final me = ref.watch(authProvider).user;
    if (!(me?.isAdmin ?? false)) {
      return Scaffold(
        appBar: AppBar(title: const Text('Server users')),
        body: const EmptyState(icon: Icons.lock_outline_rounded, title: 'Admins only'),
      );
    }
    final async = ref.watch(adminUsersProvider);
    final api = ref.read(apiClientProvider);

    Future<void> act(Future<Object?> Function() f) async {
      await runOnline(f);
      ref.invalidate(adminUsersProvider);
    }

    return Scaffold(
      appBar: AppBar(title: const Text('Server users')),
      body: async.when(
        loading: () => const LoadingView(),
        error: (e, _) => ErrorState(error: e, onRetry: () => ref.invalidate(adminUsersProvider)),
        data: (users) => RefreshIndicator(
          onRefresh: () => ref.refresh(adminUsersProvider.future),
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 48),
            children: [
              MaxWidth(
                maxWidth: 720,
                child: Card(
                  child: Column(
                    children: [
                      for (final u in users)
                        ListTile(
                          leading: CircleAvatar(
                            backgroundColor: u.disabled ? Theme.of(context).colorScheme.surfaceContainerHighest : null,
                            child: Text(u.name.isEmpty ? '?' : u.name[0].toUpperCase()),
                          ),
                          title: Text(u.name + (u.id == me!.id ? ' (you)' : '')),
                          subtitle: Text(
                            [
                              '@${u.username}',
                              if (u.email != null) u.email!,
                              if (u.isAdmin) 'admin',
                              if (u.disabled) 'disabled',
                            ].join(' · '),
                          ),
                          trailing: u.id == me.id
                              ? null
                              : PopupMenuButton<String>(
                                  onSelected: (v) async {
                                    switch (v) {
                                      case 'admin':
                                        await act(() => api.adminUpdateUser(u.id, {'is_admin': !u.isAdmin}));
                                      case 'disable':
                                        await act(() => api.adminUpdateUser(u.id, {'disabled': !u.disabled}));
                                      case 'password':
                                        if (!context.mounted) return;
                                        final pw = await promptText(
                                          context,
                                          title: 'New password for ${u.username}',
                                          label: 'At least 8 characters',
                                          confirm: 'Set',
                                        );
                                        if (pw == null) return;
                                        if (pw.length < 8) return showSnack('Password too short', error: true);
                                        await act(() => api.adminUpdateUser(u.id, {'new_password': pw}));
                                        showSnack('Password updated');
                                      case 'delete':
                                        if (!context.mounted) return;
                                        if (await confirm(
                                          context,
                                          title: 'Delete ${u.username}?',
                                          message: 'This cannot be undone.',
                                        )) {
                                          await act(() => api.adminDeleteUser(u.id));
                                        }
                                    }
                                  },
                                  itemBuilder: (_) => [
                                    PopupMenuItem(
                                      value: 'admin',
                                      child: Text(u.isAdmin ? 'Remove admin' : 'Make admin'),
                                    ),
                                    PopupMenuItem(
                                      value: 'disable',
                                      child: Text(u.disabled ? 'Enable account' : 'Disable account'),
                                    ),
                                    const PopupMenuItem(value: 'password', child: Text('Reset password')),
                                    const PopupMenuItem(value: 'delete', child: Text('Delete user')),
                                  ],
                                ),
                        ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
