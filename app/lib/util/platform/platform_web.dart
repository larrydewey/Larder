import 'dart:js_interop';

import 'package:flutter/painting.dart';
import 'package:flutter/services.dart';
import 'package:web/web.dart' as web;

/// Triggers a browser download of [bytes].
Future<String?> saveAndOpenFile(Uint8List bytes, String filename, String mimeType) async {
  final blob = web.Blob([bytes.toJS].toJS, web.BlobPropertyBag(type: mimeType));
  final url = web.URL.createObjectURL(blob);
  final anchor = web.HTMLAnchorElement()
    ..href = url
    ..download = filename
    ..style.display = 'none';
  web.document.body?.append(anchor);
  anchor.click();
  anchor.remove();
  Future<void>.delayed(const Duration(seconds: 30), () => web.URL.revokeObjectURL(url));
  return filename;
}

String currentUrlFragment() {
  final h = web.window.location.hash;
  return h.startsWith('#') ? h.substring(1) : h;
}

void clearUrlFragment() {
  final loc = web.window.location;
  web.window.history.replaceState(null, '', '${loc.pathname}${loc.search}');
}

void navigateTo(String url) => web.window.location.assign(url);

String? pageOrigin() => web.window.location.origin;

web.AudioContext? _ctx;

/// Plays three short beeps with WebAudio (no assets needed).
Future<void> alertBeep() async {
  try {
    final ctx = _ctx ??= web.AudioContext();
    final t0 = ctx.currentTime;
    for (var i = 0; i < 3; i++) {
      final osc = ctx.createOscillator();
      final gain = ctx.createGain();
      osc.type = 'sine';
      osc.frequency.value = 880;
      final start = t0 + i * 0.3;
      gain.gain.setValueAtTime(0.0001, start);
      gain.gain.exponentialRampToValueAtTime(0.4, start + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, start + 0.22);
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.start(start);
      osc.stop(start + 0.25);
    }
  } catch (_) {
    // Autoplay restrictions etc.
  }
  await HapticFeedback.vibrate();
}

/// No local files on the web.
ImageProvider? localImageProvider(String path) => null;
