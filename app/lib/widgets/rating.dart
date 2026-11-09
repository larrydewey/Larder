import 'package:material_ui/material_ui.dart';

class StarRating extends StatelessWidget {
  const StarRating({super.key, required this.rating, this.onChanged, this.size = 20});
  final int? rating;
  final ValueChanged<int?>? onChanged;
  final double size;

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme.primary;
    final r = rating ?? 0;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        for (var i = 1; i <= 5; i++)
          onChanged == null
              ? Icon(i <= r ? Icons.star_rounded : Icons.star_outline_rounded, size: size, color: color)
              : IconButton(
                  visualDensity: VisualDensity.compact,
                  tooltip: '$i star${i == 1 ? '' : 's'}',
                  onPressed: () => onChanged!(r == i ? null : i),
                  icon: Icon(i <= r ? Icons.star_rounded : Icons.star_outline_rounded, size: size, color: color),
                ),
      ],
    );
  }
}
