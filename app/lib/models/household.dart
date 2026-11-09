import 'json.dart';

class HouseholdMember {
  const HouseholdMember({required this.id, required this.username, required this.displayName, required this.role});
  final String id;
  final String username;
  final String displayName;
  final String role;

  String get name => displayName.isNotEmpty ? displayName : username;

  factory HouseholdMember.fromJson(Json j) => HouseholdMember(
    id: asStr(j['id']),
    username: asStr(j['username']),
    displayName: asStr(j['display_name']),
    role: asStr(j['role'], 'member'),
  );
}

class Household {
  const Household({required this.id, required this.name, this.createdAt, this.members = const []});
  final String id;
  final String name;
  final DateTime? createdAt;
  final List<HouseholdMember> members;

  factory Household.fromJson(Json j) => Household(
    id: asStr(j['id']),
    name: asStr(j['name']),
    createdAt: asDate(j['created_at']),
    members: asJsonList(j['members']).map(HouseholdMember.fromJson).toList(),
  );
}

class Invite {
  const Invite({required this.code, this.createdAt, this.expiresAt, this.createdBy});
  final String code;
  final DateTime? createdAt;
  final DateTime? expiresAt;
  final String? createdBy;

  factory Invite.fromJson(Json j) => Invite(
    code: asStr(j['code']),
    createdAt: asDate(j['created_at']),
    expiresAt: asDate(j['expires_at']),
    createdBy: asString(j['created_by']),
  );
}

class NameCount {
  const NameCount(this.name, this.count);
  final String name;
  final int count;

  factory NameCount.fromJson(Json j) => NameCount(asStr(j['name']), asInt(j['count']) ?? 0);
}
