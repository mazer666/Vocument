//! UniFFI types and exports for M3.4 Bates Numbering, Headers/Footers, Bookmarks, and Stamps/Watermarks (`ROADMAP.md` M3.4).

use crate::navigation_search_dto::FfiPdfRectMilliPt;
use crate::workbench_dto::FfiPageWorkbenchOptions;
use crate::{DocumentCoreError, mutation_dto::FfiTransactionRecord, session_registry};
use vocument_core::workbench_stamps::{
    BatesNumberingOptions, BatesPosition, BookmarkItem, HeaderFooterOptions,
    StampOrWatermarkOptions,
};

/// Page positions for Bates numbering stamps (`M3.4`).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiBatesPosition {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl From<FfiBatesPosition> for BatesPosition {
    fn from(pos: FfiBatesPosition) -> Self {
        match pos {
            FfiBatesPosition::TopLeft => BatesPosition::TopLeft,
            FfiBatesPosition::TopCenter => BatesPosition::TopCenter,
            FfiBatesPosition::TopRight => BatesPosition::TopRight,
            FfiBatesPosition::BottomLeft => BatesPosition::BottomLeft,
            FfiBatesPosition::BottomCenter => BatesPosition::BottomCenter,
            FfiBatesPosition::BottomRight => BatesPosition::BottomRight,
        }
    }
}

impl From<BatesPosition> for FfiBatesPosition {
    fn from(pos: BatesPosition) -> Self {
        match pos {
            BatesPosition::TopLeft => FfiBatesPosition::TopLeft,
            BatesPosition::TopCenter => FfiBatesPosition::TopCenter,
            BatesPosition::TopRight => FfiBatesPosition::TopRight,
            BatesPosition::BottomLeft => FfiBatesPosition::BottomLeft,
            BatesPosition::BottomCenter => FfiBatesPosition::BottomCenter,
            BatesPosition::BottomRight => FfiBatesPosition::BottomRight,
        }
    }
}

/// Configuration options for legal Bates numbering (`M3.4`).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct FfiBatesNumberingOptions {
    pub prefix: String,
    pub start_number: u32,
    pub digit_padding: u32,
    pub suffix: String,
    pub position: FfiBatesPosition,
    pub font_size_pt: u32,
    pub margin_pt: u32,
}

impl From<FfiBatesNumberingOptions> for BatesNumberingOptions {
    fn from(opts: FfiBatesNumberingOptions) -> Self {
        Self {
            prefix: opts.prefix,
            start_number: opts.start_number,
            digit_padding: opts.digit_padding,
            suffix: opts.suffix,
            position: opts.position.into(),
            font_size_pt: opts.font_size_pt,
            margin_pt: opts.margin_pt,
        }
    }
}

impl From<BatesNumberingOptions> for FfiBatesNumberingOptions {
    fn from(opts: BatesNumberingOptions) -> Self {
        Self {
            prefix: opts.prefix,
            start_number: opts.start_number,
            digit_padding: opts.digit_padding,
            suffix: opts.suffix,
            position: opts.position.into(),
            font_size_pt: opts.font_size_pt,
            margin_pt: opts.margin_pt,
        }
    }
}

/// Configuration options for multi-zone headers and footers with dynamic macros (`M3.4`).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct FfiHeaderFooterOptions {
    pub header_left: Option<String>,
    pub header_center: Option<String>,
    pub header_right: Option<String>,
    pub footer_left: Option<String>,
    pub footer_center: Option<String>,
    pub footer_right: Option<String>,
    pub font_size_pt: u32,
    pub margin_pt: u32,
}

impl From<FfiHeaderFooterOptions> for HeaderFooterOptions {
    fn from(opts: FfiHeaderFooterOptions) -> Self {
        Self {
            header_left: opts.header_left,
            header_center: opts.header_center,
            header_right: opts.header_right,
            footer_left: opts.footer_left,
            footer_center: opts.footer_center,
            footer_right: opts.footer_right,
            font_size_pt: opts.font_size_pt,
            margin_pt: opts.margin_pt,
        }
    }
}

impl From<HeaderFooterOptions> for FfiHeaderFooterOptions {
    fn from(opts: HeaderFooterOptions) -> Self {
        Self {
            header_left: opts.header_left,
            header_center: opts.header_center,
            header_right: opts.header_right,
            footer_left: opts.footer_left,
            footer_center: opts.footer_center,
            footer_right: opts.footer_right,
            font_size_pt: opts.font_size_pt,
            margin_pt: opts.margin_pt,
        }
    }
}

