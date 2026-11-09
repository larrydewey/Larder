import 'package:cached_network_image/cached_network_image.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../data/providers.dart';
import '../models/recipe.dart';
import '../util/platform/platform.dart';

/// Recipe photo with a tinted placeholder (no image / loading / error).
class RecipeImage extends ConsumerWidget {
  const RecipeImage({
    super.key,
    required this.url,
    this.localPath,
    this.size = 'thumb',
    this.fit = BoxFit.cover,
    this.iconSize = 36,
  });

  /// Prefers the recipe's on-device image, falling back to the network.
  RecipeImage.of(Recipe recipe, {Key? key, String size = 'thumb', BoxFit fit = BoxFit.cover, double iconSize = 36})
    : this(
        key: key,
        url: recipe.imageUrl,
        localPath: recipe.localImage(size),
        size: size,
        fit: fit,
        iconSize: iconSize,
      );

  final String? url;
  final String? localPath;
  final String size;
  final BoxFit fit;
  final double iconSize;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final local = localPath == null ? null : localImageProvider(localPath!);
    if (local != null) {
      return Image(
        image: local,
        fit: fit,
        gaplessPlayback: true,
        errorBuilder: (_, _, _) => ImagePlaceholder(iconSize: iconSize),
      );
    }
    final full = ref.watch(apiClientProvider).imageUrl(url, size: size);
    if (full == null) return ImagePlaceholder(iconSize: iconSize);
    return CachedNetworkImage(
      imageUrl: full,
      fit: fit,
      fadeInDuration: const Duration(milliseconds: 200),
      placeholder: (_, _) => ImagePlaceholder(iconSize: iconSize, loading: true),
      errorWidget: (_, _, _) => ImagePlaceholder(iconSize: iconSize),
    );
  }
}

class ImagePlaceholder extends StatelessWidget {
  const ImagePlaceholder({super.key, this.iconSize = 36, this.loading = false});
  final double iconSize;
  final bool loading;

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [cs.primaryContainer.withValues(alpha: 0.55), cs.tertiaryContainer.withValues(alpha: 0.45)],
        ),
      ),
      child: Center(
        child: Icon(
          Icons.restaurant_menu_rounded,
          size: iconSize,
          color: cs.onPrimaryContainer.withValues(alpha: loading ? 0.15 : 0.35),
        ),
      ),
    );
  }
}
