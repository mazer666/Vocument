#!/usr/bin/env bash
# Cross-builds Tesseract OCR and Leptonica for Android arm64-v8a.
#
# The Android build of Vocument links against the real Tesseract engine. The
# prebuilt AAR (`cz.adaptech.tesseract4android`) is not reachable from every
# build environment, so the engine can also be built from source with the
# Android NDK. This script implements that path.
#
# The result is written to `engines/ocr/android-libs/arm64-v8a`, which is the
# directory `engines/ocr/build.rs` searches on Android targets.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# The NDK may live inside the Android SDK, or be pointed at explicitly through
# ANDROID_NDK_HOME / ANDROID_NDK_ROOT. The directory of the toolchain differs
# between the hosts: a Linux machine has `linux-x86_64`, a macOS machine
# `darwin-arm64`. The name is therefore **probed**, not assumed — a script
# that assumes the name of the other platform cannot build anything.
NDK_ROOT="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
if [[ -z "$NDK_ROOT" ]]; then
  for candidate in \
    "${ANDROID_HOME:-}/ndk" \
    "${ANDROID_SDK_ROOT:-}/ndk" \
    "$HOME/Android/Sdk/ndk" \
    "/opt/android-ndk/android-ndk-r27c" \
    "/usr/local/lib/android/sdk/ndk"; do
    if [[ -n "$candidate" && -d "$candidate" ]]; then
      NDK_ROOT="$(find "$candidate" -maxdepth 1 -mindepth 1 -type d | sort -V | tail -n1)"
      break
    fi
  done
fi
if [[ -z "$NDK_ROOT" || ! -d "$NDK_ROOT" ]]; then
  echo "ERROR: Android NDK not found. Set ANDROID_NDK_HOME or ANDROID_NDK_ROOT." >&2
  exit 1
fi

TOOLCHAIN_HOST=""
for host in linux-x86_64 darwin-x86_64 darwin-arm64 linux-arm64; do
  if [[ -d "${NDK_ROOT}/toolchains/llvm/prebuilt/${host}" ]]; then
    TOOLCHAIN_HOST="${NDK_ROOT}/toolchains/llvm/prebuilt/${host}"
    break
  fi
done
if [[ -z "$TOOLCHAIN_HOST" ]]; then
  echo "ERROR: No supported LLVM toolchain found under ${NDK_ROOT}/toolchains/llvm/prebuilt" >&2
  exit 1
fi
BIN="${TOOLCHAIN_HOST}/bin"
TARGET_API=24
TARGET_TRIPLE="aarch64-linux-android"

# The number of the parallel jobs of the build.
#
# `nproc` does not exist on macOS; `sysctl` does not exist on Linux. A build
# that asks for `$(nproc)` without a fallback ends with "command not found"
# (exit code 127) — on a Mac, and in the middle of the cross-compilation.
detected_jobs() {
    local jobs=""
    jobs="$(nproc 2>/dev/null || true)"
    if [[ -z "$jobs" ]]; then
        jobs="$(sysctl -n hw.ncpu 2>/dev/null || true)"
    fi
    if [[ -z "$jobs" ]]; then
        jobs=2
    fi
    echo "$jobs"
}

TESSERACT_VERSION="${TESSERACT_VERSION:-5.3.0}"
LEPTONICA_VERSION="${LEPTONICA_VERSION:-1.82.0}"

WORK_DIR="${WORK_DIR:-$REPO_ROOT/target/tesseract-android-build}"
INSTALL_DIR="${INSTALL_DIR:-$REPO_ROOT/target/tesseract-android}"
OUTPUT_DIR="$REPO_ROOT/engines/ocr/android-libs/arm64-v8a"

CC="${BIN}/${TARGET_TRIPLE}${TARGET_API}-clang"
CXX="${BIN}/${TARGET_TRIPLE}${TARGET_API}-clang++"
AR="${BIN}/llvm-ar"
RANLIB="${BIN}/llvm-ranlib"
STRIP="${BIN}/llvm-strip"

