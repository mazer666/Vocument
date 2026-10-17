#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "==> Building vocument-ffi library and uniffi-bindgen..."
cargo build --offline -p vocument-ffi --bins --lib 2>/dev/null || cargo build -p vocument-ffi --bins --lib

DYLIB_PATH="$REPO_ROOT/target/debug/libvocument_ffi.dylib"
if [[ ! -f "$DYLIB_PATH" ]]; then
  DYLIB_PATH="$REPO_ROOT/target/debug/libvocument_ffi.so"
fi

SWIFT_OUT_DIR="$REPO_ROOT/apps/macos/Sources/VocumentFFI"
SWIFT_C_INCLUDE_DIR="$REPO_ROOT/apps/macos/Sources/VocumentFFIHeaders/include"
KOTLIN_OUT_DIR="$REPO_ROOT/apps/android/app/src/main/java"

rm -rf "$SWIFT_OUT_DIR" "$SWIFT_C_INCLUDE_DIR" "$KOTLIN_OUT_DIR/uniffi"
mkdir -p "$SWIFT_OUT_DIR" "$SWIFT_C_INCLUDE_DIR" "$KOTLIN_OUT_DIR"

echo "==> Generating Swift UniFFI bindings..."
"$REPO_ROOT/target/debug/uniffi-bindgen" generate \
  --library "$DYLIB_PATH" \
  --language swift \
  --no-format \
  --out-dir "$SWIFT_OUT_DIR"

mv "$SWIFT_OUT_DIR/vocument_ffiFFI.h" "$SWIFT_C_INCLUDE_DIR/vocument_ffiFFI.h"
mv "$SWIFT_OUT_DIR/vocument_ffiFFI.modulemap" "$SWIFT_C_INCLUDE_DIR/module.modulemap"

# Provide a dummy C file for SwiftPM's C target requirement
cat > "$REPO_ROOT/apps/macos/Sources/VocumentFFIHeaders/shim.c" <<'EOF'
// Intentionally empty C translation unit for SwiftPM C target wrapping vocument_ffiFFI.h.
EOF

echo "==> Generating Kotlin UniFFI bindings..."
"$REPO_ROOT/target/debug/uniffi-bindgen" generate \
  --library "$DYLIB_PATH" \
  --language kotlin \
  --no-format \
  --out-dir "$KOTLIN_OUT_DIR"

echo "✓ Generated Swift bindings in apps/macos/Sources/{VocumentFFI,VocumentFFIHeaders}"
echo "✓ Generated Kotlin bindings in apps/android/app/src/main/java/uniffi/vocument_ffi"
