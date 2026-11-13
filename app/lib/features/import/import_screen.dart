import 'dart:async';

import 'package:file_picker/file_picker.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:image_picker/image_picker.dart';
import 'package:intl/intl.dart';
import 'package:material_ui/material_ui.dart';

import '../../api/api_exception.dart';
import '../../data/backend.dart';
import '../../data/preferences.dart';
import '../../models/import_job.dart';
import '../../models/recipe.dart';
import '../../widgets/snack.dart';
import '../../widgets/states.dart';

const importSources = {
  'auto': 'Detect automatically',
  'mealie': 'Mealie export / backup (.zip)',
  'tandoor': 'Tandoor export (.zip)',
  'paprika': 'Paprika (.paprikarecipes)',
  'nextcloud': 'Nextcloud Cookbook (.zip)',
  'recipekeeper': 'Recipe Keeper (.zip)',
  'larder': 'Larder backup (.zip)',
  'jsonld': 'schema.org JSON-LD (.json)',
};

final importJobsProvider = FutureProvider.autoDispose<List<ImportJob>>(
  (ref) => ref.watch(importServiceProvider).history(),
);

/// Single-recipe imports (website, text, photo) go to the trial list unless
/// this is switched off. Shared by those tabs; starts on every time the
/// import screen opens.
final _singleToTrialProvider = NotifierProvider.autoDispose<_SingleToTrial, bool>(_SingleToTrial.new);

class _SingleToTrial extends Notifier<bool> {
  @override
  bool build() => true;
  void set(bool v) => state = v;
}

/// The status chosen for single-recipe imports.
String _singleStatus(WidgetRef ref) => ref.read(_singleToTrialProvider) ? RecipeStatus.trial : RecipeStatus.cookbook;

/// The import screen's choice, for a single-recipe import.
class _SingleTrialSwitch extends ConsumerWidget {
  const _SingleTrialSwitch();

  @override
  Widget build(BuildContext context, WidgetRef ref) => SwitchListTile(
    key: const Key('import-to-trial'),
    contentPadding: EdgeInsets.zero,
    value: ref.watch(_singleToTrialProvider),
    onChanged: (v) => ref.read(_singleToTrialProvider.notifier).set(v),
    title: const Text('Add to Trial'),
    subtitle: const Text('Try it before it joins your cookbook'),
    secondary: const Icon(Icons.science_outlined),
  );
}

/// The import screen's choice for a bulk import: off keeps the default.
class _BulkTrialSwitch extends StatelessWidget {
  const _BulkTrialSwitch({required this.value, required this.onChanged});
  final bool value;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) => SwitchListTile(
    contentPadding: EdgeInsets.zero,
    value: value,
    onChanged: onChanged,
    title: const Text('Add to Trial'),
    subtitle: const Text('When off, recipes go to your cookbook (Larder backups keep each recipe\'s place)'),
  );
}

class ImportScreen extends ConsumerStatefulWidget {
  const ImportScreen({super.key, this.initialTab, this.trial = false});

  /// The tab to open on (`website`, `text`, `photo`, `file`, `mealie` or
  /// `history`); the first tab when null or unknown.
  final String? initialTab;

  /// Opened from the trial list: collection imports also start with
  /// "Add to Trial" on. Single recipes start on the trial list either way.
  final bool trial;

  @override
  ConsumerState<ImportScreen> createState() => _ImportScreenState();
}

class _ImportScreenState extends ConsumerState<ImportScreen> {
  ImportJob? _job;
  StreamSubscription<ImportJob>? _sub;

  @override
  void dispose() {
    _sub?.cancel();
    super.dispose();
  }

