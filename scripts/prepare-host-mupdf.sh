#!/usr/bin/env bash
# Builds MuPDF 1.28.5 for the **host** — the machine that runs the build — and
# installs it into `target/mupdf-host`.
#
# The PDF engine of Vocument is linked against MuPDF. A build without MuPDF
# cannot link the engine at all, and a build that links a *different* version
# of MuPDF reports a different engine id than the released one (`1.28.5`).
# The project therefore pins the version and builds it from the official
# source archive — the same archive that the Android build uses
# (`scripts/prepare-android-mupdf.sh`).
#
# The script is called by the workflows of the continuous integration:
#   * the Linux job needs the library for the gate (`scripts/local-ci.sh`),
#   * the Android job needs it for the host build of the FFI library that the
#     Gradle unit tests load,
#   * the macOS job needs it for the Swift build and for the handshake.
#
# Usage:
#   scripts/prepare-host-mupdf.sh
#
# Environment:
#   MUPDF_DIR              the installation directory (default: target/mupdf-host)
#   MUPDF_BUILD_WORK_DIR   the cache of the source (default: target/mupdf-android-build)
#   MUPDF_HOST_JOBS        the number of parallel build jobs (default: the number of cores)
#
# Exit code 0: the library and the headers are installed in `MUPDF_DIR`.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

MUPDF_VERSION="1.28.5"
MUPDF_ARCHIVE="mupdf-${MUPDF_VERSION}-source.tar.gz"
MUPDF_URL="https://mupdf.com/downloads/archive/${MUPDF_ARCHIVE}"

SOURCE_ONLY=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --source-only)
      SOURCE_ONLY=1
      shift
      ;;
    --help | -h)
      echo "Usage: $0 [--source-only]"
      exit 0
      ;;
    *)
      echo "ERROR: Unknown option '$1'" >&2
      exit 1
      ;;
  esac
done
# The SHA-256 of the official source archive. The same value is pinned in
# `scripts/prepare-android-mupdf.sh`; it is confirmed by two independent
# distributions (the Homebrew formula `mupdf-tools` and the FreeBSD ports
# tree `graphics/mupdf/distinfo`).
MUPDF_SHA256="98a5c10cda20c3992cdf76ff6b2a1149c32bd79cc796d3f703230b1185b7e934"

INSTALL_DIR="${MUPDF_DIR:-$REPO_ROOT/target/mupdf-host}"
# The source of the pinned release is cached **once** for the whole project.
# The directory is the one of the Android build on purpose: the licence audit
# (`scripts/audit-licenses.sh`) reads the third-party licences and the font
# licences of MuPDF from this directory, and the licence inventory of the
# repository (`THIRD_PARTY.md`) names this path. A second name for the same
# source would make the inventory of the repository stale.
WORK_DIR="${MUPDF_BUILD_WORK_DIR:-$REPO_ROOT/target/mupdf-android-build}"

# macOS has no `sha256sum`; it has `shasum -a 256` instead.
sha256_of_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

# `nproc` does not exist on macOS; `sysctl` does not exist on Linux.
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
JOBS="${MUPDF_HOST_JOBS:-$(detected_jobs)}"

SOURCE_DIR="${WORK_DIR}/mupdf-${MUPDF_VERSION}-source"
TARBALL="${WORK_DIR}/${MUPDF_ARCHIVE}"
SHARED_BUILD_DIR="${SOURCE_DIR}/build/host-shared-release"

# The name of the shared library and the name that the loader looks for.
# On Linux the version belongs to the name of the file and to the SONAME
# (`libmupdf.so.28.5`); the linker uses the short name (`libmupdf.so`).
# On macOS the version is not part of the name (`libmupdf.dylib`).
case "$(uname -s)" in
  Darwin)
    LIBRARY_NAME="libmupdf.dylib"
    SHORT_LIBRARY_NAME="libmupdf.dylib"
    # The loader of macOS resolves a library by its *install name*. MuPDF
    # does not set one, and the default (the path of the build directory)
    # would be meaningless in the installation. The name is therefore
    # `@rpath`, and the consumers of the library carry the rpath of the
    # installation directory: the Rust library through `engines/pdf-mupdf/
    # build.rs`, the Swift package through `apps/macos/Package.swift`.
    LIBRARY_LINK_FLAGS=(XLIB_LDFLAGS="-Wl,-install_name,@rpath/${LIBRARY_NAME}")
    ;;
  *)
    LIBRARY_NAME=""
    SHORT_LIBRARY_NAME="libmupdf.so"
    LIBRARY_LINK_FLAGS=()
    ;;
