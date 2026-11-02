//! DOCX export (`FlowDocument` + `AssetMap` -> ECMA-376 `.docx` ZIP package).

use std::collections::{BTreeMap, HashMap};

use vocument_flow::model::{
    Block, FlowDocument, HeadingBlock, ImageBlock, ListBlock, ListKind, PageOrientation,
    ParagraphBlock, Section, SectionGeometry, TableBlock,
};
use vocument_flow::review::{ChangeType, TrackedChange};
use vocument_flow::styles::{Alignment, TextSpan};

use crate::convert::{AssetMap, ExportedDocument};
use crate::docx::export_parts::{
    NS_A, NS_PIC, NS_R, NS_W, NS_WP, build_app_props, build_comments_xml, build_content_types,
    build_core_props, build_custom_props, build_document_rels, build_numbering_xml,
    build_root_rels, build_styles_xml, write_font_style_tags,
};
use crate::error::OfficeError;
use crate::package::PackageWriter;
use crate::report::{ConversionReport, Severity};
use crate::util::{color_hex_rgb, pt_to_emu, pt_to_twips, safe_hyperlink, sniff_image};
use crate::xml::{escape_attribute, escape_text};

#[derive(Clone)]
pub(super) struct ExportRel {
    pub(super) id: String,
    pub(super) rel_type: String,
    pub(super) target: String,
    pub(super) external: bool,
}

struct ExportContext<'a> {
    doc: &'a FlowDocument,
    assets: &'a AssetMap,
    report: ConversionReport,
    rels: Vec<ExportRel>,
    next_rel_num: u32,
    list_defs: Vec<(u32, ListKind, String)>,
    footnote_order: Vec<String>,
    footnote_num_by_id: HashMap<String, u32>,
    comment_xml_ids_by_block: HashMap<String, Vec<(u32, usize, usize)>>,
    tracked_by_id: HashMap<String, &'a TrackedChange>,
    media_parts: BTreeMap<String, Vec<u8>>,
    media_types: BTreeMap<String, String>,
    header_parts: Vec<(String, String)>,
    footer_parts: Vec<(String, String)>,
    drawing_seq: u32,
    change_seq: u32,
}

/// Exports a [`FlowDocument`] and its referenced binary [`AssetMap`] into a valid DOCX archive.
pub fn export(document: &FlowDocument, assets: &AssetMap) -> Result<ExportedDocument, OfficeError> {
    let mut ctx = ExportContext::new(document, assets);
    let document_xml = ctx.build_document_xml();

    let has_numbering = !ctx.list_defs.is_empty();
    let has_footnotes = !ctx.footnote_order.is_empty();
    let has_comments = !document.review.comments.is_empty();
    let has_custom_props =
        !document.metadata.custom_attributes.is_empty() || !document.metadata.extra.is_empty();

    ctx.add_rel(
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles",
        "styles.xml",
        false,
    );
    if has_numbering {
        ctx.add_rel(
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering",
            "numbering.xml",
            false,
        );
    }
    if has_footnotes {
        ctx.add_rel(
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes",
            "footnotes.xml",
            false,
        );
    }
    if has_comments {
        ctx.add_rel(
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments",
            "comments.xml",
            false,
        );
        ctx.add_rel(
            "http://schemas.microsoft.com/office/2011/relationships/commentsExtended",
            "commentsExtended.xml",
            false,
        );
    }

    let footnotes_xml = if has_footnotes {
        Some(ctx.build_footnotes_xml())
    } else {
        None
    };

    let hdr_names: Vec<String> = ctx.header_parts.iter().map(|(n, _)| n.clone()).collect();
    let ftr_names: Vec<String> = ctx.footer_parts.iter().map(|(n, _)| n.clone()).collect();
    let media_exts: Vec<(&str, &str)> = ctx
        .media_types
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let mut pkg = PackageWriter::new();
    pkg.add(
        "[Content_Types].xml",
        build_content_types(
            has_numbering,
            has_footnotes,
            has_comments,
            has_custom_props,
            &hdr_names,
            &ftr_names,
            &media_exts,
        )
        .as_bytes(),
    )?;
    pkg.add("_rels/.rels", build_root_rels(has_custom_props).as_bytes())?;
    pkg.add(
        "docProps/core.xml",
        build_core_props(&document.metadata).as_bytes(),
    )?;
    pkg.add("docProps/app.xml", build_app_props().as_bytes())?;
    if has_custom_props {
        pkg.add(
            "docProps/custom.xml",
            build_custom_props(&document.metadata).as_bytes(),
        )?;
    }
    pkg.add("word/document.xml", document_xml.as_bytes())?;
    pkg.add(
        "word/_rels/document.xml.rels",
        build_document_rels(&ctx.rels).as_bytes(),
    )?;
    pkg.add("word/styles.xml", build_styles_xml(document).as_bytes())?;

    if has_numbering {
        pkg.add(
            "word/numbering.xml",
            build_numbering_xml(&ctx.list_defs).as_bytes(),
        )?;
    }
    if let Some(fn_xml) = footnotes_xml {
        pkg.add("word/footnotes.xml", fn_xml.as_bytes())?;
    }
    if has_comments {
        let (c_xml, cex_xml) = build_comments_xml(document);
        pkg.add("word/comments.xml", c_xml.as_bytes())?;
        pkg.add("word/commentsExtended.xml", cex_xml.as_bytes())?;
    }
    for (name, xml) in &ctx.header_parts {
        pkg.add(&format!("word/{name}"), xml.as_bytes())?;
    }
    for (name, xml) in &ctx.footer_parts {
        pkg.add(&format!("word/{name}"), xml.as_bytes())?;
    }
    for (path, bytes) in ctx.media_parts {
        pkg.add(&path, &bytes)?;
    }

    let bytes = pkg.finish()?;
    Ok(ExportedDocument {
        bytes,
        report: ctx.report,
    })
}

