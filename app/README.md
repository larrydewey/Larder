# Larder — Flutter client

One Flutter codebase for **Android** (native app) and the **Web** (served by the
Larder server). Material 3, a warm terracotta-on-cream palette, DM Serif Display
headings and Inter body text (both bundled; no runtime font fetching).

## Modes

| Mode | Where | Source of truth | Logic |
|---|---|---|---|
| **Standalone** | Android only | local sembast store | on-device Rust engine (`liblarder_engine.so`, `dart:ffi`) |
| **Connected** | Android + Web | the server (`docs/API.md`) | server; local cache + outbox for offline use |

* First launch on Android asks: *Use on this device* or *Connect to a server*.
  The web build is always connected and defaults to the page origin (the server
  serves the web build); the URL can be overridden in Settings.
* **Connect later** (Settings → *Connect to a server…*): server URL → sign in /
  register (invite code / OIDC) → a resumable migration uploads every recipe
  (same ids, photos via `PUT /api/recipes/{id}/image`, favourites), meal plan
  entries, shopping lists and items. Optionally skips recipes whose name already
  exists on the server (meal plan entries are re-pointed to the server's recipe).
  Then the app switches to connected mode and does a full sync.
* **Disconnect** (Android, Settings → Server): flushes the outbox, downloads all
  photos and processes them into local JPEGs, then keeps the data as the new
  standalone store.

## Building

Prerequisites: Flutter 3.47 (Dart 3.13).

```sh
flutter pub get
flutter analyze
flutter test                      # unit + widget tests (no native engine needed)
flutter build web --release       # output in build/web, served by the server
```

### Android

```sh
tool/build_engine.sh              # cargo-ndk → android/app/src/main/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}
flutter build apk --release
```

`tool/build_engine.sh` runs from `../server` and needs Rust, `cargo-ndk`, the
Android NDK and the Android Rust targets. `android/app/src/main/jniLibs/` is
git-ignored. Without the library the app still runs in connected mode; the
"Use on this device" option reports that the engine is missing.

* applicationId / namespace: `io.github.larrydewey.larder`, label "Larder"
* `INTERNET` permission, cleartext HTTP allowed (self-hosted LAN servers)
* OIDC callback: `larder://auth` (`flutter_web_auth_2` `CallbackActivity`)
* minSdk: `max(flutter.minSdkVersion, 23)`

### Engine integration test

```sh
tool/build_engine.sh --host
LARDER_ENGINE_LIB=../server/target/release/liblarder_engine.so flutter test test/engine
```

The test is skipped when `LARDER_ENGINE_LIB` is unset.

## Architecture

```
lib/
  main.dart, app.dart, router.dart, theme.dart
  api/          ApiClient (bearer auth, single-flight refresh on 401 + one retry), typed endpoints
  models/       hand-written fromJson/toJson, tolerant of missing/unknown fields
  engine/       Engine interface + typed method helpers; FfiEngine (dart:ffi, Isolate.run per call)
  data/
    local_store.dart     sembast stores: recipes, meal_plan, shopping_lists, outbox, meta
    db/                  sembast_io (Android) / sembast_web (IndexedDB) via conditional import
    app_mode.dart        unset | standalone | connected
    backend.dart         RecipeRepository, MealPlanRepository, ShoppingRepository,
                         ImportService, CookbookService + backendProvider (picks by mode)
    remote_backend.dart  connected implementations (optimistic write + outbox, server calls)
    outbox.dart          ordered replay; 409 → refetch (server wins); other 4xx → drop + notice;
                         network/5xx → stop and retry later
    sync_controller.dart GET /api/sync?since= on start, resume, connectivity regained,
                         pull-to-refresh and periodically while changes are pending
    migration.dart       connect-later uploader (resumable)
    auth_controller.dart, preferences.dart, providers.dart
  standalone/   (native only; conditional import keeps dart:io/ffi out of the web build)
    local_backend.dart   standalone implementations on store + engine; images under <documents>/images
    standalone_io.dart   StandaloneSupport (engine check, photo download for disconnect)
  features/     shell, setup (welcome, server, migration), auth, recipes, cook, editor,
                mealplan, shopping, import, cookbook, settings
  util/         fractions, units, scaling, timer detection, accent folding, platform helpers
  widgets/      shared UI (images with local-file preference, states, rating, snackbars)
```

* State: `flutter_riverpod` 3 (no codegen). Routing: `go_router` with path URL
  strategy (`usePathUrlStrategy`) so OIDC can redirect to `/oidc-callback#tokens`.
* All reads come from the local store (reactive sembast queries); recipe
  search/filter/sort is always done locally (accent-insensitive, using the
  engine's `search_text` when present).
* Mutations in connected mode are applied optimistically and queued in a
  persistent outbox (client-generated UUIDs make replays idempotent).
* Ingredient display: scaling and metric/imperial conversion are done in Dart
  (`util/units.dart`, `util/scaling.dart`) in both modes.

### Routes

`/welcome`, `/setup-server`, `/login`, `/oidc-callback`, `/connect`,
`/connect/login`, `/connect/migrate`, `/recipes`, `/recipes/new`,
`/recipes/:id`, `/recipes/:id/cook`, `/recipes/:id/edit`, `/mealplan`,
`/shopping`, `/shopping/:id`, `/import`, `/cookbook`, `/settings`,
`/settings/household`, `/settings/admin`.

## Fonts

DM Serif Display and Inter are licensed under the SIL Open Font License 1.1;
license texts are in `assets/fonts/*-OFL.txt` and shown on the licenses page.
