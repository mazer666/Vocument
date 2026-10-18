# ADR-0011: M1.0 Android Runtime Foundation and Camera Scan-Capture Pipeline

- **Status:** Accepted
- **Date:** 2026-10-18
- **Milestone:** M1.0 — Android Runtime Foundation & Scan-Capture Pipeline (`ROADMAP.md` M1.0, `ARCHITECTURE.md` §21, `REQUIREMENTS.md` §8)
- **Requirements Addressed:** `SCAN-001`, `SCAN-004`, `SCAN-006`, `MOBILE-001`, `MOBILE-002`, `NET-001`, `CODE-001`

---

## Context

With Milestone M0 (Foundation / Trustworthy Viewer) completed and verified across all interoperability gates, work begins on **Milestone M1 (Android Scan & OCR)**.

Initial gap analysis identified that the previous Android validation in `local-ci.sh` was restricted to a desktop Kotlin/JNA wrapper (`VerifyKotlinHandshakeCli.kt`) rather than an authentic Android runtime test suite with Gradle APK packaging, Android SDK target verification, and manifest permission checks.

To establish a solid, production-grade basis for M1, submilestone **M1.0** defines the core scan-capture foundation:
1. **Real Android Runtime Harness:** Installing Android command-line tools, SDK platforms (`android-35`), build tools (`35.0.0`), generating the Gradle wrapper (`gradlew`), and building an authentic release/debug APK (`app-debug.apk`) alongside Gradle unit tests linked to the native Rust shared library.
2. **Camera Scan-Capture Pipeline:** Ingesting raw camera scan frames (`Rgb8`, `Rgba8`, `Gray8`, `Nv21`, `Yuv420`) and performing real-time document edge detection and quadrilateral corner proposal in the shared `ImagingEngine` (`vocument-imaging`).
3. **Session Management:** Multi-page scan capture session state registry in Rust core, avoiding UI state duplication or data loss before OCR processing.
4. **Offline & Security Invariants:** Declaring camera hardware access (`android.permission.CAMERA`) while strictly maintaining the zero-network policy (`NET-001`, no `android.permission.INTERNET`).

---

## Decision

1. **Shared Imaging Core (`vocument-imaging`):**
   - Implemented `engines/imaging/src/lib.rs` (588 lines, strictly adhering to `CODE-001` $\le 600$ target):
     - `Point2D`: 2D point normalized coordinates `[0.0, 1.0]`.
     - `QuadCorners`: 4 ordered corners (`top_left`, `top_right`, `bottom_right`, `bottom_left`) with geometric convexity validation (`is_convex()`), polygon area (`area()`), and sanity checks (`is_valid_document_quad()`).
     - `ScanFrame` & `ScanFrameMetadata`: Multi-format raw pixel container (`Rgb8`, `Rgba8`, `Gray8`, `Nv21`, `Yuv420`), device angle, brightness hints, and camera stability flags.
     - `detect_document_edges`: Real-time edge detection analyzing high-contrast luminance gradients, computing optimal quadrilateral bounding corners and confidence ratings (0–1000‰).
     - `ScanCaptureSession` & `ScanCaptureSessionRegistry`: Thread-safe multi-page scan capture session lifecycle management (`start`, `append_page`, `get_session_snapshot`, `close`).
     - `ScanCleanupProfile`: Profiles for `DocumentAuto`, `PreservePhoto`, and `HighContrastBw`.

2. **UniFFI Boundary Layer (`vocument-ffi/src/scan_dto.rs`):**
   - Exposed DTOs and boundary functions:
     - `document_core_detect_document_edges(frame: FfiScanFrame) -> Result<Option<FfiQuadCorners>, DocumentCoreError>`
     - `document_core_start_scan_session() -> String`
     - `document_core_append_scan_page(session_id, frame, corners, cleanup_profile) -> Result<FfiCapturedScanPage, DocumentCoreError>`
     - `document_core_get_scan_session(session_id) -> Result<FfiScanSessionSnapshot, DocumentCoreError>`
     - `document_core_close_scan_session(session_id) -> Result<(), DocumentCoreError>`
   - Added `ImagingFailure { details: String }` to `DocumentCoreError` with automated conversion from `vocument_imaging::ImagingError`.

3. **Real Android Runtime Test Harness (`scripts/verify-android-runtime.sh`):**
   - Configured `apps/android/gradlew` with Gradle 9.8.1 and Android Gradle Plugin 8.8.2.
   - Wired Gradle unit tests (`:app:testDebugUnitTest`) with `jna.library.path` pointing to `$REPO_ROOT/target/debug` and JVM arg `--enable-native-access=ALL-UNNAMED`.
   - Built real debug APK: `apps/android/app/build/outputs/apk/debug/app-debug.apk` (23 MB).
   - Validated APK metadata via `apkanalyzer`:
     - Package: `org.vocument.app`, version code `1`, version name `0.1.0`.
     - Declared permissions: `android.permission.CAMERA` present.
     - Strict offline-first invariant: `android.permission.INTERNET` strictly absent (`NET-001`).
   - Wired `scripts/verify-android-runtime.sh` directly into `scripts/local-ci.sh`.

4. **Android Native Compose UI Integration (`ScanCaptureScreen.kt`):**
   - Implemented `ScanCaptureScreen` Jetpack Compose view:
     - Live camera viewfinder canvas with dynamic quadrilateral edge detection overlay (switching to vibrant green `#00E676` when confidence $\ge 650$‰).
     - Multi-page reel displaying captured page thumbnails and count badge.
     - Mode selection chips for `DocumentAuto`, `PreservePhoto`, `HighContrastBw`.
     - Frame simulation and camera capture hooks.
   - Integrated as the 4th tab ("Scan (M1)") in `MainActivity.kt`.
   - Added Android unit test `ScanCaptureBridgeTest.kt` verifying edge detection and multi-page scan session lifecycle.

---

## Consequences

- **Authentic Android Pipeline:** Android CI is no longer limited to desktop Kotlin wrappers; it now verifies genuine Android APK packaging and Android Gradle unit test execution.
- **Shared Core Separation:** All geometric validation and edge detection calculations remain in the shared Rust `vocument-imaging` engine, completely decoupled from Android UI code.
- **Ready for M1.1:** The multi-page captured frames and corner coordinates are stored in the Rust core session registry, ready for perspective warp and deskew transformation in M1.1.
