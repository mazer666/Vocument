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
- [`adr/ADR_TEMPLATE.md`](adr/ADR_TEMPLATE.md) — Template for Architecture Decision Records
- [`adr/ADR-0001-cargo-workspace-and-module-boundaries.md`](adr/ADR-0001-cargo-workspace-and-module-boundaries.md) — Cargo workspace & crate layering
- [`adr/ADR-0002-central-configuration-sync.md`](adr/ADR-0002-central-configuration-sync.md) — Central `/config/*.toml` synchronization
- [`adr/ADR-0003-uniffi-binding-generation-and-bridge-verification.md`](adr/ADR-0003-uniffi-binding-generation-and-bridge-verification.md) — UniFFI binding generation & Swift/Kotlin bridge verification
- [`adr/ADR-0004-m0-1-filebroker-document-session-and-mupdf-adapter.md`](adr/ADR-0004-m0-1-filebroker-document-session-and-mupdf-adapter.md) — M0.1 `FileBroker`, `DocumentSession`, `FileIdentity`, and MuPDF adapter
- [`adr/ADR-0005-m0-2-rendering-actor-tile-cache-and-canvas.md`](adr/ADR-0005-m0-2-rendering-actor-tile-cache-and-canvas.md) — M0.2 `DocumentActor`, `JobManager`, `TileCache`, and native `DocumentCanvas`
- [`cr/CR_TEMPLATE.md`](cr/CR_TEMPLATE.md) — Template for Change Requests
- [`cr/CR-0001-source-file-size-limits.md`](cr/CR-0001-source-file-size-limits.md) — Source file size limits (`CODE-001`: target $\le 600$, max $900$ lines)