  /// Follows a bulk import's progress (server job polling, or on-device).
  void track(Stream<ImportJob> jobs) {
    _sub?.cancel();
    _sub = jobs.listen(
      (j) {
        if (!mounted) return;
        setState(() => _job = j);
        if (j.isFinished) {
          ref.invalidate(importJobsProvider);
          showSnack(j.status == 'done' ? 'Import finished: ${j.imported} recipes imported' : 'Import failed');
        }
      },
      onError: (Object e) {
        if (mounted) showError(e);
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    final llm = ref.watch(llmEnabledProvider);
    final tabs = <(String, IconData, Widget)>[
      ('Website', Icons.public_rounded, const _UrlTab()),
      ('Text', Icons.notes_rounded, _TextTab(llm: llm)),
      if (llm) ('Photo', Icons.photo_camera_outlined, const _PhotoTab()),
      ('File', Icons.upload_file_rounded, _FileTab(onJob: track, initialTrial: widget.trial)),
      ('Mealie', Icons.cloud_download_outlined, _MealieTab(onJob: track, initialTrial: widget.trial)),
      ('History', Icons.history_rounded, const _HistoryTab()),
    ];
    final initial = tabs.indexWhere((t) => t.$1.toLowerCase() == widget.initialTab);
    return DefaultTabController(
      length: tabs.length,
      initialIndex: initial < 0 ? 0 : initial,
      child: Scaffold(
        appBar: AppBar(
          title: const Text('Import'),
          bottom: TabBar(
            isScrollable: true,
            tabAlignment: TabAlignment.start,
            tabs: [for (final t in tabs) Tab(icon: Icon(t.$2), text: t.$1)],
          ),
        ),
        body: Column(
          children: [
            if (_job != null)
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
                child: MaxWidth(
                  child: ImportJobCard(
                    job: _job!,
                    onClose: _job!.isFinished ? () => setState(() => _job = null) : null,
                  ),
                ),
              ),
            Expanded(child: TabBarView(children: [for (final t in tabs) t.$3])),
          ],
        ),
      ),
    );
  }
}

/// Opens the editor with an unsaved import preview, as [status] (the
/// editor can still change it).
void openPreview(BuildContext context, ImportPreview preview, {required String status}) => context.push(
  status == RecipeStatus.trial ? '/trial/new' : '/recipes/new',
  extra: ImportPreview(preview.recipe.copyWith(status: status), imagePath: preview.imagePath),
);

/// Where a saved recipe's page lives.
String recipePath(Recipe r) => r.isTrial ? '/trial/${r.id}' : '/recipes/${r.id}';

class _TabBody extends StatelessWidget {
  const _TabBody({required this.children});
  final List<Widget> children;

  @override
  Widget build(BuildContext context) => ListView(
    padding: const EdgeInsets.fromLTRB(16, 20, 16, 48),
    children: [
      MaxWidth(
        maxWidth: 720,
        child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: children),
      ),
    ],
  );
}

class _Hint extends StatelessWidget {
  const _Hint(this.text);
  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: 16),
    child: Text(
      text,
      style: Theme.of(context).textTheme.bodyMedium?.copyWith(color: Theme.of(context).colorScheme.onSurfaceVariant),
    ),
  );
}

