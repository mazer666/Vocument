//! Deterministic multi-page, multi-column pagination engine (`ADR-0020`, `AUTHOR-004`).

use vocument_flow::model::{
    Block, CalloutBlock, CalloutType, FlowDocument, ImageBlock, ParagraphBlock, Section,
    SectionGeometry, TableBlock,
};
use vocument_flow::styles::{ColorRgba, FontStyle, TextSpan};

use crate::font::FontRegistry;
use crate::footnote::{layout_page_footnotes, measure_footnotes_height};
use crate::geometry::{Insets, Rect, Size};
use crate::paragraph::{
    PositionedParagraph, TypesettingContext, layout_heading, layout_list, layout_paragraph,
};
use crate::table::{layout_table, slice_table};
use crate::tree::{LayoutBox, LayoutBoxKind, LayoutDocument, LayoutPage};

/// Pagination configuration options.
#[derive(Debug, Clone, Default)]
pub struct PaginatorOptions {
    pub use_knuth_plass: bool,
}

/// Global pagination orchestrator transforming a `FlowDocument` into a `LayoutDocument`.
pub fn paginate_document(
    doc: &FlowDocument,
    font_registry: &FontRegistry,
    options: &PaginatorOptions,
) -> LayoutDocument {
    let mut pages: Vec<LayoutPage> = Vec::new();

    for section in &doc.sections {
        let mut section_paginator =
            SectionPaginator::new(section, doc, font_registry, options, pages.len() + 1);
        let section_pages = section_paginator.paginate();
        pages.extend(section_pages);
    }

    if pages.is_empty() {
        let g = SectionGeometry::a4_portrait();
        let insets = Insets::new(
            g.margin_top_pt,
            g.margin_right_pt,
            g.margin_bottom_pt,
            g.margin_left_pt,
        );
        pages.push(LayoutPage::new(
            1,
            Size::new(g.width_pt, g.height_pt),
            insets,
        ));
    }

    let total_pages = pages.len();

    // Second pass: resolve page number placeholders in headers and footers
    for page in &mut pages {
        resolve_page_number_placeholders(&mut page.header_boxes, page.page_number, total_pages);
        resolve_page_number_placeholders(&mut page.footer_boxes, page.page_number, total_pages);
    }

    LayoutDocument::new(pages)
}

struct SectionPaginator<'a> {
    section: &'a Section,
    doc: &'a FlowDocument,
    font_registry: &'a FontRegistry,
    options: &'a PaginatorOptions,
    page_start_number: usize,
    pages: Vec<LayoutPage>,
    current_page_idx: usize,
    cursor_y: f32,
    page_footnote_ids: Vec<String>,
}

impl<'a> SectionPaginator<'a> {
    fn new(
        section: &'a Section,
        doc: &'a FlowDocument,
        font_registry: &'a FontRegistry,
        options: &'a PaginatorOptions,
        page_start_number: usize,
    ) -> Self {
        Self {
            section,
            doc,
            font_registry,
            options,
            page_start_number,
            pages: Vec::new(),
            current_page_idx: 0,
            cursor_y: 0.0,
            page_footnote_ids: Vec::new(),
        }
    }

    fn paginate(&mut self) -> Vec<LayoutPage> {
        self.start_new_page();

        for block in &self.section.blocks {
            self.paginate_block(block);
        }

        self.finalize_current_page();
        std::mem::take(&mut self.pages)
    }

    fn content_bounds(&self) -> (f32, f32, f32, f32) {
        let geom = &self.section.geometry;
        let x = geom.margin_left_pt;
        let y_top = geom.margin_top_pt;
        let w = (geom.width_pt - geom.margin_left_pt - geom.margin_right_pt).max(50.0);
        let y_bottom = (geom.height_pt - geom.margin_bottom_pt).max(y_top + 50.0);
        (x, y_top, w, y_bottom)
    }

    fn current_available_bottom(&self) -> f32 {
        let (_, _, w, y_bottom) = self.content_bounds();
        let fn_height = measure_footnotes_height(
            &self.page_footnote_ids,
            &self.doc.footnotes,
            &self.doc.stylesheet,
            self.font_registry,
            w,
        );
        y_bottom - fn_height
    }

