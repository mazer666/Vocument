#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "==> Validating Vocument repository structure and baseline..."

REQUIRED_FILES=(
  "README.md"
  "ARCHITECTURE.md"
  "ROADMAP.md"
  "REQUIREMENTS.md"
  "AGENTS.md"
  "PROJECT_MANAGEMENT.md"
  "QUALITY.md"
  "WHERE_TO_CHANGE.md"
  "GLOSSARY.md"
  "CONTRIBUTING.md"
  "SECURITY.md"
  "THIRD_PARTY.md"
  "CHANGELOG.md"
  "VERSION"
  "LICENSE"
  "REUSE.toml"
  "LICENSES/AGPL-3.0-or-later.txt"
  "rust-toolchain.toml"
  "deny.toml"
  "Cargo.toml"
  "Makefile"
  "project/project.yaml"
  "config/README.md"
  "config/app-defaults.toml"
  "config/performance-budgets.toml"
  "config/security-policy.toml"
  "docs/README.md"
  "docs/BEST_IN_CLASS.md"
  "docs/DOCUMENTATION_POLICY.md"
  "docs/project/RISKS.md"
  "docs/features/FEATURE_TEMPLATE.md"
  "docs/acceptance/ACCEPTANCE_TEMPLATE.md"
  "docs/adr/ADR_TEMPLATE.md"
  "docs/adr/ADR-0001-cargo-workspace-and-module-boundaries.md"
  "docs/adr/ADR-0002-central-configuration-sync.md"
  "docs/cr/CR_TEMPLATE.md"
  "archive/baselines/0.1/MANIFEST.sha256"
)

for f in "${REQUIRED_FILES[@]}"; do
  if [[ ! -f "$f" ]]; then
    echo "ERROR: Missing required file: $f" >&2
    exit 1
  fi
done

echo "✓ All required baseline, governance, and workspace files exist."

# Verify GitHub Actions workflows never use GitHub-hosted runners
if [[ -d ".github/workflows" ]]; then
  if grep -RInE 'runs-on:\s*(ubuntu|macos|windows)-' .github/workflows/ >/dev/null 2>&1; then
    echo "ERROR: GitHub-hosted runner detected in .github/workflows/ (must use runs-on: self-hosted)" >&2
    exit 1
  fi
  echo "✓ Workflow runner policy verified (self-hosted only)."
fi

echo "==> Repository validation passed."
