import 'package:flutter_test/flutter_test.dart';
import 'package:larder/api/api_client.dart';
import 'package:larder/api/api_exception.dart';
import 'package:larder/api/endpoints.dart';

import '../support/fakes.dart';

void main() {
  test('single-flight refresh on 401 with rotation, retried once', () async {
    final server = FakeServer();
    var refreshes = 0;
    server.on('POST', '/api/auth/refresh', (r) {
      refreshes++;
      expect((r.body as Map)['refresh_token'], 'r1');
      return json({'access_token': 'a2', 'refresh_token': 'r2', 'expires_in': 900});
    });
    server.on(
      'GET',
      '/api/users/me',
      (r) => r.headers['authorization'] == 'Bearer a2'
          ? json({'id': 'u', 'username': 'alice'})
          : json({'error': 'expired', 'code': 'unauthorized'}, 401),
    );
    final saved = <String?>[];
    final api = ApiClient(
      baseUrl: 'http://srv/',
      httpClient: server.client,
      accessToken: 'a1',
      refreshToken: 'r1',
      onTokens: (a, r) => saved.add(r),
    );
    final results = await Future.wait([api.get('/api/users/me'), api.get('/api/users/me'), api.get('/api/users/me')]);
    expect(results.every((r) => (r as Map)['username'] == 'alice'), isTrue);
    expect(refreshes, 1);
    expect(api.refreshToken, 'r2');
    expect(saved, ['r2']);
  });

  test('failed refresh clears the session and reports auth failure', () async {
    final server = FakeServer();
    server.on('POST', '/api/auth/refresh', (r) => json({'error': 'revoked', 'code': 'unauthorized'}, 401));
    server.on('GET', '/api/users/me', (r) => json({'error': 'expired', 'code': 'unauthorized'}, 401));
    var failed = false;
    final api = ApiClient(
      baseUrl: 'http://srv',
      httpClient: server.client,
      accessToken: 'a1',
      refreshToken: 'r1',
      onAuthFailure: () => failed = true,
    );
    await expectLater(
      api.get('/api/users/me'),
      throwsA(isA<ApiException>().having((e) => e.statusCode, 'status', 401)),
    );
    expect(failed, isTrue);
    expect(api.hasSession, isFalse);
  });

  test('error body and network errors', () async {
    final server = FakeServer();
    server.on('GET', '/api/x', (r) => json({'error': 'Nope', 'code': 'forbidden'}, 403));
    final api = ApiClient(baseUrl: 'srv:8080/api', httpClient: server.client);
    expect(api.baseUrl, 'http://srv:8080');
    await expectLater(api.get('/api/x'), throwsA(isA<ApiException>().having((e) => e.message, 'message', 'Nope')));
    server.offline = true;
    await expectLater(api.get('/api/x'), throwsA(isA<NetworkException>()));
    expect(api.imageUrl('/api/images/abc', size: 'thumb'), 'http://srv:8080/api/images/abc?size=thumb');
  });

  test('change password sends the refresh token so this device stays signed in', () async {
    final server = FakeServer();
    server.on('PATCH', '/api/users/me', (r) => json({'id': 'u', 'username': 'alice'}));
    final saved = <String?>[];
    final api = ApiClient(
      baseUrl: 'http://srv',
      httpClient: server.client,
      accessToken: 'a1',
      refreshToken: 'r1',
      onTokens: (a, r) => saved.add(r),
    );
    await api.changePassword(currentPassword: 'old-pass', newPassword: 'new-password');
    expect(server.requests.single.body, {
      'current_password': 'old-pass',
      'new_password': 'new-password',
      'refresh_token': 'r1',
    });
    // The stored session is untouched.
    expect(api.accessToken, 'a1');
    expect(api.refreshToken, 'r1');
    expect(api.hasSession, isTrue);
    expect(saved, isEmpty);

    // SSO-only accounts send no current password.
    server.requests.clear();
    await api.changePassword(newPassword: 'new-password');
    expect(server.requests.single.body, {'new_password': 'new-password', 'refresh_token': 'r1'});
  });

  test('change password retried after a token refresh sends the rotated refresh token', () async {
    final server = FakeServer();
    server.on(
      'POST',
      '/api/auth/refresh',
      (r) => json({'access_token': 'a2', 'refresh_token': 'r2', 'expires_in': 900}),
    );
    server.on(
      'PATCH',
      '/api/users/me',
      (r) => r.headers['authorization'] == 'Bearer a2'
          ? json({'id': 'u', 'username': 'alice'})
          : json({'error': 'expired', 'code': 'unauthorized'}, 401),
    );
    final api = ApiClient(baseUrl: 'http://srv', httpClient: server.client, accessToken: 'a1', refreshToken: 'r1');
    await api.changePassword(currentPassword: 'old-pass', newPassword: 'new-password');
    final patches = server.requests.where((r) => r.method == 'PATCH').toList();
    expect(patches.map((r) => (r.body as Map)['refresh_token']), ['r1', 'r2']);
    expect(api.refreshToken, 'r2');
  });

  test('clearing the email sends an explicit null', () async {
    final server = FakeServer();
    server.on('PATCH', '/api/users/me', (r) => json({'id': 'u', 'username': 'alice'}));
    final api = ApiClient(baseUrl: 'http://srv', httpClient: server.client, accessToken: 'a1', refreshToken: 'r1');
    await api.updateProfile(displayName: 'Alice', email: '   ');
    final body = server.requests.single.body as Map;
    expect(body.containsKey('email'), isTrue);
    expect(body['email'], isNull);
    expect(body['display_name'], 'Alice');
    server.requests.clear();
    await api.updateProfile(displayName: 'Alice', email: ' a@b.c ');
    expect((server.requests.single.body as Map)['email'], 'a@b.c');
  });

  test('recipe share links: missing is null, create and delete hit the share endpoint', () async {
    final server = FakeServer();
    var shared = false;
    server.on(
      'GET',
      '/api/recipes/r1/share',
      (r) => shared
          ? json({'token': 'abc', 'url': 'https://larder.example/s/abc', 'created_at': '2026-11-09T12:00:00.000000Z'})
          : json({'error': 'recipe is not shared', 'code': 'not_found'}, 404),
    );
    server.on('POST', '/api/recipes/r1/share', (r) {
      shared = true;
      return json({
        'token': 'abc',
        'url': 'https://larder.example/s/abc',
        'created_at': '2026-11-09T12:00:00.000000Z',
      }, 201);
    });
    server.on('DELETE', '/api/recipes/r1/share', (r) {
      shared = false;
      return json(null, 204);
    });
    final api = ApiClient(baseUrl: 'http://srv', httpClient: server.client, accessToken: 'a', refreshToken: 'r');

    expect(await api.recipeShare('r1'), isNull);
    final created = await api.createRecipeShare('r1');
    expect(created.url, 'https://larder.example/s/abc');
    expect(created.createdAt, DateTime.utc(2026, 11, 9, 12));
    expect((await api.recipeShare('r1'))?.token, 'abc');
    await api.deleteRecipeShare('r1');
    expect(await api.recipeShare('r1'), isNull);
  });

  test('server config reports public sharing, off for older servers', () async {
    final server = FakeServer();
    var body = <String, dynamic>{'version': '0.1.3', 'public_sharing': true};
    server.on('GET', '/api/config', (r) => json(body));
    final api = ApiClient(baseUrl: 'http://srv', httpClient: server.client);
    expect((await api.config()).publicSharing, isTrue);
    body = {'version': '0.1.2'};
    expect((await api.config()).publicSharing, isFalse);
  });
}
