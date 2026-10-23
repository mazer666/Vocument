# Vocument Development & Quality Assurance Plan: Milestones M3 through M7

- **Document Version:** 1.0
- **Status:** Approved Baseline Plan (`CR-0005`, `ADR-0018`, `ADR-0019`, `ADR-0020`)
- **Authoring Team:** Multi-Agent Engineering & Governance Team per `AGENTS.md`
- **Owner Roles:** ROLE-01 (Product Steward), ROLE-02 (Chief Architect), ROLE-13 (QA Lead), ROLE-09 (Security), ROLE-04 (PDF), ROLE-20 (Project Manager)
- **Governance:** AGPL-3.0-or-later, strictly offline-first, no telemetry, no stubs (`QA-003`), `runs-on: self-hosted` CI.

---

## 1. Project Baseline Reconciliation & Current Status

### 1.1 M2 Technical Acceptance & Scope Realignment (`CR-0005`)
- **Delivered & Accepted in M2:**
  - `M2.1`: Secure Redaction with 4-way post-redaction verification (extraction, search, rendering, byte walk) and adversarial stream-dictionary descent.
  - `M2.2`: Link and action security inspection (`LINKSEC-001`).
  - `M2.3`: Attachment and digital signature inspection (`ATTACH-001`, `SIG-001`).
  - `M2.4`: Reviewer comments, `/Text` popups, and the atomic Jurist Tool Palette (`ANNOT-001`, `0600`/`0700` POSIX modes).
  - `SAN-001`: Selective document sanitization bridge and macOS review panels (`ReviewWorkflow.swift`, `ReviewPanelView.swift`).
- **Scope Realignment via `CR-0005`:**
  - *Document Comparison (`COMPARE-001`, `COMPARE-002`):* Formally rescheduled into a dedicated comparison track after M3/M4 visual & layout engines mature.
  - *Rubber Stamps (`WATER-003`):* Formally relocated to **M3.4** alongside page numbering, headers/footers, and watermarks.
  - *Kit Tests Demarcation:* `VocumentMacKitTests` prove client-core boundary contracts; interactive AppKit window-server GUI testing remains an explicitly demarcated on-display gate.

### 1.2 Boundaries of `SAN-001` Document Sanitization (`ADR-0018`)
- **Guaranteed Removals:** Document metadata (`/Info`, `/Metadata`, `/PieceInfo`), form fields (`/AcroForm`), executable actions (`/OpenAction`, `/AA`, `/JavaScript`), embedded files (`/EmbeddedFiles`), and annotations (`/Annots`), compacted via MuPDF garbage collection (`do_garbage = 2`).
- **Explicit Limitations:** Does NOT eliminate off-canvas graphics outside page boxes, text rendered in font color matching background, hidden OCG layers, or steganography in raster images.

### 1.3 M2.5 Release Readiness & Governance
- **Technical Readiness Passed:** Verified byte-for-byte reproducible source archive, 16 KB ELF page alignment, clean runtime dependency closure (`RISK-059` closed).
- **Open Publication Blockers:** Direct binary distribution requires owner legal review (`E5`), macOS signing/notarization credentials, and Android release keystore. Source-only alpha remains the active foundation.

---

## 2. Milestone M3: Page Workbench — Detailed Execution Plan

