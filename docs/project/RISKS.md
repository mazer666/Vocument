# Vocument Risk Register

Tracked according to `PROJECT_MANAGEMENT.md` §9.

| ID | Description | Probability | Impact | Mitigation | Owner Role | Status |
|---|---|---|---|---|---|---|
| `RISK-001` | Concurrent access to mutable MuPDF state across threads causes crashes or memory corruption | Medium | Critical (S4) | Enforce serialized `DocumentActor` per session; only immutable display-lists/tiles run on worker threads (`ARCHITECTURE.md` §7) | ROLE-04 (PDF/MuPDF) | Active |
| `RISK-002` | External file modification (Finder, iCloud, SAF provider) races with local save and causes silent data loss | Medium | Critical (S4) | Multi-signal file identity check before atomic replacement; mandatory conflict UI (`ARCHITECTURE.md` §14, §16) | ROLE-03 (Rust Core) | Active |
| `RISK-003` | Visual-only redaction leaves underlying text, OCR layer, or hidden objects extractable | Low | Critical (S4) | Two-stage redaction with mandatory independent reopen/search/extract verification (`ARCHITECTURE.md` §29) | ROLE-09 (Security) | Active |
| `RISK-004` | UniFFI boundary leaks engine internals or blocks future out-of-process worker isolation | Low | High (S3) | Restrict UniFFI surface to opaque handles, immutable DTOs, and `EngineTransport` abstraction (`ARCHITECTURE.md` §5, §37) | ROLE-02 (Architect) | Active |
| `RISK-005` | Large scan PDFs exhaust mobile RAM on Android | Medium | High (S3) | Adaptive tile cache with strict memory budgets (`config/performance-budgets.toml`) and OS memory-pressure eviction | ROLE-14 (Performance) | Active |
| `RISK-006` | Android links a placeholder MuPDF bridge (`engines/pdf-mupdf/csrc/mupdf_stub_bridge.c`) instead of a real MuPDF build, so PDF viewing/rendering/editing on Android is not functional on device | High | Critical (S4) | Documented in `ADR-0016`; the placeholder reports a distinguishable engine identifier (`mupdf-stub-1.28`) and `scripts/validate-repo.sh` rejects an undocumented placeholder. Real MuPDF must be cross-compiled for `aarch64-linux-android` before Android PDF support may be claimed | ROLE-04 (PDF/MuPDF) | Active |
| `RISK-007` | No background worker pool exists: `JobManager` only tracks jobs, while rendering/thumbnailing/indexing run synchronously on the calling thread (`ARCHITECTURE.md` §8.1) | Medium | High (S3) | Documented as not implemented in `ARCHITECTURE.md` §8.1; the worker pool, pause/resume, job dependencies and resumability are scoped for M1.5 (`ROADMAP.md`) | ROLE-03 (Rust Core) | Active |
