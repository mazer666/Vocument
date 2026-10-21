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

/// The rail of the user has to survive the restart of the application: it is
/// written to a file and read back.
#[test]
fn the_rail_of_the_user_survives_the_restart() {
    let directory =
        std::env::temp_dir().join(format!("vocument_palette_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let path = directory.join("jurist_palette.json");
    let store = vocument_core::annotations_store::JuristPaletteStore::new(&path);

    // No file yet: the rail of the user does not exist.
    assert_eq!(store.load().expect("the store must be readable"), None);

    let palette = JuristPalette::new(vec![
        JuristPaletteEntry::new("house", "Hausfarbe", "Die Farbe des Hauses.", [12, 34, 56]),
        JuristPaletteEntry::new("note", "Hinweis", "Ein Hinweis.", [255, 230, 80]),
    ])
    .expect("the rail");
    store.save(&palette).expect("the rail must be written");

    let loaded = store
        .load()
        .expect("the store must be readable")
        .expect("the rail must be there");
    assert_eq!(loaded, palette, "the rail comes back unchanged");
}

#[test]
fn the_file_of_the_rail_belongs_to_the_user_alone() {
    let directory =
        std::env::temp_dir().join(format!("vocument_palette_mode_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let path = directory.join("jurist_palette.json");
    let store = vocument_core::annotations_store::JuristPaletteStore::new(&path);
    let palette = JuristPalette::default_palette();
    store.save(&palette).expect("the rail must be written");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path)
            .expect("the file")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o077,
            0,
            "the file of the rail must not be readable for others: {mode:o}"
        );
    }
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_rail_of_a_newer_format_is_not_read() {
    let directory =
        std::env::temp_dir().join(format!("vocument_palette_version_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the directory");
    let path = directory.join("jurist_palette.json");
    std::fs::write(
        &path,
        r#"{"format_version": 99, "entries": [], "modified_unix_ms": 0}"#,
    )
    .expect("the file");

    let store = vocument_core::annotations_store::JuristPaletteStore::new(&path);
    let error = store
        .load()
        .expect_err("a rail of an unknown version is refused");
    assert!(
        error.to_string().contains("99"),
        "the error names the version: {error}"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_broken_rail_is_an_error_and_not_a_silent_standard_rail() {
    let directory =
        std::env::temp_dir().join(format!("vocument_palette_broken_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the directory");
    let path = directory.join("jurist_palette.json");
    std::fs::write(&path, "{ this is not json").expect("the file");

    let store = vocument_core::annotations_store::JuristPaletteStore::new(&path);
    let error = store.load().expect_err("a broken rail is an error");
    assert!(
        error.to_string().contains("jurist_palette.json"),
        "the error names the file: {error}"
    );
    let _ = std::fs::remove_dir_all(&directory);
}