/// Hierarchical bookmark item (`M3.4`).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct FfiBookmarkItem {
    pub title: String,
    pub target_page: u32,
    pub children: Vec<FfiBookmarkItem>,
}

impl From<FfiBookmarkItem> for BookmarkItem {
    fn from(item: FfiBookmarkItem) -> Self {
        Self {
            title: item.title,
            target_page: item.target_page,
            children: item.children.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<BookmarkItem> for FfiBookmarkItem {
    fn from(item: BookmarkItem) -> Self {
        Self {
            title: item.title,
            target_page: item.target_page,
            children: item.children.into_iter().map(Into::into).collect(),
        }
    }
}

/// Configuration options for rubber stamps and watermarks (`WATER-001`–`003`, `M3.4`).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct FfiStampOrWatermarkOptions {
    pub text: String,
    pub opacity_milli: u32,
    pub rotation_degrees: i32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub is_background: bool,
    pub rect: Option<FfiPdfRectMilliPt>,
}

impl From<FfiStampOrWatermarkOptions> for StampOrWatermarkOptions {
    fn from(opts: FfiStampOrWatermarkOptions) -> Self {
        Self {
            text: opts.text,
            opacity_milli: opts.opacity_milli,
            rotation_degrees: opts.rotation_degrees,
            color_rgb: [opts.r, opts.g, opts.b],
            is_background: opts.is_background,
            rect: opts.rect.map(Into::into),
        }
    }
}

impl From<StampOrWatermarkOptions> for FfiStampOrWatermarkOptions {
    fn from(opts: StampOrWatermarkOptions) -> Self {
        Self {
            text: opts.text,
            opacity_milli: opts.opacity_milli,
            rotation_degrees: opts.rotation_degrees,
            r: opts.color_rgb[0],
            g: opts.color_rgb[1],
            b: opts.color_rgb[2],
            is_background: opts.is_background,
            rect: opts.rect.map(Into::into),
        }
    }
}

/// Applies Bates numbering to the selected pages (`M3.4`).
#[uniffi::export]
pub fn document_core_apply_bates_numbering_with_options(
    session_id: String,
    page_indices: Vec<u32>,
    options: FfiBatesNumberingOptions,
    workbench_options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.apply_bates_numbering_with_options(
        &session_id,
        page_indices,
        options.into(),
        workbench_options.into(),
    )?;
    Ok(record.into())
}

/// Applies multi-zone headers and footers with dynamic macros (`M3.4`).
#[uniffi::export]
pub fn document_core_apply_header_footer_with_options(
    session_id: String,
    page_indices: Vec<u32>,
    options: FfiHeaderFooterOptions,
    doc_title: Option<String>,
    date_str: Option<String>,
    workbench_options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.apply_header_footer_with_options(
        &session_id,
        page_indices,
        options.into(),
        doc_title,
        date_str,
        workbench_options.into(),
    )?;
    Ok(record.into())
}

/// Retrieves document bookmark hierarchy (`M3.4`).
#[uniffi::export]
pub fn document_core_get_bookmarks(
    session_id: String,
) -> Result<Vec<FfiBookmarkItem>, DocumentCoreError> {
    let registry = session_registry();
    let bookmarks = registry.get_bookmarks(&session_id)?;
    Ok(bookmarks.into_iter().map(Into::into).collect())
}

/// Sets document bookmark hierarchy (`M3.4`).
#[uniffi::export]
pub fn document_core_set_bookmarks_with_options(
    session_id: String,
    bookmarks: Vec<FfiBookmarkItem>,
    workbench_options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.set_bookmarks_with_options(
        &session_id,
        bookmarks.into_iter().map(Into::into).collect(),
        workbench_options.into(),
    )?;
    Ok(record.into())
}

/// Applies a rubber stamp or watermark overlay to the selected pages (`WATER-001`–`003`, `M3.4`).
#[uniffi::export]
pub fn document_core_apply_stamp_or_watermark_with_options(
    session_id: String,
    page_indices: Vec<u32>,
    options: FfiStampOrWatermarkOptions,
    workbench_options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.apply_stamp_or_watermark_with_options(
        &session_id,
        page_indices,
        options.into(),
        workbench_options.into(),
    )?;
    Ok(record.into())
}
