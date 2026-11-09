import 'package:material_ui/material_ui.dart';

/// Chip list with an autocomplete text field for adding values.
class ChipsInput extends StatefulWidget {
  const ChipsInput({
    super.key,
    required this.label,
    required this.values,
    required this.suggestions,
    required this.onChanged,
    this.icon,
  });

  final String label;
  final List<String> values;
  final List<String> suggestions;
  final ValueChanged<List<String>> onChanged;
  final IconData? icon;

  @override
  State<ChipsInput> createState() => _ChipsInputState();
}

class _ChipsInputState extends State<ChipsInput> {
  TextEditingController? _field;

  void _add(String v) {
    final value = v.trim().replaceAll(RegExp(r',$'), '').trim();
    if (value.isEmpty) return;
    if (!widget.values.any((e) => e.toLowerCase() == value.toLowerCase())) {
      widget.onChanged([...widget.values, value]);
    }
    _field?.clear();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (widget.values.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: Wrap(
              spacing: 6,
              runSpacing: 6,
              children: [
                for (final v in widget.values)
                  InputChip(
                    label: Text(v),
                    onDeleted: () => widget.onChanged(widget.values.where((e) => e != v).toList()),
                  ),
              ],
            ),
          ),
        Autocomplete<String>(
          optionsBuilder: (value) {
            final q = value.text.trim().toLowerCase();
            final taken = widget.values.map((e) => e.toLowerCase()).toSet();
            return widget.suggestions
                .where((s) => !taken.contains(s.toLowerCase()) && (q.isEmpty || s.toLowerCase().contains(q)))
                .take(12);
          },
          onSelected: _add,
          fieldViewBuilder: (context, controller, focus, onSubmit) {
            _field = controller;
            return TextField(
              controller: controller,
              focusNode: focus,
              decoration: InputDecoration(
                labelText: 'Add ${widget.label.toLowerCase()}',
                prefixIcon: widget.icon == null ? null : Icon(widget.icon),
                suffixIcon: IconButton(icon: const Icon(Icons.add_rounded), onPressed: () => _add(controller.text)),
              ),
              onChanged: (v) {
                if (v.endsWith(',')) _add(v);
              },
              onSubmitted: (v) {
                _add(v);
                focus.requestFocus();
              },
            );
          },
        ),
      ],
    );
  }
}
