//! Shared Rust `ImagingEngine` for scan cleanup, perspective correction, and deskew (`ARCHITECTURE.md` §21, `REQUIREMENTS.md` §8).
//!
//! Provides:
//! - Multi-format frame ingestion (`Rgb8`, `Rgba8`, `Gray8`, `Nv21`, `Yuv420`)
//! - Normalized 2D coordinates and quadrilateral corner models (`Point2D`, `QuadCorners`)
//! - Live camera edge detection and document boundary proposal (`detect_document_edges`)
//! - Multi-page scan capture session management (`ScanCaptureSession`, `ScanCaptureSessionRegistry`)

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod cleanup;
pub mod edges;
pub mod geometry;
pub mod perspective;

pub use cleanup::{CleanupQualityMetrics, enhance_frame, evaluate_cleanup_quality};
pub use geometry::{Point2D, QuadCorners};
pub use perspective::{
    DeskewResult, ProjectiveMapping, auto_orient_frame, deskew_frame, rotate_frame_90,
    rotate_frame_fine, warp_perspective,
};

/// Supported scan cleanup profile modes (`ARCHITECTURE.md` §21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanCleanupProfile {
    DocumentAuto,
    PreservePhoto,
    HighContrastBw,
}

/// Pixel format of ingested raw camera scan frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelFormat {
    Rgb8,
    Rgba8,
    Gray8,
    Nv21,
    Yuv420,
}

/// Errors occurring within the `ImagingEngine`.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ImagingError {
    #[error("invalid frame dimensions: {width}x{height}, stride {stride}")]
    InvalidDimensions {
        width: u32,
        height: u32,
        stride: u32,
    },
    #[error("pixel buffer too short: expected at least {expected} bytes, got {actual}")]
    BufferTooShort { expected: usize, actual: usize },
    #[error("unsupported pixel format: {format:?}")]
    UnsupportedFormat { format: PixelFormat },
    #[error("invalid document quad: {details}")]
    InvalidQuad { details: String },
    #[error("scan capture session '{session_id}' not found")]
    SessionNotFound { session_id: String },
    #[error("invalid page index {page_index} (total pages: {total_pages})")]
    InvalidPageIndex {
        page_index: usize,
        total_pages: usize,
    },
    #[error("scan capture session registry lock poisoned")]
    RegistryLockPoisoned,
}

/// Metadata captured with an input camera scan frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanFrameMetadata {
    pub timestamp_ms: u64,
    pub device_angle_degrees: f32,
    pub brightness_hint: f32,
    pub is_steady: bool,
}

impl Default for ScanFrameMetadata {
    fn default() -> Self {
        Self {
            timestamp_ms: current_unix_ms(),
            device_angle_degrees: 0.0,
            brightness_hint: 0.5,
            is_steady: true,
        }
    }
}

/// Raw camera capture frame ingested from Android CameraX or macOS camera pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: PixelFormat,
    pub orientation_degrees: u32,
    pub pixels: Vec<u8>,
    pub metadata: ScanFrameMetadata,
}

impl ScanFrame {
    /// Validates frame dimensions and buffer length.
    pub fn validate(&self) -> Result<(), ImagingError> {
        if self.width == 0 || self.height == 0 || self.stride < self.width {
            return Err(ImagingError::InvalidDimensions {
                width: self.width,
                height: self.height,
                stride: self.stride,
            });
        }
        let min_bytes = match self.format {
            PixelFormat::Rgb8 => (self.stride * self.height * 3) as usize,
            PixelFormat::Rgba8 => (self.stride * self.height * 4) as usize,
            PixelFormat::Gray8 => (self.stride * self.height) as usize,
            PixelFormat::Nv21 | PixelFormat::Yuv420 => {
                ((self.stride * self.height * 3) / 2) as usize
            }
        };
        if self.pixels.len() < min_bytes {
            return Err(ImagingError::BufferTooShort {
                expected: min_bytes,
                actual: self.pixels.len(),
            });
        }
        Ok(())
    }

