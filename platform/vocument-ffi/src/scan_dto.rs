use vocument_imaging::{
    CapturedScanPage, DeskewResult, PixelFormat, Point2D, QuadCorners, ScanCaptureSession,
    ScanCaptureSessionRegistry, ScanCleanupProfile, ScanFrame, ScanFrameMetadata,
    auto_orient_frame, deskew_frame, detect_document_edges, rotate_frame_90, warp_perspective,
};

use crate::DocumentCoreError;

/// Pixel format of ingested raw camera scan frames crossing UniFFI boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiScanPixelFormat {
    Rgb8,
    Rgba8,
    Gray8,
    Nv21,
    Yuv420,
}

impl From<FfiScanPixelFormat> for PixelFormat {
    fn from(fmt: FfiScanPixelFormat) -> Self {
        match fmt {
            FfiScanPixelFormat::Rgb8 => Self::Rgb8,
            FfiScanPixelFormat::Rgba8 => Self::Rgba8,
            FfiScanPixelFormat::Gray8 => Self::Gray8,
            FfiScanPixelFormat::Nv21 => Self::Nv21,
            FfiScanPixelFormat::Yuv420 => Self::Yuv420,
        }
    }
}

impl From<PixelFormat> for FfiScanPixelFormat {
    fn from(fmt: PixelFormat) -> Self {
        match fmt {
            PixelFormat::Rgb8 => Self::Rgb8,
            PixelFormat::Rgba8 => Self::Rgba8,
            PixelFormat::Gray8 => Self::Gray8,
            PixelFormat::Nv21 => Self::Nv21,
            PixelFormat::Yuv420 => Self::Yuv420,
        }
    }
}

/// Cleanup and enhancement profile for captured scan pages (`SCAN-002`, `SCAN-003`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiScanCleanupProfile {
    DocumentAuto,
    PreservePhoto,
    HighContrastBw,
}

impl From<FfiScanCleanupProfile> for ScanCleanupProfile {
    fn from(p: FfiScanCleanupProfile) -> Self {
        match p {
            FfiScanCleanupProfile::DocumentAuto => Self::DocumentAuto,
            FfiScanCleanupProfile::PreservePhoto => Self::PreservePhoto,
            FfiScanCleanupProfile::HighContrastBw => Self::HighContrastBw,
        }
    }
}

impl From<ScanCleanupProfile> for FfiScanCleanupProfile {
    fn from(p: ScanCleanupProfile) -> Self {
        match p {
            ScanCleanupProfile::DocumentAuto => Self::DocumentAuto,
            ScanCleanupProfile::PreservePhoto => Self::PreservePhoto,
            ScanCleanupProfile::HighContrastBw => Self::HighContrastBw,
        }
    }
}

/// 2D point in normalized coordinates `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct FfiPoint2D {
    pub x: f32,
    pub y: f32,
}

impl From<Point2D> for FfiPoint2D {
    fn from(p: Point2D) -> Self {
        Self { x: p.x, y: p.y }
    }
}

impl From<FfiPoint2D> for Point2D {
    fn from(p: FfiPoint2D) -> Self {
        Self::new(p.x, p.y)
    }
}

/// Quadrilateral corner coordinates defining a document outline in normalized coordinates `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct FfiQuadCorners {
    pub top_left: FfiPoint2D,
    pub top_right: FfiPoint2D,
    pub bottom_right: FfiPoint2D,
    pub bottom_left: FfiPoint2D,
    pub confidence_milli: u32,
}

impl From<QuadCorners> for FfiQuadCorners {
    fn from(q: QuadCorners) -> Self {
        Self {
            top_left: q.top_left.into(),
            top_right: q.top_right.into(),
            bottom_right: q.bottom_right.into(),
            bottom_left: q.bottom_left.into(),
            confidence_milli: q.confidence_milli,
        }
    }
}

impl From<FfiQuadCorners> for QuadCorners {
    fn from(q: FfiQuadCorners) -> Self {
        Self {
            top_left: q.top_left.into(),
            top_right: q.top_right.into(),
            bottom_right: q.bottom_right.into(),
            bottom_left: q.bottom_left.into(),
            confidence_milli: q.confidence_milli,
        }
    }
}

/// Raw camera scan frame crossing UniFFI boundary.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiScanFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: FfiScanPixelFormat,
    pub orientation_degrees: u32,
    pub pixels: Vec<u8>,
    pub timestamp_ms: u64,
    pub device_angle_degrees: f32,
    pub brightness_hint: f32,
    pub is_steady: bool,
}

