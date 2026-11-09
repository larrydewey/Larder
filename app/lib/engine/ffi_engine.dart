import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import 'engine.dart';

typedef _CallNative = Pointer<Uint8> Function(Pointer<Utf8>, Pointer<Uint8>, Size, Pointer<Size>);
typedef _CallDart = Pointer<Uint8> Function(Pointer<Utf8>, Pointer<Uint8>, int, Pointer<Size>);
typedef _FreeNative = Void Function(Pointer<Uint8>, Size);
typedef _FreeDart = void Function(Pointer<Uint8>, int);

/// `liblarder_engine.so` via dart:ffi. Every call runs in a short-lived
/// background isolate (calls can block for a long time).
class FfiEngine implements Engine {
  FfiEngine({String? libraryPath}) : libraryPath = libraryPath ?? defaultLibraryPath();

  final String libraryPath;

  static String defaultLibraryPath() {
    final env = Platform.environment['LARDER_ENGINE_LIB'];
    if (env != null && env.isNotEmpty) return env;
    if (Platform.isMacOS || Platform.isIOS) return 'liblarder_engine.dylib';
    if (Platform.isWindows) return 'larder_engine.dll';
    return 'liblarder_engine.so';
  }

  /// Throws [EngineUnavailableException] if the library cannot be opened.
  void ensureLoadable() {
    try {
      DynamicLibrary.open(libraryPath).lookup<NativeFunction<_CallNative>>('larder_call');
    } catch (e) {
      throw EngineUnavailableException(e);
    }
  }

  @override
  Future<Object?> call(String method, Map<String, Object?> args) async {
    final path = libraryPath;
    final input = jsonEncode(args);
    final String raw;
    try {
      raw = await Isolate.run(() => _invoke(path, method, input));
    } on ArgumentError catch (e) {
      throw EngineUnavailableException(e);
    }
    return unwrapEngineResponse(jsonDecode(raw));
  }

  static String _invoke(String path, String method, String input) {
    final lib = DynamicLibrary.open(path);
    final call = lib.lookupFunction<_CallNative, _CallDart>('larder_call');
    final free = lib.lookupFunction<_FreeNative, _FreeDart>('larder_free');
    final m = method.toNativeUtf8();
    final bytes = utf8.encode(input);
    final buf = malloc<Uint8>(bytes.isEmpty ? 1 : bytes.length);
    final outLen = malloc<Size>();
    try {
      buf.asTypedList(bytes.length).setAll(0, bytes);
      final out = call(m, buf, bytes.length, outLen);
      final len = outLen.value;
      final copy = Uint8List.fromList(out.asTypedList(len));
      free(out, len);
      return utf8.decode(copy);
    } finally {
      malloc.free(m);
      malloc.free(buf);
      malloc.free(outLen);
    }
  }
}
