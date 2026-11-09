import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/json.dart';
import '../util/units.dart';
import 'app_mode.dart';
import 'auth_controller.dart';
import 'local_store.dart';
import 'providers.dart';

/// User-supplied LLM settings for standalone mode (stored on device only).
@immutable
class LlmConfig {
  const LlmConfig({required this.provider, this.apiKey, this.model, this.baseUrl});
  final String provider; // anthropic | openai | ollama
  final String? apiKey;
  final String? model;
  final String? baseUrl;

  factory LlmConfig.fromJson(Json j) => LlmConfig(
    provider: asStr(j['provider'], 'anthropic'),
    apiKey: asString(j['api_key']),
    model: asString(j['model']),
    baseUrl: asString(j['base_url']),
  );

  Json toJson() => {'provider': provider, 'api_key': apiKey, 'model': model, 'base_url': baseUrl};
}

@immutable
class LocalPrefs {
  const LocalPrefs({this.units = 'original', this.llm});
  final String units;
  final LlmConfig? llm;
}

final localPrefsProvider = NotifierProvider<LocalPrefsController, LocalPrefs>(LocalPrefsController.new);

class LocalPrefsController extends Notifier<LocalPrefs> {
  @override
  LocalPrefs build() {
    final b = ref.watch(bootstrapProvider);
    return LocalPrefs(units: b.localUnits, llm: b.llm == null ? null : LlmConfig.fromJson(asJson(b.llm)));
  }

  void setUnits(String units) {
    state = LocalPrefs(units: units, llm: state.llm);
    unawaited(ref.read(localStoreProvider).setMeta(MetaKeys.localUnits, units));
  }

  void setLlm(LlmConfig? llm) {
    state = LocalPrefs(units: state.units, llm: llm);
    unawaited(ref.read(localStoreProvider).setMeta(MetaKeys.llmConfig, llm?.toJson()));
  }
}

/// Preferred unit system: the user's server preference when connected, a
/// device preference in standalone mode.
final unitsPreferenceProvider = Provider<UnitSystem>((ref) {
  if (ref.watch(appModeProvider) == AppMode.standalone) {
    return UnitSystem.parse(ref.watch(localPrefsProvider).units);
  }
  return UnitSystem.parse(ref.watch(authProvider).user?.units);
});

/// Whether AI-assisted imports are available.
final llmEnabledProvider = Provider<bool>((ref) {
  if (ref.watch(appModeProvider) == AppMode.standalone) return ref.watch(localPrefsProvider).llm != null;
  return ref.watch(authProvider).config?.llmEnabled ?? false;
});
