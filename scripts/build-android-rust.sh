#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk}"
export ANDROID_HOME="${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}"
export PATH="$HOME/.cargo/bin:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools:${JAVA_HOME}/bin:$PATH"

# The Android native API level the project targets.
ANDROID_API_LEVEL="${ANDROID_API_LEVEL:-24}"

# 0. Locate the Android NDK
#
# The NDK may live inside the Android SDK, or be pointed at explicitly through
# ANDROID_NDK_HOME / ANDROID_NDK_ROOT. The toolchain host directory differs
# between macOS and Linux hosts, so both are probed.
NDK_DIR=""
if [[ -n "${ANDROID_NDK_HOME:-}" ]]; then
  NDK_DIR="${ANDROID_NDK_HOME}"
elif [[ -n "${ANDROID_NDK_ROOT:-}" ]]; then
  NDK_DIR="${ANDROID_NDK_ROOT}"
elif [[ -d "${ANDROID_HOME}/ndk" ]]; then
  NDK_DIR="$(find "${ANDROID_HOME}/ndk" -maxdepth 1 -mindepth 1 -type d | sort -V | tail -n1)"
fi

if [[ -z "$NDK_DIR" || ! -d "$NDK_DIR" ]]; then
  echo "ERROR: Android NDK not found. Set ANDROID_NDK_HOME or ANDROID_NDK_ROOT." >&2
  exit 1
fi

TOOLCHAIN=""
for host in linux-x86_64 darwin-x86_64 darwin-arm64 linux-arm64; do
  candidate="${NDK_DIR}/toolchains/llvm/prebuilt/${host}"
  if [[ -d "$candidate" ]]; then
    TOOLCHAIN="$candidate"
    break
  fi
done
if [[ -z "$TOOLCHAIN" ]]; then
  echo "ERROR: No supported LLVM toolchain found under ${NDK_DIR}/toolchains/llvm/prebuilt" >&2
  exit 1
fi

CC_TARGET="${TOOLCHAIN}/bin/aarch64-linux-android${ANDROID_API_LEVEL}-clang"
if [[ ! -x "$CC_TARGET" ]]; then
  echo "ERROR: aarch64-linux-android${ANDROID_API_LEVEL}-clang not found in ${TOOLCHAIN}/bin" >&2
  exit 1
fi

export CC_aarch64_linux_android="$CC_TARGET"
export CXX_aarch64_linux_android="${TOOLCHAIN}/bin/aarch64-linux-android${ANDROID_API_LEVEL}-clang++"
export AR_aarch64_linux_android="${TOOLCHAIN}/bin/llvm-ar"
export RANLIB_aarch64_linux_android="${TOOLCHAIN}/bin/llvm-ranlib"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_TARGET"

# 1. Deterministically ensure native Android Tesseract dependencies are extracted
"$REPO_ROOT/scripts/prepare-android-tesseract.sh"

# 2. Deterministically ensure the real MuPDF engine is cross-compiled for Android.
#
# The Android build links the genuine MuPDF 1.28.5 shared library. A placeholder
# engine is not acceptable (QA-003, ADR-0017, RISK-006).
echo "==> Preparing the Android MuPDF engine (aarch64-v8a)..."
"$REPO_ROOT/scripts/prepare-android-mupdf.sh"
export MUPDF_ANDROID_DIR="${MUPDF_ANDROID_DIR:-$REPO_ROOT/target/mupdf-android}"

# 3. Build native Android libvocument_ffi.so
echo "==> Compiling native Android libvocument_ffi.so (target: aarch64-linux-android)..."
cargo build -p vocument-ffi --target aarch64-linux-android

# 4. Verify that the produced library really contains the MuPDF engine.
#
# The check inspects the dynamic dependencies of the produced library: a build
# that silently fell back to a placeholder would not depend on libmupdf.
JNI_ARM64_DIR="$REPO_ROOT/apps/android/app/src/main/jniLibs/arm64-v8a"
mkdir -p "$JNI_ARM64_DIR"
cp "$REPO_ROOT/target/aarch64-linux-android/debug/libvocument_ffi.so" "$JNI_ARM64_DIR/libvocument_ffi.so"

if [[ ! -s "$JNI_ARM64_DIR/libvocument_ffi.so" ]]; then
  echo "ERROR: Output $JNI_ARM64_DIR/libvocument_ffi.so is missing or empty!" >&2
  exit 1
fi

if command -v readelf >/dev/null 2>&1; then
  NEEDED="$(readelf -d "$JNI_ARM64_DIR/libvocument_ffi.so" | grep NEEDED || true)"
  echo "    Native dependencies of libvocument_ffi.so:"
  echo "$NEEDED" | sed 's/^/      /'
  if ! echo "$NEEDED" | grep -q "libmupdf"; then
    echo "ERROR: libvocument_ffi.so does not depend on libmupdf!" >&2
    echo "The Android build must link the real MuPDF engine (QA-003, ADR-0017)." >&2
    exit 1
  fi
  if echo "$NEEDED" | grep -q "libmupdf-stub"; then
    echo "ERROR: libvocument_ffi.so links a placeholder MuPDF stub!" >&2
    exit 1
  fi
  if ! echo "$NEEDED" | grep -q "libtesseract"; then
    echo "ERROR: libvocument_ffi.so does not depend on libtesseract!" >&2
    echo "The Android build must link the real Tesseract engine (QA-003, ADR-0017)." >&2
    exit 1
  fi
  if echo "$NEEDED" | grep -q "stub"; then
    echo "ERROR: libvocument_ffi.so links a placeholder engine stub!" >&2
    exit 1
  fi
fi

# 5. Ship the engine runtime libraries next to the app library.
#
# Android packages native libraries per ABI in `jniLibs/<abi>/`, so the MuPDF and
# the Tesseract shared objects have to be placed there as well.
MUPDF_RUNTIME_SRC="${MUPDF_ANDROID_DIR}/lib/libmupdf.so"
if [[ -f "$MUPDF_RUNTIME_SRC" ]]; then
  cp "$MUPDF_RUNTIME_SRC" "$JNI_ARM64_DIR/libmupdf.so"
  echo "✓ MuPDF runtime library deployed to $JNI_ARM64_DIR/libmupdf.so ($(du -h "$JNI_ARM64_DIR/libmupdf.so" | cut -f1))"
else
  echo "ERROR: MuPDF runtime library not found at $MUPDF_RUNTIME_SRC" >&2
  exit 1
fi

TESSERACT_LIB_DIR="$REPO_ROOT/engines/ocr/android-libs/arm64-v8a"
for library in libtesseract.so libleptonica.so; do
  if [[ -f "$TESSERACT_LIB_DIR/$library" ]]; then
    cp "$TESSERACT_LIB_DIR/$library" "$JNI_ARM64_DIR/$library"
    echo "✓ $library deployed to $JNI_ARM64_DIR/$library ($(du -h "$JNI_ARM64_DIR/$library" | cut -f1))"
  else
    echo "ERROR: $library not found at $TESSERACT_LIB_DIR/$library" >&2
    exit 1
  fi
done

echo "✓ Native libvocument_ffi.so rebuilt and deployed to $JNI_ARM64_DIR/libvocument_ffi.so ($(du -h "$JNI_ARM64_DIR/libvocument_ffi.so" | cut -f1))"
