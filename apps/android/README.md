# Vocument Android (`apps/android/`)

Android 13+ (`API 33+`, primary ABI `arm64-v8a`) native UI layer built with Kotlin and Jetpack Compose (`ARCHITECTURE.md` §3).

## Responsibilities
- Jetpack Compose UI (`Minimal Reader` preset, `Pages / Search / Tools` bottom bar)
- `Storage Access Framework` implementation of `FileBroker`
- Camera capture and live edge assistance (`M1`)
- OS-compliant background execution adapters
- TalkBack and S-Pen integration

All document state (`DocumentSession`, `TransactionManager`, `JobManager`, `RecoveryStore`) remains in `core/vocument-core` and is accessed via `platform/vocument-ffi`.
