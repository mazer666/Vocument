#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk}"
export ANDROID_HOME="${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}"

TARGET_DIR="$REPO_ROOT/engines/ocr/android-libs/arm64-v8a"
mkdir -p "$TARGET_DIR"

if [[ ! -f "$TARGET_DIR/libtesseract.so" ]] || [[ ! -f "$TARGET_DIR/libleptonica.so" ]]; then
  echo "==> Extracting Android Tesseract native dependencies from AAR via Gradle..."
  (cd "$REPO_ROOT/apps/android" && ./gradlew :app:extractTesseractNativeLibs --quiet)
fi

if [[ ! -f "$TARGET_DIR/libtesseract.so" ]] || [[ ! -f "$TARGET_DIR/libleptonica.so" ]]; then
  echo "ERROR: Failed to extract Android Tesseract native dependencies to $TARGET_DIR" >&2
  exit 1
fi

echo "✓ Android Tesseract native dependencies verified at $TARGET_DIR"
