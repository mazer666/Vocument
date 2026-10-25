//! Vocument FlowDocument Core Model and Schema (`ADR-0020`, `AUTHOR-001`–`AUTHOR-005`, `SOURCE-001`).
//!
//! Provides a pure-Rust, platform-independent semantic representation of authored, reflowable
//! documents independent of fixed-layout PDF syntax.
//!
//! # Architecture
//!
//! ```text
//! FlowDocument
//! ├─ DocumentMetadata (Dublin Core attributes, timestamps, UUID)
//! ├─ Stylesheet (ParagraphStyles, CharacterStyles, TableStyles, ListStyles)
//! ├─ Sections: Vec<Section>
//! │   ├─ SectionGeometry (page size, orientation, margins, columns)
//! │   └─ Blocks: Vec<Block>
//! │       ├─ Paragraph (inline spans, formatting, alignment)
//! │       ├─ Heading (levels 1–6, numbering prefix)
//! │       ├─ List (ordered, unordered, task lists, nested lists)
//! │       ├─ Table (cells, rows, rowspans, colspans, repeating headers)
//! │       ├─ Image (asset reference, intrinsic dimensions, caption, alt-text)
//! │       ├─ Callout (note, tip, warning, caution, legal alert)
//! │       └─ ThematicBreak (horizontal rule)
//! └─ Footnotes: BTreeMap<String, FootnoteDefinition>
//! ```

pub mod error;
pub mod metadata;
pub mod model;
pub mod review;
pub mod schema;
pub mod serde_flow;
pub mod styles;

pub use error::FlowError;
pub use metadata::{Author, DocumentMetadata};
pub use model::{
    Block, CalloutBlock, CalloutType, FlowDocument, FootnoteDefinition, HeadingBlock, ImageBlock,
    ListBlock, ListItem, ListKind, PageOrientation, ParagraphBlock, Section, SectionGeometry,
    TableBlock, TableCell, TableRow, ThematicBreakBlock,
};
pub use review::{ChangeType, FlowComment, FlowCommentReply, FlowReviewData, TrackedChange};
pub use schema::{FLOW_SCHEMA_VERSION, MAX_HEADING_LEVEL, MIN_HEADING_LEVEL, validate_document};
pub use serde_flow::{from_json_bytes, from_json_str, to_json_canonical, to_json_pretty};
pub use styles::{
    Alignment, CharacterStyle, ColorRgba, FontStyle, ListStyle, ParagraphStyle, Stylesheet,
    TableStyle, TextSpan,
};
