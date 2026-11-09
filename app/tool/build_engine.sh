#!/usr/bin/env bash
# Builds the on-device Rust engine (liblarder_engine.so), see docs/ENGINE.md.
#
#   tool/build_engine.sh          # Android ABIs -> android/app/src/main/jniLibs/
#   tool/build_engine.sh --host   # host .so for tests (server/target/release/)
#
# Requires: Rust toolchain; for Android also cargo-ndk and the Android NDK
# (ANDROID_NDK_HOME), plus `rustup target add aarch64-linux-android
# armv7-linux-androideabi x86_64-linux-android`.
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVER_DIR="$(cd "$APP_DIR/../server" && pwd)"

cd "$SERVER_DIR"
if [[ "${1:-}" == "--host" ]]; then
  cargo build --release -p larder-engine
  echo "Built $SERVER_DIR/target/release/liblarder_engine.so"
  echo "Run tests with: LARDER_ENGINE_LIB=$SERVER_DIR/target/release/liblarder_engine.so flutter test"
  exit 0
fi

cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 \
  -o "$APP_DIR/android/app/src/main/jniLibs" \
  build --release -p larder-engine
echo "Engine libraries written to $APP_DIR/android/app/src/main/jniLibs"
