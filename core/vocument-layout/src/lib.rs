//! Deterministic cross-platform layout engine for FlowDocument (`ADR-0020`, `ROADMAP.md` M4.1).

pub mod error;
pub mod font;
pub mod footnote;
pub mod geometry;
pub mod incremental;
pub mod line_breaker;
pub mod paginator;
pub mod paragraph;
pub mod shaper;
pub mod table;
pub mod tree;

pub use error::LayoutError;
pub use font::{Font, FontMetrics, FontRegistry};
pub use footnote::{PageFootnotes, layout_page_footnotes, measure_footnotes_height};
pub use geometry::{FixedPt, Insets, Point, Rect, Size};
pub use incremental::{IncrementalLayoutEngine, ParagraphFingerprint};
pub use line_breaker::{LineBreakerConfig, break_greedy, break_knuth_plass, break_paragraph_runs};
pub use paginator::{PaginatorOptions, paginate_document};
pub use paragraph::{
    PositionedParagraph, TypesettingContext, layout_heading, layout_list, layout_paragraph,
    resolve_paragraph_style, shape_spans,
};
pub use shaper::{Direction, ShapedGlyph, ShapedRun, shape_span};
pub use table::{
    PositionedTable, PositionedTableCell, PositionedTableRow, TableSlice, layout_table, slice_table,
};
pub use tree::{LayoutBox, LayoutBoxKind, LayoutDocument, LayoutLine, LayoutPage, PositionedRun};

/// Convenience function to typeset and paginate a `FlowDocument` using standard options.
pub fn layout_document(
    doc: &vocument_flow::model::FlowDocument,
    font_registry: &FontRegistry,
) -> LayoutDocument {
    paginate_document(doc, font_registry, &PaginatorOptions::default())
}

/// Convenience function to typeset and paginate a `FlowDocument` with explicit options.
pub fn layout_document_with_options(
    doc: &vocument_flow::model::FlowDocument,
    font_registry: &FontRegistry,
    options: &PaginatorOptions,
) -> LayoutDocument {
    paginate_document(doc, font_registry, options)
}

pub fn version() -> &'static str {
    "0.1.0"
}