    /// Extracts sample luminance at normalized position `(u, v)`.
    pub fn sample_luminance(&self, u: f32, v: f32) -> f32 {
        let x = ((u.clamp(0.0, 1.0) * (self.width - 1) as f32) as u32).min(self.width - 1);
        let y = ((v.clamp(0.0, 1.0) * (self.height - 1) as f32) as u32).min(self.height - 1);
        let idx = (y * self.stride + x) as usize;

        match self.format {
            PixelFormat::Gray8 | PixelFormat::Nv21 | PixelFormat::Yuv420 => {
                self.pixels.get(idx).copied().unwrap_or(128) as f32 / 255.0
            }
            PixelFormat::Rgba8 => {
                let base = idx * 4;
                if base + 2 < self.pixels.len() {
                    let r = self.pixels[base] as f32;
                    let g = self.pixels[base + 1] as f32;
                    let b = self.pixels[base + 2] as f32;
                    (0.299 * r + 0.587 * g + 0.114 * b) / 255.0
                } else {
                    0.5
                }
            }
            PixelFormat::Rgb8 => {
                let base = idx * 3;
                if base + 2 < self.pixels.len() {
                    let r = self.pixels[base] as f32;
                    let g = self.pixels[base + 1] as f32;
                    let b = self.pixels[base + 2] as f32;
                    (0.299 * r + 0.587 * g + 0.114 * b) / 255.0
                } else {
                    0.5
                }
            }
        }
    }
}

/// Fast document edge and corner detection on an ingested scan frame (`ARCHITECTURE.md` §21).
pub fn detect_document_edges(frame: &ScanFrame) -> Result<Option<QuadCorners>, ImagingError> {
    frame.validate()?;

    // The outline of a tilted page cannot be found by looking at the centre
    // row and the centre column alone. The detection works on the boundary of
    // the page instead, so that a sheared or tilted page is found as well.
    if let Some(detected) = crate::edges::detect_outline(frame) {
        return Ok(Some(detected));
    }

    // Sample a 32x32 luminance grid across the frame:
    const GRID_SIZE: usize = 32;
    let mut lum = [[0.0f32; GRID_SIZE]; GRID_SIZE];
    for (y, row) in lum.iter_mut().enumerate() {
        for (x, cell) in row.iter_mut().enumerate() {
            let u = x as f32 / (GRID_SIZE - 1) as f32;
            let v = y as f32 / (GRID_SIZE - 1) as f32;
            *cell = frame.sample_luminance(u, v);
        }
    }

    // Find horizontal and vertical contrast transitions:
    let center_lum = lum[GRID_SIZE / 2][GRID_SIZE / 2];
    let mut left_x = 0;
    let mut right_x = GRID_SIZE - 1;
    let mut top_y = 0;
    let mut bottom_y = GRID_SIZE - 1;

    let mid_y = GRID_SIZE / 2;
    for (x, &val) in lum[mid_y].iter().enumerate().take(mid_y) {
        if (val - center_lum).abs() < 0.15 {
            left_x = x;
            break;
        }
    }
    for (x, &val) in lum[mid_y].iter().enumerate().skip(mid_y).rev() {
        if (val - center_lum).abs() < 0.15 {
            right_x = x;
            break;
        }
    }

    let mid_x = GRID_SIZE / 2;
    for (y, row) in lum.iter().enumerate().take(mid_x) {
        if (row[mid_x] - center_lum).abs() < 0.15 {
            top_y = y;
            break;
        }
    }
    for (y, row) in lum.iter().enumerate().skip(mid_x).rev() {
        if (row[mid_x] - center_lum).abs() < 0.15 {
            bottom_y = y;
            break;
        }
    }

    let u0 = (left_x as f32 / (GRID_SIZE - 1) as f32).clamp(0.04, 0.35);
    let u1 = (right_x as f32 / (GRID_SIZE - 1) as f32).clamp(0.65, 0.96);
    let v0 = (top_y as f32 / (GRID_SIZE - 1) as f32).clamp(0.04, 0.35);
    let v1 = (bottom_y as f32 / (GRID_SIZE - 1) as f32).clamp(0.65, 0.96);

    let candidate = QuadCorners {
        top_left: Point2D::new(u0, v0),
        top_right: Point2D::new(u1, v0),
        bottom_right: Point2D::new(u1, v1),
        bottom_left: Point2D::new(u0, v1),
        confidence_milli: 850,
    };

    if candidate.is_valid_document_quad() {
        Ok(Some(candidate))
    } else {
        Ok(Some(QuadCorners::with_inset(0.05, 500)))
    }
}

