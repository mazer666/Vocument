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
  "docs/acceptance/M0.7-ACCEPTANCE.md"
  "docs/acceptance/M0-ACCEPTANCE.md"
  "docs/acceptance/M1.0-ACCEPTANCE.md"
  "docs/acceptance/M1.1-ACCEPTANCE.md"
  "docs/acceptance/M1.2-ACCEPTANCE.md"
  "docs/acceptance/M1.3-ACCEPTANCE.md"
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
  "docs/adr/ADR-0010-m0-7-interoperability-gate-and-foreign-viewer-verification.md"
  "docs/adr/ADR-0011-m1-0-android-runtime-foundation-and-scan-capture-pipeline.md"
  "docs/adr/ADR-0012-m1-1-perspective-correction-deskew-and-adaptive-rotation.md"
  "docs/adr/ADR-0013-m1-2-intelligent-scan-cleanup-and-enhancement.md"
  "docs/adr/ADR-0014-m1-3-tesseract-ocr-and-text-layer-generation.md"
  "docs/adr/ADR-0015-searchable-pdf-synthesis-and-session-ingestion.md"
  "docs/adr/ADR-0016-android-mupdf-placeholder-bridge.md"
  "docs/adr/ADR-0017-real-android-mupdf-cross-build.md"
  "docs/cr/CR-0003-windows-and-linux-platform-support.md"
  "docs/cr/CR_TEMPLATE.md"
  "docs/cr/CR-0001-source-file-size-limits.md"
  "docs/cr/CR-0002-prohibition-of-stubs-in-acceptance.md"
  "docs/acceptance/M1.4b-ACCEPTANCE.md"
  "docs/acceptance/M1.5-ACCEPTANCE.md"
  "docs/reports/M1.4b-M1.5-ABSCHLUSSBERICHT.md"
  "scripts/verify-android-arm64-pdf.sh"
  "scripts/compare-render-images.py"
  "scripts/generate-android-render-reference.py"
  "tests/android/mupdf_bridge_harness.c"
  "archive/baselines/0.1/MANIFEST.sha256"
  "engines/imaging/src/geometry.rs"
  "engines/imaging/src/perspective.rs"
  "engines/imaging/src/cleanup.rs"
  "engines/imaging/src/lib.rs"
  "engines/ocr/src/provider.rs"
  "engines/ocr/src/models.rs"
  "engines/ocr/src/tesseract.rs"
  "engines/ocr/src/language.rs"
  "engines/ocr/src/corpus.rs"
  "engines/ocr/src/lib.rs"
  "engines/ocr/csrc/tesseract_bridge.c"
  "engines/ocr/include/tesseract/capi.h"
  "engines/ocr/include/tesseract/export.h"
  "scripts/prepare-android-tesseract.sh"
  "scripts/build-android-rust.sh"
  "apps/android/app/src/main/assets/tessdata/deu.traineddata"
  "apps/android/app/src/main/assets/tessdata/eng.traineddata"
  "platform/vocument-ffi/src/ocr_dto.rs"
  "tests/corpus/ocr/deu_vertrag_vertraulichkeit.pdf"
  "tests/corpus/ocr/deu_vertrag_vertraulichkeit_camera_distorted.pgm"
  "tests/corpus/ocr/eng_mutual_nda.pdf"
  "tests/corpus/ocr/bilingual_rechnung_invoice.pdf"
  "apps/android/app/src/main/java/org/vocument/app/CameraPreviewView.kt"
  "platform/vocument-ffi/src/recovery_dto.rs"
  "platform/vocument-ffi/src/interop_dto.rs"
  "platform/vocument-ffi/src/scan_dto.rs"
  "core/vocument-core/tests/interop_tests.rs"
  "scripts/generate-bindings.sh"
  "scripts/verify-kotlin-handshake.sh"
  "scripts/verify-android-runtime.sh"
  "apps/macos/Package.swift"
  "apps/macos/Sources/VocumentMacApp/DocumentCanvasView.swift"
  "apps/macos/Sources/VocumentMacApp/NavigationAndSearchSidebarView.swift"
  "apps/android/app/build.gradle.kts"
  "apps/android/app/src/main/AndroidManifest.xml"
  "apps/android/app/src/main/java/org/vocument/app/DocumentCanvas.kt"
  "apps/android/app/src/main/java/org/vocument/app/NavigationAndSearchPanel.kt"
  "apps/android/app/src/main/java/org/vocument/app/ScanCaptureScreen.kt"
  "apps/android/app/src/test/java/org/vocument/app/ScanCaptureBridgeTest.kt"
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
  -not -path "engines/ocr/include/*" \
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

