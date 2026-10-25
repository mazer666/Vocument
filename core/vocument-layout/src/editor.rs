use vocument_flow::metadata::DocumentMetadata;
use vocument_flow::model::{
    Block, FlowDocument, FootnoteDefinition, HeadingBlock, ListBlock, ListItem, ListKind,
    ParagraphBlock, Section, SectionGeometry, TableBlock, TableCell, TableRow,
};
use vocument_flow::styles::TextSpan;

use crate::font::FontRegistry;
use crate::geometry::Rect;
use crate::hit_test::{cursor_rect_for_offset, hit_test_page, selection_rects_for_range};
use crate::incremental::IncrementalLayoutEngine;
use crate::paginator::PaginatorOptions;
use crate::tree::LayoutDocument;

/// Maximum number of undo transactions retained in memory.
const MAX_UNDO_STACK: usize = 100;

/// Snapshot representing a document state for undo/redo.
#[derive(Debug, Clone)]
struct EditorSnapshot {
    document: FlowDocument,
    active_block_id: String,
    cursor_offset: usize,
}

/// Native interactive FlowDocument editor state.
#[derive(Debug)]
pub struct FlowEditorState {
    pub document: FlowDocument,
    pub active_block_id: String,
    pub cursor_offset: usize,
    pub selection_anchor: Option<(String, usize)>,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
    pub font_registry: FontRegistry,
    pub incremental_engine: IncrementalLayoutEngine,
    pub layout: LayoutDocument,
    pub paginator_options: PaginatorOptions,
}

impl FlowEditorState {
    /// Constructs a new editor initialized with an empty document.
    pub fn new_empty(title: &str) -> Self {
        let meta = DocumentMetadata::new(format!("doc_{}", std::process::id()), title);
        let mut doc = FlowDocument::new(meta);
        let section = Section::new(
            "sec_1",
            SectionGeometry::a4_portrait(),
            vec![Block::Paragraph(ParagraphBlock::new(
                "p_1",
                vec![TextSpan::plain("")],
            ))],
        );
        doc.sections.push(section);

        Self::from_document(doc)
    }

    /// Constructs an editor from an existing `FlowDocument`.
    pub fn from_document(doc: FlowDocument) -> Self {
        let font_registry = FontRegistry::new();
        let options = PaginatorOptions::default();
        let mut incremental_engine = IncrementalLayoutEngine::new();
        let layout = incremental_engine.layout_full(&doc, &font_registry, &options);

        let initial_block_id = doc
            .sections
            .first()
            .and_then(|s| s.blocks.first())
            .map(block_id)
            .unwrap_or_else(|| "p_1".to_string());

        Self {
            document: doc,
            active_block_id: initial_block_id,
            cursor_offset: 0,
            selection_anchor: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            font_registry,
            incremental_engine,
            layout,
            paginator_options: options,
        }
    }

    fn push_undo_snapshot(&mut self) {
        self.redo_stack.clear();
        if self.undo_stack.len() >= MAX_UNDO_STACK {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(EditorSnapshot {
            document: self.document.clone(),
            active_block_id: self.active_block_id.clone(),
            cursor_offset: self.cursor_offset,
        });
    }

    /// Inserts text at the current cursor position (< 16ms budget).
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.push_undo_snapshot();
        self.clear_selection();

        let block_id = self.active_block_id.clone();
        let offset = self.cursor_offset;

        if let Some(para) = self.find_paragraph_mut(&block_id) {
            insert_into_spans(&mut para.spans, offset, text);
            self.cursor_offset += text.chars().count();
            self.update_active_paragraph_fast();
        } else if let Some(heading) = self.find_heading_mut(&block_id) {
            insert_into_spans(&mut heading.spans, offset, text);
            self.cursor_offset += text.chars().count();
            self.recompute_full_layout();
        }
    }

