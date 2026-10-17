# Documentation Policy

Documentation in Vocument is part of the product (`README.md`, `PROJECT_MANAGEMENT.md` §7).

---

## 1. Same-Change Rule

A code or configuration change is **incomplete** if it makes any documentation stale. Updates must happen in the **same commit/PR**:

- Behaviour change → `REQUIREMENTS.md` (via Change Request `docs/cr/` if material) and `docs/features/`
- Structural/boundary change → `ARCHITECTURE.md` (via ADR `docs/adr/` if material) and `WHERE_TO_CHANGE.md`
- Default/limit change → `config/*.toml` and corresponding typed Rust models
- Milestone/status change → `ROADMAP.md` and `project/project.yaml`
- Quality/test gate change → `QUALITY.md` and `docs/acceptance/`

---

## 2. Single Source of Truth

Never duplicate magic numbers or contradictory rules across multiple files:

- Machine-readable project state lives in [`project/project.yaml`](../project/project.yaml).
- Configuration defaults and budgets live in [`config/`](../config/README.md).
- Historical snapshots live read-only under [`archive/`](../archive/).