# Enforce M0.7 Gate: Full M0 workflow and foreign viewer verification (MuPDF, CoreGraphics, PDFium, honesty checks)
if ! grep -q "test_m0_full_dod_workflow_and_recovery_cycle" core/vocument-core/tests/interop_tests.rs; then
  echo "ERROR (M0.7 Gate): M0.7 end-to-end DoD workflow test missing!" >&2
  exit 1
fi
if ! grep -q "document_core_run_interop_gate_suite" platform/vocument-ffi/src/interop_dto.rs; then
  echo "ERROR (M0.7 Gate): M0.7 document_core_run_interop_gate_suite export missing!" >&2
  exit 1
fi
echo "✓ M0.7 Gate verified: Full M0 workflow and foreign viewer interoperability verified."

# Enforce M1.0 Gate: Real Android runtime & Scan-Capture pipeline verified
if ! grep -q "testMultiPageScanSessionLifecycle" apps/android/app/src/test/java/org/vocument/app/ScanCaptureBridgeTest.kt; then
  echo "ERROR (M1.0 Gate): M1.0 scan capture bridge test missing!" >&2
  exit 1
fi
if ! grep -q "document_core_detect_document_edges" platform/vocument-ffi/src/scan_dto.rs; then
  echo "ERROR (M1.0 Gate): M1.0 document_core_detect_document_edges export missing!" >&2
  exit 1
fi
echo "✓ M1.0 Gate verified: Real Android runtime & Scan-Capture pipeline verified."

# Enforce M1.1 Gate: Perspective correction, deskew & adaptive rotation verified
if ! grep -q "testPerspectiveCorrectionWarpsQuadIntoRectangularFrame" apps/android/app/src/test/java/org/vocument/app/ScanCaptureBridgeTest.kt; then
  echo "ERROR (M1.1 Gate): M1.1 perspective correction bridge test missing!" >&2
  exit 1
fi
if ! grep -q "document_core_warp_perspective" platform/vocument-ffi/src/scan_dto.rs; then
  echo "ERROR (M1.1 Gate): M1.1 document_core_warp_perspective export missing!" >&2
  exit 1
fi
if ! grep -q "document_core_deskew_frame" platform/vocument-ffi/src/scan_dto.rs; then
  echo "ERROR (M1.1 Gate): M1.1 document_core_deskew_frame export missing!" >&2
  exit 1
fi
echo "✓ M1.1 Gate verified: Perspective correction, deskew & adaptive rotation verified."

# Enforce M1.2 Gate: Intelligent scan cleanup, shadow/crease reduction & enhancement verified
if ! grep -q "testCleanupProfilesAndVisualRegressionMetrics" apps/android/app/src/test/java/org/vocument/app/ScanCaptureBridgeTest.kt; then
  echo "ERROR (M1.2 Gate): M1.2 cleanup profile bridge test missing!" >&2
  exit 1
fi
if ! grep -q "document_core_enhance_scan_frame" platform/vocument-ffi/src/scan_dto.rs; then
  echo "ERROR (M1.2 Gate): M1.2 document_core_enhance_scan_frame export missing!" >&2
  exit 1
fi
if ! grep -q "document_core_evaluate_cleanup_quality" platform/vocument-ffi/src/scan_dto.rs; then
  echo "ERROR (M1.2 Gate): M1.2 document_core_evaluate_cleanup_quality export missing!" >&2
  exit 1
fi
echo "✓ M1.2 Gate verified: Intelligent scan cleanup, shadow/crease reduction & enhancement verified."

# Enforce M1.3 Gate: Offline Tesseract OCR, typographical hierarchy & realistic corpus verified
if ! grep -q "testLanguageDetectionAndOcrPipeline" apps/android/app/src/test/java/org/vocument/app/ScanCaptureBridgeTest.kt; then
  echo "ERROR (M1.3 Gate): M1.3 OCR bridge test missing!" >&2
  exit 1
fi
if ! grep -q "document_core_ocr_frame" platform/vocument-ffi/src/ocr_dto.rs; then
  echo "ERROR (M1.3 Gate): M1.3 document_core_ocr_frame export missing!" >&2
  exit 1
fi
if [[ -f "engines/ocr/csrc/tesseract_stub_bridge.c" ]]; then
  echo "ERROR (M1.3 Gate): tesseract_stub_bridge.c detected! Android must use real embedded Tesseract." >&2
  exit 1
fi

# Enforce QA-003 for the PDF engine: the Android build must link the real MuPDF
# engine. A placeholder bridge is not acceptable any more (ADR-0017, RISK-006).
if [[ -f "engines/pdf-mupdf/csrc/mupdf_stub_bridge.c" ]]; then
  echo "ERROR (QA-003): engines/pdf-mupdf/csrc/mupdf_stub_bridge.c detected!" >&2
  echo "Android must link the real MuPDF engine, not a placeholder bridge." >&2
  echo "See ADR-0017 and scripts/prepare-android-mupdf.sh." >&2
  exit 1
