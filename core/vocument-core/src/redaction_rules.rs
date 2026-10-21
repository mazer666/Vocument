//! The rules of the redaction (`REDACT-001`, `REDACT-002`).
//!
//! The rules decide what a marked region removes and what counts as a
//! survivor:
//!
//! - the **comparison form** of a word, the form in which two words are the
//!   same word;
//! - the **fragment**: a word that is only a part of a removed word is still
//!   content;
//! - the **region**: a word that the region touches belongs to the
//!   redaction, and the region grows to the whole word.

use crate::pdf_engine::{PdfRectMilliPt, PdfTextWord};

use crate::redaction::RedactionRegion;

/// Normalises a text for the comparison in the bytes of the file.
///
/// A PDF may store the text in an encoding that is not UTF-8 — a subset font
/// with its own encoding, for example. The check over the bytes is therefore
/// only a supplementary signal; the extraction and the rendered page are the
/// proof.
pub fn normalized_for_bytes(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// The comparison form of a word: lower case, without punctuation.
pub fn comparison_key(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// The number of characters a word must have to be checked.
///
/// The shortest word that the check over the bytes of the file looks for.
///
/// The check over the bytes looks for the word in the **whole document**, and
/// it compares the bytes as a text. A word of one or two letters would be
/// found in every line of every font — `TOP` in `STOP` — and the check would
/// refuse every redaction. The other checks are exact and carry every word,
/// however short it is: the search over the document, the search for the
/// places of the document and the check over the redacted region. A secret
/// of three letters therefore does not slip through — it is looked for in
/// the places of the document, and if it stands in a place that cannot be
/// cleaned, the redaction fails.
pub const MIN_CONTENT_CHARACTERS: usize = 4;

/// The tolerance of the position comparison, in milli-points.
///
/// The picture of a page and the text layer of a page never agree to the
/// last micrometre: the rendering rounds, the OCR estimates. A word counts as
/// "still there" when it lies inside the redacted region, enlarged by this
/// tolerance.
pub const POSITION_TOLERANCE_MILLI_PT: i32 = 2_000;

/// Answers whether a word carries content.
pub fn is_content_word(word: &str) -> bool {
    comparison_key(word).chars().count() >= MIN_CONTENT_CHARACTERS
}

/// Answers whether the word `candidate` is the same word as `reference`.
pub fn is_same_word(candidate: &str, reference: &str) -> bool {
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
pub fn is_fragment_of(candidate: &str, reference: &str) -> bool {
    let candidate_key = comparison_key(candidate);
    let reference_key = comparison_key(reference);
    candidate_key.chars().count() >= 2
        && candidate_key.chars().count() < reference_key.chars().count()
        && (reference_key.starts_with(&candidate_key) || reference_key.ends_with(&candidate_key))
}

/// Answers whether the region **touches** a word.
///
/// This is the rule that decides what a marked region removes. MuPDF removes
/// character by character: a word whose centre lies outside the region keeps
/// its first or its last characters, and the rest of the word stays in the
/// document as a readable fragment. A word that the region touches at all
/// therefore belongs to the region — and the region is extended to the whole
/// word, so that the word goes as a whole.
///
/// The overlap has to be a real one: two boxes that only share their edge do
/// not overlap. A word that stands directly next to the region — the next
/// word of the same line — is not part of the redaction, and removing it
/// would take away something the user did not mark.
///
/// The rule is the one of the common tools: a marked region removes every
/// word it touches, completely. The report names the words that were
/// removed, so that nobody is surprised by a word that goes further than the
/// rectangle suggested.
pub fn word_touches_region(word: &PdfTextWord, regions: &[RedactionRegion]) -> bool {
    // The boxes of the text layer are exact; the tolerance belongs to the
    // words of the picture, which the OCR estimates.
    regions.iter().any(|region| {
        let rect = region.as_rect();
        word.bbox.x0_milli_pt < rect.x1_milli_pt
            && word.bbox.x1_milli_pt > rect.x0_milli_pt
            && word.bbox.y0_milli_pt < rect.y1_milli_pt
            && word.bbox.y1_milli_pt > rect.y0_milli_pt
    })
}

/// Answers whether two boxes overlap.
pub fn boxes_overlap(first: PdfRectMilliPt, second: PdfRectMilliPt) -> bool {
    first.x0_milli_pt <= second.x1_milli_pt
        && first.x1_milli_pt >= second.x0_milli_pt
        && first.y0_milli_pt <= second.y1_milli_pt
        && first.y1_milli_pt >= second.y0_milli_pt
}

/// Extends the regions to the words they touch.
///
/// The removal works character by character. A region that covers only a part
/// of a word leaves the rest of the word behind — and the rest of the word
/// is still content. The region therefore grows to the box of every word it
/// touches, and the whole word is removed.
pub fn extend_regions_to_touching_words(
    regions: &[RedactionRegion],
    words: &[PdfTextWord],
) -> Vec<RedactionRegion> {
    regions
        .iter()
        .map(|region| {
            let mut extended = *region;
            for word in words {
                if !word_touches_region(word, &[*region]) {
                    continue;
                }
                extended.x0_milli_pt = extended.x0_milli_pt.min(word.bbox.x0_milli_pt);
                extended.y0_milli_pt = extended.y0_milli_pt.min(word.bbox.y0_milli_pt);
                extended.x1_milli_pt = extended.x1_milli_pt.max(word.bbox.x1_milli_pt);
                extended.y1_milli_pt = extended.y1_milli_pt.max(word.bbox.y1_milli_pt);
            }
            extended
        })
        .collect()
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
/// the first place: a word belongs to the region when the region touches it.
/// What was there before has to be gone afterwards — otherwise the region
/// still carries something that a reader can see.
pub fn word_remains_in_region(word: &PdfTextWord, regions: &[RedactionRegion]) -> bool {
    word_touches_region(word, regions)
}

/// Answers whether a word lies in the redacted region, enlarged by the
/// tolerance.
pub fn word_lies_in_region_tolerant(word: &PdfTextWord, regions: &[RedactionRegion]) -> bool {
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
        assert!(is_fragment_of("CH", "VERTRAULICH"));
        assert!(is_fragment_of("VERTRAU", "VERTRAULICH"));
        assert!(is_fragment_of("HEIM", "GEHEIM"));
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

    #[test]
    fn a_word_that_only_stands_next_to_the_region_is_not_removed() {
        // The region covers the first word of the line. The next word stands
        // directly next to it: it shares the edge of the boxes, and that is
        // not an overlap. The user did not mark it.
        let regions = [region(0, 0, 100, 200)];
        let next = word("Anhang", 100, 0, 400, 200);
        assert!(!word_touches_region(&next, &regions));
        let extended = extend_regions_to_touching_words(&regions, &[next]);
        assert_eq!(
            extended[0].x1_milli_pt, 100,
            "the region must not grow to the word next to it"
        );
    }

    #[test]
    fn a_region_grows_to_the_words_it_touches() {
        // The region covers the first half of the word. The word belongs to
        // the redaction, and the region grows to the whole word.
        let regions = [region(0, 0, 100, 200)];
        let words = [word("GEHEIM", 0, 0, 600, 200)];
        let extended = extend_regions_to_touching_words(&regions, &words);
        assert_eq!(extended.len(), 1);
        assert_eq!(extended[0].x0_milli_pt, 0);
        assert_eq!(extended[0].x1_milli_pt, 600);
    }
}
