# Building from source

## Prerequisites

| For | You need |
|---|---|
| Server | A recent stable Rust toolchain (CI uses `dtolnay/rust-toolchain@stable`) |
| Web UI | Flutter 3.47 (Dart 3.13); CI and the Dockerfile pin Flutter 3.47.9 |
| Android app | Flutter, Java 17, the Android SDK and NDK, Rust with the Android targets, `cargo-ndk` |
| Docker image | Docker with BuildKit |

## Server

```bash
cd server
cargo build --release -p larder-server --bin larder
./target/release/larder --help
```

For development:

```bash
cd server
cargo run -p larder-server --bin larder     # http://localhost:8080, data in server/data
```

The server serves the web UI from `./web` if it exists; otherwise only the API.

## Web UI

```bash
cd app
flutter pub get
flutter build web --release --no-web-resources-cdn   # output in app/build/web
```

`--no-web-resources-cdn` bundles CanvasKit instead of loading it from Google's CDN (the
Dockerfile and CI use it). Serve the result with the server:

```bash
LARDER_WEB_DIR=../app/build/web cargo run -p larder-server --bin larder
```

### Developing the web UI against a local server

Run the server with CORS for the Flutter dev server's origin, then run the app in Chrome:

```bash
# terminal 1
cd server
LARDER_CORS_ORIGINS=http://localhost:5000 cargo run -p larder-server --bin larder

# terminal 2
cd app
flutter run -d chrome --web-port 5000
```

On first load, enter `http://localhost:8080` as the server.

## Android

The Android app needs the on-device engine, `liblarder_engine.so`, for standalone mode.
Without it the app still builds and works in connected mode; **Use on this device** then
reports that the engine is missing.

### 1. Install the toolchain

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk --locked
export ANDROID_NDK_HOME=$HOME/Android/Sdk/ndk/<version>
```

### 2. Build the engine

```bash
app/tool/build_engine.sh
```

This runs from `server/`:

```bash
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 \
  -o app/android/app/src/main/jniLibs build --release -p larder-engine
```

and writes `app/android/app/src/main/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/liblarder_engine.so`
(git-ignored).

### 3. Build the APK

```bash
cd app
flutter pub get
flutter build apk --release                    # one universal APK
flutter build apk --release --split-per-abi    # one APK per ABI, as in releases
```

APKs land in `app/build/app/outputs/flutter-apk/`. Or run on a connected device with
`flutter run -d <device>`.

Facts about the Android build:

- Application ID and namespace: `io.github.larrydewey.larder`, label "Larder".
- `minSdk` is `max(flutter.minSdkVersion, 23)`.
- Permissions: `INTERNET`, `ACCESS_NETWORK_STATE`, `VIBRATE`. Cleartext HTTP is allowed for
  LAN servers.
- OIDC callback: the `larder` scheme, handled by `flutter_web_auth_2`'s `CallbackActivity`.
- Release builds are signed with the key described in `app/android/key.properties` if that
  file exists, otherwise with the debug key (fine for testing, but such an APK cannot update
  one signed with a release key). See [Signing release APKs](#signing-release-apks).

### Signing release APKs

1. Create a keystore once and keep it (and its passwords) safe: every future update must be
   signed with it.

    ```bash
    keytool -genkeypair -v -keystore release.jks -storetype JKS \
      -keyalg RSA -keysize 4096 -validity 10000 -alias larder
    ```

2. **Local builds**: copy `release.jks` to `app/android/` and create
   `app/android/key.properties` (both are secrets; do not commit them):

    ```properties title="app/android/key.properties"
    storeFile=release.jks
    storePassword=<keystore password>
    keyAlias=larder
    keyPassword=<key password>
    ```

    `storeFile` is relative to `app/android/`. Then `flutter build apk --release` signs with it.

3. **GitHub releases**: base64-encode the keystore and add four repository secrets under
   **Settings → Secrets and variables → Actions**:

    ```bash
    base64 -w0 release.jks > release.jks.b64     # macOS: base64 -i release.jks -o release.jks.b64
    ```

    | Secret | Value |
    |---|---|
    | `ANDROID_KEYSTORE_BASE64` | Contents of `release.jks.b64` |
    | `ANDROID_KEYSTORE_PASSWORD` | Keystore password |
    | `ANDROID_KEY_ALIAS` | `larder` (the alias you chose) |
    | `ANDROID_KEY_PASSWORD` | Key password (optional; defaults to the keystore password) |

    The release workflow decodes the keystore into `app/android/release.jks`, writes
    `key.properties` and builds signed APKs. If `ANDROID_KEYSTORE_BASE64` is not set, it
    prints a warning and the APKs are signed with a debug key.

### Engine for host tests

```bash
app/tool/build_engine.sh --host    # builds server/target/release/liblarder_engine.so
```

See [Testing](testing.md).

## Docker

```bash
docker build -t larder .
```

The multi-stage `Dockerfile`:

1. **web**: `debian:bookworm-slim` with Flutter `FLUTTER_VERSION` (build argument, default
   3.47.9); `flutter build web --release --no-web-resources-cdn`.
2. **server**: `rust:1-bookworm`; `cargo build --release --locked -p larder-server --bin
   larder` with BuildKit caches for the registry and `target/`.
3. **runtime**: `debian:bookworm-slim` with CA certificates and tzdata, user `larder`
   (uid 10001), the binary at `/usr/local/bin/larder` and the web build at `/app/web`.

The image is built for `linux/amd64` only, because the Flutter Linux SDK used by the web stage
is x64 only.

## Sample cookbooks

To render sample PDFs in every theme:

```bash
cd server
cargo run --release -p larder-cookbook --example sample -- /tmp/cookbooks letter
# arguments: <output dir> [letter|a4|a5] [number of extra synthetic recipes]
```

## Documentation site

```bash
cd website
pip install -r requirements.txt       # mkdocs 1.6.1, mkdocs-material 9.8.1
mkdocs serve                          # http://127.0.0.1:8000
mkdocs build --strict                 # what CI runs
```