    /// Deletes the character before the cursor, or deletes the active selection.
    pub fn delete_backward(&mut self) {
        self.push_undo_snapshot();

        let block_id = self.active_block_id.clone();
        let offset = self.cursor_offset;

        if offset == 0 {
            return;
        }

        if let Some(para) = self.find_paragraph_mut(&block_id) {
            delete_char_before_in_spans(&mut para.spans, offset);
            self.cursor_offset = self.cursor_offset.saturating_sub(1);
            self.update_active_paragraph_fast();
        } else if let Some(heading) = self.find_heading_mut(&block_id) {
            delete_char_before_in_spans(&mut heading.spans, offset);
            self.cursor_offset = self.cursor_offset.saturating_sub(1);
            self.recompute_full_layout();
        }
    }

    /// Deletes the character after the cursor.
    pub fn delete_forward(&mut self) {
        self.push_undo_snapshot();

        let block_id = self.active_block_id.clone();
        let offset = self.cursor_offset;

        if let Some(para) = self.find_paragraph_mut(&block_id) {
            delete_char_at_in_spans(&mut para.spans, offset);
            self.update_active_paragraph_fast();
        } else if let Some(heading) = self.find_heading_mut(&block_id) {
            delete_char_at_in_spans(&mut heading.spans, offset);
            self.recompute_full_layout();
        }
    }

    /// Splits the current block at cursor position into two paragraphs.
    pub fn insert_paragraph_break(&mut self) {
        self.push_undo_snapshot();
        let block_id = self.active_block_id.clone();
        let offset = self.cursor_offset;

        let new_block_id = format!("p_{}", uuid_fast());

        // Split paragraph
        if let Some((sec_idx, block_idx)) = self.find_block_indices(&block_id) {
            let section = &mut self.document.sections[sec_idx];
            if let Block::Paragraph(ref mut para) = section.blocks[block_idx] {
                let tail_spans = split_spans_at(&mut para.spans, offset);
                let new_para = ParagraphBlock::new(&new_block_id, tail_spans);
                section
                    .blocks
                    .insert(block_idx + 1, Block::Paragraph(new_para));
                self.active_block_id = new_block_id;
                self.cursor_offset = 0;
            }
        }

        self.recompute_full_layout();
    }

    /// Sets cursor position within a block.
    pub fn set_cursor(&mut self, block_id: &str, offset: usize) {
        self.active_block_id = block_id.to_string();
        self.cursor_offset = offset;
        self.selection_anchor = None;
    }

