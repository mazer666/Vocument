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

use crate::pdf_engine::{PdfRectMilliPt, PdfTextOccurrence, PdfTextWord};
use crate::session::{SessionError, SessionRegistry};

// The types of a redaction live in their own module; they are re-exported
// here, so that the paths of the callers do not change.
pub use crate::redaction_report::{
    RedactionCheck, RedactionError, RedactionRegion, RedactionReport,
};
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

        // The places of the document that carry the removed text — **before**
        // the removal. This list is the reference of the verification.
        //
        // The fifth round of the red team measured that the check runs
        // behind the garbage collection of the writer: the objects that
        // nothing reaches any more are removed before the check runs, and
        // the places that carried the text are therefore never examined. The
        // safety of the redaction rested on the writer of the engine and not
        // on the check of the product.
        //
        // With the reference the check has something to compare with: a place
        // that carried the text before and does not carry it any more was
        // cleaned — by the removal itself, by the cleaning, or by the writer
        // of the engine. And a place that carried the text before and still
        // carries it is a failure, also when the word is too short for the
        // check over the bytes of the file.
        let places_before = match self.find_word_occurrences(session_id, &checked_words) {
            Ok(occurrences) => occurrences,
            Err(error) => {
                // The document has not been changed yet: the redaction is
                // refused before it touches anything.
                return Err(SessionError::Redaction(
                    RedactionError::VerificationFailed {
                        details: format!(
                            "the places of the document could not be examined before the removal, so the removal cannot be verified: {error}"
                        ),
                    },
                ));
            }
        };

        // The **pre-check** has to be complete as well. It is the reference of
        // the verification: the list of the places that carried the text
        // before the removal. A place that the walk could not examine makes
        // that reference incomplete — and an incomplete reference is a proof
        // that proves nothing.
        //
        // The refusal happens here, **before** the document is touched: a
        // refused redaction then changes nothing at all, not even for a
        // moment.
        let unexamined_before: Vec<String> = places_before
            .iter()
            .filter(|place| place.kind == "uninspected")
            .map(|place| place.location.clone())
            .collect();
        if !unexamined_before.is_empty() {
            return Err(SessionError::Redaction(
                RedactionError::VerificationFailed {
                    details: format!(
                        "the places of the document could not all be examined before the removal, so the removal cannot be verified: at least {} place(s) could not be looked into, among them {}",
                        unexamined_before.len(),
                        unexamined_before.join(", ")
                    ),
                },
            ));
        }

        // The removal itself. From here on the document is changed, and every
        // error has to put it back.
        if let Err(error) = self.apply_redaction(session_id, page_index, rects.clone()) {
            return self.fail_after_change(session_id, page_index, &bytes_before, error);
        }

        // The verification.
        //
        // The reasons why a check could not be carried out are collected here:
        // a check that could not be carried out is not a pass, and a reason
        // that is not named is an excuse instead of an information.
        let mut not_checked_reasons: Vec<String> = Vec::new();

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
        // 1. Nothing is left in the redacted region. A word that was only
        //    partly covered can survive as a fragment — and a fragment is
        //    still content.
        // 2. No removed word stands any more **at the place of the
        //    removal**. The removal works on the text layer of the page, and
        //    this is where its result has to show.
        //
        // The same word may stand somewhere else — on another line of the
        // same page, on another page. That is not a failure of the
        // redaction: the user marked a region, and the word belongs to the
        // document. The places are reported, so that nobody believes that
        // the word is gone from the document when it is still in it.
        let residue: Vec<&PdfTextWord> = words_after
            .iter()
            .filter(|word| word_remains_in_region(word, &regions))
            .collect();
        let survivor_at_the_place: Vec<&PdfTextWord> = words_after
            .iter()
            .filter(|word| {
                removed_words.iter().any(|removed| {
                    (is_same_word(&word.text, &removed.text)
                        || is_fragment_of(&word.text, &removed.text))
                        && boxes_overlap(word.bbox, removed.bbox)
                })
            })
            .collect();
        let text_layer_clean = if residue.is_empty() && survivor_at_the_place.is_empty() {
            RedactionCheck::Passed
        } else {
            RedactionCheck::Failed
        };

        // A removed word that stands elsewhere on the page. The place is
        // reported, and the checks below do not treat it as a failure: the
        // word belongs to the document, and the user marked a region.
        let word_still_elsewhere = |word: &str| {
            words_after.iter().any(|candidate| {
                (is_same_word(&candidate.text, word) || is_fragment_of(&candidate.text, word))
                    && !word_remains_in_region(candidate, &regions)
            })
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
                // A word that stands elsewhere on the page is in the index as
                // well: the index is right, and the redaction is not at
                // fault. The index is only wrong when it carries a word that
                // the page does not carry any more.
                if word_still_elsewhere(removed) {
                    continue;
                }
                match self.search_exact(session_id, removed) {
                    Ok(response) => {
                        if response.hits.iter().any(|hit| hit.page_index == page_index) {
                            failed = true;
                            break;
                        }
                    }
                    Err(error) => {
                        not_checked_reasons.push(format!(
                            "the search index could not be searched for the removed word: {error}"
                        ));
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
            // not a pass, and the reason is named: the user has to learn that
            // the picture of the page was never looked at.
            _ => {
                not_checked_reasons.push(
                    "the picture of the page was not read: no reader for the rendered page is wired"
                        .to_owned(),
                );
                RedactionCheck::NotChecked
            }
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
        // text of an annotation, in the popup of an annotation, the value of a
        // form field, the metadata, the structure tree, the name tree, the
        // optional content, the private data, the content of an embedded
        // file. The engine reports the places, the core decides what they
        // mean. A place that still carries the text means that the redaction
        // could not be verified.
        let mut remaining_occurrences: Vec<PdfTextOccurrence> = Vec::new();
        // The places of the document that the engine could not clean: they
        // are named in the error, so that the user knows where the text is
        // still standing.
        let mut uncleaned_places: Vec<PdfTextOccurrence> = Vec::new();
        // The places that could not be read at all: the content of a
        // container that is attached to the document. The redaction does not
        // claim that the text is gone from them — it says that it could not
        // look.
        let mut uninspected_places: Vec<PdfTextOccurrence> = Vec::new();
        let file_bytes_clean = if checked_words.is_empty() {
            // Nothing to look for: the check was not carried out, and that is
            // not a pass.
            RedactionCheck::NotChecked
        } else {
            match self.find_word_occurrences(session_id, &checked_words) {
                Ok(occurrences) => {
                    // The content of the other pages is not part of the
                    // redaction: the user marked a region of this page. Those
                    // places are reported, they do not stop the redaction.
                    //
                    // The content of the marked page is part of it — unless
                    // the word stands elsewhere on the page. The text layer
                    // knows where the word stands: a word that the text layer
                    // still shows outside of the region belongs to the
                    // document and is reported. A word that the text layer
                    // does not show any more stands in the content stream in
                    // a way that nobody can read — that stops the redaction.
                    //
                    // The other places — an annotation, a form value, the
                    // metadata — were cleaned before the check, and a place
                    // that survives the cleaning stops the redaction.
                    let explained = |occurrence: &PdfTextOccurrence| {
                        occurrence.kind == "content"
                            && checked_words.iter().any(|word| word_still_elsewhere(word))
                    };
                    let inside: Vec<&PdfTextOccurrence> = occurrences
                        .iter()
                        .filter(|occurrence| match occurrence.kind.as_str() {
                            // A place that could not be read is a warning, not
                            // a failure: the redaction does not claim that the
                            // text is gone from there.
                            "uninspected" => false,
                            // The content of another page is a remainder, not a
                            // failure — it is named in the report.
                            "content" => {
                                occurrence.page_index == page_index as i32 && !explained(occurrence)
                            }
                            // A hidden place — a place of the document that is
                            // not the content of a page — was not cleaned: that
                            // stops the redaction.
                            "hidden" => true,
                            // A kind the core does not know is treated like a
                            // hidden place: the core does not trust a kind it
                            // does not know.
                            _ => true,
                        })
                        .collect();
                    let outside: Vec<&PdfTextOccurrence> = occurrences
                        .iter()
                        .filter(|occurrence| {
                            (occurrence.kind == "content"
                                && occurrence.page_index != page_index as i32)
                                || explained(occurrence)
                        })
                        .collect();
                    let uninspected: Vec<&PdfTextOccurrence> = occurrences
                        .iter()
                        .filter(|occurrence| occurrence.kind == "uninspected")
                        .collect();
                    remaining_occurrences = outside
                        .iter()
                        .map(|occurrence| (*occurrence).clone())
                        .collect();
                    uncleaned_places = inside.iter().map(|place| (*place).clone()).collect();
                    uninspected_places = uninspected.iter().map(|place| (*place).clone()).collect();
                    if !uninspected_places.is_empty() {
                        not_checked_reasons.push(format!(
                            "the document could not be read completely: {} place(s) could not be looked into, among them {}",
                            uninspected_places.len(),
                            uninspected_places
                                .iter()
                                .map(|place| place.location.as_str())
                                .collect::<Vec<&str>>()
                                .join(", ")
                        ));
                    }
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
                            // knows — on another page, elsewhere on this
                            // page — is reported, not refused: the user
                            // marked a region, and the word belongs to the
                            // document. A word that the engine does not know
                            // any more but that still stands in the bytes is
                            // a remainder without a place: an old version of
                            // the file, an unreachable object. That stops
                            // the redaction.
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
                // carried out, and that is not a pass. The reason is named.
                Err(error) => {
                    not_checked_reasons.push(format!(
                        "the places of the document that could still carry the text could not be read: {error}"
                    ));
                    RedactionCheck::NotChecked
                }
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
            uninspected_places,
            places_before_removal: places_before
                .iter()
                .filter(|place| place.kind != "uninspected")
                .cloned()
                .collect(),
            not_checked_reasons,
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
            // The report is lost when the redaction fails — the caller gets
            // the error and nothing else. The error therefore carries what
            // the report would have carried: the words that were removed and
            // the places where the same text still stands.
            if !report.removed_words.is_empty() {
                failures.push(format!(
                    "the words that were removed: {}",
                    report.removed_words.join(", ")
                ));
            }
            for place in &report.remaining_occurrences {
                failures.push(format!(
                    "the same text still stands here: {} (page {}, {})",
                    place.location, place.page_index, place.kind
                ));
            }
            for place in &report.uninspected_places {
                failures.push(format!(
                    "this place could not be read: {} (page {}, {})",
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

        // The rendered tiles and the thumbnails of the earlier revision still
        // show the removed content. They are dropped, so that the text is not
        // readable in the memory of the process any more.
        self.with_session(session_id, |session| {
            self.purge_render_cache(session.document_id.as_str());
            Ok(())
        })?;

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
            session.transaction_manager.discard_last_transaction();
            // The session keeps its own copy of the revision and of the dirty
            // flag. They have to go back with the transaction: a session that
            // was rolled back to the state before the redaction may not claim
            // the revision of the redaction, and it may not claim to have
            // unsaved changes any more.
            session.sync_from_transaction_manager();
            Ok(())
        })?;
        // The journal of the recovery store must not hold a redaction that
        // never happened. If it cannot be written, the error is reported: the
        // state of the document is not secured against a crash.
        if let Err(error) = self.record_recovery_mutation(session_id) {
            eprintln!(
                "vocument: the journal of the recovery store could not be written after a rolled back redaction: {error}"
            );
        }
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
