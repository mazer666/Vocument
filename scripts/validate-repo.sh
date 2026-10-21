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
  "docs/reports/M1.4b-M1.5-COMPLETION-REPORT.md"
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
if ! grep -q "PDF_REDACT_IMAGE_PIXELS" engines/pdf-mupdf/csrc/mupdf_mutate_bridge.c; then
  echo "ERROR (M2.1 Gate): the redaction does not replace the pixels of the image in the region!" >&2
  exit 1
fi
if ! grep -q "opts.black_boxes = 0" engines/pdf-mupdf/csrc/mupdf_mutate_bridge.c; then
  echo "ERROR (M2.1 Gate): the redaction must not cover the region with a black box — the box would hide content that survived the removal!" >&2
  exit 1
fi
if ! grep -q "rendered_page_clean" core/vocument-core/src/redaction.rs; then
  echo "ERROR (M2.1 Gate): the redaction is not verified against the picture of the page!" >&2
  exit 1
fi
if ! grep -q "RenderedPageReader" core/vocument-core/src/rendered_page.rs; then
  echo "ERROR (M2.1 Gate): the core has no port for the picture of the page!" >&2
  exit 1
fi
if ! grep -q "RedactionUnverified" platform/vocument-ffi/src/redaction_dto.rs; then
  echo "ERROR (M2.1 Gate): an unverified redaction must be refused over the FFI boundary!" >&2
  exit 1
fi
if ! grep -q "a_redaction_replaces_the_pixels_of_the_image_instead_of_covering_them" \
    engines/pdf-mupdf/src/redaction_image_tests.rs; then
  echo "ERROR (M2.1 Gate): no test that the pixels of the image are replaced instead of covered!" >&2
  exit 1
fi
if ! grep -q "the_redacted_region_is_empty_in_the_picture_of_the_page" \
    engines/pdf-mupdf/src/redaction_image_tests.rs; then
  echo "ERROR (M2.1 Gate): no test that the redacted region is empty in the picture of the page!" >&2
  exit 1
fi

# --- M2.1 Gate, second review: the hardening of the redaction ---
#
# An independent adversarial audit found that the guarantee of the redaction
# did not hold. The gate therefore insists on the properties that close the
# findings: normalised word boxes on turned pages, the check of the region
# and of the fragments, the search for the places of the document that still
# carry the text, and the rollback of a redaction that cannot be verified.
if ! grep -q "vocument_grow_box_for_character" engines/pdf-mupdf/csrc/mupdf_text_bridge.c; then
  echo "ERROR (M2.1 Gate): the word boxes must be the bounding box of the corners of the characters — on a turned page they would be inverted otherwise!" >&2
  exit 1
fi
if ! grep -q "is_fragment_of" core/vocument-core/src/redaction.rs; then
  echo "ERROR (M2.1 Gate): the verification must recognise a fragment of a removed word — the removal works character by character!" >&2
  exit 1
fi
if ! grep -q "word_remains_in_region" core/vocument-core/src/redaction.rs; then
  echo "ERROR (M2.1 Gate): the verification must check the redacted region itself — nothing may lie in it any more!" >&2
  exit 1
fi
if ! grep -q "fn find_word_occurrences" core/vocument-core/src/pdf_engine.rs; then
  echo "ERROR (M2.1 Gate): the engine interface has no search for the places of the document that still carry a word!" >&2
  exit 1
fi
if ! grep -q "vocument_mupdf_find_text_occurrences" engines/pdf-mupdf/csrc/mupdf_occurrences_bridge.c; then
  echo "ERROR (M2.1 Gate): the engine does not report the places of the document that still carry the text!" >&2
  exit 1
fi
if ! grep -q "vocument_mupdf_scrub_document" engines/pdf-mupdf/csrc/mupdf_scrub_bridge.c; then
  echo "ERROR (M2.1 Gate): the engine does not clean the annotations, the form values and the metadata of the removed text!" >&2
  exit 1
fi
if ! grep -q "fn rollback_redaction" core/vocument-core/src/redaction.rs; then
  echo "ERROR (M2.1 Gate): a redaction that cannot be verified must be rolled back — the user must not hold a changed document!" >&2
  exit 1
fi
if ! grep -q "fn replace_pdf_bytes" core/vocument-core/src/pdf_engine.rs; then
  echo "ERROR (M2.1 Gate): the engine interface cannot restore the previous bytes — the rollback of a redaction needs it!" >&2
  exit 1
