//! ODT (`content.xml`, `styles.xml`, `meta.xml`, `META-INF/manifest.xml`) import into `FlowDocument`.

use std::collections::{BTreeMap, HashMap};

use vocument_flow::model::{
    Block, FlowDocument, FootnoteDefinition, HeadingBlock, ImageBlock, ListBlock, ListItem,
    ListKind, ParagraphBlock, Section, TableBlock, TableCell, TableRow,
};
use vocument_flow::review::{ChangeType, FlowComment, FlowCommentReply, TrackedChange};
use vocument_flow::styles::TextSpan;

use crate::convert::{AssetMap, ImportedDocument};
use crate::error::OfficeError;
use crate::finalize::finalize_document;
use crate::limits::OfficeLimits;
use crate::odt::styles::{
    NS_DC, NS_DRAW, NS_MANIFEST, NS_OFFICE, NS_TABLE, NS_TEXT, OdtParaStyle, OdtStyles,
    OdtTextProps, parse_meta,
};
use crate::package::OfficePackage;
use crate::report::{ConversionReport, Severity};
use crate::util::IdGenerator;
use crate::xml::{Element, parse};

pub(super) struct OdtAnnotation {
    pub(super) name: String,
    pub(super) author: String,
    pub(super) date: String,
    pub(super) content: String,
    pub(super) block_id: String,
    pub(super) char_start: usize,
    pub(super) char_end: Option<usize>,
    pub(super) anchor_text: String,
    pub(super) resolved: bool,
    pub(super) replies: Vec<FlowCommentReply>,
}

pub(super) struct OdtImportContext<'a> {
    pub(super) package: &'a OfficePackage,
    pub(super) styles: &'a OdtStyles,
    pub(super) assets: AssetMap,
    pub(super) footnotes: BTreeMap<String, FootnoteDefinition>,
    pub(super) tracked_changes: Vec<TrackedChange>,
    pub(super) change_id_map: HashMap<String, String>,
    pub(super) deleted_texts: HashMap<String, String>,
    pub(super) annotations: Vec<OdtAnnotation>,
    pub(super) ids: IdGenerator,
    pub(super) report: &'a mut ConversionReport,
}

