//! Secure redaction with verification (`REDACT-001`, `REDACT-002`,
//! `ROADMAP.md` M2 Gate: *"redaction extraction/search verification"*).
//!
//! A redaction is only worth something if the removed content is really gone.
//! Covering a text with a black rectangle leaves the text in the file: it can
//! still be read with a text extraction, it is still found by the search, and
//! it is still there when the file is opened in a viewer that ignores the
//! rectangle.
//!
//! A document can carry the same text **twice**: as a text layer and as pixels
//! in an image. That is the normal case for a scan, where the text layer is
//! invisible and the picture carries the words. A redaction that only removes
//! the text layer leaves the picture readable. This module therefore checks
//! the removal against four independent facts:
//!
//! 1. **The text layer** — the removed words are no longer extractable.
//! 2. **The search** — the full-text index no longer holds them.
//! 3. **The picture of the page** — the removed words are no longer readable
//!    on the rendered page. This is the check that catches a scan whose image
//!    still carries the words.
//! 4. **The bytes of the file** — the removed words are no longer present as
//!    plain text.
//!
//! Every check reports its own result. A redaction is only secure if **all**
//! of them pass. A redaction that cannot be verified is not a redaction, and
//! the platform must not present such a document as redacted.
//!
//! A redaction cannot be undone. The content is gone from the document, and
//! the checkpoints of the recovery store — which still hold the old bytes —
//! are deleted with it.

use serde::{Deserialize, Serialize};

use crate::pdf_engine::{PdfEngineError, PdfRectMilliPt, PdfTextWord};
use crate::session::{SessionError, SessionRegistry};

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
    /// The revision of the document after the redaction.
    pub revision: u64,
}

