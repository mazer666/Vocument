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
  # Deterministically build native Android libvocument_ffi.so (forces rebuild on any OCR/core change)
  ./scripts/build-android-rust.sh
fi

echo "==> Step 1: Building Android debug APK via Gradle..."
(cd "$REPO_ROOT/apps/android" && ./gradlew assembleDebug)

if [[ ! -f "$APK_PATH" ]]; then
  echo "ERROR: Expected APK not found at $APK_PATH" >&2
  exit 1
fi
echo "✓ Debug APK built successfully: $APK_PATH ($(du -h "$APK_PATH" | cut -f1))"

echo "==> Step 1b: Verifying the native libraries as they are packaged in the APK..."
# The build directory and the package are not the same thing: the packaging step
# only includes files whose name ends in `.so`, and the loader resolves a
# dependency by its exact file name. A library that is missing from the package
# makes the application fail to start on the device.
./scripts/verify-android-native-libs.sh --apk "$APK_PATH"

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

echo "==> Step 4: Running binding on-device / emulator real OCR gate verification..."
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

echo "    4a. Running authentic on-device OCR quality gate on Android..."
adb -s "$TARGET_DEVICE" logcat -c
adb -s "$TARGET_DEVICE" shell "run-as org.vocument.app rm -f files/ocr_verification_result.json" 2>/dev/null || true
adb -s "$TARGET_DEVICE" shell am start -W -n org.vocument.app/.MainActivity --es "action" "verify-ocr"

# Wait for on-device OCR pipeline execution
OCR_RESULT=""
for i in {1..15}; do
  OCR_RESULT="$(adb -s "$TARGET_DEVICE" exec-out run-as org.vocument.app cat files/ocr_verification_result.json 2>/dev/null || true)"
  if [[ -n "$OCR_RESULT" ]]; then
    break
  fi
  sleep 1
done

echo "    On-Device OCR result: $OCR_RESULT"
OCR_LOG="$(adb -s "$TARGET_DEVICE" logcat -d -s VocumentOcrGate:I | grep "SUCCESS:" || true)"
if [[ -n "$OCR_LOG" ]]; then
  echo "    Logcat verification: $OCR_LOG"
fi

if [[ "$OCR_RESULT" != *"\"passed\": true"* ]]; then
  echo "ERROR: On-device Android OCR quality gate failed! Expected passed: true in result." >&2
  adb -s "$TARGET_DEVICE" logcat -d -s VocumentOcrGate:V AndroidRuntime:E
  exit 1
fi

if [[ "$OCR_RESULT" != *"\"bounding_boxes_valid\": true"* ]]; then
  echo "ERROR: On-device Android OCR produced missing or invalid word bounding boxes!" >&2
  exit 1
fi

if [[ "$OCR_RESULT" != *"\"character_bounding_boxes_valid\": true"* ]]; then
  echo "ERROR: On-device Android OCR produced missing or invalid character glyph bounding boxes!" >&2
  exit 1
fi

if [[ "$OCR_RESULT" != *"\"detected_language\": \"deu\""* ]]; then
  echo "ERROR: On-device Android OCR language detection failed! Expected 'deu', got result: $OCR_RESULT" >&2
  exit 1
fi

echo "✓ On-device authentic OCR quality gate passed on $TARGET_DEVICE (CER <= 5.0%, Char/Word Acc >= 95%, valid word & char bboxes, detected lang deu)."

echo "    4b. Verifying live UI scan capture & OCR workflow on $TARGET_DEVICE..."
adb -s "$TARGET_DEVICE" shell am force-stop org.vocument.app
adb -s "$TARGET_DEVICE" shell am start -W -n org.vocument.app/.MainActivity
sleep 1

# Tap Scan (M1) tab (approx x=280, y=604 on standard 320x640 display)
adb -s "$TARGET_DEVICE" shell input tap 280 604
sleep 1
# Tap Capture button (bounds: [32,475][137,528] -> center 85, 500)
adb -s "$TARGET_DEVICE" shell input tap 85 500
sleep 4

# Dump UI hierarchy to verify scan session and captured page
adb -s "$TARGET_DEVICE" shell uiautomator dump /data/local/tmp/verify_scan_dump.xml
DUMP_TEXT="$(adb -s "$TARGET_DEVICE" shell cat /data/local/tmp/verify_scan_dump.xml 2>/dev/null || true)"
if [[ "$DUMP_TEXT" == *"Scan &"* && "$DUMP_TEXT" == *"1 pages"* && "$DUMP_TEXT" == *"OCR: deu"* && "$DUMP_TEXT" == *"valid bboxes"* ]]; then
  echo "✓ Verified live scan capture & real OCR workflow in Android UI on $TARGET_DEVICE!"
else
  echo "ERROR: Live scan capture & OCR could not be confirmed in UI dump on $TARGET_DEVICE!" >&2
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
