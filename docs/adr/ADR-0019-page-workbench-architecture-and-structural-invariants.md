# ADR-0019: Page Workbench Architecture, Object Invariants, and Structural Integrity (M3)

- **Status:** Accepted
- **Date:** 2026-10-23
- **Author:** Chief Architect, PDF/MuPDF Engineer, QA Lead
- **Deciders:** ROLE-01 (Product Steward), ROLE-02 (Chief Architect), ROLE-04 (PDF Engineer), ROLE-09 (Security Engineer), ROLE-13 (QA Lead), ROLE-15 (Standards Lead)
- **Requirement IDs:** `PAGE-001`–`PAGE-005`, `SIG-001`, `ANNOT-001`, `UNDO-001`, `UNDO-002`, `RECOVER-001`, `CODE-001`

---

## Context & Problem Statement

Milestone M3 ("Page Workbench") introduces comprehensive visual and programmatic manipulation of PDF pages: reordering, duplication, deletion, extraction, merging, splitting, box cropping, rotation, Bates numbering, headers/footers, and watermarks/stamps.

Unlike naive document tools that treat PDFs as mere sequences of raster pages or loosely concatenated streams, PDF is an object-graph database where pages are interconnected with:
1. Digital signatures (`/ByteRange` cryptographic hashes)
2. Interactive AcroForm fields (hierarchical `/Kids` arrays and tab orders)
3. Logical navigation outlines (`/Outlines` and bookmark target dictionaries)
4. Named destinations (`/Dests` and name trees)
5. Page labels (`/PageLabels` number tree)
6. Internal hyperlink annotations (`/Link` with `/GoTo` actions)
7. Tagged PDF / PDF/UA logical structure trees (`/StructTreeRoot`, `/ParentTree`, Marked Content Identifiers / MCIDs)
8. Document security and encryption dictionaries (`/Encrypt`)

If a page manipulation tool arbitrarily alters page trees without understanding and maintaining these document-level invariants, documents will silently suffer broken links, invalid signatures, corrupted form fields, or destroyed accessibility tagging.

This ADR defines the binding architecture and integrity rules governing all Page Workbench operations in Vocument.

---

## Decision Drivers

1. **Non-Destructive Integrity:** A page operation on page $k$ must never silently corrupt objects on page $j$ or document-level structures.
2. **Explicit Handling of Structural Incompatibilities:** Where structural preservation is mathematically or cryptographically impossible (e.g., cryptographic signatures), the software must detect the conflict and explicitly require user confirmation rather than silently breaking the document.
3. **Transaction Safety & Reversibility (`UNDO-001`, `UNDO-002`):** Every page operation must be an atomic `TransactionCommand` with exact forward and inverse representations, supporting unlimited Undo and Redo.
4. **Native C Bridge Grafting with Zero Disk I/O:** Leverage MuPDF's native `pdf_graft_page` and object cloning mechanisms directly in memory without intermediate disk writes or process boundaries.
5. **Memory & Scaling Efficiency:** Support large documents ($\ge 1{,}000$ pages) with bounded memory consumption and fast UI thumbnail generation.

---

## Architectural Decisions

### 1. Document Invariant Rules for Page Operations