mixin _Busy<T extends ConsumerStatefulWidget> on ConsumerState<T> {
  bool busy = false;

  Future<void> run(Future<void> Function() f) async {
    setState(() => busy = true);
    try {
      await f();
    } on NetworkException {
      showSnack('Importing needs a connection to the server.', error: true);
    } catch (e) {
      showError(e);
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Widget spinnerOr(String label) =>
      busy ? const SizedBox(width: 18, height: 18, child: CircularProgressIndicator(strokeWidth: 2)) : Text(label);
}

class _UrlTab extends ConsumerStatefulWidget {
  const _UrlTab();

  @override
  ConsumerState<_UrlTab> createState() => _UrlTabState();
}

class _UrlTabState extends ConsumerState<_UrlTab> with _Busy {
  final _url = TextEditingController();

  @override
  void dispose() {
    _url.dispose();
    super.dispose();
  }

  void _go(bool save) {
    final url = _url.text.trim();
    if (url.isEmpty) return;
    final status = _singleStatus(ref);
    run(() async {
      final imports = ref.read(importServiceProvider);
      if (save) {
        final r = await imports.importUrl(url, status: status);
        if (mounted) context.go(recipePath(r));
      } else {
        final preview = await imports.previewUrl(url);
        if (mounted) openPreview(context, preview, status: status);
      }
    });
  }

  @override
  Widget build(BuildContext context) => _TabBody(
    children: [
      const _Hint(
        'Paste a link to a recipe page. Most sites publish structured recipe data that can be imported cleanly.',
      ),
      TextField(
        controller: _url,
        keyboardType: TextInputType.url,
        decoration: const InputDecoration(labelText: 'Recipe URL', prefixIcon: Icon(Icons.link_rounded)),
        onSubmitted: (_) => _go(false),
      ),
      const _SingleTrialSwitch(),
      const SizedBox(height: 8),
      Wrap(
        spacing: 12,
        runSpacing: 8,
        children: [
          FilledButton.icon(
            onPressed: busy ? null : () => _go(false),
            icon: const Icon(Icons.preview_outlined),
            label: spinnerOr('Preview & edit'),
          ),
          OutlinedButton(onPressed: busy ? null : () => _go(true), child: const Text('Import directly')),
        ],
      ),
    ],
  );
}

class _TextTab extends ConsumerStatefulWidget {
  const _TextTab({required this.llm});
  final bool llm;

  @override
  ConsumerState<_TextTab> createState() => _TextTabState();
}

class _TextTabState extends ConsumerState<_TextTab> with _Busy {
  final _text = TextEditingController();
  bool _useLlm = false;

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  void _go() {
    if (_text.text.trim().isEmpty) return;
    final status = _singleStatus(ref);
    run(() async {
      final preview = await ref.read(importServiceProvider).previewText(_text.text, useLlm: widget.llm && _useLlm);
      if (mounted) openPreview(context, preview, status: status);
    });
  }

  @override
  Widget build(BuildContext context) => _TabBody(
    children: [
      const _Hint('Paste a recipe. Headings like "Ingredients" and "Instructions" help the parser.'),
      TextField(
        controller: _text,
        minLines: 10,
        maxLines: 24,
        decoration: const InputDecoration(
          hintText: 'Grandma\'s Apple Pie\n\nIngredients\n…\n\nInstructions\n…',
          alignLabelWithHint: true,
        ),
      ),
      if (widget.llm)
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          value: _useLlm,
          onChanged: (v) => setState(() => _useLlm = v),
          title: const Text('Use AI to structure the recipe'),
          subtitle: const Text('Better results for messy text'),
          secondary: const Icon(Icons.auto_awesome_outlined),
        ),
      const _SingleTrialSwitch(),
      const SizedBox(height: 12),
      Align(
        alignment: Alignment.centerLeft,
        child: FilledButton.icon(
          onPressed: busy ? null : _go,
          icon: const Icon(Icons.preview_outlined),
          label: spinnerOr('Preview & edit'),
        ),
      ),
    ],
  );
}

class _PhotoTab extends ConsumerStatefulWidget {
  const _PhotoTab();

  @override
  ConsumerState<_PhotoTab> createState() => _PhotoTabState();
}

class _PhotoTabState extends ConsumerState<_PhotoTab> with _Busy {
  void _pick(ImageSource source) => run(() async {
    final status = _singleStatus(ref);
    final file = await ImagePicker().pickImage(source: source, maxWidth: 2400, maxHeight: 2400, imageQuality: 90);
    if (file == null) return;
    final bytes = await file.readAsBytes();
    final preview = await ref
        .read(importServiceProvider)
        .previewImage(bytes, file.name.isEmpty ? 'recipe.jpg' : file.name);
    if (mounted) openPreview(context, preview, status: status);
  });

  @override
  Widget build(BuildContext context) => _TabBody(
    children: [
      const _Hint(
        'Take a photo of a recipe card or cookbook page. AI will read it and fill in a recipe for you to review.',
      ),
      const _SingleTrialSwitch(),
      if (busy)
        const Padding(
          padding: EdgeInsets.all(24),
          child: Center(child: CircularProgressIndicator()),
        ),
      Wrap(
        spacing: 12,
        runSpacing: 8,
        children: [
          FilledButton.icon(
            onPressed: busy ? null : () => _pick(ImageSource.camera),
            icon: const Icon(Icons.photo_camera_outlined),
            label: const Text('Take photo'),
          ),
          OutlinedButton.icon(
            onPressed: busy ? null : () => _pick(ImageSource.gallery),
            icon: const Icon(Icons.photo_library_outlined),
            label: const Text('Choose image'),
          ),
        ],
      ),
    ],
  );
}

class _FileTab extends ConsumerStatefulWidget {
  const _FileTab({required this.onJob, this.initialTrial = false});
  final ValueChanged<Stream<ImportJob>> onJob;
  final bool initialTrial;

  @override
  ConsumerState<_FileTab> createState() => _FileTabState();
}

class _FileTabState extends ConsumerState<_FileTab> with _Busy {
  String _source = 'auto';
  bool _skipDuplicates = true;
  bool _categorize = true;
  late bool _trial = widget.initialTrial;

  void _pick() => run(() async {
    final file = await FilePicker.pickFile();
    if (file == null) return;
    final bytes = await file.readAsBytes();
    widget.onJob(
      ref
          .read(importServiceProvider)
          .importFile(
            bytes,
            file.name,
            source: _source,
            skipDuplicates: _skipDuplicates,
            categorize: _categorize,
            status: _trial ? RecipeStatus.trial : null,
          ),
    );
  });

  @override
  Widget build(BuildContext context) => _TabBody(
    children: [
      const _Hint('Import a whole collection from another recipe app\'s export file.'),
      DropdownButtonFormField<String>(
        initialValue: _source,
        decoration: const InputDecoration(labelText: 'Source', prefixIcon: Icon(Icons.apps_rounded)),
        items: [for (final e in importSources.entries) DropdownMenuItem(value: e.key, child: Text(e.value))],
        onChanged: (v) => setState(() => _source = v ?? 'auto'),
      ),
      SwitchListTile(
        contentPadding: EdgeInsets.zero,
        value: _skipDuplicates,
        onChanged: (v) => setState(() => _skipDuplicates = v),
        title: const Text('Skip recipes I already have'),
      ),
      SwitchListTile(
        contentPadding: EdgeInsets.zero,
        value: _categorize,
        onChanged: (v) => setState(() => _categorize = v),
        title: const Text('Sort recipes into categories'),
        subtitle: const Text('Recipes without a category get a suggested one'),
      ),
      _BulkTrialSwitch(value: _trial, onChanged: (v) => setState(() => _trial = v)),
      const SizedBox(height: 8),
      Align(
        alignment: Alignment.centerLeft,
        child: FilledButton.icon(
          onPressed: busy ? null : _pick,
          icon: const Icon(Icons.upload_file_rounded),
          label: spinnerOr('Choose file & import'),
        ),
      ),
    ],
  );
}

class _MealieTab extends ConsumerStatefulWidget {
  const _MealieTab({required this.onJob, this.initialTrial = false});
  final ValueChanged<Stream<ImportJob>> onJob;
  final bool initialTrial;

  @override
  ConsumerState<_MealieTab> createState() => _MealieTabState();
}

class _MealieTabState extends ConsumerState<_MealieTab> with _Busy {
  final _url = TextEditingController();
  final _token = TextEditingController();
  bool _skipDuplicates = true;
  bool _categorize = true;
  late bool _trial = widget.initialTrial;

  @override
  void dispose() {
    _url.dispose();
    _token.dispose();
    super.dispose();
  }

  void _go() {
    if (_url.text.trim().isEmpty || _token.text.trim().isEmpty) {
      showSnack('Enter the Mealie address and an API token');
      return;
    }
    run(() async {
      widget.onJob(
        ref
            .read(importServiceProvider)
            .importMealie(
              _url.text.trim(),
              _token.text.trim(),
              skipDuplicates: _skipDuplicates,
              categorize: _categorize,
              status: _trial ? RecipeStatus.trial : null,
            ),
      );
    });
  }

  @override
  Widget build(BuildContext context) => _TabBody(
    children: [
      const _Hint(
        'Copy every recipe (with photos) straight from a running Mealie server. '
        'Create an API token in Mealie under User Profile → API Tokens.',
      ),
      TextField(
        controller: _url,
        keyboardType: TextInputType.url,
        decoration: const InputDecoration(
          labelText: 'Mealie URL',
          hintText: 'https://mealie.example.com',
          prefixIcon: Icon(Icons.dns_outlined),
        ),
      ),
      const SizedBox(height: 12),
      TextField(
        controller: _token,
        obscureText: true,
        decoration: const InputDecoration(labelText: 'API token', prefixIcon: Icon(Icons.key_outlined)),
      ),
      SwitchListTile(
        contentPadding: EdgeInsets.zero,
        value: _skipDuplicates,
        onChanged: (v) => setState(() => _skipDuplicates = v),
        title: const Text('Skip recipes I already have'),
      ),
      SwitchListTile(
        contentPadding: EdgeInsets.zero,
        value: _categorize,
        onChanged: (v) => setState(() => _categorize = v),
        title: const Text('Sort recipes into categories'),
        subtitle: const Text('Recipes without a category get a suggested one'),
      ),
      _BulkTrialSwitch(value: _trial, onChanged: (v) => setState(() => _trial = v)),
      Align(
        alignment: Alignment.centerLeft,
        child: FilledButton.icon(
          onPressed: busy ? null : _go,
          icon: const Icon(Icons.cloud_download_outlined),
          label: spinnerOr('Start import'),
        ),
      ),
    ],
  );
}

class _HistoryTab extends ConsumerWidget {
  const _HistoryTab();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(importJobsProvider);
    return RefreshIndicator(
      onRefresh: () => ref.refresh(importJobsProvider.future),
      child: async.when(
        loading: () => const LoadingView(),
        error: (e, _) => ErrorState(error: e, onRetry: () => ref.invalidate(importJobsProvider)),
        data: (jobs) => jobs.isEmpty
            ? ListView(
                children: const [
                  SizedBox(height: 80),
                  EmptyState(icon: Icons.history_rounded, title: 'No imports yet'),
                ],
              )
            : ListView.separated(
                padding: const EdgeInsets.all(16),
                itemCount: jobs.length,
                separatorBuilder: (_, _) => const SizedBox(height: 10),
                itemBuilder: (_, i) => MaxWidth(maxWidth: 720, child: ImportJobCard(job: jobs[i])),
              ),
      ),
    );
  }
}

