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

use crate::pdf_engine::{PdfEngineError, PdfRectMilliPt, PdfTextOccurrence, PdfTextWord};
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
/// no evidence of a leak. This limit applies to the checks that look for a
/// word in the document — the text layer, the search and the bytes of the
/// file. A short word is not left unchecked: the check over the redacted
/// region (see [`word_lies_in_region`]) covers every word, however short it
/// is.
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

/// Answers whether `candidate` is a fragment of `reference`.
///
/// The removal works character by character. A word that is only partly
/// covered can lose its first or its last characters — what is left is a
/// fragment of the word, and a fragment is still readable. A check that only
/// looks for the whole word does not see it: "CH" is not the word
/// "VERTRAULICH", but it is still part of it and it is still in the file.
///
/// Only the beginning and the end of a word can survive, because the region
/// has an edge: a fragment is therefore a prefix or a suffix of the word. A
/// single character is not enough evidence — it occurs in almost every
/// document and would refuse every redaction.
fn is_fragment_of(candidate: &str, reference: &str) -> bool {
    let candidate_key = comparison_key(candidate);
    let reference_key = comparison_key(reference);
    candidate_key.chars().count() >= 2
        && candidate_key.chars().count() < reference_key.chars().count()
        && (reference_key.starts_with(&candidate_key) || reference_key.ends_with(&candidate_key))
}

/// Answers whether a word lies in the redacted region.
///
/// The centre of the word decides: a word that only touches the border of the
/// region is not counted as removed.
fn word_lies_in_region(word: &PdfTextWord, regions: &[RedactionRegion]) -> bool {
    regions.iter().any(|region| {
        let rect = region.as_rect();
        let centre_x = (word.bbox.x0_milli_pt + word.bbox.x1_milli_pt) / 2;
        let centre_y = (word.bbox.y0_milli_pt + word.bbox.y1_milli_pt) / 2;
        centre_x >= rect.x0_milli_pt
            && centre_x <= rect.x1_milli_pt
            && centre_y >= rect.y0_milli_pt
            && centre_y <= rect.y1_milli_pt
    })
}

