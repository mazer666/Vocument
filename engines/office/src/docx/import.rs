//! DOCX import (`word/document.xml`, headers/footers, footnotes/endnotes, tables, lists,
//! comments, tracked changes).

use std::collections::{BTreeMap, HashMap};

use vocument_flow::model::{
    Block, FlowDocument, FootnoteDefinition, HeadingBlock, ImageBlock, ListBlock, ListItem,
    ListKind, PageOrientation, ParagraphBlock, Section, SectionGeometry, TableBlock, TableCell,
    TableRow,
};
use vocument_flow::review::{FlowComment, FlowCommentReply, TrackedChange};
use vocument_flow::styles::{ColorRgba, TextSpan};

use crate::convert::{AssetMap, ImportedDocument};
use crate::docx::parts::{
    NumberingMap, RawDocxComment, Relationship, ResolvedDocxStyles, parse_comments, parse_metadata,
    parse_numbering, parse_relationships, parse_styles,
};
use crate::docx::props::{ParagraphProps, attr, child_w, is_w, val};
use crate::error::OfficeError;
use crate::finalize::finalize_document;
use crate::limits::OfficeLimits;
use crate::package::OfficePackage;
use crate::report::{ConversionReport, Severity};
use crate::util::{IdGenerator, parse_hex_color, twips_to_pt};
use crate::xml::{Element, parse, serialize};

/// Imports a DOCX package into a validated [`ImportedDocument`].
pub fn import(bytes: &[u8], limits: &OfficeLimits) -> Result<ImportedDocument, OfficeError> {
    let package = OfficePackage::from_bytes(bytes, limits)?;
    let mut report = ConversionReport::new();

    if package.contains("word/vbaProject.bin") {
        report.note(
            Severity::Security,
            "docx.macro.vba",
            "VBA macro project ('word/vbaProject.bin') stripped for security",
            Some("word/vbaProject.bin"),
        );
    }

    let root_rels = parse_relationships(&package, "_rels/.rels", limits, &mut report)?;
    let main_part = root_rels
        .values()
        .find(|rel| !rel.is_external && rel.rel_type.ends_with("/officeDocument"))
        .map(|rel| rel.resolved_part_path(""))
        .unwrap_or_else(|| "word/document.xml".to_string());

    if !package.contains(&main_part) {
        return Err(OfficeError::MissingPart { name: main_part });
    }
    if main_part.starts_with("xl/") || main_part.starts_with("ppt/") {
        return Err(OfficeError::UnsupportedDocument {
            details: format!("package main part '{main_part}' is not a WordprocessingML document"),
        });
    }

    let base_dir = main_part
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("word");
    let doc_rels_part = {
        let file_name = main_part
            .rsplit_once('/')
            .map_or(main_part.as_str(), |(_, f)| f);
        format!("{base_dir}/_rels/{file_name}.rels")
    };
    let doc_rels = parse_relationships(&package, &doc_rels_part, limits, &mut report)?;

    let styles = parse_styles(&package, &format!("{base_dir}/styles.xml"), limits)?;
    let numbering = parse_numbering(&package, &format!("{base_dir}/numbering.xml"), limits)?;
    let raw_comments = parse_comments(
        &package,
        &format!("{base_dir}/comments.xml"),
        &format!("{base_dir}/commentsExtended.xml"),
        limits,
    )?;
    let metadata = parse_metadata(&package, limits)?;

    let mut ctx = ImportContext {
        package: &package,
        limits,
        base_dir,
        rels: &doc_rels,
        styles: &styles,
        numbering: &numbering,
        raw_comments,
        comment_anchors: HashMap::new(),
        assets: AssetMap::new(),
        footnotes: BTreeMap::new(),
        footnote_xml_map: HashMap::new(),
        endnote_xml_map: HashMap::new(),
        tracked_changes: Vec::new(),
        ids: IdGenerator::default(),
        report: &mut report,
    };
    ctx.load_notes_parts()?;

    let doc_bytes = package.required_part(&main_part)?;
    let doc_root = parse(doc_bytes, &main_part, limits)?;
    let body = child_w(&doc_root, "body").ok_or_else(|| OfficeError::UnsupportedDocument {
        details: "word/document.xml has no w:body element".to_string(),
    })?;

    let sections = ctx.convert_body(body)?;
    let comments = ctx.assemble_comments();
    let footnotes = ctx.footnotes;
    let tracked_changes = ctx.tracked_changes;
    let assets = ctx.assets;
    let mut ids = ctx.ids;

    let mut document = FlowDocument::new(metadata);
    document.stylesheet = styles.stylesheet;
    document.sections = sections;
    document.footnotes = footnotes;
    document.review.comments = comments;
    document.review.tracked_changes = tracked_changes;

    finalize_document(&mut document, &mut ids)?;

    Ok(ImportedDocument {
        document,
        assets,
        report,
    })
}

