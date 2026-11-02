#!/usr/bin/env bash
# M4.5 DOCX & ODT Interoperability Corpus Bake-Off Runner (`ADR-0020`, `ROADMAP.md` M4.5, `OFFICE-001`–`005`).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$WORKSPACE_ROOT"

echo "==> Running vocument-office unit & integration test suite..."
cargo test -p vocument-office

echo "==> Running M4.5 real-document DOCX & ODT bake-off..."
cargo run -p vocument-office --example bakeoff -- "$@"

