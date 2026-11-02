# M4.5 Office Interoperability Corpus Bake-Off Report (`OFFICE-001`–`005`, `ADR-0020`)

**Milestone:** M4.5 — DOCX/ODT Interoperability & Roundtrip Hardening  
**Crate:** `vocument-office` (`engines/office`)  
**Runner:** `scripts/office-bakeoff.sh` (`cargo run -p vocument-office --example bakeoff`)

---

## 1. Corpus Composition & Transparency (`QA-003`)

In accordance with `QA-003` and `AGENTS.md`, Vocument distinguishes strictly between executed real-document verification and external proprietary corpora that are not bundled in the repository:

1. **78 Real LibreOffice ODF Documents (`.ott` / `.odt`):**
   - Sourced directly from `/Applications/LibreOffice.app/Contents/Resources/template/common` (business letters, CVs, modern/traditional correspondence, localization templates, presentation notes, and style-only `.ott` packages).
2. **78 Real LibreOffice-Generated OOXML Documents (`.docx`):**
   - Produced by converting the 78 real LibreOffice templates through headless LibreOffice (`soffice --headless --convert-to docx`), exercising real-world OOXML quirks (`<mc:AlternateContent>`, complex `PAGE`/`NUMPAGES` fields, outline-numbered heading styles, drawingML anchors, headers/footers, and table cell properties).
3. **24 Legal & Technical Reference Documents (12 `.docx` + 12 `.odt`):**
   - Multi-section legal contracts, briefs, statutes, and audit reports exercising headings (`H1`–`H6`), nested ordered/unordered lists, tables with `gridSpan`/`vMerge` (`number-columns-spanned`/`number-rows-spanned`), embedded PNG images (`assets/<sha256>.png`), footnotes, headers/footers with dynamic page fields, safe hyperlinks, anchored margin comments with replies and resolved state, and tracked changes (insertions, deletions, format changes).

**Total Executed Corpus:** **180 Documents** (`90 DOCX`, `90 ODT/OTT`).

> [!IMPORTANT]
> **External Corpus, Scope of Metrics & Microsoft Word Status (`QA-003` Disclosure):**
> - **Methodological Scope of the 100% Gate Metric:** The Tier 1 and Tier 2 percentages (`100.00%`) measure deterministic preservation across the **`Import -> FlowDocument -> Export -> Re-Import` roundtrip** for all supported `FlowDocument` constructs. **A 100% gate pass rate does NOT mean complete format-level losslessness for arbitrary foreign Office features:** unsupported constructs on initial import are recorded in `ConversionReport` (`83` losses, `57` approximations, and `2,766` unrendered passthrough entries across the 180 documents; see §3 below).
> - **External 200-Document Corpus:** The external 200-document proprietary customer/GovDocs corpus referenced in `ROADMAP.md` M4.5 is **not bundled in this repository** (`NOT BUNDLED`; evaluated locally on the 180-document LibreOffice + legal reference corpus).
> - **LibreOffice 26.2.2.2 (`soffice --headless`):** **EXECUTED & VERIFIED** (`56 / 56` sampled exported `.docx` and `.odt` documents reopened and rendered to PDF via `soffice --headless --convert-to pdf` without repair or error).
> - **Microsoft Word (Desktop macOS/Windows):** **`NOT RUN`** (Microsoft Word is not installed on the build/test host; no synthetic or simulated Word acceptance is claimed).

---

## 2. Three-Tier Fidelity Results (180-Document Run)

| Metric Tier | Metric (`Import -> FlowDocument -> Export -> Re-Import`) | Target (`M4.5` Gate) | Measured Result (`180` Docs) | Status |
|---|---|---|---|---|
| **Overall Gate** | Documents passing all Tier 1 & Tier 2 roundtrip thresholds | $\ge 90\%$ | **180 / 180 (100.00%)** | **PASS** |
| **Tier 1 (Must Preserve)** | Plain text character similarity (normalised LCS/bigram) | $\ge 99.5\%$ | **100.00%** | **PASS** |
| **Tier 1 (Must Preserve)** | Heading hierarchy (`H1`–`H6` level + text) | $100\%$ | **100.00%** | **PASS** |
| **Tier 1 (Must Preserve)** | Character style runs (bold / italic / underline / strike / code) | $\ge 95\%$ | **100.00%** | **PASS** |
| **Tier 1 (Must Preserve)** | Footnote & endnote definitions and inline references | $100\%$ | **100.00%** | **PASS** |
| **Tier 2 (High-Value Layout)** | Table cell count, spans (`col_span`/`row_span`), and cell text | $\ge 95\%$ | **100.00%** | **PASS** |
| **Tier 2 (High-Value Layout)** | Nested list structure (`Ordered`/`Unordered`/`Task`) | $\ge 95\%$ | **100.00%** | **PASS** |
| **Tier 2 (High-Value Layout)** | Embedded image asset count & point dimensions (`±1 pt`) | $\ge 95\%$ | **100.00%** | **PASS** |
| **Tier 3 (Transparency)** | Unsupported / passthrough / approximated items logged in `ConversionReport` | $100\%$ logged | `passthrough=2766`, `approximations=57`, `losses=83` | **PASS** |
| **External Validator** | LibreOffice headless reopen & PDF conversion (`soffice --headless --convert-to pdf`) | $100\%$ | **56 / 56 (100.00%)** | **PASS** |
| **External Validator** | Microsoft Word native reopen check | Manual / CI | **`NOT RUN`** | **NOT RUN** |