    fn start_new_page(&mut self) {
        if !self.pages.is_empty() {
            self.finalize_current_page();
        }

        let geom = &self.section.geometry;
        let page_num = self.page_start_number + self.pages.len();
        let page_size = Size::new(geom.width_pt, geom.height_pt);
        let insets = Insets::new(
            geom.margin_top_pt,
            geom.margin_right_pt,
            geom.margin_bottom_pt,
            geom.margin_left_pt,
        );

        self.pages
            .push(LayoutPage::new(page_num, page_size, insets));
        self.current_page_idx = self.pages.len() - 1;
        self.cursor_y = geom.margin_top_pt;
        self.page_footnote_ids.clear();
    }

    fn finalize_current_page(&mut self) {
        if self.current_page_idx >= self.pages.len() {
            return;
        }

        let (content_x, content_y_top, content_w, content_y_bottom) = self.content_bounds();

        // 1. Layout Header
        if !self.section.header_blocks.is_empty() {
            let header_y = (content_y_top - 20.0).max(10.0);
            let h_boxes = self.layout_auxiliary_blocks(
                &self.section.header_blocks,
                content_x,
                header_y,
                content_w,
            );
            self.pages[self.current_page_idx].header_boxes = h_boxes;
        }

        // 2. Layout Footnotes
        if !self.page_footnote_ids.is_empty() {
            let fn_h = measure_footnotes_height(
                &self.page_footnote_ids,
                &self.doc.footnotes,
                &self.doc.stylesheet,
                self.font_registry,
                content_w,
            );
            let page_fn = layout_page_footnotes(
                &self.page_footnote_ids,
                &self.doc.footnotes,
                &self.doc.stylesheet,
                self.font_registry,
                content_x,
                content_y_bottom - fn_h,
                content_w,
            );
            self.pages[self.current_page_idx].footnote_boxes = page_fn.boxes;
        }

        // 3. Layout Footer
        if !self.section.footer_blocks.is_empty() {
            let f_boxes = self.layout_auxiliary_blocks(
                &self.section.footer_blocks,
                content_x,
                content_y_bottom + 8.0,
                content_w,
            );
            self.pages[self.current_page_idx].footer_boxes = f_boxes;
        }
    }

    fn layout_auxiliary_blocks(&self, blocks: &[Block], x: f32, y: f32, w: f32) -> Vec<LayoutBox> {
        let mut boxes = Vec::new();
        let mut curr_y = y;
        let ctx = TypesettingContext {
            stylesheet: &self.doc.stylesheet,
            font_registry: self.font_registry,
            available_width_pt: w,
            use_knuth_plass: false,
        };

        for b in blocks {
            if let Block::Paragraph(p) = b {
                let pos = layout_paragraph(p, &ctx);
                let bxs = pos.to_layout_boxes(x, curr_y);
                boxes.extend(bxs);
                curr_y += pos.total_height_pt + 2.0;
            }
        }
        boxes
    }

    fn paginate_block(&mut self, block: &Block) {
        let (content_x, content_y_top, content_w, _) = self.content_bounds();
        let ctx = TypesettingContext {
            stylesheet: &self.doc.stylesheet,
            font_registry: self.font_registry,
            available_width_pt: content_w,
            use_knuth_plass: self.options.use_knuth_plass,
        };

        match block {
            Block::Paragraph(p) => {
                let pos = layout_paragraph(p, &ctx);
                self.paginate_paragraph(pos, content_x, content_y_top);
            }
            Block::Heading(h) => {
                let pos = layout_heading(h, &ctx);
                self.paginate_heading(pos, content_x, content_y_top);
            }
            Block::List(l) => {
                let pos_list = layout_list(l, &ctx, 0);
                for item in pos_list {
                    self.paginate_paragraph(item, content_x, content_y_top);
                }
            }
            Block::Table(t) => {
                self.paginate_table(t, &ctx, content_x, content_y_top);
            }
            Block::Image(img) => {
                self.paginate_image(img, content_x, content_y_top, content_w);
            }
            Block::Callout(c) => {
                self.paginate_callout(c, &ctx, content_x, content_y_top, content_w);
            }
            Block::ThematicBreak(tb) => {
                self.paginate_thematic_break(tb, content_x, content_y_top, content_w);
            }
        }
    }