class ImportJobCard extends StatelessWidget {
  const ImportJobCard({super.key, required this.job, this.onClose});
  final ImportJob job;
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    final (icon, color) = switch (job.status) {
      'done' => (Icons.check_circle_rounded, Colors.green.shade700),
      'failed' => (Icons.error_rounded, cs.error),
      _ => (Icons.downloading_rounded, cs.primary),
    };
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Icon(icon, color: color),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    '${importSources[job.source]?.split(' (').first ?? job.source} import',
                    style: theme.textTheme.titleMedium,
                  ),
                ),
                if (job.createdAt != null)
                  Text(
                    DateFormat.yMMMd().add_jm().format(job.createdAt!.toLocal()),
                    style: theme.textTheme.labelSmall?.copyWith(color: cs.onSurfaceVariant),
                  ),
                if (onClose != null) IconButton(onPressed: onClose, icon: const Icon(Icons.close_rounded)),
              ],
            ),
            const SizedBox(height: 10),
            if (!job.isFinished) ...[LinearProgressIndicator(value: job.progress), const SizedBox(height: 8)],
            Text(
              [
                job.total > 0 ? '${job.processed} of ${job.total} processed' : job.status,
                '${job.imported} imported',
                if (job.skipped > 0) '${job.skipped} skipped',
                if (job.failed > 0) '${job.failed} failed',
              ].join(' · '),
              style: theme.textTheme.bodyMedium,
            ),
            if (job.errors.isNotEmpty)
              ExpansionTile(
                tilePadding: EdgeInsets.zero,
                title: Text('${job.errors.length} problem${job.errors.length == 1 ? '' : 's'}'),
                children: [
                  for (final e in job.errors.take(50))
                    Align(
                      alignment: Alignment.centerLeft,
                      child: Padding(
                        padding: const EdgeInsets.only(bottom: 4),
                        child: Text('• $e', style: theme.textTheme.bodySmall),
                      ),
                    ),
                ],
              ),
          ],
        ),
      ),
    );
  }
}