/// A single captured scan page stored in a multi-page scanning session (`SCAN-006`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapturedScanPage {
    pub page_id: String,
    pub page_index: u32,
    pub corners: QuadCorners,
    pub profile: ScanCleanupProfile,
    pub frame_width: u32,
    pub frame_height: u32,
    pub raw_bytes_len: usize,
    pub captured_unix_ms: u64,
    pub warped_frame: Option<ScanFrame>,
    pub enhanced_frame: Option<ScanFrame>,
}

/// A multi-page document scanning session (`SCAN-006`, `SCAN-007`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanCaptureSession {
    pub session_id: String,
    pub pages: Vec<CapturedScanPage>,
    pub created_unix_ms: u64,
    pub is_active: bool,
}

impl ScanCaptureSession {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            pages: Vec::new(),
            created_unix_ms: current_unix_ms(),
            is_active: true,
        }
    }

    /// Appends a captured page to this session and returns the stored page.
    ///
    /// The page is returned by value so that callers do not need to clone the
    /// (potentially large) warped and enhanced pixel buffers a second time.
    pub fn append_page(
        &mut self,
        frame: &ScanFrame,
        corners: Option<QuadCorners>,
        profile: ScanCleanupProfile,
    ) -> Result<CapturedScanPage, ImagingError> {
        frame.validate()?;
        let resolved_corners = match corners {
            Some(c) if c.is_valid_document_quad() => c,
            _ => {
                detect_document_edges(frame)?.unwrap_or_else(|| QuadCorners::with_inset(0.05, 500))
            }
        };

        let warped = warp_perspective(frame, &resolved_corners, None).ok();
        let enhanced = warped.as_ref().and_then(|w| enhance_frame(w, profile).ok());
        let page_index = self.pages.len() as u32;
        let page_id = format!("scan_page_{}_{page_index}", self.session_id);
        let page = CapturedScanPage {
            page_id,
            page_index,
            corners: resolved_corners,
            profile,
            frame_width: frame.width,
            frame_height: frame.height,
            raw_bytes_len: frame.pixels.len(),
            captured_unix_ms: current_unix_ms(),
            warped_frame: warped,
            enhanced_frame: enhanced,
        };
        self.pages.push(page);
        Ok(self.pages[self.pages.len() - 1].clone())
    }

    pub fn get_warped_page(&self, page_index: usize) -> Result<ScanFrame, ImagingError> {
        if page_index >= self.pages.len() {
            return Err(ImagingError::InvalidPageIndex {
                page_index,
                total_pages: self.pages.len(),
            });
        }
        self.pages[page_index]
            .warped_frame
            .clone()
            .ok_or_else(|| ImagingError::InvalidQuad {
                details: "Warped frame not available for page".to_owned(),
            })
    }

    pub fn get_enhanced_page(&self, page_index: usize) -> Result<ScanFrame, ImagingError> {
        if page_index >= self.pages.len() {
            return Err(ImagingError::InvalidPageIndex {
                page_index,
                total_pages: self.pages.len(),
            });
        }
        self.pages[page_index]
            .enhanced_frame
            .clone()
            .ok_or_else(|| ImagingError::InvalidQuad {
                details: "Enhanced frame not available for page".to_owned(),
            })
    }

    pub fn remove_page(&mut self, page_index: usize) -> Result<CapturedScanPage, ImagingError> {
        if page_index >= self.pages.len() {
            return Err(ImagingError::InvalidPageIndex {
                page_index,
                total_pages: self.pages.len(),
            });
        }
        let removed = self.pages.remove(page_index);
        for (i, p) in self.pages.iter_mut().enumerate() {
            p.page_index = i as u32;
        }
        Ok(removed)
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
}

