import 'package:material_ui/material_ui.dart';

const terracotta = Color(0xFFB5532F);
const cream = Color(0xFFFBF6EE);

const _serif = 'DMSerifDisplay';
const _sans = 'Inter';

ThemeData buildTheme(Brightness brightness) {
  final light = brightness == Brightness.light;
  var scheme = ColorScheme.fromSeed(seedColor: terracotta, brightness: brightness);
  if (light) {
    scheme = scheme.copyWith(
      primary: terracotta,
      surface: cream,
      surfaceContainerLowest: const Color(0xFFFFFDF9),
      surfaceContainerLow: const Color(0xFFF8F1E7),
      surfaceContainer: const Color(0xFFF4EBDF),
      surfaceContainerHigh: const Color(0xFFEFE5D8),
      surfaceContainerHighest: const Color(0xFFE9DECF),
    );
  } else {
    scheme = scheme.copyWith(
      surface: const Color(0xFF1B1714),
      surfaceContainerLowest: const Color(0xFF151210),
      surfaceContainerLow: const Color(0xFF221D19),
      surfaceContainer: const Color(0xFF27211D),
      surfaceContainerHigh: const Color(0xFF312A25),
      surfaceContainerHighest: const Color(0xFF3B332D),
    );
  }

  final base = ThemeData(useMaterial3: true, colorScheme: scheme, fontFamily: _sans);
  TextStyle? serif(TextStyle? s, {double? letterSpacing}) =>
      s?.copyWith(fontFamily: _serif, fontWeight: FontWeight.w400, letterSpacing: letterSpacing);
  final t = base.textTheme;
  final textTheme = t.copyWith(
    displayLarge: serif(t.displayLarge),
    displayMedium: serif(t.displayMedium),
    displaySmall: serif(t.displaySmall),
    headlineLarge: serif(t.headlineLarge),
    headlineMedium: serif(t.headlineMedium),
    headlineSmall: serif(t.headlineSmall),
    titleLarge: serif(t.titleLarge, letterSpacing: 0.2),
    titleMedium: t.titleMedium?.copyWith(fontWeight: FontWeight.w600),
    titleSmall: t.titleSmall?.copyWith(fontWeight: FontWeight.w600),
    labelLarge: t.labelLarge?.copyWith(fontWeight: FontWeight.w600),
  );

  return base.copyWith(
    textTheme: textTheme,
    scaffoldBackgroundColor: scheme.surface,
    appBarTheme: AppBarTheme(
      backgroundColor: scheme.surface,
      foregroundColor: scheme.onSurface,
      surfaceTintColor: Colors.transparent,
      scrolledUnderElevation: 1,
      centerTitle: false,
      titleTextStyle: textTheme.titleLarge?.copyWith(fontSize: 24, color: scheme.onSurface),
    ),
    cardTheme: CardThemeData(
      elevation: 0,
      color: light ? scheme.surfaceContainerLowest : scheme.surfaceContainerLow,
      surfaceTintColor: Colors.transparent,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: scheme.outlineVariant.withValues(alpha: 0.6)),
      ),
      clipBehavior: Clip.antiAlias,
      margin: EdgeInsets.zero,
    ),
    chipTheme: base.chipTheme.copyWith(
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(20)),
      side: BorderSide(color: scheme.outlineVariant),
    ),
    inputDecorationTheme: InputDecorationThemeData(
      filled: true,
      fillColor: light ? scheme.surfaceContainerLowest : scheme.surfaceContainerLow,
      border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(12),
        borderSide: BorderSide(color: scheme.outlineVariant),
      ),
    ),
    navigationBarTheme: NavigationBarThemeData(
      backgroundColor: scheme.surfaceContainerLow,
      indicatorColor: scheme.primaryContainer,
    ),
    navigationRailTheme: NavigationRailThemeData(
      backgroundColor: scheme.surfaceContainerLow,
      indicatorColor: scheme.primaryContainer,
    ),
    floatingActionButtonTheme: FloatingActionButtonThemeData(
      backgroundColor: scheme.primary,
      foregroundColor: scheme.onPrimary,
    ),
    snackBarTheme: const SnackBarThemeData(behavior: SnackBarBehavior.floating),
    dividerTheme: DividerThemeData(color: scheme.outlineVariant.withValues(alpha: 0.6)),
  );
}
