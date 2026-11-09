import 'json.dart';

/// Defaults the server uses for one provider.
class AiProviderDefaults {
  const AiProviderDefaults({required this.model, required this.baseUrl, required this.needsKey});
  final String model;
  final String baseUrl;
  final bool needsKey;

  factory AiProviderDefaults.fromJson(Json j) =>
      AiProviderDefaults(model: asStr(j['model']), baseUrl: asStr(j['base_url']), needsKey: asBool(j['needs_key']));
}

/// The server's AI assistant settings as admins see them. The API key itself is never sent.
class AiSettings {
  const AiSettings({
    required this.source,
    required this.enabled,
    this.provider,
    this.model,
    this.baseUrl,
    this.hasKey = false,
    this.keyHint,
    this.keyUnreadable = false,
    this.defaults = const {},
  });

  /// `environment` (set by LARDER_LLM_* variables, read-only), `server` (set in the app) or `off`.
  final String source;
  final bool enabled;
  final String? provider;
  final String? model;
  final String? baseUrl;
  final bool hasKey;
  final String? keyHint;
  final bool keyUnreadable;
  final Map<String, AiProviderDefaults> defaults;

  bool get fromEnvironment => source == 'environment';

  factory AiSettings.fromJson(Json j) => AiSettings(
    source: asStr(j['source'], 'off'),
    enabled: asBool(j['enabled']),
    provider: asString(j['provider']),
    model: asString(j['model']),
    baseUrl: asString(j['base_url']),
    hasKey: asBool(j['has_key']),
    keyHint: asString(j['key_hint']),
    keyUnreadable: asBool(j['key_unreadable']),
    defaults: asJson(j['defaults']).map((k, v) => MapEntry(k, AiProviderDefaults.fromJson(asJson(v)))),
  );
}