impl<'a> ExportContext<'a> {
    fn new(doc: &'a FlowDocument, assets: &'a AssetMap) -> Self {
        let mut comment_xml_ids_by_block: HashMap<String, Vec<(u32, usize, usize)>> =
            HashMap::new();
        let mut next_cid = 1u32;
        for c in &doc.review.comments {
            let root_id = next_cid;
            next_cid += 1 + (c.replies.len() as u32);
            if let Some(block_id) = &c.anchor_block_id {
                let s = c.anchor_char_start.unwrap_or(0);
                let e = c.anchor_char_end.unwrap_or(s);
                comment_xml_ids_by_block
                    .entry(block_id.clone())
                    .or_default()
                    .push((root_id, s, e));
            }
        }
        let tracked_by_id = doc
            .review
            .tracked_changes
            .iter()
            .map(|tc| (tc.change_id.clone(), tc))
            .collect();

        Self {
            doc,
            assets,
            report: ConversionReport::new(),
            rels: Vec::new(),
            next_rel_num: 1,
            list_defs: Vec::new(),
            footnote_order: Vec::new(),
            footnote_num_by_id: HashMap::new(),
            comment_xml_ids_by_block,
            tracked_by_id,
            media_parts: BTreeMap::new(),
            media_types: BTreeMap::new(),
            header_parts: Vec::new(),
            footer_parts: Vec::new(),
            drawing_seq: 1,
            change_seq: 1,
        }
    }

    fn add_rel(&mut self, rel_type: &str, target: &str, external: bool) -> String {
        let id = format!("rId{}", self.next_rel_num);
        self.next_rel_num += 1;
        self.rels.push(ExportRel {
            id: id.clone(),
            rel_type: rel_type.to_string(),
            target: target.to_string(),
            external,
        });
        id
    }