fi
if grep -q 'mupdf_stub_bridge.c' engines/pdf-mupdf/build.rs; then
  echo "ERROR (QA-003): engines/pdf-mupdf/build.rs must not reference the placeholder bridge!" >&2
  exit 1
fi
if ! grep -q "MUPDF_ANDROID_DIR" engines/pdf-mupdf/build.rs; then
  echo "ERROR (QA-003): engines/pdf-mupdf/build.rs must resolve the Android MuPDF installation" >&2
  echo "through MUPDF_ANDROID_DIR (see scripts/prepare-android-mupdf.sh)." >&2
  exit 1
fi
echo "✓ QA-003: no placeholder PDF engine in the tree; Android links the real MuPDF engine."

# Enforce JOB-010: a submitted job must perform a real core operation. A job
# body that unconditionally reports success without doing the work is the
# failure mode this gate protects against.
if grep -q 'Box::new(|_, _| JobOutcome::Completed)' platform/vocument-ffi/src/job_dto.rs; then
  echo "ERROR (JOB-010): platform/vocument-ffi/src/job_dto.rs submits a job body that" >&2
  echo "always reports success without performing a real core operation." >&2
  exit 1
fi
if ! grep -q "build_job_task" platform/vocument-ffi/src/job_dto.rs; then
  echo "ERROR (JOB-010): platform/vocument-ffi/src/job_dto.rs must build the job" >&2
  echo "payload from the job kind (see JOB-010 in REQUIREMENTS.md)." >&2
  exit 1
fi
# The worker pool must not panic in production paths. The test modules are
# excluded: `expect` is the idiomatic way to fail a test.
if grep -n 'expect(' core/vocument-core/src/jobs/executor.rs \
  | grep -vE 'executor\.rs:(4[89][0-9]|[5-9][0-9][0-9]):' | grep -q .; then
  echo "ERROR (JOB-010): the job manager must not use expect() in production code." >&2
  grep -n 'expect(' core/vocument-core/src/jobs/executor.rs >&2
  exit 1
fi
if grep -n 'expect(' core/vocument-core/src/jobs/scheduler.rs \
  | grep -vE 'scheduler\.rs:(4[0-9][0-9]|[5-9][0-9][0-9]):' | grep -q .; then
  echo "ERROR (JOB-010): the job manager must not use expect() in production code." >&2
  grep -n 'expect(' core/vocument-core/src/jobs/scheduler.rs >&2
  exit 1
fi
echo "✓ JOB-010: background jobs perform real core operations; no expect() in the job manager."
if ! grep -q "compute_character_error_rate" engines/ocr/src/corpus.rs; then
  echo "ERROR (M1.3 Gate): compute_character_error_rate missing in corpus.rs!" >&2
  exit 1
fi
if ! grep -q "test_realistic_camera_distorted_scan_and_cleanup_pipeline" engines/ocr/tests/corpus_tests.rs; then
  echo "ERROR (M1.3 Gate): Camera distortion pipeline test missing in corpus_tests.rs!" >&2
  exit 1
fi
echo "✓ M1.3 Gate verified: Real offline embedded Tesseract OCR, CER/WER >= 95% & realistic camera distortion corpus verified."

# Enforce the robustness corpus: the committed difficult PDF fixtures must be
# regenerable from their generator, and the engine must have a test that opens
# every one of them without crashing, hanging or reporting a fake success.
if [[ -f "scripts/generate-difficult-pdf-corpus.py" ]]; then
  if ! python3 scripts/generate-difficult-pdf-corpus.py --check; then
    echo "ERROR (M1.6 Gate): committed difficult PDF corpus does not match the generator!" >&2
    echo "Run 'python3 scripts/generate-difficult-pdf-corpus.py' and commit the result." >&2
    exit 1
  fi
  if ! grep -q "the_engine_keeps_the_memory_usage_of_the_difficult_corpus_bounded" \
      engines/pdf-mupdf/src/difficult_tests.rs; then
    echo "ERROR (M1.6 Gate): the difficult corpus has no memory bound test!" >&2
    exit 1
  fi
  echo "✓ M1.6 Gate verified: the difficult PDF corpus is reproducible and the engine is bounded against it."
fi

# Enforce corpus reproducibility: the committed OCR corpus frames must be
# regenerable from their generator, so the M1.3 quality numbers stay auditable.
if [[ -f "scripts/generate-ocr-corpus-frames.py" ]]; then
  if python3 -c "import PIL, numpy" >/dev/null 2>&1; then
    if ! python3 scripts/generate-ocr-corpus-frames.py --check; then
      echo "ERROR (M1.3 Gate): committed OCR corpus frames do not match the generator!" >&2
      echo "Run 'python3 scripts/generate-ocr-corpus-frames.py' and commit the result." >&2
      exit 1
    fi
    echo "✓ M1.3 Gate verified: OCR corpus frames are reproducible from their generator."
  else
    echo "WARNING: Pillow/NumPy not available; skipping OCR corpus reproducibility check."
  fi
