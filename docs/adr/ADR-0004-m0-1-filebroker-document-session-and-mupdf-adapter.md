# ADR-0004 — M0.1 FileBroker, DocumentSession, FileIdentity, and MuPDF Adapter

**Status:** accepted  
**Date:** 2026-10-17  
**Owner role:** ROLE-02 (Chief Architect), ROLE-03 (Rust Core Engineer), ROLE-04 (PDF/MuPDF Engineer)  
**Related requirement IDs:** FILEINT-001, FILELOCK-002, HASH-001, INSPECT-001, META-001, SECARCH-001  

## Context

`ROADMAP.md` M0.1 requires:
- Opening PDFs from Finder / Open With on macOS and Storage Access Framework on Android via `FileBroker` (`ARCHITECTURE.md` §15).
- Creating an application-owned `DocumentSession` with stable internal `SessionId` and `DocumentId` (`ARCHITECTURE.md` §6).
- Multi-signal `FileIdentity` and SHA-256 fingerprinting that never relies on path alone (`ARCHITECTURE.md` §14).
- Reading PDF page count and basic metadata via MuPDF behind Vocument's `PdfEngine` interface (`ARCHITECTURE.md` §18) while enforcing the gate: *"No UI layer accesses MuPDF directly"*.

## Decision

1. **`FileIdentity` & Stable IDs (`core/vocument-core/src/identity.rs`):**
   - `FileIdentity` combines `source_token`, `display_name`, optional `os_file_id` (POSIX `dev:ino` on macOS, SAF Document ID on Android), `size_bytes`, `modified_unix_ms`, and `sha256_hex`.
   - `FileIdentity::compare_with` deterministically classifies observed files into `Unchanged`, `SameFileMoved`, `ProbableCopyOfSameSource`, `ModifiedExternally`, `DifferentFileReplacingSamePath`, or `Unrelated`.
   - `DocumentId` (`doc_<16-hex-of-sha256>`) and `SessionId` (`sess_<counter>_<document_id>`) are opaque internal identifiers.
2. **`FileBroker` (`core/vocument-core/src/file_broker.rs`):**
   - Defines `FileAccessOrigin` (`MacOsScopedUrl`, `AndroidSafUri`, `InMemoryBuffer`), `BrokeredFileRead`, and `FileBroker`.
   - `ScopedLocalFileBroker` reads security-scoped macOS files and extracts POSIX `dev:ino` and modification time. Android SAF passes `FfiBrokeredFileDescriptor` resolved via `ContentResolver` + `DocumentsContract`.
3. **`PdfEngine` & `MuPdfEngineAdapter` (`core/vocument-core/src/pdf_engine.rs`, `engines/pdf-mupdf/`):**
   - `PdfEngine` trait owns `open_pdf_bytes(&[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError>`.
   - `engines/pdf-mupdf` compiles a thin C shim (`csrc/mupdf_bridge.c`) that safely encapsulates MuPDF's `setjmp`/`longjmp` (`fz_try`/`fz_always`/`fz_catch`) on an isolated `fz_context`, opening the byte stream via `fz_open_memory` + `fz_open_document_with_stream("application/pdf", ...)` and reading `fz_count_pages`, `fz_needs_password`, and standard `FZ_META_INFO_*` keys without ever leaking `fz_*` pointers across FFI.
4. **`DocumentSession` & `SessionRegistry` (`core/vocument-core/src/session.rs`):**
   - Owns active sessions and exposes immutable `DocumentSessionSnapshot` DTOs over `platform/vocument-ffi`.

## Consequences

### Positive
- Complete isolation of MuPDF C internals and `longjmp` exceptions inside `engines/pdf-mupdf`.
- Identical `DocumentId` (`doc_1a00192b75fb403b`) and `PdfDocumentSummary` produced whether the same PDF is opened via macOS security-scoped URL or Android Storage Access Framework URI.
- All hand-written files remain well under 350 lines (`CODE-001` target $\le 600$ lines).
