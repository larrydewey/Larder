import 'json.dart';

class User {
  const User({
    required this.id,
    required this.username,
    required this.displayName,
    this.email,
    this.isAdmin = false,
    this.householdId = '',
    this.householdRole = 'member',
    this.units = 'original',
    this.createdAt,
    this.disabled = false,
  });

  final String id;
  final String username;
  final String displayName;
  final String? email;
  final bool isAdmin;
  final String householdId;
  final String householdRole;
  final String units;
  final DateTime? createdAt;

  /// Only present on admin listings.
  final bool disabled;

  bool get isOwner => householdRole == 'owner';
  String get name => displayName.isNotEmpty ? displayName : username;

  factory User.fromJson(Json j) => User(
    id: asStr(j['id']),
    username: asStr(j['username']),
    displayName: asStr(j['display_name']),
    email: asString(j['email']),
    isAdmin: asBool(j['is_admin']),
    householdId: asStr(j['household_id']),
    householdRole: asStr(j['household_role'], 'member'),
    units: asStr(asJson(j['preferences'])['units'], 'original'),
    createdAt: asDate(j['created_at']),
    disabled: asBool(j['disabled']),
  );

  Json toJson() => {
    'id': id,
    'username': username,
    'display_name': displayName,
    'email': email,
    'is_admin': isAdmin,
    'household_id': householdId,
    'household_role': householdRole,
    'preferences': {'units': units},
    'created_at': createdAt?.toUtc().toIso8601String(),
    'disabled': disabled,
  };

  User copyWith({String? displayName, String? email, String? units}) => User(
    id: id,
    username: username,
    displayName: displayName ?? this.displayName,
    email: email ?? this.email,
    isAdmin: isAdmin,
    householdId: householdId,
    householdRole: householdRole,
    units: units ?? this.units,
    createdAt: createdAt,
    disabled: disabled,
  );
}

class AuthResponse {
  const AuthResponse({required this.accessToken, required this.refreshToken, required this.expiresIn, this.user});
  final String accessToken;
  final String refreshToken;
  final int expiresIn;
  final User? user;

  factory AuthResponse.fromJson(Json j) => AuthResponse(
    accessToken: asStr(j['access_token']),
    refreshToken: asStr(j['refresh_token']),
    expiresIn: asInt(j['expires_in']) ?? 900,
    user: j['user'] is Map ? User.fromJson(asJson(j['user'])) : null,
  );
}

class ServerConfig {
  const ServerConfig({
    this.version = '',
    this.setupRequired = false,
    this.registrationOpen = false,
    this.oidcEnabled = false,
    this.oidcName = '',
    this.llmEnabled = false,
    this.publicSharing = false,
    this.database = '',
  });

  final String version;
  final bool setupRequired;
  final bool registrationOpen;
  final bool oidcEnabled;
  final String oidcName;
  final bool llmEnabled;

  /// Whether this server lets members publish recipes at public share links.
  final bool publicSharing;
  final String database;

  factory ServerConfig.fromJson(Json j) {
    final oidc = asJson(j['oidc']);
    return ServerConfig(
      version: asStr(j['version']),
      setupRequired: asBool(j['setup_required']),
      registrationOpen: asBool(j['registration_open']),
      oidcEnabled: asBool(oidc['enabled']),
      oidcName: asStr(oidc['name'], 'SSO'),
      llmEnabled: asBool(j['llm_enabled']),
      publicSharing: asBool(j['public_sharing']),
      database: asStr(j['database']),
    );
  }

  Json toJson() => {
    'version': version,
    'setup_required': setupRequired,
    'registration_open': registrationOpen,
    'oidc': {'enabled': oidcEnabled, 'name': oidcName},
    'llm_enabled': llmEnabled,
    'public_sharing': publicSharing,
    'database': database,
  };
}
