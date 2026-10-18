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

if [[ -d "${ANDROID_HOME}/ndk" ]]; then
  NDK_DIR="$(find "${ANDROID_HOME}/ndk" -maxdepth 1 -mindepth 1 | sort -V | tail -n1)"
  TOOLCHAIN="${NDK_DIR}/toolchains/llvm/prebuilt/darwin-x86_64"
  if [[ -x "${TOOLCHAIN}/bin/aarch64-linux-android34-clang" ]]; then
    export CC_aarch64_linux_android="${TOOLCHAIN}/bin/aarch64-linux-android34-clang"
    export CXX_aarch64_linux_android="${TOOLCHAIN}/bin/aarch64-linux-android34-clang++"
    export AR_aarch64_linux_android="${TOOLCHAIN}/bin/llvm-ar"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${TOOLCHAIN}/bin/aarch64-linux-android34-clang"
    export PATH="$HOME/.cargo/bin:$PATH"
    if [[ ! -f "$REPO_ROOT/apps/android/app/src/main/jniLibs/arm64-v8a/libvocument_ffi.so" ]] || [[ "$REPO_ROOT/platform/vocument-ffi/src/scan_dto.rs" -nt "$REPO_ROOT/apps/android/app/src/main/jniLibs/arm64-v8a/libvocument_ffi.so" ]] || [[ "$REPO_ROOT/platform/vocument-ffi/src/ocr_dto.rs" -nt "$REPO_ROOT/apps/android/app/src/main/jniLibs/arm64-v8a/libvocument_ffi.so" ]]; then
      echo "==> Building native Android libvocument_ffi.so for aarch64-linux-android..."
      cargo build -p vocument-ffi --target aarch64-linux-android
      mkdir -p "$REPO_ROOT/apps/android/app/src/main/jniLibs/arm64-v8a"
      cp "$REPO_ROOT/target/aarch64-linux-android/debug/libvocument_ffi.so" "$REPO_ROOT/apps/android/app/src/main/jniLibs/arm64-v8a/libvocument_ffi.so"
    fi
  fi
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

echo "==> Step 4: Running binding on-device / emulator scan workflow verification..."
if ! command -v adb >/dev/null 2>&1; then
  echo "ERROR: adb command not found in PATH! Binding Android runtime gate requires adb." >&2
  exit 1
fi

export PATH="${ANDROID_HOME}/emulator:$PATH"
STARTED_EMULATOR=0
CONNECTED_DEVICES="$(adb devices | awk 'NR>1 && $2=="device" {print $1}')"

if [[ -z "$CONNECTED_DEVICES" ]]; then
  if command -v emulator >/dev/null 2>&1 && emulator -list-avds 2>/dev/null | grep -q "test_avd"; then
    echo "    Starting headless test emulator (test_avd)..."
    emulator -avd test_avd -no-window -no-audio -no-boot-anim -gpu swiftshader_indirect >/dev/null 2>&1 &
    EMU_PID=$!
    STARTED_EMULATOR=1
    adb wait-for-device
    while [[ "$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" != "1" ]]; do
      sleep 2
    done
    CONNECTED_DEVICES="$(adb devices | awk 'NR>1 && $2=="device" {print $1}')"
  else
    echo "ERROR: No active Android device connected and test_avd emulator not found! Android runtime gate is binding and cannot be skipped." >&2
    exit 1
  fi
fi

if [[ -z "$CONNECTED_DEVICES" ]]; then
  echo "ERROR: Failed to connect to Android target device or emulator! Android runtime gate failed." >&2
  exit 1
fi

TARGET_DEVICE="$(echo "$CONNECTED_DEVICES" | head -n1)"
echo "==> Active Android target verified: $TARGET_DEVICE"
echo "    Installing $APK_PATH on $TARGET_DEVICE..."
adb -s "$TARGET_DEVICE" install -r "$APK_PATH"

echo "    Granting CAMERA permission..."
adb -s "$TARGET_DEVICE" shell pm grant org.vocument.app android.permission.CAMERA

echo "    Launching Vocument MainActivity on $TARGET_DEVICE..."
adb -s "$TARGET_DEVICE" logcat -c
adb -s "$TARGET_DEVICE" shell am start -W -n org.vocument.app/.MainActivity

echo "    Verifying process is alive..."
APP_PID="$(adb -s "$TARGET_DEVICE" shell pidof org.vocument.app | tr -d '\r')"
if [[ -z "$APP_PID" ]]; then
  echo "ERROR: org.vocument.app failed to start on $TARGET_DEVICE!" >&2
  adb -s "$TARGET_DEVICE" logcat -d -s AndroidRuntime:E
  exit 1
fi
echo "    App running with PID: $APP_PID"

echo "    Switching to Scan tab and capturing a document frame..."
sleep 1
# Tap Scan (M1) tab (approx x=280, y=604 on standard 320x640 display)
adb -s "$TARGET_DEVICE" shell input tap 280 604
sleep 1
# Tap Capture button
adb -s "$TARGET_DEVICE" shell input tap 160 504
sleep 1

# Dump UI hierarchy to verify scan session and captured page
adb -s "$TARGET_DEVICE" shell uiautomator dump /data/local/tmp/verify_scan_dump.xml
DUMP_TEXT="$(adb -s "$TARGET_DEVICE" shell cat /data/local/tmp/verify_scan_dump.xml 2>/dev/null || true)"
if [[ "$DUMP_TEXT" == *"Scan &"* && "$DUMP_TEXT" == *"1 pages"* ]]; then
  echo "✓ Verified live scan capture workflow on $TARGET_DEVICE: Session created and page appended!"
else
  echo "ERROR: Live scan capture could not be confirmed in UI dump on $TARGET_DEVICE!" >&2
  echo "UI dump output was: $DUMP_TEXT" >&2
  adb -s "$TARGET_DEVICE" shell am force-stop org.vocument.app
  exit 1
fi

# Check for any fatal crashes
CRASH_LOG="$(adb -s "$TARGET_DEVICE" logcat -d -s AndroidRuntime:E 2>/dev/null || true)"
if [[ "$CRASH_LOG" == *"FATAL EXCEPTION"* ]]; then
  echo "ERROR: Fatal exception detected in Android runtime!" >&2
  echo "$CRASH_LOG" >&2
  exit 1
fi

echo "    Stopping app on $TARGET_DEVICE..."
adb -s "$TARGET_DEVICE" shell am force-stop org.vocument.app
echo "✓ Real Android runtime & scan-capture path verified on target $TARGET_DEVICE."

if (( STARTED_EMULATOR == 1 )); then
  echo "    Stopping headless emulator (PID $EMU_PID)..."
  adb -s "$TARGET_DEVICE" emu kill || kill "$EMU_PID" 2>/dev/null || true
fi

echo "==> Step 5: Running desktop Kotlin handshake CLI..."
./scripts/verify-kotlin-handshake.sh

echo "✓ Real Android runtime & scan-capture verification completed successfully."