impl From<FfiScanFrame> for ScanFrame {
    fn from(f: FfiScanFrame) -> Self {
        Self {
            width: f.width,
            height: f.height,
            stride: f.stride,
            format: f.format.into(),
            orientation_degrees: f.orientation_degrees,
            pixels: f.pixels,
            metadata: ScanFrameMetadata {
                timestamp_ms: f.timestamp_ms,
                device_angle_degrees: f.device_angle_degrees,
                brightness_hint: f.brightness_hint,
                is_steady: f.is_steady,
            },
        }
    }
}

impl From<ScanFrame> for FfiScanFrame {
    fn from(f: ScanFrame) -> Self {
        Self {
            width: f.width,
            height: f.height,
            stride: f.stride,
            format: f.format.into(),
            orientation_degrees: f.orientation_degrees,
            pixels: f.pixels,
            timestamp_ms: f.metadata.timestamp_ms,
            device_angle_degrees: f.metadata.device_angle_degrees,
            brightness_hint: f.metadata.brightness_hint,
            is_steady: f.metadata.is_steady,
        }
    }
}

/// A single captured scan page stored in a scanning session.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiCapturedScanPage {
    pub page_id: String,
    pub page_index: u32,
    pub corners: FfiQuadCorners,
    pub frame_width: u32,
    pub frame_height: u32,
    pub raw_bytes_len: u64,
    pub captured_unix_ms: u64,
}

impl From<CapturedScanPage> for FfiCapturedScanPage {
    fn from(p: CapturedScanPage) -> Self {
        Self {
            page_id: p.page_id,
            page_index: p.page_index,
            corners: p.corners.into(),
            frame_width: p.frame_width,
            frame_height: p.frame_height,
            raw_bytes_len: p.raw_bytes_len as u64,
            captured_unix_ms: p.captured_unix_ms,
        }
    }
}

/// Snapshot of an active scanning session.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiScanSessionSnapshot {
    pub session_id: String,
    pub page_count: u32,
    pub pages: Vec<FfiCapturedScanPage>,
    pub is_active: bool,
}

impl From<ScanCaptureSession> for FfiScanSessionSnapshot {
    fn from(s: ScanCaptureSession) -> Self {
        let pages: Vec<FfiCapturedScanPage> = s.pages.into_iter().map(Into::into).collect();
        let page_count = pages.len() as u32;
        Self {
            session_id: s.session_id,
            page_count,
            pages,
            is_active: s.is_active,
        }
    }
}

/// Detects document edges and corner coordinates in real time from a live camera frame (`SCAN-001`, `SCAN-004`).
#[uniffi::export]
pub fn document_core_detect_document_edges(
    frame: FfiScanFrame,
) -> Result<Option<FfiQuadCorners>, DocumentCoreError> {
    let internal_frame: ScanFrame = frame.into();
    let detected = detect_document_edges(&internal_frame)?;
    Ok(detected.map(Into::into))
}

/// Starts a new multi-page scan capture session (`SCAN-006`).
#[uniffi::export]
pub fn document_core_start_scan_session() -> String {
    ScanCaptureSessionRegistry::global().create_session()
}

/// Appends a captured camera scan frame to an active scanning session (`SCAN-006`).
#[uniffi::export]
pub fn document_core_append_scan_page(
    session_id: String,
    frame: FfiScanFrame,
    corners: Option<FfiQuadCorners>,
    cleanup_profile: Option<FfiScanCleanupProfile>,
) -> Result<FfiCapturedScanPage, DocumentCoreError> {
    let internal_frame: ScanFrame = frame.into();
    let internal_corners = corners.map(Into::into);
    let internal_profile = cleanup_profile
        .map(Into::into)
        .unwrap_or(ScanCleanupProfile::DocumentAuto);
    let page = ScanCaptureSessionRegistry::global().append_page(
        &session_id,
        &internal_frame,
        internal_corners,
        internal_profile,
    )?;
    Ok(page.into())
}

/// Queries the current state of a scanning session.
#[uniffi::export]
pub fn document_core_get_scan_session(
    session_id: String,
) -> Result<FfiScanSessionSnapshot, DocumentCoreError> {
    let snap = ScanCaptureSessionRegistry::global().get_session_snapshot(&session_id)?;
    Ok(snap.into())
}

/// Closes and releases an active scanning session.
#[uniffi::export]
pub fn document_core_close_scan_session(session_id: String) -> Result<(), DocumentCoreError> {
    ScanCaptureSessionRegistry::global().close_session(&session_id)?;
    Ok(())
}

/// Result of deskewing and orientation correction crossing UniFFI boundary.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiDeskewResult {
    pub frame: FfiScanFrame,
    pub skew_angle_degrees: f32,
    pub rotation_degrees: u32,
}

