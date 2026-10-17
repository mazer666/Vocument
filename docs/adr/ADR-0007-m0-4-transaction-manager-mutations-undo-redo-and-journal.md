# ADR-0007: M0.4 TransactionManager, Highlight Annotations, Page Rotation, Undo/Redo, and Persistent Transaction Journal

- **Status:** Accepted
- **Date:** 2026-10-17
- **Milestone:** M0.4 — First mutations (`ROADMAP.md` lines 117–130)
- **Requirements Addressed:** `ANNOT-001`, `ROTATE-001`, `PAGE-001`, `UNDO-001`, `UNDO-002`, `UNDO-004`, `INTEROP-002`, `HASH-001`, `CODE-001`

---

## Context

Milestone M0.4 introduces the first document mutations in Vocument:
1. Standard PDF `/Highlight` annotations (`ANNOT-001`, `INTEROP-002`).
2. Page rotation by multiples of 90° (`ROTATE-001`, `PAGE-001`, `INTEROP-002`).
3. Semantic transaction commands with human-readable descriptions (`UNDO-004`).
4. Full Undo/Redo stack and dirty-state tracking (`UNDO-001`, `UNDO-002`).
5. A versioned, checksum-verified persistent transaction representation (`PersistentTransactionJournal`) supporting crash-resilient inspection and replay (`ARCHITECTURE.md` §§11–12, `UNDO-001`).
6. Enforcement of the **M0.4 Gate**: *Mutation never bypasses `TransactionManager`.*

---

## Decision

1. **Exclusive Mutation Authority (`TransactionManager` & M0.4 Gate):**
   - `DocumentActor::apply_engine_mutation_for_transaction_manager` in `core/vocument-core/src/render.rs` is marked `pub(crate)` and called **only** from `TransactionManager::apply_command_on_actor` in `core/vocument-core/src/transactions.rs`.
   - `scripts/validate-repo.sh` enforces at CI time that `apply_engine_mutation_for_transaction_manager` is never invoked outside `transactions.rs`.
   - Every mutation bumps the actor's revision counter (`revision + 1`), ensuring `TileCache` and `ThumbnailCache` keys (`(document_id, revision, ...)`) automatically invalidate stale tiles without manual cache flushing.

2. **Semantic `TransactionCommand` & Inverse Command Capture (`UNDO-004`):**
   - `TransactionCommand` models user-meaningful operations (`RotatePages`, `AddHighlightAnnotation`, `RemoveHighlightAnnotation`) with human-readable descriptions (`user_description()`).
   - Executing a forward command automatically synthesizes its deterministic inverse command (e.g. `RotatePages { delta_degrees: +90 }` $\leftrightarrow$ `RotatePages { delta_degrees: -90 }`, `AddHighlightAnnotation` $\leftrightarrow$ `RemoveHighlightAnnotation` with full snapshot restoration).

3. **MuPDF 1.28.5 C Bridge for Standard PDF Mutations (`engines/pdf-mupdf/csrc/mupdf_mutate_bridge.c`):**
   - **Page Rotation (`vocument_mupdf_rotate_pages`):** Updates `/Rotate` on each target `pdf_page` dictionary to a normalized `[0, 90, 180, 270]` angle (`ROTATE-001`) and serializes the updated PDF into an in-memory `fz_buffer` via `pdf_write_document`.
   - **Highlight Annotations (`vocument_mupdf_add_highlight`, `vocument_mupdf_remove_highlight`, `vocument_mupdf_list_highlights`):** Creates standard PDF `/Subtype /Highlight` annotations using `pdf_create_annot(ctx, page, PDF_ANNOT_HIGHLIGHT)`, populates `/QuadPoints` (`pdf_set_annot_quad_points`), `/C` (`pdf_set_annot_color`), `/CA` (`pdf_set_annot_opacity`), `/NM` (`annotation_id`), `/T` (`author`), and `/Contents` (`contents`), and calls `pdf_update_annot` so MuPDF's display-list renderer draws the highlight natively (`INTEROP-002`).

4. **Persistent Transaction Representation (`PersistentTransactionJournal`, `ARCHITECTURE.md` §§11–12):**
   - Versioned (`schema_version = 1`), containing `document_id`, `base_pdf_sha256`, `current_revision`, `saved_revision`, `committed_records`, `redo_records`, and `journal_checksum_sha256` (`HASH-001`).
   - Each `TransactionRecord` carries its own `before_pdf_sha256`, `after_pdf_sha256`, and `record_checksum_sha256`, enabling tamper/corruption detection (`verify_integrity()`) and deterministic replay onto a freshly opened session (`restore_and_replay_journal()`).

5. **Code Modularity (`CODE-001`):**
   - Mutation logic is split cleanly into `engines/pdf-mupdf/csrc/mupdf_mutate_bridge.c` (342 lines), `engines/pdf-mupdf/src/mutate.rs` (223 lines), `core/vocument-core/src/transactions.rs` (537 lines), `core/vocument-core/src/session_mutations.rs` (551 lines), `platform/vocument-ffi/src/mutation_dto.rs` (356 lines), and `apps/android/app/src/main/java/org/vocument/app/MutationToolsPanel.kt` (219 lines), keeping every hand-written source file $\le 600$ lines.

---

## Consequences

- **Positive:** Standard PDF readers (macOS Preview, Adobe Acrobat, MuPDF) immediately see rotated pages and `/Highlight` annotations (`INTEROP-002`); Undo/Redo and journal replay are 100% deterministic across macOS and Android.
- **Next Step:** Proceed to **M0.5 — Safe Save + Crash Recovery** (`SaveCoordinator`, staging, post-save verification, journal persistence on disk, and recovery dialog).
