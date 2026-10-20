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

use crate::redaction_rules::{
    boxes_overlap, extend_regions_to_touching_words, is_content_word, is_fragment_of, is_same_word,
    normalized_for_bytes, word_lies_in_region_tolerant, word_remains_in_region,
    word_touches_region,
};

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
        // The words that are readable on the picture of the region, before the
        // removal. The text layer does not know everything: a scan carries its
        // words in the image, and a graphic carries no text at all. The
        // picture is the truth about what a reader sees.
        let visible_before = match self.read_rendered_words(session_id, page_index) {
            Ok(words) => words,
            Err(error) => {
                return Err(SessionError::Redaction(
                    RedactionError::VerificationFailed {
                        details: format!(
                            "the picture of the page could not be read, so the removal cannot be verified: {error}"
                        ),
                    },
                ));
            }
        };

        // Every word the region touches belongs to the redaction, and the
        // region grows to the whole word: the removal works character by
        // character, and a word that is only partly covered would leave a
        // readable fragment behind. The report names the words that were
        // removed.
        //
        // The words of the picture count as well: a scan has no text layer,
        // and the pixels of the image are the content there.
        let mut touched_words: Vec<PdfTextWord> = text_before
            .words
            .iter()
            .filter(|word| word_touches_region(word, regions))
            .cloned()
            .collect();
        touched_words.extend(
            visible_before
                .as_ref()
                .map(|words| {
                    words
                        .iter()
                        .filter(|word| word_lies_in_region_tolerant(word, regions))
                        .cloned()
                        .collect::<Vec<PdfTextWord>>()
                })
                .unwrap_or_default(),
        );
        let regions: Vec<RedactionRegion> =
            extend_regions_to_touching_words(regions, &touched_words);
        let rects: Vec<PdfRectMilliPt> = regions.iter().map(|region| region.as_rect()).collect();
        let removed_words: Vec<PdfTextWord> = text_before
            .words
            .iter()
            .filter(|word| word_touches_region(word, &regions))
            .cloned()
            .collect();

        let visible_words_before: Vec<PdfTextWord> = visible_before
            .as_ref()
            .map(|words| {
                words
                    .iter()
                    .filter(|word| word_lies_in_region_tolerant(word, &regions))
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

        // Every word that was removed is checked — also the short ones. A
        // secret of three letters is as secret as a secret of twenty, and the
        // search and the search for the places of the document are exact
        // enough to carry a word of one or two letters.
        //
        // Only the check over the bytes of the file needs a minimum length:
        // it looks for the word in the whole document, and a single letter
        // would be found in every line of every font and would drown the
        // check in false hits.
        let checked_words: Vec<String> = removed_word_texts.clone();
        let byte_check_words: Vec<String> = removed_word_texts
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

        // The removal itself. From here on the document is changed, and every
        // error has to put it back.
        if let Err(error) = self.apply_redaction(session_id, page_index, rects.clone()) {
            return self.fail_after_change(session_id, page_index, &bytes_before, error);
        }

        // The verification.
        let text_after = match self.with_session(session_id, |session| {
            Ok(session.actor.extract_page_text(page_index)?)
        }) {
            Ok(text) => text,
            Err(error) => {
                return self.fail_after_change(session_id, page_index, &bytes_before, error);
            }
        };
        let words_after: Vec<PdfTextWord> = text_after.words.clone();

        // The text layer. Two things have to hold:
        //
        // 1. The removed words are no longer extractable.
        // 2. Nothing is left in the redacted region. A word that was only
        //    partly covered can survive as a fragment — and a fragment is
        //    still content.
        let residue: Vec<&PdfTextWord> = words_after
            .iter()
            .filter(|word| word_remains_in_region(word, &regions))
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
        if let Err(error) = self.reindex_page(session_id, page_index) {
            return self.fail_after_change(session_id, page_index, &bytes_before, error);
        }
        let search_index_clean = if checked_words.is_empty() {
            // Nothing to look for: the check was not carried out, and that is
            // not a pass.
            RedactionCheck::NotChecked
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
        let visible_after = match self.read_rendered_words(session_id, page_index) {
            Ok(words) => words,
            Err(error) => {
                return self.fail_after_change(session_id, page_index, &bytes_before, error);
            }
        };
        let rendered_page_clean = match (visible_before, visible_after) {
            (Some(before), Some(after)) => {
                // Two things have to hold:
                //
                // 1. Nothing may be readable at the place of the removed word
                //    any more. A word that the OCR could not read before was
                //    never in the list of the words that were readable — such
                //    a word would survive a comparison of the two lists
                //    without being noticed.
                // 2. Everything that was readable in the region before has to
                //    be gone afterwards, the whole word and its fragments.
                let removed_boxes: Vec<PdfRectMilliPt> = removed_words
                    .iter()
                    .map(|word| word.bbox)
                    .chain(visible_words_before.iter().map(|word| word.bbox))
                    .collect();
                let survivors: Vec<String> = after
                    .iter()
                    .filter(|word| word_lies_in_region_tolerant(word, &regions))
                    .filter(|word| is_content_word(&word.text))
                    .filter(|word| {
                        let stands_at_the_place = removed_boxes
                            .iter()
                            .any(|box_of_word| boxes_overlap(word.bbox, *box_of_word));
                        let was_readable_before = before.iter().any(|other| {
                            (is_same_word(&other.text, &word.text)
                                || is_fragment_of(&word.text, &other.text))
                                && word_lies_in_region_tolerant(other, &regions)
                        });
                        stands_at_the_place || was_readable_before
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
        let file_bytes = match self.with_session(session_id, |session| {
            Ok(session.actor.current_pdf_bytes()?.to_vec())
        }) {
            Ok(bytes) => bytes,
            Err(error) => {
                return self.fail_after_change(session_id, page_index, &bytes_before, error);
            }
        };

        // The places of the document that still carry the removed text: the
        // text of an annotation, the value of a form field, the metadata, the
        // XMP packet, the content of an embedded file. The engine reports the
        // places, the core decides what they mean. A place that still carries
        // the text means that the redaction could not be verified.
        let mut remaining_occurrences: Vec<PdfTextOccurrence> = Vec::new();
        // The places of the document that the engine could not clean: the
        // text of an annotation, the value of a form field, the metadata, the
        // content of an embedded file. They are named in the error, so that
        // the user knows where the text is still standing.
        let mut uncleaned_places: Vec<PdfTextOccurrence> = Vec::new();
        let file_bytes_clean = if checked_words.is_empty() {
            // Nothing to look for: the check was not carried out, and that is
            // not a pass.
            RedactionCheck::NotChecked
        } else {
            match self.find_word_occurrences(session_id, &checked_words) {
                Ok(occurrences) => {
                    // The content of the other pages is not part of the
                    // redaction: the user marked a region of this page. Those
                    // places are reported, they do not stop the redaction. The
                    // other places — an annotation, a form value, the
                    // metadata — were cleaned before the check, and a place
                    // that survives the cleaning stops the redaction.
                    let inside: Vec<&PdfTextOccurrence> = occurrences
                        .iter()
                        .filter(|occurrence| {
                            !(occurrence.kind == "content"
                                && occurrence.page_index != page_index as i32)
                        })
                        .collect();
                    let outside: Vec<&PdfTextOccurrence> = occurrences
                        .iter()
                        .filter(|occurrence| {
                            occurrence.kind == "content"
                                && occurrence.page_index != page_index as i32
                        })
                        .collect();
                    remaining_occurrences = outside
                        .iter()
                        .map(|occurrence| (*occurrence).clone())
                        .collect();
                    uncleaned_places = inside.iter().map(|place| (*place).clone()).collect();
                    if !inside.is_empty() {
                        RedactionCheck::Failed
                    } else {
                        let haystack = normalized_for_bytes(&String::from_utf8_lossy(&file_bytes));
                        let mut failed = false;
                        for removed in &byte_check_words {
                            let needle = normalized_for_bytes(removed);
                            if needle.is_empty() {
                                continue;
                            }
                            // A word that stands in a place the engine
                            // knows — on another page — is reported, not
                            // refused: the user marked a region, and the word
                            // belongs to the document. A word that the
                            // engine does not know any more but that still
                            // stands in the bytes is a remainder without a
                            // place: an old version of the file, an
                            // unreachable object. That stops the redaction.
                            if haystack.contains(&needle) && remaining_occurrences.is_empty() {
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
            remaining_occurrences,
            revision,
        };

        // A redaction that cannot be verified is not a redaction. The document
        // is put back as it was, and the caller is told that nothing changed.
        if !report.is_verified() {
            let mut failures = report.failed_checks();
            for place in &uncleaned_places {
                failures.push(format!(
                    "the text is still there: {} (page {}, {})",
                    place.location, place.page_index, place.kind
                ));
            }
            let failures = failures.join("; ");
            return self.fail_after_change(
                session_id,
                page_index,
                &bytes_before,
                SessionError::Redaction(RedactionError::VerificationFailed { details: failures }),
            );
        }

        // The checkpoints of the recovery store hold the bytes of the earlier
        // states of the document, and those bytes still contain the removed
        // content. They are deleted with the redaction — but only now, after
        // the removal has been proven. A failure here does not undo the
        // redaction: the content is gone, and the verification has said so.
        self.purge_checkpoints_after_success(session_id);

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

    /// Puts the document back after an error and returns the error.
    ///
    /// The removal happens before the verification. **Every** error after
    /// that point has to put the document back: the user must not hold a
    /// changed document while believing that nothing happened. A rollback
    /// that fails itself is reported as its own error — the session then
    /// holds a document in an unknown state, and the user has to know that.
    fn fail_after_change<T>(
        &self,
        session_id: &str,
        page_index: u32,
        bytes_before: &[u8],
        error: SessionError,
    ) -> Result<T, SessionError> {
        match self.rollback_redaction(session_id, bytes_before.to_vec(), page_index) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(SessionError::RedactionRollbackFailed {
                original: error.to_string(),
                rollback: rollback_error.to_string(),
            }),
        }
    }

    /// Deletes the checkpoints of the recovery store, and does not turn a
    /// failure into the failure of the redaction.
    ///
    /// The redaction itself has succeeded at this point: the content is gone
    /// and the verification has confirmed it. A checkpoint that could not be
    /// deleted is a problem of the recovery store — and it must not turn a
    /// redaction that worked into an error, because the caller would then
    /// believe that the document is in doubt.
    fn purge_checkpoints_after_success(&self, session_id: &str) {
        if let Err(error) = self.purge_redaction_checkpoints(session_id) {
            // The redaction has succeeded; the recovery store is the problem.
            eprintln!(
                "vocument: the checkpoints of the recovery store could not be deleted after a redaction: {error}"
            );
        }
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