impl From<DeskewResult> for FfiDeskewResult {
    fn from(r: DeskewResult) -> Self {
        Self {
            frame: r.frame.into(),
            skew_angle_degrees: r.skew_angle_degrees,
            rotation_degrees: r.rotation_degrees,
        }
    }
}

/// Warps a convex document quadrilateral from an input camera frame into a rectified rectangular frame (`SCAN-003`).
#[uniffi::export]
pub fn document_core_warp_perspective(
    frame: FfiScanFrame,
    corners: FfiQuadCorners,
    target_aspect_ratio: Option<f32>,
) -> Result<FfiScanFrame, DocumentCoreError> {
    let internal_frame: ScanFrame = frame.into();
    let internal_corners: QuadCorners = corners.into();
    let warped = warp_perspective(&internal_frame, &internal_corners, target_aspect_ratio)?;
    Ok(warped.into())
}

/// Estimates skew angle and straightens document content baselines (`SCAN-004`, `SCAN-005`).
#[uniffi::export]
pub fn document_core_deskew_frame(
    frame: FfiScanFrame,
) -> Result<FfiDeskewResult, DocumentCoreError> {
    let internal_frame: ScanFrame = frame.into();
    let res = deskew_frame(&internal_frame)?;
    Ok(res.into())
}

/// Rotates a scan frame by multiples of 90 degrees clockwise (0, 90, 180, 270) (`SCAN-004`).
#[uniffi::export]
pub fn document_core_rotate_frame(
    frame: FfiScanFrame,
    clockwise_steps: u32,
) -> Result<FfiScanFrame, DocumentCoreError> {
    let internal_frame: ScanFrame = frame.into();
    let rotated = rotate_frame_90(&internal_frame, clockwise_steps)?;
    Ok(rotated.into())
}

/// Automatically aligns a scan frame into upright portrait based on aspect ratio and orientation hints (`SCAN-004`).
#[uniffi::export]
pub fn document_core_auto_orient_frame(
    frame: FfiScanFrame,
    prefer_portrait: bool,
) -> Result<FfiScanFrame, DocumentCoreError> {
    let internal_frame: ScanFrame = frame.into();
    let oriented = auto_orient_frame(&internal_frame, prefer_portrait)?;
    Ok(oriented.into())
}

/// Retrieves the rectified warped scan page image from an active scanning session (`SCAN-003`, `SCAN-006`).
#[uniffi::export]
pub fn document_core_get_warped_scan_page(
    session_id: String,
    page_index: u32,
) -> Result<FfiScanFrame, DocumentCoreError> {
    let frame =
        ScanCaptureSessionRegistry::global().get_warped_page(&session_id, page_index as usize)?;
    Ok(frame.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_scan_dto_conversions_and_edge_detection() {
        let mut pixels = vec![30u8; 100 * 100 * 4];
        for y in 20..80 {
            for x in 20..80 {
                let idx = (y * 100 + x) * 4;
                pixels[idx] = 240;
                pixels[idx + 1] = 240;
                pixels[idx + 2] = 240;
                pixels[idx + 3] = 255;
            }
        }
        let frame = FfiScanFrame {
            width: 100,
            height: 100,
            stride: 100,
            format: FfiScanPixelFormat::Rgba8,
            orientation_degrees: 0,
            pixels,
            timestamp_ms: 1000,
            device_angle_degrees: 0.0,
            brightness_hint: 0.8,
            is_steady: true,
        };

        let detected = document_core_detect_document_edges(frame.clone()).expect("detect edges");
        assert!(detected.is_some());
        let corners = detected.expect("detected document corners");
        assert!(corners.confidence_milli > 0);

        let warped = document_core_warp_perspective(frame.clone(), corners, None).expect("warp");
        assert!(warped.width > 0 && warped.height > 0);

        let deskewed = document_core_deskew_frame(warped).expect("deskew");
        assert!(deskewed.skew_angle_degrees.abs() <= 15.0);

        let rotated = document_core_rotate_frame(frame.clone(), 1).expect("rotate");
        assert_eq!(rotated.orientation_degrees, 90);

        let session_id = document_core_start_scan_session();
        let page = document_core_append_scan_page(session_id.clone(), frame, Some(corners), None)
            .expect("append page");
        assert_eq!(page.page_index, 0);

        let warped_page =
            document_core_get_warped_scan_page(session_id.clone(), 0).expect("get warped page");
        assert!(warped_page.width > 0);

        let snap = document_core_get_scan_session(session_id.clone()).expect("get snap");
        assert_eq!(snap.page_count, 1);
        assert_eq!(snap.pages[0].page_index, 0);

        document_core_close_scan_session(session_id.clone()).expect("close");
        assert!(document_core_get_scan_session(session_id).is_err());
    }
}
