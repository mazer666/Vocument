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
use vocument_core::pdf_engine::PdfTextOccurrence;
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

/// A place of the document that still carries a redacted word
/// (`REDACT-001`).
///
/// The redaction removes what the user marked. A word that occurs somewhere
/// else in the document is not part of the redaction — but the shell has to
/// know about it, so that it cannot tell the user that the word is gone from
/// the document when it is still in it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiRedactionOccurrence {
    /// The kind of the place: `content`, `annotation`, `form`, `attachment`,
    /// `info` or `xmp`.
    pub kind: String,
    /// The page the place lies on, `-1` for the document itself.
    pub page_index: i32,
    /// A short description of the place.
    pub location: String,
}

impl From<PdfTextOccurrence> for FfiRedactionOccurrence {
    fn from(occurrence: PdfTextOccurrence) -> Self {
        Self {
            kind: occurrence.kind,
            page_index: occurrence.page_index,
            location: occurrence.location,
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
    /// The places of the document where the same word still stands — on
    /// another page, for example.
    ///
    /// The redaction removes what the user marked. The shell should show
    /// these places to the user: the word is still in the document, only not
    /// under the region that was marked.
    pub remaining_occurrences: Vec<FfiRedactionOccurrence>,
    /// The places of the document that could not be read.
    ///
    /// The redaction does not claim that the text is gone from a place it
    /// could not look into — the content of a container that is attached to
    /// the document, for example. The shell should show these places to the
    /// user.
    pub uninspected_places: Vec<FfiRedactionOccurrence>,
    /// The places of the document that carried the removed text **before**
    /// the removal.
    ///
    /// This list is the reference of the verification: the product examined
    /// the places itself, before it changed the document. A place that
    /// carried the text before and does not carry it any more was cleaned.
    pub places_before_removal: Vec<FfiRedactionOccurrence>,
    /// The reasons why a check could not be carried out.
    ///
    /// A check that could not be carried out is not a pass. The reason is
    /// named, so that the shell can tell the user **why** the removal could
    /// not be proven.
    pub not_checked_reasons: Vec<String>,
    /// The removal was verified in every respect.
    ///
    /// The flag is false when a check could not be carried out **or** when a
    /// place of the document could not be read: the tool does not promise
    /// what it cannot prove.
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
            remaining_occurrences: report
                .remaining_occurrences
                .into_iter()
                .map(Into::into)
                .collect(),
            uninspected_places: report
                .uninspected_places
                .into_iter()
                .map(Into::into)
                .collect(),
            places_before_removal: report
                .places_before_removal
                .into_iter()
                .map(Into::into)
                .collect(),
            not_checked_reasons: report.not_checked_reasons,
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
