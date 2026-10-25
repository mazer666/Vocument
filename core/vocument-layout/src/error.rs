//! Typed errors for deterministic layout, typography, and pagination (`ADR-0020`, `AUTHOR-004`).

use thiserror::Error;

/// Typed error enumeration for layout and pagination operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LayoutError {
    /// An error originated in the FlowDocument model or schema validation.
    #[error("flow document error: {0}")]
    Flow(#[from] vocument_flow::error::FlowError),

    /// A font parsing, glyph shaping, or metric error occurred.
    #[error("font error in family '{family}': {details}")]
    FontError { family: String, details: String },

    /// Layout geometry constraints were violated.
    #[error("geometry error: {details}")]
    GeometryError { details: String },

    /// An error occurred during page breaking or pagination assembly.
    #[error("pagination error on page {page}: {details}")]
    PaginationError { page: usize, details: String },

    /// An asset or image referenced by an image block could not be located.
    #[error("asset not found: {asset_ref}")]
    AssetNotFound { asset_ref: String },
}
