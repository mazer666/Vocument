#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

./scripts/validate-repo.sh

echo "==> Running Cargo workspace checks..."
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

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

if command -v kotlinc >/dev/null 2>&1; then
  echo "==> Building and running Android Kotlin → UniFFI → Rust handshake..."
  ./scripts/verify-kotlin-handshake.sh
fi

echo "==> Local CI completed successfully."