---

## 3. Complete Breakdown of Initial-Import `ConversionReport` Findings (`OFFICE-004`)

During the initial import of the 180 documents into `FlowDocument`, every construct not natively represented in `FlowDocument` is logged in `ConversionReport`:

| Code | Severity | Occurrences | Affected Docs | Technical Explanation & Impact |
|---|---|---:|---:|---|
| `odt.body.user-field-decls` | `Loss` | 58 | 57 | ODF `<text:user-field-decls>` declaration container at the start of `<office:text>` is skipped; visible field display text inside paragraphs is preserved. |
| `docx.header.non-default` | `Loss` | 12 | 12 | First-page/even-page specific `<w:headerReference w:type="first\|even">` is skipped because `Section` currently models one default header block list per section. |
| `odt.body.variable-decls` | `Loss` | 10 | 10 | ODF `<text:variable-decls>` declaration block is skipped; visible variable text inside paragraphs is preserved. |
| `docx.footer.non-default` | `Loss` | 3 | 3 | First-page/even-page specific `<w:footerReference w:type="first\|even">` is skipped because `Section` models one default footer block list per section. |
| `docx.drawing.textbox` | `Approximated` | 53 | 24 | Floating DrawingML `<wps:txbx>` / `<v:textbox>` frames are flattened into inline paragraph text rather than positioned as floating 2D canvas boxes. |
| `docx.image.inline-moved` | `Approximated` | 4 | 4 | Inline `<w:drawing>` images embedded inside non-empty text paragraphs are emitted as an `ImageBlock` immediately following the paragraph. |
| `docx.paragraph.unmapped-props` | `PreservedUnrendered` | 2,711 | 76 | Unmapped `<w:pPr>` child elements (e.g., widowControl, kinsoku, bidi, custom tabs/borders) are preserved as raw XML in `ParagraphBlock.extra["docx.pPr.raw"]` for DOCX roundtrip, plus `45` export-side passthrough notifications (`2,766` total). |
| `docx.paragraph.page-break-before` | `PreservedUnrendered` | 10 | 10 | `<w:pageBreakBefore/>` is stored in `extra["office.page_break_before"]` and re-emitted on DOCX/ODT export. |
| `odt.template.styles_only` | `Info` | 3 | 3 | Style-only ODF templates (`Default.ott`, `Modern.ott`, `Simple.ott`) containing `styles.xml` without `content.xml` are imported with an empty body and full stylesheet. |

> [!NOTE]
> **Remediated Bug (`docx.body.tcPr` — 1,764 False-Positive Losses Fixed):**
> In the initial bake-off run, `losses` was reported as `1,847` because `convert_table` in `engines/office/src/docx/import.rs` parsed `<w:tcPr>` (table cell properties: `gridSpan`, `vMerge`, `shd`) on each `<w:tc>` and then iterated over `tc.child_elements()` without filtering out `tcPr`, causing `convert_block_element` to log `docx.body.tcPr` (`Unsupported body element <w:tcPr> skipped`) `1,764` times across `58` documents. Filtering `!is_w(el, "tcPr")` in `convert_table` eliminated all `1,764` false-positive loss entries, leaving the `83` genuine format-feature losses listed above.

---

## 4. Security & Hostile Input Verification (`OFFICE-005`)

All hostile-input vectors in `engines/office/tests/office_interop_tests.rs` are deterministically rejected or neutralised:
- **Path Traversal (`../evil.xml`):** Rejected with `OfficeError::UnsafeArchivePath`.
- **XML DTD / XXE (`<!DOCTYPE ...>` / `<!ENTITY ...>`):** Rejected by custom entity-free scanner (`OfficeError::DtdForbidden`).
- **Encrypted OOXML (`EncryptionInfo` / `EncryptedPackage`) & ODF (`manifest:encryption-data`):** Rejected with `OfficeError::EncryptedDocument` / `OfficeError::EncryptedEntry`.
- **Macros (`word/vbaProject.bin`, `.docm`, `Basic/`, `Scripts/`):** Stripped and logged with `Severity::Security` in `ConversionReport`.
- **External Relationships (`TargetMode="External"`, `w:subDoc`, external `xlink:href` images, `javascript:`/`file:` URLs):** Stripped and logged with `Severity::Security`; Zero-Network-Access invariant preserved (`NET-001`).

---

## 5. Remaining Fidelity Limitations (`OFFICE-004`)

Every unsupported construct is surfaced in `ConversionReport` rather than silently dropped:
- First-page and even-page specific headers/footers (`docx.header.non-default`, `docx.footer.non-default`) are not yet modelled separately from the section default header/footer.
- Floating DrawingML text boxes (`docx.drawing.textbox`), SmartArt, charts, and WordArt frames are not laid out as positioned 2D drawing layers; their text content is flattened into the paragraph flow (`Severity::Approximated`).
- ODF variable and user-field declaration tables (`odt.body.user-field-decls`, `odt.body.variable-decls`) are not retained as live recalculating fields (`Severity::Loss`), though their rendered text values in paragraphs are preserved.
- Multi-column section layouts (`w:cols`, `style:columns`) are normalised to single-column `FlowDocument` sections.
