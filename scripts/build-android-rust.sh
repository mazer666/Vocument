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

# 16 KB memory page compatibility (ANDROID-16K-PAGES).
#
# Android 15 and later devices may use 16 KB memory pages. Every LOAD segment of
# a native library must then be aligned to at least 0x4000. The NDK linker
# defaults to 4 KB, so the alignment has to be requested explicitly.
export ANDROID_MAX_PAGE_SIZE="${ANDROID_MAX_PAGE_SIZE:-16384}"
MAX_PAGE_SIZE_FLAG="-Wl,-z,max-page-size=${ANDROID_MAX_PAGE_SIZE}"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-C link-arg=${MAX_PAGE_SIZE_FLAG}"

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
#
# The build profile decides the size of the shipped library. The debug profile
# keeps the DWARF debug information, which is useful while developing but adds
# roughly 90 MB to the library. The release profile builds without debug
# information and is what a distributable build uses.
BUILD_PROFILE="${BUILD_PROFILE:-debug}"
case "$BUILD_PROFILE" in
  debug) CARGO_PROFILE_ARGS=() ; CARGO_PROFILE_DIR="debug" ;;
  release) CARGO_PROFILE_ARGS=(--release) ; CARGO_PROFILE_DIR="release" ;;
  *)
    echo "ERROR: Unknown BUILD_PROFILE '$BUILD_PROFILE' (expected 'debug' or 'release')" >&2
    exit 1
    ;;
esac

echo "==> Compiling native Android libvocument_ffi.so (target: aarch64-linux-android, profile: $BUILD_PROFILE)..."
cargo build -p vocument-ffi --target aarch64-linux-android "${CARGO_PROFILE_ARGS[@]}"

# 4. Verify that the produced library really contains the MuPDF engine.
#
# The check inspects the dynamic dependencies of the produced library: a build
# that silently fell back to a placeholder would not depend on libmupdf.
JNI_ARM64_DIR="$REPO_ROOT/apps/android/app/src/main/jniLibs/arm64-v8a"
mkdir -p "$JNI_ARM64_DIR"
# Remove the previously deployed libraries. A stale file would otherwise be
# shipped inside the APK: it would waste space, and an outdated library could
# shadow the current one. The removal is not limited to `*.so`, because a
# versioned library name such as `libmupdf.so.28.5` would not be matched by it
# and would still end up in the APK.
find "$JNI_ARM64_DIR" -maxdepth 1 -type f -delete
BUILT_LIBRARY="$REPO_ROOT/target/aarch64-linux-android/${CARGO_PROFILE_DIR}/libvocument_ffi.so"
cp "$BUILT_LIBRARY" "$JNI_ARM64_DIR/libvocument_ffi.so"

# The debug symbols are not needed at runtime. They are removed so that the
# library does not ship ~90 MB of DWARF information.
if command -v "${TOOLCHAIN}/bin/llvm-strip" >/dev/null 2>&1; then
  STRIP_BIN="${TOOLCHAIN}/bin/llvm-strip"
elif command -v strip >/dev/null 2>&1; then
  STRIP_BIN="strip"
else
  STRIP_BIN=""
fi
if [[ -n "$STRIP_BIN" ]]; then
  BEFORE="$(stat -c%s "$JNI_ARM64_DIR/libvocument_ffi.so")"
  "$STRIP_BIN" --strip-debug "$JNI_ARM64_DIR/libvocument_ffi.so"
  AFTER="$(stat -c%s "$JNI_ARM64_DIR/libvocument_ffi.so")"
  echo "    Debug information removed: $((BEFORE / 1024 / 1024)) MB -> $((AFTER / 1024 / 1024)) MB"
fi

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
#
# The file name in the APK must be the name that the dynamic linker looks up.
# The linker resolves a `DT_NEEDED` entry by the *exact* name, so a library
# whose SONAME is `libmupdf.so.28.5` has to be shipped under that name. A file
# named `libmupdf.so` would NOT be found, and the process would fail to start.
MUPDF_SONAME="$(readelf -d "${MUPDF_ANDROID_DIR}/lib/libmupdf.so" 2>/dev/null | sed -n 's/.*SONAME.*\[\(.*\)\].*/\1/p')"
if [[ -z "$MUPDF_SONAME" ]]; then
  echo "ERROR: The MuPDF library has no SONAME; the Android linker could not resolve it." >&2
  exit 1
fi
MUPDF_RUNTIME_SRC="${MUPDF_ANDROID_DIR}/lib/${MUPDF_SONAME}"
if [[ ! -f "$MUPDF_RUNTIME_SRC" ]]; then
  echo "ERROR: MuPDF runtime library not found at $MUPDF_RUNTIME_SRC" >&2
  exit 1
fi
cp "$MUPDF_RUNTIME_SRC" "$JNI_ARM64_DIR/${MUPDF_SONAME}"
echo "✓ MuPDF runtime library deployed to $JNI_ARM64_DIR/${MUPDF_SONAME} ($(du -h "$JNI_ARM64_DIR/${MUPDF_SONAME}" | cut -f1))"

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

# 5b. Remove the debug information from the deployed libraries in a release
# build. The engine libraries are built with the NDK and keep their symbol
# table, which is not needed on the device and dominates the package size.
# `--strip-debug` keeps the dynamic symbol table, so the dependency closure
# stays intact and the library remains loadable.
if [[ "$BUILD_PROFILE" == "release" && -n "$STRIP_BIN" ]]; then
  echo "==> Removing the debug information from the deployed libraries..."
  for library in "$JNI_ARM64_DIR"/*.so; do
    name="$(basename "$library")"
    before="$(stat -c %s "$library")"
    if "$STRIP_BIN" --strip-debug "$library" 2>/dev/null; then
      after="$(stat -c %s "$library")"
      echo "    $name: $((before / 1024)) KB -> $((after / 1024)) KB"
    else
      echo "    WARNING: $name could not be stripped and stays larger than needed." >&2
    fi
  done
fi

# 6. Verify that every native dependency can actually be resolved by the Android
# dynamic linker, and that the libraries are 16 KB page compatible.
echo "==> Verifying the native library closure and the page alignment..."
"$REPO_ROOT/scripts/verify-android-native-libs.sh" "$JNI_ARM64_DIR"

echo "✓ Native libvocument_ffi.so rebuilt and deployed to $JNI_ARM64_DIR/libvocument_ffi.so ($(du -h "$JNI_ARM64_DIR/libvocument_ffi.so" | cut -f1))"
