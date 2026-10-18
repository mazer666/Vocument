# ADR-0012: M1.1 Perspective Correction, Deskew and Adaptive Rotation

- **Status:** Accepted
- **Date:** 2026-10-18
- **Milestone:** M1.1 — Perspective Correction, Deskew & Adaptive Rotation (`ROADMAP.md` M1.1, `ARCHITECTURE.md` §21, `REQUIREMENTS.md` §8)
- **Requirements Addressed:** `SCAN-003`, `SCAN-004`, `SCAN-005`, `MOBILE-001`, `CODE-001`

---

## Context

Document pages captured via handheld camera sensors are inherently subject to perspective distortion (non-orthogonal camera angle resulting in arbitrary convex trapezoidal page bounds) and rotational tilt.

While Milestone M1.0 successfully delivered live CameraX frame ingestion, real-time edge detection, and multi-page session state management, downstream stages—specifically document cleanup (`M1.2`), offline Tesseract OCR (`M1.3`), and searchable PDF synthesis (`M1.4`)—require an upright, rectified, planar rectangular image coordinate space.

`REQUIREMENTS.md` §8 (`SCAN-003`) mandates:
> *"Perspective correction is mandatory."*

and `SCAN-004` requires:
> *"Automatic detection, rotation, deskew, shadow reduction, contrast/colour/white balance, noise reduction, OCR and sensible compression."*

---

## Decision

1. **Modular Architecture & `CODE-001` Adherence:**
   - To strictly preserve the 600-line target per source file (`CODE-001`), the imaging engine was factored into focused, single-responsibility modules:
     - `engines/imaging/src/geometry.rs` (155 lines): `Point2D`, `QuadCorners`, convexity testing (`is_convex`), polygon area (`area`), and document aspect/area sanity validation (`is_valid_document_quad`).
     - `engines/imaging/src/perspective.rs` (505 lines): `ProjectiveMapping`, `warp_perspective`, `deskew_frame`, `rotate_frame_fine`, `rotate_frame_90`, and `auto_orient_frame`.
     - `engines/imaging/src/lib.rs` (511 lines): Root engine façade, multi-format pixel buffer containers (`ScanFrame`), edge detection, and session lifecycle (`ScanCaptureSessionRegistry`).

2. **Heckbert 3x3 Projective Homography Mapping (`SCAN-003`):**
   - Implemented closed-form 3x3 homography mapping between destination unit square `[0, 1]^2` and quadrilateral vertices $(p_0, p_1, p_2, p_3)$:
     $$\begin{pmatrix} x' \\ y' \\ 1 \end{pmatrix} \sim \begin{pmatrix} a & b & c \\ d & e & f \\ g & h & 1 \end{pmatrix} \begin{pmatrix} u \\ v \\ 1 \end{pmatrix}$$
   - Degenerate affine cases ($\text{det} \approx 0$, e.g. exact parallelograms) automatically collapse into standard affine transforms.
   - Bilinear interpolation resampling produces clean, artifact-free output across `Rgba8`, `Rgb8`, and `Gray8` buffers, dynamically deriving target width and height from Euclidean quad edge lengths or an optional target aspect ratio.

3. **Multi-Angle Baseline Deskew Estimation (`SCAN-004`, `SCAN-005`):**
   - Document skew is determined by sampling horizontal line brightness variance across an angular sweep ($[-8.0^\circ, +8.0^\circ]$): alignment with dominant text baselines produces peak variance between text and interline spacing.
   - For angles with $|\theta| \ge 0.5^\circ$, `rotate_frame_fine` performs center-pivoted bilinear rotation resampling to level the page content.

4. **Adaptive Discrete Rotation (0°, 90°, 180°, 270°) (`SCAN-004`):**
   - `rotate_frame_90`: Exact 90° clockwise step rotations swapping buffer strides and coordinates without interpolation blur.
   - `auto_orient_frame`: Evaluates device orientation flags and landscape-vs-portrait aspect ratios to enforce upright portrait orientation by default.

5. **Session Ingestion & Warped Frame Caching (`SCAN-006`):**
   - In `ScanCaptureSession::append_page`, newly ingested camera frames are automatically warped using resolved quad corners and cached in `CapturedScanPage.warped_frame`.
   - On-demand retrieval is exposed via `ScanCaptureSessionRegistry::get_warped_page`, avoiding large buffer duplication in session snapshot listings.

6. **UniFFI Exposure & Android Native Integration:**
   - Exposed C/UniFFI boundary functions in `platform/vocument-ffi/src/scan_dto.rs` (409 lines):
     - `document_core_warp_perspective`
     - `document_core_deskew_frame`
     - `document_core_rotate_frame`
     - `document_core_auto_orient_frame`
     - `document_core_get_warped_scan_page`
   - Generated Kotlin bindings and wired helpers into `DocumentCoreBridge.kt` (539 lines).
   - Integrated into `ScanCaptureScreen.kt` and verified in `ScanCaptureBridgeTest.kt`.

---

## Consequences

- **Rectified Pipeline Ready:** Captured scans are automatically transformed into planar rectangular bitmaps upon capture, completely neutralizing camera tilt and trapezoidal skew before cleanup.
- **Strict Architecture Boundaries:** All geometric homography and rotational pixel transformations run entirely in the shared Rust `vocument-imaging` core; Android and macOS UI layers remain pure presentation wrappers.
- **Zero Regressions:** All unit tests pass across `vocument-imaging`, `vocument-ffi`, and Android Gradle `testDebugUnitTest`. Authentic Android 14 emulator testing confirms live session creation and frame rectification.
- **Direct Path to M1.2:** With rectified scan images available in the session registry, M1.2 can apply contrast enhancement, binarization, and background whitening directly to planar page buffers.
