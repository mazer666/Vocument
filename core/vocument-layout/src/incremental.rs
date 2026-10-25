//! Incremental layout caching and paragraph-local re-breaking (< 16ms budget, `ADR-0020`, `UI-009`).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use vocument_flow::model::{FlowDocument, ParagraphBlock};

use crate::font::FontRegistry;
use crate::paginator::{PaginatorOptions, paginate_document};
use crate::paragraph::{PositionedParagraph, TypesettingContext, layout_paragraph};
use crate::tree::LayoutDocument;

/// Fingerprint key identifying a paragraph's content and styling state.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParagraphFingerprint {
    pub block_id: String,
    pub content_hash: u64,
    pub width_fixed: i32,
    pub style_id: Option<String>,
}

impl ParagraphFingerprint {
    pub fn compute(para: &ParagraphBlock, width_pt: f32) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        para.id.hash(&mut hasher);
        for span in &para.spans {
            span.text.hash(&mut hasher);
            span.font_size_pt.map(|s| s.to_bits()).hash(&mut hasher);
            span.font_family.hash(&mut hasher);
            span.font_weight.hash(&mut hasher);
            span.style.bold.hash(&mut hasher);
            span.style.italic.hash(&mut hasher);
        }
        let width_fixed = (width_pt * 64.0).round() as i32;

        Self {
            block_id: para.id.clone(),
            content_hash: hasher.finish(),
            width_fixed,
            style_id: para.style_id.clone(),
        }
    }
}

/// An incremental layout manager maintaining paragraph caches and fast in-place mutations.
#[derive(Debug, Default)]
pub struct IncrementalLayoutEngine {
    paragraph_cache: HashMap<ParagraphFingerprint, PositionedParagraph>,
    last_layout: Option<LayoutDocument>,
}

impl IncrementalLayoutEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Performs full pagination and initializes the incremental paragraph cache.
    pub fn layout_full(
        &mut self,
        doc: &FlowDocument,
        font_registry: &FontRegistry,
        options: &PaginatorOptions,
    ) -> LayoutDocument {
        let layout_doc = paginate_document(doc, font_registry, options);
        self.last_layout = Some(layout_doc.clone());
        self.prime_cache(doc, font_registry);
        layout_doc
    }

    /// Incrementally updates layout after editing a single paragraph.
    ///
    /// If the paragraph's typeset height is identical to its previous height,
    /// the layout tree is updated in-place without triggering a global reflow (< 16ms).
    /// If the height changes, a fast reflow is performed.
    pub fn update_paragraph(
        &mut self,
        doc: &FlowDocument,
        modified_para: &ParagraphBlock,
        section_idx: usize,
        font_registry: &FontRegistry,
        options: &PaginatorOptions,
    ) -> LayoutDocument {
        let geom = if let Some(sec) = doc.sections.get(section_idx) {
            &sec.geometry
        } else {
            return self.layout_full(doc, font_registry, options);
        };

        let content_width = (geom.width_pt - geom.margin_left_pt - geom.margin_right_pt).max(50.0);
        let fp = ParagraphFingerprint::compute(modified_para, content_width);

        // Check if paragraph was previously typeset with known height
        let prev_pos = self.paragraph_cache.get(&fp).cloned();

        let ctx = TypesettingContext {
            stylesheet: &doc.stylesheet,
            font_registry,
            available_width_pt: content_width,
            use_knuth_plass: options.use_knuth_plass,
        };

        let new_pos = layout_paragraph(modified_para, &ctx);
        let height_delta = prev_pos
            .as_ref()
            .map(|p| (p.total_height_pt - new_pos.total_height_pt).abs())
            .unwrap_or(f32::MAX);

        // Update cache entry
        self.paragraph_cache.insert(fp, new_pos.clone());

        // In-place replacement if height is preserved (< 1.0pt difference) and last layout exists
        if height_delta < 1.0 && self.last_layout.is_some() {
            if let Some(layout) = &mut self.last_layout {
                let replaced = replace_paragraph_boxes(layout, modified_para.id.as_str(), &new_pos);
                if replaced {
                    return layout.clone();
                }
            }
        }

        // Height changed or box not found: perform full fast reflow
        self.layout_full(doc, font_registry, options)
    }

    fn prime_cache(&mut self, doc: &FlowDocument, font_registry: &FontRegistry) {
        for section in &doc.sections {
            let width = (section.geometry.width_pt
                - section.geometry.margin_left_pt
                - section.geometry.margin_right_pt)
                .max(50.0);

            let ctx = TypesettingContext {
                stylesheet: &doc.stylesheet,
                font_registry,
                available_width_pt: width,
                use_knuth_plass: false,
            };

            for block in &section.blocks {
                if let vocument_flow::model::Block::Paragraph(p) = block {
                    let fp = ParagraphFingerprint::compute(p, width);
                    let pos = layout_paragraph(p, &ctx);
                    self.paragraph_cache.insert(fp, pos);
                }
            }
        }
    }
}

fn replace_paragraph_boxes(
    layout: &mut LayoutDocument,
    block_id: &str,
    new_pos: &PositionedParagraph,
) -> bool {
    for page in &mut layout.pages {
        let mut first_match_idx = None;
        let mut match_count = 0;
        let mut base_y = 0.0;
        let mut base_x = page.margin_insets.left;

        for (idx, b) in page.content_boxes.iter().enumerate() {
            if b.id.starts_with(block_id) {
                if first_match_idx.is_none() {
                    first_match_idx = Some(idx);
                    base_y = b.bounds.y;
                    base_x = b.bounds.x;
                }
                match_count += 1;
            }
        }

        if let Some(start_idx) = first_match_idx {
            let new_boxes = new_pos.to_layout_boxes(base_x, base_y);
            page.content_boxes
                .splice(start_idx..(start_idx + match_count), new_boxes);
            return true;
        }
    }
    false
}
