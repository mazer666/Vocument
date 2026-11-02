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
> **External Corpus & Microsoft Word Status (`QA-003` Disclosure):**
> - The external 200-document proprietary customer/GovDocs corpus referenced in `ROADMAP.md` M4.5 is **not bundled in this repository** for licensing/privacy reasons (`180 / 200` evaluated locally via the reproducible LibreOffice + legal reference corpus).
> - **LibreOffice 26.x (`soffice --headless`):** **EXECUTED & VERIFIED** (`56 / 56` sampled exported `.docx` and `.odt` documents reopened and rendered to PDF via `soffice --headless --convert-to pdf` without repair or error).
> - **Microsoft Word (Desktop macOS/Windows):** **`NOT RUN`** (Microsoft Word is not installed on the build/test host; no synthetic or simulated Word acceptance is claimed).

---

## 2. Three-Tier Fidelity Results (180-Document Run)

| Metric Tier | Metric | Target (`M4.5` Gate) | Measured Result (`180` Docs) | Status |
|---|---|---|---|---|
| **Overall Gate** | Documents passing all Tier 1 & Tier 2 thresholds | $\ge 90\%$ | **180 / 180 (100.00%)** | **PASS** |
| **Tier 1 (Must Preserve)** | Plain text character similarity (normalised LCS/bigram) | $\ge 99.5\%$ | **100.00%** | **PASS** |
| **Tier 1 (Must Preserve)** | Heading hierarchy (`H1`–`H6` level + text) | $100\%$ | **100.00%** | **PASS** |
| **Tier 1 (Must Preserve)** | Character style runs (bold / italic / underline / strike / code) | $\ge 95\%$ | **100.00%** | **PASS** |
| **Tier 1 (Must Preserve)** | Footnote & endnote definitions and inline references | $100\%$ | **100.00%** | **PASS** |
| **Tier 2 (High-Value Layout)** | Table cell count, spans (`col_span`/`row_span`), and cell text | $\ge 95\%$ | **100.00%** | **PASS** |
| **Tier 2 (High-Value Layout)** | Nested list structure (`Ordered`/`Unordered`/`Task`) | $\ge 95\%$ | **100.00%** | **PASS** |
| **Tier 2 (High-Value Layout)** | Embedded image asset count & point dimensions (`±1 pt`) | $\ge 95\%$ | **100.00%** | **PASS** |
| **Tier 3 (Transparency)** | Unsupported / passthrough / approximated items logged in `ConversionReport` | $100\%$ logged | `passthrough=2766`, `approximations=57`, `losses=1847` | **PASS** |
| **External Validator** | LibreOffice headless reopen & PDF conversion (`soffice --headless --convert-to pdf`) | $100\%$ | **56 / 56 (100.00%)** | **PASS** |
| **External Validator** | Microsoft Word native reopen check | Manual / CI | **`NOT RUN`** | **NOT RUN** |

---

## 3. Security & Hostile Input Verification (`OFFICE-005`)

All hostile-input vectors in `engines/office/tests/office_interop_tests.rs` are deterministically rejected or neutralised:
- **Path Traversal (`../evil.xml`):** Rejected with `OfficeError::UnsafeArchivePath`.
- **XML DTD / XXE (`<!DOCTYPE ...>` / `<!ENTITY ...>`):** Rejected by custom entity-free scanner (`OfficeError::DtdForbidden`).
- **Encrypted OOXML (`EncryptionInfo` / `EncryptedPackage`) & ODF (`manifest:encryption-data`):** Rejected with `OfficeError::EncryptedDocument` / `OfficeError::EncryptedEntry`.
- **Macros (`word/vbaProject.bin`, `.docm`, `Basic/`, `Scripts/`):** Stripped and logged with `Severity::Security` in `ConversionReport`.
- **External Relationships (`TargetMode="External"`, `w:subDoc`, external `xlink:href` images, `javascript:`/`file:` URLs):** Stripped and logged with `Severity::Security`; Zero-Network-Access invariant preserved (`NET-001`).

---

## 4. Known Limitations & Documented Losses (`OFFICE-004`)

Every unsupported construct is surfaced in `ConversionReport` rather than silently dropped:
- Floating anchored shapes, SmartArt, charts, and WordArt frames in master styles or body paragraphs are not laid out as positioned drawing layers; their plain text is extracted or logged as `Loss` / `Approximated`.
- Complex fields other than `PAGE` and `NUMPAGES` (`TOC`, `SEQ`, `REF`, user fields) preserve their cached display text (`Severity::Approximated`) without live field recalculation.
- Multi-column section layouts (`w:cols`, `style:columns`) and text-box chains are normalised to single-column `FlowDocument` sections and logged in `ConversionReport`.

