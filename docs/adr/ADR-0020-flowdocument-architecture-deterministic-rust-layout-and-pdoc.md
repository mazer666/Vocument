# ADR-0020: FlowDocument Architecture, Deterministic Rust Layout, and `.pdoc` Specification (M4)

- **Status:** Accepted
- **Date:** 2026-10-23
- **Author:** Chief Architect, Document/Office Engineer, Rust Core Engineer
- **Deciders:** ROLE-01 (Product Steward), ROLE-02 (Chief Architect), ROLE-08 (Document/Office Engineer), ROLE-11 (Accessibility Engineer), ROLE-13 (QA Lead), ROLE-15 (Standards Lead)
- **Requirement IDs:** `FLOW-001`, `FLOW-002`, `PDOC-001`, `LAYOUT-001`, `OFFICE-001`, `OFFICE-002`, `A11Y-001`, `CODE-001`

---

## Context & Problem Statement

Milestone M4 ("Authoring / FlowDocument / Office") transitions Vocument from a pure review and page-manipulation workstation to an authentic document authoring environment. 

Existing solutions suffer from three fundamental architectural flaws:
1. **Platform Layout Drift:** Documents opened across macOS, Android, Windows, and Linux reflow unpredictably because each platform relies on its own proprietary text layout engine (macOS CoreText, Windows DirectWrite, Android Minikin/HarfBuzz, Linux Pango). A page boundary on macOS shifts to the next page on Android, breaking pagination, table cells, and page references.
2. **Proprietary Format Lock-In:** Traditional office suites use sprawling, undocumented, or heavily legacy-laden XML containers (DOCX, ODF) that make clean diffing, offline transaction journaling, and zero-loss semantic editing nearly impossible.
3. **Unverifiable Roundtrip Claims:** Many editors claim "100% Word compatibility" while silently stripping advanced typography, tracked changes, styles, and field codes upon save.

This ADR defines the technical architecture for Vocument's native authoring system: the reflowable `FlowDocument` data model, the deterministic cross-platform Rust layout engine, the open `.pdoc` document format, and the strict phased approach to Office (DOCX/ODT) interoperability.

---

## Decision Drivers

1. **Deterministic Pagination across All Platforms (`FLOW-001`):** A `.pdoc` document must produce 100% bit-identical line breaks, word coordinates, table cell splits, and page counts regardless of whether it is rendered on macOS Apple Silicon, Android arm64, Windows x86_64, or Linux.
2. **Open, Versioned Container (`PDOC-001`):** A modern, transparent, package-based format using standard ZIP64, UTF-8 JSON, schema versioning, and unknown-field preservation.
3. **Native Platform Input & Accessibility:** Native macOS TextKit 2 and Android Jetpack Compose text entry for keyboard, IME, dictation, and screen-readers, while delegating layout geometry to the shared Rust core.
4. **Professional Typography & Legal Authoring:** Advanced typography (Knuth-Plass line breaking, optical margin alignment, hyphenation dictionaries), legal numbering hierarchies, footnotes, margin comments, and tracked changes.
5. **PDF/A & PDF/UA Authored Output:** Direct export of high-fidelity, fully tagged PDFs with semantic structure trees and alt-text out of the box.
6. **No Fake Compatibility:** Strict bake-off corpus evaluation before any DOCX/ODT claim is accepted.

---

## Architectural Decisions

### 1. The `FlowDocument` Semantic Data Model (`core/vocument-flow`)

`FlowDocument` models authored, reflowable content completely independently from fixed-page PDF syntax:

```text
FlowDocument
├─ DocumentMetadata (UUID, author, creation/mod timestamps, revision)
├─ Stylesheet (ParagraphStyles, CharacterStyles, TableStyles, ListStyles)
├─ Sections: Vec<Section>
│   ├─ SectionGeometry (page size, orientation, margins, columns, header/footer links)
│   └─ Blocks: Vec<Block>
│       ├─ Paragraph (spans of text, inline styles, alignments, tab stops)
│       ├─ Heading (level 1–6, numbering link)
│       ├─ List (ordered, bulleted, legal multilevel with custom prefix/suffix)
│       ├─ Table (cells, rowspans, colspans, borders, repeating header rows)
│       ├─ Image (raster/vector asset ref, intrinsic size, caption, alt-text)
│       └─ Callout / Blockquote
├─ ReviewData
│   ├─ Comments: Vec<FlowComment> (anchored to character ranges, thread replies)
│   └─ TrackedChanges: Vec<ChangeRecord> (Insert, Delete, FormatChange with author/timestamp)
└─ Footnotes: BTreeMap<FootnoteId, Vec<Block>>
```

