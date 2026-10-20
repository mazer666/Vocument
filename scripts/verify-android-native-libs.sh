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
set -euo pipefail

JNI_DIR="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/apps/android/app/src/main/jniLibs/arm64-v8a}"
REQUIRED_PAGE_ALIGNMENT="${REQUIRED_PAGE_ALIGNMENT:-0x4000}"

if [[ ! -d "$JNI_DIR" ]]; then
  echo "ERROR: The native library directory does not exist: $JNI_DIR" >&2
  exit 1
fi

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

echo "    Native libraries in $JNI_DIR:"
shopt -s nullglob
libraries=("$JNI_DIR"/*.so)
shopt -u nullglob
if [[ ${#libraries[@]} -eq 0 ]]; then
  echo "ERROR: No native libraries found in $JNI_DIR" >&2
  exit 1
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
    if [[ ! -f "$JNI_DIR/$needed" ]]; then
      echo "    ERROR: $name needs '$needed', but the APK contains no file with that name." >&2
      echo "           The Android dynamic linker resolves dependencies by the exact" >&2
      echo "           file name and would fail to start the process." >&2
      errors=$((errors + 1))
      continue
    fi
    # The shipped file must be the one that is actually loaded, so its SONAME
    # has to match the requested name.
    soname="$(readelf -d "$JNI_DIR/$needed" | sed -n 's/.*SONAME.*\[\(.*\)\].*/\1/p')"
    if [[ -n "$soname" && "$soname" != "$needed" ]]; then
      echo "    ERROR: $name needs '$needed', but that file declares the SONAME '$soname'." >&2
      errors=$((errors + 1))
    fi
  done

  echo "      - $name ($(du -h "$library" | cut -f1))"
done

if [[ $errors -gt 0 ]]; then
  echo "ERROR: $errors native library problem(s) found." >&2
  exit 1
fi

echo "✓ Native library closure resolves and every library is 16 KB page compatible."
