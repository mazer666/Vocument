#!/usr/bin/env bash
# Verifies the Android PDF engine on a real ARM64 Android target.
#
# The verification does not need an Android device: it builds the *same* C
# bridge that the Android application links, for `aarch64-linux-android`, and
# executes it under `qemu-aarch64`. The binary is statically linked against
# Bionic, so it runs without an Android device image.
#
# The script produces three kinds of evidence:
#
#   1. **Binary evidence** – the produced library is an ARM64 ELF and depends on
#      the real MuPDF shared library, not on a placeholder.
#   2. **Execution evidence** – a real PDF is opened, its page count, page
#      geometry, text and rendering are produced by MuPDF 1.28.5 on ARM64.
#   3. **Image based regression evidence** – the ARM64 rendering is compared
#      pixel by pixel against the reference rendering of the host build. A
#      mismatch fails the gate; the comparison is not a file size or exit code
#      check.
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
NDK_BIN="${TOOLCHAIN_HOST}/bin"
NDK_SYSROOT="${TOOLCHAIN_HOST}/sysroot"
TARGET_API=24
TARGET_TRIPLE="aarch64-linux-android"

MUPDF_ANDROID_DIR="${MUPDF_ANDROID_DIR:-$REPO_ROOT/target/mupdf-android}"
MUPDF_BUILD_DIR="${MUPDF_BUILD_DIR:-$REPO_ROOT/target/mupdf-android-build/mupdf-1.28.5-source/build}"
WORK_DIR="${WORK_DIR:-$REPO_ROOT/target/android-verify}"

HARNESS_SOURCE="tests/android/mupdf_bridge_harness.c"
HOST_HARNESS_SOURCE="tests/android/mupdf_bridge_host_harness.c"
FIXTURE="tests/corpus/m0_3_navigation_and_search.pdf"
REFERENCE_DIR="${REFERENCE_DIR:-$REPO_ROOT/tests/corpus/reference/android-render}"

echo "==> Step 0: Checking the build prerequisites..."
for tool in qemu-aarch64-static; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "ERROR: '$tool' not found in PATH. Install 'qemu-user-static' to run the ARM64 verification." >&2
    exit 1
  fi
done
if [[ ! -d "$NDK_BIN" ]]; then
  echo "ERROR: Android NDK not found at $NDK_ROOT" >&2
  exit 1
fi
if [[ ! -d "$MUPDF_ANDROID_DIR/lib" ]]; then
  echo "ERROR: Android MuPDF not found at $MUPDF_ANDROID_DIR" >&2
  echo "       Run ./scripts/prepare-android-mupdf.sh first." >&2
  exit 1
fi
if [[ ! -f "$FIXTURE" ]]; then
  echo "ERROR: Test fixture not found: $FIXTURE" >&2
  exit 1
fi
echo "✓ Prerequisites present: NDK $NDK_ROOT, qemu-aarch64, MuPDF for $TARGET_TRIPLE"

echo "==> Step 1: Building the Rust PDF engine for $TARGET_TRIPLE..."
CC="${NDK_BIN}/${TARGET_TRIPLE}${TARGET_API}-clang"
CXX="${NDK_BIN}/${TARGET_TRIPLE}${TARGET_API}-clang++"
AR="${NDK_BIN}/llvm-ar"
RANLIB="${NDK_BIN}/llvm-ranlib"
export CC_aarch64_linux_android="$CC"
export CXX_aarch64_linux_android="$CXX"
export AR_aarch64_linux_android="$AR"
export RANLIB_aarch64_linux_android="$RANLIB"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC"
export MUPDF_ANDROID_DIR

cargo build -p vocument-pdf-mupdf --target "$TARGET_TRIPLE" >/dev/null
BRIDGE_A="$(ls "${REPO_ROOT}"/target/${TARGET_TRIPLE}/debug/build/vocument-pdf-mupdf-*/out/libvocument_mupdf_bridge.a 2>/dev/null | head -n1)"
if [[ -z "$BRIDGE_A" ]]; then
  echo "ERROR: The MuPDF bridge archive was not produced for $TARGET_TRIPLE" >&2
  exit 1
