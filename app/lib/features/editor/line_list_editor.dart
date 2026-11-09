import 'package:material_ui/material_ui.dart';

class _Line {
  _Line(String text) : controller = TextEditingController(text: text), key = UniqueKey();
  final TextEditingController controller;
  final Key key;
}

/// Holds an ordered list of editable text lines.
class LineListController extends ChangeNotifier {
  final List<_Line> _lines = [];

  List<String> get lines => _lines.map((l) => l.controller.text).toList();

  void setLines(List<String> lines) {
    for (final l in _lines) {
      l.controller.dispose();
    }
    _lines
      ..clear()
      ..addAll(lines.map(_Line.new));
    if (_lines.isEmpty) _lines.add(_Line(''));
    notifyListeners();
  }

  void add([String text = '', int? at]) {
    _lines.insert(at ?? _lines.length, _Line(text));
    notifyListeners();
  }

  void removeAt(int i) {
    _lines.removeAt(i).controller.dispose();
    notifyListeners();
  }

  /// Moves a line; [newIndex] is the final position (already adjusted).
  void move(int oldIndex, int newIndex) {
    _lines.insert(newIndex, _lines.removeAt(oldIndex));
    notifyListeners();
  }

  /// Rebuild listeners (e.g. step numbering after a heading edit).
  void refresh() => notifyListeners();

  @override
  void dispose() {
    for (final l in _lines) {
      l.controller.dispose();
    }
    super.dispose();
  }
}

/// Reorderable list of text fields. Lines starting with `#` render as
/// section headings.
class LineListEditor extends StatelessWidget {
  const LineListEditor({
    super.key,
    required this.controller,
    required this.hint,
    required this.addLabel,
    this.numbered = false,
    this.multiline = false,
    this.onChanged,
  });

  final LineListController controller;
  final String hint;
  final String addLabel;
  final bool numbered;
  final bool multiline;
  final VoidCallback? onChanged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final items = controller._lines;
        var step = 0;
        final numbers = <int?>[];
        for (final l in items) {
          numbers.add(l.controller.text.trimLeft().startsWith('#') ? null : ++step);
        }
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            ReorderableListView.builder(
              shrinkWrap: true,
              physics: const NeverScrollableScrollPhysics(),
              buildDefaultDragHandles: false,
              itemCount: items.length,
              onReorderItem: (a, b) {
                controller.move(a, b);
                onChanged?.call();
              },
              itemBuilder: (context, i) {
                final line = items[i];
                return Padding(
                  key: line.key,
                  padding: const EdgeInsets.only(bottom: 8),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      ReorderableDragStartListener(
                        index: i,
                        child: const Padding(
                          padding: EdgeInsets.only(top: 14, right: 4),
                          child: Icon(Icons.drag_indicator_rounded),
                        ),
                      ),
                      if (numbered)
                        SizedBox(
                          width: 28,
                          child: Padding(
                            padding: const EdgeInsets.only(top: 16),
                            child: Text(
                              numbers[i] == null ? '' : '${numbers[i]}.',
                              style: theme.textTheme.labelLarge?.copyWith(color: theme.colorScheme.primary),
                            ),
                          ),
                        ),
                      Expanded(
                        child: ValueListenableBuilder(
                          valueListenable: line.controller,
                          builder: (context, value, _) {
                            final heading = value.text.trimLeft().startsWith('#');
                            return TextField(
                              controller: line.controller,
                              minLines: 1,
                              maxLines: multiline && !heading ? 6 : 1,
                              keyboardType: multiline ? TextInputType.multiline : TextInputType.text,
                              textInputAction: multiline ? TextInputAction.newline : TextInputAction.next,
                              style: heading
                                  ? theme.textTheme.titleSmall?.copyWith(color: theme.colorScheme.primary)
                                  : null,
                              decoration: InputDecoration(
                                hintText: hint,
                                isDense: true,
                                prefixIcon: heading ? const Icon(Icons.segment_rounded) : null,
                              ),
                              onChanged: (_) {
                                onChanged?.call();
                                if (numbered) controller.refresh();
                              },
                              onSubmitted: multiline
                                  ? null
                                  : (_) {
                                      controller.add('', i + 1);
                                      onChanged?.call();
                                    },
                            );
                          },
                        ),
                      ),
                      IconButton(
                        tooltip: 'Remove',
                        icon: const Icon(Icons.remove_circle_outline_rounded),
                        onPressed: () {
                          controller.removeAt(i);
                          onChanged?.call();
                        },
                      ),
                    ],
                  ),
                );
              },
            ),
            Wrap(
              spacing: 8,
              children: [
                TextButton.icon(
                  onPressed: () {
                    controller.add();
                    onChanged?.call();
                  },
                  icon: const Icon(Icons.add_rounded),
                  label: Text(addLabel),
                ),
                TextButton.icon(
                  onPressed: () {
                    controller.add('## ');
                    onChanged?.call();
                  },
                  icon: const Icon(Icons.segment_rounded),
                  label: const Text('Add section'),
                ),
              ],
            ),
          ],
        );
      },
    );
  }
}
