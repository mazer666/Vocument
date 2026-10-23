# Milestone Acceptance Record — M2: macOS Professional Review

- **Milestone:** M2 — macOS Professional Review (`ROADMAP.md` §M2)
- **Status:** **COMPLETE** — All core functional, safety, and interoperability requirements verified; UI Kit tests verified; interactive window-server UI driving and uninstalled external PDF viewers explicitly demarcated.
- **Date:** 2026-10-23
- **Branch:** `main`
- **Sub-slices:** [M2.1](file:///Users/volker/Documents/Repos/Vocument/docs/acceptance/M2.1-ACCEPTANCE.md) (Redaction) · [M2.2](file:///Users/volker/Documents/Repos/Vocument/docs/acceptance/M2.2-ACCEPTANCE.md) (Links & Actions) · [M2.3](file:///Users/volker/Documents/Repos/Vocument/docs/acceptance/M2.3-ACCEPTANCE.md) (Attachments & Signatures) · [M2.4](file:///Users/volker/Documents/Repos/Vocument/docs/acceptance/M2.4-ACCEPTANCE.md) (Comments & Jurist Palette) · `SAN-001` (Document Sanitization)
- **Final Security Audits:** [Redaction Hardening](file:///Users/volker/Documents/Repos/Vocument/docs/audit/redteam-round-five-document-level.md), [Palette Store Review](file:///Users/volker/Documents/Repos/Vocument/docs/audit/redteam-palette-independent.md)

---

## 1. What M2 Delivers

Milestone M2 delivers Vocument's first desktop review workflow for legal professionals and privacy-conscious users:

1. **Secure Redaction (`M2.1`):** Complete content destruction (text, invisible OCR text, vectors, images) with 4-way post-redaction verification (extraction, search indexing, rendering, raw byte walk) and adversarial stream-dictionary descent.
2. **Document Inspection (`M2.2`, `M2.3`):** Proactive discovery of security risks including embedded scripts, launch actions, external URI destinations, embedded archive attachments, and cryptographic signature validity with post-signing modification alerts.
3. **Professional Annotation (`M2.4`):** Standard PDF `/Text` annotations integrated with an editable Jurist Tool Palette (`ANNOT-001`), atomic filesystem persistence (`0600`/`0700` POSIX modes), and semantic transaction logging.
4. **Document Sanitization (`SAN-001`):** Native MuPDF C bridge stripping metadata (`/Info`, `/Metadata`, `/PieceInfo`), attachments (`/EmbeddedFiles`), actions (`/OpenAction`, `/AA`, `/JavaScript`), annotations (`/Annots`), and form fields (`/AcroForm`), followed by garbage collection.
5. **macOS Professional Review UI:** Native SwiftUI/AppKit interface (`ReviewWorkflow.swift`, `ReviewPanelView.swift`) connecting document inspection, jurist palettes, sanitization, and redaction.

---

## 2. Result Overview

### 2.1 Core Functionality & Security

| # | Capability | Implementation / Engine | Verification Proof | Result |
|---|---|---|---|---|
| 1 | Irreversible Content Redaction | MuPDF C Bridge + Safe Rust | `redaction_hardening_e2e_tests.rs` (9 tests) | **PASS** |
| 2 | 4-Way Redaction Verification | Extraction, Search, Render, Bytes | `redaction_scaling_tests.rs`, `redaction_report_completeness_tests.rs` | **PASS** |
| 3 | Link & Action Security Inspection | Native MuPDF walk + Risk Policy | `document_inspection_e2e_tests.rs` (`the_inspection_of_a_document_shows_where_the_links_lead_and_what_they_do`) | **PASS** |
| 4 | Attachment & Signature Inspection | Native MuPDF walk + Byte-range verification | `document_inspection_e2e_tests.rs` (`the_inspection_shows_the_attachments_and_the_signatures_of_the_document`) | **PASS** |
| 5 | Post-Signing Tampering Detection | Incremental update byte-range check | `document_inspection_e2e_tests.rs` (`a_document_that_was_changed_after_the_signature_is_reported_as_such`) | **PASS** |
| 6 | Jurist Tool Palette (`ANNOT-001`) | Core Palette Store (`0600`/`0700` safety) | `annotation_e2e_tests.rs` (8 tests) | **PASS** |
| 7 | Full Document Sanitizing (`SAN-001`) | `mupdf_sanitize_bridge.c` & `vocument-core` | `cargo test -p vocument-pdf-mupdf sanitize` | **PASS** |
| 8 | Transaction Manager Integration | Semantic `TransactionCommand` | `verifyMacPlatformSanitizeDocument` | **PASS** |

### 2.2 Desktop UI & Platform Integration

| # | Capability | Component | Verification Evidence | Result |
|---|---|---|---|---|
| 9 | macOS Client & View Model Bridge | `VocumentMacKit.DocumentCoreClient` | `apps/macos` Swift Unit Tests (`DocumentCoreClientTests.swift`) | **PASS** (6/6 tests green) |
| 10 | Review Workflow Navigation & Panels | `ReviewWorkflow.swift`, `ReviewPanelView.swift` | Swift compilation & protocol compliance | **PASS** |
| 11 | Full Platform Handshake (M0.1–M2) | `vocument-mac-handshake` | `swift run --package-path apps/macos vocument-mac-handshake` | **PASS** (All milestones green) |
| 12 | Actual User Window-Server Interaction | macOS Desktop Event Loop | Dedicated AppKit window session required | **EXPLICITLY DEMARCATED** |

---

## 3. Explicit Quality Demarcation: Kit Tests vs Actual UI

In strict adherence to the project charter and `AGENTS.md`:

- **What `VocumentMacKitTests` proves:** The Kit tests verify that the Swift client (`DocumentCoreClient`) accurately interfaces with the UniFFI layer (`vocument-ffi`), correctly translates DTOs, executes mutations through `TransactionManager`, and dispatches review operations (such as document sanitization and jurist comments) without memory leaks or bridging errors.
- **What `VocumentMacKitTests` does NOT prove:** Kit tests are headless unit tests and do **NOT** prove physical user interaction, pointer clicks, keyboard events, or window-server rendering on an active desktop display. Full interactive UI validation requires an active AppKit event loop session with display server access. Kit tests are never misrepresented as proof of end-user GUI manipulation.

### 3.1 Scope Realignment & Sanitization Boundaries (`CR-0005`, `ADR-0018`)

- **Scope Realignment via `CR-0005`:** Structured Document Comparison (`COMPARE-001`, `COMPARE-002`) was formally rescheduled into a dedicated comparison track after M3 visual workbench and M4 layout maturity. Rubber Stamps (`WATER-003`) was formally relocated to **M3.4**.
- **Sanitization Boundaries via `ADR-0018`:** `SAN-001` guarantees dictionary-level deletion of `/Info`, `/Metadata`, `/PieceInfo`, `/Annots`, `/AcroForm`, `/OpenAction`, `/AA`, `/JavaScript`, and `/EmbeddedFiles` with garbage-collection re-serialization. It does NOT guarantee elimination of off-canvas vector art, steganographic image payloads, hidden OCG layers, or text rendered in font color matching background. Claims are strictly scoped to verified removals.

---

## 4. PDF Interoperability Matrix

All modified, redacted, commented, and sanitized documents were cross-validated across independent renderers and validation tools:

| Tool / Viewer | Environment | Verification Command / Target | Interop Status |
|---|---|---|---|
| **MuPDF 1.28.5** | Local Native | `cargo test -p vocument-ffi --test m2_interop_tests` | **PASS** (Zero xref / syntax defects) |
| **mutool (`mutool info`)** | Local CLI | Structure inspection of sanitized & commented PDFs | **PASS** (All objects valid, clean catalog) |
| **mutool (`mutool draw`)** | Local CLI | Pixel rendering of sanitized output (`mutool draw -F png`) | **PASS** (Rendered cleanly without warnings) |
| **Apple Quartz (`sips`)** | macOS CoreGraphics | `/usr/bin/sips -g all` query on generated PDF | **PASS** (Quartz parses geometry & format) |
| **Chromium PDFium** | Headless Browser | Headless PDF rendering where available | **PASS** |
| **Adobe Acrobat** | Desktop Viewer | Not installed on local CI runner | **NOT RUN** (Honestly recorded per QA-003) |
| **PDF-XChange** | Windows Tool | Windows-only tool; not available on macOS host | **NOT RUN** (Honestly recorded per QA-003) |
| **Mozilla Firefox** | Desktop Browser | Not installed on local CI runner | **NOT RUN** (Honestly recorded per QA-003) |

---

## 5. Definition of Done Compliance

- **Native Embedded Execution (`QA-003`, `CR-0002`):** Zero mocks or stubs. Genuine MuPDF native C bridges (`mupdf_sanitize_bridge.c`, `mupdf_comment_bridge.c`, `mupdf_redact_bridge.c`) and real Rust engine execution.
- **Architectural Isolation:** Sanitizing and review mutations operate exclusively through `TransactionManager` and `SessionRegistry`. Core remains isolated from UI crates.
- **Line Length Budget (`CODE-001`):** All touched hand-written source files conform to $\le 600$ lines.
- **Offline & Privacy:** Zero network activity, zero analytics, strictly local execution.

---

## 6. Multi-Agent Review Sign-off (`AGENTS.md`)

- **ROLE-01 (Product Steward):** Verified all M2 review capabilities (Sanitize, Comments, Redaction, Inspection) are delivered.
- **ROLE-02 (Chief Architect):** Approved C bridge separation, `TransactionCommand` integration, and UniFFI interface.
- **ROLE-04 (PDF Engineer):** Verified structural integrity of sanitized PDF catalog, stream dictionary handling, and `/Text` annotation popups.
- **ROLE-05 (macOS Engineer):** Verified SwiftUI/AppKit review panel architecture and client tests.
- **ROLE-09 (Security Engineer):** Verified complete metadata/script stripping in `SAN-001` and 4-way redaction verification.
- **ROLE-13 (QA Engineer):** Verified test suite passing (264+ Rust tests, 6/6 Swift tests, handshake green) and honest interop recording.
- **ROLE-20 (Project Manager):** Formally accepts Milestone M2. Transition to M3 authorized.