impl RedactionReport {
    /// Answers whether the redaction was verified in every respect.
    ///
    /// A redaction that cannot be verified is not a redaction. The caller has
    /// to treat an unverified redaction as a failure.
    pub fn is_verified(&self) -> bool {
        self.text_layer_clean == RedactionCheck::Passed
            && self.search_index_clean == RedactionCheck::Passed
            && self.rendered_page_clean == RedactionCheck::Passed
            && self.file_bytes_clean == RedactionCheck::Passed
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

/// Normalises a text for the comparison in the bytes of the file.
///
/// A PDF may store the text in an encoding that is not UTF-8 — a subset font
/// with its own encoding, for example. The check over the bytes is therefore
/// only a supplementary signal; the extraction and the rendered page are the
/// proof.
fn normalized_for_bytes(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// The comparison form of a word: lower case, without punctuation.
fn comparison_key(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// The number of characters a word must have to be checked.
///
/// A single character would match almost everywhere in a PDF file and would be
/// no evidence of a leak.
const MIN_CONTENT_CHARACTERS: usize = 4;

/// The tolerance of the position comparison, in milli-points.
///
/// The picture of a page and the text layer of a page never agree to the
/// last micrometre: the rendering rounds, the OCR estimates. A word counts as
/// "still there" when it lies inside the redacted region, enlarged by this
/// tolerance.
const POSITION_TOLERANCE_MILLI_PT: i32 = 2_000;

/// Answers whether a word carries content.
fn is_content_word(word: &str) -> bool {
    comparison_key(word).chars().count() >= MIN_CONTENT_CHARACTERS
}

/// Answers whether the word `candidate` is the same word as `reference`.
fn is_same_word(candidate: &str, reference: &str) -> bool {
    let candidate_key = comparison_key(candidate);
    let reference_key = comparison_key(reference);
    !candidate_key.is_empty() && candidate_key == reference_key
}

/// Answers whether a word lies in the redacted region.
fn word_lies_in_region(word: &PdfTextWord, regions: &[RedactionRegion]) -> bool {
    regions.iter().any(|region| {
        let rect = region.as_rect();
        // The word counts as removed when it lies in the region. The
        // comparison uses the centre of the word, so that a word that only
        // touches the border of the region is not counted as removed.
        let centre_x = (word.bbox.x0_milli_pt + word.bbox.x1_milli_pt) / 2;
        let centre_y = (word.bbox.y0_milli_pt + word.bbox.y1_milli_pt) / 2;
        centre_x >= rect.x0_milli_pt
            && centre_x <= rect.x1_milli_pt
            && centre_y >= rect.y0_milli_pt
            && centre_y <= rect.y1_milli_pt
    })
}

/// Answers whether a word lies in the redacted region, enlarged by the
/// tolerance.
fn word_lies_in_region_tolerant(word: &PdfTextWord, regions: &[RedactionRegion]) -> bool {
    regions.iter().any(|region| {
        let rect = region.as_rect();
        let centre_x = (word.bbox.x0_milli_pt + word.bbox.x1_milli_pt) / 2;
        let centre_y = (word.bbox.y0_milli_pt + word.bbox.y1_milli_pt) / 2;
        centre_x >= rect.x0_milli_pt - POSITION_TOLERANCE_MILLI_PT
            && centre_x <= rect.x1_milli_pt + POSITION_TOLERANCE_MILLI_PT
            && centre_y >= rect.y0_milli_pt - POSITION_TOLERANCE_MILLI_PT
            && centre_y <= rect.y1_milli_pt + POSITION_TOLERANCE_MILLI_PT
    })
}

impl SessionRegistry {
    /// Removes the content of the given regions of a page and verifies the
    /// result (`REDACT-001`, `REDACT-002`).
    ///
    /// The redaction cannot be undone. The text under the region is removed
    /// from the document, and the checkpoints of the recovery store that still
    /// hold the old bytes are deleted.
    ///
    /// The report names every check. A redaction is secure only if all of
    /// them passed; the caller must not treat an unverified redaction as a
    /// success.
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

        // The words under the region, before the removal. They are the
        // reference for the verification and the report to the user.
        let text_before = self.with_session(session_id, |session| {
            Ok(session.actor.extract_page_text(page_index)?)
        })?;
        let removed_words: Vec<PdfTextWord> = text_before
            .words
            .iter()
            .filter(|word| word_lies_in_region(word, regions))
            .cloned()
            .collect();

        // The words that are readable on the picture of the region, before the
        // removal. The text layer does not know everything: a scan carries its
        // words in the image, and a graphic carries no text at all. The
        // picture is the truth about what a reader sees.
        let visible_before = self.read_rendered_words(session_id, page_index)?;
        let visible_words_before: Vec<PdfTextWord> = visible_before
            .as_ref()
            .map(|words| {
                words
                    .iter()
                    .filter(|word| word_lies_in_region_tolerant(word, regions))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        // A region without any content must not be reported as a successful
        // redaction. The user would believe that something was removed.
        if removed_words.is_empty() && visible_words_before.is_empty() {
            return Err(SessionError::Redaction(RedactionError::NothingToRemove));
        }

        let mut removed_word_texts: Vec<String> = removed_words
            .iter()
            .map(|word| word.text.trim().to_owned())
            .filter(|text| !text.is_empty())
            .filter(|text| is_content_word(text))
            .collect();
        // The words that only the picture carries are removed as well.
        for word in &visible_words_before {
            let text = word.text.trim().to_owned();
            if text.is_empty() || !is_content_word(&text) {
                continue;
            }
            if !removed_word_texts
                .iter()
                .any(|known| is_same_word(known, &text))
            {
                removed_word_texts.push(text);
            }
        }

        // The removal itself.
        self.apply_redaction(session_id, page_index, rects)?;

        // The verification.
        let text_after = self.with_session(session_id, |session| {
            Ok(session.actor.extract_page_text(page_index)?)
        })?;
        let words_after: Vec<PdfTextWord> = text_after.words.clone();
        let text_layer_clean = if removed_word_texts.iter().all(|removed| {
            !words_after
                .iter()
                .any(|word| is_same_word(&word.text, removed))
        }) {
            RedactionCheck::Passed
        } else {
            RedactionCheck::Failed
        };

        // The search index holds the text of the page. It has to be updated,
        // otherwise the search would still find the removed text.
        self.reindex_page(session_id, page_index)?;
        let search_index_clean = if removed_word_texts.is_empty() {
            RedactionCheck::Passed
        } else {
            let mut failed = false;
            for removed in &removed_word_texts {
                match self.search_exact(session_id, removed) {
                    Ok(response) => {
                        if response.hits.iter().any(|hit| hit.page_index == page_index) {
                            failed = true;
                            break;
                        }
                    }
                    Err(_) => {
                        failed = true;
                        break;
                    }
                }
            }
            if failed {
                RedactionCheck::Failed
            } else {
                RedactionCheck::Passed
            }
        };

        // The picture of the page. A document can carry the same text twice:
        // as a text layer and as pixels in an image. This check is the one
        // that catches a scan whose image still carries the words.
        //
        // The check compares the picture *before* and *after* the removal:
        // every word that was readable in the region before must be gone
        // afterwards. It does not rely on the text layer, which does not know
        // about the content of an image.
        let rendered_page_clean = match (
            visible_before,
            self.read_rendered_words(session_id, page_index)?,
        ) {
            (Some(before), Some(after)) => {
                let survivors: Vec<String> = before
                    .iter()
                    .filter(|word| word_lies_in_region_tolerant(word, regions))
                    .filter(|word| is_content_word(&word.text))
                    .filter(|word| {
                        after.iter().any(|other| {
                            is_same_word(&other.text, &word.text)
                                && word_lies_in_region_tolerant(other, regions)
                        })
                    })
                    .map(|word| word.text.trim().to_owned())
                    .collect();
                if survivors.is_empty() {
                    RedactionCheck::Passed
                } else {
                    RedactionCheck::Failed
                }
            }
            // No reader is wired: the check could not be carried out. That is
            // not a pass.
            _ => RedactionCheck::NotChecked,
        };

        // The bytes of the file. The check is supplementary: a PDF can hold
        // the text in an encoding that this comparison does not recognise.
        let file_bytes = self.with_session(session_id, |session| {
            Ok(session.actor.current_pdf_bytes()?.to_vec())
        })?;
        let file_bytes_clean = if removed_word_texts.is_empty() {
            RedactionCheck::Passed
        } else {
            let haystack = normalized_for_bytes(&String::from_utf8_lossy(&file_bytes));
            let mut failed = false;
            for removed in &removed_word_texts {
                let needle = normalized_for_bytes(removed);
                if needle.is_empty() {
                    continue;
                }
                if haystack.contains(&needle) {
                    failed = true;
                    break;
                }
            }
            if failed {
                RedactionCheck::Failed
            } else {
                RedactionCheck::Passed
            }
        };

        let revision = self.with_session(session_id, |session| Ok(session.revision))?;

        Ok(RedactionReport {
            page_index,
            region_count: regions.len() as u32,
            removed_word_count: removed_word_texts.len() as u32,
            removed_words: removed_word_texts,
            text_layer_clean,
            search_index_clean,
            rendered_page_clean,
            file_bytes_clean,
            revision,
        })
    }
}
