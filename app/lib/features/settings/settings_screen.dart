import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:intl/intl.dart';
import 'package:material_ui/material_ui.dart';
import 'package:package_info_plus/package_info_plus.dart';

import '../../api/endpoints.dart';
import '../../app.dart';
import '../../data/app_mode.dart';
import '../../data/auth_controller.dart';
import '../../data/preferences.dart';
import '../../data/providers.dart';
import '../../data/sync_controller.dart';
import '../../models/user.dart';
import '../../util/units.dart';
import '../../widgets/snack.dart';
import '../../widgets/states.dart';
import '../cookbook/cookbook_screen.dart';
import '../recipes/recipe_actions.dart';
import 'mode_settings.dart';

final packageInfoProvider = FutureProvider<PackageInfo?>((ref) async {
  try {
    return await PackageInfo.fromPlatform();
  } catch (_) {
    return null;
  }
});

class SettingsScreen extends ConsumerWidget {
  const SettingsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final auth = ref.watch(authProvider);
    final user = auth.user;
    final sync = ref.watch(syncProvider);
    final pending = ref.watch(pendingOpsProvider).value ?? 0;
    final themeMode = ref.watch(themeModeProvider);
    final theme = Theme.of(context);
    final info = ref.watch(packageInfoProvider).value;
    final server = ref.watch(apiClientProvider).baseUrl;
    final standalone = ref.watch(appModeProvider) == AppMode.standalone;
    final units = ref.watch(unitsPreferenceProvider);

    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 48),
        children: [
          MaxWidth(
            maxWidth: 720,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                if (standalone)
                  const StandaloneCard()
                else
                  Card(
                    child: Padding(
                      padding: const EdgeInsets.all(16),
                      child: Row(
                        children: [
                          CircleAvatar(
                            radius: 28,
                            backgroundColor: theme.colorScheme.primary,
                            child: Text(
                              (user?.name.isNotEmpty ?? false) ? user!.name[0].toUpperCase() : '?',
                              style: theme.textTheme.headlineSmall?.copyWith(color: theme.colorScheme.onPrimary),
                            ),
                          ),
                          const SizedBox(width: 16),
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Text(user?.name ?? 'Signed in', style: theme.textTheme.titleLarge),
                                Text(
                                  [
                                    if (user != null) '@${user.username}',
                                    if (user?.email != null) user!.email!,
                                  ].join(' · '),
                                  style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                                ),
                                if (user != null)
                                  Padding(
                                    padding: const EdgeInsets.only(top: 6),
                                    child: Wrap(
                                      spacing: 6,
                                      children: [
                                        _Badge(user.isOwner ? 'Household owner' : 'Member'),
                                        if (user.isAdmin) const _Badge('Server admin'),
                                      ],
                                    ),
                                  ),
                              ],
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                if (!standalone) ...[
                  const SectionHeader('Account'),
                  _Group(
                    children: [
                      ListTile(
                        leading: const Icon(Icons.badge_outlined),
                        title: const Text('Edit profile'),
                        subtitle: const Text('Display name and email'),
                        onTap: user == null ? null : () => _editProfile(context, ref, user),
                      ),
                      ListTile(
                        leading: const Icon(Icons.password_rounded),
                        title: const Text('Change password'),
                        onTap: () => _changePassword(context, ref),
                      ),
                    ],
                  ),
                ],
                const SectionHeader('Preferences'),
                _Group(
                  children: [
                    ListTile(
                      leading: const Icon(Icons.straighten_rounded),
                      title: const Text('Units'),
                      subtitle: Padding(
                        padding: const EdgeInsets.only(top: 8),
                        child: SegmentedButton<UnitSystem>(
                          showSelectedIcon: false,
                          segments: [for (final u in UnitSystem.values) ButtonSegment(value: u, label: Text(u.label))],
                          selected: {units},
                          onSelectionChanged: standalone
                              ? (s) => ref.read(localPrefsProvider.notifier).setUnits(s.first.name)
                              : user == null
                              ? null
                              : (s) => _setUnits(ref, user, s.first),
                        ),
                      ),
                    ),
                    ListTile(
                      leading: const Icon(Icons.dark_mode_outlined),
                      title: const Text('Theme'),
                      subtitle: Padding(
                        padding: const EdgeInsets.only(top: 8),
                        child: SegmentedButton<ThemeMode>(
                          showSelectedIcon: false,
                          segments: const [
                            ButtonSegment(value: ThemeMode.system, label: Text('System')),
                            ButtonSegment(value: ThemeMode.light, label: Text('Light')),
                            ButtonSegment(value: ThemeMode.dark, label: Text('Dark')),
                          ],
                          selected: {themeMode},
                          onSelectionChanged: (s) => ref.read(themeModeProvider.notifier).set(s.first),
                        ),
                      ),
                    ),
                    if (standalone)
                      ListTile(
                        leading: const Icon(Icons.auto_awesome_outlined),
                        title: const Text('AI assistant'),
                        subtitle: Text(
                          ref.watch(localPrefsProvider).llm == null
                              ? 'Off · reads recipe photos and messy text'
                              : 'On · ${ref.watch(localPrefsProvider).llm!.provider}',
                        ),
                        trailing: const Icon(Icons.chevron_right_rounded),
                        onTap: () => showLlmSettings(context, ref),
                      ),
                  ],
                ),
                const SectionHeader('Recipes'),
                _Group(
                  children: [
                    ListTile(
                      leading: const Icon(Icons.auto_fix_high_rounded),
                      title: const Text('Organize recipes'),
                      subtitle: const Text('Tidy ingredient text and fill in missing categories'),
                      trailing: const Icon(Icons.chevron_right_rounded),
                      onTap: () => context.push('/organize'),
                    ),
                  ],
                ),
                if (!standalone) ...[
                  const SectionHeader('Household'),
                  _Group(
                    children: [
                      ListTile(
                        leading: const Icon(Icons.home_outlined),
                        title: const Text('Household'),
                        subtitle: Text(user?.isOwner ?? false ? 'Members, roles and invites' : 'Members'),
                        trailing: const Icon(Icons.chevron_right_rounded),
                        onTap: () => context.go('/settings/household'),
                      ),
                      if (user?.isAdmin ?? false)
                        ListTile(
                          leading: const Icon(Icons.admin_panel_settings_outlined),
                          title: const Text('Server users'),
                          subtitle: const Text('Administer all accounts'),
                          trailing: const Icon(Icons.chevron_right_rounded),
                          onTap: () => context.go('/settings/admin'),
                        ),
                      if (user?.isAdmin ?? false)
                        ListTile(
                          leading: const Icon(Icons.auto_awesome_outlined),
                          title: const Text('AI assistant'),
                          subtitle: Text(
                            ref.watch(authProvider).config?.llmEnabled ?? false
                                ? 'On · reads recipe photos for everyone'
                                : 'Off · add an API key to read recipe photos',
                          ),
                          trailing: const Icon(Icons.chevron_right_rounded),
                          onTap: () => context.go('/settings/ai'),
                        ),
                    ],
                  ),
                ],
                const SectionHeader('Data'),
                _Group(
                  children: [
                    if (!standalone)
                      ListTile(
                        leading: sync.syncing
                            ? const SizedBox(width: 24, height: 24, child: CircularProgressIndicator(strokeWidth: 2))
                            : Icon(sync.online ? Icons.cloud_done_outlined : Icons.cloud_off_outlined),
                        title: const Text('Sync now'),
                        subtitle: Text(
                          [
                            sync.online ? 'Online' : 'Offline',
                            if (sync.lastSync != null) 'last synced ${DateFormat.jm().format(sync.lastSync!)}',
                            if (pending > 0) '$pending pending change${pending == 1 ? '' : 's'}',
                            if (sync.lastError != null) sync.lastError!,
                          ].join(' · '),
                        ),
                        onTap: () => ref.read(syncProvider.notifier).sync(),
                      ),
                    ListTile(
                      leading: const Icon(Icons.picture_as_pdf_outlined),
                      title: const Text('Export cookbook (PDF)'),
                      trailing: const Icon(Icons.chevron_right_rounded),
                      onTap: () => context.push('/cookbook'),
                    ),
                    ListTile(
                      leading: const Icon(Icons.archive_outlined),
                      title: const Text('Export all as JSON backup'),
                      onTap: () => exportJsonBackup(ref),
                    ),
                  ],
                ),
                if (!standalone) ...[
                  const SectionHeader('Server'),
                  _Group(
                    children: [
                      ListTile(
                        leading: const Icon(Icons.dns_outlined),
                        title: Text(server),
                        subtitle: Text(
                          auth.config == null
                              ? 'Larder server'
                              : 'Larder ${auth.config!.version} · ${auth.config!.database}',
                        ),
                        trailing: TextButton(onPressed: () => _changeServer(context, ref), child: const Text('Change')),
                      ),
                      if (!kIsWeb)
                        ListTile(
                          leading: const Icon(Icons.phone_android_rounded),
                          title: const Text('Disconnect and use on this device only'),
                          subtitle: const Text('Keeps your recipes and downloads their photos'),
                          onTap: () => showDisconnect(context, ref),
                        ),
                    ],
                  ),
                  const SizedBox(height: 16),
                  OutlinedButton.icon(
                    style: OutlinedButton.styleFrom(
                      foregroundColor: theme.colorScheme.error,
                      minimumSize: const Size.fromHeight(48),
                    ),
                    onPressed: () async {
                      final p = ref.read(pendingOpsProvider).value ?? 0;
                      if (await confirm(
                        context,
                        title: 'Sign out?',
                        message: p > 0
                            ? 'You have $p change${p == 1 ? '' : 's'} that have not synced yet. They will be lost.'
                            : 'Your offline copy of recipes on this device will be removed.',
                        action: 'Sign out',
                      )) {
                        await ref.read(authProvider.notifier).logout();
                      }
                    },
                    icon: const Icon(Icons.logout_rounded),
                    label: const Text('Sign out'),
                  ),
                ],
                const SectionHeader('About'),
                _Group(
                  children: [
                    ListTile(
                      leading: const Icon(Icons.info_outline_rounded),
                      title: const Text('Larder'),
                      subtitle: Text(
                        'App ${info?.version ?? ''}${info != null ? ' (${info.buildNumber})' : ''}'
                        '${auth.config != null ? ' · Server ${auth.config!.version}' : ''}',
                      ),
                    ),
                    ListTile(
                      leading: const Icon(Icons.gavel_outlined),
                      title: const Text('Open source licenses'),
                      onTap: () => showLicensePage(
                        context: context,
                        applicationName: 'Larder',
                        applicationVersion: info?.version,
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _setUnits(WidgetRef ref, User user, UnitSystem units) async {
    final auth = ref.read(authProvider.notifier);
    auth.setUser(user.copyWith(units: units.name));
    final updated = await runOnline(
      () => ref.read(apiClientProvider).updateMe({
        'preferences': {'units': units.name},
      }),
    );
    if (updated != null) auth.setUser(updated);
  }

  Future<void> _editProfile(BuildContext context, WidgetRef ref, User user) async {
    final result = await showDialog<(String, String)>(
      context: context,
      builder: (_) => _ProfileDialog(user: user),
    );
    if (result == null) return;
    final updated = await runOnline(
      () => ref.read(apiClientProvider).updateProfile(displayName: result.$1, email: result.$2),
    );
    if (updated != null) {
      ref.read(authProvider.notifier).setUser(updated);
      showSnack('Profile updated');
    }
  }

  Future<void> _changePassword(BuildContext context, WidgetRef ref) async {
    final result = await showDialog<(String, String)>(context: context, builder: (_) => const _PasswordDialog());
    if (result == null) return;
    final updated = await runOnline(
      () => ref.read(apiClientProvider).changePassword(currentPassword: result.$1, newPassword: result.$2),
    );
    if (updated != null) showSnack('Password changed');
  }

  Future<void> _changeServer(BuildContext context, WidgetRef ref) async {
    if (!kIsWeb) {
      if (await confirm(
        context,
        title: 'Change server?',
        message: 'You will be signed out and the recipes saved on this device will be removed.',
        action: 'Change server',
      )) {
        await ref.read(authProvider.notifier).forgetServer();
      }
      return;
    }
    final url = await promptText(
      context,
      title: 'Server URL',
      label: 'e.g. https://recipes.example.com',
      initial: ref.read(apiClientProvider).baseUrl,
      confirm: 'Connect',
      keyboardType: TextInputType.url,
    );
    if (url == null || url.trim().isEmpty) return;
    try {
      await ref.read(authProvider.notifier).connect(url);
      showSnack('Connected to ${ref.read(apiClientProvider).baseUrl}');
    } catch (e) {
      showError(e);
    }
  }
}

class _Group extends StatelessWidget {
  const _Group({required this.children});
  final List<Widget> children;

  @override
  Widget build(BuildContext context) => Card(child: Column(children: children));
}

class _Badge extends StatelessWidget {
  const _Badge(this.label);
  final String label;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      decoration: BoxDecoration(color: cs.secondaryContainer, borderRadius: BorderRadius.circular(8)),
      child: Text(label, style: Theme.of(context).textTheme.labelSmall?.copyWith(color: cs.onSecondaryContainer)),
    );
  }
}

class _ProfileDialog extends StatefulWidget {
  const _ProfileDialog({required this.user});
  final User user;

  @override
  State<_ProfileDialog> createState() => _ProfileDialogState();
}

class _ProfileDialogState extends State<_ProfileDialog> {
  late final _name = TextEditingController(text: widget.user.displayName);
  late final _email = TextEditingController(text: widget.user.email ?? '');

  @override
  void dispose() {
    _name.dispose();
    _email.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: const Text('Edit profile'),
    content: SizedBox(
      width: 360,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(
            controller: _name,
            decoration: const InputDecoration(labelText: 'Display name'),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _email,
            keyboardType: TextInputType.emailAddress,
            decoration: const InputDecoration(labelText: 'Email'),
          ),
        ],
      ),
    ),
    actions: [
      TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancel')),
      FilledButton(
        onPressed: () => Navigator.pop(context, (_name.text.trim(), _email.text.trim())),
        child: const Text('Save'),
      ),
    ],
  );
}

class _PasswordDialog extends StatefulWidget {
  const _PasswordDialog();

  @override
  State<_PasswordDialog> createState() => _PasswordDialogState();
}

class _PasswordDialogState extends State<_PasswordDialog> {
  final _current = TextEditingController();
  final _new = TextEditingController();
  final _confirm = TextEditingController();
  String? _error;

  @override
  void dispose() {
    _current.dispose();
    _new.dispose();
    _confirm.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: const Text('Change password'),
    content: SizedBox(
      width: 360,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(
            controller: _current,
            obscureText: true,
            decoration: const InputDecoration(
              labelText: 'Current password',
              helperText: 'Leave empty if you only sign in with SSO',
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _new,
            obscureText: true,
            decoration: const InputDecoration(labelText: 'New password'),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _confirm,
            obscureText: true,
            decoration: InputDecoration(labelText: 'Confirm new password', errorText: _error),
          ),
        ],
      ),
    ),
    actions: [
      TextButton(onPressed: () => Navigator.pop(context), child: const Text('Cancel')),
      FilledButton(
        onPressed: () {
          if (_new.text.length < 8) return setState(() => _error = 'At least 8 characters');
          if (_new.text != _confirm.text) return setState(() => _error = 'Passwords do not match');
          Navigator.pop(context, (_current.text, _new.text));
        },
        child: const Text('Change'),
      ),
    ],
  );
}