esac

echo "==> Fetching the source of MuPDF ${MUPDF_VERSION}..."
mkdir -p "$WORK_DIR"
if [[ ! -f "$TARBALL" ]]; then
  curl -fsSL "$MUPDF_URL" -o "$TARBALL"
fi

echo "==> Verifying the checksum of the archive..."
ARCHIVE_CHECKSUM="$(sha256_of_file "$TARBALL")"
if [[ "$ARCHIVE_CHECKSUM" != "$MUPDF_SHA256" ]]; then
  echo "ERROR: The MuPDF source archive does not match the pinned checksum." >&2
  echo "       expected: ${MUPDF_SHA256}" >&2
  echo "       actual:   ${ARCHIVE_CHECKSUM}" >&2
  exit 1
fi
echo "    SHA-256 verified: ${ARCHIVE_CHECKSUM}"

if [[ ! -d "$SOURCE_DIR" ]]; then
  echo "==> Unpacking the source..."
  tar xzf "$TARBALL" -C "$WORK_DIR"
fi
if [[ ! -f "$SOURCE_DIR/Makefile" ]]; then
  echo "ERROR: The MuPDF source is not complete: ${SOURCE_DIR}" >&2
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

if [[ "$SOURCE_ONLY" -eq 1 || "${MUPDF_SOURCE_ONLY:-0}" -eq 1 ]]; then
  echo "==> MuPDF ${MUPDF_VERSION} source prepared and verified in ${SOURCE_DIR} (--source-only)."
  exit 0
fi

# The recipe is the one of `docs/releasing/BUILD-FROM-SOURCE.md`: the shared
# library, no X11, no GLUT and no CURL. The project does not use them, and
# their development files are not on every build machine.
echo "==> Building MuPDF ${MUPDF_VERSION} for the host (${JOBS} jobs)..."
make -C "$SOURCE_DIR" -j"$JOBS" \
  HAVE_X11=no HAVE_GLUT=no HAVE_CURL=no \
  build_prefix=host- build=release shared=yes \
  "${LIBRARY_LINK_FLAGS[@]}"

# --- The installation ----------------------------------------------------
# The library is installed by hand, not with the `install` target of MuPDF:
# that target installs the command line tools as well, and on macOS it
# replaces the installed library with a link to itself.
if [[ -z "$LIBRARY_NAME" ]]; then
  VERSION_MINOR="$(sed -n 's/.*#define FZ_VERSION_MINOR \([0-9]*\).*/\1/p' "${SOURCE_DIR}/include/mupdf/fitz/version.h" | head -n 1)"
  VERSION_PATCH="$(sed -n 's/.*#define FZ_VERSION_PATCH \([0-9]*\).*/\1/p' "${SOURCE_DIR}/include/mupdf/fitz/version.h" | head -n 1)"
  LIBRARY_NAME="libmupdf.so.${VERSION_MINOR}.${VERSION_PATCH}"
fi

if [[ ! -f "${SHARED_BUILD_DIR}/${LIBRARY_NAME}" ]]; then
  echo "ERROR: The shared library of MuPDF was not produced: ${SHARED_BUILD_DIR}/${LIBRARY_NAME}" >&2
  exit 1
fi

echo "==> Installing the library into ${INSTALL_DIR}..."
mkdir -p "${INSTALL_DIR}/lib" "${INSTALL_DIR}/include" "${INSTALL_DIR}/share/doc/mupdf" "${INSTALL_DIR}/bin"

