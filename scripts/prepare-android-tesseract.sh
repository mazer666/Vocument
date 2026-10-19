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

EXPECTED_TESS_SHA256="bce8dcca1544d9f8708ac126c1e920bfa92481509672d8472470e8c2eebcd43c"
EXPECTED_LEPT_SHA256="00436d8562bc833a22111a1451cf46147f05aecf8a257751e127689a1ee6e3c4"

ACTUAL_TESS_SHA="$(shasum -a 256 "$TARGET_DIR/libtesseract.so" | awk '{print $1}')"
ACTUAL_LEPT_SHA="$(shasum -a 256 "$TARGET_DIR/libleptonica.so" | awk '{print $1}')"

if [[ "$ACTUAL_TESS_SHA" != "$EXPECTED_TESS_SHA256" ]]; then
  echo "ERROR: libtesseract.so checksum mismatch! Expected $EXPECTED_TESS_SHA256, got $ACTUAL_TESS_SHA" >&2
  exit 1
fi

if [[ "$ACTUAL_LEPT_SHA" != "$EXPECTED_LEPT_SHA256" ]]; then
  echo "ERROR: libleptonica.so checksum mismatch! Expected $EXPECTED_LEPT_SHA256, got $ACTUAL_LEPT_SHA" >&2
  exit 1
fi

echo "✓ Android Tesseract native dependencies and pinned SHA-256 checksums verified at $TARGET_DIR"