if [[ ! -x "$CC" ]]; then
  echo "ERROR: Android NDK compiler not found at $CC" >&2
  exit 1
fi

mkdir -p "$WORK_DIR" "$INSTALL_DIR" "$OUTPUT_DIR"

fetch() {
  local url="$1" target="$2"
  if [[ ! -f "$WORK_DIR/$target" ]]; then
    echo "    Fetching $target"
    curl -fsSL -o "$WORK_DIR/$target" "$url"
  fi
}

echo "==> Fetching the Tesseract and Leptonica sources"
fetch "https://github.com/tesseract-ocr/tesseract/archive/refs/tags/${TESSERACT_VERSION}.tar.gz" \
  "tesseract-${TESSERACT_VERSION}.tar.gz"
fetch "https://github.com/DanBloomberg/leptonica/archive/refs/tags/${LEPTONICA_VERSION}.tar.gz" \
  "leptonica-${LEPTONICA_VERSION}.tar.gz"

extract() {
  local archive="$1" directory="$2"
  if [[ ! -d "$WORK_DIR/$directory" ]]; then
    tar -xzf "$WORK_DIR/$archive" -C "$WORK_DIR"
  fi
}

extract "leptonica-${LEPTONICA_VERSION}.tar.gz" "leptonica-${LEPTONICA_VERSION}"
extract "tesseract-${TESSERACT_VERSION}.tar.gz" "tesseract-${TESSERACT_VERSION}"

# The cross compilation of Leptonica and Tesseract uses autotools. The host
# tools are used to *configure* the build; the NDK toolchain compiles and links
# the result.
export CC="$CC"
export CXX="$CXX"
export AR="$AR"
export RANLIB="$RANLIB"
export STRIP="$STRIP"
export LD="${BIN}/ld.lld -m aarch64linux"
export PKG_CONFIG_PATH="$INSTALL_DIR/lib/pkgconfig"
export CFLAGS="-fPIC -D__ANDROID__ -I$INSTALL_DIR/include"
export CXXFLAGS="-fPIC -D__ANDROID__ -I$INSTALL_DIR/include"
# 16 KB memory page compatibility (ANDROID-16K-PAGES): every PT_LOAD segment of
# a shipped native library must be aligned to at least 0x4000. The NDK linker
# defaults to 4 KB.
export LDFLAGS="-L$INSTALL_DIR/lib -Wl,-z,max-page-size=${ANDROID_MAX_PAGE_SIZE:-16384}"

# The autotools build system writes the absolute path of the compiler into the
# generated Makefiles. A build tree therefore only works with the toolchain it
# was configured with. When the NDK sits somewhere else, the build tree is
# removed and configured again.
TOOLCHAIN_MARKER="$WORK_DIR/.toolchain"
stale_toolchain=0
if [[ -f "$TOOLCHAIN_MARKER" ]] && [[ "$(cat "$TOOLCHAIN_MARKER")" != "$NDK_ROOT" ]]; then
  stale_toolchain=1
fi
# A build tree that was configured with a different toolchain still contains
# the old absolute compiler path in its generated `libtool`.
for stale_libtool in \
  "$WORK_DIR/leptonica-${LEPTONICA_VERSION}/libtool" \
  "$WORK_DIR/tesseract-${TESSERACT_VERSION}/libtool"; do
  if [[ -f "$stale_libtool" ]] && ! grep -q "$NDK_ROOT" "$stale_libtool"; then
    stale_toolchain=1
  fi
done
if [[ $stale_toolchain -eq 1 ]]; then
  echo "    The toolchain changed ($NDK_ROOT), the build tree is rebuilt"
  rm -rf "$WORK_DIR/leptonica-${LEPTONICA_VERSION}/Makefile" \
         "$WORK_DIR/leptonica-${LEPTONICA_VERSION}/config.status" \
         "$WORK_DIR/leptonica-${LEPTONICA_VERSION}/config.log" \
         "$WORK_DIR/leptonica-${LEPTONICA_VERSION}/libtool" \
         "$WORK_DIR/tesseract-${TESSERACT_VERSION}/Makefile" \
         "$WORK_DIR/tesseract-${TESSERACT_VERSION}/config.status" \
         "$WORK_DIR/tesseract-${TESSERACT_VERSION}/config.log" \
         "$WORK_DIR/tesseract-${TESSERACT_VERSION}/libtool"
