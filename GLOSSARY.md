# Vocument Glossary

**DocumentSession** — App-owned live state for one opened document.

**DocumentActor** — Serialised mutation/parsing owner for a PDF session; prevents unsafe concurrent access to the mutable engine document.

**Transaction** — Human-meaningful document change supporting undo/recovery.

**Checkpoint** — Periodic recoverable state used so recovery need not replay an unbounded transaction chain.

**Recovery Container** — Per-document internal persisted journal/checkpoint area.

**FileBroker** — Platform-specific controlled file-access boundary.

**NetworkBroker** — Central policy gate for all external network access.

**PdfEngine** — Vocument-defined PDF engine interface; initial implementation uses MuPDF.

**PdfEditPlan** — Explicit description of a PDF edit, preserved/replaced objects, geometry, fonts and validation impact.

**FlowDocument** — Vocument's reflowable authored-document model; separate from PDF.

**OfficeEngine** — Adapter interface for DOCX/ODT import/export/preservation.

**OcrProvider** — Replaceable OCR provider interface; initial provider Tesseract.

**ImagingEngine** — Shared Rust image cleanup/normalisation pipeline.

**JobManager** — Central scheduler for render/OCR/index/export/compression jobs.

**AppCatalog** — Small global SQLite bookkeeping store; never a document-content store.

**Preset** — Default behaviour shown/used for normal users.

**Optional** — Available capability not enabled/prominent by default.

**Later** — Accepted direction intentionally deferred beyond current milestone.

**ADR** — Architecture Decision Record.

**CR** — Change Request for material product/requirement changes.

**DCO** — Developer Certificate of Origin for external contributions.

**Best in Class** — Measurable quality target, not a feature-count or marketing claim.