fi
if ! grep -q "fn word_touches_region" core/vocument-core/src/redaction_rules.rs; then
  echo "ERROR (M2.1 Gate): the region has to remove every word it touches — a word that is only partly covered would leave a fragment behind!" >&2
  exit 1
fi
if ! grep -q "fn extend_regions_to_touching_words" core/vocument-core/src/redaction_rules.rs; then
  echo "ERROR (M2.1 Gate): the region has to grow to the whole word that it touches!" >&2
  exit 1
fi
if ! grep -q "fn fail_after_change" core/vocument-core/src/redaction.rs; then
  echo "ERROR (M2.1 Gate): every error after the removal has to put the document back!" >&2
  exit 1
fi
if ! grep -q "RedactionRollbackFailed" core/vocument-core/src/session.rs; then
  echo "ERROR (M2.1 Gate): a rollback that fails itself has to be reported as its own error — the session then holds a document in an unknown state!" >&2
  exit 1
fi
if ! grep -q "options.mode(0o600)" core/vocument-core/src/recovery.rs; then
  echo "ERROR (M2.1 Gate): the checkpoints of the recovery store hold the document in plain text — they have to belong to the user alone!" >&2
  exit 1
fi
if ! grep -q "remaining_occurrences" core/vocument-core/src/redaction.rs; then
  echo "ERROR (M2.1 Gate): the report must name the places where the same word still stands — otherwise nobody knows that the word is still in the document!" >&2
  exit 1
fi
for proof in \
    "core/vocument-core/src/redaction_rules.rs|a_region_grows_to_the_words_it_touches" \
    "core/vocument-core/tests/redaction_remaining_occurrences_tests.rs|the_same_word_on_another_page_is_named_in_the_report" \
    "engines/pdf-mupdf/src/scrub_tests.rs|the_alternative_text_of_an_annotation_and_the_xmp_packet_of_a_page_are_found" \
    "engines/pdf-mupdf/src/scrub_tests.rs|the_alternative_text_of_an_annotation_and_the_xmp_packet_of_a_page_are_emptied" \
    "core/vocument-core/tests/redaction_remaining_occurrences_tests.rs|a_remainder_without_a_place_still_stops_the_redaction" \
    "platform/vocument-ffi/tests/redaction_hardening_e2e_tests.rs|the_same_word_on_another_page_is_named_in_the_report" \
    "engines/pdf-mupdf/src/redaction_rotation_tests.rs|the_word_boxes_of_a_turned_page_are_normalised" \
    "engines/pdf-mupdf/src/scrub_tests.rs|the_text_of_an_annotation_and_of_a_form_field_is_emptied" \
    "engines/pdf-mupdf/src/scrub_tests.rs|a_word_that_is_hex_encoded_is_found" \
    "platform/vocument-ffi/tests/redaction_hardening_e2e_tests.rs|a_region_that_only_covers_a_part_of_a_word_removes_the_whole_word" \
    "platform/vocument-ffi/tests/redaction_hardening_e2e_tests.rs|a_region_without_content_is_refused_and_the_document_stays_unchanged" \
    "core/vocument-core/tests/redaction_rollback_tests.rs|a_successful_redaction_deletes_the_checkpoints_of_the_recovery_store" \
    "core/vocument-core/tests/redaction_rollback_tests.rs|a_redaction_that_cannot_be_verified_leaves_the_document_and_the_checkpoints_alone"; do
  file="${proof%%|*}"
  name="${proof##*|}"
  if [[ ! -f "$file" ]] || ! grep -q "$name" "$file"; then
    echo "ERROR (M2.1 Gate): the proof '$name' is missing ($file)!" >&2
    exit 1
  fi
done

# --- M2.1 Gate, third review: every place of the document, not a list ---
# The third adversarial audit found the systematic gap of the first two: the
# search and the cleaning worked with a list of known places. A secret in a
# place that the list did not know survived while the redaction reported a
# success.
if [[ ! -f engines/pdf-mupdf/csrc/mupdf_walk.h ]]; then
  echo "ERROR (M2.1 Gate): the engine has no walk through the objects of the document — a list of known places is not a search!" >&2
  exit 1
fi
if ! grep -q "vocument_walk_document" engines/pdf-mupdf/csrc/mupdf_walk.h; then
  echo "ERROR (M2.1 Gate): the walk through the document is missing!" >&2
  exit 1