fi
printf '%s' "$NDK_ROOT" > "$TOOLCHAIN_MARKER"

echo "==> Building Leptonica $LEPTONICA_VERSION for $TARGET_TRIPLE"
(
  cd "$WORK_DIR/leptonica-${LEPTONICA_VERSION}"
  if [[ ! -f Makefile ]]; then
    # The release tarball of Leptonica has no `configure` script; it is generated
    # with the autotools.
    if [[ ! -f configure ]]; then
      ./autogen.sh >/dev/null 2>&1 || autoreconf -i >/dev/null 2>&1
    fi
    # `--host` is mandatory for a cross compilation: without it, the configure
    # script tries to run the freshly built programs on the build host.
    #
    # JPEG, PNG and TIFF support is disabled: the Android NDK does not ship
    # those development headers, and the OCR pipeline works on the raw bitmaps
    # that the imaging engine produces. The image *file* formats are handled by
    # the imaging engine of Vocument, not by Tesseract.
    ./configure --host="$TARGET_TRIPLE" --prefix="$INSTALL_DIR" \
      --disable-shared --enable-static \
      --without-jpeg --without-libpng --without-libtiff \
      --without-libwebp --without-libwebpmux --without-gif \
      --without-libopenjpeg >/dev/null
  fi
  make -j"$(detected_jobs)" >/dev/null
  # Only the library is needed. The Leptonica command line tools pull in
  # additional image format libraries that the OCR engine does not use.
  make -C src install >/dev/null
)

echo "==> Building Tesseract $TESSERACT_VERSION for $TARGET_TRIPLE"
(
  cd "$WORK_DIR/tesseract-${TESSERACT_VERSION}"
  if [[ ! -f Makefile ]]; then
    ./autogen.sh >/dev/null 2>&1 || autoreconf -i >/dev/null 2>&1
    # `--host` is mandatory for a cross compilation.
    ./configure --host="$TARGET_TRIPLE" --prefix="$INSTALL_DIR" \
      --disable-shared --enable-static \
      --disable-graphics --disable-openmp --disable-luajit \
      --with-leptonica="$INSTALL_DIR" --without-curl --without-archive \
      --without-libarchive --without-tcl --without-icu >/dev/null
  fi
  make -j"$(detected_jobs)" >/dev/null
  make install >/dev/null
)

echo "==> Installing the runtime libraries into $OUTPUT_DIR"
for library in libtesseract libleptonica; do
  source_file="$(find "$INSTALL_DIR/lib" -maxdepth 1 -name "${library}.a" | head -n1)"
  if [[ -z "$source_file" ]]; then
    echo "ERROR: ${library}.a was not produced" >&2
    exit 1
  fi
  # The Android build links the engine dynamically, so a shared library is
  # required. The static archive is turned into a shared library with the NDK
  # compiler, which keeps the build free of an external download.
  shared_file="$(find "$INSTALL_DIR/lib" -maxdepth 1 -name "${library}.so.*" | head -n1)"
  if [[ -n "$shared_file" ]]; then
    install -m 644 "$shared_file" "$OUTPUT_DIR/${library}.so"
  else
    echo "    Linking ${library}.so from the static archive"
    # Bionic provides the threading primitives as part of libc; there is no
    # separate `-lpthread` on Android. The 16 KB page alignment is requested
    # explicitly, because the NDK linker defaults to 4 KB.
    "$CC" -shared -o "$OUTPUT_DIR/${library}.so" \
      -Wl,--whole-archive "$source_file" -Wl,--no-whole-archive \
      -Wl,-z,max-page-size="${ANDROID_MAX_PAGE_SIZE:-16384}" \
      -llog -lz
  fi
done

echo "✓ Android Tesseract libraries installed in $OUTPUT_DIR"
ls -la "$OUTPUT_DIR"
