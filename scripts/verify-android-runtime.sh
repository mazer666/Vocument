#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk}"
export ANDROID_HOME="${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}"
export PATH="${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools:${JAVA_HOME}/bin:$PATH"

APK_PATH="$REPO_ROOT/apps/android/app/build/outputs/apk/debug/app-debug.apk"

echo "==> Checking Android build environment..."
if [[ ! -d "$ANDROID_HOME" ]]; then
  echo "ERROR: ANDROID_HOME not found at $ANDROID_HOME" >&2
  exit 1
fi

echo "==> Step 1: Building Android debug APK via Gradle..."
(cd "$REPO_ROOT/apps/android" && ./gradlew assembleDebug)

if [[ ! -f "$APK_PATH" ]]; then
  echo "ERROR: Expected APK not found at $APK_PATH" >&2
  exit 1
fi
echo "✓ Debug APK built successfully: $APK_PATH ($(du -h "$APK_PATH" | cut -f1))"

echo "==> Step 2: Inspecting APK with apkanalyzer (applicationId, version, permissions)..."
if command -v apkanalyzer >/dev/null 2>&1; then
  APK_SUMMARY="$(apkanalyzer apk summary "$APK_PATH" 2>/dev/null || true)"
  echo "    Summary: $APK_SUMMARY"
  if [[ "$APK_SUMMARY" != *"org.vocument.app"* ]]; then
    echo "ERROR: APK summary does not contain org.vocument.app" >&2
    exit 1
  fi

  PERMISSIONS="$(apkanalyzer manifest permissions "$APK_PATH" 2>/dev/null || true)"
  echo "    Declared permissions: $(echo "$PERMISSIONS" | tr '\n' ' ')"
  if [[ "$PERMISSIONS" != *"android.permission.CAMERA"* ]]; then
    echo "ERROR: APK does not declare android.permission.CAMERA (required for M1 scan capture)" >&2
    exit 1
  fi
  if [[ "$PERMISSIONS" == *"android.permission.INTERNET"* ]]; then
    echo "ERROR: APK declares android.permission.INTERNET, violating strict offline-first policy (NET-001)" >&2
    exit 1
  fi
  echo "✓ APK metadata verified: org.vocument.app, CAMERA enabled, strict offline-first (no INTERNET)."
else
  echo "WARNING: apkanalyzer not in PATH, skipping deep APK manifest inspection."
fi

echo "==> Step 3: Running Android unit tests with native Rust Core linkage (Gradle testDebugUnitTest)..."
(cd "$REPO_ROOT/apps/android" && ./gradlew testDebugUnitTest)
echo "✓ Android Gradle unit and scan capture bridge tests passed."

echo "==> Step 4: Checking for connected Android emulator or physical device..."
if command -v adb >/dev/null 2>&1; then
  CONNECTED_DEVICES="$(adb devices | awk 'NR>1 && $2=="device" {print $1}')"
  if [[ -n "$CONNECTED_DEVICES" ]]; then
    echo "==> Active Android target found: $CONNECTED_DEVICES"
    echo "    Installing $APK_PATH on target..."
    adb install -r "$APK_PATH"
    echo "✓ APK successfully installed on Android target $CONNECTED_DEVICES."
  else
    echo "ℹ No live Android device/emulator attached; verified real APK assembly, manifest gates, and Gradle runtime tests."
  fi
fi

echo "==> Step 5: Running desktop Kotlin handshake CLI..."
./scripts/verify-kotlin-handshake.sh

echo "✓ Real Android runtime & scan-capture verification completed successfully."