#[derive(Clone, Default)]
pub(super) struct CommentAnchorInfo {
    pub(super) block_id: String,
    pub(super) char_start: usize,
    pub(super) char_end: Option<usize>,
    pub(super) text: String,
}

pub(super) struct ImportContext<'a> {
    pub(super) package: &'a OfficePackage,
    pub(super) limits: &'a OfficeLimits,
    pub(super) base_dir: &'a str,
    pub(super) rels: &'a HashMap<String, Relationship>,
    pub(super) styles: &'a ResolvedDocxStyles,
    pub(super) numbering: &'a NumberingMap,
    pub(super) raw_comments: BTreeMap<String, RawDocxComment>,
    pub(super) comment_anchors: HashMap<String, CommentAnchorInfo>,
    pub(super) assets: AssetMap,
    pub(super) footnotes: BTreeMap<String, FootnoteDefinition>,
    pub(super) footnote_xml_map: HashMap<String, Element>,
    pub(super) endnote_xml_map: HashMap<String, Element>,
    pub(super) tracked_changes: Vec<TrackedChange>,
    pub(super) ids: IdGenerator,
    pub(super) report: &'a mut ConversionReport,
}

#[derive(Clone)]
pub(super) struct PendingListItem {
    num_id: String,
    ilvl: u8,
    kind: ListKind,
    num_fmt: String,
    spans: Vec<TextSpan>,
}

#[derive(Clone)]
pub(super) enum ConvertedChild {
    Block(Block),
    ListItem(PendingListItem),
    SectionBreak(SectionGeometry, Vec<Block>, Vec<Block>),
}

