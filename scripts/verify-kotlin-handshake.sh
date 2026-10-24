#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

JNA_VERSION="5.15.0"
JNA_CACHE_DIR="$REPO_ROOT/target/kotlin-test-deps"
JNA_JAR="$JNA_CACHE_DIR/jna-${JNA_VERSION}.jar"
KOTLIN_BUILD_DIR="$REPO_ROOT/target/kotlin-test-classes"

mkdir -p "$JNA_CACHE_DIR" "$KOTLIN_BUILD_DIR"

if [[ ! -f "$JNA_JAR" ]]; then
  echo "==> Downloading JNA ${JNA_VERSION} for Kotlin UniFFI verification..."
  curl -fsSL "https://repo1.maven.org/maven2/net/java/dev/jna/jna/${JNA_VERSION}/jna-${JNA_VERSION}.jar" -o "$JNA_JAR"
fi

export PATH="/opt/homebrew/opt/openjdk/bin:/opt/homebrew/opt/openjdk@17/bin:$PATH"

echo "==> Compiling Android Kotlin UniFFI bindings and DocumentCoreBridge..."
kotlinc \
  "$REPO_ROOT/apps/android/app/src/main/java/uniffi/vocument_ffi/vocument_ffi.kt" \
  "$REPO_ROOT/apps/android/app/src/main/java/org/vocument/app/DocumentCoreBridge.kt" \
  "$REPO_ROOT/apps/android/app/src/main/java/org/vocument/app/DocumentCoreBridgeWorkbench.kt" \
  "$REPO_ROOT/apps/android/app/src/test/java/org/vocument/app/VerifyKotlinHandshakeCli.kt" \
  -cp "$JNA_JAR" \
  -include-runtime \
  -d "$KOTLIN_BUILD_DIR/vocument-android-handshake.jar"

echo "==> Executing Kotlin → UniFFI → Rust DocumentCore handshake..."
java \
  --enable-native-access=ALL-UNNAMED \
  -Djna.library.path="$REPO_ROOT/target/debug" \
  -cp "$KOTLIN_BUILD_DIR/vocument-android-handshake.jar:$JNA_JAR" \
  org.vocument.app.VerifyKotlinHandshakeCliKt
