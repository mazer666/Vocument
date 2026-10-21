//! The jurist's comment and the palette of the profession (`ANNOT-001`).
//!
//! A reviewer does not write free text on a document: every note carries a
//! **meaning**. "This needs to be clarified", "I disagree", "agreed", "a
//! question to the other party" — the colour of the note says what the note
//! means, and the palette is the rail that makes the meaning readable at a
//! glance.
//!
//! The palette therefore lives in the core and not in the interface: the
//! engine gets a colour, the core decides which colours exist and what they
//! mean. The user may edit the palette — the default is a starting point, not
//! a law.
//!
//! The comment itself is a standard PDF `/Text` annotation with a popup. It
//! is written into the document, it survives the save, and other tools can
//! read it.

use serde::{Deserialize, Serialize};

use crate::pdf_engine::{PdfCommentAnnotation, PdfEngineError};
use crate::session::SessionError;
use crate::transactions::{TransactionCommand, TransactionRecord};

/// One entry of the palette of the profession (`ANNOT-001`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JuristPaletteEntry {
    /// Stable key of the entry (`"note"`, `"objection"`, …). The key is what
    /// the caller sends; the colour is looked up in the palette.
    pub key: String,
    /// The name the interface shows.
    pub label: String,
    /// What the colour means in the review — the text of the rail.
    pub meaning: String,
    /// The colour of the note, RGB.
    pub color_rgb: [u8; 3],
}

impl JuristPaletteEntry {
    /// A new entry of the palette.
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        meaning: impl Into<String>,
        color_rgb: [u8; 3],
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            meaning: meaning.into(),
            color_rgb,
        }
    }
}

/// The palette of the profession: the rail of the reviewer (`ANNOT-001`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JuristPalette {
    entries: Vec<JuristPaletteEntry>,
}

/// The error of the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteError {
    /// The palette is empty — a rail without a colour is no rail.
    Empty,
    /// Two entries carry the same key.
    DuplicateKey { key: String },
    /// An entry has no key or no name.
    IncompleteEntry { key: String },
}

impl std::fmt::Display for PaletteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "the palette of the profession is empty"),
            Self::DuplicateKey { key } => {
                write!(f, "the palette carries the key '{key}' more than once")
            }
            Self::IncompleteEntry { key } => write!(
                f,
                "the entry '{key}' has no name — a note without a name cannot be shown"
            ),
        }
    }
}

impl std::error::Error for PaletteError {}

impl JuristPalette {
    /// The default rail of the reviewer (`ANNOT-001`).
    ///
    /// The colours are the ones a reviewer expects: a note to clarify, a
    /// question, an objection, an agreement, a reference to a rule, and a
    /// neutral note.
    pub fn default_palette() -> Self {
        Self {
            entries: vec![
                JuristPaletteEntry::new(
                    "note",
                    "To be clarified",
                    "A point that has to be clarified before the document is signed.",
                    [255, 230, 80],
                ),
                JuristPaletteEntry::new(
                    "question",
                    "Question to the other party",
                    "A question that the other party has to answer.",
                    [90, 160, 240],
                ),
                JuristPaletteEntry::new(
                    "objection",
                    "Objection",
                    "A point that the reviewer disputes.",
                    [235, 90, 80],
                ),
                JuristPaletteEntry::new(
                    "agreed",
                    "Agreed",
                    "A point that both sides have agreed on.",
                    [110, 200, 130],
                ),
                JuristPaletteEntry::new(
                    "citation",
                    "Reference to a rule",
                    "A reference to a rule, a case or a paragraph.",
                    [180, 140, 230],
                ),
                JuristPaletteEntry::new(
                    "information",
                    "Information",
                    "A note for the reader that asks for no action.",
                    [170, 170, 170],
                ),
            ],
        }
    }

    /// A palette of the given entries.
    pub fn new(entries: Vec<JuristPaletteEntry>) -> Result<Self, PaletteError> {
        let palette = Self { entries };
        palette.validate()?;
        Ok(palette)
    }

    /// The entries of the rail.
    pub fn entries(&self) -> &[JuristPaletteEntry] {
        &self.entries
    }

    /// Looks up the colour of a key.
    pub fn color_of(&self, key: &str) -> Option<[u8; 3]> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.color_rgb)
    }

    /// Looks up the entry of a key.
    pub fn entry(&self, key: &str) -> Option<&JuristPaletteEntry> {
        self.entries.iter().find(|entry| entry.key == key)
    }

    /// Checks that the rail is usable: it has entries, every entry has a key
    /// and a name, and no key stands twice.
    pub fn validate(&self) -> Result<(), PaletteError> {
        if self.entries.is_empty() {
            return Err(PaletteError::Empty);
        }
        let mut seen: Vec<&str> = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            if entry.key.trim().is_empty() {
                return Err(PaletteError::IncompleteEntry {
                    key: entry.key.clone(),
                });
            }
            if entry.label.trim().is_empty() {
                return Err(PaletteError::IncompleteEntry {
                    key: entry.key.clone(),
                });
            }
            if seen.contains(&entry.key.as_str()) {
                return Err(PaletteError::DuplicateKey {
                    key: entry.key.clone(),
                });
            }
            seen.push(entry.key.as_str());
        }
        Ok(())
    }
}

