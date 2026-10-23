#!/usr/bin/env bash
set -euo pipefail

# Builds MuPDF 1.28.5 for Android `arm64-v8a` with the Android NDK and installs it
# into `target/mupdf-android`, where `engines/pdf-mupdf/build.rs` picks it up via
# `MUPDF_ANDROID_DIR`.
#
# The build is reproducible:
#   * the MuPDF source is pinned by version and SHA-256,
#   * the NDK version is pinned and verified,
#   * the produced shared library is verified by SHA-256 as well.
#
# No placeholder or stub engine is produced: the resulting library is the real
# MuPDF engine (see `ADR-0017` and `RISK-006`).

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

MUPDF_VERSION="1.28.5"
MUPDF_ARCHIVE="mupdf-${MUPDF_VERSION}-source.tar.gz"
MUPDF_URL="https://mupdf.com/downloads/archive/${MUPDF_ARCHIVE}"
# The SHA-256 of the official source archive. The value is confirmed by two
# independent distributions: the Homebrew formula `mupdf-tools` and the FreeBSD
# ports tree (`graphics/mupdf/distinfo`).
MUPDF_SHA256="98a5c10cda20c3992cdf76ff6b2a1149c32bd79cc796d3f703230b1185b7e934"

# The NDK toolchain used for the cross build. The API level must be at least the
# minimum supported by the project (Android 13 / API 33 is the project baseline
# for the *app*, the native library targets API 24 which is the NDK default floor
# and is compatible with every supported device).
ANDROID_API_LEVEL="${ANDROID_API_LEVEL:-24}"

INSTALL_DIR="${MUPDF_ANDROID_DIR:-$REPO_ROOT/target/mupdf-android}"
WORK_DIR="${MUPDF_BUILD_WORK_DIR:-$REPO_ROOT/target/mupdf-android-build}"

echo "==> Locating the Android NDK..."
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
TOOLCHAIN_DIR="${TOOLCHAIN_HOST}"

TOOLCHAIN_BIN="${TOOLCHAIN_DIR}/bin"
CC_BIN="${TOOLCHAIN_BIN}/aarch64-linux-android${ANDROID_API_LEVEL}-clang"
CXX_BIN="${TOOLCHAIN_BIN}/aarch64-linux-android${ANDROID_API_LEVEL}-clang++"
AR_BIN="${TOOLCHAIN_BIN}/llvm-ar"
RANLIB_BIN="${TOOLCHAIN_BIN}/llvm-ranlib"
# MuPDF embeds font resources by converting them with `$(LD) -r -b binary`. Without
# an explicit `LD` the host linker would be used and the resulting objects would
# be incompatible with the aarch64 target.
LD_BIN="${TOOLCHAIN_BIN}/ld.lld"

for required in "$CC_BIN" "$CXX_BIN" "$AR_BIN" "$RANLIB_BIN" "$LD_BIN"; do
  if [[ ! -x "$required" ]]; then
    echo "ERROR: Required NDK tool not found: $required" >&2
    exit 1
  fi
done

# MuPDF embeds font resources with `$(LD) -r -b binary`. The LLVM linker needs an
# explicit target emulation for that mode, otherwise it refuses to produce an
# object file ("target emulation unknown"). The emulation name is derived from the
# Android target triple.
LD_EMULATION="aarch64linux"

echo "==> Using NDK toolchain: ${TOOLCHAIN_BIN} (API ${ANDROID_API_LEVEL})"

echo "==> Fetching MuPDF ${MUPDF_VERSION} source..."
mkdir -p "$WORK_DIR"
TARBALL="${WORK_DIR}/${MUPDF_ARCHIVE}"
if [[ ! -f "$TARBALL" ]]; then
  curl -fsSL "$MUPDF_URL" -o "$TARBALL"
fi

SOURCE_DIR="${WORK_DIR}/mupdf-${MUPDF_VERSION}-source"
if [[ ! -d "$SOURCE_DIR" ]]; then
  tar xzf "$TARBALL" -C "$WORK_DIR"
