import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;

import '../models/json.dart';
import '../models/user.dart';
import 'api_exception.dart';

typedef TokenListener = FutureOr<void> Function(String? access, String? refresh);

/// A file to upload in a multipart request.
class UploadFile {
  const UploadFile({required this.field, required this.bytes, required this.filename});
  final String field;
  final List<int> bytes;
  final String filename;
}

/// Low-level HTTP client for the Larder API.
///
/// Adds bearer auth, decodes JSON / error bodies and transparently refreshes
/// the access token on `401` (single-flight: concurrent requests share one
/// refresh) and retries the request once.
class ApiClient {
  ApiClient({
    String? baseUrl,
    http.Client? httpClient,
    this.accessToken,
    this.refreshToken,
    this.onTokens,
    this.onAuthFailure,
    this.timeout = const Duration(seconds: 30),
  }) : _http = httpClient ?? http.Client(),
       _baseUrl = normalizeBaseUrl(baseUrl ?? '');

  final http.Client _http;
  String _baseUrl;
  String? accessToken;
  String? refreshToken;
  TokenListener? onTokens;
  VoidCallback? onAuthFailure;
  Duration timeout;
  Future<bool>? _refreshing;

  String get baseUrl => _baseUrl;
  set baseUrl(String v) => _baseUrl = normalizeBaseUrl(v);

  bool get hasSession => refreshToken != null && refreshToken!.isNotEmpty;

  static String normalizeBaseUrl(String url) {
    var u = url.trim();
    if (u.isEmpty) return u;
    if (!u.startsWith('http://') && !u.startsWith('https://')) u = 'http://$u';
    while (u.endsWith('/')) {
      u = u.substring(0, u.length - 1);
    }
    if (u.endsWith('/api')) u = u.substring(0, u.length - 4);
    return u;
  }

  Uri uri(String path, [Map<String, dynamic>? query]) {
    final base = Uri.parse(_baseUrl.isEmpty ? Uri.base.origin : _baseUrl);
    final p = path.startsWith('/') ? path : '/$path';
    final qp = <String, dynamic>{};
    query?.forEach((k, v) {
      if (v == null) return;
      if (v is List) {
        qp[k] = v.map((e) => e.toString()).toList();
      } else {
        qp[k] = v.toString();
      }
    });
    return base.replace(path: '${base.path}$p', queryParameters: qp.isEmpty ? null : qp);
  }

  /// Absolute URL for a (possibly relative) image URL with an optional size.
  String? imageUrl(String? url, {String? size}) {
    if (url == null || url.isEmpty) return null;
    var full = url.startsWith('http') ? url : uri(url).toString();
    if (size != null && !full.contains('size=')) {
      full += full.contains('?') ? '&size=$size' : '?size=$size';
    }
    return full;
  }

  Future<void> setTokens(String? access, String? refresh) async {
    accessToken = access;
    refreshToken = refresh;
    await onTokens?.call(access, refresh);
  }

  // ---- JSON helpers --------------------------------------------------------

  Future<dynamic> get(String path, {Map<String, dynamic>? query, bool auth = true}) =>
      sendJson('GET', path, query: query, auth: auth);
  Future<dynamic> post(String path, [Object? body, Map<String, dynamic>? query]) =>
      sendJson('POST', path, body: body, query: query);
  Future<dynamic> put(String path, [Object? body]) => sendJson('PUT', path, body: body);
  Future<dynamic> patch(String path, [Object? body]) => sendJson('PATCH', path, body: body);
  Future<dynamic> delete(String path, [Object? body]) => sendJson('DELETE', path, body: body);

  /// Sends a JSON request and returns the decoded body (or null when empty).
  Future<dynamic> sendJson(
    String method,
    String path, {
    Object? body,
    Map<String, dynamic>? query,
    bool auth = true,
  }) async {
    final res = await send(() {
      final req = http.Request(method, uri(path, query));
      req.headers['Accept'] = 'application/json';
      if (body != null) {
        req.headers['Content-Type'] = 'application/json';
        req.body = jsonEncode(body);
      }
      return req;
    }, auth: auth);
    return decodeBody(res);
  }

