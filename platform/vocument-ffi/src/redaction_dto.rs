//! UniFFI DTOs and exported functions for the secure redaction (`REDACT-001`,
//! `REDACT-002`, `ROADMAP.md` M2 Gate: *redaction extraction/search
//! verification*).
//!
//! A redaction is only worth something if the removed content is really gone.
//! The exported function therefore does not only remove the content, it also
//! **verifies** the removal and reports what was removed:
//!
//! - the words must no longer be extractable from the text layer,
//! - the search must no longer find them,
//! - they must no longer be readable on the rendered page — a document can
//!   carry the same text twice, as a text layer and as pixels in an image,
//! - they must no longer be present in the bytes of the file as plain text.
//!
//! **A redaction that is not verified in every respect is refused.** The
//! function returns an error in that case, so that no caller can treat an
//! unverified redaction as a success.
//!
//! A redaction cannot be undone. The report tells the platform shell what it
//! has to show to the user before the document is saved.

use crate::{DocumentCoreError, session_registry};
use vocument_core::redaction::{RedactionCheck, RedactionRegion, RedactionReport};

/// A region of a page whose content shall be removed (`REDACT-001`).
///
/// The coordinates are in the page space of the engine: milli-points, origin
/// at the top left of the page, y descending — the same space in which the
/// bounding box of a text word and the rendered page are reported.
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

/// The result of one verification step (`REDACT-002`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiRedactionCheck {
    /// The step was carried out and the content is gone.
    Passed,
    /// The step was carried out and the content is still there.
    Failed,
    /// The step could not be carried out. It is **not** a pass.
    NotChecked,
}

impl From<RedactionCheck> for FfiRedactionCheck {
    fn from(check: RedactionCheck) -> Self {
        match check {
            RedactionCheck::Passed => FfiRedactionCheck::Passed,
            RedactionCheck::Failed => FfiRedactionCheck::Failed,
            RedactionCheck::NotChecked => FfiRedactionCheck::NotChecked,
        }
    }
}

/// The report of a redaction and of its verification (`REDACT-001`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiRedactionReport {
    /// The page on which the content was removed.
    pub page_index: u32,
    /// The number of regions that were removed.
    pub region_count: u32,
    /// The words that lay under the regions.
    pub removed_words: Vec<String>,
    /// The number of words that were removed.
    pub removed_word_count: u32,
    /// The words are no longer extractable from the text layer of the page.
    pub text_layer_clean: FfiRedactionCheck,
    /// The words are no longer found by the search over the document.
    pub search_index_clean: FfiRedactionCheck,
    /// The words are no longer readable on the rendered page.
    pub rendered_page_clean: FfiRedactionCheck,
    /// The words are no longer present in the bytes of the file as plain text.
    pub file_bytes_clean: FfiRedactionCheck,
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
            removed_words: report.removed_words,
            removed_word_count: report.removed_word_count,
            text_layer_clean: report.text_layer_clean.into(),
            search_index_clean: report.search_index_clean.into(),
            rendered_page_clean: report.rendered_page_clean.into(),
            file_bytes_clean: report.file_bytes_clean.into(),
            verified,
            revision: report.revision,
        }
    }
}

/// Removes the content of the given regions of a page for good and verifies
/// the removal (`REDACT-001`, `REDACT-002`).
///
/// The redaction cannot be undone. The words under the region are removed
/// from the document, not covered by a black rectangle.
///
/// The function returns an error when the removal could not be verified in
/// every respect. An unverified redaction must not be presented as a secure
/// document.
#[uniffi::export]
pub fn document_core_redact_page_regions(
    session_id: String,
    page_index: u32,
    regions: Vec<FfiRedactionRegion>,
) -> Result<FfiRedactionReport, DocumentCoreError> {
    let registry = session_registry();
    let regions: Vec<RedactionRegion> = regions.into_iter().map(Into::into).collect();
    let report = registry.redact_page_regions(&session_id, page_index, &regions)?;
    let failures = report.failed_checks();
    if !failures.is_empty() {
        return Err(DocumentCoreError::RedactionUnverified {
            details: failures.join("; "),
        });
    }
    Ok(report.into())
}
