/// Platform-specific helpers (file saving, URL fragment, alert sound).
library;

export 'platform_io.dart' if (dart.library.js_interop) 'platform_web.dart';