    fn build_document_xml(&mut self) -> String {
        let mut xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\" xmlns:wp=\"{NS_WP}\" xmlns:a=\"{NS_A}\" xmlns:pic=\"{NS_PIC}\">\n  <w:body>"
        );
        let section_count = self.doc.sections.len();
        for (idx, section) in self.doc.sections.iter().enumerate() {
            for block in &section.blocks {
                self.write_block(&mut xml, block);
            }
            let sect_pr = self.build_sect_pr(section, idx + 1);
            if idx + 1 < section_count {
                xml.push_str(&format!("\n    <w:p><w:pPr>{sect_pr}</w:pPr></w:p>"));
            } else {
                xml.push_str(&format!("\n    {sect_pr}"));
            }
        }
        xml.push_str("\n  </w:body>\n</w:document>\n");
        xml
    }

    fn build_sect_pr(&mut self, section: &Section, sec_num: usize) -> String {
        let mut sect = String::from("<w:sectPr>");
        if !section.header_blocks.is_empty() {
            let name = format!("header{sec_num}.xml");
            let mut h_xml = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:hdr xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\" xmlns:wp=\"{NS_WP}\" xmlns:a=\"{NS_A}\" xmlns:pic=\"{NS_PIC}\">"
            );
            for block in &section.header_blocks {
                self.write_block(&mut h_xml, block);
            }
            h_xml.push_str("\n</w:hdr>\n");
            self.header_parts.push((name.clone(), h_xml));
            let rid = self.add_rel(
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header",
                &name,
                false,
            );
            sect.push_str(&format!(
                "<w:headerReference w:type=\"default\" r:id=\"{rid}\"/>"
            ));
        }
        if !section.footer_blocks.is_empty() {
            let name = format!("footer{sec_num}.xml");
            let mut f_xml = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:ftr xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\" xmlns:wp=\"{NS_WP}\" xmlns:a=\"{NS_A}\" xmlns:pic=\"{NS_PIC}\">"
            );
            for block in &section.footer_blocks {
                self.write_block(&mut f_xml, block);
            }
            f_xml.push_str("\n</w:ftr>\n");
            self.footer_parts.push((name.clone(), f_xml));
            let rid = self.add_rel(
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer",
                &name,
                false,
            );
            sect.push_str(&format!(
                "<w:footerReference w:type=\"default\" r:id=\"{rid}\"/>"
            ));
        }

        let g: &SectionGeometry = &section.geometry;
        let w = pt_to_twips(g.width_pt).max(1440);
        let h = pt_to_twips(g.height_pt).max(1440);
        let orient = match g.orientation {
            PageOrientation::Landscape => " w:orient=\"landscape\"",
            PageOrientation::Portrait => "",
        };
        sect.push_str(&format!("<w:pgSz w:w=\"{w}\" w:h=\"{h}\"{orient}/>"));
        sect.push_str(&format!(
            "<w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/>",
            pt_to_twips(g.margin_top_pt).max(0),
            pt_to_twips(g.margin_right_pt).max(0),
            pt_to_twips(g.margin_bottom_pt).max(0),
            pt_to_twips(g.margin_left_pt).max(0),
        ));
        if g.column_count > 1 {
            sect.push_str(&format!(
                "<w:cols w:num=\"{}\" w:space=\"{}\"/>",
                g.column_count,
                pt_to_twips(g.column_gap_pt).max(0)
            ));
        }
        sect.push_str("</w:sectPr>");
        sect
    }

    fn build_footnotes_xml(&mut self) -> String {
        let mut xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:footnotes xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\">"
        );
        let order = self.footnote_order.clone();
        for (idx, fn_id) in order.iter().enumerate() {
            let num = (idx as u32) + 1;
            xml.push_str(&format!("\n  <w:footnote w:id=\"{num}\">"));
            if let Some(def) = self.doc.footnotes.get(fn_id) {
                for block in &def.blocks {
                    self.write_block(&mut xml, block);
                }
            } else {
                xml.push_str("<w:p><w:r><w:t/></w:r></w:p>");
            }
            xml.push_str("</w:footnote>");
        }
        xml.push_str("\n</w:footnotes>\n");
        xml
    }

    fn write_block(&mut self, xml: &mut String, block: &Block) {
        match block {
            Block::Paragraph(p) => self.write_paragraph(xml, p),
            Block::Heading(h) => self.write_heading(xml, h),
            Block::List(l) => self.write_list(xml, l),
            Block::Table(t) => self.write_table(xml, t),
            Block::Image(img) => self.write_image(xml, img),
            Block::Callout(callout) => {
                for b in &callout.blocks {
                    self.write_block(xml, b);
                }
            }
            Block::ThematicBreak(_) => {
                xml.push_str(
                    "\n    <w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"auto\"/></w:pBdr></w:pPr></w:p>",
                );
            }
        }
    }

    fn write_paragraph(&mut self, xml: &mut String, p: &ParagraphBlock) {
        xml.push_str("\n    <w:p><w:pPr>");
        if let Some(sid) = &p.style_id {
            xml.push_str(&format!("<w:pStyle w:val=\"{}\"/>", escape_attribute(sid)));
        }
        if let Some(align) = p.alignment {
            let jc = match align {
                Alignment::Left => "left",
                Alignment::Center => "center",
                Alignment::Right => "right",
                Alignment::Justified => "both",
            };
            xml.push_str(&format!("<w:jc w:val=\"{jc}\"/>"));
        }
        if p.space_before_pt.is_some() || p.space_after_pt.is_some() {
            xml.push_str("<w:spacing");
            if let Some(sb) = p.space_before_pt {
                xml.push_str(&format!(" w:before=\"{}\"", pt_to_twips(sb).max(0)));
            }
            if let Some(sa) = p.space_after_pt {
                xml.push_str(&format!(" w:after=\"{}\"", pt_to_twips(sa).max(0)));
            }
            xml.push_str("/>");
        }
        write_extra_ppr(xml, &p.extra);
        xml.push_str("</w:pPr>");
        self.write_spans(xml, &p.id, &p.spans);
        xml.push_str("</w:p>");
    }

    fn write_heading(&mut self, xml: &mut String, h: &HeadingBlock) {
        let lvl = h.level.clamp(1, 6);
        let style_id = h
            .style_id
            .clone()
            .unwrap_or_else(|| format!("Heading{lvl}"));
        xml.push_str(&format!(
            "\n    <w:p><w:pPr><w:pStyle w:val=\"{}\"/><w:outlineLvl w:val=\"{}\"/>",
            escape_attribute(&style_id),
            lvl - 1
        ));
        write_extra_ppr(xml, &h.extra);
        xml.push_str("</w:pPr>");
        self.write_spans(xml, &h.id, &h.spans);
        xml.push_str("</w:p>");
    }

    fn write_list(&mut self, xml: &mut String, list: &ListBlock) {
        let num_id = (self.list_defs.len() as u32) + 1;
        let num_fmt = list
            .extra
            .get("office.number_format")
            .and_then(|v| v.as_str())
            .unwrap_or(match list.kind {
                ListKind::Ordered => "decimal",
                ListKind::Unordered | ListKind::Task => "bullet",
            })
            .to_string();
        self.list_defs.push((num_id, list.kind, num_fmt));
        self.write_list_items(xml, list, num_id, 0);
    }

    fn write_list_items(&mut self, xml: &mut String, list: &ListBlock, num_id: u32, level: u8) {
        let ilvl = level.min(8);
        for item in &list.items {
            let has_text = item
                .spans
                .iter()
                .any(|s| !s.text.is_empty() || s.footnote_ref.is_some());
            if has_text || item.nested_list.is_none() {
                xml.push_str(&format!(
                    "\n    <w:p><w:pPr><w:numPr><w:ilvl w:val=\"{ilvl}\"/><w:numId w:val=\"{num_id}\"/></w:numPr></w:pPr>"
                ));
                self.write_spans(xml, &item.id, &item.spans);
                xml.push_str("</w:p>");
            }
            if let Some(nested) = &item.nested_list {
                self.write_list_items(xml, nested, num_id, ilvl + 1);
            }
        }
    }

    fn write_table(&mut self, xml: &mut String, tbl: &TableBlock) {
        xml.push_str("\n    <w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/>");
        if let Some(cap) = &tbl.caption
            && !cap.is_empty()
        {
            xml.push_str(&format!(
                "<w:tblCaption w:val=\"{}\"/>",
                escape_attribute(cap)
            ));
        }
        xml.push_str("<w:tblBorders><w:top w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"CCCCCC\"/><w:left w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"CCCCCC\"/><w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"CCCCCC\"/><w:right w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"CCCCCC\"/><w:insideH w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"CCCCCC\"/><w:insideV w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"CCCCCC\"/></w:tblBorders></w:tblPr>");

        xml.push_str("<w:tblGrid>");
        for w in &tbl.column_widths_pt {
            xml.push_str(&format!(
                "<w:gridCol w:w=\"{}\"/>",
                pt_to_twips(*w).max(120)
            ));
        }
        xml.push_str("</w:tblGrid>");

        let mut active_vmerges: BTreeMap<usize, (u32, u32)> = BTreeMap::new();

        for (r_idx, row) in tbl.rows.iter().enumerate() {
            xml.push_str("\n      <w:tr>");
            if row.is_header || (r_idx as u32) < tbl.repeat_header_rows {
                xml.push_str("<w:trPr><w:tblHeader/></w:trPr>");
            }
            let mut col_idx = 0usize;
            for cell in &row.cells {
                while let Some((rem, cspan)) = active_vmerges.get(&col_idx).copied() {
                    emit_vmerge_continue_cell(xml, cspan);
                    if rem <= 1 {
                        active_vmerges.remove(&col_idx);
                    } else {
                        active_vmerges.insert(col_idx, (rem - 1, cspan));
                    }
                    col_idx += cspan as usize;
                }
                xml.push_str("<w:tc><w:tcPr>");
                if cell.col_span > 1 {
                    xml.push_str(&format!("<w:gridSpan w:val=\"{}\"/>", cell.col_span));
                }
                if cell.row_span > 1 {
                    xml.push_str("<w:vMerge w:val=\"restart\"/>");
                    active_vmerges.insert(col_idx, (cell.row_span - 1, cell.col_span));
                }
                if let Some(bg) = cell.background_color {
                    xml.push_str(&format!(
                        "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
                        color_hex_rgb(bg)
                    ));
                }
                xml.push_str("</w:tcPr>");
                let mut cell_xml = String::new();
                for block in &cell.blocks {
                    self.write_block(&mut cell_xml, block);
                }
                if !cell_xml.contains("<w:p>") {
                    cell_xml.push_str("<w:p/>");
                }
                xml.push_str(&cell_xml);
                xml.push_str("</w:tc>");
                col_idx += cell.col_span as usize;
            }
            let trailing_cols: Vec<usize> =
                active_vmerges.range(col_idx..).map(|(k, _)| *k).collect();
            for c in trailing_cols {
                if let Some((rem, cspan)) = active_vmerges.get(&c).copied() {
                    emit_vmerge_continue_cell(xml, cspan);
                    if rem <= 1 {
                        active_vmerges.remove(&c);
                    } else {
                        active_vmerges.insert(c, (rem - 1, cspan));
                    }
                }
            }
            xml.push_str("</w:tr>");
        }
        xml.push_str("\n    </w:tbl>");
    }

    fn write_image(&mut self, xml: &mut String, img: &ImageBlock) {
        let Some(asset) = self.assets.get(&img.asset_ref) else {
            self.report.note(
                Severity::Loss,
                "docx.export.missing-image-asset",
                &format!(
                    "Referenced image asset '{}' not provided in AssetMap",
                    img.asset_ref
                ),
                Some(&img.id),
            );
            xml.push_str(&format!(
                "\n    <w:p><w:r><w:t>[Image: {}]</w:t></w:r></w:p>",
                escape_text(&img.alt_text)
            ));
            return;
        };
        let info = sniff_image(&asset.bytes);
        let (ext, mime) = match &info {
            Some(i) => (i.extension.to_string(), i.media_type.to_string()),
            None => ("png".to_string(), asset.media_type.clone()),
        };
        let seq = self.drawing_seq;
        self.drawing_seq += 1;
        let rel_target = format!("media/image{seq}.{ext}");
        let pkg_path = format!("word/{rel_target}");
        self.media_parts.insert(pkg_path, asset.bytes.clone());
        self.media_types.insert(ext, mime);
        let rid = self.add_rel(
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
            &rel_target,
            false,
        );

        let cx = pt_to_emu(
            img.display_width_pt
                .unwrap_or(img.intrinsic_width_pt)
                .max(1.0),
        );
        let cy = pt_to_emu(
            img.display_height_pt
                .unwrap_or(img.intrinsic_height_pt)
                .max(1.0),
        );
        let alt = escape_attribute(&img.alt_text);

        xml.push_str(&format!(
            "\n    <w:p><w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:docPr id=\"{seq}\" name=\"Picture {seq}\" descr=\"{alt}\"/><a:graphic><a:graphicData uri=\"{NS_PIC}\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"{seq}\" name=\"Picture {seq}\" descr=\"{alt}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"{rid}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"
        ));
    }

    fn write_spans(&mut self, xml: &mut String, block_id: &str, spans: &[TextSpan]) {
        let anchors = self
            .comment_xml_ids_by_block
            .get(block_id)
            .cloned()
            .unwrap_or_default();
        let mut started_comments = Vec::new();
        let mut finished_comments = Vec::new();
        let mut cursor = 0usize;

        for (cid, start, _) in &anchors {
            if *start == 0 {
                xml.push_str(&format!("<w:commentRangeStart w:id=\"{cid}\"/>"));
                started_comments.push(*cid);
            }
        }

        for span in spans {
            let span_len = span.text.chars().count();
            for (cid, start, _) in &anchors {
                if !started_comments.contains(cid) && *start <= cursor {
                    xml.push_str(&format!("<w:commentRangeStart w:id=\"{cid}\"/>"));
                    started_comments.push(*cid);
                }
            }
            self.write_single_span(xml, span);
            cursor += span_len;
            for (cid, _, end) in &anchors {
                if started_comments.contains(cid)
                    && !finished_comments.contains(cid)
                    && *end <= cursor
                {
                    xml.push_str(&format!(
                        "<w:commentRangeEnd w:id=\"{cid}\"/><w:r><w:commentReference w:id=\"{cid}\"/></w:r>"
                    ));
                    finished_comments.push(*cid);
                }
            }
        }

        for (cid, _, _) in &anchors {
            if !started_comments.contains(cid) {
                xml.push_str(&format!("<w:commentRangeStart w:id=\"{cid}\"/>"));
            }
            if !finished_comments.contains(cid) {
                xml.push_str(&format!(
                    "<w:commentRangeEnd w:id=\"{cid}\"/><w:r><w:commentReference w:id=\"{cid}\"/></w:r>"
                ));
            }
        }
    }

    fn write_single_span(&mut self, xml: &mut String, span: &TextSpan) {
        if let Some(fn_id) = &span.footnote_ref {
            let num = match self.footnote_num_by_id.get(fn_id).copied() {
                Some(n) => n,
                None => {
                    let n = (self.footnote_order.len() as u32) + 1;
                    self.footnote_order.push(fn_id.clone());
                    self.footnote_num_by_id.insert(fn_id.clone(), n);
                    n
                }
            };
            xml.push_str(&format!(
                "<w:r><w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr><w:footnoteReference w:id=\"{num}\"/></w:r>"
            ));
            if span.text.is_empty() {
                return;
            }
        }

        let tc = span
            .tracked_change_id
            .as_ref()
            .and_then(|id| self.tracked_by_id.get(id).copied());

        let mut link_open = String::new();
        let mut link_close = "";
        if let Some(url) = &span.link_url
            && let Some(safe) = safe_hyperlink(url)
        {
            if let Some(anchor) = safe.strip_prefix('#') {
                link_open = format!("<w:hyperlink w:anchor=\"{}\">", escape_attribute(anchor));
            } else {
                let rid = self.add_rel(
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink",
                    &safe,
                    true,
                );
                link_open = format!("<w:hyperlink r:id=\"{rid}\">");
            }
            link_close = "</w:hyperlink>";
        }

        let mut chg_open = String::new();
        let mut chg_close = "";
        let mut is_delete = false;
        let mut rpr_change_xml = String::new();

        if let Some(change) = tc {
            let cid = self.change_seq;
            self.change_seq += 1;
            let author = escape_attribute(&change.author);
            let date = escape_attribute(&change.timestamp);
            match change.change_type {
                ChangeType::Insert => {
                    chg_open =
                        format!("<w:ins w:id=\"{cid}\" w:author=\"{author}\" w:date=\"{date}\">");
                    chg_close = "</w:ins>";
                }
                ChangeType::Delete => {
                    chg_open =
                        format!("<w:del w:id=\"{cid}\" w:author=\"{author}\" w:date=\"{date}\">");
                    chg_close = "</w:del>";
                    is_delete = true;
                }
                ChangeType::FormatChange => {
                    rpr_change_xml.push_str(&format!(
                        "<w:rPrChange w:id=\"{cid}\" w:author=\"{author}\" w:date=\"{date}\"><w:rPr>"
                    ));
                    if let Some(orig) = &span.original_style {
                        write_font_style_tags(&mut rpr_change_xml, orig);
                    }
                    rpr_change_xml.push_str("</w:rPr></w:rPrChange>");
                }
            }
        }

        if !is_delete && (span.text == "{page}" || span.text == "{total_pages}") {
            let instr = if span.text == "{page}" {
                "PAGE"
            } else {
                "NUMPAGES"
            };
            xml.push_str(&format!(
                "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> {instr} </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r>"
            ));
            return;
        }

        xml.push_str(&link_open);
        xml.push_str(&chg_open);
        xml.push_str("<w:r>");
        write_run_props(xml, span, is_delete, &rpr_change_xml);
        write_run_text_tokens(xml, &span.text, is_delete);
        xml.push_str("</w:r>");
        xml.push_str(chg_close);
        xml.push_str(link_close);
    }
}

