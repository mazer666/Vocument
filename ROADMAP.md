# Vocument Roadmap

**Roadmap version:** 0.1  
**Current milestone:** M0 — Foundation / Trustworthy Viewer

The roadmap is milestone-driven. A milestone is not complete because “the UI seems to work”; it is complete only when its functional, technical, performance, usability, security and interoperability gates are satisfied.

---

## Milestone model

Every milestone has:

- scope
- explicit non-goals
- functional acceptance criteria
- technical acceptance criteria
- performance budgets
- usability criteria
- security/privacy criteria
- interoperability criteria
- documentation criteria
- QA evidence
- release decision

See `QUALITY.md` and `docs/acceptance/ACCEPTANCE_TEMPLATE.md`.

---

# M0 — Foundation / “Trustworthy Viewer”

## Purpose

Prove that the architecture works end-to-end on both macOS and Android before adding feature breadth.

## M0.0 — Repository & Build Skeleton (Status: `done` — see [`docs/acceptance/M0.0-ACCEPTANCE.md`](docs/acceptance/M0.0-ACCEPTANCE.md))

Deliver:

- Cargo workspace
- macOS app project
- Android app project
- UniFFI binding generation
- CI
- licence/DCO/REUSE setup
- central config structure
- documentation checks
- trivial Rust call successfully invoked from Swift and Kotlin

### Gate

A clean clone can be bootstrapped and built by following documented commands without tribal knowledge (`make check` / `./scripts/local-ci.sh`).

---

## M0.1 — FileBroker + MuPDF session (Status: `done` — see [`docs/acceptance/M0.1-ACCEPTANCE.md`](docs/acceptance/M0.1-ACCEPTANCE.md))

Deliver:

- open PDF from Finder/Open With on macOS
- open PDF through Storage Access Framework on Android
- create `DocumentSession`
- read page count/basic metadata
- assign stable internal session/document IDs
- file identity/fingerprint foundation

### Gate

No UI layer accesses MuPDF directly (enforced automatically in `./scripts/validate-repo.sh`).

---

## M0.2 — Rendering *(COMPLETED — see [`docs/acceptance/M0.2-ACCEPTANCE.md`](docs/acceptance/M0.2-ACCEPTANCE.md))*

Deliver:

- `DocumentActor`
- display-list/render worker pipeline
- adaptive tile cache
- native `DocumentCanvas`
- scroll
- zoom
- neighbouring-page prefetch
- memory-pressure handling

### Performance target

On reference hardware, aim for:

- normal PDF first page visible within 300–500 ms after document load path is ready *(achieved: ~51 ms on macOS, ~56 ms on Kotlin/Android)*
- 60 fps-oriented scroll/zoom behaviour *(achieved: <0.1 ms tile cache hits, <2 ms display-list sub-tile render)*
- UI remains responsive for large documents

Exact device-specific gates are refined during implementation.

---

## M0.3 — Navigation & Classic Search *(COMPLETED — see [`docs/acceptance/M0.3-ACCEPTANCE.md`](docs/acceptance/M0.3-ACCEPTANCE.md))*

Deliver:

- thumbnails
- logical page labels
- page navigation
- text selection
- cleaned plain-text copy
- FTS5 index
- progressive indexing
- classic exact search

### Gate

Search begins returning useful results before a very large document is fully indexed. *(Verified in Rust, Swift, and Kotlin tests — < 1 ms query latency on partially indexed documents.)*

---

## M0.4 — First mutations *(Status: **COMPLETE** — see [M0.4 Acceptance Record](docs/acceptance/M0.4-ACCEPTANCE.md))*

Deliver:

- [x] highlight annotation (`PdfHighlightAnnotation`, standard PDF `/Subtype /Highlight` with `/QuadPoints`, `/C`, `/CA`, `/NM`, `/T`, `/Contents` in `engines/pdf-mupdf/csrc/mupdf_mutate_bridge.c`)
- [x] page rotation (`vocument_mupdf_rotate_pages` updating `/Rotate` by multiples of 90° and swapping page geometry `595000x842000` $\leftrightarrow$ `842000x595000`)
- [x] semantic transaction commands (`TransactionCommand::RotatePages`, `AddHighlightAnnotation`, `RemoveHighlightAnnotation` with human-readable descriptions in `core/vocument-core/src/transactions.rs`)
- [x] Undo/Redo (`TransactionManager` with `undo`, `redo`, `can_undo`, `can_redo`, `is_dirty`, `undo_description`, `redo_description` exposed across UniFFI to macOS and Android)
- [x] persistent transaction representation (`PersistentTransactionJournal` v1 with SHA-256 record & journal checksums, JSON export, integrity verification, and deterministic session replay)

### Gate

