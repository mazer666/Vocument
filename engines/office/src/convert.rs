//! Types shared by the import and export entry points of both formats.

use std::collections::BTreeMap;

use vocument_flow::FlowDocument;

use crate::report::ConversionReport;

/// Binary content referenced by an `ImageBlock::asset_ref`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedAsset {
    pub media_type: String,
    pub bytes: Vec<u8>,
}

/// Assets by content-addressed path (`assets/<sha256>.<ext>`), the `.pdoc` asset convention.
pub type AssetMap = BTreeMap<String, ImportedAsset>;

/// Result of importing an Office file.
#[derive(Debug, Clone)]
pub struct ImportedDocument {
    pub document: FlowDocument,
    pub assets: AssetMap,
    /// Everything that was lost, simplified, kept-but-not-rendered or blocked.
    pub report: ConversionReport,
}

/// Result of exporting a document to an Office file.
#[derive(Debug, Clone)]
pub struct ExportedDocument {
    pub bytes: Vec<u8>,
    pub report: ConversionReport,
}

/// The Office formats Vocument reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OfficeFormat {
    Docx,
    Odt,
}

impl OfficeFormat {
    /// File extension without dot.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Odt => "odt",
        }
    }

    /// IANA media type of the format.
    pub fn media_type(self) -> &'static str {
        match self {
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Odt => "application/vnd.oasis.opendocument.text",
        }
    }
}
