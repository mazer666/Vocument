# Office Interoperability (M4.5 — DOCX & ODT)

Vocument provides native, offline-first import and export adapters for **OOXML WordprocessingML (`.docx`)** and **OpenDocument Text (`.odt` / `.ott`)** in `engines/office` (`vocument-office`), bridging foreign Office documents directly into the canonical `FlowDocument` tree (`core/vocument-flow`), deterministic Rust layout engine (`core/vocument-layout`), and `.pdoc` container (`formats/pdoc`).

---

## 1. Architecture & Data Flow

```text
.docx / .odt file bytes
   │
   ▼  Security preflight (ZIP limits, path traversal, XXE/DTD, macros, external rels)
engines/office (import_docx / import_odt / import_office_bytes)
   │
   ├──► FlowDocument (canonical blocks, inline spans, styles, metadata, comments, tracked changes)
   ├──► AssetMap     (SHA-256 deduplicated embedded images: assets/<sha256>.<ext>)
   └──► ConversionReport (structured passthrough, approximation, loss & security entries)
   │
   ▼  platform/vocument-ffi (FlowEditorSession + PdocRecoveryStore)
   ├──► macOS native editor (NSTextInputClient, PdocAuthoringCanvasView)
   └──► Android native editor (Compose PdocEditorScreen, DocumentCoreBridgeEditor)
   │
   ▼  Export
   ├──► .pdoc (native container with full AssetMap & lossless roundtrip)
   ├──► .docx (export_docx — WordprocessingML package)
   ├──► .odt  (export_odt — ODF 1.2/1.3 package)
   └──► .pdf  (Tagged PDF/A-2b & PDF/UA-1 with /ActualText Unicode preservation)
```

---

## 2. Supported Features (`OFFICE-001`–`005`)

| Category | DOCX (`.docx`) | ODT (`.odt` / `.ott`) | FlowDocument Mapping |
|---|---|---|---|
| **Paragraphs & Headings** | `w:p`, `w:pStyle` (`Heading1`–`6`, `w:outlineLvl`) | `text:p`, `text:h` (`text:outline-level="1..6"`) | `Block::Paragraph`, `Block::Heading` (`H1`–`H6`) |
| **Inline Styling** | `w:b`, `w:i`, `w:u`, `w:strike`, `w:vertAlign`, `w:rFonts`, `w:sz`, `w:color`, `w:highlight` | `fo:font-weight`, `fo:font-style`, `style:text-underline-style`, `style:text-line-through-style`, `style:text-position`, `fo:color`, `fo:background-color` | `InlineStylePatch` (`bold`, `italic`, `underline`, `strikethrough`, `superscript`, `subscript`, font, size, color, highlight) |
| **Style Hierarchy** | `word/styles.xml` (`w:basedOn`, `w:pPr`, `w:rPr`) | `styles.xml` & `content.xml` (`style:parent-style-name`, `style:paragraph-properties`, `style:text-properties`) | `StyleRegistry` (`ParagraphStyle`, `CharacterStyle` with inheritance) |
| **Lists** | `word/numbering.xml` (`w:abstractNum`, `w:num`, decimal/bullet/roman/letter, multi-level) | `text:list` / `text:list-item` (`text:list-style`, nested levels) | `Block::List` (`Ordered`, `Unordered`, `Task`, nested sub-lists) |
| **Tables** | `w:tbl`, `w:tblGrid`, `w:tr`, `w:tc`, `w:tblHeader`, `w:gridSpan`, `w:vMerge`, `w:shd` | `table:table`, `table:table-header-rows`, `table:table-row`, `table:table-cell`, `table:number-columns-spanned`, `table:number-rows-spanned` | `Block::Table` (`header_rows`, `col_span`, `row_span`, `width_milli_pt`, `background_color`) |
| **Images** | `w:drawing` (`wp:inline`, `wp:anchor`, `a:blip r:embed`, EMU $\leftrightarrow$ milli-pt) | `draw:frame` / `draw:image` (`xlink:href="Pictures/..."`, `svg:width`, `svg:height`) | `Block::Image` + SHA-256 content-addressed `AssetMap` |
| **Footnotes & Endnotes** | `word/footnotes.xml`, `word/endnotes.xml`, `w:footnoteReference` | `text:note` (`text:note-class="footnote\|endnote"`, `text:note-citation`, `text:note-body`) | `DocumentMetadata.footnotes` + inline superscript marker |
| **Headers & Footers** | `word/header*.xml`, `word/footer*.xml`, `PAGE` / `NUMPAGES` fields | `style:master-page` (`style:header`, `style:footer`, `text:page-number`, `text:page-count`) | `SectionProperties.header` / `footer` (`{page}`, `{total_pages}` placeholders) |
| **Page Geometry** | `w:sectPr` (`w:pgSz`, `w:pgMar`, twips $\leftrightarrow$ milli-pt) | `style:page-layout-properties` (`fo:page-width`, `fo:page-height`, `fo:margin-*`) | `PageSetup` (`width_milli_pt`, `height_milli_pt`, `Margins`) |
| **Comments** | `word/comments.xml`, `w:commentRangeStart`, `w:commentRangeEnd` | `office:annotation` (`dc:creator`, `dc:date`, `office:annotation-end`) | `CommentAnchor` (anchored `BlockId` + char offsets, replies, resolved state) |
| **Tracked Changes** | `w:ins`, `w:del` (`w:delText`), `w:rPrChange` | `text:tracked-changes`, `text:changed-region`, `text:insertion`, `text:deletion`, `text:format-change` | `TrackedChange` (`Insert`, `Delete`, `FormatChange` with author & timestamp) |

