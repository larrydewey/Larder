/// Standalone (on-device) mode support. The real implementation uses
/// dart:io + dart:ffi and is only compiled on native platforms; on the web
/// [createStandaloneSupport] returns null.
library;

import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/api_client.dart';
import '../data/backend.dart';
import '../data/local_store.dart';
import '../engine/engine.dart';
import 'standalone_stub.dart' if (dart.library.io) 'standalone_io.dart' as impl;

/// Progress callback: [done] of [total] steps, with a short description.
typedef ProgressCallback = void Function(int done, int total, String message);

abstract class StandaloneSupport {
  Engine get engine;

  /// Repositories backed by the local store + engine.
  Backend createBackend(Ref ref);

  /// Null when the engine library loads and answers `version`, else a
  /// user-facing reason.
  Future<String?> checkEngine();

  /// Reads a local file (e.g. a recipe image to upload during migration).
  Future<Uint8List?> readFile(String path);

  /// Connected -> standalone: downloads every recipe image for offline use
  /// and processes it into local JPEGs. Store data is otherwise kept.
  Future<void> downloadImages(LocalStore store, ApiClient api, ProgressCallback onProgress);
}

final standaloneSupportProvider = Provider<StandaloneSupport?>((ref) => impl.createStandaloneSupport());
