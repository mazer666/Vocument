#!/usr/bin/env bash
# Verifies the native libraries that are shipped inside the Android APK.
#
# Two properties are checked, because both of them can only be detected here and
# both of them break the application at runtime:
#
# 1. **Resolvable dependency closure.** The Android dynamic linker resolves a
#    `DT_NEEDED` entry by the *exact* file name. A library whose SONAME is
#    `libmupdf.so.28.5` therefore has to be shipped under that name. A file
#    named `libmupdf.so` is NOT found, and the process fails to start with
#    "cannot open shared object file".
#
# 2. **16 KB page compatibility.** Android 15 and later devices may use 16 KB
#    memory pages. Every `PT_LOAD` segment of every shipped library must be
#    aligned to at least 0x4000. The NDK linker defaults to 4 KB, so this has
#    to be requested explicitly with `-Wl,-z,max-page-size=16384`.
#
# The script exits with a non-zero status when a property is violated, so it can
# be used as a build gate.
#
# Usage:
#   scripts/verify-android-native-libs.sh [jni_dir]
#   scripts/verify-android-native-libs.sh --apk <path/to.apk>
#
# Without an argument the build directory of the project is checked. That
# directory is not part of the repository: when it does not exist, the proof
# is **NOT RUN** (the convention of the project) and the script ends with the
# exit code 0. A directory that is named explicitly is always checked
# strictly.
#
# The APK mode checks the libraries **as they are really shipped**. The build
# directory and the package are not the same thing: the packaging step only
# includes files whose name ends in `.so`, so a versioned library can be present
# in the build directory and still be missing from the package.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

APK_PATH=""
JNI_DIR=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --apk)
      APK_PATH="${2:-}"
      shift 2
      ;;
    -h|--help)
      sed -n '2,30p' "${BASH_SOURCE[0]}"
      exit 0
      ;;
    *)
      JNI_DIR="$1"
      shift
      ;;
  esac
done

if [[ -z "$JNI_DIR" && -z "$APK_PATH" ]]; then
  JNI_DIR="$REPO_ROOT/apps/android/app/src/main/jniLibs/arm64-v8a"
  # The mode without an argument looks at the build directory of the project.
  # That directory is **not** part of the repository: it is filled by
  # `scripts/build-android-rust.sh`. On a machine that has not built the
  # Android libraries there is nothing to verify — the proof is then NOT RUN
  # and not failed, which is the convention of the project. A directory that
  # was named explicitly is always verified strictly.
  AMBIENT_MODE=1
else
  AMBIENT_MODE=0
fi

REQUIRED_PAGE_ALIGNMENT="${REQUIRED_PAGE_ALIGNMENT:-0x4000}"

# The libraries the application needs at runtime.
REQUIRED_LIBRARIES=(
  libvocument_ffi.so
  libmupdf.so
  libtesseract.so
  libleptonica.so
)

# The libraries that the Android runtime provides. They are never shipped inside
# the APK and must not be reported as missing.
SYSTEM_LIBRARIES=(
  libc.so libm.so libdl.so liblog.so libz.so libjnigraphics.so
  libandroid.so libmediandk.so libOpenSLES.so libvulkan.so
  libstdc++.so libc++_shared.so libz.so libcamera2ndk.so
)

is_system_library() {
  local candidate="$1"
  local system
  for system in "${SYSTEM_LIBRARIES[@]}"; do
    if [[ "$candidate" == "$system" ]]; then
      return 0
    fi
  done
  return 1
}

if ! command -v readelf >/dev/null 2>&1; then
  echo "ERROR: readelf is required to verify the native libraries" >&2
  exit 1
fi

errors=0

