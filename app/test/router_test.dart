import 'package:flutter_test/flutter_test.dart';
import 'package:larder/data/app_mode.dart';
import 'package:larder/data/auth_controller.dart';
import 'package:larder/router.dart';

void main() {
  test('first launch goes to welcome', () {
    expect(appRedirect(AppMode.unset, AuthStatus.needsServer, '/recipes'), '/welcome');
    expect(appRedirect(AppMode.unset, AuthStatus.needsServer, '/welcome'), isNull);
  });

  test('standalone needs no auth and allows the connect flow', () {
    expect(appRedirect(AppMode.standalone, AuthStatus.signedOut, '/recipes'), isNull);
    expect(appRedirect(AppMode.standalone, AuthStatus.signedOut, '/login'), '/recipes');
    expect(appRedirect(AppMode.standalone, AuthStatus.signedOut, '/connect'), isNull);
    expect(appRedirect(AppMode.standalone, AuthStatus.signedIn, '/connect/migrate'), isNull);
  });

  test('connected mode auth guard', () {
    expect(appRedirect(AppMode.connected, AuthStatus.needsServer, '/recipes'), '/setup-server');
    expect(appRedirect(AppMode.connected, AuthStatus.signedOut, '/mealplan'), '/login');
    expect(appRedirect(AppMode.connected, AuthStatus.signedOut, '/setup-server'), isNull);
    expect(appRedirect(AppMode.connected, AuthStatus.signedIn, '/login'), '/recipes');
    expect(appRedirect(AppMode.connected, AuthStatus.signedIn, '/connect/migrate'), '/recipes');
    expect(appRedirect(AppMode.connected, AuthStatus.signedOut, '/oidc-callback'), isNull);
  });
}