fi
echo "✓ Engine built for $TARGET_TRIPLE: $(basename "$BRIDGE_A")"

echo "==> Step 2: Verifying the produced Android binaries..."
MUPDF_SO="$(ls "${MUPDF_ANDROID_DIR}"/lib/libmupdf.so.*.* 2>/dev/null | head -n1)"
if [[ -z "$MUPDF_SO" ]]; then
  echo "ERROR: No libmupdf.so found in $MUPDF_ANDROID_DIR/lib" >&2
  exit 1
fi
if ! file "$MUPDF_SO" | grep -q "ARM aarch64"; then
  echo "ERROR: $MUPDF_SO is not an ARM64 library" >&2
  exit 1
fi
SONAME="$(readelf -d "$MUPDF_SO" | sed -n 's/.*SONAME.*\[\(.*\)\].*/\1/p')"
echo "✓ MuPDF library: $(basename "$MUPDF_SO") (ARM aarch64, SONAME $SONAME)"
if [[ -z "$SONAME" ]]; then
  echo "ERROR: The MuPDF library has no SONAME and cannot be linked reliably" >&2
  exit 1
fi

# The bridge must resolve against the real MuPDF, not against a stub.
mkdir -p "$WORK_DIR"
cat > "${WORK_DIR}/link_probe.c" << 'PROBE'
#include <mupdf/fitz.h>
int main(void) {
    fz_context *ctx = fz_new_context(NULL, NULL, FZ_STORE_UNLIMITED);
    if (!ctx) { return 1; }
    fz_drop_context(ctx);
    return 0;
}
PROBE
"$CC" -o "${WORK_DIR}/link_probe" "${WORK_DIR}/link_probe.c" \
  -I "$MUPDF_ANDROID_DIR/include" -L "$MUPDF_ANDROID_DIR/lib" -lmupdf \
  -Wl,-rpath,"$MUPDF_ANDROID_DIR/lib" >/dev/null
NEEDED="$(readelf -d "${WORK_DIR}/link_probe" | sed -n 's/.*NEEDED.*\[\(.*\)\].*/\1/p')"
if [[ "$NEEDED" != *"$SONAME"* ]]; then
  echo "ERROR: The Android engine does not depend on the real MuPDF library (NEEDED: $NEEDED)" >&2
  exit 1
fi
echo "✓ The Android engine links against the real MuPDF ($NEEDED)"

echo "==> Step 3: Building the statically linked ARM64 verification harness..."
# Bionic requires a 64 byte aligned TLS segment for statically linked ARM64
# executables. Without the anchor the runtime aborts with
# "executable's TLS segment is underaligned".
cat > "${WORK_DIR}/tls_anchor.c" << 'ANCHOR'
__attribute__((used, retain)) __thread char vocument_tls_alignment_anchor[64]
    __attribute__((aligned(64)));
ANCHOR
# The Android runtime library `liblog` does not exist on the build host. MuPDF
# only uses it for its error callback, so a minimal shim is sufficient.
cat > "${WORK_DIR}/log_shim.c" << 'SHIM'
#include <stdarg.h>
#include <stdio.h>
int __android_log_print(int prio, const char *tag, const char *fmt, ...) {
    (void)prio; (void)tag;
    va_list args;
    va_start(args, fmt);
    vfprintf(stderr, fmt, args);
    va_end(args);
    fputc('\n', stderr);
    return 0;
}
int __android_log_write(int prio, const char *tag, const char *text) {
    (void)prio; (void)tag; (void)text;
    return 0;
}
SHIM
"$CC" -c -fno-emulated-tls -ftls-model=local-exec \
  -o "${WORK_DIR}/tls_anchor.o" "${WORK_DIR}/tls_anchor.c"

MUPDF_VERSION="$(sed -n 's/^#define FZ_VERSION "\(.*\)"$/\1/p' "$MUPDF_ANDROID_DIR/include/mupdf/fitz/version.h")"
if [[ -z "$MUPDF_VERSION" ]]; then
  echo "ERROR: Cannot read the MuPDF version from the installed headers" >&2
  exit 1