Mutation never bypasses `TransactionManager`. *(Enforced via `pub(crate)` visibility on `DocumentActor::apply_engine_mutation_for_transaction_manager`, automated CI check in `scripts/validate-repo.sh`, and end-to-end Rust/Swift/Kotlin tests.)*

---

## M0.5 — Safe Save

Deliver:

- atomic save path
- save validation
- Save As
- file identity
- external-change detection
- conflict UI contract
- collision-free safe-copy option

### Gate

No silent overwrite of externally changed content.

---

## M0.6 — Recovery

Deliver:

- per-document recovery container
- MuPDF journal integration
- semantic transaction journal
- periodic checkpoint
- crash detection
- Restore / Preview / Discard / Open as New

### Required destructive test

1. open document
2. make unsaved edit
3. hard-kill process
4. restart
5. detect recovery
6. preview/restore
7. confirm original remains intact until explicit save

---

## M0.7 — Interoperability Gate

Test changed documents in:

- MuPDF reopen
- reference renderer
- macOS Preview
- Adobe Acrobat where available
- PDF-XChange where available
- Chrome/Chromium PDF
- Firefox PDF

At minimum verify:

- highlights
- rotation
- text/search behaviour
- page labels
- saved structure remains valid

### M0 Definition of Done

The same scenario is green on macOS and Android:

`Open → search → select text → highlight → rotate → undo → redo → save → hard-kill → recover → save → reopen in foreign viewer`

M0 also has recorded evidence for:

- startup
- first page
- scroll responsiveness
- RAM
- search latency
- save time
- recovery success
- visual regression

---

# M1 — Android Scan & OCR

## Purpose

Deliver Vocument's first strong mobile differentiator while reusing M0 foundations.

Pipeline:

`Camera → edge assist → capture → perspective correction → cleanup → OCR → searchable PDF → search → save/share`

Deliver:

- native camera capture
- automatic page detection assistance
- perspective correction
- deskew/rotation
- intelligent cleanup
- multi-page scan
- intelligent batch mode
- Tesseract DE/EN
- searchable PDF
- OCR text integrated into search
- background/resumable jobs
- battery/thermal rules
- adaptive large-document behaviour

### Quality gates

- OCR benchmark corpus
- scan cleanup visual comparison
- blur/duplicate/blank-page behaviour tests
- perspective accuracy tests
- no UI blocking during OCR

---

# M2 — macOS Professional Review

## Purpose

Deliver the first professionally useful desktop workflow.

Deliver:

- comments/review sidebar
- highlights/annotations
- editable jurist tool palette
- stamps
- secure redaction + verification
- structured document comparison
- signature status and details
- links/actions inspection
- attachments safety inspection
- Save/Recovery/Undo integration for all review actions

### Quality gates

- redaction extraction/search verification
- compare correctness corpus
- signature revision safety tests
- interoperability with major PDF tools
- keyboard workflow review

---

# M3 — Page Workbench

Deliver:

- merge
- split
- extract
- insert
- reorder
- duplicate
- crop
- page boxes
- auto crop
- rotate/orientation/deskew
- numbering
- header/footer
- bookmarks
- watermarks/stamps profiles
- image/vector/table extraction
- compression/optimisation
- professional print/export improvements

### Best-in-class gate

For each tool compare Vocument with PDFCraft and relevant commercial tools. The acceptance question is not “does Vocument have Merge?”, but:

> Is the workflow faster/clearer, undoable, recoverable, previewable and interoperable?

---

# M4 — Authoring / FlowDocument / Office

Deliver:

- FlowDocument core
- deterministic Rust layout
- native editing input
- `.pdoc` v1
- shared FontService
- PdfExportEngine
- PDF/A/PDF/UA-oriented authored export
- OfficeEngine bake-off
- selected DOCX/ODT adapter
- styles
- lists
- tables
- images
- sections
- header/footer
- footnotes/endnotes
- comments
- track changes
- clean/markup/original views

### Gate

A `.pdoc` edited on macOS and Android must retain deterministic pagination under the same available-font conditions.

DOCX/ODT support must pass the roundtrip corpus before being labelled “supported for editing”.

---

# Later milestones / tracks

These are intentionally not scheduled before core maturity:

- advanced semantic/legal compare
- local semantic search model packs
- QES/A-Trust production integration
- public plugin SDK
- plugin ecosystem/distribution
- worker-process hardening
- web evidence capture
- chain of custody/evidence packages
- handwriting intelligence
- advanced damaged-PDF forensics
- Windows/Linux
- iPadOS/iOS licensing/distribution gate
- legal source modules
- CLI/automation server mode

---

# Roadmap change rule

A milestone scope may change only through a documented Change Request if it affects accepted product requirements or release gates.

A coding agent must not silently move an unfinished item into a later milestone just to make a milestone “green”.