fn write_run_props(xml: &mut String, span: &TextSpan, is_delete: bool, rpr_change_xml: &str) {
    let mut rpr = String::new();
    if let Some(fam) = &span.font_family {
        let f = escape_attribute(fam);
        rpr.push_str(&format!(
            "<w:rFonts w:ascii=\"{f}\" w:hAnsi=\"{f}\" w:cs=\"{f}\"/>"
        ));
    }
    let mut fs = span.style;
    if is_delete {
        fs.strikethrough = false;
    }
    write_font_style_tags(&mut rpr, &fs);
    if let Some(c) = span.color {
        rpr.push_str(&format!("<w:color w:val=\"{}\"/>", color_hex_rgb(c)));
    }
    if let Some(bg) = span.background_color {
        rpr.push_str(&format!(
            "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
            color_hex_rgb(bg)
        ));
    }
    if let Some(sz) = span.font_size_pt {
        let hp = ((sz * 2.0).round() as i64).clamp(8, 400);
        rpr.push_str(&format!("<w:sz w:val=\"{hp}\"/><w:szCs w:val=\"{hp}\"/>"));
    }
    rpr.push_str(rpr_change_xml);
    if !rpr.is_empty() {
        xml.push_str("<w:rPr>");
        xml.push_str(&rpr);
        xml.push_str("</w:rPr>");
    }
}

