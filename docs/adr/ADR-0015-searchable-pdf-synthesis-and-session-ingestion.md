# ADR-0015: M1.4 — Searchable PDF Synthesis & Session Ingestion

- **Status:** Accepted
- **Date:** 2026-10-19
- **Author:** Vocument Core & PDF/OCR Engineering Team
- **Deciders:** Chief Architect, PDF/MuPDF Engineer, Rust Core Engineer, QA Lead
- **Requirement IDs:** `PDF-001`, `PDF-002`, `OCR-004`, `SEARCH-001`, `SEARCH-002`, `SESSION-001`, `CODE-001`, `NET-001`

---

## Context & Problem Statement

Milestone M1.4 requires synthesizing standard-conforming searchable PDF documents directly from camera scan sessions. Scanned document pages (rectified by M1.1 and cleaned by M1.2) must be combined with the hierarchical OCR text and bounding boxes (extracted by M1.3) into a single PDF document.

The text layer must be completely invisible to the human eye (`3 Tr` rendering mode) while remaining fully selectable, searchable, and accessible by PDF viewers (MuPDF, Apple Preview/Quartz, Chromium PDFium, Adobe Acrobat) and directly ingested into Vocument's application-managed `DocumentSession` for immediate progressive SQLite FTS5 search.

## Decision Drivers

1. **Standard Conformance & Interoperability (`PDF-001`):** Produce clean PDF 1.7 syntax without external non-standard dependencies that renders identically across MuPDF, Quartz, and PDFium.
2. **Invisible Text Layer Fidelity (`3 Tr`):** Position text tokens with spatial precision matching the underlying scanned image glyphs. This requires exact baseline alignment (`Tm`) and horizontal font scaling (`Tz`).
3. **Encoding & Diacritics (`I18N-002`):** Handle German umlauts (`ä`, `ö`, `ü`, `ß`) and Austrian legal symbols (`§`) reliably in standard WinAnsiEncoding using octal character escapes.
4. **Zero-Copy Ingestion (`SESSION-001`):** Ingest synthesized PDF bytes directly into `DocumentSession` via `FileAccessOrigin::InMemoryBuffer` without requiring intermediate disk writes or SAF re-permission hops.
5. **Progressive SQLite FTS5 Search (`SEARCH-001`, `SEARCH-002`):** Ensure the synthesized PDF is immediately indexable and searchable via the existing FTS5 search pipeline.
6. **Code Simplicity & `CODE-001`:** Keep hand-written source files $\le 600$ lines (hard limit 900 lines).

---

## Architectural Decisions

### 1. Embedded PDF 1.7 Synthesizer (`engines/ocr/src/synthesis.rs`)

Rather than pulling in heavy, opinionated third-party PDF generators that introduce dependency risk or bloat, `vocument-ocr` implements a lightweight, robust PDF 1.7 synthesizer:

- **Objects:**
  - `1 0 obj`: Document Catalog (`/Pages 2 0 R`).
  - `2 0 obj`: Page Tree (`/Kids [...] /Count N`).
  - `3 0 obj`: Font Resource (`/Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >> >>`).
  - Per Page:
    - Page Object (`/Parent 2 0 R /MediaBox [0 0 W H] /Contents [...] /Resources [...]`).
    - Image XObject (`/Subtype /Image /Width W /Height H /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode`).
    - Content Stream: Draws background image (`q W 0 0 H 0 0 cm /Im1 Do Q`) and overlays invisible text layer (`BT 3 Tr ... ET`).
  - Cross-Reference Table (`xref`) and Trailer (`trailer << /Size ... /Root 1 0 R >>`).

### 2. Invisible Text Layer Operator Pipeline (`BT 3 Tr ... ET`)

To align text with OCR bounding boxes:
- **Text Rendering Mode:** `3 Tr` (neither fill nor stroke text, invisible to eye but indexed and selectable).
- **Font & Size:** `/F1 <height_pt> Tf` where font size matches bounding box height in PDF points.
- **Horizontal Scaling (`Tz`):** Calculated as `(target_box_width / natural_helvetica_width) * 100.0`. This ensures that word highlight boxes in PDF viewers precisely enclose the scanned text.
- **Baseline Offset (`Tm`):** PDF text matrix coordinates `(x, y)` represent the baseline of the font. For Helvetica, the baseline is calculated with a $0.18 \times h$ offset above the bottom of the bounding box: `y_pdf = (1.0 - bbox.y1) * page_h + h_pt * 0.18`.

### 3. Direct Session Ingestion & FTS5 Search

Through `SessionRegistry::open_brokered_file`:
```rust
let brokered = BrokeredFileRead {
    source_token: format!("scan://{}", scan_session_id),
    display_name: format!("{}.pdf", display_name),
    access_origin: FileAccessOrigin::InMemoryBuffer,
    bytes: pdf_bytes,
    modified_unix_ms: Some(now_ms),
    os_file_id: Some(format!("scan_{}", scan_session_id)),
};
let session = session_registry.open_brokered_file(brokered)?;
```
Once opened in `DocumentSession`, the document can immediately be indexed in batches via `index_search_batch`, enabling instant full-text search across all scanned pages.

---

## Consequences & Verification

- **Positive:** Scanned documents become fully searchable, selectable PDFs without external cloud tools.
- **Positive:** Verified with `mutool info` and `mutool draw` without warnings or syntax errors.
- **Positive:** MuPDF `extract_page_text` roundtrip confirms Austrian legal terminology (`ABGB`, `UWG`, `Vertraulichkeit`, `Wien`) is 100% recovered from the invisible text layer.
- **Positive:** Tested on real Android hardware / emulator, passing device gate, unit tests, and Kotlin handshake CLI.
- **Positive:** Full adherence to `CODE-001` (`synthesis.rs` is 384 lines).
