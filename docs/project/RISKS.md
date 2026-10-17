# Vocument Risk Register

Tracked according to `PROJECT_MANAGEMENT.md` §9.

| ID | Description | Probability | Impact | Mitigation | Owner Role | Status |
|---|---|---|---|---|---|---|
| `RISK-001` | Concurrent access to mutable MuPDF state across threads causes crashes or memory corruption | Medium | Critical (S4) | Enforce serialized `DocumentActor` per session; only immutable display-lists/tiles run on worker threads (`ARCHITECTURE.md` §7) | ROLE-04 (PDF/MuPDF) | Active |
| `RISK-002` | External file modification (Finder, iCloud, SAF provider) races with local save and causes silent data loss | Medium | Critical (S4) | Multi-signal file identity check before atomic replacement; mandatory conflict UI (`ARCHITECTURE.md` §14, §16) | ROLE-03 (Rust Core) | Active |
| `RISK-003` | Visual-only redaction leaves underlying text, OCR layer, or hidden objects extractable | Low | Critical (S4) | Two-stage redaction with mandatory independent reopen/search/extract verification (`ARCHITECTURE.md` §29) | ROLE-09 (Security) | Active |
| `RISK-004` | UniFFI boundary leaks engine internals or blocks future out-of-process worker isolation | Low | High (S3) | Restrict UniFFI surface to opaque handles, immutable DTOs, and `EngineTransport` abstraction (`ARCHITECTURE.md` §5, §37) | ROLE-02 (Architect) | Active |
| `RISK-005` | Large scan PDFs exhaust mobile RAM on Android | Medium | High (S3) | Adaptive tile cache with strict memory budgets (`config/performance-budgets.toml`) and OS memory-pressure eviction | ROLE-14 (Performance) | Active |