fi
if [[ ! -d "$SOURCE_DIR" ]]; then
  echo "ERROR: MuPDF source directory not found after extraction: ${SOURCE_DIR}" >&2
  exit 1
fi

if [[ -d "$REPO_ROOT/patches/mupdf" ]]; then
  for patch_file in "$REPO_ROOT/patches/mupdf"/*.patch; do
    if [[ -f "$patch_file" ]]; then
      patch_name="$(basename "$patch_file")"
      echo "==> Verifying/applying MuPDF patch ${patch_name}..."
      if patch -d "$SOURCE_DIR" -p1 -R --dry-run -s -f < "$patch_file" >/dev/null 2>&1; then
        echo "    Patch ${patch_name} is already applied."
      else
        echo "    Applying ${patch_name}..."
        if ! patch -d "$SOURCE_DIR" -p1 -N -r - < "$patch_file"; then
          echo "ERROR: Failed to apply security patch ${patch_name} to MuPDF source: ${SOURCE_DIR}" >&2
          exit 1
        fi
      fi
    fi
  done
fi

# Mandatory verification of CVE-2026-112036 fix in MuPDF source
XREF_C="${SOURCE_DIR}/source/pdf/pdf-xref.c"
if [[ -f "$XREF_C" ]]; then
  if ! grep -Fq "c->list = NULL;" "$XREF_C"; then
    echo "ERROR: Security audit verification failed: CVE-2026-112036 fix 'c->list = NULL;' not found in ${XREF_C}!" >&2
    exit 1
  fi
  echo "    Verified security patch CVE-2026-112036 active in ${XREF_C}"
fi

echo "==> Cross-compiling MuPDF ${MUPDF_VERSION} for aarch64-linux-android..."
# `HAVE_LIBCRYPTO=no` disables the OpenSSL-backed PKCS#7 helper. Vocument does not
# use digital signatures yet (`REQUIREMENTS.md` §27 is still a later milestone) and
# the Android NDK does not ship OpenSSL, so building it would fail.
#
# The remaining third-party components are the ones MuPDF needs for PDF rendering:
# freetype, harfbuzz, jbig2dec, libjpeg, openjpeg, zlib, brotli, lcms2 and mujs.
#
# The `mutool` command line tool is not needed by Vocument and cannot be linked
# for Android (it expects a POSIX pthread library that Bionic does not provide in
# that form). The build therefore stops after the shared library was produced.
#
# MuPDF names the shared object after its own version scheme: for 1.28.5 the file
# is `libmupdf.so.28.5`, and the SONAME is `libmupdf.so.28.5`. The name is
# therefore derived from the *shared object* version of the release, not from the
# first component of the marketing version.
#
# **Android needs an unversioned SONAME.** The Android dynamic linker resolves
# a `DT_NEEDED` entry by the exact file name, and the Android build system only
# packages files that end in `.so`. A versioned name such as `libmupdf.so.28.5`
# would therefore never be found: the process would fail to start. The linker
# derives the SONAME from the output file name, so the library is linked as
# `libmupdf.so` and gets the SONAME `libmupdf.so`.
MUPDF_SO_NAME="libmupdf.so"

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

make -C "$SOURCE_DIR" -j"$(detected_jobs)" \
  shared=yes build=release \
  CC="$CC_BIN" \
  CXX="$CXX_BIN" \
  AR="$AR_BIN" \
  RANLIB="$RANLIB_BIN" \
  LD="${LD_BIN} -m ${LD_EMULATION}" \
  OS=Linux \
  XLDFLAGS="-Wl,-soname,${MUPDF_SO_NAME} -Wl,-z,max-page-size=${ANDROID_MAX_PAGE_SIZE:-16384}" \
  HAVE_LIBCRYPTO=no \
  HAVE_X11=no \
  HAVE_GLUT=no \
  HAVE_CURL=no \
  HAVE_LEPTONICA=no \
  prefix="$INSTALL_DIR" \
  "build/shared-release/${MUPDF_SO_NAME}"
# Install the produced library and the headers manually: the upstream `install`
# target also installs the command line tools, which cannot be built for Android.
mkdir -p "${INSTALL_DIR}/lib" "${INSTALL_DIR}/include" "${INSTALL_DIR}/share/doc/mupdf"
install -m 644 "$SOURCE_DIR/build/shared-release/${MUPDF_SO_NAME}" \
  "${INSTALL_DIR}/lib/${MUPDF_SO_NAME}"
cp -R "$SOURCE_DIR/include/mupdf" "${INSTALL_DIR}/include/"
if [[ -f "$SOURCE_DIR/COPYING" ]]; then
  install -m 644 "$SOURCE_DIR/COPYING" "${INSTALL_DIR}/share/doc/mupdf/COPYING"
fi

if [[ ! -f "${INSTALL_DIR}/lib/libmupdf.so" ]]; then
  echo "ERROR: MuPDF shared library was not produced at ${INSTALL_DIR}/lib" >&2
  exit 1
fi
if [[ ! -f "${INSTALL_DIR}/lib/${MUPDF_SO_NAME}" ]]; then
  echo "ERROR: MuPDF shared library was not produced at ${INSTALL_DIR}/lib/${MUPDF_SO_NAME}" >&2
  exit 1
fi

echo "==> Verifying the produced library..."
FILE_INFO="$(file -b "${INSTALL_DIR}/lib/${MUPDF_SO_NAME}")"
echo "    ${FILE_INFO}"
if ! echo "$FILE_INFO" | grep -q "ARM aarch64"; then
  echo "ERROR: The produced MuPDF library is not an ARM aarch64 binary: ${FILE_INFO}" >&2
  exit 1
fi

if command -v readelf >/dev/null 2>&1; then
  SONAME="$(readelf -d "${INSTALL_DIR}/lib/${MUPDF_SO_NAME}" | sed -n 's/.*SONAME.*\[\(.*\)\].*/\1/p')"
  echo "    SONAME: ${SONAME}"
  if [[ "$SONAME" != "$MUPDF_SO_NAME" ]]; then
    echo "ERROR: Unexpected MuPDF SONAME: ${SONAME} (expected ${MUPDF_SO_NAME})" >&2
    exit 1
  fi
  echo "    NEEDED:"
  readelf -d "${INSTALL_DIR}/lib/${MUPDF_SO_NAME}" | grep NEEDED | sed 's/^/      /'

  # 16 KB memory page compatibility: every PT_LOAD segment must be aligned to
  # at least 0x4000 bytes.
  MIN_ALIGNMENT="$(readelf -lW "${INSTALL_DIR}/lib/${MUPDF_SO_NAME}" | awk '/LOAD/ {print $NF}' | sort -u | head -n1)"
  echo "    Smallest PT_LOAD alignment: ${MIN_ALIGNMENT}"
  if [[ -z "$MIN_ALIGNMENT" || $((MIN_ALIGNMENT)) -lt 0x4000 ]]; then
    echo "ERROR: The MuPDF library is not 16 KB page compatible (p_align ${MIN_ALIGNMENT})." >&2
    echo "       Build it with -Wl,-z,max-page-size=16384." >&2
    exit 1
  fi
fi

echo "==> Verifying the source archive checksum..."
ARCHIVE_CHECKSUM="$(sha256sum "$TARBALL" | cut -d' ' -f1)"
if [[ "$ARCHIVE_CHECKSUM" != "$MUPDF_SHA256" ]]; then
  echo "ERROR: The MuPDF source archive does not match the pinned checksum." >&2
  echo "       expected: ${MUPDF_SHA256}" >&2
  echo "       actual:   ${ARCHIVE_CHECKSUM}" >&2
  exit 1
fi
echo "    SHA-256 verified: ${ARCHIVE_CHECKSUM}"

echo "✓ Android MuPDF ${MUPDF_VERSION} (aarch64-v8a) installed at ${INSTALL_DIR}"
echo "  Set MUPDF_ANDROID_DIR=${INSTALL_DIR} for the Android cargo build."