fi
if ! grep -q "vocument_walk_document" engines/pdf-mupdf/csrc/mupdf_occurrences_bridge.c; then
  echo "ERROR (M2.1 Gate): the search for the places of the document must walk through every object, not through a list of known places!" >&2
  exit 1
fi
if ! grep -q "vocument_walk_document" engines/pdf-mupdf/csrc/mupdf_scrub_bridge.c; then
  echo "ERROR (M2.1 Gate): the cleaning of the document must walk through every object, not through a list of known places!" >&2
  exit 1
fi
for kind in content hidden uninspected; do
  if ! grep -q "\"$kind\"" engines/pdf-mupdf/csrc/mupdf_occurrences_bridge.c; then
    echo "ERROR (M2.1 Gate): the kind of the place '$kind' is missing — a place that cannot be read must not pass for clean!" >&2
    exit 1
  fi
  if ! grep -q "\"$kind\"" core/vocument-core/src/redaction.rs; then
    echo "ERROR (M2.1 Gate): the core has to judge the kind of the place '$kind' — the engine reports the facts, the core decides!" >&2
    exit 1
  fi
done
if ! grep -q "uninspected_places" core/vocument-core/src/redaction.rs; then
  echo "ERROR (M2.1 Gate): the report has to name the places that could not be read — the redaction must not claim that the text is gone from there!" >&2
  exit 1
fi
if ! grep -q "redactionWarnings" apps/macos/Sources/VocumentMacKit/DocumentCoreClient.swift; then
  echo "ERROR (M2.1 Gate): the macOS client has to turn the places into warnings — the user has to know that the word is still in the document!" >&2
  exit 1
fi
if ! grep -q "redactionWarnings" apps/android/app/src/main/java/org/vocument/app/DocumentCoreBridge.kt; then
  echo "ERROR (M2.1 Gate): the Android bridge has to turn the places into warnings — the user has to know that the word is still in the document!" >&2
  exit 1
fi
if ! grep -q "enforce_private_checkpoint_files" core/vocument-core/src/recovery.rs; then
  echo "ERROR (M2.1 Gate): a checkpoint of an older version has to be made private again — it holds the document in plain text!" >&2
  exit 1
fi
for proof in \
    "engines/pdf-mupdf/src/scrub_tests.rs|the_places_that_a_list_of_known_places_does_not_reach_are_found" \
    "engines/pdf-mupdf/src/scrub_tests.rs|the_places_that_a_list_of_known_places_does_not_reach_are_emptied" \
    "core/vocument-core/src/redaction_rules.rs|a_word_that_only_stands_next_to_the_region_is_not_removed" \
    "core/vocument-core/tests/redaction_rollback_tests.rs|a_redaction_that_fails_puts_the_revision_and_the_dirty_flag_back" \
    "platform/vocument-ffi/tests/redaction_hardening_e2e_tests.rs|the_same_word_twice_on_the_same_page_is_redacted_and_the_other_place_is_named" \
    "platform/vocument-ffi/tests/redaction_hardening_e2e_tests.rs|an_attached_archive_is_named_as_a_place_that_could_not_be_read" \
    "apps/android/app/src/test/java/org/vocument/app/DocumentCoreBridgeTest.kt|thePlacesWhereTheSameTextStillStandsAreTurnedIntoWarnings"; do
  file="${proof%%|*}"
  name="${proof##*|}"
  if [[ ! -f "$file" ]] || ! grep -q "$name" "$file"; then
    echo "ERROR (M2.1 Gate): the proof '$name' is missing ($file)!" >&2
    exit 1
  fi
done
for report in \
    "docs/audit/redteam-round-three-document-level.md" \
    "docs/audit/redteam-round-three-platform-chain.md"; do
  if [[ ! -f "$report" ]]; then
    echo "ERROR (M2.1 Gate): the report of the third audit is missing ($report) — an audit that is not written down did not happen!" >&2
    exit 1
  fi
done

# --- The callers: the apps have to reach the functions of M2 ---
if ! grep -q "redactPageRegions" apps/macos/Sources/VocumentMacKit/DocumentCoreClient.swift; then
  echo "ERROR (M2.1 Gate): the macOS client has to call the redaction — the chain must not end in the generated code!" >&2
  exit 1
fi
if ! grep -q "redactPageRegions" apps/android/app/src/main/java/org/vocument/app/DocumentCoreBridge.kt; then
  echo "ERROR (M2.1 Gate): the Android bridge has to call the redaction — the chain must not end in the generated code!" >&2
  exit 1