# Verifies one directory of native libraries. The directory is either the build
# directory or one ABI directory of the extracted package.
verify_directory() {
  local directory="$1"
  local name arch min_alignment needed_names needed soname

  echo "    Native libraries in $directory:"
  shopt -s nullglob
  local libraries=("$directory"/*.so)
  shopt -u nullglob
  if [[ ${#libraries[@]} -eq 0 ]]; then
    echo "    ERROR: No native libraries found in $directory" >&2
    errors=$((errors + 1))
    return
  fi

  for library in "${libraries[@]}"; do
    name="$(basename "$library")"
    arch="$(file -b "$library")"
    if ! echo "$arch" | grep -q "ARM aarch64"; then
      echo "    ERROR: $name is not an ARM64 library: $arch" >&2
      errors=$((errors + 1))
    fi

    # 16 KB page alignment: every PT_LOAD segment must be aligned to at least
    # 0x4000 bytes.
    min_alignment="$(readelf -lW "$library" | awk '/LOAD/ {print $NF}' | sort -u | head -n1)"
    if [[ -z "$min_alignment" ]]; then
      echo "    ERROR: $name has no PT_LOAD segment" >&2
      errors=$((errors + 1))
    else
      if [[ $((min_alignment)) -lt $((REQUIRED_PAGE_ALIGNMENT)) ]]; then
        echo "    ERROR: $name is not 16 KB page compatible: p_align $min_alignment < $REQUIRED_PAGE_ALIGNMENT" >&2
        errors=$((errors + 1))
      fi
    fi

    # Resolvable dependency closure.
    needed_names="$(readelf -d "$library" | sed -n 's/.*NEEDED.*\[\(.*\)\].*/\1/p')"
    for needed in $needed_names; do
      if is_system_library "$needed"; then
        continue
      fi
      if [[ ! -f "$directory/$needed" ]]; then
        echo "    ERROR: $name needs '$needed', but no file with that name is shipped." >&2
        echo "           The Android dynamic linker resolves dependencies by the exact" >&2
        echo "           file name and would fail to start the process." >&2
        errors=$((errors + 1))
        continue
      fi
      # The shipped file must be the one that is actually loaded, so its SONAME
      # has to match the requested name.
      soname="$(readelf -d "$directory/$needed" | sed -n 's/.*SONAME.*\[\(.*\)\].*/\1/p')"
      if [[ -n "$soname" && "$soname" != "$needed" ]]; then
        echo "    ERROR: $name needs '$needed', but that file declares the SONAME '$soname'." >&2
        errors=$((errors + 1))
      fi
    done

    echo "      - $name ($(du -h "$library" | cut -f1))"
  done
}

# In APK mode the libraries are taken from the package itself, because that is
# what the device loads. A library that is missing from the package is missing
# at runtime, even when it sits in the build directory.
TEMP_DIR=""
if [[ -n "$APK_PATH" ]]; then
  if [[ ! -f "$APK_PATH" ]]; then
    echo "ERROR: The APK does not exist: $APK_PATH" >&2
    exit 1
  fi
  if ! command -v unzip >/dev/null 2>&1; then
    echo "ERROR: unzip is required to verify the packaged native libraries" >&2
    exit 1
  fi

  echo "    Native libraries packaged in $APK_PATH:"
  packaged_entries="$(unzip -Z1 "$APK_PATH" | grep -E '^lib/[^/]+/.*\.so$' || true)"
  if [[ -z "$packaged_entries" ]]; then
    echo "ERROR: The APK contains no native library at all." >&2
    exit 1
  fi

  # A versioned library must never be shipped: the packaging step only includes
  # files whose name ends in `.so`, and the loader resolves a dependency by the
  # exact file name.
  versioned="$(unzip -Z1 "$APK_PATH" | grep -E '^lib/[^/]+/.*\.so\.[0-9]+' || true)"
  if [[ -n "$versioned" ]]; then
    while IFS= read -r entry; do
      [[ -z "$entry" ]] && continue
      echo "    ERROR: The APK ships the versioned library '$entry'." >&2
      echo "           The packaging step only includes files ending in '.so'." >&2
      errors=$((errors + 1))
    done <<< "$versioned"
  fi

  TEMP_DIR="$(mktemp -d)"
  trap 'rm -rf "$TEMP_DIR"' EXIT
  unzip -q -o "$APK_PATH" 'lib/*/*.so' -d "$TEMP_DIR"

  abis="$(find "$TEMP_DIR/lib" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort)"
  if [[ -z "$abis" ]]; then
    echo "ERROR: The APK contains no native library directory." >&2
    exit 1
  fi
  for abi in $abis; do
    echo "    --- ABI $abi"
    for required in "${REQUIRED_LIBRARIES[@]}"; do
      if [[ ! -f "$TEMP_DIR/lib/$abi/$required" ]]; then
        echo "    ERROR: The required library '$required' is not packaged for $abi." >&2
        echo "           The application would fail to start on this device." >&2
        errors=$((errors + 1))
      fi
    done
    verify_directory "$TEMP_DIR/lib/$abi"
  done
else
  if [[ ! -d "$JNI_DIR" ]]; then
    if [[ "$AMBIENT_MODE" -eq 1 ]]; then
      echo "NOT RUN: The native libraries of Android are not built in this environment"
      echo "         ($JNI_DIR does not exist). They are built by"
      echo "         'scripts/build-android-rust.sh' and verified in the package by"
      echo "         'scripts/verify-android-native-libs.sh --apk <apk>'. They were NOT"
      echo "         verified here and must not be reported as verified."
      exit 0
    fi
    echo "ERROR: The native library directory does not exist: $JNI_DIR" >&2
    exit 1
  fi
  verify_directory "$JNI_DIR"
fi

if [[ $errors -gt 0 ]]; then
  echo "ERROR: $errors native library problem(s) found." >&2
  exit 1
fi

echo "✓ Native library closure resolves and every library is 16 KB page compatible."