/// Answers whether a word still sticks out of the redacted region.
///
/// The removal works character by character: a word that is only partly
/// covered can lose its first or its last characters and survive as a
/// fragment. Such a fragment is still content — it can still be read and it
/// is still in the file. A check that only looks for the *whole* word would
/// not see it, because "CH" is not the word "VERTRAULICH".
///
/// The rule is the same as the one that decides which words were removed in
/// the first place: a word belongs to the region when its centre lies in it.
/// What was there before has to be gone afterwards — otherwise the region
/// still carries something that a reader can see.
fn word_remains_in_region(word: &PdfTextWord, regions: &[RedactionRegion]) -> bool {
    word_lies_in_region(word, regions)
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
    /// The places of the document that still carry one of the given words.
    ///
    /// The engine reports the places; the core decides what they mean. An
    /// engine that cannot report them fails the call — a redaction whose
    /// verification is missing is not a redaction.
    fn find_word_occurrences(
        &self,
        session_id: &str,
        words: &[String],
    ) -> Result<Vec<PdfTextOccurrence>, SessionError> {
        if words.is_empty() {
            return Ok(Vec::new());
        }
        self.with_session(session_id, |session| {
            Ok(session.actor.find_word_occurrences(words)?)
        })
    }

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

        // Every word that lay under the region is reported — also the short
        // ones. A report that names only the long words would hide what was
        // removed.
        let mut removed_word_texts: Vec<String> = removed_words
            .iter()
            .map(|word| word.text.trim().to_owned())
            .filter(|text| !text.is_empty())
            .collect();
        // The words that only the picture carries are removed as well.
        for word in &visible_words_before {
            let text = word.text.trim().to_owned();
            if text.is_empty() {
                continue;
            }
            if !removed_word_texts
                .iter()
                .any(|known| is_same_word(known, &text))
            {
                removed_word_texts.push(text);
            }
        }

        // The checks that look for a word in the document — the text layer,
        // the search and the bytes of the file — only work for words that are
        // long enough to be evidence of a leak. A short word is not left
        // unchecked: the check over the redacted region covers every word,
        // however short it is.
        let checked_words: Vec<String> = removed_word_texts
            .iter()
            .filter(|text| is_content_word(text))
            .cloned()
            .collect();

        // The bytes of the document before the removal. They are the way back:
        // a redaction that cannot be verified is rolled back, and the user
        // must not hold a changed document while believing that nothing
        // happened.
        let bytes_before = self.with_session(session_id, |session| {
            Ok(session.actor.current_pdf_bytes()?.to_vec())
        })?;

        // The removal itself.
        self.apply_redaction(session_id, page_index, rects)?;

        // The verification.
        let text_after = self.with_session(session_id, |session| {
            Ok(session.actor.extract_page_text(page_index)?)
        })?;
        let words_after: Vec<PdfTextWord> = text_after.words.clone();

        // The text layer. Two things have to hold:
        //
        // 1. The removed words are no longer extractable.
        // 2. Nothing is left in the redacted region. A word that was only
        //    partly covered can survive as a fragment — and a fragment is
        //    still content.
        let residue: Vec<&PdfTextWord> = words_after
            .iter()
            .filter(|word| word_remains_in_region(word, regions))
            .collect();
        let fragment: Option<(&str, &str)> = checked_words.iter().find_map(|target| {
            words_after
                .iter()
                .find(|word| is_fragment_of(&word.text, target))
                .map(|word| (word.text.as_str(), target.as_str()))
        });
        let word_still_present = checked_words.iter().any(|removed| {
            words_after
                .iter()
                .any(|word| is_same_word(&word.text, removed))
        });
        let text_layer_clean = if residue.is_empty() && fragment.is_none() && !word_still_present {
            RedactionCheck::Passed
        } else {
            RedactionCheck::Failed
        };

        // The search index holds the text of the page. It has to be updated,
        // otherwise the search would still find the removed text.
        self.reindex_page(session_id, page_index)?;
        let search_index_clean = if checked_words.is_empty() {
            RedactionCheck::Passed
        } else {
            let mut failed = false;
            for removed in &checked_words {
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
                // A word that is still readable on the picture of the region
                // is a survivor. That is also true for a fragment of a word:
                // the picture can show "CH" where the text layer had
                // "VERTRAULICH", and a fragment is still readable.
                let survivors: Vec<String> = before
                    .iter()
                    .filter(|word| word_lies_in_region_tolerant(word, regions))
                    .filter(|word| is_content_word(&word.text))
                    .filter(|word| {
                        after.iter().any(|other| {
                            (is_same_word(&other.text, &word.text)
                                || is_fragment_of(&other.text, &word.text))
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
        // It looks at the whole document, and it therefore also finds content
        // that the text layer does not show — a note in the metadata, the
        // value of a form field, the text of an annotation. A word that
        // legitimately occurs somewhere else in the document also stops the
        // redaction: the check is strict on purpose, because a redaction
        // that cannot be verified is not a redaction.
        let file_bytes = self.with_session(session_id, |session| {
            Ok(session.actor.current_pdf_bytes()?.to_vec())
        })?;

        // The places of the document that still carry the removed text: the
        // text of an annotation, the value of a form field, the metadata, the
        // XMP packet, the content of an embedded file. The engine reports the
        // places, the core decides what they mean. A place that still carries
        // the text means that the redaction could not be verified.
        let file_bytes_clean = if checked_words.is_empty() {
            RedactionCheck::Passed
        } else {
            match self.find_word_occurrences(session_id, &checked_words) {
                Ok(occurrences) => {
                    let places_that_carry_the_text: Vec<&PdfTextOccurrence> = occurrences
                        .iter()
                        .filter(|occurrence| {
                            // The content of the other pages is not part of the
                            // redaction: the user marked a region of this
                            // page. The other places belong to the document;
                            // they are cleaned before the check and a place
                            // that survives the cleaning stops the redaction.
                            !(occurrence.kind == "content"
                                && occurrence.page_index != page_index as i32)
                        })
                        .collect();
                    if !places_that_carry_the_text.is_empty() {
                        RedactionCheck::Failed
                    } else {
                        let haystack = normalized_for_bytes(&String::from_utf8_lossy(&file_bytes));
                        let mut failed = false;
                        for removed in &checked_words {
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
                    }
                }
                // The engine cannot report the places. The check could not be
                // carried out, and that is not a pass.
                Err(_) => RedactionCheck::NotChecked,
            }
        };

        let revision = self.with_session(session_id, |session| Ok(session.revision))?;

        let report = RedactionReport {
            page_index,
            region_count: regions.len() as u32,
            removed_word_count: removed_word_texts.len() as u32,
            removed_words: removed_word_texts,
            text_layer_clean,
            search_index_clean,
            rendered_page_clean,
            file_bytes_clean,
            revision,
        };

        // A redaction that cannot be verified is not a redaction. The document
        // is put back as it was, and the caller is told that nothing changed.
        if !report.is_verified() {
            let failures = report.failed_checks().join("; ");
            self.rollback_redaction(session_id, bytes_before, page_index)?;
            return Err(SessionError::Redaction(
                RedactionError::VerificationFailed { details: failures },
            ));
        }

        // The checkpoints of the recovery store hold the bytes of the earlier
        // states of the document, and those bytes still contain the removed
        // content. They are deleted with the redaction — but only now, after
        // the removal has been proven.
        self.purge_redaction_checkpoints(session_id)?;

        Ok(report)
    }

    /// Puts the document back as it was before a redaction that could not be
    /// verified (`REDACT-002`).
    ///
    /// The bytes of the document are restored, the search index is built
    /// again, the transaction of the attempt is removed from the transaction
    /// manager — the journal of the recovery store would otherwise replay a
    /// redaction that never happened — and the journal is written again.
    fn rollback_redaction(
        &self,
        session_id: &str,
        bytes_before: Vec<u8>,
        page_index: u32,
    ) -> Result<(), SessionError> {
        self.with_session(session_id, |session| {
            Ok(session.actor.restore_pdf_bytes(bytes_before.clone())?)
        })?;
        // The search index holds the text of the page: it has to hold the old
        // text again.
        self.reindex_page(session_id, page_index)?;
        self.with_session_mut(session_id, |session| {
            Ok(session.transaction_manager.discard_last_transaction())
        })?;
        let _ = self.record_recovery_mutation(session_id);
        Ok(())
    }

    /// Deletes the checkpoints of the recovery store of the document
    /// (`REDACT-005`).
    ///
    /// The checkpoints hold the bytes of the earlier states of the document,
    /// and those bytes still contain the removed content.
    fn purge_redaction_checkpoints(&self, session_id: &str) -> Result<(), SessionError> {
        let document_id =
            self.with_session(session_id, |session| Ok(session.document_id.clone()))?;
        self.recovery_store
            .container_for(document_id.as_str())
            .purge_checkpoints()
            .map_err(SessionError::Recovery)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, x0: i32, y0: i32, x1: i32, y1: i32) -> PdfTextWord {
        PdfTextWord {
            text: text.to_owned(),
            bbox: PdfRectMilliPt {
                x0_milli_pt: x0,
                y0_milli_pt: y0,
                x1_milli_pt: x1,
                y1_milli_pt: y1,
            },
        }
    }

    fn region(x0: i32, y0: i32, x1: i32, y1: i32) -> RedactionRegion {
        RedactionRegion::new(x0, y0, x1, y1)
    }

    #[test]
    fn a_fragment_of_a_removed_word_is_recognised() {
        // "CH" is what is left of "VERTRAULICH" when the region covered the
        // beginning of the word. It is not the word any more — but it is
        // still part of it, and it is still readable.
        assert!(is_fragment_of("CH", "VERTRAULICH"));
        assert!(is_fragment_of("VERT", "VERTRAULICH"));
        assert!(is_fragment_of("ch", "vertraulich"));
    }

    #[test]
    fn a_word_that_is_only_similar_is_not_a_fragment() {
        // Another word that happens to end in the same letters is not a
        // fragment of the removed one.
        assert!(!is_fragment_of("Antrag", "VERTRAULICH"));
        assert!(!is_fragment_of("Fach", "VERTRAULICH"));
        assert!(!is_fragment_of("VERTRAULICH", "VERTRAULICH"));
    }

    #[test]
    fn a_single_character_is_no_evidence_of_a_leak() {
        // "S" occurs in almost every document and would refuse every
        // redaction.
        assert!(!is_fragment_of("S", "SECRET"));
    }

    #[test]
    fn a_word_that_stays_in_the_region_is_a_residue() {
        let regions = [region(0, 0, 1_000, 1_000)];
        // The rest of a word that was only partly covered.
        assert!(word_remains_in_region(
            &word("CH", 100, 100, 300, 200),
            &regions
        ));
        // A word of the next line, outside of the region.
        assert!(!word_remains_in_region(
            &word("Anhang", 100, 2_000, 900, 2_100),
            &regions
        ));
    }
}
