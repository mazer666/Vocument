#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

./scripts/validate-repo.sh

echo "==> Running Cargo workspace checks..."
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

echo "==> Verifying the secure redaction (M2.1): the content is removed, the removal is verified, the undo is refused..."
cargo test -p vocument-pdf-mupdf redaction_tests
cargo test -p vocument-ffi --test scan_workflow_e2e_tests a_redaction

echo "==> Verifying the inspection of the links and the actions of a document (M2.2)..."
cargo test -p vocument-pdf-mupdf inspection_tests
cargo test -p vocument-core document_inspection
cargo test -p vocument-ffi --test document_inspection_e2e_tests

echo "==> Verifying the inspection of the attachments and the signatures of a document (M2.3)..."
cargo test -p vocument-pdf-mupdf attachment_signature_tests
cargo test -p vocument-ffi --test document_inspection_e2e_tests

echo "==> Generating and verifying UniFFI bindings (Swift & Kotlin)..."
BINDING_HASH_BEFORE="$(shasum -a 256 apps/macos/Sources/VocumentFFI/* apps/macos/Sources/VocumentFFIHeaders/include/* apps/android/app/src/main/java/uniffi/vocument_ffi/* 2>/dev/null || true)"
./scripts/generate-bindings.sh
BINDING_HASH_AFTER="$(shasum -a 256 apps/macos/Sources/VocumentFFI/* apps/macos/Sources/VocumentFFIHeaders/include/* apps/android/app/src/main/java/uniffi/vocument_ffi/*)"

if [[ "$BINDING_HASH_BEFORE" != "$BINDING_HASH_AFTER" ]]; then
  echo "ERROR: Generated UniFFI bindings were out of sync with Rust source." >&2
  echo "Run './scripts/generate-bindings.sh' and commit the regenerated files." >&2
  exit 1
fi
echo "✓ Generated UniFFI bindings are clean and in sync."

if [[ -f "apps/macos/Package.swift" ]] && command -v swift >/dev/null 2>&1; then
  echo "==> Building macOS Swift targets (VocumentMacApp & VocumentMacHandshakeCLI) and running Swift → UniFFI → Rust handshake..."
  swift build --package-path apps/macos --product VocumentMacApp
  swift run --package-path apps/macos vocument-mac-handshake
fi

echo "==> Verifying the native Android libraries (dependency closure, SONAME, 16 KB page alignment)..."
./scripts/verify-android-native-libs.sh

echo "==> Verifying the Android PDF engine on a real ARM64 target (qemu-aarch64)..."
if command -v qemu-aarch64-static >/dev/null 2>&1 || [[ -x /usr/bin/qemu-aarch64-static ]]; then
  ./scripts/verify-android-arm64-pdf.sh
else
  echo "SKIPPED: qemu-aarch64-static is not available on this runner."
  echo "         The ARM64 engine proof is NOT RUN here and must not be reported as passed."
fi

echo "==> Running real Android runtime & scan-capture verification (Gradle assembleDebug, apkanalyzer, unit tests, and Kotlin handshake)..."
./scripts/verify-android-runtime.sh

echo "==> Local CI completed successfully."
