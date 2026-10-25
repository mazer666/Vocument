//! Open `.pdoc` authored document format specification and container (`ARCHITECTURE.md` §25, `ADR-0020`, `SOURCE-001`).

pub mod assets;
pub mod container;
pub mod error;
pub mod manifest;
pub mod review;
pub mod security;

pub use assets::PdocAssetStore;
pub use container::{PdocContainer, PdocDocumentContent, PdocPackage};
pub use error::PdocError;
pub use manifest::{CURRENT_FORMAT_VERSION, PdocAssetEntry, PdocManifest};
pub use review::{
    PdocChangeType, PdocFlowComment, PdocFlowCommentReply, PdocReviewData, PdocTrackedChange,
};
pub use security::{validate_entry_limits, validate_entry_path};

pub use vocument_flow::*;

/// Current `.pdoc` container format schema version.
pub const PDOC_FORMAT_VERSION: &str = "1.0";
