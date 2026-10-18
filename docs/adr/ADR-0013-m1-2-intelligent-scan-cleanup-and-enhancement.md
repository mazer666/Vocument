# ADR-0013: M1.2 Intelligent Scan Cleanup and Enhancement

- **Status:** Accepted
- **Date:** 2026-10-18
- **Milestone:** M1.2 — Intelligent Scan Cleanup & Enhancement (`ROADMAP.md` M1.2, `ARCHITECTURE.md` §21, `REQUIREMENTS.md` §8)
- **Requirements Addressed:** `SCAN-004`, `SCAN-005`, `MOBILE-001`, `CODE-001`

---

## Context

Camera-captured document frames inherently suffer from complex real-world illumination flaws:
1. **Uneven Illumination Gradients & Cast Shadows:** Handheld capture frequently casts dark gradients from the smartphone body, user hands, or angled ambient room lighting.
2. **Creases and Fold Marks:** Physical documents often feature sharp dark crease lines and fold shadows that interfere with visual legibility and degrade OCR recognition accuracy.
3. **Paper Tint and Low Contrast:** Non-white paper or indoor lighting produces gray or yellow backgrounds and muddy text with low dynamic range.

While submilestone `M1.1` successfully rectified camera frames into upright rectangular planar coordinates, downstream offline Tesseract OCR (`M1.3`) and PDF synthesis (`M1.4`) require clean, high-contrast text with normalized paper backgrounds.

`REQUIREMENTS.md` §8 (`SCAN-004`) mandates:
> *"Automatic detection, rotation, deskew, shadow reduction, contrast/colour/white balance, noise reduction, OCR and sensible compression."*

---

## Decision

1. **Modular Architecture & Strict `CODE-001` Compliance:**
   - All scan cleanup and enhancement algorithms are isolated in a dedicated module:
     - `engines/imaging/src/cleanup.rs` (581 lines): Local background estimation, homomorphic division, cleanup profile processing, and visual regression quality metrics.
   - All source files remain strictly under the 600-line budget (`CODE-001`):
     - `engines/imaging/src/geometry.rs` (155 lines)
     - `engines/imaging/src/perspective.rs` (519 lines)
     - `engines/imaging/src/lib.rs` (545 lines)
     - `engines/imaging/src/cleanup.rs` (581 lines)
     - `platform/vocument-ffi/src/scan_dto.rs` (479 lines)
     - `apps/android/app/src/main/java/org/vocument/app/DocumentCoreBridge.kt` (564 lines)
     - `apps/android/app/src/main/java/org/vocument/app/ScanCaptureScreen.kt` (526 lines)

2. **Adaptive 2D Block-Based Background Illumination Modeling:**
   - To estimate the low-frequency paper illumination surface $B(x, y)$ while ignoring ink text and fold lines:
     - The luminance plane is divided into a spatial grid of blocks ($16 \times 16$ to $64 \times 64$ pixels).
     - For each block, the 90th percentile luminance value $P_{90}$ is extracted, isolating the paper background from dark ink characters.
     - A $3 \times 3$ smoothing box filter eliminates block boundary steps.
     - Bilinear interpolation maps the smoothed block values to full pixel resolution, yielding a continuous, smooth illumination surface $B(x, y) \in [1.0, 255.0]$.

3. **Shadow and Crease Reduction via Homomorphic Division:**
   - Paper background normalization is achieved via division:
     $$L_{\text{norm}}(x, y) = \frac{L(x, y)}{B(x, y)} \times 255.0$$
   - Since $B(x, y)$ tracks the local cast shadows and creases, dividing by $B(x, y)$ cancels out the lighting gradient across the page and attenuates crease shadows by $> 80\%$.

4. **Three Dedicated Cleanup Profiles:**
   - **`DocumentAuto` (Intelligent Adaptive Clean Color/Gray):**
     - Whitens paper background ($L_{\text{norm}} \ge 215 \to 255$).
     - Stretches dark ink downward ($L_{\text{norm}} \le 80 \to \text{darken}$).
     - Applies a smooth power-law S-curve ($t^{1.25}$) between $[80, 215]$.
     - For color frames (`Rgba8`/`Rgb8`), scales RGB channels proportionally, preserving ink colors (blue pens, red stamps) on clean pure-white paper without color cast.
   - **`HighContrastBw` (Adaptive Bradley/Sauvola Threshold Binarization):**
     - Dynamically thresholds pixels at $18\%$ below the local paper background ($T(x, y) = B(x, y) \times 0.82$).
     - Produces clean binary black-and-white output ($\{0, 255\}$), eliminating faint bleed-through, stains, and wrinkles for maximum OCR accuracy.
   - **`PreservePhoto` (Continuous-Tone Dynamic Range Preservation):**
     - Softly flattens illumination ($\gamma = 0.60$) without thresholding or tone clipping.
     - Preserves photographic gradients, color fidelity, and smooth continuous skin tones across certificates, IDs, and mixed documents.

5. **Quantitative Visual Regression Metrics:**
   - Implemented `CleanupQualityMetrics` and `evaluate_cleanup_quality`:
     - `background_uniformity`: $1.0 - (\sigma_{\text{bg}} / \mu_{\text{bg}})$ (target: $\ge 0.90$).
     - `contrast_ratio`: $(\mu_{\text{bg}} - \mu_{\text{ink}}) / \mu_{\text{bg}}$ (target: $\ge 0.80$).
     - `shadow_attenuation`: $1.0 - (\Delta B_{\text{out}} / \Delta B_{\text{in}})$ (target: $\ge 0.80$).
     - `is_binarized`: asserts strict binary output for `HighContrastBw` and non-binarized output for `DocumentAuto` / `PreservePhoto`.

6. **M1.1 Gap Resolutions:**
   - **Interactive Draggable Corner Handles:** Implemented `pointerInput` and `detectDragGestures` in Jetpack Compose `ViewfinderOverlay` with touch hit targets, active glowing indicators (24dp), and manual adjustment mode locking with "Reset Auto Corners".
   - **Binding Android Runtime Gate:** Removed all silent skip fallbacks in `scripts/verify-android-runtime.sh`. The gate strictly requires `adb`, an active target device/emulator, and verified UI hierarchy dump confirmation of page capture, failing with exit code 1 if unavailable.
   - **Status Synchronization:** Project status updated to `m1-in-progress` in `project/project.yaml`.

---

## Consequences

- **High-Fidelity Input for OCR (`M1.3`):** Cleaned, rectified pages with high contrast and pure-white backgrounds dramatically improve Tesseract text segmentation and character recognition.
- **Strict Architecture Boundaries:** All image enhancement, filtering, and metric evaluations run in the shared Rust `vocument-imaging` core. Platform bridges (`DocumentCoreBridge.kt` and `VocumentMacApp`) remain thin UniFFI clients.
- **Verified Offline Quality:** Unit tests across `vocument-imaging`, `vocument-ffi`, and Android `ScanCaptureBridgeTest.kt` prove quantitative cleanup quality metrics and profile adherence without external dependencies.