| Object Subsystem | Rule on Reorder / Duplicate / Delete / Merge | Enforcement Mechanism |
|---|---|---|
| **Digital Signatures (`SIG-001`)** | **Cryptographic Invalidation Alert:** Any page mutation alters the document byte sequence and breaks the signature `/ByteRange`.<br>• The system must detect signed documents.<br>• Warn the user: *"Modifying pages will invalidate existing digital signatures."*<br>• Upon confirmation, the signature is marked as broken/tampered according to ISO 32000-1. | Pre-mutation check via `DocumentActor::document_signatures()`. Rejects operation if user cancels. |
| **AcroForm Fields** | **Unique Field Naming on Duplication:**<br>• Duplicating a page must NOT create duplicate `/T` (field name) entries, which would cause shared state bugs.<br>• Duplicated fields receive a standardized suffix (e.g. `Field_Copy1`).<br>• Page deletion cleans up `/Fields` arrays in the document `/AcroForm` to avoid orphaned field references. | Native tree update in `page_ops.rs` and MuPDF bridge. |
| **Bookmarks / Outlines** | **Destination Preservation & Target Re-anchoring:**<br>• Bookmarks targeting deleted pages are either pruned or converted to point to the nearest surviving page.<br>• Bookmarks targeting reordered pages retain their visual page target through object reference tracking (`[page_obj /Fit]`), NOT physical page indices. | `Outlines` destination walker preserves indirect object references. |
| **Named Destinations** | **Name Tree Preservation:**<br>• Targets in `/Dests` and `/Names /Dests` pointing to indirect page references remain valid through page reordering.<br>• Targets pointing to deleted pages are flagged or cleared. | Re-indexing of named destination table upon transaction commit. |
| **Page Labels (`/PageLabels`)** | **Dynamic Catalog Adjustment:**<br>• Deleting or inserting pages automatically recalculates the `/PageLabels` number tree so that logical numbering (e.g. `i, ii, 1, 2, A-1`) remains coherent. | Managed by core `PageLabelCatalog` in `vocument-core/src/navigation.rs`. |
| **Internal Links (`/Link`)** | **Hyperlink Target Validation:**<br>• Hyperlinks targeting surviving pages are updated to reflect new page sequences.<br>• Hyperlinks targeting deleted pages are neutralized (action removed) with a non-destructive warning. | Scan and update of `/Annots` `/Link` entries. |
| **Tagged PDF / PDF/UA** | **Structure Tree (`/StructTreeRoot`) Safety:**<br>• When page operations are performed on Tagged PDFs, the `/ParentTree` must be updated to prevent dangling MCIDs.<br>• If structural integrity cannot be fully preserved, the document retains an accessibility warning: *"Document structure modified; PDF/UA re-validation recommended."* | Core inspection of `/MarkInfo` dictionary. |
| **Document Encryption** | **Permission Enforcement:**<br>• Documents protected by user/owner passwords cannot undergo page operations unless permissions grant `ModifyContents` / `AssembleDocument`. | Verified via `PdfDocumentSummary::needs_password` and MuPDF permissions. |

---

### 2. Transaction Architecture & Inverse Operation Mapping

All Page Workbench mutations implement `TransactionCommand` with fully deterministic forward and inverse commands:

1. **Reorder Pages:**
   - **Forward:** `ReorderPages { order: Vec<u32> }` (e.g., permutation `[2, 0, 1]`)
   - **Inverse:** `ReorderPages { order: compute_inverse_permutation(&order) }` (e.g., `[1, 2, 0]`)
   - **Properties:** In-place permutation; zero document byte loss; perfectly reversible.
2. **Duplicate Page:**
   - **Forward:** `DuplicatePage { page_index: u32 }`
   - **Inverse:** `DeletePages { page_indices: vec![page_index + 1] }`
   - **Properties:** Reversible; undoing deletes the newly grafted copy.
3. **Delete Pages:**
   - **Forward:** `DeletePages { page_indices: Vec<u32> }`
   - **Inverse:** `RestorePages { previous_bytes: before_bytes }`
   - **Properties:** Restores prior state from in-memory transaction buffer without reading from disk.
4. **Extract Pages (Query, Non-Mutating):**
   - Pure read query: `extract_pages(session_id, &[u32]) -> Vec<u8>`.
   - Leaves source `DocumentSession` untouched (revision unchanged, `is_dirty == false`).

---

### 3. Native Engine Execution via MuPDF Page Grafting

In `engines/pdf-mupdf/csrc/mupdf_page_bridge.c`:
- Utilizes MuPDF's `pdf_graft_page(ctx, dst_doc, dst_page_index, src_doc, src_page_index)` to clone pages across document contexts.
- Clones all page dictionary resources, font descriptors, and content streams with deduplication.
- Serializes the modified document directly to memory via `pdf_save_document` with `do_garbage = 2` to eliminate orphaned objects.

---

## Consequences

### Positive
- Robust, crash-proof page operations that never corrupt PDF structure or leave broken cross-references.
- Instant, fully reversible Undo/Redo across all operations.
- Clean separation between core transaction orchestration and native UI presentation.

### Negative / Trade-offs
- Digital signatures cannot be preserved when pages are manipulated (a fundamental property of digital cryptography). The UI must communicate this clearly to avoid user surprise.
