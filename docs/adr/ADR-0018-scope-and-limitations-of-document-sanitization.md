# ADR-0018: Scope, Boundaries, and Limitations of Document Sanitization (`SAN-001`)

- **Status:** Accepted
- **Date:** 2026-10-23
- **Author:** Chief Architect, Security Engineer, PDF/MuPDF Engineer
- **Deciders:** ROLE-01 (Product Steward), ROLE-02 (Chief Architect), ROLE-04 (PDF Engineer), ROLE-09 (Security Engineer), ROLE-13 (QA Lead)
- **Requirement IDs:** `SAN-001`, `REDACT-001`, `REDACT-002`, `LINKSEC-001`, `ATTACH-001`, `CODE-001`, `NET-001`

---

## Context & Problem Statement

Requirement `SAN-001` specifies document sanitization for scrubbing metadata, hidden content, attachments, actions, comments, and form remnants prior to document distribution. However, in security-critical legal and enterprise domains, calling any procedure "full sanitization" without mathematically rigorous boundaries risks creating a false sense of security.

PDF is a complex, rich, and historically accretive file format. Content can be hidden through numerous mechanisms beyond explicit metadata dictionaries (e.g., hidden layers, off-screen clipping, text color matching background, steganography in raster images, or malformed unreferenced streams).

This ADR establishes the exact technical scope, guarantees, and explicit non-guarantees of Vocument's sanitization engine so that the product, documentation, and user interface accurately represent its capabilities without overclaiming.

---

## Decision Drivers

1. **Security Truth-in-Advertising (`QA-003`, `CR-0005`):** Never promise more than what the software actually verifies. Explicitly declare what is scrubbed and what is out of scope.
2. **Native Embedded Execution:** Execute scrubbing directly inside MuPDF C bridge (`mupdf_sanitize_bridge.c`) and Rust core without third-party cloud services or external black-box utilities.
3. **Preservation of Document Readability:** Sanitization must remove unwanted data without altering the readable visual page layout or corrupting legitimate text streams.
4. **Transaction & Recovery Safety:** Sanitization is an irreversible destructive mutation; it must be executed through `TransactionManager` and must clear recovery checkpoints containing pre-sanitized bytes.
5. **Cross-Viewer Interoperability:** The sanitized output must remain 100% compliant with PDF 1.7 / ISO 32000-1 and open cleanly across MuPDF, Apple Quartz, PDFium, and Adobe Acrobat.

---

## Architectural Decisions

### 1. What `SAN-001` Guarantees (Implemented Native Capabilities)

When invoked with full options (`PdfSanitizeOptions::default()`), Vocument's sanitization engine (`vocument_mupdf_sanitize_document` in `mupdf_sanitize_bridge.c`) executes the following operations:

1. **Document-Level Metadata:**
   - Deletes `/Info` dictionary in the PDF trailer (`/Title`, `/Author`, `/Subject`, `/Keywords`, `/Creator`, `/Producer`, `/CreationDate`, `/ModDate`, and any custom keys).
   - Deletes the XMP XML metadata stream (`/Metadata` in document catalog and on pages).
   - Deletes private application metadata dictionaries (`/PieceInfo` in catalog and page dictionaries).
2. **Interactive Form Fields & Data:**
   - Deletes `/AcroForm` from the document catalog, eliminating form field structures, default appearances, and calculation scripts.
3. **Document & Page Actions (Executable Scripts):**
   - Deletes `/OpenAction` and Additional Actions (`/AA`) from the document catalog.
   - Deletes page-level `/AA` dictionaries across all pages.
   - Deletes `/JavaScript` name tree entries from `/Names`.
4. **Embedded Files & Attachments:**
   - Deletes `/EmbeddedFiles` name tree from `/Names` and `/EmbeddedFiles` from the catalog dictionary.
5. **Annotations & Review Notes:**
   - When `remove_annotations` is active, drops all `/Annots` arrays from all page objects. This removes all visual markups, highlights, notes, popups, and link annotations.
   - *Technical Limitation:* If `remove_annotations` is false and `remove_actions` is true, action dictionaries (`/A`) inside individual page annotations (e.g. URI links) are NOT stripped by the current bridge. Fine-grained annotation action scrubbing while preserving non-action markup is planned for M5.
6. **Unreferenced Object Garbage Collection:**
   - Serializes the document using MuPDF's unreferenced object collection pass (`opts.do_garbage = 1` in `mupdf_serialize.h`). This ensures that unreferenced objects orphaned by the dictionary deletions are physically excluded from the written PDF byte stream.

### 2. Planned Hardening Enhancements (Scheduled for M5)

The following refinements are recognized as valuable security improvements and are formally scheduled for M5:
- **Full Xref Compaction (`opts.do_garbage = 2`):** Defragmenting and renumbering the entire cross-reference table.
- **Deep Annotation Action Scrubbing:** Parsing individual annotation dictionaries to strip `/A` actions (URI, Launch, GoToR) while preserving non-action annotations.
- **Stream Content Token Scanning:** Detecting and scrubbing unreferenced or hidden inline image resources.

### 3. What `SAN-001` Does NOT Guarantee (Explicit Boundaries & Limitations)

The following attack vectors and hidden data forms are explicitly outside the scope of `SAN-001`:

1. **Visual / Textual Steganography:**
   - Does NOT detect text rendered in colors matching the background (e.g., white text on white background).
   - Does NOT detect micro-fonts ($< 1\,\text{pt}$) or text rendered behind opaque raster images.
   - Does NOT detect steganographic payloads or watermarks encoded within DCT/Flate raster image pixels.
2. **Off-Canvas Content:**
   - Does NOT clip or delete vector artwork or text positioned outside the `MediaBox` or `CropBox`. (Handled separately via M3.3 Page Box cropping).
3. **Optional Content Groups (OCG / Layers):**
   - Does NOT automatically flatten or delete invisible or turned-off PDF layers.
4. **Content Stream Obfuscation:**
   - Does NOT normalize complex Type 3 font glyph programs or decipher proprietary encrypted streams.

### 4. User Interface Demarcation & Transparency

The UI must present `SAN-001` as **"Document Sanitization: Metadata, Attachments, Scripts & Annotations Scrubbing"**, rather than "Complete Hidden Data Elimination".
The pre-save dialog must provide granular check-boxes for:
- [x] Clear document metadata and author history
- [x] Remove embedded attachments and external scripts
- [x] Strip interactive form data and document actions
- [x] Remove review comments and annotations

---

## Consequences

### Positive
- Transparent, audit-proof security claims backed by unit and interop tests (`sanitize_tests.rs`, `m2_interop_tests.rs`).
- Guaranteed removal of common accidental data leaks (author name, file paths, hidden draft comments).
- High execution speed through in-memory MuPDF catalog traversal.
- Zero overclaimed capabilities, fully complying with `AGENTS.md` and `CR-0002`.

### Negative / Trade-offs
- Users expecting optical/visual hidden-text elimination must use explicit visual redaction (`M2.1`) rather than relying solely on `SAN-001`.
