import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:http/http.dart' as http;
import 'package:path_provider/path_provider.dart';

import '../data/backend.dart';
import '../api/api_client.dart';
import '../data/local_store.dart';
import '../engine/engine.dart';
import '../engine/ffi_engine.dart';
import 'local_backend.dart';
import 'standalone.dart';

StandaloneSupport? createStandaloneSupport() => IoStandaloneSupport();

class IoStandaloneSupport implements StandaloneSupport {
  IoStandaloneSupport({Engine? engine, Future<Directory> Function()? root, http.Client? httpClient})
    : _injected = engine,
      _http = httpClient,
      paths = LocalPaths(root ?? getApplicationDocumentsDirectory);

  final Engine? _injected;
  Engine? _engine;
  final http.Client? _http;
  final LocalPaths paths;

  @override
  Engine get engine => _injected ?? (_engine ??= FfiEngine());

  @override
  Backend createBackend(Ref ref) => createLocalBackend(ref, engine, paths);

  @override
  Future<String?> checkEngine() async {
    try {
      final e = engine;
      if (e is FfiEngine) e.ensureLoadable();
      await e.version();
      return null;
    } catch (e) {
      return e.toString();
    }
  }

  @override
  Future<Uint8List?> readFile(String path) async {
    final f = File(path);
    return f.existsSync() ? f.readAsBytes() : null;
  }

  @override
  Future<void> downloadImages(LocalStore store, ApiClient api, ProgressCallback onProgress) async {
    final client = _http ?? http.Client();
    final todo = (await store.recipes()).where((r) => r.imageUrl != null && r.localImages.isEmpty).toList();
    final imagesDir = (await paths.images()).path;
    var done = 0;
    onProgress(0, todo.length, 'Downloading photos');
    for (final r in todo) {
      try {
        final url = api.imageUrl(r.imageUrl, size: 'full')!;
        final res = await client.get(Uri.parse(url));
        if (res.statusCode == 200) {
          final tmp = await paths.tmpFile('${r.id}.img');
          await File(tmp).writeAsBytes(res.bodyBytes, flush: true);
          final out = await engine.processImage(tmp, imagesDir, r.id);
          await File(tmp).delete().catchError((_) => File(tmp));
          await store.putRecipe(r.copyWith(localImages: out, imageUrl: () => null));
        }
      } catch (_) {
        // Keep going; the recipe simply has no offline photo.
      }
      onProgress(++done, todo.length, 'Downloading photos');
    }
  }
}
