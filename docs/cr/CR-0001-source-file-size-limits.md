# CR-0001 — Source File Size Limits for Maintainability (CODE-001)

**Status:** implemented  
**Date:** 2026-10-17  
**Proposer role:** ROLE-01 (Product & Requirements Steward), ROLE-02 (Chief Architect)  
**Reviewer roles:** ROLE-13 (QA & Test Engineer), ROLE-17 (Documentation & DX Lead)  

## Problem

As Vocument grows across Rust (`core/`, `engines/`, `formats/`, `platform/`), Swift (`apps/macos`), and Kotlin (`apps/android`), large monolithic source files make the codebase harder to navigate, review, and maintain — especially for coding beginners and multi-agent reviews.

## Requested Change

Add a binding source-file size requirement (`CODE-001`) and enforce it automatically in `config/performance-budgets.toml`, `REQUIREMENTS.md`, `AGENTS.md`, `QUALITY.md`, and `scripts/validate-repo.sh`:

- **Target file size:** $\le 600$ lines per hand-written source code file (`.rs`, `.swift`, `.kt`, `.c`, `.h`, `.sh`).
- **Hard maximum:** $900$ lines per hand-written source code file (generated UniFFI binding files are excluded from the hand-written limit, as they are machine-generated and never hand-edited).

## Reason

Keeps modules focused on single responsibilities, preserves architectural clarity, and ensures any file can be understood and reviewed in full without context overflow.

## Impacted Requirement IDs

- Adds `CODE-001` in `REQUIREMENTS.md` (§43 Governance / Code Maintainability).

## Impacted Architecture

- Reinforces modular file splitting across `core/`, `engines/`, `formats/`, `platform/`, and `apps/`.

## Impacted Platforms

- All (`Rust`, `macOS/Swift`, `Android/Kotlin`, `C` shims, `scripts/`).

## Milestone Impact

- Active immediately from M0.1 onward.

## Acceptance / Test Changes

- `scripts/validate-repo.sh` checks all hand-written source files: warns above 600 lines and fails CI above 900 lines.
- `config/performance-budgets.toml` and `core/vocument-core/src/config.rs` include `code_limits.target_max_lines_per_file = 600` and `code_limits.hard_max_lines_per_file = 900`.

## Risks

- None; encourages clean module decomposition early.

## Decision

**approved & implemented**
