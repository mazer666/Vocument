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

    /// A table cell has an invalid span (e.g. col_span = 0 or row_span = 0).
    #[error(
        "table '{table_id}' cell '{cell_id}' has invalid span: row_span={row_span}, col_span={col_span} (must be >= 1)"
    )]
    InvalidCellSpan {
        table_id: String,
        cell_id: String,
        row_span: u32,
        col_span: u32,
    },

    /// A table cell's row_span extends beyond the total row count of the table.
    #[error(
        "table '{table_id}' cell '{cell_id}' row_span {row_span} at row {row_index} exceeds total rows {total_rows}"
    )]
    RowSpanOutOfBounds {
        table_id: String,
        cell_id: String,
        row_index: usize,
        row_span: u32,
        total_rows: usize,
    },

    /// Multiple table cells overlap on the same grid coordinate.
    #[error(
        "table '{table_id}' has overlapping cells at row {row_index}, column {col_index} (cell '{cell_id}')"
    )]
    OverlappingTableCells {
        table_id: String,
        cell_id: String,
        row_index: usize,
        col_index: usize,
    },

    /// Table contains an incomplete, ragged, or non-rectangular grid.
    #[error("table '{table_id}' grid integrity failure: {details}")]
    InvalidTableGrid { table_id: String, details: String },

    /// A table cell contains no blocks.
    #[error("table cell '{cell_id}' is empty; must contain at least one content block")]
    EmptyTableCell { cell_id: String },

    /// Section geometry failed validation.
    #[error("invalid geometry in section '{section_id}': {details}")]
    InvalidGeometry { section_id: String, details: String },

    /// Image block dimensions failed validation.
    #[error("invalid image dimensions in '{image_id}': {details}")]
    InvalidImageDimensions { image_id: String, details: String },

    /// Typography or styling values failed validation.
    #[error("invalid typography in '{context}': {details}")]
    InvalidTypography { context: String, details: String },

    /// A block references an undefined style ID in the stylesheet.
    #[error("block '{block_id}' references unknown {style_type} style '{style_id}'")]
    UnknownStyle {
        style_type: String,
        style_id: String,
        block_id: String,
    },

    /// A style definition has an empty or whitespace-only identifier.
    #[error("empty or invalid identifier in {style_type} style")]
    EmptyStyleId { style_type: String },

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