### 2. The Deterministic Rust Layout Engine (`core/vocument-layout`)

All layout calculations execute exclusively in the shared Rust core, never in platform-specific UI toolkits:

1. **Text Shaping & Font Abstraction:**
   - Unified font parsing using `ttf-parser` and OpenType glyph shaping via `rustybuzz` (a memory-safe HarfBuzz implementation in pure Rust).
   - Unicode segmentation (grapheme clusters, word boundaries, sentence boundaries) and bi-directional text layout (UAX #9) via `icu_segmenter` and `unicode-bidi`.
2. **Line Breaking Algorithm:**
   - Dual-mode line breaking:
     - *Fast Greedy:* For real-time typing in active paragraphs.
     - *Knuth-Plass Optimal Paragraph Breaking:* For quiescent layout and export, minimizing raggedness and hyphenation frequency across paragraphs.
3. **Deterministic Pagination Pipeline:**
   - Multi-column, multi-page layout engine calculating exact floating box placement, footnote splitting, table page-breaks with repeated header rows, and orphan/widow suppression.
   - **Cross-Platform Determinism Guarantee:** Given the identical `.pdoc` and font files, the pagination output (page index, line breaks, word bounding boxes) is 100% bit-identical across macOS, Android, Windows, and Linux.

### 3. The `.pdoc` Document Format Specification

`.pdoc` is a standardized, open ZIP container:

```text
MyDocument.pdoc (ZIP64 archive)
├─ manifest.json     # Format version ("1.0"), UUID, checksums
├─ document.json     # Complete FlowDocument block tree in UTF-8
├─ styles.json       # Palette, font families, paragraph/table styles
├─ metadata.json     # Dublin Core, custom legal metadata
├─ assets/           # Embedded PNG, JPEG, SVG, or subsetted fonts
└─ review.json       # Optional: Tracked changes and comment threads
```

- **Schema Evolution:** Every JSON document includes `schema_version`. Unknown fields in newer format versions are preserved intact during save (forward compatibility).
- **Security:** Standard zip-slip and bomb-decompression guards (maximum uncompressed size limits, directory traversal checks).

### 4. Native Platform Integration

- **macOS (`apps/macos`):**
  - Custom AppKit/SwiftUI canvas view.
  - Native NSTextInputClient protocol handling complex IME (CJK input), accent popups, and macOS dictation.
  - Rust core delivers visual display tiles and cursor coordinate mapping.
- **Android (`apps/android`):**
  - Jetpack Compose custom Layout with touch/stylus cursor positioning and software keyboard (IME) synchronization.

### 5. Office Interoperability Strategy (DOCX / ODT)

Vocument rejects hasty, bug-ridden format converters. Office format integration follows a strict three-phase gate:

1. **Phase 1 — High-Fidelity Corpus Bake-off (M4.4):**
   - Assemble a curated test corpus of 200 real-world legal and technical DOCX and ODT documents (styles, nested tables, footnotes, multilevel lists, tracked changes).
   - Evaluate candidate Rust engines (e.g. `docx-rs`, custom parser) against structural fidelity metrics.
2. **Phase 2 — Reliable Import (M4.5):**
   - Translate DOCX/ODT structures into `FlowDocument` without data loss. Elements that cannot be mapped natively are preserved in an unmapped extension store.
3. **Phase 3 — Authored Export:**
   - Export `FlowDocument` to clean, valid DOCX and ODT with zero schema corruption.
   - **Strict Rule:** Never claim roundtrip fidelity without verifiable corpus pass evidence.

---

## Consequences

### Positive
- True cross-platform authoring with guaranteed layout consistency.
- Open, future-proof `.pdoc` format immune to proprietary vendor lock-in.
- Foundation for professional PDF/A and PDF/UA document production.

### Negative / Trade-offs
- Writing a deterministic layout engine in Rust requires significant initial engineering investment compared to wrapping webviews or platform text engines.
