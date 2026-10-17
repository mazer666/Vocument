#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

./scripts/validate-repo.sh

if [[ -f "Cargo.toml" ]]; then
  echo "==> Running Cargo workspace checks..."
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
fi

echo "==> Local CI completed successfully."