---

## 3. Security Hardening (`SEC-OFFICE`)

All incoming `.docx` and `.odt`/`.ott` archives are treated as untrusted inputs and validated through `engines/office/src/zip_guard.rs` and `engines/office/src/xml.rs`:

1. **Format & Encryption Detection:** Rejects OLE2 Compound File Binary (`D0 CF 11 E0` — legacy `.doc` or Office `EncryptionInfo`/`EncryptedPackage`) and password-encrypted ODF (`manifest:encryption-data` in `META-INF/manifest.xml`) with `OfficeError::EncryptedDocument`.
2. **ZIP Bomb & Resource Caps (`ZipLimits::default()`):**
   - Maximum compressed archive size: **100 MiB**
   - Maximum decompressed total bytes: **250 MiB**
   - Maximum single entry size: **64 MiB**
   - Maximum compression ratio per entry (> 64 KiB): **100:1**
   - Maximum entry count: **4,096**
3. **Path Traversal Protection:** Rejects absolute paths, drive prefixes, `..` segments, and NUL bytes.
4. **XML Hardening (`xml.rs`):** Rejects `<!DOCTYPE` / `<!ENTITY` declarations (XXE & Billion Laughs mitigation), enforces maximum element nesting depth (**256**), maximum attributes per element (**128**), and maximum text node size (**16 MiB**).
5. **Active Content & External Resource Stripping (Offline-First `NET-001`):**
   - Strips VBA macros (`word/vbaProject.bin`, `Basic/`, `Scripts/`) and ActiveX controls, recording a `Severity::Security` warning in `ConversionReport`.
   - Blocks external relationships (`TargetMode="External"` on image/frame/subdocument/OLE relationships) and external `xlink:href` URLs on ODF images/frames so opening a document never triggers network requests or local file inclusion.

---

## 4. Transparent Conversion Report (`OFFICE-004`)

Every import and export operation returns a structured `ConversionReport` (`FfiOfficeConversionReport` over UniFFI) with:
- `EntryCategory::Passthrough`: Construct imported/exported natively.
- `EntryCategory::Approximation`: Construct mapped to closest `FlowDocument` equivalent (e.g. `odt.template.styles_only`, custom tab stops).
- `EntryCategory::Loss`: Unsupported Office construct skipped while preserving visible fallback text where available (`docx.math.omml`, `docx.sdt.unwrapped`, `docx.DrawingML.shape`, `odt.draw.custom_shape`, `odt.text.section_columns`).
- `EntryCategory::Security`: Stripped macro, external resource, or active content (`docx.macro.vba_stripped`, `docx.external_resource.blocked`, `odt.external_image.blocked`).

---

## 5. Verification & Bake-Off Evidence

See [`docs/benchmarks/M4_5_OFFICE_BAKEOFF.md`](../benchmarks/M4_5_OFFICE_BAKEOFF.md) and `./scripts/office-bakeoff.sh` for the 180-document real-world corpus bake-off results (180/180 pass rate, 100% Tier 1 & Tier 2 preservation, 56/56 LibreOffice headless reopen & PDF render validation).

