import '../models/json.dart';

/// A non-2xx HTTP response from the server.
class ApiException implements Exception {
  ApiException(this.statusCode, this.message, {this.code, this.body});
  final int statusCode;
  final String message;
  final String? code;
  final Json? body;

  bool get isConflict => statusCode == 409;
  bool get isUnauthorized => statusCode == 401;
  bool get isNotFound => statusCode == 404;

  @override
  String toString() => message;
}

/// The server could not be reached (offline, DNS, timeout, CORS...).
class NetworkException implements Exception {
  NetworkException(this.cause);
  final Object cause;

  @override
  String toString() => 'Cannot reach the server. Check your connection.';
}

/// Human readable message for any error.
String errorMessage(Object e) {
  if (e is ApiException) return e.message;
  if (e is NetworkException) return e.toString();
  return e.toString().replaceFirst('Exception: ', '');
}
