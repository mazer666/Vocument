# ADR-0010: M0.7 Interoperability Gate and Foreign Viewer Verification

- **Status:** Accepted
- **Date:** 2026-10-18
- **Milestone:** M0.7 — Interoperability Gate (`ROADMAP.md` lines 174–210, M0 Definition of Done)
- **Requirements Addressed:** `INTEROP-001`, `INTEROP-002`, `ANNOT-001`, `ROTATE-001`, `SAVE-001`, `RECOVERY-001`, `CODE-001`

---

## Context

Milestone M0.7 establishes the mandatory **Interoperability Gate** completing the Milestone M0 (Foundation / Trustworthy Viewer) Definition of Done:
> *"The complete M0 workflow must be tested from end to end: open -> search -> select text -> highlight -> rotate -> undo -> redo -> save -> hard-kill -> recover -> save -> foreign viewer reopen. Mutated and saved PDFs must open cleanly in MuPDF, reference renderers, and available third-party viewers. Unavailable manual checks must be honestly documented without simulation."*

Specific engineering challenges:
1. **Multi-Save Baseline Invariant:** In a multi-save workflow (save -> mutate -> crash -> recover -> save), saving a document changes its physical SHA-256 and content on disk. Subsequent mutations and recovery containers must update their baseline identity to the new disk state, avoiding invalid journal replay rejections.
2. **Foreign Viewer Cross-Validation:** The output PDF containing rotation (+180° cumulative) and standard `/Highlight` annotations must render accurately across distinct rendering engines without syntax corruptions or missing annotations.
3. **Viewer Availability & Honesty Rule:** Real environments vary; on this macOS developer host, MuPDF (`mutool`), macOS CoreGraphics/Quartz (`sips`), and Chromium/PDFium (`Google Chrome`) are available. Viewers requiring Windows (PDF-XChange) or uninstalled packages (Adobe Acrobat Reader, Firefox) must be recorded honestly as not installed, rather than faked or simulated.

---

## Decision

1. **Rust Core Baseline Rebinding on Safe Save (`transactions.rs`, `session.rs`):**
   - Updated `TransactionManager::mark_as_saved(document_id, base_pdf_sha256)`.
   - When a document is saved, `session.document_id` and `transaction_manager` baseline state update to reflect the newly written disk file.
   - Undo/Redo stacks reset their baseline origin to the saved state, ensuring subsequent crash-recovery journals correctly match the current baseline.

2. **Automated Interoperability Harness (`interop_dto.rs`, `interop_tests.rs`):**
   - Implemented `document_core_run_interop_gate_suite() -> FfiInteropReport` in `platform/vocument-ffi/src/interop_dto.rs`.
   - Executes the complete M0 Definition of Done sequence:
     1. Open sample PDF via `FileBroker`.
     2. Progressive text search (`SEARCH-001`, `SEARCH-002`).
     3. Select page text with whitespace/hyphenation cleanup (`OCRCOPY-002`).
     4. Add standard PDF highlight annotation (`ANNOT-001`).
     5. Rotate page (+90°) via `TransactionManager` (`ROTATE-001`).
     6. Undo rotation (reverts to 0°).
     7. Redo rotation (re-applies 90°).
     8. Safe Save with atomic staging and post-save validation (`SAVE-001`).
     9. Unsaved mutation (rotate to 180°) followed by simulated crash/unclosed state.
     10. Detect recovery container and verify disk file remains 100% byte-for-byte identical (`RECOVERY-001` M0.6 Gate Invariant).
     11. Execute `RecoveryAction::Restore` and perform second Safe Save.
     12. Reopen saved PDF in a fresh MuPDF engine instance; verify rotation=180° and highlight annotation.
     13. Validate via external `mutool draw` / `mutool info` CLI.
     14. Validate via macOS CoreGraphics / Quartz (`/usr/bin/sips`).
     15. Validate via Google Chrome headless PDFium screenshot rendering.
     16. Inspect installed status for Adobe Acrobat Reader, PDF-XChange, and Firefox.

3. **Honesty Verification Policy for Third-Party Viewers:**
   - Evaluated host availability:
     - MuPDF (`mutool` 1.28.5): Available & Verified.
     - macOS CoreGraphics / Quartz (`sips`): Available & Verified.
     - Google Chrome (Chromium / PDFium): Available & Verified.
     - Adobe Acrobat Reader: Not installed on host runner.
     - PDF-XChange: Windows-only, unavailable on macOS host.
     - Firefox: Not installed on host runner.
   - The system records their uninstalled status honestly in `FfiInteropReport` and `M0.7-ACCEPTANCE.md`. No test simulates a false pass for missing software.

4. **Cross-Platform Verification (macOS & Android):**
   - Swift macOS handshake CLI (`vocument-mac-handshake`) executes `client.runInteropGateSuite()`.
   - Kotlin Android handshake CLI (`VerifyKotlinHandshakeCli.kt`) executes `bridge.runInteropGateSuite()`.
   - Both verify all gates passed on their respective platform bridges.

5. **Strict File Size Limits (`CODE-001`):**
   - `platform/vocument-ffi/src/interop_dto.rs`: 250 lines ($\le 600$).
   - `core/vocument-core/tests/interop_tests.rs`: 501 lines ($\le 600$).
   - `apps/macos/Sources/VocumentMacHandshakeCLI/main.swift`: 524 lines ($\le 600$).
   - `apps/android/app/src/test/java/org/vocument/app/VerifyKotlinHandshakeCli.kt`: 551 lines ($\le 600$).

---

## Consequences

- All M0.0 through M0.7 milestone requirements are fully implemented, verified, and demonstrated.
- Milestone M0 (Foundation / Trustworthy Viewer) is formally completed and closed.
- The project is ready for Milestone M1 (Core Document Experience).
