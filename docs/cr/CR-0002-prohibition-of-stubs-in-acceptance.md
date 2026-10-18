# CR-0002 — Prohibition of Stubs in Feature Acceptance (QA-003)

**Status:** implemented  
**Date:** 2026-10-18  
**Proposer role:** ROLE-01 (Product & Requirements Steward), ROLE-13 (QA & Test Engineer)  
**Reviewer roles:** ROLE-02 (Chief Architect), ROLE-20 (Project / Release Manager)  

## Problem

During the implementation of cross-platform milestones (such as Android Scan & OCR), there is a risk that stub/dummy C shims, synthetic placeholders, or mock bridges are introduced to allow CI builds or tests to succeed without executing actual functionality. If a milestone is signed off or accepted on the basis of stubs, product requirements are violated, real runtime defects remain undetected, and the user-facing app fails in production or on target devices.

## Requested Change

Add a binding quality and governance requirement (`QA-003`) to `REQUIREMENTS.md`, `AGENTS.md`, `QUALITY.md`, `ROADMAP.md`, and `scripts/validate-repo.sh`:

1. **No Stubs for Acceptance (`QA-003`):** No functionality, engine adapter, provider, or platform workflow presented for milestone or feature acceptance may be implemented solely as a stub, dummy return value, synthetic placeholder, or mock bridge.
2. **Authentic Verification:** Acceptance gates, automated test suites, and CI pipelines must execute real, fully integrated native/embedded implementations with genuine language models, native libraries, and realistic test corpora on authentic host or target platform runtimes (e.g. Android devices/emulators, macOS host binaries).
3. **Strict Gate Enforcement:** If a platform or engine integration does not yet have its genuine implementation ready, that scope cannot be accepted or marked green; it must remain open or be explicitly descoped via a formal Change Request rather than faked with a stub.
4. **Agent Discipline:** Coding and review agents are strictly prohibited from implementing stubs as accepted deliverables or declaring a task/milestone complete when relying on stubs.

## Reason

Guarantees the integrity and veracity of acceptance records, protects product quality, eliminates false senses of security, and upholds Vocument's standard of trustworthy, verifiable engineering.

## Impacted Requirement IDs

- Adds `QA-003` in `REQUIREMENTS.md` (§43 Governance / Quality Assurance).

## Impacted Architecture

- `engines/` (all engine adapters must be real implementations when accepted).
- `platform/` (FFI bindings and bridges must invoke genuine native logic).
- `apps/` (native platform apps must verify real execution on target devices/emulators).

## Impacted Platforms

- All (`macOS`, `Android`, `Rust Core`, native C/NDK bridges).

## Milestone Impact

- Active immediately and binding for all current and future milestones (M0, M1, M2, etc.).
- Retroactively enforced on M1.3 (where the initial Android Tesseract stub was completely removed and replaced with embedded native Tesseract 5 before acceptance was granted).

## Acceptance / Test Changes

- `scripts/validate-repo.sh` enforces the presence of `QA-003` in requirements and governance documents, rejects stub files for accepted milestones, and validates genuine native binaries and assets.
- `AGENTS.md` adds explicit rules in §1 (Universal rules), §5 (Definition of agent-done), and §6 (Forbidden agent behaviour).

## Risks

- Developing real embedded integrations on mobile platforms requires more setup effort and native toolchains (NDK, native shared libraries), but this is necessary to ensure actual functional delivery.

## Decision

**approved & implemented**
