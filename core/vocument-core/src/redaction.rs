//! Secure redaction with verification (`REDACT-001`, `ROADMAP.md` M2 Gate:
//! *"redaction extraction/search verification"*).
//!
//! A redaction is only worth something if the removed content is really gone.
//! Covering a text with a black rectangle leaves the text in the file: it can
//! still be read with a text extraction, it is still found by the search, and
//! it is still there when the file is opened in a viewer that ignores the
//! rectangle.
//!
//! This module therefore does three things:
//!
//! 1. It removes the content under the region from the document, with the
//!    engine, not with a drawn rectangle.
//! 2. It **verifies** the result: the text must no longer be extractable, it
//!    must no longer be found by the search, and it must no longer be present
//!    in the bytes of the file as plain text.
//! 3. It reports what was removed, so that the user can see what the
//!    redaction did before saving the document.
//!
//! A redaction cannot be undone. The content is gone from the document, and
//! the checkpoints of the recovery store — which still hold the old bytes —
//! are deleted with it. The undo history of the document is therefore
//! truncated at the redaction.

use serde::{Deserialize, Serialize};

use crate::pdf_engine::{PdfEngineError, PdfRectMilliPt};
use crate::session::{SessionError, SessionRegistry};

/// A region of a page whose content shall be removed (`REDACT-001`).
///
/// The coordinates are in PDF user space, in milli-points, with the origin at
/// the lower left of the page — the same space in which the bounding box of a
/// text line is reported.
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

    fn as_rect(self) -> PdfRectMilliPt {
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

/// The report of a redaction and of its verification (`REDACT-001`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionReport {
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
    /// The revision of the document after the redaction.
    pub revision: u64,
}

impl RedactionReport {
    /// Answers whether the redaction was verified in every respect.
    ///
    /// A redaction that cannot be verified is not a redaction. The caller has
    /// to treat an unverified redaction as a failure.
    pub fn is_verified(&self) -> bool {
        self.text_extraction_clean && self.search_index_clean && self.file_bytes_clean
    }
}

/// Error of a redaction (`REDACT-001`).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RedactionError {
    #[error("the redaction needs at least one region")]
    EmptySelection,
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

/// Normalises a text for the comparison in the bytes of the file.
///
/// A PDF may store the text in an encoding that is not UTF-8 — a subset font
/// with its own encoding, for example. The check over the bytes is therefore
/// only a supplementary signal; the text extraction is the proof.
fn normalized_for_bytes(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Answers whether a text fragment carries content.
///
/// A line that consists only of punctuation — a separator, a bracket — is not
/// evidence of a leak. A single character would match almost everywhere in a
/// PDF file, so such a fragment says nothing about the removal.
fn is_content_fragment(text: &str) -> bool {
    text.chars().filter(|c| c.is_alphanumeric()).count() >= MIN_CONTENT_CHARACTERS
}

/// The number of characters a fragment must have to be checked.
const MIN_CONTENT_CHARACTERS: usize = 4;

/// Builds a search query from a text fragment.
///
/// The full-text search of SQLite FTS5 treats some characters as operators.
/// The query is therefore reduced to the words of the fragment.
fn search_query_for(fragment: &str) -> Option<String> {
    let query: String = fragment
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
        })
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if query.is_empty() { None } else { Some(query) }
}

impl SessionRegistry {
    /// Removes the content of the given regions of a page and verifies the
    /// result (`REDACT-001`).
    ///
    /// The redaction cannot be undone. The text under the region is removed
    /// from the document, and the checkpoints of the recovery store that still
    /// hold the old bytes are deleted.
    pub fn redact_page_regions(
        &self,
        session_id: &str,
        page_index: u32,
        regions: &[RedactionRegion],
    ) -> Result<RedactionReport, SessionError> {
        if regions.is_empty() {
            return Err(SessionError::Redaction(RedactionError::EmptySelection));
        }
        let rects: Vec<PdfRectMilliPt> = regions.iter().map(|region| region.as_rect()).collect();

        let page_count =
            self.with_session(session_id, |session| Ok(session.pdf_summary.page_count))?;
        if page_index >= page_count {
            return Err(SessionError::Redaction(RedactionError::PageOutOfBounds {
                page_index,
                page_count,
            }));
        }

        // The text under the region, before the removal. It is the reference
        // for the verification and the report to the user.
        let text_before = self.with_session(session_id, |session| {
            Ok(session.actor.extract_page_text(page_index)?)
        })?;
        let removed_fragments: Vec<String> = text_before
            .lines
            .iter()
            .filter(|line| {
                regions
                    .iter()
                    .any(|region| region.overlaps_with(&line.bbox))
            })
            .map(|line| line.text.trim().to_owned())
            .filter(|text| !text.is_empty())
            .filter(|text| is_content_fragment(text))
            .collect();

        // The removal itself.
        self.apply_redaction(session_id, page_index, rects)?;

        // The verification: the text must be gone from the page, from the
        // search and from the bytes of the file.
        let text_after = self.with_session(session_id, |session| {
            Ok(session.actor.extract_page_text(page_index)?)
        })?;
        let text_extraction_clean = removed_fragments.iter().all(|fragment| {
            let needle = normalized_for_bytes(fragment);
            !text_after
                .lines
                .iter()
                .any(|line| normalized_for_bytes(&line.text).contains(&needle))
        });

        // The search index holds the text of the page. It has to be updated,
        // otherwise the search would still find the removed text.
        self.reindex_page(session_id, page_index)?;
        let search_index_clean = removed_fragments.iter().all(|fragment| {
            let Some(query) = search_query_for(fragment) else {
                return true;
            };
            match self.search_exact(session_id, &query) {
                Ok(response) => !response.hits.iter().any(|hit| hit.page_index == page_index),
                Err(_) => false,
            }
        });

        let file_bytes = self.with_session(session_id, |session| {
            Ok(session.actor.current_pdf_bytes()?.to_vec())
        })?;
        let file_bytes_clean = removed_fragments.iter().all(|fragment| {
            let needle = normalized_for_bytes(fragment);
            if needle.is_empty() {
                return true;
            }
            let haystack = normalized_for_bytes(&String::from_utf8_lossy(&file_bytes));
            !haystack.contains(&needle)
        });

        let revision = self.with_session(session_id, |session| Ok(session.revision))?;

        Ok(RedactionReport {
            page_index,
            region_count: regions.len() as u32,
            removed_fragment_count: removed_fragments.len() as u32,
            removed_fragments,
            text_extraction_clean,
            search_index_clean,
            file_bytes_clean,
            revision,
        })
    }
}
