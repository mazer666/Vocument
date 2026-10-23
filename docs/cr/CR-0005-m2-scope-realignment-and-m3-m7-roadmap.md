# CR-0005 — Scope Realignment for M2 Closure, Sanitization Demarcation, and M3–M7 Roadmap Harmonization

- **Status:** accepted
- **Date:** 2026-10-23
- **Proposer roles:** ROLE-01 (Product & Requirements Steward), ROLE-02 (Chief Architect), ROLE-20 (Project / Release Manager)
- **Reviewer roles:** ROLE-04 (PDF Engineer), ROLE-09 (Security Engineer), ROLE-13 (QA & Test Engineer), ROLE-15 (Interoperability & Standards Engineer)

---

## 1. Problem Statement

A thorough audit of `REQUIREMENTS.md`, `ROADMAP.md`, `ARCHITECTURE.md`, `project/project.yaml`, and existing acceptance records revealed several scope, architectural, and governance discrepancies that require formal reconciliation before proceeding with Milestone M3:

1. **M2 Scope Discrepancy (Document Comparison & Stamps):**
   - The initial summary bullet points in `ROADMAP.md` §M2 ("macOS Professional Review") listed both "structured document comparison" (`COMPARE-001`, `COMPARE-002`) and "stamps" (`WATER-003`).
   - However, when M2 was broken down into execution slices, the work strictly concentrated on security, legal review, and annotation integrity: M2.1 (Redaction), M2.2 (Links & Actions), M2.3 (Attachments & Signatures), M2.4 (Reviewer Comments & Jurist Palette), and Document Sanitization (`SAN-001`).
   - Neither document comparison nor custom rubber stamps were implemented or verified in M2. Under `AGENTS.md` §1 (Rule 8: "never silently weaken an acceptance criterion") and `ROADMAP.md` ("A coding agent must not silently move an unfinished item into a later milestone just to make a milestone 'green'"), these items cannot be quietly dropped; they require explicit, formal realignment.

2. **Scope and Technical Boundaries of `SAN-001` (Document Sanitization):**
   - In `vocument-pdf-mupdf` and `vocument-ffi`, `SAN-001` implements dictionary-level removal of `/Info`, `/Metadata`, `/PieceInfo`, `/Annots`, `/AcroForm`, `/OpenAction`, `/AA`, `/JavaScript`, and `/EmbeddedFiles`, followed by garbage-collection serialization.
   - While effective for standard scrubbing, labelling this as "complete sanitization" without qualification creates an unacceptable security liability: it does not eliminate off-canvas content, steganography, invisible white-on-white text, OCG hidden layers, or clipped vector paths. The boundaries and limitations of `SAN-001` must be formally defined and documented.

3. **Demarcation of Kit Tests vs. Interactive UI:**
   - Headless unit/integration tests (`VocumentMacKitTests`) verify the Swift-to-Rust UniFFI bridging, transaction execution, and state propagation. They do not constitute verification of end-user AppKit window-server interaction or physical pointer/keyboard manipulation. This distinction must be formalized in the project status.

4. **CI Runner Architecture Alignment:**
   - In `project/project.yaml`, stale notes described workflows as "running-on-the-hosted-runners", contradicting the binding governance rule (`RULE[user_global]`) requiring `runs-on: self-hosted`.

---

## 2. Requested Changes

### 2.1 Formal Realignment of M2 Scope
- **Milestone M2 Definition:** Formally declare Milestone M2 as **macOS Professional Review — Security, Inspection & Jurist Annotations**. The delivered scope covers:
  - M2.1: Secure Redaction with 4-way post-redaction verification.
  - M2.2: Link and action security inspection (`LINKSEC-001`).
  - M2.3: Attachment and cryptographic signature inspection (`ATTACH-001`, `SIG-001`).
  - M2.4: Reviewer comments, `/Text` annotation popups, and the atomic Jurist Tool Palette (`ANNOT-001`).
  - `SAN-001`: Selective document sanitization.
  - Desktop Review UI: macOS workflow panels connecting inspection, palette, redaction, and sanitization.
- **Relocation of Document Comparison (`COMPARE-001`, `COMPARE-002`):**
  - Document comparison is formally rescheduled as a dedicated feature track (`Track: Document Comparison & Diffing`), positioned after the core visual page manipulation of M3 and authoring layout of M4.
- **Relocation of Watermarks and Stamps (`WATER-001`–`WATER-003`):**
  - Watermarks and rubber stamps are formally scheduled into **M3.4** (Page Workbench: Numbering, Headers/Footers, Bookmarks & Stamps), where page-geometry and overlay rendering infrastructure naturally reside.

### 2.2 Formal Definition of `SAN-001` Scope and Limitations
- The documentation of `SAN-001` is updated in `ARCHITECTURE.md`, `REQUIREMENTS.md`, and [ADR-0018](file:///Users/volker/Documents/Repos/Vocument/docs/adr/ADR-0018-scope-and-limitations-of-document-sanitization.md) to explicitly state:
  - **What `SAN-001` guarantees:** Removal of document-level metadata dictionaries (`/Info`), XMP metadata streams (`/Metadata`), document piece info (`/PieceInfo`), page annotation arrays (`/Annots`), interactive form dictionaries (`/AcroForm`), document/page actions (`/OpenAction`, `/AA`), embedded JavaScript, and name tree embedded file entries (`/EmbeddedFiles`), followed by xref compaction and garbage collection.
  - **What `SAN-001` does NOT guarantee:** Removal of text rendered in colors matching background, vector content clipped outside page boundaries, hidden layers (Optional Content Groups), steganographic image payloads, or content encoded within unreferenced orphaned objects in damaged PDFs.

### 2.3 Clarification of Platform Testing Status
- Machine-readable project status (`project/project.yaml`) and roadmap documentation must strictly distinguish between:
  - **Tested on Host / Emulator:** Headless macOS Swift tests, `vocument-mac-handshake`, Android QEMU emulator on-device OCR and UI scan capture.
  - **NOT RUN:** Physical Android hardware testing (thermal/battery profiling), interactive AppKit window-server GUI driving, and uninstalled external commercial PDF viewers (Adobe Acrobat, PDF-XChange).

### 2.4 Governance & CI Consistency
- Update `project/project.yaml` to confirm that all workflows adhere to `runs-on: self-hosted` per `RULE[user_global]`.

---

## 3. Impact Assessment

- **Product Integrity:** Prevents scope debt and protects the team from making unverified security or UI claims.
- **Architecture:** Provides a clean separation of concerns: M2 delivers review and inspection; M3 delivers page manipulation and stamping; M4 delivers FlowDocument authoring.
- **Acceptance Criteria:** Aligns all acceptance records with reality, leaving zero undocumented gaps.

---

## 4. Multi-Agent Review and Approval

- **ROLE-01 (Product Steward):** Approved. Ensures requirements are preserved and properly sequenced without silent deletions.
- **ROLE-02 (Chief Architect):** Approved. Resolves architectural placement of comparison and stamps.
- **ROLE-04 (PDF Engineer):** Approved. Confirms `SAN-001` definition matches actual MuPDF engine capabilities.
- **ROLE-09 (Security Engineer):** Approved. Enforces truth-in-advertising regarding sanitization boundaries.
- **ROLE-13 (QA Engineer):** Approved. Enforces strict distinction between Kit tests and interactive UI tests.
- **ROLE-20 (Project Manager):** Approved. Authorizes M3–M7 baseline updates.
