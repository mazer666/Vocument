//! Text selection and cleaned plain-text copy (`OCRCOPY-001`, `OCRCOPY-002`, `ROADMAP.md` M0.3).
//!
//! Requirements:
//! - `OCRCOPY-001`: Raw text and cleaned readable text are both available.
//! - `OCRCOPY-002` (Preset): Cleaned readable text removes artificial line breaks and performs
//!   sensible dehyphenation while preserving paragraphs.

use crate::pdf_engine::{PdfPageStructuredText, PdfRectMilliPt, PdfTextLine};
use serde::{Deserialize, Serialize};

/// Result of selecting text on a page (by bounding box or full page) (`ROADMAP.md` M0.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextSelectionResult {
    pub page_index: u32,
    /// Raw extracted text preserving original PDF line breaks (`OCRCOPY-001`).
    pub raw_text: String,
    /// Cleaned readable plain text with artificial line breaks removed, soft word-wrap hyphens
    /// rejoined, and paragraphs separated by `\n\n` (`OCRCOPY-002`).
    pub cleaned_text: String,
    /// Bounding boxes (`PdfRectMilliPt`) of the selected lines for canvas highlight rendering.
    pub highlight_rects: Vec<PdfRectMilliPt>,
}

/// Selects all text lines on `page` (`ROADMAP.md` M0.3).
pub fn select_all_page_text(page: &PdfPageStructuredText) -> TextSelectionResult {
    build_selection_from_lines(page.page_index, &page.lines)
}

/// Selects all text lines on `page` whose bounding box intersects `selection_rect` (`ROADMAP.md` M0.3).
pub fn select_page_text_in_rect(
    page: &PdfPageStructuredText,
    selection_rect: PdfRectMilliPt,
) -> TextSelectionResult {
    let matching: Vec<PdfTextLine> = page
        .lines
        .iter()
        .filter(|line| line.bbox.intersects(&selection_rect))
        .cloned()
        .collect();
    build_selection_from_lines(page.page_index, &matching)
}

/// Builds both `raw_text` (`OCRCOPY-001`) and `cleaned_text` (`OCRCOPY-002`) from a slice of `PdfTextLine`.
pub fn build_selection_from_lines(page_index: u32, lines: &[PdfTextLine]) -> TextSelectionResult {
    let mut raw_text = String::new();
    let mut cleaned_paragraphs: Vec<String> = Vec::new();
    let mut current_para = String::new();
    let mut highlight_rects = Vec::with_capacity(lines.len());

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.text.trim();
        if trimmed.is_empty() {
            continue;
        }
        highlight_rects.push(line.bbox);

        // 1. Raw text preserves every line break and paragraph break:
        if !raw_text.is_empty() {
            raw_text.push('\n');
        }
        raw_text.push_str(trimmed);
        if line.ends_paragraph && idx + 1 < lines.len() {
            raw_text.push('\n');
        }

        // 2. Cleaned readable text (OCRCOPY-002):
        append_line_to_cleaned_paragraph(&mut current_para, trimmed);

        if line.ends_paragraph {
            let finished = current_para.trim().to_owned();
            if !finished.is_empty() {
                cleaned_paragraphs.push(finished);
            }
            current_para.clear();
        }
    }

    let trailing = current_para.trim().to_owned();
    if !trailing.is_empty() {
        cleaned_paragraphs.push(trailing);
    }

    TextSelectionResult {
        page_index,
        raw_text,
        cleaned_text: cleaned_paragraphs.join("\n\n"),
        highlight_rects,
    }
}

/// Appends `next_line` into `paragraph`, performing sensible dehyphenation (`OCRCOPY-002`):
/// - If `paragraph` ends with a word hyphen (`'-'` preceded by a lowercase ASCII/Unicode letter)
///   and `next_line` starts with a lowercase letter (e.g. `"trust-"` + `"worthy"` -> `"trustworthy"`),
///   the trailing hyphen is stripped and the word is joined directly.
/// - Otherwise, lines within the same paragraph are joined with a single space.
fn append_line_to_cleaned_paragraph(paragraph: &mut String, next_line: &str) {
    if paragraph.is_empty() {
        paragraph.push_str(next_line);
        return;
    }

    let next_starts_lowercase = next_line.chars().next().is_some_and(|c| c.is_lowercase());

    if next_starts_lowercase
        && let Some(without_hyphen) = paragraph
            .strip_suffix('-')
            .or_else(|| paragraph.strip_suffix('\u{00AD}'))
        && without_hyphen
            .chars()
            .last()
            .is_some_and(|c| c.is_alphabetic())
    {
        // Sensible dehyphenation: preserve well-known compound prefixes like "local-" or "offline-"
        let last_word = without_hyphen
            .split_whitespace()
            .last()
            .unwrap_or("")
            .to_ascii_lowercase();
        let keep_compound_hyphen = matches!(
            last_word.as_str(),
            "local" | "offline" | "cross" | "self" | "multi" | "non" | "co" | "re"
        );
        if keep_compound_hyphen {
            paragraph.push_str(next_line);
        } else {
            let len_without = without_hyphen.len();
            paragraph.truncate(len_without);
            paragraph.push_str(next_line);
        }
        return;
    }

    paragraph.push(' ');
    paragraph.push_str(next_line);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_bbox(y0: i32) -> PdfRectMilliPt {
        PdfRectMilliPt {
            x0_milli_pt: 72_000,
            y0_milli_pt: y0,
            x1_milli_pt: 500_000,
            y1_milli_pt: y0 + 12_000,
        }
    }

    #[test]
    fn cleans_line_breaks_dehyphenates_words_and_preserves_paragraphs() {
        let page = PdfPageStructuredText {
            page_index: 1,
            embedded_page_label: Some("ii".to_owned()),
            words: vec![],
            lines: vec![
                PdfTextLine {
                    bbox: dummy_bbox(100_000),
                    text: "Vocument provides trust-".to_owned(),
                    ends_paragraph: false,
                },
                PdfTextLine {
                    bbox: dummy_bbox(116_000),
                    text: "worthy offline document workflows".to_owned(),
                    ends_paragraph: false,
                },
                PdfTextLine {
                    bbox: dummy_bbox(132_000),
                    text: "across macOS and Android.".to_owned(),
                    ends_paragraph: true,
                },
                PdfTextLine {
                    bbox: dummy_bbox(168_000),
                    text: "Second paragraph remains separated.".to_owned(),
                    ends_paragraph: true,
                },
            ],
        };

        let sel = select_all_page_text(&page);
        assert_eq!(sel.highlight_rects.len(), 4);
        assert!(sel.raw_text.contains("trust-\nworthy"));
        assert_eq!(
            sel.cleaned_text,
            "Vocument provides trustworthy offline document workflows across macOS and Android.\n\nSecond paragraph remains separated."
        );

        // Rect selection covering only the first 3 lines:
        let rect_sel = select_page_text_in_rect(
            &page,
            PdfRectMilliPt {
                x0_milli_pt: 70_000,
                y0_milli_pt: 95_000,
                x1_milli_pt: 400_000,
                y1_milli_pt: 140_000,
            },
        );
        assert_eq!(rect_sel.highlight_rects.len(), 3);
        assert_eq!(
            rect_sel.cleaned_text,
            "Vocument provides trustworthy offline document workflows across macOS and Android."
        );
    }
}
