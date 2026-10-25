# Vocument Page Workbench — User Manual

**Document Version:** 1.0.0  
**Compliance Standards:** `HELP-001`, `HELP-002`, `UI-010`, `ACC-001`, `PRIV-001`  
**Target Platforms:** macOS (AppKit/SwiftUI), Android (Jetpack Compose), Native CLI

---

## 1. Overview & Core Philosophy

The Vocument **Page Workbench** provides a high-performance, strictly non-destructive workspace for organizing, numbering, stamping, extracting, inspecting, and optimizing PDF documents.

### Key Tenets
1. **Zero Cloud & Absolute Privacy (`PRIV-001`):** All operations run locally on device. No telemetry, no remote processing, and no cloud fallbacks.
2. **Strict Non-Destructive Operation (`PAGE-001`):** Structural modifications preserve existing interactive form fields, metadata, and appearances (`do_clean = 0`, `do_sanitize = 0`).
3. **Digital Signature Protection:** Operations that would break or invalidate digital signatures are blocked by default and require explicit user opt-in (`allow_signature_invalidation: true`).
4. **Integrität & Crash Recovery (`UNDO-001`, `UNDO-004`):** Jede Mutation wird in einer deterministischen SHA-256-Prüfsummenkette im Transaktionsjournal erfasst. Dies sichert die Wiederherstellung nach Abstürzen und schützt gegen versehentliche Dateikorruption und Bitrot (ehrliches Bedrohungsmodell: kein HMAC gegen aktive Angreifer mit Schreibzugriff; Vertraulichkeit über OS-Sandbox).
5. **No Synthetic Shortcuts (`QA-003`, `CR-0002`):** All features use genuine native/embedded implementations via MuPDF and Apple Quartz / Android graphics pipelines.

---

## 2. Page Organization Workflows

The Page Workbench displays an interactive visual thumbnail rail alongside document page dimensions, orientation, and box boundaries.

### 2.1 Reordering Pages
- **Pointer/Mouse:** Drag and drop page thumbnails within the rail to reorder.
- **Keyboard (macOS):** Select a thumbnail and press `⌥↑` (Option + Up) or `⌥↓` (Option + Down) to move the selected page earlier or later.
- **Invariants:** The core enforces full permutation validation. You cannot drop or duplicate page indices during reordering.

### 2.2 Duplicating Pages
- **Action:** Select target page(s) and choose **Page > Duplicate Page** (`⌘D`).
- **Behavior:** The selected page is cloned immediately following the original, maintaining all visual elements, fonts, and dimensions.

### 2.3 Deleting Pages & Last-Page Protection
- **Action:** Select page(s) and press `⌫` (Delete) or choose **Page > Delete Pages...**.
- **Protection:** The system enforces **Last Page Protection**. A document must always retain at least one page. Attempting to delete all pages will be rejected fail-closed with a descriptive error.

### 2.4 Rotating Pages
- **Action:** Rotate selected pages clockwise (`⌘R`) or counter-clockwise (`⇧⌘R`) by 90°, or invert by 180°.
- **Behavior:** Rotation updates both the page display matrix and bounding box coordinates, automatically adjusting layout geometry.

### 2.5 Inserting External Pages
- **Action:** Choose **Page > Insert Pages from PDF...** (`⌘I`).
- **Behavior:** Select a source PDF file. Pages from the source document are imported at the selected index without altering source or destination document profiles.

### 2.6 Extracting Pages
- **Action:** Select one or more pages and choose **Page > Extract Pages to New PDF...** (`⇧⌘E`).
- **Behavior:** Generates a new standalone PDF file containing only the selected pages. The source document in the active session remains unmodified.

---

## 3. Bates Numbering & Document Tracking (`BATES-001`)

For legal discovery, compliance, and archival tracking, the Page Workbench provides high-precision Bates numbering across arbitrary page selections.

### Configuration Parameters
- **Prefix & Suffix:** Alphanumeric identifiers (e.g., `VOC-` and `-CONF`).
- **Sequence Numbering:** Configurable starting number (e.g., `100`) and digit padding (e.g., 6 digits: `000100`).
- **Positioning:** Six standardized placement anchors:
  - Top Left, Top Center, Top Right
  - Bottom Left, Bottom Center, Bottom Right
- **Typography & Margins:** Configurable font size (points) and page margin offsets to ensure numbers do not clip or collide with document content.

---

## 4. Multi-Zone Headers & Footers (`HF-001`)

Add dynamic running headers and footers across page ranges with dynamic token evaluation.

### Dynamic Macro Tokens
| Token | Replacement Value |
|---|---|
| `[PAGE]` | Current 1-based page number within the selection |
| `[TOTAL]` | Total page count of the target range |
| `[DATE]` | Document date or custom timestamp (ISO 8601 or user-defined) |
| `[TITLE]` | Document title or custom subject identifier |
| `[BATES]` | Linked Bates reference if active on target page |
| `[CONFIDENTIAL]` | Standardized protective legal notice |

---

## 5. Stamps & Watermarks (`STAMP-001`)

Apply text-based approval markers, security classifications, or draft notices.

### Options
- **Text & Color:** Arbitrary unicode text with 24-bit RGB color selection.
- **Opacity / Transparency:** Alpha channel control from 10% (subtle background watermark) to 100% (opaque stamp).
- **Rotation Angle:** Any angular rotation (standard preset: `-45°` diagonal).
- **Layering:** Choose foreground overlay or background underlay (behind text).
- **Bounding Box Placement:** Optional normalized rect coordinates (`[x0, y0, x1, y1]`) or automatic center placement.

---