fn write_run_text_tokens(xml: &mut String, text: &str, is_delete: bool) {
    let tag = if is_delete { "w:delText" } else { "w:t" };
    let mut buf = String::new();
    let flush = |xml: &mut String, buf: &mut String| {
        if !buf.is_empty() {
            xml.push_str(&format!(
                "<{tag} xml:space=\"preserve\">{}</{tag}>",
                escape_text(buf)
            ));
            buf.clear();
        }
    };
    for ch in text.chars() {
        match ch {
            '\t' => {
                flush(xml, &mut buf);
                xml.push_str("<w:tab/>");
            }
            '\n' => {
                flush(xml, &mut buf);
                xml.push_str("<w:br/>");
            }
            other => buf.push(other),
        }
    }
    flush(xml, &mut buf);
}

fn write_extra_ppr(xml: &mut String, extra: &BTreeMap<String, serde_json::Value>) {
    if extra
        .get("office.page_break_before")
        .and_then(|v| v.as_bool())
        == Some(true)
    {
        xml.push_str("<w:pageBreakBefore/>");
    }
    if let Some(serde_json::Value::Array(raw_list)) = extra.get("office.docx.pPr_unmapped") {
        for raw in raw_list.iter().filter_map(|v| v.as_str()) {
            xml.push_str(raw);
        }
    }
}

fn emit_vmerge_continue_cell(xml: &mut String, col_span: u32) {
    xml.push_str("<w:tc><w:tcPr>");
    if col_span > 1 {
        xml.push_str(&format!("<w:gridSpan w:val=\"{col_span}\"/>"));
    }
    xml.push_str("<w:vMerge/></w:tcPr><w:p/></w:tc>");
}
