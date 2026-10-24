//! UniFFI types and exports for M3.5 Optimization, Image Extraction, and Preflight (`ROADMAP.md` M3.5).

use crate::mutation_dto::FfiTransactionRecord;
use crate::workbench_dto::FfiPageWorkbenchOptions;
use crate::{DocumentCoreError, session_registry};
use vocument_core::workbench_optimize::{
    ExtractedImageData, ExtractedImageInfo, OptimizationOptions, OptimizationPreset,
    PdfxPreflightReport, PrintPreflightReport,
};

/// Preset strategies for document optimization (`M3.5`).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiOptimizationPreset {
    Lossless,
    Balanced,
    Maximum,
}

impl From<FfiOptimizationPreset> for OptimizationPreset {
    fn from(p: FfiOptimizationPreset) -> Self {
        match p {
            FfiOptimizationPreset::Lossless => OptimizationPreset::Lossless,
            FfiOptimizationPreset::Balanced => OptimizationPreset::Balanced,
            FfiOptimizationPreset::Maximum => OptimizationPreset::Maximum,
        }
    }
}

impl From<OptimizationPreset> for FfiOptimizationPreset {
    fn from(p: OptimizationPreset) -> Self {
        match p {
            OptimizationPreset::Lossless => FfiOptimizationPreset::Lossless,
            OptimizationPreset::Balanced => FfiOptimizationPreset::Balanced,
            OptimizationPreset::Maximum => FfiOptimizationPreset::Maximum,
        }
    }
}

/// Options controlling document optimization and compression (`M3.5`).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct FfiOptimizationOptions {
    pub preset: FfiOptimizationPreset,
    pub do_linearize: bool,
}

impl From<FfiOptimizationOptions> for OptimizationOptions {
    fn from(opts: FfiOptimizationOptions) -> Self {
        Self {
            preset: opts.preset.into(),
            do_linearize: opts.do_linearize,
        }
    }
}

impl From<OptimizationOptions> for FfiOptimizationOptions {
    fn from(opts: OptimizationOptions) -> Self {
        Self {
            preset: opts.preset.into(),
            do_linearize: opts.do_linearize,
        }
    }
}

/// Metadata description of an embedded raster image in a document (`M3.5`).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct FfiExtractedImageInfo {
    pub page_index: u32,
    pub image_index: u32,
    pub obj_num: u32,
    pub width: u32,
    pub height: u32,
    pub bits: u32,
    pub format: String,
    pub colorspace: String,
}

impl From<ExtractedImageInfo> for FfiExtractedImageInfo {
    fn from(info: ExtractedImageInfo) -> Self {
        Self {
            page_index: info.page_index,
            image_index: info.image_index,
            obj_num: info.obj_num,
            width: info.width,
            height: info.height,
            bits: info.bits,
            format: info.format,
            colorspace: info.colorspace,
        }
    }
}

/// Full extracted raster image payload (`M3.5`).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct FfiExtractedImageData {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub data: Vec<u8>,
}

impl From<ExtractedImageData> for FfiExtractedImageData {
    fn from(data: ExtractedImageData) -> Self {
        Self {
            width: data.width,
            height: data.height,
            format: data.format,
            data: data.data,
        }
    }
}

/// Compliance preflight report for PDF/X (ISO 15930) verification (`ROADMAP.md` M3.5).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct FfiPdfxPreflightReport {
    pub is_valid: bool,
    pub version: String,
    pub has_output_intent: bool,
    pub output_intent_profile: String,
    pub has_trapped_key: bool,
    pub has_bleed_or_trim_box: bool,
    pub is_encrypted: bool,
    pub issues: Vec<String>,
}

impl From<PdfxPreflightReport> for FfiPdfxPreflightReport {
    fn from(r: PdfxPreflightReport) -> Self {
        Self {
            is_valid: r.is_valid,
            version: r.version,
            has_output_intent: r.has_output_intent,
            output_intent_profile: r.output_intent_profile,
            has_trapped_key: r.has_trapped_key,
            has_bleed_or_trim_box: r.has_bleed_or_trim_box,
            is_encrypted: r.is_encrypted,
            issues: r.issues,
        }
    }
}

/// Preflight readiness report for professional printing (`PRINT-003`, `PRINT-004`).
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct FfiPrintPreflightReport {
    pub page_count: u32,
    pub is_uniform_size: bool,
    pub width_pt: f32,
    pub height_pt: f32,
    pub has_bleed_box: bool,
    pub has_crop_box: bool,
    pub warnings: Vec<String>,
}

impl From<PrintPreflightReport> for FfiPrintPreflightReport {
    fn from(r: PrintPreflightReport) -> Self {
        Self {
            page_count: r.page_count,
            is_uniform_size: r.is_uniform_size,
            width_pt: r.width_pt,
            height_pt: r.height_pt,
            has_bleed_box: r.has_bleed_box,
            has_crop_box: r.has_crop_box,
            warnings: r.warnings,
        }
    }
}

/// Optimizes document streams, compacts objects, and optionally linearizes for Fast Web View (`M3.5`).
#[uniffi::export]
pub fn document_core_optimize_document_with_options(
    session_id: String,
    options: FfiOptimizationOptions,
    workbench_options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.optimize_document_with_options(
        &session_id,
        options.into(),
        workbench_options.into(),
    )?;
    Ok(record.into())
}

/// Returns metadata of all embedded raster images discovered in the document (`M3.5`).
#[uniffi::export]
pub fn document_core_get_document_images(
    session_id: String,
) -> Result<Vec<FfiExtractedImageInfo>, DocumentCoreError> {
    let registry = session_registry();
    let images = registry.get_document_images(&session_id)?;
    Ok(images.into_iter().map(Into::into).collect())
}

/// Extracts raster image bytes for a specified page and image index (`M3.5`).
#[uniffi::export]
pub fn document_core_extract_image_bytes(
    session_id: String,
    page_index: u32,
    image_index: u32,
) -> Result<FfiExtractedImageData, DocumentCoreError> {
    let registry = session_registry();
    let data = registry.extract_image_bytes(&session_id, page_index, image_index)?;
    Ok(data.into())
}

/// Performs PDF/X compliance preflight analysis (`ISO 15930`, `M3.5`).
#[uniffi::export]
pub fn document_core_preflight_pdfx(
    session_id: String,
) -> Result<FfiPdfxPreflightReport, DocumentCoreError> {
    let registry = session_registry();
    let report = registry.preflight_pdfx(&session_id)?;
    Ok(report.into())
}

/// Performs print readiness preflight analysis (`PRINT-003`, `PRINT-004`).
#[uniffi::export]
pub fn document_core_preflight_print(
    session_id: String,
) -> Result<FfiPrintPreflightReport, DocumentCoreError> {
    let registry = session_registry();
    let report = registry.preflight_print(&session_id)?;
    Ok(report.into())
}
