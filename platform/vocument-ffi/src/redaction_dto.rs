//! UniFFI DTOs and exported functions for the secure redaction (`REDACT-001`,
//! `ROADMAP.md` M2 Gate: *redaction extraction/search verification*).
//!
//! A redaction is only worth something if the removed content is really gone.
//! The exported function therefore does not only remove the content, it also
//! **verifies** the removal and reports what was removed:
//!
//! - the text must no longer be extractable from the page,
//! - the search must no longer find it,
//! - it must no longer be present in the bytes of the file as plain text.
//!
//! A redaction cannot be undone. The report tells the platform shell what it
//! has to show to the user before the document is saved.

use crate::{DocumentCoreError, session_registry};
use vocument_core::redaction::{RedactionRegion, RedactionReport};

/// A region of a page whose content shall be removed (`REDACT-001`).
///
/// The coordinates are in PDF user space, in milli-points, with the origin at
/// the lower left of the page — the same space in which the bounding box of a
/// text line is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct FfiRedactionRegion {
    pub x0_milli_pt: i32,
    pub y0_milli_pt: i32,
    pub x1_milli_pt: i32,
    pub y1_milli_pt: i32,
}

impl From<FfiRedactionRegion> for RedactionRegion {
    fn from(region: FfiRedactionRegion) -> Self {
        RedactionRegion::new(
            region.x0_milli_pt,
            region.y0_milli_pt,
            region.x1_milli_pt,
            region.y1_milli_pt,
        )
    }
}

/// The report of a redaction and of its verification (`REDACT-001`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiRedactionReport {
    /// The page on which the content was removed.
    pub page_index: u32,
    /// The number of regions that were removed.
    pub region_count: u32,
    /// The text that lay under the regions.
    pub removed_fragments: Vec<String>,
    /// The number of text fragments that were removed.
    pub removed_fragment_count: u32,
    /// The text is no longer extractable from the page.
    pub text_extraction_clean: bool,
    /// The text is no longer found by the search over the document.
    pub search_index_clean: bool,
    /// The text is no longer present in the bytes of the file as plain text.
    pub file_bytes_clean: bool,
    /// The removal was verified in every respect.
    pub verified: bool,
    /// The revision of the document after the redaction.
    pub revision: u64,
}

impl From<RedactionReport> for FfiRedactionReport {
    fn from(report: RedactionReport) -> Self {
        let verified = report.is_verified();
        Self {
            page_index: report.page_index,
            region_count: report.region_count,
            removed_fragments: report.removed_fragments,
            removed_fragment_count: report.removed_fragment_count,
            text_extraction_clean: report.text_extraction_clean,
            search_index_clean: report.search_index_clean,
            file_bytes_clean: report.file_bytes_clean,
            verified,
            revision: report.revision,
        }
    }
}

/// Removes the content of the given regions of a page for good and verifies
/// the removal (`REDACT-001`).
///
/// The redaction cannot be undone. The text under the region is removed from
/// the document, not covered by a black rectangle.
#[uniffi::export]
pub fn document_core_redact_page_regions(
    session_id: String,
    page_index: u32,
    regions: Vec<FfiRedactionRegion>,
) -> Result<FfiRedactionReport, DocumentCoreError> {
    let registry = session_registry();
    let regions: Vec<RedactionRegion> = regions.into_iter().map(Into::into).collect();
    let report = registry.redact_page_regions(&session_id, page_index, &regions)?;
    Ok(report.into())
}