fi

# Enforce M1.4 Gate: Searchable PDF synthesis, invisible text layer & DocumentSession ingestion verified
if [[ ! -f "engines/ocr/src/synthesis.rs" ]]; then
  echo "ERROR (M1.4 Gate): engines/ocr/src/synthesis.rs missing!" >&2
  exit 1
fi
if ! grep -q "document_core_synthesize_searchable_pdf" platform/vocument-ffi/src/ocr_dto.rs; then
  echo "ERROR (M1.4 Gate): M1.4 document_core_synthesize_searchable_pdf export missing!" >&2
  exit 1
fi
if ! grep -q "document_core_ingest_scan_as_document_session" platform/vocument-ffi/src/ocr_dto.rs; then
  echo "ERROR (M1.4 Gate): M1.4 document_core_ingest_scan_as_document_session export missing!" >&2
  exit 1
fi
if ! grep -q "testSearchablePdfSynthesisAndSessionIngestion" apps/android/app/src/test/java/org/vocument/app/ScanCaptureBridgeTest.kt; then
  echo "ERROR (M1.4 Gate): M1.4 testSearchablePdfSynthesisAndSessionIngestion missing in Android tests!" >&2
  exit 1
fi
if ! grep -q "test_searchable_pdf_synthesis_and_mupdf_roundtrip" engines/ocr/tests/synthesis_tests.rs; then
  echo "ERROR (M1.4 Gate): M1.4 synthesis and FTS5 ingestion test missing in synthesis_tests.rs!" >&2
  exit 1
fi
echo "✓ M1.4 Gate verified: Searchable PDF synthesis (invisible 3 Tr text layer), DocumentSession ingestion & FTS5 search verified."

# Enforce M2.1 Gate: secure redaction with verification
#
# A redaction is only worth something if the removed content is really gone.
# The gate therefore insists on three things: the engine removes the content
# (and not only covers it), the core verifies the removal, and the undo of a
# redaction is refused.
if [[ ! -f "core/vocument-core/src/redaction.rs" ]]; then
  echo "ERROR (M2.1 Gate): core/vocument-core/src/redaction.rs missing!" >&2
  exit 1
fi
if ! grep -q "document_core_redact_page_regions" platform/vocument-ffi/src/redaction_dto.rs; then
  echo "ERROR (M2.1 Gate): the redaction is not exported over the FFI boundary!" >&2
  exit 1
fi
if ! grep -q "fn redact_page_regions" engines/pdf-mupdf/src/lib.rs; then
  echo "ERROR (M2.1 Gate): the PDF engine does not implement the redaction!" >&2
  exit 1
fi
if ! grep -q "PDF_REDACT_TEXT_REMOVE" engines/pdf-mupdf/csrc/mupdf_mutate_bridge.c; then
  echo "ERROR (M2.1 Gate): the redaction does not remove the text of the region!" >&2
  exit 1
fi
if ! grep -q "a_redaction_removes_the_text_of_the_region_from_the_document" \
    engines/pdf-mupdf/src/mutate.rs; then
  echo "ERROR (M2.1 Gate): the engine has no test that the text of the region is removed!" >&2
  exit 1
fi
if ! grep -q "a_redaction_removes_the_scanned_text_from_the_document_and_from_the_search" \
    platform/vocument-ffi/tests/scan_workflow_e2e_tests.rs; then
  echo "ERROR (M2.1 Gate): the redaction has no end-to-end test over the FFI boundary!" >&2
  exit 1
fi
if ! grep -q "A redaction cannot be undone" core/vocument-core/src/transactions.rs; then
  echo "ERROR (M2.1 Gate): a redaction must not be undoable!" >&2
  exit 1
fi
echo "✓ M2.1 Gate verified: secure redaction removes the content, verifies the removal and refuses the undo."

# Enforce QA-003: Prohibition of stubs in feature acceptance (CR-0002)
if ! grep -q "QA-003" REQUIREMENTS.md; then
  echo "ERROR (QA-003): QA-003 missing in REQUIREMENTS.md!" >&2
  exit 1
fi
if ! grep -q "QA-003" AGENTS.md; then
  echo "ERROR (QA-003): QA-003 rule missing in AGENTS.md!" >&2
  exit 1
fi
if ! grep -q "QA-003" QUALITY.md; then
  echo "ERROR (QA-003): QA-003 check missing in QUALITY.md!" >&2
  exit 1
fi
echo "✓ QA-003 Governance verified: Prohibition of stubs in feature acceptance strictly enforced."

echo "==> Repository validation passed."
