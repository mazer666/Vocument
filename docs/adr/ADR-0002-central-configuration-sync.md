# ADR-0002 — Central Configuration Loading and Typed Sync

**Status:** accepted  
**Date:** 2026-10-17  
**Owner role:** ROLE-02 (Chief Architect), ROLE-03 (Rust Core Engineer)  
**Related requirement IDs:** CORE-005, NET-001, PERF-001, LOG-001  

## Context

`README.md` and `ARCHITECTURE.md` §36 require all defaults, performance budgets, and security policies to live in `/config/*.toml` with typed code representations kept in sync, forbidding scattered magic values in Rust, Swift, or Kotlin.

## Decision

1. `/config/app-defaults.toml`, `/config/performance-budgets.toml`, and `/config/security-policy.toml` are the canonical configuration sources.
2. `core/vocument-core` defines strongly typed Rust structs (`AppDefaultsConfig`, `PerformanceBudgetsConfig`, `SecurityPolicyConfig`) deserialized via `serde` + `toml`.
3. Automated unit tests in `core/vocument-core` parse the repository's `/config/*.toml` files on every test run and assert structural completeness and policy invariants (e.g. `default_policy == DenyAll`, `enable_telemetry == false`, `enable_automatic_crash_upload == false`).
4. Native apps query configuration snapshots through `platform/vocument-ffi` rather than parsing or hardcoding defaults themselves.

## Consequences

### Positive
- Any drift between `/config/*.toml` and Rust types immediately fails `cargo test` and `./scripts/local-ci.sh`.
- Security and privacy invariants (`NET-001`, `LOG-001`) are verified by tests.

### Negative / Trade-offs
- Adding a new setting requires updating both the `.toml` file and the typed Rust struct in the same commit.

## Alternatives Considered

- **Hardcoded constants per platform:** Explicitly forbidden by architecture rules.
