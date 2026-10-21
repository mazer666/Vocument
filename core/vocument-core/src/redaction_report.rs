//! The types of a redaction and their report (`REDACT-001`, `REDACT-002`).
//!
//! A redaction removes the content of a region of a page and proves that the
//! content is gone. The report of the redaction names **every** check that
//! was carried out, and the state of each of them: `Passed`, `Failed` or
//! `NotChecked`.
//!
//! `NotChecked` is **not** a pass. A check that could not be carried out
//! means that the verification is incomplete, and a tool that says
//! "verified" while a check was skipped makes a promise it cannot keep.

use serde::{Deserialize, Serialize};

use crate::pdf_engine::{PdfEngineError, PdfRectMilliPt, PdfTextOccurrence};

/// A region of a page whose content shall be removed (`REDACT-001`).
///
/// The coordinates are in the page space of the engine: milli-points, origin
/// at the top left of the page, y descending — the same space in which the
/// bounding box of a text word and the rendered page are reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionRegion {
    pub x0_milli_pt: i32,
    pub y0_milli_pt: i32,
    pub x1_milli_pt: i32,
    pub y1_milli_pt: i32,
}

impl RedactionRegion {
    pub fn new(x0_milli_pt: i32, y0_milli_pt: i32, x1_milli_pt: i32, y1_milli_pt: i32) -> Self {
        Self {
            x0_milli_pt,
            y0_milli_pt,
            x1_milli_pt,
            y1_milli_pt,
        }
    }

    pub fn as_rect(self) -> PdfRectMilliPt {
        PdfRectMilliPt {
            x0_milli_pt: self.x0_milli_pt,
            y0_milli_pt: self.y0_milli_pt,
            x1_milli_pt: self.x1_milli_pt,
            y1_milli_pt: self.y1_milli_pt,
        }
    }

    /// Answers whether the rectangle overlaps with `other`.
    pub fn overlaps_with(&self, other: &PdfRectMilliPt) -> bool {
        self.x0_milli_pt < other.x1_milli_pt
            && self.x1_milli_pt > other.x0_milli_pt
            && self.y0_milli_pt < other.y1_milli_pt
            && self.y1_milli_pt > other.y0_milli_pt
    }
}

/// The result of one verification step (`REDACT-002`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedactionCheck {
    /// The step was carried out and the content is gone.
    Passed,
    /// The step was carried out and the content is still there.
    Failed,
    /// The step could not be carried out. It is **not** a pass.
    NotChecked,
}

/// The report of a redaction and of its verification (`REDACT-001`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionReport {
    /// The page on which the content was removed.
    pub page_index: u32,
    /// The number of regions that were removed.
    pub region_count: u32,
    /// The words that lay under the regions.
    pub removed_words: Vec<String>,
    /// The number of words that were removed.
    pub removed_word_count: u32,
    /// The words are no longer extractable from the text layer of the page.
    pub text_layer_clean: RedactionCheck,
    /// The words are no longer found by the search over the document.
    pub search_index_clean: RedactionCheck,
    /// The words are no longer readable on the rendered page.
    pub rendered_page_clean: RedactionCheck,
    /// The words are no longer present in the bytes of the file as plain text.
    pub file_bytes_clean: RedactionCheck,
    /// The places of the document where the same word still stands — on
    /// another page, for example.
    ///
    /// The redaction removes what the user marked. A word that occurs
    /// somewhere else in the document is not part of the redaction, but the
    /// user has to know about it: the report names the places, so that nobody
    /// believes that a word is gone from the document when it is still in it.
    pub remaining_occurrences: Vec<PdfTextOccurrence>,
    /// The places of the document that could not be read.
    ///
    /// The redaction does not claim that the text is gone from a place it
    /// could not look into — the content of a container that is attached to
    /// the document, for example. The places are named here, so that the
    /// user can decide what to do with them.
    ///
    /// A place that could not be read means that the verification is
    /// **incomplete**: the redaction is not a secure document, and it is
    /// refused rather than reported as verified.
    pub uninspected_places: Vec<PdfTextOccurrence>,
    /// The places of the document that carried the removed text **before**
    /// the removal.
    ///
    /// This list is the reference of the verification. The check of the
    /// product used to run behind the garbage collection of the writer: the
    /// objects that nothing reached any more were removed before the check
    /// ran, and the places that carried the text were never examined. The
    /// safety of the redaction rested on the writer of the engine and not on
    /// the check of the product.
    ///
    /// The list is the evidence that the product looked at the places
    /// itself: a place that carried the text before and does not carry it
    /// any more was cleaned.
    pub places_before_removal: Vec<PdfTextOccurrence>,
    /// The reasons why a check could not be carried out.
    ///
    /// A check that could not be carried out is not a pass. The reason is
    /// named, so that the user learns **why** the redaction could not be
    /// proven — a missing reader, a document that the engine could not read,
    /// a place that could not be looked into.
    pub not_checked_reasons: Vec<String>,
    /// The revision of the document after the redaction.
    pub revision: u64,
}

impl RedactionReport {
    /// Answers whether the redaction was verified in every respect.
    ///
    /// A redaction that cannot be verified is not a redaction. The caller has
    /// to treat an unverified redaction as a failure.
    ///
    /// A place of the document that could **not** be read makes the
    /// verification incomplete: the redaction cannot claim that the text is
    /// gone from a place nobody looked into. A tool that says "verified" while
    /// a place was not read would make a promise it cannot keep.
    pub fn is_verified(&self) -> bool {
        self.text_layer_clean == RedactionCheck::Passed
            && self.search_index_clean == RedactionCheck::Passed
            && self.rendered_page_clean == RedactionCheck::Passed
            && self.file_bytes_clean == RedactionCheck::Passed
            && self.uninspected_places.is_empty()
    }

    /// The checks that did not pass, as a readable list.
    pub fn failed_checks(&self) -> Vec<String> {
        let mut failures = Vec::new();
        for (name, check) in [
            ("text layer", self.text_layer_clean),
            ("search index", self.search_index_clean),
            ("rendered page", self.rendered_page_clean),
            ("file bytes", self.file_bytes_clean),
        ] {
            match check {
                RedactionCheck::Passed => {}
                RedactionCheck::Failed => {
                    failures.push(format!("{name}: the content is still there"))
                }
                RedactionCheck::NotChecked => {
                    failures.push(format!("{name}: the check could not be carried out"))
                }
            }
        }
        for reason in &self.not_checked_reasons {
            failures.push(reason.clone());
        }
        failures
    }
}

/// Error of a redaction (`REDACT-001`).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RedactionError {
    #[error("the redaction needs at least one region")]
    EmptySelection,
    #[error("the region contains no text; nothing was removed")]
    NothingToRemove,
    #[error("the page index {page_index} is out of bounds (page count: {page_count})")]
    PageOutOfBounds { page_index: u32, page_count: u32 },
    #[error("the content could not be removed: {details}")]
    EngineFailure { details: String },
    #[error("the redaction could not be verified: {details}")]
    VerificationFailed { details: String },
}

impl From<PdfEngineError> for RedactionError {
    fn from(error: PdfEngineError) -> Self {
        match error {
            PdfEngineError::PageOutOfBounds {
                page_index,
                page_count,
            } => RedactionError::PageOutOfBounds {
                page_index,
                page_count,
            },
            other => RedactionError::EngineFailure {
                details: other.to_string(),
            },
        }
    }
}
