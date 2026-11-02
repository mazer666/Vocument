# Tagged PDF/A-2b and PDF/UA-1 export (M4.4)

**Requirements:** `WRITE-001`, `APP-A11Y-001`, `TRACK-001`–`006` · **ADR:** [ADR-0020](../adr/ADR-0020-flowdocument-architecture-deterministic-rust-layout-and-pdoc.md) · **Code:** `core/vocument-layout/src/pdf_export.rs` and `src/pdf_export/`

## What the export writes

| Part | File | Purpose |
|---|---|---|
| Embedded Lato Regular, `/Widths` read from the font program, `/ToUnicode` | `font_embedding.rs` | ISO 19005-2 6.2.11, ISO 14289-1 7.21 |
| sRGB output intent with embedded ICC profile | `font_embedding.rs` | ISO 19005-2 6.2.3 |
| XMP packet: `pdfaid`, `pdfuaid` (with extension schema), `dc`, `xmp` | `xmp.rs` | PDF/A and PDF/UA identification; text is XML-escaped |
| Structure tree: `Document`, `H1`–`H6`, `P`, `L/LI/LBody`, `Table/TR/TH/TD`, `Sect`, `Annot`, `/ParentTree` | `structure.rs` | Tagged PDF; the semantic path of every line comes from the `FlowDocument` block, not from its identifier |
| Page content: marked content for text, `/Artifact` for backgrounds, borders, rules, headers, footers | `page_content.rs` | ISO 14289-1 7.1 |
| Comment annotations (text, replies, resolved state) with appearance stream, print flag and `/StructParent` | `pdf_export.rs` | ISO 19005-2 6.3, ISO 14289-1 7.18 |
| Deterministic trailer `/ID` | `pdf_export.rs` | ISO 19005-2 6.1.3; same input gives the same bytes |

## Evidence

- `cargo test -p vocument-layout` runs `veraPDF --flavour 2b` and `--flavour ua1` against a document with heading, paragraph, table with header row, list, callout and comment (`tests/pdf_export_conformance_tests.rs`). Both are **compliant** with veraPDF 1.30.3.
- `scripts/verify-pdfa-compliance.sh` repeats the check on the demo export and fails when veraPDF rejects it.
- Where veraPDF is not installed, the tests and the script print **NOT RUN**. That proves nothing about conformance.

## Known limitations

- **PAC** (PDF Accessibility Checker) is a Windows tool and has **not** been run. `PdfExportOptions::certified_by_external_tooling` therefore stays `false`, and the XMP description still says that certification is pending.
- **Images** are not exported (the layout tree carries no asset resolver); image blocks need `/Figure` with alt text once they are.
- **One font:** every run uses Lato Regular. Bold is drawn as stroked text, italic as a sheared matrix. Underline and strikethrough of tracked changes are not drawn yet.
- **Layout versus glyph widths:** the layout engine measures with calibrated metrics, the PDF embeds the real glyph widths. Each run gets a character-spacing correction (capped at 12 % of the font size) so it stays inside the layout geometry; the layout engine itself still has to adopt the embedded font for exact agreement (ADR-0020).
- Characters outside WinAnsi are written as `?`.