## 6. Lossless Image Extraction (`IMG-001`)

Extract embedded images directly from document streams without quality degradation.

### Stream Extraction Modes
1. **Raw Lossless Stream Extraction (JPEG / JPEG2000):**
   - For images embedded with `/Filter /DCTDecode` (JPEG) or `/Filter /JPXDecode` (JPEG 2000), Vocument extracts the raw byte stream directly from the PDF dictionary.
   - Preserves original compression, quantization tables, resolution, and embedded ICC color profiles without re-encoding artifacts.
   - Exported with native extensions (`.jpg`, `.jp2`).
2. **Lossless Pixmap Rendering (PNG):**
   - For uncompressed bitmap streams or FlateDecode/JBIG2 masks, Vocument extracts the uncompressed pixmap and encodes it as lossless PNG with alpha channel support.

---

## 7. Restricted PDF/X Structural Preflight (`PDFX-001`)

Vocument provides an automated structural pre-check to assess document print-readiness.

> [!WARNING]
> **Eingeschränkte Vorprüfung (Restricted Structural Preflight):**  
> This automated inspection checks essential PDF/X technical criteria (PDF/X dictionary keys, page boundary boxes, font embedding, and ExtGState transparency). **It does not constitute official ISO 15930 conformity certification.** Official certification requires validation by accredited graphic arts preflight systems.

### Verification Matrix
- **PDF/X Dictionary Entries:** Audits `/GTS_PDFXVersion` in the document catalog and Info dictionary.
- **Output Intents:** Verifies presence of standard `/OutputIntents` dictionary and ICC output condition profile.
- **Page Geometry Boxes:** Validates that every page specifies both `/MediaBox` and `/TrimBox` or `/BleedBox`.
- **Font Embedding Audit:** Inspects `/FontDescriptor` across all page resource dictionaries to verify that every font program is embedded (`/FontFile`, `/FontFile2`, or `/FontFile3`). Lists any unembedded fonts explicitly.
- **Transparency Detection:** Inspects extended graphic states (`/ExtGState`) for non-normal blend modes (`/BM`), soft masks (`/SMask`), or reduced alpha (`/ca < 1`).

---

## 8. Document Compaction & Optimization (`OPT-001`)

Reduce PDF file size safely while guaranteeing document integrity and standard compliance.

### Optimization Presets
1. **Lossless:** Compresses uncompressed streams, cleans unused cross-reference entries, and packs objects into ISO 32000-1 Object Streams (`/ObjStm`).
2. **Balanced (Recommended):** Combines object streams, xref streams, and garbage collection of dead objects.
3. **Maximum:** Aggressive object deduplication and full cross-reference stream compaction.

### Non-Destructive Integrity Guarantee
- In accordance with `CODE-001` and user safety policies, Vocument optimization presets strictly enforce non-destructive write options:
  - `opts.do_clean = 0` (no altering of content streams)
  - `opts.do_sanitize = 0` (no stripping of metadata or custom dictionaries)
  - `opts.do_preserve_metadata = 1`
  - `opts.do_appearance = 1`
- Content sanitization is isolated strictly to explicit redaction workflows (`SAN-001`).

### Note on Linearization (Fast Web View)
- In modern MuPDF (1.25+), legacy PDF linearization has been permanently deprecated and removed. Modern web delivery relies on HTTP byte-range requests (RFC 7233) against standard ISO xref streams and object streams (`/ObjStm`). Vocument does not make false linearization claims.

---

## 9. Keyboard Shortcuts & Accessibility (Tastaturkürzel)

Vocument provides full keyboard navigation and VoiceOver / TalkBack compatibility.

### macOS Keyboard Shortcuts (Tastaturkürzel)
| Command | Shortcut | Description |
|---|---|---|
| Open Offline Help | `⌘?` | Opens the bundled Offline Manual (`HELP-002`) |
| Duplicate Page | `⌘D` | Clones selected page |
| Delete Pages | `⌫` (Delete) | Removes selected page (subject to last-page guard) |
| Rotate Clockwise | `⌘R` | Rotates selected page 90° clockwise |
| Rotate Counter-Clockwise | `⇧⌘R` | Rotates selected page 90° counter-clockwise |
| Move Page Earlier | `⌥↑` | Shifts page up in sequence |
| Move Page Later | `⌥↓` | Shifts page down in sequence |
| Insert Pages | `⌘I` | Opens insert document dialog |
| Extract Pages | `⇧⌘E` | Extracts selection to new PDF |
| Undo | `⌘Z` | Reverts last transaction |
| Redo | `⇧⌘Z` | Re-applies undone transaction |
| Close Sheet / Cancel | `⎋` (Esc) | Closes active inspector or sheet |

---

## 10. Crash Resilience & Bedrohungsmodell (Threat Model)

Every operation conducted in the Page Workbench is managed by the `TransactionManager`:
- **Deterministische SHA-256-Prüfsummen:** Every journal entry records the unkeyed SHA-256 digest of its forward command, inverse command, and resultant document state.
- **Integrität & Recovery:** Protects against accidental corruption, crash interruption, and bitrot during recovery replay.
- **Ehrliches Bedrohungsmodell:** Unkeyed SHA-256 is NOT an HMAC and does not protect against an active attacker who has write permissions on the local filesystem. Confidentiality and access control are guaranteed by operating system sandbox perimeters (macOS App Sandbox, Android SAF / private internal storage) and disk encryption (FileVault / LUKS / Android FBE).
- **Keine unnötigen Klartextkopien:** Persistent transaction journals purge duplicate raw PDF byte snapshots from inverse commands prior to serialization, preventing unneeded plaintext retention on disk.
