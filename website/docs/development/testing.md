# Testing

## Rust

```bash
cd server
cargo test --workspace
```

This runs the unit tests of every crate (parser, units, aisles, shopping merge, importers,
cookbook rendering, engine, auth, images) and the server's API scenario test
(`crates/server/tests/api.rs`) against a temporary SQLite database. The scenario exercises
registration, households and invites, recipes, conflicts, images, meal plans, shopping lists,
sync, imports and exports end to end.

### PostgreSQL

Set `LARDER_TEST_POSTGRES_URL` to run the same scenario against PostgreSQL too:

```bash
docker run -d --name larder-pg -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=larder_test \
  -p 5432:5432 postgres:17
LARDER_TEST_POSTGRES_URL=postgres://postgres:postgres@localhost:5432/larder_test \
  cargo test --workspace
```

!!! danger

    The test **wipes the database** first. Never point it at real data.

Without the variable the PostgreSQL test prints "skipping: LARDER_TEST_POSTGRES_URL not set"
and passes.

### Other knobs

| Variable | Effect |
|---|---|
| `LARDER_IMPORT_SAMPLES=/path/to/exports` | With `cargo test -p larder-import -- --ignored --nocapture parse_sample_directory`, parses every file in the directory and prints a summary. Useful with real exports you cannot commit. |
| `COOKBOOK_PERF_RECIPES=500` | Number of recipes in the cookbook performance test (default 200). |

### Lints

CI also runs:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Flutter

```bash
cd app
flutter pub get
flutter analyze
flutter test
```

The tests (`app/test/`) cover the API client (token refresh, errors), the outbox (ordering,
conflicts, retries), the connect-later migration, the standalone backend with a fake engine,
JSON round-trips, the router guard, cook timers, editor line conversion, fractions, recipe
search, scaling, timer detection and units, plus widget tests for sign-in and the recipe page.
They need no server and no native library.

### Engine integration tests

`test/engine/engine_integration_test.dart` runs against the real native engine:

```bash
app/tool/build_engine.sh --host
cd app
LARDER_ENGINE_LIB=../server/target/release/liblarder_engine.so flutter test test/engine
```

It is skipped ("LARDER_ENGINE_LIB not set") when the variable is unset or the file does not
exist.

## Continuous integration

`.github/workflows/ci.yml` runs on pushes to `master`/`main` and on pull requests:

| Job | Steps |
|---|---|
| **Server** | `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace` with a PostgreSQL 17 service and `LARDER_TEST_POSTGRES_URL` set, so both databases are tested. |
| **Flutter app** | Builds the engine for the host and for Android (`cargo-ndk`), `flutter analyze`, `flutter test` with `LARDER_ENGINE_LIB`, `flutter build web`, `flutter build apk`; uploads the artifacts `larder-android-apk` and `larder-web`. |
| **Docker image** | After both, builds the image (not pushed). |

`.github/workflows/release.yml` runs on `v*` tags: it builds and pushes the `linux/amd64` image
to `ghcr.io/<owner>/larder` (tags: version and `latest`) and attaches per-ABI APKs to the
GitHub release, signed with the release key from the `ANDROID_KEYSTORE_*` secrets (see
[Signing release APKs](building.md#signing-release-apks); without them it warns and uses a
debug key). The CI workflow's `larder-android-apk` artifact is always debug-signed.

`.github/workflows/docs.yml` builds this site with `mkdocs build --strict` on changes under
`website/` and deploys it to GitHub Pages from the main branch.
