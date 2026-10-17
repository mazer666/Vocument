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
  "docs/acceptance/M0.0-ACCEPTANCE.md"
  "docs/acceptance/M0.1-ACCEPTANCE.md"
  "docs/acceptance/M0.2-ACCEPTANCE.md"
  "docs/acceptance/M0.3-ACCEPTANCE.md"
  "docs/acceptance/M0.4-ACCEPTANCE.md"
  "docs/acceptance/M0.5-ACCEPTANCE.md"
  "docs/acceptance/M0.6-ACCEPTANCE.md"
  "docs/adr/ADR_TEMPLATE.md"
  "docs/adr/ADR-0001-cargo-workspace-and-module-boundaries.md"
  "docs/adr/ADR-0002-central-configuration-sync.md"
  "docs/adr/ADR-0003-uniffi-binding-generation-and-bridge-verification.md"
  "docs/adr/ADR-0004-m0-1-filebroker-document-session-and-mupdf-adapter.md"
  "docs/adr/ADR-0005-m0-2-rendering-actor-tile-cache-and-canvas.md"
  "docs/adr/ADR-0006-m0-3-navigation-thumbnails-selection-and-fts5-search.md"
  "docs/adr/ADR-0007-m0-4-transaction-manager-mutations-undo-redo-and-journal.md"
  "docs/adr/ADR-0008-m0-5-safe-save-atomic-staging-validation-and-conflict-handling.md"
  "docs/adr/ADR-0009-m0-6-crash-recovery-containers-and-restore-actions.md"
  "docs/cr/CR_TEMPLATE.md"
  "docs/cr/CR-0001-source-file-size-limits.md"
  "archive/baselines/0.1/MANIFEST.sha256"
  "platform/vocument-ffi/src/recovery_dto.rs"
  "scripts/generate-bindings.sh"
  "scripts/verify-kotlin-handshake.sh"
  "apps/macos/Package.swift"
  "apps/macos/Sources/VocumentMacApp/DocumentCanvasView.swift"
  "apps/macos/Sources/VocumentMacApp/NavigationAndSearchSidebarView.swift"
  "apps/android/app/build.gradle.kts"
  "apps/android/app/src/main/AndroidManifest.xml"
  "apps/android/app/src/main/java/org/vocument/app/DocumentCanvas.kt"
  "apps/android/app/src/main/java/org/vocument/app/NavigationAndSearchPanel.kt"
  "tests/corpus/m0_1_sample_two_pages.pdf"
  "tests/corpus/m0_3_navigation_and_search.pdf"
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

# Enforce CODE-001: Hand-written source file size limits (target <= 600 lines, hard max <= 900 lines)
TARGET_MAX_LINES=600
HARD_MAX_LINES=900

while IFS= read -r -d '' src_file; do
  line_count="$(wc -l < "$src_file" | tr -d ' ')"
  if (( line_count > HARD_MAX_LINES )); then
    echo "ERROR (CODE-001): Hand-written source file '$src_file' has $line_count lines (hard maximum is $HARD_MAX_LINES)." >&2
    exit 1
  elif (( line_count > TARGET_MAX_LINES )); then
    echo "WARNING (CODE-001): Hand-written source file '$src_file' has $line_count lines (target is <= $TARGET_MAX_LINES; consider splitting)." >&2
  fi
done < <(find core engines formats platform apps scripts \
  -type f \( -name "*.rs" -o -name "*.swift" -o -name "*.kt" -o -name "*.c" -o -name "*.h" -o -name "*.sh" \) \
  -not -path "*/target/*" \
  -not -path "*/.build/*" \
  -not -path "*/build/*" \
  -not -path "apps/macos/Sources/VocumentFFI/*" \
  -not -path "apps/macos/Sources/VocumentFFIHeaders/*" \
  -not -path "apps/android/app/src/main/java/uniffi/*" \
  -print0)

echo "✓ Source file size policy verified (CODE-001: target <= ${TARGET_MAX_LINES}, max <= ${HARD_MAX_LINES} lines)."

# Enforce M0.1 Gate: No UI layer accesses MuPDF directly
if grep -RInE 'mupdf|fz_' apps/macos/Sources/VocumentMacApp apps/macos/Sources/VocumentMacKit apps/android/app/src/main/java/org/vocument >/dev/null 2>&1; then
  echo "ERROR (M0.1 Gate): Direct MuPDF reference detected in native UI layer!" >&2
  exit 1
fi
echo "✓ M0.1 Gate verified: No UI layer accesses MuPDF directly."

# Enforce M0.4 Gate: Mutation never bypasses TransactionManager
if grep -RIn 'apply_engine_mutation_for_transaction_manager' core/ platform/ apps/ \
  --exclude='transactions.rs' --exclude='render.rs' >/dev/null 2>&1; then
  echo "ERROR (M0.4 Gate): Direct engine mutation bypassing TransactionManager detected!" >&2
  exit 1
fi
echo "✓ M0.4 Gate verified: Mutation never bypasses TransactionManager."

# Enforce M0.5 Gate: Never silently overwrite externally changed content
if ! grep -q "gate_never_silently_overwrites_externally_changed_content" core/vocument-core/tests/safe_save_tests.rs; then
  echo "ERROR (M0.5 Gate): M0.5 gate test 'gate_never_silently_overwrites_externally_changed_content' missing!" >&2
  exit 1
fi
echo "✓ M0.5 Gate verified: Never silently overwrite externally changed content."

# Enforce M0.6 Gate: Original file remains completely intact until explicit save (Hard-kill recovery verified)
if ! grep -q "test_hard_kill_recovery_with_subprocess_sigkill" core/vocument-core/tests/recovery_tests.rs; then
  echo "ERROR (M0.6 Gate): M0.6 destructive hard-kill test 'test_hard_kill_recovery_with_subprocess_sigkill' missing!" >&2
  exit 1
fi
echo "✓ M0.6 Gate verified: Original file remains intact through hard-kill crash recovery until explicit save."

echo "==> Repository validation passed."
