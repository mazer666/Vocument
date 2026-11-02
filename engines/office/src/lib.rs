//! DOCX and ODT import/export adapters for the `FlowDocument` model (M4.5, `OFFICE-001`–`005`,
//! `ADR-0020`).
//!
//! Layers, bottom to top:
//!
//! - [`xml`]: hardened XML reader (no DTDs, no entities, bounded depth);
//! - [`package`]: ZIP reader with bomb/traversal protection and a deterministic writer;
//! - [`report`]: the transparent record of everything lost, simplified or blocked;
//! - format adapters ([`docx`], [`odt`]) and [`fidelity`] measurement used by the bake-off.

pub mod convert;
pub mod docx;
pub mod error;
pub mod fidelity;
pub mod finalize;
pub mod limits;
pub mod odt;
pub mod package;
pub mod report;
pub mod util;
pub mod walk;
pub mod xml;

pub use convert::{AssetMap, ExportedDocument, ImportedAsset, ImportedDocument, OfficeFormat};
pub use error::OfficeError;
pub use fidelity::{
    FidelityEvaluation, Tier1Metrics, Tier2Metrics, Tier3Metrics, evaluate_fidelity,
};
pub use limits::OfficeLimits;
pub use report::{ConversionReport, ReportEntry, Severity};

use vocument_flow::FlowDocument;

/// Detects whether bytes are a DOCX or an ODT package, from package structure, not file name.
pub fn detect_format(bytes: &[u8], limits: &OfficeLimits) -> Option<OfficeFormat> {
    let package = package::OfficePackage::from_bytes(bytes, limits).ok()?;
    if let Some(mimetype) = package.part("mimetype")
        && mimetype.starts_with(b"application/vnd.oasis.opendocument.text")
    {
        return Some(OfficeFormat::Odt);
    }
    if package.contains("[Content_Types].xml") && package.contains("_rels/.rels") {
        return Some(OfficeFormat::Docx);
    }
    if package.contains("content.xml") {
        return Some(OfficeFormat::Odt);
    }
    None
}

/// Imports an Office file of the given format into the `FlowDocument` model.
pub fn import_document(
    bytes: &[u8],
    format: OfficeFormat,
    limits: &OfficeLimits,
) -> Result<ImportedDocument, OfficeError> {
    match format {
        OfficeFormat::Docx => docx::import(bytes, limits),
        OfficeFormat::Odt => odt::import(bytes, limits),
    }
}

/// Exports a `FlowDocument` (with its image assets) as an Office file of the given format.
pub fn export_document(
    document: &FlowDocument,
    assets: &AssetMap,
    format: OfficeFormat,
) -> Result<ExportedDocument, OfficeError> {
    match format {
        OfficeFormat::Docx => docx::export(document, assets),
        OfficeFormat::Odt => odt::export(document, assets),
    }
}

/// Convenience helper: imports a DOCX archive with default [`OfficeLimits`].
pub fn import_docx(bytes: &[u8]) -> Result<ImportedDocument, OfficeError> {
    docx::import(bytes, &OfficeLimits::default())
}

/// Convenience helper: exports a [`FlowDocument`] to a DOCX archive.
pub fn export_docx(
    document: &FlowDocument,
    assets: &AssetMap,
) -> Result<ExportedDocument, OfficeError> {
    docx::export(document, assets)
}

/// Convenience helper: imports an ODT archive with default [`OfficeLimits`].
pub fn import_odt(bytes: &[u8]) -> Result<ImportedDocument, OfficeError> {
    odt::import(bytes, &OfficeLimits::default())
}

/// Convenience helper: exports a [`FlowDocument`] to an ODT archive.
pub fn export_odt(
    document: &FlowDocument,
    assets: &AssetMap,
) -> Result<ExportedDocument, OfficeError> {
    odt::export(document, assets)
}