    fn paginate_paragraph(&mut self, pos: PositionedParagraph, x: f32, y_top: f32) {
        if pos.lines.is_empty() {
            return;
        }

        let at_page_top = (self.cursor_y - y_top).abs() < 1.0;
        let space_before = if at_page_top {
            0.0
        } else {
            pos.space_before_pt
        };
        let mut avail_bottom = self.current_available_bottom();

        // If entire paragraph fits with space before
        if self.cursor_y + space_before + pos.total_height_pt <= avail_bottom {
            self.cursor_y += space_before;
            let boxes = pos.to_layout_boxes(x, self.cursor_y);
            self.pages[self.current_page_idx]
                .content_boxes
                .extend(boxes);
            self.cursor_y += pos.total_height_pt + pos.space_after_pt;
            self.page_footnote_ids.extend(pos.footnote_refs);
            return;
        }

        // Does not fit completely: check keep_lines_together or orphan control
        if pos.keep_lines_together || pos.lines.len() <= 2 {
            self.start_new_page();
            let boxes = pos.to_layout_boxes(x, self.cursor_y);
            self.pages[self.current_page_idx]
                .content_boxes
                .extend(boxes);
            self.cursor_y += pos.total_height_pt + pos.space_after_pt;
            self.page_footnote_ids.extend(pos.footnote_refs);
            return;
        }

        // Paragraph splitting across pages
        self.cursor_y += space_before;
        avail_bottom = self.current_available_bottom();

        // Calculate how many lines fit on the current page
        let mut split_idx = 0;
        for (idx, line) in pos.lines.iter().enumerate() {
            if self.cursor_y + line.bounds.y + line.bounds.height > avail_bottom {
                break;
            }
            split_idx = idx + 1;
        }

        // Apply orphan control (minimum 2 lines on first page, minimum 2 on next page)
        if pos.orphan_control {
            if split_idx < 2 {
                // Orphan: cannot leave only 1 line, move entire paragraph to next page
                self.start_new_page();
                let boxes = pos.to_layout_boxes(x, self.cursor_y);
                self.pages[self.current_page_idx]
                    .content_boxes
                    .extend(boxes);
                self.cursor_y += pos.total_height_pt + pos.space_after_pt;
                self.page_footnote_ids.extend(pos.footnote_refs);
                return;
            } else if pos.lines.len() - split_idx < 2 {
                // Widow: next page would only have 1 line, pull one line back
                split_idx = pos.lines.len().saturating_sub(2);
            }
        }

        // Emit first slice of lines on current page
        let mut first_slice = pos.clone();
        first_slice.lines.truncate(split_idx);
        first_slice.footnote_refs.clear();
        first_slice.total_height_pt = first_slice
            .lines
            .last()
            .map(|l| l.bounds.y + l.bounds.height)
            .unwrap_or(0.0);
        let bxs1 = first_slice.to_layout_boxes(x, self.cursor_y);
        self.pages[self.current_page_idx].content_boxes.extend(bxs1);

        // Advance to new page for second slice
        self.start_new_page();
        let split_y = pos.lines[split_idx].bounds.y;
        let mut second_slice = pos;
        second_slice.lines = second_slice.lines.split_off(split_idx);
        for l in &mut second_slice.lines {
            l.bounds.y -= split_y;
        }
        second_slice.space_before_pt = 0.0;
        second_slice.total_height_pt -= split_y;

        let bxs2 = second_slice.to_layout_boxes(x, self.cursor_y);
        self.pages[self.current_page_idx].content_boxes.extend(bxs2);
        self.cursor_y += second_slice.total_height_pt + second_slice.space_after_pt;
        self.page_footnote_ids.extend(second_slice.footnote_refs);
    }