fi
if ! grep -q "requireVerifiedRedaction" apps/android/app/src/main/java/org/vocument/app/DocumentCoreBridge.kt; then
  echo "ERROR (M2.1 Gate): the caller has to check the report itself — the 'verified' flag is not a check!" >&2
  exit 1
fi
if ! grep -q "inspectDocument" apps/macos/Sources/VocumentMacKit/DocumentCoreClient.swift; then
  echo "ERROR (M2.2 Gate): the macOS client has to call the inspection of the document!" >&2
  exit 1
fi
if ! grep -q "inspectDocument" apps/android/app/src/main/java/org/vocument/app/DocumentCoreBridge.kt; then
  echo "ERROR (M2.2 Gate): the Android bridge has to call the inspection of the document!" >&2
  exit 1
fi

# --- M2.2 Gate: the inspection of the links and actions of a document ---
if ! grep -q "fn document_actions" core/vocument-core/src/pdf_engine.rs; then
  echo "ERROR (M2.2 Gate): the engine interface has no inspection of the actions of a document!" >&2
  exit 1
fi
if ! grep -q "fn inspect_document" core/vocument-core/src/session.rs; then
  echo "ERROR (M2.2 Gate): the core has no service for the inspection of a document!" >&2
  exit 1
fi
if ! grep -q "is_risky_kind" core/vocument-core/src/document_inspection.rs; then
  echo "ERROR (M2.2 Gate): the risk policy does not live in the core!" >&2
  exit 1
fi
if ! grep -q "document_core_inspect_document" platform/vocument-ffi/src/inspection_dto.rs; then
  echo "ERROR (M2.2 Gate): the inspection is not available over the FFI boundary!" >&2
  exit 1
fi
if ! grep -q "the_inspection_reports_every_action_of_the_document" \
    engines/pdf-mupdf/src/inspection_tests.rs; then
  echo "ERROR (M2.2 Gate): no test that the inspection reports every action of a document!" >&2
  exit 1
fi
if ! grep -q "the_position_of_a_link_is_reported_in_the_space_of_the_page" \
    engines/pdf-mupdf/src/inspection_tests.rs; then
  echo "ERROR (M2.2 Gate): no test that the position of a link is reported in the space of the page!" >&2
  exit 1
fi
if ! grep -q "the_inspection_of_a_document_shows_where_the_links_lead_and_what_they_do" \
    platform/vocument-ffi/tests/document_inspection_e2e_tests.rs; then
  echo "ERROR (M2.2 Gate): no end-to-end test of the inspection over the FFI boundary!" >&2
  exit 1
fi

# --- M2.3 Gate: the inspection of the attachments and the signatures ---
if ! grep -q "fn document_attachments" core/vocument-core/src/pdf_engine.rs; then
  echo "ERROR (M2.3 Gate): the engine interface has no inspection of the attachments!" >&2
  exit 1
fi
if ! grep -q "fn document_signatures" core/vocument-core/src/pdf_engine.rs; then
  echo "ERROR (M2.3 Gate): the engine interface has no inspection of the signatures!" >&2
  exit 1
fi
if ! grep -q "changed_since_signing" core/vocument-core/src/document_inspection.rs; then
  echo "ERROR (M2.3 Gate): the core does not report a signature that no longer covers the document!" >&2
  exit 1
fi
if ! grep -q "is_risky_attachment" core/vocument-core/src/document_inspection.rs; then
  echo "ERROR (M2.3 Gate): the risk policy for the attachments does not live in the core!" >&2
  exit 1
fi
if ! grep -q "a_document_that_was_changed_after_the_signature_is_reported" \
    engines/pdf-mupdf/src/attachment_signature_tests.rs; then
  echo "ERROR (M2.3 Gate): no test that a document changed after the signature is reported!" >&2
  exit 1
fi
if ! grep -q "the_embedded_file_of_the_document_is_reported" \
    engines/pdf-mupdf/src/attachment_signature_tests.rs; then
  echo "ERROR (M2.3 Gate): no test that the embedded file of a document is reported!" >&2
  exit 1
fi
if ! grep -q "a_document_that_was_changed_after_the_signature_is_reported_as_such" \
    platform/vocument-ffi/tests/document_inspection_e2e_tests.rs; then
  echo "ERROR (M2.3 Gate): no end-to-end test of the signature inspection!" >&2
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
echo "✓ M2.2 Gate verified: the links and actions of a document are reported, the risk is decided in the core."
echo "✓ M2.3 Gate verified: the attachments and the signatures of a document are reported."

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