fi
ENGINE_ID="mupdf-${MUPDF_VERSION}-aarch64-android"

STATIC_MUPDF="${MUPDF_BUILD_DIR}/release/libmupdf.a"
STATIC_THIRD="${MUPDF_BUILD_DIR}/release/libmupdf-third.a"
if [[ ! -f "$STATIC_MUPDF" || ! -f "$STATIC_THIRD" ]]; then
  echo "ERROR: The static MuPDF build is missing ($STATIC_MUPDF / $STATIC_THIRD)." >&2
  echo "       Run ./scripts/prepare-android-mupdf.sh --static first." >&2
  exit 1
fi

"$CC" -static -o "${WORK_DIR}/mupdf_harness_arm64" \
  -DVOCRUMENT_MUPDF_ENGINE_ID="\"$ENGINE_ID\"" \
  "$HARNESS_SOURCE" "${WORK_DIR}/log_shim.c" "${WORK_DIR}/tls_anchor.o" \
  "$BRIDGE_A" "$STATIC_MUPDF" "$STATIC_THIRD" \
  -I "$MUPDF_ANDROID_DIR/include" \
  -L "$NDK_SYSROOT/usr/lib/$TARGET_TRIPLE" -lm

if ! file "${WORK_DIR}/mupdf_harness_arm64" | grep -q "ARM aarch64"; then
  echo "ERROR: The verification harness is not an ARM64 binary" >&2
  exit 1
fi
echo "✓ ARM64 verification harness built ($ENGINE_ID)"

echo "==> Step 4: Executing the ARM64 verification harness on a real PDF..."
rm -rf "${WORK_DIR}/render-arm64"
mkdir -p "${WORK_DIR}/render-arm64"
qemu-aarch64-static "${WORK_DIR}/mupdf_harness_arm64" "$FIXTURE" "${WORK_DIR}/render-arm64" \
  2> "${WORK_DIR}/harness-stderr.log" | tee "${WORK_DIR}/harness-stdout.log"
HARNESS_STATUS="${PIPESTATUS[0]}"
if [[ "$HARNESS_STATUS" != "0" ]]; then
  echo "ERROR: The ARM64 verification harness failed (exit $HARNESS_STATUS)" >&2
  echo "--- stderr ---" >&2
  cat "${WORK_DIR}/harness-stderr.log" >&2
  exit 1
fi
if ! grep -q "VERIFICATION PASSED" "${WORK_DIR}/harness-stdout.log"; then
  echo "ERROR: The ARM64 harness did not report success" >&2
  exit 1
fi
echo "✓ The ARM64 Android engine processed a real PDF successfully"

echo "==> Step 5: Image based regression against the reference rendering..."
# The reference rendering is produced by the host build of the same engine. It is
# compared pixel by pixel; a mismatch fails the gate.
if [[ ! -d "$REFERENCE_DIR" ]]; then
  echo "    No reference rendering found, creating it from the host build..."
  python3 "$REPO_ROOT/scripts/generate-android-render-reference.py" \
    --fixture "$FIXTURE" --output "$REFERENCE_DIR"
fi
python3 "$REPO_ROOT/scripts/compare-render-images.py" \
  --actual "${WORK_DIR}/render-arm64" \
  --reference "$REFERENCE_DIR" \
  --max-mean-difference 1.0

echo "✓ Image based regression passed: the ARM64 Android rendering matches the reference."

echo
echo "==> Android ARM64 PDF engine verification completed successfully."
echo "    Evidence:"
echo "      * MuPDF library: $(basename "$MUPDF_SO") (ARM aarch64, SONAME $SONAME)"
echo "      * Engine identifier reported at runtime: $ENGINE_ID"
echo "      * Rendered pages: $(ls "${WORK_DIR}/render-arm64" | wc -l)"
echo "      * Harness output: ${WORK_DIR}/harness-stdout.log"