Milestone M3 delivers professional visual and structural page manipulation. All page operations must enforce document invariants per [ADR-0019](file:///Users/volker/Documents/Repos/Vocument/docs/adr/ADR-0019-page-workbench-architecture-and-structural-invariants.md).

### Sub-Milestone Breakdown

```text
M3.0 (Architecture & Invariants) ──► M3.1 (Visual Workbench & Core Ops)
                                             │
      ┌──────────────────────────────────────┴──────────────────────────────────────┐
      ▼                                                                             ▼
M3.2 (Merge, Insert & Split)                                         M3.3 (Geometry, Crop & Deskew)
      │                                                                             │
      └──────────────────────────────────────┬──────────────────────────────────────┘
                                             ▼
                                  M3.4 (Stamping, Numbers, Headers)
                                             │
                                             ▼
                                  M3.5 (Optimization, Export & Print)
                                             │
                                             ▼
                                  M3.6 (Platform Integration & Acceptance)
```

#### M3.0 — Architecture, Invariants & Structural Integrity
- **Objective:** Establish enforceable document-level invariant rules before page mutations execute.
- **Scope:**
  - Invariant rules for Digital Signatures (`SIG-001`), AcroForm fields, Bookmarks/Outlines, Named Destinations, Page Labels, Links, and Tagged PDF/UA structure trees ([ADR-0019](file:///Users/volker/Documents/Repos/Vocument/docs/adr/ADR-0019-page-workbench-architecture-and-structural-invariants.md)).
  - Digital signature tampering detection: Pre-flight check warning user that page modifications invalidate cryptographic signatures.
  - Unique AcroForm field name preservation on duplication (`Field_Copy1`).
  - Outline re-anchoring to preserve visual bookmark targets during reordering.
- **Criteria:** Unit tests in `vocument-core` proving zero orphaned object references or broken catalogs.

#### M3.1 — Visual Page Workbench & Core Manipulation (Single Document)
- **Objective:** Deliver smooth, thumbnail-driven visual page management within an active document session.
- **Scope:**
  - UI Grid: Asynchronous page thumbnail grid in macOS AppKit/SwiftUI and Android Compose.
  - Multi-Selection: Contiguous range (Shift-Click), non-contiguous (Cmd-Click), select all, invert selection.
  - Operations: Reorder (drag-and-drop & keyboard), Duplicate, Delete (with last-page protection), Extract.
  - Transaction Engine: Reversible Undo/Redo (`ReorderPages`, `DuplicatePage`, `DeletePages`, `RestorePages`), dirty state tracking, and recovery journaling.
- **Criteria:** 60 FPS scrolling on 500-page documents; 100% reversible Undo/Redo across FFI boundary.

#### M3.2 — Multi-Document Operations: Merge, Insert & Split
- **Objective:** Enable seamless assembly and division of PDF documents.
- **Scope:**
  - Inter-document page grafting (`pdf_graft_page`) across disparate `DocumentSession` instances.
  - Merge: Sequential appending of multiple PDFs, unifying page label catalogs and outlines.
  - Insert: Inserting page sequences from external documents at arbitrary 0-based indices.
  - Split: Splitting by page ranges, fixed page intervals ($N$), top-level bookmarks, or blank divider pages.
  - Conflict Resolution: Deduplicating shared font resources and resolving `/Names` dictionary collisions.
- **Criteria:** Merged and split PDFs pass `mutool info`, `qpdf --check`, and Quartz rendering without syntax errors.

#### M3.3 — Page Geometry, Crop, Page Boxes, Rotation & Deskew
- **Objective:** Provide millimeter-accurate page boundary calibration and orientation correction.
- **Scope:**
  - Page Box Management: Explicit inspection and modification of `MediaBox`, `CropBox`, `BleedBox`, `TrimBox`, and `ArtBox`.
  - Non-destructive display crop vs destructive hard crop (clipping path removal).
  - Rotation: Discrete 90°/180°/270° orientation updates, re-aligning underlying OCR text layers.
  - Deskew: Automated angle detection via `vocument-imaging` Hough transform and sub-degree rotation.
  - Auto-Crop: Automated margin trimming detecting scanning borders and whitespace.
- **Criteria:** Extracted text layer bounding boxes remain perfectly aligned with visual glyphs post-rotation/deskew.

#### M3.4 — Bates Stamping, Numbering, Headers/Footers, Bookmarks & Rubber Stamps
- **Objective:** Comprehensive legal document processing and metadata overlay.
- **Scope:**
  - Bates Numbering: Legal prefix, sequential numbering, zero-padding, suffix, and starting index.
  - Multi-Zone Headers & Footers: Left/Center/Right positioning with dynamic macros (`{page}`, `{total}`, `{date}`, `{filename}`).
  - Bookmark Editor: Interactive outline hierarchy creation, re-titling, indent/outdent, and viewport linking.
  - Stamps & Watermarks (`WATER-001`–`WATER-003`): Preset legal rubber stamps ("DRAFT", "CONFIDENTIAL", "APPROVED"), custom text/image stamps, opacity control, and foreground/background placement.
- **Criteria:** Overlays conform to PDF 1.7 `/Watermark` and `/Stamp` annotation specifications.

#### M3.5 — Image Extraction, Compression, Optimization & Export
- **Objective:** Professional optimization and asset extraction.
- **Scope:**
  - Lossless extraction of embedded raster images (JPEG, PNG, JBIG2 streams).
  - Compression: Flate stream re-compression, duplicate object coalescing, and metadata stripping.
  - Linearization: Fast Web View optimization for network streaming.
  - Print/Export: Color profile preservation (CMYK/RGB) and PDF/X compliance verification.
- **Criteria:** File size reduction $\ge 20\%$ on uncompressed scans without visible visual degradation.

#### M3.6 — Platform Integration, Interoperability & Final M3 Acceptance
- **Objective:** Complete end-to-end integration and third-party validation.
- **Scope:**
  - Native macOS AppKit/SwiftUI Workbench and Android Compose touch/stylus workbench.
  - Cross-tool interoperability matrix: MuPDF, Apple Quartz, PDFium, `mutool`, `qpdf`.
  - Usability benchmarking against reference tools (PDFCraft, Adobe Acrobat).
- **Criteria:** All M3 acceptance criteria met; zero stubs; acceptance record [M3-ACCEPTANCE.md](file:///Users/volker/Documents/Repos/Vocument/docs/acceptance/M3-ACCEPTANCE.md) signed off.

---

## 3. Milestone M4: Authoring / FlowDocument / Office — Detailed Execution Plan

Milestone M4 establishes Vocument's native authoring system per [ADR-0020](file:///Users/volker/Documents/Repos/Vocument/docs/adr/ADR-0020-flowdocument-architecture-deterministic-rust-layout-and-pdoc.md).

```text
M4.0 (FlowDocument Model) ──► M4.1 (Rust Layout Engine) ──► M4.2 (.pdoc Container)
                                                                   │
                                                                   ▼
M4.5 (Office Interop Bake-off) ◄── M4.4 (Reviews & PDF/UA) ◄── M4.3 (Native Editing UI)
```

### Sub-Milestone Breakdown

#### M4.0 — FlowDocument Data Model & Schema (`core/vocument-flow`)
- **Objective:** Pure Rust semantic document representation independent of fixed-layout PDF.
- **Scope:** Sections, paragraphs, spans, heading hierarchies (H1–H6), nested lists, tables (rowspan/colspan), embedded images, footnotes, and Dublin Core metadata.
- **Criteria:** Full serialization/deserialization to JSON; zero memory leaks; 100% test coverage of schema validation.

#### M4.1 — Deterministic Rust Layout Engine (`core/vocument-layout`)
- **Objective:** 100% cross-platform layout determinism across macOS, Android, Windows, and Linux.
- **Scope:**
  - Glyph shaping via `rustybuzz` (memory-safe HarfBuzz in Rust).
  - Unicode segmentation (graphemes, words, lines) and bi-directional text (UAX #9) via `icu_segmenter` and `unicode-bidi`.
  - Dual line-breaking: Fast greedy mode (typing) and Knuth-Plass optimal break mode (export/reading).
  - Deterministic multi-page pagination with footnote placement, orphan/widow controls, and table header repeating.
- **Criteria:** Given identical `.pdoc` and font files, pagination and coordinates match bit-for-bit across macOS and Android.

#### M4.2 — The `.pdoc` Format Specification & Container
- **Objective:** Robust, open, versioned package format.
- **Scope:**
  - Standard ZIP64 archive container (`manifest.json`, `document.json`, `styles.json`, `metadata.json`, `assets/`).
  - Schema versioning with unknown-field preservation for forward compatibility.
  - Safe extraction guarding against zip-slip and decompression bombs.
- **Criteria:** Roundtrip test suite verifying byte-for-byte asset preservation and seamless schema migration.

#### M4.3 — Native Platform Editing Canvas & Input
- **Objective:** Seamless, native typing and touch experience.
- **Scope:**
  - macOS: AppKit custom view conforming to `NSTextInputClient` for CJK IME, accent popups, dictation, and VoiceOver.
  - Android: Compose Foundation editor supporting touch selection, stylus handwriting, and Gboard IME.
  - Core Bridge: Bidirectional coordinate mapping between screen taps and `FlowDocument` text offsets.
- **Criteria:** Sub-16ms typing latency (60 FPS); flawless IME composition across languages.

#### M4.4 — Reviewing System (Comments, Tracked Changes) & PDF/A-PDF/UA Generation
- **Objective:** Enterprise legal reviewing and universally accessible document production.
- **Scope:**
  - Tracked Changes: Inline insertions, deletions, and format changes attributed to author UUID and timestamp.
  - Margin Comments: Character-range anchored threads with replies and resolution status.
  - PDF/A & PDF/UA Export: Generating Tagged PDF output with structure trees (`/StructTreeRoot`), semantic tags (`/P`, `/H1`-`/H6`, `/Table`), and alt-text.
- **Criteria:** Output validated green by PAC (PDF Accessibility Checker) and veraPDF.

#### M4.5 — Office Interoperability: DOCX and ODT Bake-Off & Adapters
- **Objective:** Reliable Office document import and export without deceptive claims.
- **Scope:**
  - Corpus Bake-Off: Curated test suite of 200 real-world DOCX and ODT legal documents.
  - Evaluation of Rust converters against strict fidelity criteria.
  - Import Pipeline: Mapping Word/Writer styles, tables, and lists into `FlowDocument`.
  - Export Pipeline: Serializing `FlowDocument` into valid DOCX/ODT.
- **Criteria:** Pass rate $\ge 95\%$ on structural corpus; explicit failure reporting on unsupported constructs.

---

## 4. Milestones M5, M6 & M7: Expansion & Local Interfaces

### Platform Sequence (`CR-0003`)
The platform sequence is strictly binding:
1. **macOS 15+** (Active)
2. **Android 13+** (Active)
3. **Windows 11+** (Planned — Entry Gate: macOS and Android Release Candidates green)
4. **Linux x86_64** (Planned — Entry Gate: Windows reaches Alpha)

### M5 — Windows Shell (`CR-0003`)
- **UI Architecture:** Modern WinUI 3 / Windows App SDK (C# WinRT or Rust `windows-rs`).
- **Core Abstractions:**
  - `PlatformFileBroker`: Windows Common Item Dialog (`IFileOpenDialog`, `IFileSaveDialog`).
  - `PlatformSettingsStore`: Windows Registry or AppData `%LOCALAPPDATA%\Vocument\settings.json`.
  - `PlatformAppLifecycle`: Windows AppLifecycle suspend/resume events.
- **Native Engine:** `x86_64-pc-windows-msvc` build of MuPDF 1.28.5 via `scripts/prepare-windows-mupdf.ps1`.
- **Gate:** Shared acceptance scenario: Open $\to$ Search $\to$ Highlight $\to$ Rotate $\to$ Undo $\to$ Redo $\to$ Save $\to$ Recover.

### M6 — Linux Shell (`CR-0003`)
- **UI Architecture:** GTK 4 + Libadwaita via Rust `gtk4-rs` bindings; Flatpak distribution.
- **Core Abstractions:**
  - `PlatformFileBroker`: XDG Desktop Portal FileChooser (`org.freedesktop.portal.FileChooser`).
  - `PlatformSettingsStore`: XDG Base Directory (`$XDG_CONFIG_HOME/vocument`).
  - `PlatformFontProvider`: Fontconfig & FreeType integration for M4 deterministic authoring.
- **Native Engine:** `x86_64-linux-gnu` build of MuPDF 1.28.5.
- **Gate:** Shared acceptance scenario running under Wayland and X11 display servers.

### M7 — Optional Local Interfaces: Model Context Protocol (MCP) (`CR-0004`)
- **Architecture:** Lightweight local MCP server over standard input/output (`stdio`) or UNIX domain socket.
- **Strict Principles:**
  - **Optional & Off by Default:** The application is 100% complete without it; zero automatic startup.
  - **Zero Network Rights (`E4`):** Strictly local-first; no listening network ports, no telemetry, no cloud dependencies.
  - **Single Shared Security Core:** The MCP server is merely an alternate boundary around `vocument-core`. All document mutations, sanitizations, and redactions pass through the exact same 4-way verification and transaction manager.
- **Tools Exposed:** `open_document`, `read_page_text`, `search_document`, `inspect_security_risks`, `apply_redaction`, `add_comment`, `save_document`.
- **Gate:** End-to-end MCP workflow passing the exact same compliance corpus as GUI applications.

---

## 5. Self-Hosted CI Architecture & Infrastructure Plan

In strict adherence to `RULE[user_global]`, all workflows in `.github/workflows/` remain configured with `runs-on: self-hosted`.

### 5.1 Runner Topologies & Labels

| Runner Pool | Host OS & Hardware | Runner Labels | Toolchains & Native Dependencies | Assigned Workflows |
|---|---|---|---|---|
| **Pool 1: macOS Apple Silicon** | macOS 15+ (Apple Silicon) | `self-hosted`, `macos`, `arm64` | Xcode, `swift`, Homebrew, Rust `aarch64-apple-darwin`, MuPDF 1.28.5 dylib | `ci-macos.yml` (Swift build, Kit tests, Handshake) |
| **Pool 2: Linux Host** | Ubuntu 24.04 LTS (x86_64) | `self-hosted`, `linux`, `x86_64` | Rust stable, GCC/Clang, Make, Tesseract 5.5.0, MuPDF 1.28.5 (source built), `cargo-deny`, `llvm-readelf` | `ci.yml` (Rust workspace, clippy, fmt, license audit, difficult corpus) |
| **Pool 3: Android Build & Emulator** | Linux KVM or macOS host | `self-hosted`, `android` | JDK 17, Android SDK (API 34/35), NDK r27c, Android Emulator QEMU KVM | `ci-android.yml` (Native cross-compilation, 16 KB page checks, Gradle APK, on-device OCR gate) |

### 5.2 Security Sandboxing & Ephemeral Execution
- Runners execute in isolated, disposable workspaces (`clean-workspace` step on start and exit).
- Zero ambient repository write tokens or publishing secrets in CI runners (`E5` barrier).
- Transparent three-state reporting:
  1. *Workflow Configured:* YAML workflow definition with `runs-on: self-hosted`.
  2. *Runner Available:* Registered self-hosted runner online and accepting jobs.
  3. *Run Verified:* Green execution log matched to commit SHA.

---

## 6. Unified Planning & Acceptance Matrix (M3–M7)

| Milestone | User Value | Core Requirements | Architecture & ADRs | Entry Criteria | Exit Acceptance Gate |
|---|---|---|---|---|---|
| **M3.0** | Document structural stability | `PAGE-001`–`005`, `SIG-001` | [ADR-0019](file:///Users/volker/Documents/Repos/Vocument/docs/adr/ADR-0019-page-workbench-architecture-and-structural-invariants.md) | M2 complete (`CR-0005`) | Unit tests proving signature invalidation warnings & AcroForm unique naming. |
| **M3.1** | Visual page reorder, duplicate, delete | `PAGE-001`, `UNDO-001` | [ADR-0019](file:///Users/volker/Documents/Repos/Vocument/docs/adr/ADR-0019-page-workbench-architecture-and-structural-invariants.md) | M3.0 green | 60 FPS thumbnail grid; 100% reversible Undo/Redo across FFI boundary. |
| **M3.2** | Multi-document merge, insert & split | `PAGE-002`, `PAGE-003` | [ADR-0019](file:///Users/volker/Documents/Repos/Vocument/docs/adr/ADR-0019-page-workbench-architecture-and-structural-invariants.md) | M3.1 green | Zero syntax defects in merged/split files via `mutool info` and `qpdf --check`. |
| **M3.3** | Precision crop, boxes, deskew | `PAGE-004`, `ROTATE-001` | [ADR-0019](file:///Users/volker/Documents/Repos/Vocument/docs/adr/ADR-0019-page-workbench-architecture-and-structural-invariants.md) | M3.1 green | Media/CropBox calibrated; text layer coordinates match visual glyphs post-crop. |
| **M3.4** | Bates stamping, numbers, rubber stamps | `WATER-001`–`003`, `HEADER-001` | `WATER-003` ([CR-0005](file:///Users/volker/Documents/Repos/Vocument/docs/cr/CR-0005-m2-scope-realignment-and-m3-m7-roadmap.md)) | M3.3 green | Bates sequence verification; PDF 1.7 `/Stamp` and `/Watermark` conformance. |
| **M3.5** | Lossless image extract & compression | `PERF-001`, `PDF-001` | [ADR-0019](file:///Users/volker/Documents/Repos/Vocument/docs/adr/ADR-0019-page-workbench-architecture-and-structural-invariants.md) | M3.2 green | $\ge 20\%$ file size reduction; lossless image extraction roundtrip. |
| **M3.6** | Complete M3 workbench release | All M3 requirements | M3 ADRs | M3.0–M3.5 green | macOS & Android UI integration; full foreign viewer interop suite green. |
| **M4.0–4.2** | Deterministic `.pdoc` authoring | `FLOW-001`, `PDOC-001` | [ADR-0020](file:///Users/volker/Documents/Repos/Vocument/docs/adr/ADR-0020-flowdocument-architecture-deterministic-rust-layout-and-pdoc.md) | M3 complete | Bit-identical pagination across macOS & Android; `.pdoc` ZIP64 validation. |
| **M4.3–4.4** | Native editing, tracking & PDF/UA | `LAYOUT-001`, `A11Y-001` | [ADR-0020](file:///Users/volker/Documents/Repos/Vocument/docs/adr/ADR-0020-flowdocument-architecture-deterministic-rust-layout-and-pdoc.md) | M4.2 green | Sub-16ms typing latency; PAC & veraPDF green for exported Tagged PDF/UA. |
| **M4.5** | Reliable DOCX & ODT interop | `OFFICE-001`, `OFFICE-002` | [ADR-0020](file:///Users/volker/Documents/Repos/Vocument/docs/adr/ADR-0020-flowdocument-architecture-deterministic-rust-layout-and-pdoc.md) | M4.4 green | $\ge 95\%$ pass rate on 200-document legal test corpus. |
| **M5** | Windows native shell | `PLATFORM-002` (`CR-0003`) | ADR-0017 | macOS & Android RCs green | Shared acceptance scenario verified on Windows 11. |
| **M6** | Linux native shell | `PLATFORM-002` (`CR-0003`) | ADR-0018 | Windows Alpha reached | Shared acceptance scenario verified on Wayland and X11. |
| **M7** | Optional local MCP interface | `LOCAL-001` (`CR-0004`) | `CR-0004` | Core review stable | Stdio MCP workflow passing identical compliance tests with 0 network calls. |

---

## 7. Responsibilities & Development Quality Standards

- **Coding Standard (`CODE-001`):** All new hand-written source files must target $\le 600$ lines, with an absolute hard ceiling of $900$ lines.
- **Prohibition of Stubs (`QA-003`, `CR-0002`):** Zero mocks, stubs, or synthetic bridges in accepted features. Every accepted capability requires authentic, native execution.
- **Offline Integrity (`NET-001`, `E4`):** Zero network calls, zero analytics, zero external cloud dependencies.
- **Documentation Freshness (`ROLE-17`):** Every code mutation must update relevant specifications, architecture diagrams, and `WHERE_TO_CHANGE.md` within the same pull request.