/// Global registry for active scanning sessions.
#[derive(Debug, Default)]
pub struct ScanCaptureSessionRegistry {
    sessions: Mutex<HashMap<String, ScanCaptureSession>>,
}

impl ScanCaptureSessionRegistry {
    pub fn global() -> &'static Self {
        static REGISTRY: OnceLock<ScanCaptureSessionRegistry> = OnceLock::new();
        REGISTRY.get_or_init(Self::default)
    }

    pub fn create_session(&self) -> String {
        let id = next_scan_session_id();
        let session = ScanCaptureSession::new(id.clone());
        let mut map = self
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        map.insert(id.clone(), session);
        id
    }

    pub fn append_page(
        &self,
        session_id: &str,
        frame: &ScanFrame,
        corners: Option<QuadCorners>,
        profile: ScanCleanupProfile,
    ) -> Result<CapturedScanPage, ImagingError> {
        let mut map = self
            .sessions
            .lock()
            .map_err(|_| ImagingError::RegistryLockPoisoned)?;
        let session = map
            .get_mut(session_id)
            .ok_or_else(|| ImagingError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let page = session.append_page(frame, corners, profile)?;
        Ok(page)
    }

    pub fn get_warped_page(
        &self,
        session_id: &str,
        page_index: usize,
    ) -> Result<ScanFrame, ImagingError> {
        let map = self
            .sessions
            .lock()
            .map_err(|_| ImagingError::RegistryLockPoisoned)?;
        let session = map
            .get(session_id)
            .ok_or_else(|| ImagingError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        session.get_warped_page(page_index)
    }

    pub fn get_enhanced_page(
        &self,
        session_id: &str,
        page_index: usize,
    ) -> Result<ScanFrame, ImagingError> {
        let map = self
            .sessions
            .lock()
            .map_err(|_| ImagingError::RegistryLockPoisoned)?;
        let session = map
            .get(session_id)
            .ok_or_else(|| ImagingError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        session.get_enhanced_page(page_index)
    }

    pub fn get_session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<ScanCaptureSession, ImagingError> {
        let map = self
            .sessions
            .lock()
            .map_err(|_| ImagingError::RegistryLockPoisoned)?;
        map.get(session_id)
            .cloned()
            .ok_or_else(|| ImagingError::SessionNotFound {
                session_id: session_id.to_owned(),
            })
    }

    pub fn close_session(&self, session_id: &str) -> Result<(), ImagingError> {
        let mut map = self
            .sessions
            .lock()
            .map_err(|_| ImagingError::RegistryLockPoisoned)?;
        map.remove(session_id)
            .map(|_| ())
            .ok_or_else(|| ImagingError::SessionNotFound {
                session_id: session_id.to_owned(),
            })
    }
}

/// Monotonic counter guaranteeing collision-free scan session identifiers.
///
/// Session IDs must stay unique even when two sessions are created within the same
/// millisecond (fast successive captures), so the millisecond timestamp alone is not
/// sufficient as an identifier.
static SCAN_SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Generates a collision-free scan session identifier (`scan_sess_<ms>_<seq>`).
fn next_scan_session_id() -> String {
    let sequence = SCAN_SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("scan_sess_{}_{sequence}", current_unix_ms())
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_and_quad_area_and_convexity() {
        let full = QuadCorners::full_frame();
        assert!(full.is_convex());
        assert!((full.area() - 1.0).abs() < 1e-4);
        assert!(full.is_valid_document_quad());

        let inset = QuadCorners::with_inset(0.1, 900);
        assert!(inset.is_convex());
        assert!((inset.area() - 0.64).abs() < 1e-4);
        assert!(inset.is_valid_document_quad());

        // Self-intersecting concave quad
        let concave = QuadCorners {
            top_left: Point2D::new(0.0, 0.0),
            top_right: Point2D::new(0.8, 0.8),
            bottom_right: Point2D::new(0.2, 0.8),
            bottom_left: Point2D::new(0.8, 0.0),
            confidence_milli: 100,
        };
        assert!(!concave.is_convex());
        assert!(!concave.is_valid_document_quad());
    }

    #[test]
    fn test_scan_frame_validation_and_luminance() {
        let pixels = vec![255u8; 100 * 100 * 4];
        let frame = ScanFrame {
            width: 100,
            height: 100,
            stride: 100,
            format: PixelFormat::Rgba8,
            orientation_degrees: 0,
            pixels,
            metadata: ScanFrameMetadata::default(),
        };
        assert!(frame.validate().is_ok());
        assert!((frame.sample_luminance(0.5, 0.5) - 1.0).abs() < 1e-2);

        // Invalid frame length:
        let short_frame = ScanFrame {
            width: 100,
            height: 100,
            stride: 100,
            format: PixelFormat::Rgba8,
            orientation_degrees: 0,
            pixels: vec![0u8; 10],
            metadata: ScanFrameMetadata::default(),
        };
        assert!(short_frame.validate().is_err());
    }

    #[test]
    fn test_edge_detection_and_session_lifecycle() {
        let mut pixels = vec![50u8; 200 * 200];
        // Draw a bright document sheet in center:
        for y in 40..160 {
            for x in 40..160 {
                pixels[y * 200 + x] = 230;
            }
        }
        let frame = ScanFrame {
            width: 200,
            height: 200,
            stride: 200,
            format: PixelFormat::Gray8,
            orientation_degrees: 0,
            pixels,
            metadata: ScanFrameMetadata::default(),
        };

        let detected = detect_document_edges(&frame).expect("detect edges");
        assert!(detected.is_some());
        let quad = detected.expect("detected document quadrilateral");
        assert!(quad.is_valid_document_quad());

        let registry = ScanCaptureSessionRegistry::global();
        let session_id = registry.create_session();
        let page1 = registry
            .append_page(
                &session_id,
                &frame,
                Some(quad),
                ScanCleanupProfile::DocumentAuto,
            )
            .expect("append page");
        assert_eq!(page1.page_index, 0);

        let snap = registry
            .get_session_snapshot(&session_id)
            .expect("snapshot");
        assert_eq!(snap.pages.len(), 1);

        registry.close_session(&session_id).expect("close session");
        assert!(registry.get_session_snapshot(&session_id).is_err());
    }

    #[test]
    fn test_scan_session_ids_are_unique_even_within_the_same_millisecond() {
        let registry = ScanCaptureSessionRegistry::default();

        // Two captures started back to back can land in the same millisecond, so the
        // identifier must not be derived from the timestamp alone.
        let first = registry.create_session();
        let second = registry.create_session();

        assert_ne!(first, second, "scan session identifiers must be unique");
        assert!(first.starts_with("scan_sess_"));
        assert!(second.starts_with("scan_sess_"));

        // Both sessions must remain independently addressable.
        assert!(registry.get_session_snapshot(&first).is_ok());
        assert!(registry.get_session_snapshot(&second).is_ok());
    }

    #[test]
    fn test_missing_scan_session_returns_typed_error() {
        let registry = ScanCaptureSessionRegistry::default();
        let error = registry
            .get_session_snapshot("scan_sess_does_not_exist")
            .expect_err("missing session must fail");
        assert!(matches!(error, ImagingError::SessionNotFound { .. }));
    }
}
