# ADR-0008: M0.5 Safe Save, Atomic Staging, Post-Save Validation, and Conflict Handling

- **Status:** Accepted
- **Date:** 2026-10-17
- **Milestone:** M0.5 — Safe Save (`ROADMAP.md` lines 132–148, `ARCHITECTURE.md` §16)
- **Requirements Addressed:** `SAVE-001`, `SAVE-002`, `SAVE-003`, `SAVE-004`, `SAVE-005`, `FILELOCK-001`, `FILELOCK-002`, `FILELOCK-003`, `FILELOCK-004`, `CODE-001`

---

## Context

Document corruption or silent data loss caused by unsafe saving or simultaneous external edits is catastrophic for user trust.
Milestone M0.5 introduces a safe saving subsystem governed by the strict gate:
> **Gate:** *Nie still extern geänderte Inhalte überschreiben.* (Never silently overwrite externally changed content.)

Specific technical challenges:
1. Save must be atomic: crashes during write must leave the existing file intact.
2. Saved files must be validated before replacing the existing file (`SAVE-005`).
3. External file edits (different process, cloud sync, or external editor) must be detected before committing changes (`FILELOCK-001`, `FILELOCK-002`).
4. Detected conflicts must be presented as a structured contract across UniFFI, offering non-destructive options including safe collision-free copies (`FILELOCK-003`, `FILELOCK-004`).
5. Filesystem access must remain brokered behind `FileBroker`, respecting platform sandboxing on macOS and Android (`ARCHITECTURE.md` §5).

---

## Decision

1. **Brokered Staged Writes (`FileBroker` extension, `SAVE-001`, `SAVE-003`):**
   - Extended `FileBroker` with `stage_write(target_token, bytes) -> BrokeredStagedWrite`.
   - On local filesystems (`ScopedLocalFileBroker`), writes bytes to a sibling temporary file (`.tmp_vocument_<stem>_<random>.<ext>`) in the same directory, calls `sync_all` (fsync) to flush OS page caches to disk, and uses `std::fs::rename` for atomic file replacement.
   - If an error or conflict occurs, `discard_staged_write` removes the temporary file without altering the target file.

2. **Pre-Save External Conflict Detection (`FILELOCK-001`, `FILELOCK-002`, M0.5 Gate):**
   - Prior to committing, `SaveCoordinator` calls `FileBroker::inspect_file_identity(target_token)`.
   - The observed identity is compared with the session's baseline identity using `FileIdentity::compare()`.
   - If `FileIdentityComparison::ModifiedExternally` or `DifferentFileReplacingSamePath` is detected and `!allow_overwrite_external`, save is aborted immediately, staged temporary files are discarded, and a `SaveResult::Conflict` is returned.
   - The on-disk file is untouched; unsaved changes remain dirty in memory.

3. **Post-Save Validation (`SAVE-005`):**
   - After atomic rename, `SaveCoordinator::validate_saved_file` inspects the committed file:
     - Confirms the file exists on disk and its byte size matches the written payload.
     - Verifies standard `%PDF-` magic header bytes.
     - Passes the bytes to `PdfEngine::summarize_bytes` to verify structural integrity and valid page count.

4. **Structured Conflict Contract & Resolution UI (`FILELOCK-003`, `FILELOCK-004`):**
   - `SaveConflict` / `FfiSaveConflict` bundles `baseline_identity`, `observed_external_identity`, `comparison_summary`, and a deterministically computed `suggested_safe_copy_path`.
   - Four distinct resolutions are supported:
     1. `SaveLocalAsCollisionFreeCopy`: saves the local edits to `<stem>.safe-copy-YYYYMMDD-HHMMSS[-N].pdf`.
     2. `OverwriteExternalExplicitly`: requires deliberate user confirmation before overwriting the externally modified file.
     3. `ReloadExternalVersion`: discards local mutations and reloads the modified disk file into the session.
     4. `Cancel`: leaves the session dirty and aborts save.

5. **Code Size and Modularity (`CODE-001`):**
   - New logic was placed in `core/vocument-core/src/save.rs` (487 lines) and `platform/vocument-ffi/src/save_dto.rs` (269 lines).
   - Integration tests live in `core/vocument-core/tests/safe_save_tests.rs` (474 lines).
   - All touched files remain under the target $\le 600$ lines limit.

---

## Consequences

- **Positive:** Impossible for incomplete saves, mid-write crashes, or external file modifications to corrupt original documents or silently overwrite external user edits.
- **Positive:** Both macOS SwiftUI and Android Kotlin have native UI and handshake coverage verifying the M0.5 gate.
- **Next Step:** Advance to **M0.6 — Recovery** (per-document recovery container, periodic checkpoints, crash detection, Restore/Preview/Discard/Open as New).
