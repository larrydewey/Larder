import 'package:material_ui/material_ui.dart';

/// Centered card layout with branding, shared by setup and login.
class AuthLayout extends StatelessWidget {
  const AuthLayout({super.key, required this.title, this.subtitle, required this.children});
  final String title;
  final String? subtitle;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    return Scaffold(
      body: Container(
        decoration: BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topCenter,
            end: Alignment.bottomCenter,
            colors: [cs.primaryContainer.withValues(alpha: 0.35), cs.surface],
          ),
        ),
        child: SafeArea(
          child: Center(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(24),
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 440),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Center(
                      child: CircleAvatar(
                        radius: 34,
                        backgroundColor: cs.primary,
                        child: Icon(Icons.soup_kitchen_rounded, size: 36, color: cs.onPrimary),
                      ),
                    ),
                    const SizedBox(height: 12),
                    Text('Larder', textAlign: TextAlign.center, style: theme.textTheme.displaySmall),
                    const SizedBox(height: 24),
                    Card(
                      child: Padding(
                        padding: const EdgeInsets.all(24),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            Text(title, style: theme.textTheme.headlineSmall),
                            if (subtitle != null) ...[
                              const SizedBox(height: 6),
                              Text(subtitle!, style: theme.textTheme.bodyMedium?.copyWith(color: cs.onSurfaceVariant)),
                            ],
                            const SizedBox(height: 20),
                            ...children,
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
