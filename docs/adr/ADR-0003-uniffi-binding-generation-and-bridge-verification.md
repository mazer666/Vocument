# ADR-0003 — UniFFI Binding Generation and Native Bridge Verification

**Status:** accepted  
**Date:** 2026-10-17  
**Owner role:** ROLE-02 (Chief Architect), ROLE-05 (macOS Engineer), ROLE-06 (Android Engineer)  
**Related requirement IDs:** PLATFORM-001, UI-001, SECARCH-004  

## Context

`ARCHITECTURE.md` §5 mandates that UniFFI is a thin transport layer exposing opaque handles and immutable DTO snapshots (`DocumentCore` API), and `AGENTS.md` §1 rule 13 mandates:
*"keep generated code generated; never hand-edit generated UniFFI bindings"*.

Furthermore, `ROADMAP.md` M0.0 requires a reproducible build and an end-to-end proof where both Swift (macOS 15+) and Kotlin (Android 13+) invoke the shared Rust core without duplicated or hardcoded values.

## Decision

1. **Proc-macro UniFFI export in `platform/vocument-ffi`:**
   We use `uniffi::setup_scaffolding!("vocument_ffi")` with `#[derive(uniffi::Record)]`, `#[derive(uniffi::Error)]`, and `#[uniffi::export]` on the `DocumentCore` API surface.
2. **Deterministic local binding generator (`scripts/generate-bindings.sh`):**
   `platform/vocument-ffi` provides a `uniffi-bindgen` binary target. Running `./scripts/generate-bindings.sh` (or `make bindings`) compiles `libvocument_ffi` and generates:
   - Swift bindings + C header/modulemap into `apps/macos/Sources/{VocumentFFI,VocumentFFIHeaders}`
   - Kotlin bindings into `apps/android/app/src/main/java/uniffi/vocument_ffi`
3. **CI drift gate:**
   `scripts/local-ci.sh` re-runs `./scripts/generate-bindings.sh` and fails if `git diff` detects any uncommitted or hand-edited changes in the generated binding directories.
4. **End-to-end runtime verification on both platforms:**
   - macOS (`apps/macos`): `VocumentMacKit` wraps the generated `VocumentFFI` module (`DocumentCoreClient.performHandshake()`). Both `VocumentMacApp` (SwiftUI/AppKit) and `vocument-mac-handshake` invoke the live Rust library.
   - Android (`apps/android`): Configured for Android 13+ (`minSdk = 33`, primary ABI `arm64-v8a`) with Jetpack Compose (`MainActivity.kt`) and `DocumentCoreBridge.kt`. `scripts/verify-kotlin-handshake.sh` compiles the generated Kotlin bindings + `DocumentCoreBridge.kt` and executes the real JNA/UniFFI call into `libvocument_ffi` during every `make check`.

## Consequences

### Positive
- Zero hardcoded or duplicated configuration defaults in Swift or Kotlin.
- Any change to `config/*.toml` or `vocument-core` is immediately reflected and tested across Rust, Swift, and Kotlin in a single command (`make check`).
- Generated bindings cannot silently drift or be manually edited.

### Negative / Trade-offs
- Error enum variant fields in `vocument-ffi` must avoid naming collisions with `kotlin.Throwable.message` (we use `details: String`).
