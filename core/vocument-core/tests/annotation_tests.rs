//! Tests of the rail of the profession and of the refusal of a comment
//! (`ANNOT-001`).
//!
//! The rail is the thing that makes the notes of a document readable: every
//! colour means one thing. The core owns the rail, checks it, and refuses a
//! note that does not fit it — before the document is touched.

use vocument_core::annotations::{
    CommentError, JuristPalette, JuristPaletteEntry, prepare_comment,
};
use vocument_core::pdf_engine::{PdfCommentAnnotation, PdfRectMilliPt};

/// A note with a text and a key.
fn comment(key: &str, contents: &str) -> PdfCommentAnnotation {
    PdfCommentAnnotation {
        annotation_id: "comment_001".to_owned(),
        page_index: 0,
        rect: PdfRectMilliPt {
            x0_milli_pt: 10_000,
            y0_milli_pt: 10_000,
            x1_milli_pt: 24_000,
            y1_milli_pt: 24_000,
        },
        palette_key: key.to_owned(),
        color_rgb: [0, 0, 0],
        contents: contents.to_owned(),
        author: Some("Dr. Klar".to_owned()),
    }
}

#[test]
fn the_default_rail_carries_the_colours_of_the_profession() {
    let palette = JuristPalette::default_palette();
    assert!(
        palette.entries().len() >= 5,
        "the rail carries more than one colour: {:?}",
        palette.entries().len()
    );
    for entry in palette.entries() {
        assert!(!entry.key.trim().is_empty(), "every entry has a key");
        assert!(!entry.label.trim().is_empty(), "every entry has a name");
        assert!(
            !entry.meaning.trim().is_empty(),
            "every colour means something: {}",
            entry.key
        );
    }
    // Die Farbe kommt aus der Schiene, nicht vom Aufrufer.
    let prepared = prepare_comment(&palette, &comment("objection", "Einwand")).expect("the note");
    let objection = palette.entry("objection").expect("the rail knows the key");
    assert_eq!(prepared.color_rgb, objection.color_rgb);
}

#[test]
fn a_note_with_a_key_that_the_rail_does_not_carry_is_refused() {
    let palette = JuristPalette::default_palette();
    let error = prepare_comment(&palette, &comment("rosa", "Einwand")).expect_err("the note");
    assert_eq!(
        error,
        CommentError::UnknownPaletteKey {
            key: "rosa".to_owned()
        }
    );
}

#[test]
fn a_note_without_a_text_is_refused() {
    let palette = JuristPalette::default_palette();
    let error = prepare_comment(&palette, &comment("note", "   ")).expect_err("the note");
    assert_eq!(error, CommentError::EmptyText);
}

#[test]
fn a_note_without_an_area_is_refused() {
    let palette = JuristPalette::default_palette();
    let mut without_area = comment("note", "Ein Hinweis");
    without_area.rect = PdfRectMilliPt {
        x0_milli_pt: 10_000,
        y0_milli_pt: 10_000,
        x1_milli_pt: 10_000,
        y1_milli_pt: 10_000,
    };
    let error = prepare_comment(&palette, &without_area).expect_err("the note");
    assert_eq!(error, CommentError::EmptyRect);
}

#[test]
fn a_rail_without_a_colour_is_refused() {
    let error = JuristPalette::new(Vec::new()).expect_err("an empty rail");
    assert_eq!(error, vocument_core::annotations::PaletteError::Empty);
}

#[test]
fn a_rail_with_a_key_that_stands_twice_is_refused() {
    let entries = vec![
        JuristPaletteEntry::new("note", "Hinweis", "Ein Hinweis", [255, 230, 80]),
        JuristPaletteEntry::new("note", "Frage", "Eine Frage", [90, 160, 240]),
    ];
    let error = JuristPalette::new(entries).expect_err("the rail");
    assert_eq!(
        error,
        vocument_core::annotations::PaletteError::DuplicateKey {
            key: "note".to_owned()
        }
    );
}

#[test]
fn a_rail_with_an_entry_without_a_name_is_refused() {
    let entries = vec![JuristPaletteEntry::new(
        "note",
        "  ",
        "Ein Hinweis",
        [255, 230, 80],
    )];
    let error = JuristPalette::new(entries).expect_err("the rail");
    assert_eq!(
        error,
        vocument_core::annotations::PaletteError::IncompleteEntry {
            key: "note".to_owned()
        }
    );
}

#[test]
fn the_rail_of_the_user_may_replace_the_default() {
    let palette = JuristPalette::new(vec![JuristPaletteEntry::new(
        "house",
        "Hausfarbe",
        "Die Farbe des Hauses.",
        [12, 34, 56],
    )])
    .expect("the rail");
    let prepared = prepare_comment(&palette, &comment("house", "Ein Hinweis")).expect("the note");
    assert_eq!(prepared.color_rgb, [12, 34, 56]);
}