/// The error of a comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentError {
    /// The note has no text — an empty note is not a note.
    EmptyText,
    /// The key is not on the rail.
    UnknownPaletteKey { key: String },
    /// The box of the note is empty.
    EmptyRect,
}

impl std::fmt::Display for CommentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyText => write!(f, "the comment carries no text"),
            Self::UnknownPaletteKey { key } => {
                write!(f, "the palette does not carry the key '{key}'")
            }
            Self::EmptyRect => write!(f, "the comment has no area on the page"),
        }
    }
}

impl std::error::Error for CommentError {}

/// Checks a comment against the rail and returns the note that the engine
/// may write (`ANNOT-001`).
///
/// The core decides: the colour comes from the palette, not from the caller,
/// and a note without a text or without an area is refused here and not in
/// the engine.
pub fn prepare_comment(
    palette: &JuristPalette,
    annotation: &PdfCommentAnnotation,
) -> Result<PdfCommentAnnotation, CommentError> {
    if !annotation.has_text() {
        return Err(CommentError::EmptyText);
    }
    if annotation.rect.x1_milli_pt <= annotation.rect.x0_milli_pt
        || annotation.rect.y1_milli_pt <= annotation.rect.y0_milli_pt
    {
        return Err(CommentError::EmptyRect);
    }
    let color = palette.color_of(&annotation.palette_key).ok_or_else(|| {
        CommentError::UnknownPaletteKey {
            key: annotation.palette_key.clone(),
        }
    })?;
    let mut prepared = annotation.clone();
    prepared.color_rgb = color;
    Ok(prepared)
}

/// The error of the engine when a comment cannot be written.
pub fn comment_engine_error(error: &PdfEngineError) -> String {
    format!("the comment could not be written: {error}")
}

impl crate::session::SessionRegistry {
    /// The rail of the profession: the palette the comments are written with
    /// (`ANNOT-001`).
    pub fn jurist_palette(&self) -> JuristPalette {
        self.jurist_palette
            .lock()
            .map(|palette| palette.clone())
            .unwrap_or_else(|_| JuristPalette::default_palette())
    }

    /// Replaces the rail of the profession with the palette of the user
    /// (`ANNOT-001`).
    ///
    /// The palette is checked before it is taken: a rail without a colour, a
    /// name or a unique key is refused here, so that a comment can always be
    /// written.
    pub fn set_jurist_palette(&self, palette: JuristPalette) -> Result<(), PaletteError> {
        palette.validate()?;
        let mut guard = self
            .jurist_palette
            .lock()
            .map_err(|_| PaletteError::Empty)?;
        *guard = palette;
        Ok(())
    }
}

impl crate::session::SessionRegistry {
    /// Queries all comments of the reviewer on `page_index` (`ANNOT-001`).
    pub fn page_comment_annotations(
        &self,
        session_id: &str,
        page_index: u32,
    ) -> Result<Vec<PdfCommentAnnotation>, SessionError> {
        let actor = {
            let guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard
                .get(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
            session.actor()
        };
        let comments = actor.page_comment_annotations(page_index)?;
        // Die Schiene kennt die Farbe, das PDF nur die Farbe. Der Schlüssel
        // wird beim Lesen nachgeschlagen: eine Notiz, die nicht auf der
        // Schiene steht, bekommt keinen Schlüssel und damit auch keine
        // Bedeutung — der Aufrufer sieht die Farbe.
        let palette = self.jurist_palette();
        Ok(comments
            .into_iter()
            .map(|mut comment| {
                // The key travels with the note. A note that carries none — a
                // note of another tool — is looked up by its colour.
                if comment.palette_key.is_empty() {
                    comment.palette_key = palette
                        .entries()
                        .iter()
                        .find(|entry| entry.color_rgb == comment.color_rgb)
                        .map(|entry| entry.key.clone())
                        .unwrap_or_default();
                }
                comment
            })
            .collect())
    }

    /// Writes a comment of the reviewer via `TransactionManager` (`ANNOT-001`).
    ///
    /// The core decides: the colour of the note comes from the rail of the
    /// profession, a note without a text is refused, and a key that the rail
    /// does not carry is refused. The engine only writes what it is told.
    pub fn add_comment_annotation(
        &self,
        session_id: &str,
        annotation: PdfCommentAnnotation,
    ) -> Result<TransactionRecord, SessionError> {
        let palette = self.jurist_palette();
        let prepared =
            crate::annotations::prepare_comment(&palette, &annotation).map_err(|error| {
                SessionError::CommentInvalid {
                    details: error.to_string(),
                }
            })?;
        self.execute_transaction(
            session_id,
            TransactionCommand::AddCommentAnnotation {
                annotation: prepared,
            },
        )
    }

    /// Removes a comment of the reviewer via `TransactionManager` (`ANNOT-001`).
    pub fn remove_comment_annotation(
        &self,
        session_id: &str,
        page_index: u32,
        annotation_id: &str,
    ) -> Result<TransactionRecord, SessionError> {
        self.execute_transaction(
            session_id,
            TransactionCommand::RemoveCommentAnnotation {
                page_index,
                annotation_id: annotation_id.to_owned(),
                restored_annotation: None,
            },
        )
    }
}