/// Imports an ODF Text (`.odt`) package into a validated [`ImportedDocument`].
pub fn import(bytes: &[u8], limits: &OfficeLimits) -> Result<ImportedDocument, OfficeError> {
    let package = OfficePackage::from_bytes(bytes, limits)?;
    let mut report = ConversionReport::new();

    if let Some(mime_bytes) = package.part("mimetype") {
        let mime = String::from_utf8_lossy(mime_bytes);
        let mime = mime.trim();
        if mime.contains("spreadsheet") || mime.contains("presentation") {
            return Err(OfficeError::UnsupportedDocument {
                details: format!("ODF mimetype '{mime}' is not an OpenDocument Text document"),
            });
        }
    }

    if let Some(manifest_bytes) = package.part("META-INF/manifest.xml") {
        let manifest_root = parse(manifest_bytes, "META-INF/manifest.xml", limits)?;
        let mut stack = vec![&manifest_root];
        while let Some(el) = stack.pop() {
            if el.is(NS_MANIFEST, "encryption-data") {
                return Err(OfficeError::EncryptedEntry {
                    name: "content.xml".to_string(),
                });
            }
            stack.extend(el.child_elements());
        }
    }

    if package
        .part_names()
        .any(|name| name.starts_with("Basic/") || name.starts_with("Scripts/"))
    {
        report.note(
            Severity::Security,
            "odt.macro.basic",
            "LibreOffice Basic/Scripts macro directory stripped for security",
            Some("Basic/"),
        );
    }

    const EMPTY_CONTENT_XML: &[u8] = br#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"><office:body><office:text/></office:body></office:document-content>"#;
    let content_bytes = match package.part("content.xml") {
        Some(bytes) => bytes,
        None if package.part("styles.xml").is_some() => {
            report.note(
                Severity::Info,
                "odt.template.styles_only",
                "ODF style-only template without content.xml imported with empty body",
                Some("styles.xml"),
            );
            EMPTY_CONTENT_XML
        }
        None => package.required_part("content.xml")?,
    };
    let content_root = parse(content_bytes, "content.xml", limits)?;
    let metadata = parse_meta(&package, limits)?;
    let styles = OdtStyles::parse_all(&package, &content_root, limits)?;

    let office_text = content_root
        .child(NS_OFFICE, "body")
        .and_then(|b| b.child(NS_OFFICE, "text"))
        .ok_or_else(|| OfficeError::UnsupportedDocument {
            details: "content.xml has no office:body/office:text element".to_string(),
        })?;

    let mut ctx = OdtImportContext {
        package: &package,
        styles: &styles,
        assets: AssetMap::new(),
        footnotes: BTreeMap::new(),
        tracked_changes: Vec::new(),
        change_id_map: HashMap::new(),
        deleted_texts: HashMap::new(),
        annotations: Vec::new(),
        ids: IdGenerator::default(),
        report: &mut report,
    };

    if let Some(tc_el) = office_text.child(NS_TEXT, "tracked-changes") {
        ctx.parse_tracked_changes(tc_el);
    }

    let mut blocks = Vec::new();
    for child in office_text.child_elements() {
        ctx.convert_block(child, &mut blocks)?;
    }

    let mut header_blocks = Vec::new();
    for hdr_el in &styles.master_headers {
        ctx.convert_block(hdr_el, &mut header_blocks)?;
    }
    let mut footer_blocks = Vec::new();
    for ftr_el in &styles.master_footers {
        ctx.convert_block(ftr_el, &mut footer_blocks)?;
    }

    let mut section = Section::new(ctx.ids.next("sec"), styles.default_geometry.clone(), blocks);
    section.header_blocks = header_blocks;
    section.footer_blocks = footer_blocks;

    let comments = ctx.assemble_comments();
    let footnotes = ctx.footnotes;
    let tracked_changes = ctx.tracked_changes;
    let assets = ctx.assets;
    let mut ids = ctx.ids;

    let mut document = FlowDocument::new(metadata);
    document.stylesheet = styles.stylesheet;
    document.sections = vec![section];
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

impl OdtImportContext<'_> {
    fn parse_tracked_changes(&mut self, tc_root: &Element) {
        for region in tc_root
            .child_elements()
            .filter(|e| e.is(NS_TEXT, "changed-region"))
        {
            let Some(xml_id) = region
                .attribute(NS_TEXT, "id")
                .or_else(|| region.attribute_any_namespace("id"))
            else {
                continue;
            };
            let kind_el = region.child_elements().next();
            let Some(kind_el) = kind_el else {
                continue;
            };
            let change_type = match kind_el.local.as_str() {
                "insertion" => ChangeType::Insert,
                "deletion" => ChangeType::Delete,
                "format-change" => ChangeType::FormatChange,
                _ => continue,
            };
            let mut author = "Author".to_string();
            let mut date = "1970-01-01T00:00:00Z".to_string();
            if let Some(info) = kind_el.child(NS_OFFICE, "change-info") {
                if let Some(c) = info.child(NS_DC, "creator") {
                    let t = c.text_content().trim().to_string();
                    if !t.is_empty() {
                        author = t;
                    }
                }
                if let Some(d) = info.child(NS_DC, "date") {
                    let t = d.text_content().trim().to_string();
                    if !t.is_empty() {
                        date = t;
                    }
                }
            }
            let mut del_paragraphs = Vec::new();
            for p in kind_el.child_elements().filter(|e| e.is(NS_TEXT, "p")) {
                let t = p.text_content();
                if !t.is_empty() {
                    del_paragraphs.push(t);
                }
            }
            let del_text = del_paragraphs.join(" ");
            if !del_text.is_empty() {
                self.deleted_texts
                    .insert(xml_id.to_string(), del_text.clone());
            }

            let chg_id = self.ids.next("chg");
            let mut tc = TrackedChange::new(&chg_id, change_type, author, date);
            if !del_text.is_empty() {
                tc.text_snippet = Some(del_text);
            }
            self.change_id_map.insert(xml_id.to_string(), chg_id);
            self.tracked_changes.push(tc);
        }
    }

    pub(super) fn convert_block(
        &mut self,
        el: &Element,
        out: &mut Vec<Block>,
    ) -> Result<(), OfficeError> {
        match (el.namespace.as_str(), el.local.as_str()) {
            (NS_TEXT, "h") => self.convert_heading(el, out)?,
            (NS_TEXT, "p") => self.convert_paragraph(el, out)?,
            (NS_TEXT, "list") => {
                if let Some(list) = self.convert_list(el, None, 0)? {
                    out.push(Block::List(list));
                }
            }
            (NS_TABLE, "table") => {
                if let Some(tbl) = self.convert_table(el)? {
                    out.push(Block::Table(tbl));
                }
            }
            (NS_DRAW, "frame") => {
                if let Some(img) = self.extract_draw_frame(el, "frame") {
                    out.push(Block::Image(img));
                }
            }
            (NS_TEXT, "section") | (NS_TEXT, "table-of-content") | (NS_TEXT, "index-body") => {
                for child in el.child_elements() {
                    self.convert_block(child, out)?;
                }
            }
            (NS_TEXT, "tracked-changes") | (NS_TEXT, "sequence-decls") | (NS_OFFICE, "forms") => {}
            _ => {
                self.report.note(
                    Severity::Loss,
                    &format!("odt.body.{}", el.local),
                    &format!("Unsupported ODF body element <{}> skipped", el.local),
                    Some("content.xml"),
                );
            }
        }
        Ok(())
    }

    fn resolve_paragraph_style_info(
        &self,
        style_name: Option<&str>,
    ) -> (Option<String>, OdtParaStyle) {
        let Some(sname) = style_name else {
            return (None, OdtParaStyle::default());
        };
        let resolved = self.styles.resolve_para(sname);
        let named_id = if self.styles.stylesheet.paragraph_styles.contains_key(sname) {
            Some(sname.to_string())
        } else if let Some(parent) = &resolved.parent
            && self.styles.stylesheet.paragraph_styles.contains_key(parent)
        {
            Some(parent.clone())
        } else {
            None
        };
        (named_id, resolved)
    }

    fn convert_heading(&mut self, h_el: &Element, out: &mut Vec<Block>) -> Result<(), OfficeError> {
        let style_attr = h_el.attribute(NS_TEXT, "style-name");
        let (style_id, resolved_p) = self.resolve_paragraph_style_info(style_attr);
        let raw_level = h_el
            .attribute(NS_TEXT, "outline-level")
            .and_then(|v| v.parse::<u8>().ok())
            .or(resolved_p.outline_level)
            .unwrap_or(1)
            .max(1);

        let block_id = self.ids.next(if raw_level <= 6 { "h" } else { "p" });
        let mut spans = Vec::new();
        let mut inline_images = Vec::new();
        let mut active_change = None;
        let mut active_annotations = Vec::new();
        let direct_text = style_attr
            .and_then(|s| self.styles.para_styles.get(s))
            .filter(|ps| ps.is_automatic)
            .map(|ps| ps.text.clone())
            .unwrap_or_default();

        self.collect_inline_nodes(
            h_el,
            &OdtTextProps::default(),
            &direct_text,
            None,
            &mut active_change,
            &mut active_annotations,
            &block_id,
            &mut spans,
            &mut inline_images,
        )?;

        let mut extra = BTreeMap::new();
        if resolved_p.page_break_before {
            extra.insert(
                "office.page_break_before".to_string(),
                serde_json::Value::Bool(true),
            );
        }

        if raw_level <= 6 {
            let mut h = HeadingBlock::new(block_id, raw_level, spans);
            h.style_id = style_id;
            h.extra = extra;
            out.push(Block::Heading(h));
        } else {
            self.report.note(
                Severity::Approximated,
                "odt.heading.level-clamped",
                "ODF heading outline-level > 6 imported as styled paragraph",
                Some(&block_id),
            );
            let mut p = ParagraphBlock::new(block_id, spans);
            p.style_id = style_id;
            p.extra = extra;
            out.push(Block::Paragraph(p));
        }
        for img in inline_images {
            out.push(Block::Image(img));
        }
        Ok(())
    }

    fn convert_paragraph(
        &mut self,
        p_el: &Element,
        out: &mut Vec<Block>,
    ) -> Result<(), OfficeError> {
        let style_attr = p_el.attribute(NS_TEXT, "style-name");
        let (style_id, resolved_p) = self.resolve_paragraph_style_info(style_attr);
        let direct_p = style_attr
            .and_then(|s| self.styles.para_styles.get(s))
            .filter(|ps| ps.is_automatic)
            .cloned()
            .unwrap_or_default();

        let block_id = self.ids.next("p");
        let mut spans: Vec<TextSpan> = Vec::new();
        let mut inline_images: Vec<ImageBlock> = Vec::new();
        let mut active_change = None;
        let mut active_annotations = Vec::new();

        self.collect_inline_nodes(
            p_el,
            &resolved_p.text,
            &direct_p.text,
            None,
            &mut active_change,
            &mut active_annotations,
            &block_id,
            &mut spans,
            &mut inline_images,
        )?;

        let has_text = spans
            .iter()
            .any(|s| !s.text.is_empty() || s.footnote_ref.is_some());
        if !has_text && !inline_images.is_empty() {
            for img in inline_images {
                out.push(Block::Image(img));
            }
            return Ok(());
        }

        let mut p = ParagraphBlock::new(block_id, spans);
        p.style_id = style_id;
        p.alignment = direct_p.alignment;
        p.space_before_pt = direct_p.space_before_pt;
        p.space_after_pt = direct_p.space_after_pt;
        if resolved_p.page_break_before {
            p.extra.insert(
                "office.page_break_before".to_string(),
                serde_json::Value::Bool(true),
            );
        }
        out.push(Block::Paragraph(p));
        for img in inline_images {
            out.push(Block::Image(img));
        }
        Ok(())
    }

    fn convert_list(
        &mut self,
        list_el: &Element,
        inherited_style: Option<&str>,
        level: u8,
    ) -> Result<Option<ListBlock>, OfficeError> {
        let style_name = list_el.attribute(NS_TEXT, "style-name").or(inherited_style);
        let (kind, num_fmt) = style_name
            .and_then(|sn| self.styles.list_styles.get(sn))
            .and_then(|lvls| lvls.get(&level).or_else(|| lvls.get(&0)))
            .map(|l| (l.kind, l.num_fmt.clone()))
            .unwrap_or((ListKind::Unordered, "bullet".to_string()));

        let mut items = Vec::new();
        for item_el in list_el
            .child_elements()
            .filter(|e| e.is(NS_TEXT, "list-item") || e.is(NS_TEXT, "list-header"))
        {
            let mut item_spans = Vec::new();
            let mut nested_list = None;
            for child in item_el.child_elements() {
                if child.is(NS_TEXT, "p") || child.is(NS_TEXT, "h") {
                    let mut tmp = Vec::new();
                    self.convert_block(child, &mut tmp)?;
                    for b in tmp {
                        match b {
                            Block::Paragraph(mut p) => {
                                if !item_spans.is_empty() {
                                    item_spans.push(TextSpan::plain("\n"));
                                }
                                item_spans.append(&mut p.spans);
                            }
                            Block::Heading(mut h) => {
                                if !item_spans.is_empty() {
                                    item_spans.push(TextSpan::plain("\n"));
                                }
                                item_spans.append(&mut h.spans);
                            }
                            _ => {}
                        }
                    }
                } else if child.is(NS_TEXT, "list") {
                    nested_list = self
                        .convert_list(child, style_name, level.saturating_add(1))?
                        .map(Box::new);
                }
            }
            if item_spans.is_empty() {
                item_spans.push(TextSpan::plain(""));
            }
            let mut li = ListItem::new(self.ids.next("li"), item_spans);
            li.nested_list = nested_list;
            items.push(li);
        }

        if items.is_empty() {
            return Ok(None);
        }
        let mut list = ListBlock::new(self.ids.next("list"), kind, items);
        list.extra.insert(
            "office.number_format".to_string(),
            serde_json::Value::String(num_fmt),
        );
        Ok(Some(list))
    }

    fn convert_table(&mut self, tbl_el: &Element) -> Result<Option<TableBlock>, OfficeError> {
        let mut col_widths = Vec::new();
        let mut rows = Vec::new();
        let mut repeat_header_rows = 0u32;

        for child in tbl_el.child_elements() {
            if child.is(NS_TABLE, "table-column") {
                let repeat = child
                    .attribute(NS_TABLE, "number-columns-repeated")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, 256);
                if let Some(sname) = child.attribute(NS_TABLE, "style-name")
                    && let Some(w) = self.styles.column_widths.get(sname).copied()
                {
                    for _ in 0..repeat {
                        if col_widths.len() < 256 {
                            col_widths.push(w);
                        }
                    }
                }
            } else if child.is(NS_TABLE, "table-header-rows") {
                for r in child
                    .child_elements()
                    .filter(|e| e.is(NS_TABLE, "table-row"))
                {
                    let before = rows.len();
                    self.convert_table_row(r, true, &mut rows)?;
                    repeat_header_rows += (rows.len() - before) as u32;
                }
            } else if child.is(NS_TABLE, "table-row") {
                self.convert_table_row(child, false, &mut rows)?;
            }
        }

        if rows.is_empty() {
            return Ok(None);
        }
        let mut tbl = TableBlock::new(self.ids.next("tbl"), rows);
        tbl.column_widths_pt = col_widths;
        tbl.repeat_header_rows = repeat_header_rows;
        Ok(Some(tbl))
    }

    fn convert_table_row(
        &mut self,
        row_el: &Element,
        is_header: bool,
        rows: &mut Vec<TableRow>,
    ) -> Result<(), OfficeError> {
        let repeat = row_el
            .attribute(NS_TABLE, "number-rows-repeated")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1)
            .clamp(1, 64);

        let mut cells = Vec::new();
        for cell_el in row_el.child_elements() {
            if cell_el.is(NS_TABLE, "covered-table-cell") {
                continue;
            }
            if !cell_el.is(NS_TABLE, "table-cell") {
                continue;
            }
            let repeat_cell = cell_el
                .attribute(NS_TABLE, "number-columns-repeated")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(1)
                .clamp(1, 64);
            let col_span = cell_el
                .attribute(NS_TABLE, "number-columns-spanned")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(1)
                .clamp(1, 64);
            let row_span = cell_el
                .attribute(NS_TABLE, "number-rows-spanned")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(1)
                .clamp(1, 64);
            let bg = cell_el
                .attribute(NS_TABLE, "style-name")
                .and_then(|sn| self.styles.cell_backgrounds.get(sn).copied());

            let mut blocks = Vec::new();
            for child in cell_el.child_elements() {
                self.convert_block(child, &mut blocks)?;
            }
            let is_blank = blocks.is_empty();
            let count = if is_blank && repeat_cell > 16 {
                1
            } else {
                repeat_cell
            };
            for _ in 0..count {
                let mut tc = TableCell::new(self.ids.next("cell"), blocks.clone());
                tc.col_span = col_span;
                tc.row_span = row_span;
                tc.background_color = bg;
                cells.push(tc);
            }
        }
        if cells.is_empty() {
            return Ok(());
        }
        let row_repeats = if repeat > 4 && cells.iter().all(|c| c.blocks.is_empty()) {
            1
        } else {
            repeat
        };
        for _ in 0..row_repeats {
            let mut r = TableRow::new(
                cells
                    .iter()
                    .map(|c| {
                        let mut cloned = c.clone();
                        cloned.id = self.ids.next("cell");
                        cloned
                    })
                    .collect(),
            );
            r.is_header = is_header;
            rows.push(r);
        }
        Ok(())
    }

    fn assemble_comments(&mut self) -> Vec<FlowComment> {
        std::mem::take(&mut self.annotations)
            .into_iter()
            .map(|ann| {
                let end = ann
                    .char_end
                    .unwrap_or(ann.char_start + ann.anchor_text.chars().count())
                    .max(ann.char_start);
                let mut c = FlowComment::new(
                    format!("comment_{}", ann.name),
                    ann.author,
                    ann.date,
                    ann.content,
                    Some(ann.block_id),
                    Some((ann.char_start, end)),
                    (!ann.anchor_text.is_empty()).then_some(ann.anchor_text),
                );
                c.resolved = ann.resolved;
                c.replies = ann.replies;
                c
            })
            .collect()
    }
}

#[allow(dead_code)]
fn _keep_props(_: &OdtTextProps) {}
