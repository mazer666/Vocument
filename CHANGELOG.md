# Changelog

All notable changes to Vocument are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0-m0.1] - 2026-10-17

### Added
- Completed **M0.1 — FileBroker + MuPDF session** (`docs/acceptance/M0.1-ACCEPTANCE.md`, `ADR-0004`).
- Added `CODE-001` requirement (`docs/cr/CR-0001-source-file-size-limits.md`): hand-written source files target $\le 600$ lines with a hard maximum of $900$ lines, enforced in `config/performance-budgets.toml` and `scripts/validate-repo.sh`.
- Implemented `FileIdentity` (SHA-256, size, mtime, OS inode/SAF document ID, external-modification comparison), `DocumentId`, and `SessionId` in `core/vocument-core/src/identity.rs`.
- Implemented `FileBroker` and `ScopedLocalFileBroker` in `core/vocument-core/src/file_broker.rs`.
- Implemented `PdfEngine` trait (`core/vocument-core/src/pdf_engine.rs`) and the production **MuPDF 1.28.5** adapter (`engines/pdf-mupdf`) with `fz_try`/`fz_catch` C bridge reading real PDF page counts and Info metadata.
- Implemented `DocumentSession` and `SessionRegistry` in `core/vocument-core/src/session.rs` and exposed session lifecycle functions over UniFFI (`platform/vocument-ffi`) to both macOS (`apps/macos`) and Android (`apps/android`).

## [0.1.0-m0.0] - 2026-10-17

### Added
- Completed **M0.0 — Repository & Build Skeleton** (`docs/acceptance/M0.0-ACCEPTANCE.md`).
- Real **UniFFI** proc-macro boundary in `platform/vocument-ffi` (`DocumentCoreHandshake`, `DocumentCoreError`, `document_core_handshake()`) and reproducible binding generator `scripts/generate-bindings.sh` (`ADR-0003`).
- Native **macOS 15+** Swift package (`apps/macos`) with `VocumentFFI` C/Swift targets, `VocumentMacKit` (`DocumentCoreClient` calling UniFFI directly with zero hardcoded defaults), `VocumentMacApp` (SwiftUI/AppKit shell), and `vocument-mac-handshake` CLI verifier.
- Native **Android 13+ (`minSdk = 33`, `arm64-v8a`)** project (`apps/android`) with Jetpack Compose (`MainActivity.kt`), `DocumentCoreBridge.kt` calling generated UniFFI bindings, and headless Kotlin/JNA verification (`scripts/verify-kotlin-handshake.sh`).
- Official AGPLv3 `LICENSE`, `REUSE.toml`, `rust-toolchain.toml`, `deny.toml`, Git hooks (`scripts/install-hooks.sh`), and immutable Baseline 0.1 snapshot (`archive/baselines/0.1/`).

## [0.1] - 2026-10-17

### Added
- Initial architecture baseline (`ARCHITECTURE.md`), product requirements (`REQUIREMENTS.md`), roadmap (`ROADMAP.md`), quality model (`QUALITY.md`), project governance (`PROJECT_MANAGEMENT.md`, `AGENTS.md`), and central configuration structure (`config/`).
- Repository directory layout for M0 ("Foundation / Trustworthy Viewer").
