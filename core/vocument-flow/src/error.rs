//! Typed errors for FlowDocument model, schema validation, and serialization (`ADR-0020`, `AUTHOR-001`–`005`).

use thiserror::Error;

/// Typed error enumeration for FlowDocument operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FlowError {
    /// The document contains no sections.
    #[error("document has no sections; at least one section is required")]
    EmptyDocument,

    /// A section contains no blocks.
    #[error("section '{section_id}' is empty; each section must contain at least one block")]
    EmptySection { section_id: String },

    /// Heading level is outside the valid 1..=6 range.
    #[error("invalid heading level {level} on block '{block_id}'; level must be between 1 and 6")]
    InvalidHeadingLevel { block_id: String, level: u8 },

    /// A heading contains no text spans.
    #[error("heading '{block_id}' contains no text spans")]
    EmptyHeading { block_id: String },

    /// A paragraph contains no text spans.
    #[error("paragraph '{block_id}' contains no text spans")]
    EmptyParagraph { block_id: String },

    /// A list contains no list items.
    #[error("list '{block_id}' contains no items; at least one item is required")]
    EmptyList { block_id: String },

    /// A table contains no rows.
    #[error("table '{table_id}' contains no rows; at least one row is required")]
    EmptyTable { table_id: String },

    /// Table row columns do not match expected column count.
    #[error(
        "table '{table_id}' dimension mismatch on row {row_index}: expected {expected_cols} columns, got {actual_cols}"
    )]
    InvalidTableDimensions {
        table_id: String,
        row_index: usize,
        expected_cols: usize,
        actual_cols: usize,
    },

    /// An element identifier (block, section, or footnote) is duplicate.
    #[error("duplicate identifier '{id}' found in document")]
    DuplicateId { id: String },

    /// An element identifier is empty or whitespace.
    #[error("empty or invalid identifier '{id}' in element of type '{element_type}'")]
    EmptyId { element_type: String, id: String },

    /// A text span references a footnote that is not defined in the document.
    #[error("span in block '{block_id}' references unknown footnote '{ref_id}'")]
    InvalidFootnoteReference { block_id: String, ref_id: String },

    /// Footnote definition contains no content blocks.
    #[error("footnote '{footnote_id}' contains no content blocks")]
    EmptyFootnote { footnote_id: String },

    /// Callout block contains no child blocks.
    #[error("callout '{block_id}' contains no child blocks")]
    EmptyCallout { block_id: String },

    /// Document metadata failed validation.
    #[error("invalid metadata field '{field}': {details}")]
    InvalidMetadata { field: String, details: String },

    /// The schema version of the document container is unsupported.
    #[error("unsupported schema version '{version}'; supported version is '{expected}'")]
    UnsupportedSchemaVersion { version: String, expected: String },

    /// JSON serialization failed.
    #[error("JSON serialization error: {details}")]
    SerializationError { details: String },

    /// JSON deserialization failed.
    #[error("JSON deserialization error: {details}")]
    DeserializationError { details: String },
}
