# ADR-0009: M0.6 Crash Recovery Containers, Journaling, and Restore Actions

- **Status:** Accepted
- **Date:** 2026-10-18
- **Milestone:** M0.6 — Recovery (`ROADMAP.md` lines 151–172, `ARCHITECTURE.md` §§11–12)
- **Requirements Addressed:** `RECOVERY-001`, `SAVE-001`, `SAVE-004`, `FILELOCK-001`, `CODE-001`

---

## Context

Unexpected system crashes, OS terminations (`SIGKILL`), battery exhaustion, or power loss while working on a document must never cause data loss or corrupt the original baseline document.
Milestone M0.6 delivers per-document crash recovery governed by the invariant:
> **Gate Invariant:** *Original file on disk remains completely intact until explicit user save.*

Specific technical requirements:
1. Isolated, per-document recovery containers containing versioned metadata, journals, and checkpoints (`ARCHITECTURE.md` §12).
2. Automatic journaling of semantic mutations (`TransactionCommand`, undo, redo) with minimal runtime overhead.
3. Reliable crash detection via OS process PID and an atomic `is_clean_shutdown` marker flag.
4. Support for the 4 core recovery actions (`RECOVERY-001` Preset):
   - **Restore:** Replays uncommitted journal into active session; disk file remains untouched.
   - **Preview:** Opens an isolated read-only preview of recovered state.
   - **Discard:** Cleans up the recovery container and opens original disk file.
   - **Open as New:** Opens recovered state as an unsaved new document with a collision-free path.
5. Mandatory destructive hard-kill test (`SIGKILL`) proving uncommitted edits survive sudden process death and disk files remain 100% intact.

---

## Decision

1. **Per-Document Recovery Container (`RecoveryContainer`, `RecoveryStore`):**
   - Implemented in `core/vocument-core/src/recovery.rs`.
   - Each active document has a dedicated recovery container inside `vocument_recovery/<document_id>/`:
     - `manifest.json`: versioned metadata (`RecoveryManifest`) including `schema_version`, `document_id`, `source_token`, `original_file_identity`, `active_pid`, `is_clean_shutdown`, `has_unsaved_changes`, `uncommitted_revisions`, and SHA-256 integrity checksum.
     - `journal.json`: persistent transaction journal (`PersistentTransactionJournal`).
     - `checkpoints/`: periodic document snapshots with SHA-256 verification.
   - Manifest writes use atomic staging (`.tmp_manifest_*` + fsync + atomic rename) to avoid corruption if a crash occurs during a manifest write.

2. **Automatic Journaling & Crash Marker Hook (`session_mutations.rs`):**
   - In `execute_transaction`, `undo`, and `redo`, after mutations succeed on `DocumentActor` and `TransactionManager`, `record_recovery_mutation` is invoked.
   - Updates `has_unsaved_changes = true`, `is_clean_shutdown = false`, and appends the transaction to `journal.json`.
   - When a session is cleanly closed or saved, `mark_clean_shutdown` marks `is_clean_shutdown = true`.
   - If the process is terminated abnormally (`SIGKILL`, crash), `mark_clean_shutdown` is never reached; upon next launch, `RecoveryStore::scan_recoverable_documents` immediately detects the pending recovery container.

3. **Recovery Actions Contract (`RecoveryAction`, `RecoveryActionResult`):**
   - Implemented on `SessionRegistry::execute_recovery_action`.
   - Exposed through UniFFI in `platform/vocument-ffi/src/recovery_dto.rs` and bound into Swift (`DocumentCoreClient`) and Kotlin (`DocumentCoreBridge`).
   - All actions uphold the M0.6 Gate Invariant: the original file on disk is never touched until explicit user save.

4. **Destructive Hard-Kill Verification (`test_hard_kill_recovery_with_subprocess_sigkill`):**
   - Automated in `core/vocument-core/tests/recovery_tests.rs`.
   - Uses `std::process::Command` to re-execute the test binary with `VOC_TEST_RUN_CRASH_VICTIM=1`.
   - Parent terminates the child process using `SIGKILL` while a mutated document is unsaved.
   - Re-opening verifies crash detection, pristine baseline disk bytes, successful restore replay, and subsequent atomic save.

5. **Code Line Limits (`CODE-001`):**
   - All modules were kept concise and well below the 900-line hard maximum:
     - `core/vocument-core/src/recovery.rs` (686 lines)
     - `core/vocument-core/src/session.rs` (584 lines $\le 600$)
     - `core/vocument-core/src/session_mutations.rs` (552 lines $\le 600$)
     - `core/vocument-core/src/save.rs` (519 lines $\le 600$)
     - `platform/vocument-ffi/src/recovery_dto.rs` (215 lines $\le 600$)
     - `core/vocument-core/tests/recovery_tests.rs` (658 lines)

---

## Consequences

- **Positive:** Full data loss protection across crashes; user edits survive sudden power-off or OS process termination.
- **Positive:** 100% adherence to M0.6 Gate Invariant: original files on disk remain strictly immutable during crash, recovery scan, preview, and restore until explicit user save.
- **Positive:** Both macOS SwiftUI app and Android Kotlin bridge provide native UI sheets and verified handshake tests.
- **Next Step:** Advance to **M0.7 — Interoperability Gate** (cross-engine comparisons, PDF/A, PDF/UA, validator test harness).
