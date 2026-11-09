import 'json.dart';

/// A recipe's public share link (`/s/<token>` on the server).
class RecipeShare {
  const RecipeShare({required this.token, required this.url, this.createdAt});

  final String token;
  final String url;
  final DateTime? createdAt;

  factory RecipeShare.fromJson(Json j) =>
      RecipeShare(token: asStr(j['token']), url: asStr(j['url']), createdAt: DateTime.tryParse(asStr(j['created_at'])));
}
