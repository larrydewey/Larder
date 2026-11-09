import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';
import 'package:material_ui/material_ui.dart';

import '../../api/endpoints.dart';
import '../../data/auth_controller.dart';
import '../../data/providers.dart';
import '../../models/household.dart';
import '../../widgets/snack.dart';
import '../../widgets/states.dart';
import '../recipes/recipe_actions.dart';

final householdProvider = FutureProvider.autoDispose<Household>((ref) => ref.watch(apiClientProvider).household());
final invitesProvider = FutureProvider.autoDispose<List<Invite>>((ref) => ref.watch(apiClientProvider).invites());

class HouseholdScreen extends ConsumerWidget {
  const HouseholdScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final me = ref.watch(authProvider).user;
    final owner = me?.isOwner ?? false;
    final async = ref.watch(householdProvider);
    final theme = Theme.of(context);
    final api = ref.read(apiClientProvider);

    return Scaffold(
      appBar: AppBar(title: const Text('Household')),
      body: async.when(
        loading: () => const LoadingView(),
        error: (e, _) => ErrorState(error: e, onRetry: () => ref.invalidate(householdProvider)),
        data: (h) => RefreshIndicator(
          onRefresh: () async {
            ref.invalidate(householdProvider);
            ref.invalidate(invitesProvider);
          },
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 48),
            children: [
              MaxWidth(
                maxWidth: 720,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Card(
                      child: ListTile(
                        leading: const Icon(Icons.home_rounded),
                        title: Text(h.name, style: theme.textTheme.titleLarge),
                        subtitle: Text('${h.members.length} member${h.members.length == 1 ? '' : 's'}'),
                        trailing: owner
                            ? IconButton(
                                tooltip: 'Rename',
                                icon: const Icon(Icons.edit_outlined),
                                onPressed: () async {
                                  final name = await promptText(
                                    context,
                                    title: 'Household name',
                                    initial: h.name,
                                    confirm: 'Save',
                                  );
                                  if (name == null || name.trim().isEmpty) return;
                                  await runOnline(() => api.renameHousehold(name.trim()));
                                  ref.invalidate(householdProvider);
                                },
                              )
                            : null,
                      ),
                    ),
                    const SectionHeader('Members'),
                    Card(
                      child: Column(
                        children: [
                          for (final m in h.members)
                            ListTile(
                              leading: CircleAvatar(child: Text(m.name.isEmpty ? '?' : m.name[0].toUpperCase())),
                              title: Text(m.name + (m.id == me?.id ? ' (you)' : '')),
                              subtitle: Text('@${m.username} · ${m.role}'),
                              trailing: owner && m.id != me?.id
                                  ? PopupMenuButton<String>(
                                      onSelected: (v) async {
                                        if (v == 'delete') {
                                          if (!await confirm(
                                            context,
                                            title: 'Remove ${m.name}?',
                                            message: 'They will be moved to a new household of their own.',
                                            action: 'Remove',
                                          )) {
                                            return;
                                          }
                                          await runOnline(() => api.removeMember(m.id));
                                        } else {
                                          await runOnline(() => api.setMemberRole(m.id, v));
                                        }
                                        ref.invalidate(householdProvider);
                                      },
                                      itemBuilder: (_) => [
                                        if (m.role != 'owner')
                                          const PopupMenuItem(value: 'owner', child: Text('Make owner')),
                                        if (m.role != 'member')
                                          const PopupMenuItem(value: 'member', child: Text('Make member')),
                                        const PopupMenuItem(value: 'delete', child: Text('Remove from household')),
                                      ],
                                    )
                                  : null,
                            ),
                        ],
                      ),
                    ),
                    if (owner) const _Invites(),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Invites extends ConsumerWidget {
  const _Invites();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(invitesProvider);
    final api = ref.read(apiClientProvider);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SectionHeader(
          'Invites',
          trailing: FilledButton.tonalIcon(
            onPressed: () async {
              final days = await showDialog<int>(
                context: context,
                builder: (context) => SimpleDialog(
                  title: const Text('Invite expires in'),
                  children: [
                    for (final d in [1, 7, 30])
                      SimpleDialogOption(
                        onPressed: () => Navigator.pop(context, d),
                        child: Text('$d day${d == 1 ? '' : 's'}'),
                      ),
                  ],
                ),
              );
              if (days == null) return;
              final invite = await runOnline(() => api.createInvite(days));
              ref.invalidate(invitesProvider);
              if (invite != null) {
                await Clipboard.setData(ClipboardData(text: invite.code));
                showSnack('Invite code ${invite.code} copied to clipboard');
              }
            },
            icon: const Icon(Icons.person_add_alt_rounded),
            label: const Text('New invite'),
          ),
        ),
        Text(
          'Share an invite code so someone can register and join this household.',
          style: Theme.of(context).textTheme.bodySmall?.copyWith(color: Theme.of(context).colorScheme.onSurfaceVariant),
        ),
        const SizedBox(height: 8),
        async.when(
          loading: () => const LinearProgressIndicator(),
          error: (e, _) => Text('Could not load invites: $e'),
          data: (invites) => invites.isEmpty
              ? const Padding(padding: EdgeInsets.all(8), child: Text('No active invites'))
              : Card(
                  child: Column(
                    children: [
                      for (final i in invites)
                        ListTile(
                          leading: const Icon(Icons.confirmation_number_outlined),
                          title: SelectableText(i.code, style: const TextStyle(fontFamily: 'monospace', fontSize: 18)),
                          subtitle: i.expiresAt == null
                              ? null
                              : Text('Expires ${DateFormat.yMMMd().add_jm().format(i.expiresAt!.toLocal())}'),
                          trailing: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              IconButton(
                                tooltip: 'Copy',
                                icon: const Icon(Icons.copy_rounded),
                                onPressed: () async {
                                  await Clipboard.setData(ClipboardData(text: i.code));
                                  showSnack('Copied');
                                },
                              ),
                              IconButton(
                                tooltip: 'Revoke',
                                icon: const Icon(Icons.block_rounded),
                                onPressed: () async {
                                  await runOnline(() => api.revokeInvite(i.code));
                                  ref.invalidate(invitesProvider);
                                },
                              ),
                            ],
                          ),
                        ),
                    ],
                  ),
                ),
        ),
      ],
    );
  }
}