    fn paginate_heading(&mut self, pos: PositionedParagraph, x: f32, y_top: f32) {
        let at_page_top = (self.cursor_y - y_top).abs() < 1.0;
        let space_before = if at_page_top {
            0.0
        } else {
            pos.space_before_pt
        };
        let avail_bottom = self.current_available_bottom();

        // Heading keep_with_next rule: heading + at least 36pt (e.g. 2 lines of text) must fit
        let min_required = space_before + pos.total_height_pt + 36.0;

        if self.cursor_y + min_required > avail_bottom && !at_page_top {
            self.start_new_page();
        }

        self.cursor_y += space_before;
        let boxes = pos.to_layout_boxes(x, self.cursor_y);
        self.pages[self.current_page_idx]
            .content_boxes
            .extend(boxes);
        self.cursor_y += pos.total_height_pt + pos.space_after_pt;
        self.page_footnote_ids.extend(pos.footnote_refs);
    }

    fn paginate_table(&mut self, table: &TableBlock, ctx: &TypesettingContext, x: f32, y_top: f32) {
        let pos_table = layout_table(table, ctx);
        let mut curr_row = 0;
        let mut is_first = true;

        while curr_row < pos_table.rows.len() {
            let at_page_top = (self.cursor_y - y_top).abs() < 1.0;
            let space_before = if is_first && !at_page_top {
                pos_table.space_before_pt
            } else {
                0.0
            };

            let avail_bottom = self.current_available_bottom();
            let max_h = avail_bottom - (self.cursor_y + space_before);

            // If remaining height is too small for even 1 row and we aren't at page top, start fresh page
            if max_h < 40.0 && !at_page_top {
                self.start_new_page();
                continue;
            }

            self.cursor_y += space_before;
            let slice_opt = slice_table(
                &pos_table,
                curr_row,
                avail_bottom - self.cursor_y,
                is_first,
                x,
                self.cursor_y,
            );

            match slice_opt {
                Some(slice) => {
                    self.pages[self.current_page_idx]
                        .content_boxes
                        .extend(slice.boxes);
                    self.page_footnote_ids.extend(slice.footnote_refs);
                    self.cursor_y += slice.total_height_pt;
                    curr_row = slice.next_row;
                    is_first = false;

                    if !slice.is_complete {
                        self.start_new_page();
                    } else {
                        self.cursor_y += pos_table.space_after_pt;
                        break;
                    }
                }
                None => {
                    // Cannot fit on current page, advance to fresh page
                    self.start_new_page();
                }
            }
        }
    }

    fn paginate_image(&mut self, img: &ImageBlock, x: f32, y_top: f32, avail_width: f32) {
        let display_w = img
            .display_width_pt
            .unwrap_or(img.intrinsic_width_pt)
            .min(avail_width);
        let scale = if img.intrinsic_width_pt > 0.0 {
            display_w / img.intrinsic_width_pt
        } else {
            1.0
        };
        let display_h = img
            .display_height_pt
            .unwrap_or(img.intrinsic_height_pt * scale);

        let at_page_top = (self.cursor_y - y_top).abs() < 1.0;
        let space_before = if at_page_top { 0.0 } else { 8.0 };
        let avail_bottom = self.current_available_bottom();

        if self.cursor_y + space_before + display_h > avail_bottom && !at_page_top {
            self.start_new_page();
        }

        self.cursor_y += space_before;
        let img_rect = Rect::new(x, self.cursor_y, display_w, display_h);
        self.pages[self.current_page_idx]
            .content_boxes
            .push(LayoutBox::image(img.id.clone(), img_rect, &img.asset_ref));
        self.cursor_y += display_h + 8.0;
    }