impl ImportContext<'_> {
    fn load_notes_parts(&mut self) -> Result<(), OfficeError> {
        let fn_part = format!("{}/footnotes.xml", self.base_dir);
        if let Some(bytes) = self.package.part(&fn_part) {
            let root = parse(bytes, &fn_part, self.limits)?;
            for note in root.child_elements().filter(|el| is_w(el, "footnote")) {
                if matches!(
                    attr(note, "type"),
                    Some("separator" | "continuationSeparator")
                ) {
                    continue;
                }
                if let Some(id) = attr(note, "id") {
                    self.footnote_xml_map.insert(id.to_string(), note.clone());
                }
            }
        }
        let en_part = format!("{}/endnotes.xml", self.base_dir);
        if let Some(bytes) = self.package.part(&en_part) {
            let root = parse(bytes, &en_part, self.limits)?;
            for note in root.child_elements().filter(|el| is_w(el, "endnote")) {
                if matches!(
                    attr(note, "type"),
                    Some("separator" | "continuationSeparator")
                ) {
                    continue;
                }
                if let Some(id) = attr(note, "id") {
                    self.endnote_xml_map.insert(id.to_string(), note.clone());
                }
            }
        }
        Ok(())
    }

    fn convert_body(&mut self, body: &Element) -> Result<Vec<Section>, OfficeError> {
        let mut sections = Vec::new();
        let mut current_items = Vec::new();
        let mut final_sect_pr: Option<&Element> = None;

        for child in body.child_elements() {
            if is_w(child, "sectPr") {
                final_sect_pr = Some(child);
                continue;
            }
            self.convert_block_element(child, &mut current_items)?;
            if let Some(ConvertedChild::SectionBreak(geom, hdr, ftr)) =
                current_items.last().cloned()
            {
                current_items.pop();
                let blocks = self.flush_block_items(std::mem::take(&mut current_items));
                let mut sec = Section::new(self.ids.next("sec"), geom, blocks);
                sec.header_blocks = hdr;
                sec.footer_blocks = ftr;
                sections.push(sec);
            }
        }

        if !current_items.is_empty() || sections.is_empty() {
            let (geom, hdr, ftr) = match final_sect_pr {
                Some(sect_pr) => self.parse_sect_pr(sect_pr)?,
                None => (SectionGeometry::a4_portrait(), Vec::new(), Vec::new()),
            };
            let blocks = self.flush_block_items(current_items);
            let mut sec = Section::new(self.ids.next("sec"), geom, blocks);
            sec.header_blocks = hdr;
            sec.footer_blocks = ftr;
            sections.push(sec);
        }
        Ok(sections)
    }

    pub(super) fn flush_block_items(&mut self, items: Vec<ConvertedChild>) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut pending_list: Vec<PendingListItem> = Vec::new();
        for item in items {
            match item {
                ConvertedChild::ListItem(li) => {
                    if let Some(prev) = pending_list.last()
                        && prev.num_id != li.num_id
                        && li.ilvl == 0
                    {
                        blocks.push(Block::List(self.build_list_tree(&pending_list)));
                        pending_list.clear();
                    }
                    pending_list.push(li);
                }
                ConvertedChild::Block(block) => {
                    if !pending_list.is_empty() {
                        blocks.push(Block::List(self.build_list_tree(&pending_list)));
                        pending_list.clear();
                    }
                    blocks.push(block);
                }
                ConvertedChild::SectionBreak(..) => {}
            }
        }
        if !pending_list.is_empty() {
            blocks.push(Block::List(self.build_list_tree(&pending_list)));
        }
        blocks
    }

    fn build_list_tree(&mut self, items: &[PendingListItem]) -> ListBlock {
        self.build_list_level(items, 0).0
    }

    fn build_list_level(&mut self, items: &[PendingListItem], start: usize) -> (ListBlock, usize) {
        let base_level = items[start].ilvl;
        let kind = items[start].kind;
        let num_fmt = items[start].num_fmt.clone();
        let mut list_items: Vec<ListItem> = Vec::new();
        let mut idx = start;

        while idx < items.len() {
            let current = &items[idx];
            if current.ilvl < base_level {
                break;
            }
            if current.ilvl > base_level {
                let (nested, next_idx) = self.build_list_level(items, idx);
                if let Some(last) = list_items.last_mut() {
                    last.nested_list = Some(Box::new(nested));
                } else {
                    let mut wrapper = ListItem::new(self.ids.next("li"), vec![TextSpan::plain("")]);
                    wrapper.nested_list = Some(Box::new(nested));
                    list_items.push(wrapper);
                }
                idx = next_idx;
            } else {
                list_items.push(ListItem::new(self.ids.next("li"), current.spans.clone()));
                idx += 1;
            }
        }

        let mut list = ListBlock::new(self.ids.next("list"), kind, list_items);
        list.extra.insert(
            "office.number_format".to_string(),
            serde_json::Value::String(num_fmt),
        );
        (list, idx)
    }

    pub(super) fn convert_block_element(
        &mut self,
        element: &Element,
        out: &mut Vec<ConvertedChild>,
    ) -> Result<(), OfficeError> {
        if is_w(element, "p") {
            self.convert_paragraph(element, out)?;
        } else if is_w(element, "tbl") {
            if let Some(tbl) = self.convert_table(element)? {
                out.push(ConvertedChild::Block(Block::Table(tbl)));
            }
        } else if is_w(element, "sdt") {
            if let Some(content) = child_w(element, "sdtContent") {
                for child in content.child_elements() {
                    self.convert_block_element(child, out)?;
                }
            }
        } else if is_w(element, "ins") || is_w(element, "customXml") {
            for child in element.child_elements() {
                self.convert_block_element(child, out)?;
            }
        } else if !is_w(element, "bookmarkStart")
            && !is_w(element, "bookmarkEnd")
            && !is_w(element, "proofErr")
            && !is_w(element, "permStart")
            && !is_w(element, "permEnd")
        {
            self.report.note(
                Severity::Loss,
                &format!("docx.body.{}", element.local),
                &format!("Unsupported body element <w:{}> skipped", element.local),
                Some("word/document.xml"),
            );
        }
        Ok(())
    }

    fn convert_paragraph(
        &mut self,
        p_el: &Element,
        out: &mut Vec<ConvertedChild>,
    ) -> Result<(), OfficeError> {
        let mut p_props = ParagraphProps::default();
        let mut unhandled_ppr = Vec::new();
        let mut inline_sect_pr: Option<&Element> = None;

        if let Some(ppr) = child_w(p_el, "pPr") {
            let (parsed, unhandled) = ParagraphProps::from_element(ppr);
            p_props = parsed;
            unhandled_ppr = unhandled;
            inline_sect_pr = child_w(ppr, "sectPr");
        }

        let style_id = p_props
            .style_id
            .clone()
            .filter(|id| self.styles.stylesheet.paragraph_styles.contains_key(id));
        let effective_p = self.styles.effective_paragraph(style_id.as_deref());
        let heading_level = p_props
            .outline_level
            .filter(|l| *l < 9)
            .map(|l| l + 1)
            .or_else(|| {
                style_id
                    .as_ref()
                    .and_then(|id| self.styles.heading_levels.get(id).copied())
            });

        let block_id_hint = self.ids.next("p");
        let mut spans: Vec<TextSpan> = Vec::new();
        let mut inline_images: Vec<ImageBlock> = Vec::new();
        let mut active_comments: Vec<String> = Vec::new();

        let run_para_style = if heading_level.is_some() {
            None
        } else {
            style_id.as_deref()
        };
        self.collect_paragraph_runs(
            p_el,
            run_para_style,
            None,
            None,
            &block_id_hint,
            &mut spans,
            &mut inline_images,
            &mut active_comments,
        )?;

        let has_text = spans
            .iter()
            .any(|s| !s.text.is_empty() || s.footnote_ref.is_some());
        if !has_text && !inline_images.is_empty() {
            for img in inline_images {
                out.push(ConvertedChild::Block(Block::Image(img)));
            }
            if let Some(sect_pr) = inline_sect_pr {
                let (geom, hdr, ftr) = self.parse_sect_pr(sect_pr)?;
                out.push(ConvertedChild::SectionBreak(geom, hdr, ftr));
            }
            return Ok(());
        }

        let mut extra = BTreeMap::new();
        if p_props.page_break_before == Some(true) {
            extra.insert(
                "office.page_break_before".to_string(),
                serde_json::Value::Bool(true),
            );
            self.report.note(
                Severity::PreservedUnrendered,
                "docx.paragraph.page-break-before",
                "Page break before paragraph preserved in metadata",
                Some(&block_id_hint),
            );
        }
        if !unhandled_ppr.is_empty() {
            let raw_xml: Vec<String> = unhandled_ppr.into_iter().map(serialize).collect();
            extra.insert(
                "office.docx.pPr_unmapped".to_string(),
                serde_json::json!(raw_xml),
            );
            self.report.note(
                Severity::PreservedUnrendered,
                "docx.paragraph.unmapped-props",
                "Unmapped paragraph properties preserved as raw XML",
                Some(&block_id_hint),
            );
        }

        if let Some(level) = heading_level {
            if level <= 6 {
                let h_id = self.ids.next("h");
                self.remap_block_id(&block_id_hint, &h_id);
                let mut heading = HeadingBlock::new(h_id, level, spans);
                heading.style_id = style_id;
                heading.extra = extra;
                out.push(ConvertedChild::Block(Block::Heading(heading)));
            } else {
                self.report.note(
                    Severity::Approximated,
                    "docx.heading.level-clamped",
                    "Heading level > 6 imported as styled paragraph",
                    Some(&block_id_hint),
                );
                let mut para = ParagraphBlock::new(block_id_hint, spans);
                para.style_id = style_id;
                para.alignment = p_props.alignment;
                para.space_before_pt = p_props.space_before_pt;
                para.space_after_pt = p_props.space_after_pt;
                para.extra = extra;
                out.push(ConvertedChild::Block(Block::Paragraph(para)));
            }
        } else if let Some((num_id, ilvl)) = p_props.numbering.clone().or(effective_p.numbering)
            && num_id != "0"
        {
            let level_info = self.numbering.get(&num_id, ilvl);
            out.push(ConvertedChild::ListItem(PendingListItem {
                num_id,
                ilvl,
                kind: level_info.kind,
                num_fmt: level_info.num_fmt,
                spans,
            }));
        } else {
            let mut para = ParagraphBlock::new(block_id_hint, spans);
            para.style_id = style_id;
            para.alignment = p_props.alignment;
            para.space_before_pt = p_props.space_before_pt;
            para.space_after_pt = p_props.space_after_pt;
            para.extra = extra;
            out.push(ConvertedChild::Block(Block::Paragraph(para)));
        }

        if !inline_images.is_empty() {
            self.report.note(
                Severity::Approximated,
                "docx.image.inline-moved",
                "Inline image inside text paragraph placed as following image block",
                None,
            );
            for img in inline_images {
                out.push(ConvertedChild::Block(Block::Image(img)));
            }
        }

        if let Some(sect_pr) = inline_sect_pr {
            let (geom, hdr, ftr) = self.parse_sect_pr(sect_pr)?;
            out.push(ConvertedChild::SectionBreak(geom, hdr, ftr));
        }
        Ok(())
    }

    fn remap_block_id(&mut self, old_id: &str, new_id: &str) {
        for anchor in self.comment_anchors.values_mut() {
            if anchor.block_id == old_id {
                anchor.block_id = new_id.to_string();
            }
        }
        for tc in &mut self.tracked_changes {
            if tc.target_block_id.as_deref() == Some(old_id) {
                tc.target_block_id = Some(new_id.to_string());
            }
        }
    }

    fn convert_table(&mut self, tbl_el: &Element) -> Result<Option<TableBlock>, OfficeError> {
        let mut col_widths = Vec::new();
        if let Some(grid) = child_w(tbl_el, "tblGrid") {
            for col in grid.child_elements().filter(|el| is_w(el, "gridCol")) {
                if let Some(w) = attr(col, "w").and_then(|v| v.parse::<i64>().ok()) {
                    col_widths.push(twips_to_pt(w.max(20)));
                }
            }
        }

        let mut caption = None;
        if let Some(tbl_pr) = child_w(tbl_el, "tblPr")
            && let Some(cap) = child_w(tbl_pr, "tblCaption").and_then(val)
            && !cap.is_empty()
        {
            caption = Some(cap.to_string());
        }

        struct RawCell {
            blocks: Vec<Block>,
            col_span: u32,
            vmerge: Option<bool>,
            bg: Option<ColorRgba>,
        }
        let mut raw_rows: Vec<(bool, Vec<RawCell>)> = Vec::new();

        for tr in tbl_el.child_elements().filter(|el| is_w(el, "tr")) {
            let is_header = child_w(tr, "trPr")
                .and_then(|pr| child_w(pr, "tblHeader"))
                .is_some();
            let mut cells = Vec::new();
            for tc in tr.child_elements().filter(|el| is_w(el, "tc")) {
                let mut col_span = 1u32;
                let mut vmerge = None;
                let mut bg = None;
                if let Some(tc_pr) = child_w(tc, "tcPr") {
                    if let Some(gs) = child_w(tc_pr, "gridSpan")
                        .and_then(val)
                        .and_then(|v| v.parse::<u32>().ok())
                    {
                        col_span = gs.clamp(1, 64);
                    }
                    if let Some(vm) = child_w(tc_pr, "vMerge") {
                        let is_restart = val(vm) == Some("restart");
                        vmerge = Some(is_restart);
                    }
                    if let Some(shd) = child_w(tc_pr, "shd") {
                        bg = attr(shd, "fill").and_then(parse_hex_color);
                    }
                }
                let mut cell_items = Vec::new();
                for child in tc.child_elements() {
                    self.convert_block_element(child, &mut cell_items)?;
                }
                let blocks = self.flush_block_items(cell_items);
                cells.push(RawCell {
                    blocks,
                    col_span,
                    vmerge,
                    bg,
                });
            }
            if !cells.is_empty() {
                raw_rows.push((is_header, cells));
            }
        }

        if raw_rows.is_empty() {
            return Ok(None);
        }

        let mut rows: Vec<TableRow> = Vec::with_capacity(raw_rows.len());
        let mut repeat_header_rows = 0u32;
        for (r_idx, (is_header, raw_cells)) in raw_rows.iter().enumerate() {
            if *is_header && r_idx as u32 == repeat_header_rows {
                repeat_header_rows += 1;
            }
            let mut out_cells = Vec::new();
            let mut col_cursor = 0usize;
            for cell in raw_cells {
                let span_cols = cell.col_span as usize;
                if cell.vmerge == Some(false) {
                    col_cursor += span_cols;
                    continue;
                }
                let mut row_span = 1u32;
                if cell.vmerge == Some(true) {
                    for (_, next_cells) in &raw_rows[(r_idx + 1)..] {
                        let mut c_pos = 0usize;
                        let mut continues = false;
                        for nc in next_cells {
                            if c_pos == col_cursor && nc.vmerge == Some(false) {
                                continues = true;
                                break;
                            }
                            c_pos += nc.col_span as usize;
                        }
                        if continues {
                            row_span += 1;
                        } else {
                            break;
                        }
                    }
                }
                let mut tc = TableCell::new(self.ids.next("cell"), cell.blocks.clone());
                tc.col_span = cell.col_span;
                tc.row_span = row_span;
                tc.background_color = cell.bg;
                out_cells.push(tc);
                col_cursor += span_cols;
            }
            if !out_cells.is_empty() {
                let mut row = TableRow::new(out_cells);
                row.is_header = *is_header;
                rows.push(row);
            }
        }

        if rows.is_empty() {
            return Ok(None);
        }
        let mut table = TableBlock::new(self.ids.next("tbl"), rows);
        table.caption = caption;
        table.column_widths_pt = col_widths;
        table.repeat_header_rows = repeat_header_rows;
        Ok(Some(table))
    }

    fn parse_sect_pr(
        &mut self,
        sect_pr: &Element,
    ) -> Result<(SectionGeometry, Vec<Block>, Vec<Block>), OfficeError> {
        let mut geom = SectionGeometry::a4_portrait();
        if let Some(pg_sz) = child_w(sect_pr, "pgSz") {
            if let Some(w) = attr(pg_sz, "w")
                .and_then(|v| v.parse::<i64>().ok())
                .filter(|v| *v > 0)
            {
                geom.width_pt = twips_to_pt(w).clamp(72.0, 5000.0);
            }
            if let Some(h) = attr(pg_sz, "h")
                .and_then(|v| v.parse::<i64>().ok())
                .filter(|v| *v > 0)
            {
                geom.height_pt = twips_to_pt(h).clamp(72.0, 5000.0);
            }
            if attr(pg_sz, "orient") == Some("landscape") || geom.width_pt > geom.height_pt {
                geom.orientation = PageOrientation::Landscape;
            }
        }
        if let Some(pg_mar) = child_w(sect_pr, "pgMar") {
            let max_h_margin = (geom.width_pt * 0.4).max(0.0);
            let max_v_margin = (geom.height_pt * 0.4).max(0.0);
            if let Some(t) = attr(pg_mar, "top").and_then(|v| v.parse::<i64>().ok()) {
                geom.margin_top_pt = twips_to_pt(t.max(0)).min(max_v_margin);
            }
            if let Some(b) = attr(pg_mar, "bottom").and_then(|v| v.parse::<i64>().ok()) {
                geom.margin_bottom_pt = twips_to_pt(b.max(0)).min(max_v_margin);
            }
            if let Some(l) = attr(pg_mar, "left").and_then(|v| v.parse::<i64>().ok()) {
                geom.margin_left_pt = twips_to_pt(l.max(0)).min(max_h_margin);
            }
            if let Some(r) = attr(pg_mar, "right").and_then(|v| v.parse::<i64>().ok()) {
                geom.margin_right_pt = twips_to_pt(r.max(0)).min(max_h_margin);
            }
        }
        if let Some(cols) = child_w(sect_pr, "cols") {
            if let Some(num) = attr(cols, "num")
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|n| *n >= 1)
            {
                geom.column_count = num.min(6);
            }
            if let Some(space) = attr(cols, "space").and_then(|v| v.parse::<i64>().ok()) {
                geom.column_gap_pt = twips_to_pt(space.max(0)).min(72.0);
            }
        }

        let mut header_blocks = Vec::new();
        let mut footer_blocks = Vec::new();
        for child in sect_pr.child_elements() {
            if is_w(child, "headerReference") || is_w(child, "footerReference") {
                let is_header = is_w(child, "headerReference");
                let kind = attr(child, "type").unwrap_or("default");
                if kind != "default" {
                    self.report.note(
                        Severity::Loss,
                        if is_header {
                            "docx.header.non-default"
                        } else {
                            "docx.footer.non-default"
                        },
                        &format!(
                            "Non-default {} ('{kind}') skipped",
                            if is_header { "header" } else { "footer" }
                        ),
                        None,
                    );
                    continue;
                }
                if let Some(rel_id) = child.attribute_any_namespace("id")
                    && let Some(rel) = self.rels.get(rel_id)
                    && !rel.is_external
                {
                    let part_path = rel.resolved_part_path(self.base_dir);
                    if let Some(bytes) = self.package.part(&part_path)
                        && let Ok(root) = parse(bytes, &part_path, self.limits)
                    {
                        let mut items = Vec::new();
                        for el in root.child_elements() {
                            self.convert_block_element(el, &mut items)?;
                        }
                        let blocks = self.flush_block_items(items);
                        if is_header {
                            header_blocks = blocks;
                        } else {
                            footer_blocks = blocks;
                        }
                    }
                }
            }
        }
        Ok((geom, header_blocks, footer_blocks))
    }

    fn assemble_comments(&mut self) -> Vec<FlowComment> {
        let para_to_comment: HashMap<String, String> = self
            .raw_comments
            .iter()
            .filter_map(|(cid, c)| c.para_id.as_ref().map(|pid| (pid.clone(), cid.clone())))
            .collect();

        let mut replies_by_parent: HashMap<String, Vec<FlowCommentReply>> = HashMap::new();
        let mut root_comments = Vec::new();

        for (cid, raw) in &self.raw_comments {
            if let Some(parent_para) = &raw.parent_para_id
                && let Some(parent_cid) = para_to_comment.get(parent_para)
            {
                replies_by_parent
                    .entry(parent_cid.clone())
                    .or_default()
                    .push(FlowCommentReply::new(
                        format!("reply_{cid}"),
                        &raw.author,
                        &raw.date,
                        &raw.text,
                    ));
            } else {
                root_comments.push(raw.clone());
            }
        }

        root_comments
            .into_iter()
            .map(|raw| {
                let anchor = self.comment_anchors.get(&raw.id);
                let range = anchor.map(|a| {
                    let end = a.char_end.unwrap_or(a.char_start + a.text.chars().count());
                    (a.char_start, end.max(a.char_start))
                });
                let mut comment = FlowComment::new(
                    format!("comment_{}", raw.id),
                    raw.author,
                    raw.date,
                    raw.text,
                    anchor.map(|a| a.block_id.clone()),
                    range,
                    anchor.and_then(|a| (!a.text.is_empty()).then(|| a.text.clone())),
                );
                comment.resolved = raw.done;
                if let Some(replies) = replies_by_parent.remove(&raw.id) {
                    comment.replies = replies;
                }
                comment
            })
            .collect()
    }
}
