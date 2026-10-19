#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk}"
export ANDROID_HOME="${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}"
export PATH="$HOME/.cargo/bin:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools:${JAVA_HOME}/bin:$PATH"

if [[ ! -d "$ANDROID_HOME" ]]; then
  echo "ERROR: ANDROID_HOME not found at $ANDROID_HOME" >&2
  exit 1
fi

if [[ ! -d "${ANDROID_HOME}/ndk" ]]; then
  echo "ERROR: Android NDK not found in ${ANDROID_HOME}/ndk" >&2
  exit 1
fi

NDK_DIR="$(find "${ANDROID_HOME}/ndk" -maxdepth 1 -mindepth 1 | sort -V | tail -n1)"
TOOLCHAIN="${NDK_DIR}/toolchains/llvm/prebuilt/darwin-x86_64"

if [[ ! -x "${TOOLCHAIN}/bin/aarch64-linux-android34-clang" ]]; then
  echo "ERROR: aarch64-linux-android34-clang not found in ${TOOLCHAIN}/bin" >&2
  exit 1
fi

export CC_aarch64_linux_android="${TOOLCHAIN}/bin/aarch64-linux-android34-clang"
export CXX_aarch64_linux_android="${TOOLCHAIN}/bin/aarch64-linux-android34-clang++"
export AR_aarch64_linux_android="${TOOLCHAIN}/bin/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${TOOLCHAIN}/bin/aarch64-linux-android34-clang"

# 1. Deterministically ensure native Android Tesseract dependencies are extracted
"$REPO_ROOT/scripts/prepare-android-tesseract.sh"

# 2. Build native Android libvocument_ffi.so
echo "==> Compiling native Android libvocument_ffi.so (target: aarch64-linux-android)..."
cargo build -p vocument-ffi --target aarch64-linux-android

# 3. Deploy to Android app jniLibs
JNI_ARM64_DIR="$REPO_ROOT/apps/android/app/src/main/jniLibs/arm64-v8a"
mkdir -p "$JNI_ARM64_DIR"
cp "$REPO_ROOT/target/aarch64-linux-android/debug/libvocument_ffi.so" "$JNI_ARM64_DIR/libvocument_ffi.so"

if [[ ! -s "$JNI_ARM64_DIR/libvocument_ffi.so" ]]; then
  echo "ERROR: Output $JNI_ARM64_DIR/libvocument_ffi.so is missing or empty!" >&2
  exit 1
fi

echo "✓ Native libvocument_ffi.so rebuilt and deployed to $JNI_ARM64_DIR/libvocument_ffi.so ($(du -h "$JNI_ARM64_DIR/libvocument_ffi.so" | cut -f1))"