  /// Sends a request expecting binary data (PDF/zip).
  Future<Uint8List> sendBytes(String method, String path, {Object? body}) async {
    final res = await send(() {
      final req = http.Request(method, uri(path));
      if (body != null) {
        req.headers['Content-Type'] = 'application/json';
        req.body = jsonEncode(body);
      }
      return req;
    });
    return res.bodyBytes;
  }

  Future<dynamic> multipart(
    String method,
    String path, {
    Map<String, String> fields = const {},
    List<UploadFile> files = const [],
  }) async {
    final res = await send(() {
      final req = http.MultipartRequest(method, uri(path));
      req.headers['Accept'] = 'application/json';
      req.fields.addAll(fields);
      for (final f in files) {
        req.files.add(http.MultipartFile.fromBytes(f.field, f.bytes, filename: f.filename));
      }
      return req;
    });
    return decodeBody(res);
  }

  static dynamic decodeBody(http.Response res) {
    if (res.bodyBytes.isEmpty) return null;
    try {
      return jsonDecode(utf8.decode(res.bodyBytes));
    } catch (_) {
      return null;
    }
  }

  /// Sends the request built by [build], handling auth + one retry after a
  /// token refresh. Throws [ApiException] for non-2xx, [NetworkException] when
  /// the server is unreachable.
  Future<http.Response> send(http.BaseRequest Function() build, {bool auth = true}) async {
    final usedToken = accessToken;
    var res = await _sendOnce(build(), auth ? usedToken : null);
    if (res.statusCode == 401 && auth && hasSession) {
      final ok = await _refreshAfter(usedToken);
      if (ok) {
        res = await _sendOnce(build(), accessToken);
      } else {
        onAuthFailure?.call();
      }
    }
    if (res.statusCode >= 200 && res.statusCode < 300) return res;
    throw _toException(res);
  }

  Future<http.Response> _sendOnce(http.BaseRequest req, String? token) async {
    if (token != null && token.isNotEmpty) req.headers['Authorization'] = 'Bearer $token';
    try {
      final streamed = await _http.send(req).timeout(timeout);
      return await http.Response.fromStream(streamed).timeout(timeout);
    } on ApiException {
      rethrow;
    } catch (e) {
      throw NetworkException(e);
    }
  }

  /// Refreshes the token unless another request already did so after
  /// [failedToken] was used. Concurrent callers share one refresh.
  Future<bool> _refreshAfter(String? failedToken) {
    if (accessToken != failedToken && accessToken != null) return Future.value(true);
    return _refreshing ??= _doRefresh().whenComplete(() => _refreshing = null);
  }

  Future<bool> _doRefresh() async {
    final rt = refreshToken;
    if (rt == null || rt.isEmpty) return false;
    final req = http.Request('POST', uri('/api/auth/refresh'))
      ..headers['Content-Type'] = 'application/json'
      ..headers['Accept'] = 'application/json'
      ..body = jsonEncode({'refresh_token': rt});
    final res = await _sendOnce(req, null); // NetworkException propagates.
    if (res.statusCode >= 200 && res.statusCode < 300) {
      final auth = AuthResponse.fromJson(asJson(decodeBody(res)));
      await setTokens(auth.accessToken, auth.refreshToken);
      return true;
    }
    if (res.statusCode == 401 || res.statusCode == 403 || res.statusCode == 400) {
      await setTokens(null, null);
      return false;
    }
    throw _toException(res);
  }

  ApiException _toException(http.Response res) {
    final body = asJson(decodeBody(res));
    final msg = asString(body['error']) ?? _defaultMessage(res.statusCode);
    return ApiException(res.statusCode, msg, code: asString(body['code']), body: body.isEmpty ? null : body);
  }

  static String _defaultMessage(int status) => switch (status) {
    400 => 'Bad request',
    401 => 'Please sign in again',
    403 => 'You are not allowed to do that',
    404 => 'Not found',
    409 => 'This was changed by someone else',
    422 => 'Could not process the request',
    _ => 'Server error ($status)',
  };
}