    fn paginate_callout(
        &mut self,
        callout: &CalloutBlock,
        ctx: &TypesettingContext,
        x: f32,
        y_top: f32,
        avail_width: f32,
    ) {
        let padding = 10.0;
        let inner_width = avail_width - padding * 2.0;
        let inner_ctx = TypesettingContext {
            stylesheet: ctx.stylesheet,
            font_registry: ctx.font_registry,
            available_width_pt: inner_width,
            use_knuth_plass: ctx.use_knuth_plass,
        };

        let mut child_boxes = Vec::new();
        let mut inner_y = padding;

        if let Some(title) = &callout.title {
            let title_span = TextSpan {
                text: title.clone(),
                style: FontStyle::bold(),
                font_size_pt: Some(11.0),
                font_family: Some("Inter".to_string()),
                font_weight: Some(700),
                color: None,
                background_color: None,
                link_url: None,
                footnote_ref: None,
                extra: Default::default(),
            };
            let title_p = ParagraphBlock::new(format!("{}:title", callout.id), vec![title_span]);
            let pos = layout_paragraph(&title_p, &inner_ctx);
            child_boxes.extend(pos.to_layout_boxes(padding, inner_y));
            inner_y += pos.total_height_pt + 4.0;
        }

        for block in &callout.blocks {
            if let Block::Paragraph(p) = block {
                let pos = layout_paragraph(p, &inner_ctx);
                inner_y += pos.space_before_pt;
                child_boxes.extend(pos.to_layout_boxes(padding, inner_y));
                inner_y += pos.total_height_pt + pos.space_after_pt;
            }
        }

        let total_h = inner_y + padding;
        let at_page_top = (self.cursor_y - y_top).abs() < 1.0;
        let space_before = if at_page_top { 0.0 } else { 8.0 };
        let avail_bottom = self.current_available_bottom();

        if self.cursor_y + space_before + total_h > avail_bottom && !at_page_top {
            self.start_new_page();
        }

        self.cursor_y += space_before;
        let (bg, border) = match callout.callout_type {
            CalloutType::Note => (ColorRgba::rgb(240, 246, 255), ColorRgba::rgb(59, 130, 246)),
            CalloutType::Tip => (ColorRgba::rgb(240, 253, 244), ColorRgba::rgb(34, 197, 94)),
            CalloutType::Warning => (ColorRgba::rgb(255, 251, 235), ColorRgba::rgb(245, 158, 11)),
            CalloutType::Caution => (ColorRgba::rgb(254, 242, 242), ColorRgba::rgb(239, 68, 68)),
            CalloutType::LegalNotice => {
                (ColorRgba::rgb(248, 250, 252), ColorRgba::rgb(71, 85, 105))
            }
        };

        let mut children = vec![
            LayoutBox::solid_rect(
                format!("{}:bg", callout.id),
                Rect::new(0.0, 0.0, avail_width, total_h),
                bg,
            ),
            LayoutBox::border_rect(
                format!("{}:border", callout.id),
                Rect::new(0.0, 0.0, avail_width, total_h),
                1.0,
                border,
            ),
        ];
        children.extend(child_boxes);

        self.pages[self.current_page_idx]
            .content_boxes
            .push(LayoutBox::container(
                callout.id.clone(),
                Rect::new(x, self.cursor_y, avail_width, total_h),
                children,
            ));
        self.cursor_y += total_h + 8.0;
    }

    fn paginate_thematic_break(
        &mut self,
        tb: &vocument_flow::model::ThematicBreakBlock,
        x: f32,
        _y_top: f32,
        avail_width: f32,
    ) {
        self.cursor_y += 10.0;
        self.pages[self.current_page_idx]
            .content_boxes
            .push(LayoutBox::rule(
                tb.id.clone(),
                Rect::new(x, self.cursor_y, avail_width, 1.0),
                ColorRgba::rgb(210, 210, 210),
                1.0,
            ));
        self.cursor_y += 11.0;
    }
}

fn resolve_page_number_placeholders(boxes: &mut [LayoutBox], page_num: usize, total_pages: usize) {
    for b in boxes {
        match &mut b.kind {
            LayoutBoxKind::TextLine(line) => {
                for run in &mut line.runs {
                    if run.text.contains("{page}") || run.text.contains("{total_pages}") {
                        run.text = run
                            .text
                            .replace("{page}", &page_num.to_string())
                            .replace("{total_pages}", &total_pages.to_string());
                    }
                }
            }
            LayoutBoxKind::Container { children } => {
                resolve_page_number_placeholders(children, page_num, total_pages);
            }
            _ => {}
        }
    }
}
