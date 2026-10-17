# Native Applications (`apps/`)

Contains the platform-native UI applications:
- `apps/macos/` — SwiftUI + AppKit (`NSDocument` lifecycle wrapper, `FileBroker`)
- `apps/android/` — Kotlin + Jetpack Compose (Storage Access Framework `FileBroker`, camera capture)

All document and business logic belongs in `core/`, accessed via UniFFI (`platform/`).
