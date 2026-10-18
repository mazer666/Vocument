# Vocument Documentation Index

This directory contains detailed living documentation for Vocument.

## Core Documents (Root)
- [`../README.md`](../README.md) — Entry point & overview
- [`../REQUIREMENTS.md`](../REQUIREMENTS.md) — Product requirements (source of truth)
- [`../ARCHITECTURE.md`](../ARCHITECTURE.md) — Technical architecture baseline
- [`../ROADMAP.md`](../ROADMAP.md) — Milestones and delivery gates
- [`../QUALITY.md`](../QUALITY.md) — 6-dimensional acceptance and quality model
- [`../AGENTS.md`](../AGENTS.md) — Roles, rules, and review matrix for coding agents
- [`../PROJECT_MANAGEMENT.md`](../PROJECT_MANAGEMENT.md) — Governance, ADRs, CRs, and archive policy
- [`../WHERE_TO_CHANGE.md`](../WHERE_TO_CHANGE.md) — Developer navigation guide

## Detailed Living Docs (`docs/`)
- [`BEST_IN_CLASS.md`](BEST_IN_CLASS.md) — Measurable benchmark and competitive framework
- [`DOCUMENTATION_POLICY.md`](DOCUMENTATION_POLICY.md) — Rules for keeping docs in sync with code
- [`project/RISKS.md`](project/RISKS.md) — Active risk register
- [`features/FEATURE_TEMPLATE.md`](features/FEATURE_TEMPLATE.md) — Template for granular feature specifications
- [`acceptance/ACCEPTANCE_TEMPLATE.md`](acceptance/ACCEPTANCE_TEMPLATE.md) — Template for milestone/feature acceptance evidence
- [`acceptance/M0.0-ACCEPTANCE.md`](acceptance/M0.0-ACCEPTANCE.md) — Completed acceptance record for M0.0
- [`acceptance/M0.1-ACCEPTANCE.md`](acceptance/M0.1-ACCEPTANCE.md) — Completed acceptance record for M0.1
- [`acceptance/M0.2-ACCEPTANCE.md`](acceptance/M0.2-ACCEPTANCE.md) — Completed acceptance record for M0.2
- [`acceptance/M0.3-ACCEPTANCE.md`](acceptance/M0.3-ACCEPTANCE.md) — Completed acceptance record for M0.3
- [`acceptance/M0.4-ACCEPTANCE.md`](acceptance/M0.4-ACCEPTANCE.md) — Completed acceptance record for M0.4
- [`acceptance/M0.5-ACCEPTANCE.md`](acceptance/M0.5-ACCEPTANCE.md) — Completed acceptance record for M0.5
- [`acceptance/M0.6-ACCEPTANCE.md`](acceptance/M0.6-ACCEPTANCE.md) — Completed acceptance record for M0.6
- [`acceptance/M0.7-ACCEPTANCE.md`](acceptance/M0.7-ACCEPTANCE.md) — Completed acceptance record for M0.7
- [`acceptance/M0-ACCEPTANCE.md`](acceptance/M0-ACCEPTANCE.md) — Completed milestone acceptance record for M0
- [`acceptance/M1.0-ACCEPTANCE.md`](acceptance/M1.0-ACCEPTANCE.md) — Completed acceptance record for M1.0
- [`acceptance/M1.1-ACCEPTANCE.md`](acceptance/M1.1-ACCEPTANCE.md) — Completed acceptance record for M1.1
- [`acceptance/M1.2-ACCEPTANCE.md`](acceptance/M1.2-ACCEPTANCE.md) — Completed acceptance record for M1.2
- [`acceptance/M1.3-ACCEPTANCE.md`](acceptance/M1.3-ACCEPTANCE.md) — Completed acceptance record for M1.3
- [`adr/ADR_TEMPLATE.md`](adr/ADR_TEMPLATE.md) — Template for Architecture Decision Records
- [`adr/ADR-0001-cargo-workspace-and-module-boundaries.md`](adr/ADR-0001-cargo-workspace-and-module-boundaries.md) — Cargo workspace & crate layering
- [`adr/ADR-0002-central-configuration-sync.md`](adr/ADR-0002-central-configuration-sync.md) — Central `/config/*.toml` synchronization
- [`adr/ADR-0003-uniffi-binding-generation-and-bridge-verification.md`](adr/ADR-0003-uniffi-binding-generation-and-bridge-verification.md) — UniFFI binding generation & Swift/Kotlin bridge verification
- [`adr/ADR-0004-m0-1-filebroker-document-session-and-mupdf-adapter.md`](adr/ADR-0004-m0-1-filebroker-document-session-and-mupdf-adapter.md) — M0.1 `FileBroker`, `DocumentSession`, `FileIdentity`, and MuPDF adapter
- [`adr/ADR-0005-m0-2-rendering-actor-tile-cache-and-canvas.md`](adr/ADR-0005-m0-2-rendering-actor-tile-cache-and-canvas.md) — M0.2 `DocumentActor`, `JobManager`, `TileCache`, and native `DocumentCanvas`
- [`adr/ADR-0006-m0-3-navigation-thumbnails-selection-and-fts5-search.md`](adr/ADR-0006-m0-3-navigation-thumbnails-selection-and-fts5-search.md) — M0.3 navigation, logical page labels, `ThumbnailCache`, `OCRCOPY-002`, and progressive SQLite FTS5 search
- [`adr/ADR-0007-m0-4-transaction-manager-mutations-undo-redo-and-journal.md`](adr/ADR-0007-m0-4-transaction-manager-mutations-undo-redo-and-journal.md) — M0.4 `TransactionManager`, highlight annotations, page rotation, Undo/Redo, and persistent transaction journal
- [`adr/ADR-0008-m0-5-safe-save-atomic-staging-validation-and-conflict-handling.md`](adr/ADR-0008-m0-5-safe-save-atomic-staging-validation-and-conflict-handling.md) — M0.5 Safe Save, atomic staging, post-save validation, and conflict handling
- [`adr/ADR-0009-m0-6-crash-recovery-containers-and-restore-actions.md`](adr/ADR-0009-m0-6-crash-recovery-containers-and-restore-actions.md) — M0.6 Crash recovery containers, dirty session detection, and restore actions
- [`adr/ADR-0010-m0-7-interoperability-gate-and-foreign-viewer-verification.md`](adr/ADR-0010-m0-7-interoperability-gate-and-foreign-viewer-verification.md) — M0.7 Interoperability gate and foreign viewer verification
- [`adr/ADR-0011-m1-0-android-runtime-foundation-and-scan-capture-pipeline.md`](adr/ADR-0011-m1-0-android-runtime-foundation-and-scan-capture-pipeline.md) — M1.0 Android runtime foundation, camera scan-capture pipeline, and live edge detection
- [`adr/ADR-0012-m1-1-perspective-correction-deskew-and-adaptive-rotation.md`](adr/ADR-0012-m1-1-perspective-correction-deskew-and-adaptive-rotation.md) — M1.1 Perspective correction, deskew, and adaptive rotation
- [`adr/ADR-0013-m1-2-intelligent-scan-cleanup-and-enhancement.md`](adr/ADR-0013-m1-2-intelligent-scan-cleanup-and-enhancement.md) — M1.2 Intelligent scan cleanup and enhancement
- [`adr/ADR-0014-m1-3-tesseract-ocr-and-text-layer-generation.md`](adr/ADR-0014-m1-3-tesseract-ocr-and-text-layer-generation.md) — M1.3 Offline Tesseract OCR & text layer generation
- [`cr/CR_TEMPLATE.md`](cr/CR_TEMPLATE.md) — Template for Change Requests
- [`cr/CR-0001-source-file-size-limits.md`](cr/CR-0001-source-file-size-limits.md) — Source file size limits (`CODE-001`: target $\le 600$, max $900$ lines)
- [`cr/CR-0002-prohibition-of-stubs-in-acceptance.md`](cr/CR-0002-prohibition-of-stubs-in-acceptance.md) — Prohibition of stubs in feature acceptance (`QA-003`)