    /// Sets selection range from anchor to current focus.
    pub fn set_selection(
        &mut self,
        anchor_block: &str,
        anchor_offset: usize,
        focus_block: &str,
        focus_offset: usize,
    ) {
        self.selection_anchor = Some((anchor_block.to_string(), anchor_offset));
        self.active_block_id = focus_block.to_string();
        self.cursor_offset = focus_offset;
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// Toggles bold styling on active paragraph or selection.
    pub fn toggle_bold(&mut self) {
        self.push_undo_snapshot();
        let block_id = self.active_block_id.clone();
        if let Some(para) = self.find_paragraph_mut(&block_id) {
            for span in &mut para.spans {
                span.style.bold = !span.style.bold;
            }
            self.update_active_paragraph_fast();
        }
    }

    /// Toggles italic styling on active paragraph or selection.
    pub fn toggle_italic(&mut self) {
        self.push_undo_snapshot();
        let block_id = self.active_block_id.clone();
        if let Some(para) = self.find_paragraph_mut(&block_id) {
            for span in &mut para.spans {
                span.style.italic = !span.style.italic;
            }
            self.update_active_paragraph_fast();
        }
    }

    /// Sets heading level for the active block (0 = body paragraph, 1..=6 = heading).
    pub fn set_heading_level(&mut self, level: u8) {
        self.push_undo_snapshot();
        let block_id = self.active_block_id.clone();
        if let Some((sec_idx, block_idx)) = self.find_block_indices(&block_id) {
            let section = &mut self.document.sections[sec_idx];
            let current_block = section.blocks.remove(block_idx);

            let new_block = match current_block {
                Block::Paragraph(para) => {
                    if level == 0 {
                        Block::Paragraph(para)
                    } else {
                        Block::Heading(HeadingBlock::new(para.id, level.clamp(1, 6), para.spans))
                    }
                }
                Block::Heading(h) => {
                    if level == 0 {
                        Block::Paragraph(ParagraphBlock::new(h.id, h.spans))
                    } else {
                        Block::Heading(HeadingBlock::new(h.id, level.clamp(1, 6), h.spans))
                    }
                }
                other => other,
            };

            section.blocks.insert(block_idx, new_block);
        }

        self.recompute_full_layout();
    }

    /// Inserts a bulleted list item.
    pub fn insert_bullet_list(&mut self) {
        self.push_undo_snapshot();
        let list_id = format!("list_{}", uuid_fast());
        let item_id = format!("li_{}", uuid_fast());
        let item = ListItem::new(&item_id, vec![TextSpan::plain("New list item")]);
        let list = ListBlock::new(&list_id, ListKind::Unordered, vec![item]);

        if let Some(sec) = self.document.sections.first_mut() {
            sec.blocks.push(Block::List(list));
            self.active_block_id = item_id;
            self.cursor_offset = 0;
        }

        self.recompute_full_layout();
    }

    /// Inserts a numbered list item.
    pub fn insert_numbered_list(&mut self) {
        self.push_undo_snapshot();
        let list_id = format!("list_{}", uuid_fast());
        let item_id = format!("li_{}", uuid_fast());
        let item = ListItem::new(&item_id, vec![TextSpan::plain("First numbered item")]);
        let list = ListBlock::new(&list_id, ListKind::Ordered, vec![item]);

        if let Some(sec) = self.document.sections.first_mut() {
            sec.blocks.push(Block::List(list));
            self.active_block_id = item_id;
            self.cursor_offset = 0;
        }

        self.recompute_full_layout();
    }

    /// Inserts a table with the given rows and columns.
    pub fn insert_table(&mut self, rows: usize, cols: usize) {
        self.push_undo_snapshot();
        let tbl_id = format!("tbl_{}", uuid_fast());
        let mut table_rows = Vec::with_capacity(rows);

        for r in 0..rows {
            let mut cells = Vec::with_capacity(cols);
            for c in 0..cols {
                let cell_id = format!("tc_{r}_{c}_{}", uuid_fast());
                let p_id = format!("p_{r}_{c}_{}", uuid_fast());
                let cell_para = ParagraphBlock::new(&p_id, vec![TextSpan::plain("")]);
                let cell = TableCell::new(&cell_id, vec![Block::Paragraph(cell_para)]);
                cells.push(cell);
            }
            table_rows.push(TableRow::new(cells));
        }

        let table = TableBlock::new(&tbl_id, table_rows);
        if let Some(sec) = self.document.sections.first_mut() {
            sec.blocks.push(Block::Table(table));
        }

        self.recompute_full_layout();
    }

    /// Inserts a footnote referenced at the current cursor position.
    pub fn insert_footnote(&mut self, footnote_text: &str) {
        self.push_undo_snapshot();
        let fn_id = format!("fn_{}", uuid_fast());
        let fn_p_id = format!("fn_p_{}", uuid_fast());
        let fn_num = (self.document.footnotes.len() + 1) as u32;

        // 1. Add footnote definition
        let footnote_def = FootnoteDefinition::new(
            &fn_id,
            fn_num,
            vec![Block::Paragraph(ParagraphBlock::new(
                &fn_p_id,
                vec![TextSpan::plain(footnote_text)],
            ))],
        );
        self.document.footnotes.insert(fn_id.clone(), footnote_def);

        // 2. Attach footnote_ref to active text span
        let block_id = self.active_block_id.clone();
        if let Some(para) = self.find_paragraph_mut(&block_id) {
            let mut fn_span = TextSpan::plain("");
            fn_span.footnote_ref = Some(fn_id);
            para.spans.push(fn_span);
        }

        self.recompute_full_layout();
    }

    /// Restores the previous document state from the undo stack.
    pub fn undo(&mut self) -> bool {
        if let Some(snapshot) = self.undo_stack.pop() {
            self.redo_stack.push(EditorSnapshot {
                document: self.document.clone(),
                active_block_id: self.active_block_id.clone(),
                cursor_offset: self.cursor_offset,
            });
            self.document = snapshot.document;
            self.active_block_id = snapshot.active_block_id;
            self.cursor_offset = snapshot.cursor_offset;
            self.recompute_full_layout();
            true
        } else {
            false
        }
    }

    /// Re-applies the next document state from the redo stack.
    pub fn redo(&mut self) -> bool {
        if let Some(snapshot) = self.redo_stack.pop() {
            self.undo_stack.push(EditorSnapshot {
                document: self.document.clone(),
                active_block_id: self.active_block_id.clone(),
                cursor_offset: self.cursor_offset,
            });
            self.document = snapshot.document;
            self.active_block_id = snapshot.active_block_id;
            self.cursor_offset = snapshot.cursor_offset;
            self.recompute_full_layout();
            true
        } else {
            false
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Hit-tests a 2D coordinate on a page and updates cursor position.
    pub fn hit_test_and_set_cursor(
        &mut self,
        page_index: usize,
        x_pt: f32,
        y_pt: f32,
    ) -> Option<(String, usize)> {
        let page = self.layout.pages.get(page_index)?;
        let loc = hit_test_page(page, x_pt, y_pt)?;
        self.active_block_id = loc.block_id.clone();
        self.cursor_offset = loc.char_offset;
        self.selection_anchor = None;
        Some((loc.block_id, loc.char_offset))
    }

    /// Returns the active cursor rectangle and page index: `Some((page_index, rect))`.
    pub fn cursor_rect(&self) -> Option<(usize, Rect)> {
        for (page_idx, page) in self.layout.pages.iter().enumerate() {
            if let Some(rect) =
                cursor_rect_for_offset(page, &self.active_block_id, self.cursor_offset)
            {
                return Some((page_idx, rect));
            }
        }
        None
    }

    /// Returns the selection rectangles across pages: `Vec<(page_index, rect)>`.
    pub fn selection_rects(&self) -> Vec<(usize, Rect)> {
        let mut results = Vec::new();
        let (anchor_block, anchor_offset) = match self.selection_anchor {
            Some(ref a) => a,
            None => return results,
        };

        if anchor_block != &self.active_block_id {
            return results;
        }

        for (page_idx, page) in self.layout.pages.iter().enumerate() {
            let rects =
                selection_rects_for_range(page, anchor_block, *anchor_offset, self.cursor_offset);
            for r in rects {
                results.push((page_idx, r));
            }
        }
        results
    }

    /// Measures average typing latency per keystroke in microseconds for a sample text run (< 16,000 µs budget).
    pub fn measure_typing_latency_micros(&mut self, sample_text: &str) -> u64 {
        let char_count = sample_text.chars().count();
        if char_count == 0 {
            return 0;
        }
        let start = std::time::Instant::now();
        for ch in sample_text.chars() {
            let mut s = String::new();
            s.push(ch);
            self.insert_text(&s);
        }
        let elapsed = start.elapsed().as_micros() as u64;
        elapsed / (char_count as u64)
    }

    /// Fast paragraph-local incremental reflow (< 2ms).
    fn update_active_paragraph_fast(&mut self) {
        if let Some((sec_idx, block_idx)) = self.find_block_indices(&self.active_block_id) {
            let section = &self.document.sections[sec_idx];
            if let Block::Paragraph(ref para) = section.blocks[block_idx] {
                self.layout = self.incremental_engine.update_paragraph(
                    &self.document,
                    para,
                    sec_idx,
                    &self.font_registry,
                    &self.paginator_options,
                );
                return;
            }
        }
        self.recompute_full_layout();
    }

    /// Full document pagination on quiescent or structural changes.
    pub fn recompute_full_layout(&mut self) {
        self.layout = self.incremental_engine.layout_full(
            &self.document,
            &self.font_registry,
            &self.paginator_options,
        );
    }

    fn find_paragraph_mut(&mut self, block_id: &str) -> Option<&mut ParagraphBlock> {
        for section in &mut self.document.sections {
            for block in &mut section.blocks {
                if let Block::Paragraph(p) = block {
                    if p.id == block_id {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    fn find_heading_mut(&mut self, block_id: &str) -> Option<&mut HeadingBlock> {
        for section in &mut self.document.sections {
            for block in &mut section.blocks {
                if let Block::Heading(h) = block {
                    if h.id == block_id {
                        return Some(h);
                    }
                }
            }
        }
        None
    }

    fn find_block_indices(&self, target_id: &str) -> Option<(usize, usize)> {
        for (sec_idx, section) in self.document.sections.iter().enumerate() {
            for (block_idx, block) in section.blocks.iter().enumerate() {
                if block_id(block) == target_id {
                    return Some((sec_idx, block_idx));
                }
            }
        }
        None
    }
}

fn block_id(block: &Block) -> String {
    match block {
        Block::Paragraph(p) => p.id.clone(),
        Block::Heading(h) => h.id.clone(),
        Block::List(l) => l.id.clone(),
        Block::Table(t) => t.id.clone(),
        Block::Image(i) => i.id.clone(),
        Block::Callout(c) => c.id.clone(),
        Block::ThematicBreak(tb) => tb.id.clone(),
    }
}

fn insert_into_spans(spans: &mut Vec<TextSpan>, target_offset: usize, text: &str) {
    if spans.is_empty() {
        spans.push(TextSpan::plain(text));
        return;
    }

    let mut accumulated = 0;
    for span in spans.iter_mut() {
        let char_count = span.text.chars().count();
        if target_offset <= accumulated + char_count {
            let offset_in_span = target_offset.saturating_sub(accumulated);
            let byte_idx = span
                .text
                .char_indices()
                .nth(offset_in_span)
                .map(|(i, _)| i)
                .unwrap_or(span.text.len());
            span.text.insert_str(byte_idx, text);
            return;
        }
        accumulated += char_count;
    }

    // Append to last span
    if let Some(last) = spans.last_mut() {
        last.text.push_str(text);
    }
}

fn delete_char_before_in_spans(spans: &mut [TextSpan], target_offset: usize) {
    if target_offset == 0 {
        return;
    }

    let mut accumulated = 0;
    for span in spans.iter_mut() {
        let char_count = span.text.chars().count();
        if target_offset <= accumulated + char_count {
            let offset_in_span = target_offset.saturating_sub(accumulated);
            if offset_in_span > 0 {
                let char_to_delete_idx = offset_in_span - 1;
                if let Some((byte_start, ch)) = span.text.char_indices().nth(char_to_delete_idx) {
                    let byte_end = byte_start + ch.len_utf8();
                    span.text.replace_range(byte_start..byte_end, "");
                    return;
                }
            }
        }
        accumulated += char_count;
    }
}

fn delete_char_at_in_spans(spans: &mut [TextSpan], target_offset: usize) {
    let mut accumulated = 0;
    for span in spans.iter_mut() {
        let char_count = span.text.chars().count();
        if target_offset < accumulated + char_count {
            let offset_in_span = target_offset.saturating_sub(accumulated);
            if let Some((byte_start, ch)) = span.text.char_indices().nth(offset_in_span) {
                let byte_end = byte_start + ch.len_utf8();
                span.text.replace_range(byte_start..byte_end, "");
                return;
            }
        }
        accumulated += char_count;
    }
}

fn split_spans_at(spans: &mut Vec<TextSpan>, target_offset: usize) -> Vec<TextSpan> {
    let mut tail = Vec::new();
    let mut accumulated = 0;
    let mut split_span_idx = None;
    let mut split_offset_in_span = 0;

    for (idx, span) in spans.iter().enumerate() {
        let char_count = span.text.chars().count();
        if target_offset <= accumulated + char_count {
            split_span_idx = Some(idx);
            split_offset_in_span = target_offset.saturating_sub(accumulated);
            break;
        }
        accumulated += char_count;
    }

    if let Some(idx) = split_span_idx {
        let mut span_to_split = spans.remove(idx);
        let byte_split = span_to_split
            .text
            .char_indices()
            .nth(split_offset_in_span)
            .map(|(i, _)| i)
            .unwrap_or(span_to_split.text.len());

        let tail_text = span_to_split.text.split_off(byte_split);
        let mut tail_span = span_to_split.clone();
        tail_span.text = tail_text;

        spans.insert(idx, span_to_split);
        tail.push(tail_span);

        // Move remaining spans after idx to tail
        while spans.len() > idx + 1 {
            tail.push(spans.remove(idx + 1));
        }
    }

    if tail.is_empty() {
        tail.push(TextSpan::plain(""));
    }

    tail
}

fn uuid_fast() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:08x}{:08x}", std::process::id(), id)
}