# The **reference renderer** `mutool` belongs to the installation: the
# interoperability gate of M0.7 opens the document that the product wrote with
# a *foreign* renderer and compares the picture. A gate that has no reference
# renderer says nothing about the interoperability — and the product links
# against this very library, so the tool and the library belong together.
#
# The tool is looked for by the product in `MUPDF_DIR/bin` (the same variable
# that the build uses to find the library) ✓.
if [[ -f "${SHARED_BUILD_DIR}/mutool" ]]; then
  install -m 755 "${SHARED_BUILD_DIR}/mutool" "${INSTALL_DIR}/bin/mutool"
  if [[ "$(uname -s)" == "Darwin" ]]; then
    # The loader of macOS resolves the library of the tool through the rpath of
    # the tool. The rpath of the build directory is meaningless in the
    # installation — the rpath of the installation is set here.
    install_name_tool -add_rpath "@loader_path/../lib" "${INSTALL_DIR}/bin/mutool" 2>/dev/null || true
  fi
  echo "    The reference renderer: ${INSTALL_DIR}/bin/mutool"
else
  echo "WARNING: The reference renderer (mutool) was not produced by the build." >&2
  echo "         The interoperability gate of M0.7 cannot open the document with a" >&2
  echo "         foreign renderer on this machine and says so (NOT RUN)." >&2
fi
install -m 644 "${SHARED_BUILD_DIR}/${LIBRARY_NAME}" "${INSTALL_DIR}/lib/${LIBRARY_NAME}"
if [[ "$SHORT_LIBRARY_NAME" != "$LIBRARY_NAME" ]]; then
  ln -sf "$LIBRARY_NAME" "${INSTALL_DIR}/lib/${SHORT_LIBRARY_NAME}"
fi
cp -R "${SOURCE_DIR}/include/mupdf" "${INSTALL_DIR}/include/"
if [[ -f "${SOURCE_DIR}/COPYING" ]]; then
  install -m 644 "${SOURCE_DIR}/COPYING" "${INSTALL_DIR}/share/doc/mupdf/COPYING"
fi

# --- The verification of the installation ---------------------------------
if [[ ! -d "${INSTALL_DIR}/lib" ]]; then
  echo "ERROR: The library directory of MuPDF is missing: ${INSTALL_DIR}/lib" >&2
  exit 1
fi
if [[ ! -f "${INSTALL_DIR}/include/mupdf/fitz/version.h" ]]; then
  echo "ERROR: The headers of MuPDF are missing: ${INSTALL_DIR}/include/mupdf" >&2
  exit 1
fi

INSTALLED_VERSION="$(sed -n 's/.*#define FZ_VERSION "\([^"]*\)".*/\1/p' "${INSTALL_DIR}/include/mupdf/fitz/version.h" | head -n 1)"
if [[ "$INSTALLED_VERSION" != "$MUPDF_VERSION" ]]; then
  echo "ERROR: The installed MuPDF has the version '${INSTALLED_VERSION}', expected '${MUPDF_VERSION}'." >&2
  exit 1
fi

# The name that the loader looks for at the run time has to be the name of
# the file that is installed.
case "$(uname -s)" in
  Darwin)
    INSTALL_NAME="$(otool -D "${INSTALL_DIR}/lib/${LIBRARY_NAME}" | tail -n 1 | awk '{print $1}')"
    if [[ "$INSTALL_NAME" != "@rpath/${LIBRARY_NAME}" ]]; then
      echo "ERROR: The install name of the MuPDF library is '${INSTALL_NAME}'," >&2
      echo "       expected '@rpath/${LIBRARY_NAME}'. The loader would not find the library." >&2
      exit 1
    fi
    echo "    Install name: ${INSTALL_NAME}"
    ;;
  *)
    SONAME="$(readelf -d "${INSTALL_DIR}/lib/${LIBRARY_NAME}" | sed -n 's/.*SONAME.*\[\(.*\)\].*/\1/p')"
    if [[ "$SONAME" != "$LIBRARY_NAME" ]]; then
      echo "ERROR: The SONAME of the MuPDF library is '${SONAME}', expected '${LIBRARY_NAME}'." >&2
      exit 1
    fi
    echo "    SONAME: ${SONAME}"
    ;;
esac

echo "✓ MuPDF ${MUPDF_VERSION} for the host installed at ${INSTALL_DIR} (${SHORT_LIBRARY_NAME})"
echo "  The build needs these two variables:"
echo "    export MUPDF_DIR=\"${INSTALL_DIR}\""
echo "    export LD_LIBRARY_PATH=\"${INSTALL_DIR}/lib\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}\"   # macOS: DYLD_LIBRARY_PATH"
