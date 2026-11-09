import 'dart:io';

import 'package:flutter/painting.dart';
import 'package:flutter/services.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';

/// Saves [bytes] to the app's documents directory and opens the share sheet
/// so the user can open it in a viewer or send it somewhere.
/// Returns the saved path.
Future<String?> saveAndOpenFile(Uint8List bytes, String filename, String mimeType) async {
  Directory dir;
  try {
    dir = await getApplicationDocumentsDirectory();
  } catch (_) {
    dir = await getTemporaryDirectory();
  }
  final exports = Directory(p.join(dir.path, 'exports'));
  await exports.create(recursive: true);
  final file = File(p.join(exports.path, filename));
  await file.writeAsBytes(bytes, flush: true);
  await SharePlus.instance.share(
    ShareParams(
      files: [XFile(file.path, mimeType: mimeType, name: filename)],
      title: filename,
    ),
  );
  return file.path;
}

/// The URL fragment of the current page (web only).
String currentUrlFragment() => '';

/// Removes the fragment from the address bar (web only).
void clearUrlFragment() {}

/// Navigates the browser tab to [url] (web only).
void navigateTo(String url) {}

/// Origin of the page serving the app (web only).
String? pageOrigin() => null;

/// Plays a short alert (sound + vibration).
Future<void> alertBeep() async {
  await SystemSound.play(SystemSoundType.alert);
  await SystemSound.play(SystemSoundType.click);
  await HapticFeedback.vibrate();
}

/// On-device image file (standalone mode), or null if missing.
ImageProvider? localImageProvider(String path) {
  final f = File(path);
  return f.existsSync() ? FileImage(f) : null;
}
