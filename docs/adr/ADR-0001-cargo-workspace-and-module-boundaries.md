# ADR-0001 — Cargo Workspace Layout and Module Boundaries

**Status:** accepted  
**Date:** 2026-10-17  
**Owner role:** ROLE-02 (Chief Architect)  
**Related requirement IDs:** CORE-002, PLUGIN-001, SECARCH-004  

## Context

Vocument requires a shared Rust core that owns all document semantics, transactions, job scheduling, search, recovery, and security rules while keeping third-party engines (`MuPDF`, `Tesseract`, `OfficeEngine`) replaceable behind Vocument-owned interfaces and keeping native UI (`SwiftUI/AppKit`, `Kotlin/Compose`) strictly separated across a clean UniFFI transport boundary (`ARCHITECTURE.md` §§2–6, §37).

## Decision

We organize the Rust workspace into explicit layered crates mapped to the repository directory tree:

1. `core/vocument-core` — Pure Rust domain core:
   - `config`: Typed representations of `config/*.toml` (`AppDefaults`, `PerformanceBudgets`, `SecurityPolicy`).
   - `session`: `DocumentSession`, `DocumentActor`, revision tokens, file identity metadata.
   - `transactions`: `TransactionManager`, semantic commands, Undo/Redo, recovery journal contracts.
   - `jobs`: `JobManager`, priority queue, cancellation handles.
   - `brokers`: `FileBroker` and `NetworkBroker` policy boundaries.
   - Engine traits (`PdfEngine`, `OcrProvider`, `OfficeEngine`).
2. `engines/pdf-mupdf` — `PdfEngine` adapter implementation (MuPDF integration).
3. `engines/ocr` — `OcrProvider` adapter implementation (Tesseract integration).
4. `engines/imaging` — Shared Rust `ImagingEngine` scan cleanup and geometry pipeline.
5. `formats/pdoc` — Open `.pdoc` ZIP+JSON container reader/writer and validator.
6. `platform/vocument-ffi` — Thin UniFFI transport crate (`DocumentCore` API) exposing opaque handles and immutable DTO snapshots to Swift (`apps/macos`) and Kotlin (`apps/android`).

Dependency direction is strictly one-way:
`apps/{macos,android}` → `platform/vocument-ffi` → `core/vocument-core` ← `engines/*` & `formats/*`.

## Consequences

### Positive
- Enforces at compile time that `core/vocument-core` never depends on native UI or concrete C/C++ engine internals.
- Allows headless unit and corpus testing of all core workflows without launching macOS or Android UI.
- Prepares the `EngineTransport` boundary so engine workers can later run out-of-process (`XPC` / Android isolated service).

### Negative / Trade-offs
- Requires explicit adapter types at crate boundaries rather than leaking third-party types.

## Alternatives Considered

- **Single monolithic Rust crate:** Rejected because it makes accidental coupling between core logic, FFI glue, and engine internals too easy.
