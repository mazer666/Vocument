//! ODT export (`FlowDocument` + `AssetMap` -> ODF 1.2 `.odt` ZIP package).

use std::collections::{BTreeMap, HashMap};

use vocument_flow::model::{
    Block, FlowDocument, HeadingBlock, ImageBlock, ListBlock, ListKind, ParagraphBlock, TableBlock,
};
use vocument_flow::review::{ChangeType, TrackedChange};
use vocument_flow::styles::{Alignment, TextSpan};

use crate::convert::{AssetMap, ExportedDocument};
use crate::error::OfficeError;
use crate::odt::export_parts::{
    ODT_MIMETYPE, build_manifest_xml, build_meta_xml, build_styles_xml, format_text_props_attrs,
};
use crate::odt::styles::{
    NS_DC, NS_DRAW, NS_FO, NS_LOEXT, NS_OFFICE, NS_STYLE, NS_SVG, NS_TABLE, NS_TEXT, NS_XLINK,
};
use crate::package::PackageWriter;
use crate::report::{ConversionReport, Severity};
use crate::util::{color_hex_rgb, format_length_cm, safe_hyperlink, sniff_image};
use crate::xml::{escape_attribute, escape_text};

struct OdtExportContext<'a> {
    doc: &'a FlowDocument,
    assets: &'a AssetMap,
    report: ConversionReport,
    auto_styles_xml: String,
    p_seq: u32,
    t_seq: u32,
    l_seq: u32,
    col_seq: u32,
    cell_seq: u32,
    img_seq: u32,
    fn_seq: u32,
    pictures: BTreeMap<String, (String, Vec<u8>)>,
    comments_by_block: HashMap<String, Vec<(usize, usize, usize)>>,
    tracked_by_id: HashMap<String, &'a TrackedChange>,
}

/// Exports a [`FlowDocument`] and its referenced binary [`AssetMap`] into a valid ODT archive.
pub fn export(document: &FlowDocument, assets: &AssetMap) -> Result<ExportedDocument, OfficeError> {
    let mut ctx = OdtExportContext::new(document, assets);

    let mut header_xml = String::new();
    let mut footer_xml = String::new();
    if let Some(first_sec) = document.sections.first() {
        for b in &first_sec.header_blocks {
            ctx.write_block(&mut header_xml, b);
        }
        for b in &first_sec.footer_blocks {
            ctx.write_block(&mut footer_xml, b);
        }
    }

    let content_xml = ctx.build_content_xml();
    let styles_xml = build_styles_xml(document, &header_xml, &footer_xml);
    let meta_xml = build_meta_xml(&document.metadata);

    let pic_manifest: Vec<(String, String)> = ctx
        .pictures
        .iter()
        .map(|(path, (mime, _))| (path.clone(), mime.clone()))
        .collect();
    let manifest_xml = build_manifest_xml(&pic_manifest);

    let mut pkg = PackageWriter::new();
    pkg.add_stored("mimetype", ODT_MIMETYPE.as_bytes())?;
    pkg.add("meta.xml", meta_xml.as_bytes())?;
    pkg.add("styles.xml", styles_xml.as_bytes())?;
    pkg.add("content.xml", content_xml.as_bytes())?;
    for (path, (_, bytes)) in ctx.pictures {
        pkg.add(&path, &bytes)?;
    }
    pkg.add("META-INF/manifest.xml", manifest_xml.as_bytes())?;

    let bytes = pkg.finish()?;
    Ok(ExportedDocument {
        bytes,
        report: ctx.report,
    })
}

impl<'a> OdtExportContext<'a> {
    fn new(doc: &'a FlowDocument, assets: &'a AssetMap) -> Self {
        let mut comments_by_block: HashMap<String, Vec<(usize, usize, usize)>> = HashMap::new();
        for (idx, c) in doc.review.comments.iter().enumerate() {
            if let Some(bid) = &c.anchor_block_id {
                let s = c.anchor_char_start.unwrap_or(0);
                let e = c.anchor_char_end.unwrap_or(s);
                comments_by_block
                    .entry(bid.clone())
                    .or_default()
                    .push((idx, s, e));
            }
        }
        let tracked_by_id = doc
            .review
            .tracked_changes
            .iter()
            .map(|tc| (tc.change_id.clone(), tc))
            .collect();

        let mut auto_styles_xml = String::new();
        auto_styles_xml.push_str(
            "\n    <style:style style:name=\"fr1\" style:family=\"graphic\"><style:graphic-properties style:wrap=\"none\" style:horizontal-pos=\"center\" style:horizontal-rel=\"paragraph\"/></style:style>",
        );

        Self {
            doc,
            assets,
            report: ConversionReport::new(),
            auto_styles_xml,
            p_seq: 1,
            t_seq: 1,
            l_seq: 1,
            col_seq: 1,
            cell_seq: 1,
            img_seq: 1,
            fn_seq: 1,
            pictures: BTreeMap::new(),
            comments_by_block,
            tracked_by_id,
        }
    }

    fn build_content_xml(&mut self) -> String {
        let mut body_xml = String::new();
        if !self.doc.review.tracked_changes.is_empty() {
            self.write_tracked_changes_table(&mut body_xml);
        }
        for section in &self.doc.sections {
            for block in &section.blocks {
                self.write_block(&mut body_xml, block);
            }
        }

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="{NS_OFFICE}" xmlns:style="{NS_STYLE}" xmlns:text="{NS_TEXT}" xmlns:table="{NS_TABLE}" xmlns:draw="{NS_DRAW}" xmlns:fo="{NS_FO}" xmlns:xlink="{NS_XLINK}" xmlns:dc="{NS_DC}" xmlns:svg="{NS_SVG}" xmlns:loext="{NS_LOEXT}" office:version="1.2">
  <office:automatic-styles>{}
  </office:automatic-styles>
  <office:body>
    <office:text>{}
    </office:text>
  </office:body>
</office:document-content>
"#,
            self.auto_styles_xml, body_xml
        )
    }

    fn write_tracked_changes_table(&self, xml: &mut String) {
        xml.push_str("\n      <text:tracked-changes>");
        for tc in &self.doc.review.tracked_changes {
            let cid = escape_attribute(&tc.change_id);
            let author = escape_text(&tc.author);
            let date = escape_text(&tc.timestamp);
            xml.push_str(&format!(
                "\n        <text:changed-region text:id=\"{cid}\">"
            ));
            match tc.change_type {
                ChangeType::Insert => {
                    xml.push_str(&format!(
                        "<text:insertion><office:change-info><dc:creator>{author}</dc:creator><dc:date>{date}</dc:date></office:change-info></text:insertion>"
                    ));
                }
                ChangeType::Delete => {
                    let snippet = tc.text_snippet.as_deref().unwrap_or("");
                    xml.push_str(&format!(
                        "<text:deletion><office:change-info><dc:creator>{author}</dc:creator><dc:date>{date}</dc:date></office:change-info><text:p>{}</text:p></text:deletion>",
                        escape_text(snippet)
                    ));
                }
                ChangeType::FormatChange => {
                    xml.push_str(&format!(
                        "<text:format-change><office:change-info><dc:creator>{author}</dc:creator><dc:date>{date}</dc:date></office:change-info></text:format-change>"
                    ));
                }
            }
            xml.push_str("</text:changed-region>");
        }
        xml.push_str("\n      </text:tracked-changes>");
    }

    fn write_block(&mut self, xml: &mut String, block: &Block) {
        match block {
            Block::Heading(h) => self.write_heading(xml, h),
            Block::Paragraph(p) => self.write_paragraph(xml, p),
            Block::List(l) => self.write_list(xml, l, None),
            Block::Table(t) => self.write_table(xml, t),
            Block::Image(img) => self.write_image(xml, img),
            Block::Callout(c) => {
                for b in &c.blocks {
                    self.write_block(xml, b);
                }
            }
            Block::ThematicBreak(_) => {
                xml.push_str("\n      <text:p>---</text:p>");
            }
        }
    }

    fn register_auto_para_style(
        &mut self,
        parent: Option<&str>,
        alignment: Option<Alignment>,
        space_before_pt: Option<f32>,
        space_after_pt: Option<f32>,
        page_break_before: bool,
    ) -> Option<String> {
        if alignment.is_none()
            && space_before_pt.is_none()
            && space_after_pt.is_none()
            && !page_break_before
        {
            return parent.map(str::to_string);
        }
        let name = format!("P{}", self.p_seq);
        self.p_seq += 1;
        let parent_attr = parent
            .map(|p| format!(" style:parent-style-name=\"{}\"", escape_attribute(p)))
            .unwrap_or_default();
        let mut props = String::new();
        if let Some(a) = alignment {
            let v = match a {
                Alignment::Left => "start",
                Alignment::Center => "center",
                Alignment::Right => "end",
                Alignment::Justified => "justify",
            };
            props.push_str(&format!(" fo:text-align=\"{v}\""));
        }
        if let Some(sb) = space_before_pt {
            props.push_str(&format!(
                " fo:margin-top=\"{}\"",
                format_length_cm(sb.max(0.0))
            ));
        }
        if let Some(sa) = space_after_pt {
            props.push_str(&format!(
                " fo:margin-bottom=\"{}\"",
                format_length_cm(sa.max(0.0))
            ));
        }
        if page_break_before {
            props.push_str(" fo:break-before=\"page\"");
        }
        self.auto_styles_xml.push_str(&format!(
            "\n    <style:style style:name=\"{name}\" style:family=\"paragraph\"{parent_attr}><style:paragraph-properties{props}/></style:style>"
        ));
        Some(name)
    }

    fn write_heading(&mut self, xml: &mut String, h: &HeadingBlock) {
        let lvl = h.level.clamp(1, 6);
        let default_style = format!("Heading_{lvl}");
        let base_style = h.style_id.as_deref().unwrap_or(&default_style);
        let pbb = h
            .extra
            .get("office.page_break_before")
            .and_then(|v| v.as_bool())
            == Some(true);
        let style_name = self
            .register_auto_para_style(Some(base_style), None, None, None, pbb)
            .unwrap_or(default_style);
        xml.push_str(&format!(
            "\n      <text:h text:style-name=\"{}\" text:outline-level=\"{lvl}\">",
            escape_attribute(&style_name)
        ));
        self.write_spans(xml, &h.id, &h.spans);
        xml.push_str("</text:h>");
    }

    fn write_paragraph(&mut self, xml: &mut String, p: &ParagraphBlock) {
        let pbb = p
            .extra
            .get("office.page_break_before")
            .and_then(|v| v.as_bool())
            == Some(true);
        let style_name = self.register_auto_para_style(
            p.style_id.as_deref(),
            p.alignment,
            p.space_before_pt,
            p.space_after_pt,
            pbb,
        );
        if let Some(sn) = style_name {
            xml.push_str(&format!(
                "\n      <text:p text:style-name=\"{}\">",
                escape_attribute(&sn)
            ));
        } else {
            xml.push_str("\n      <text:p>");
        }
        self.write_spans(xml, &p.id, &p.spans);
        xml.push_str("</text:p>");
    }

    fn write_list(&mut self, xml: &mut String, list: &ListBlock, list_style: Option<&str>) {
        let owned_style;
        let (style_attr, active_style) = if let Some(s) = list_style {
            (String::new(), Some(s))
        } else {
            let lname = format!("L{}", self.l_seq);
            self.l_seq += 1;
            let num_fmt = list
                .extra
                .get("office.number_format")
                .and_then(|v| v.as_str())
                .unwrap_or("1");
            self.auto_styles_xml
                .push_str(&format!("\n    <text:list-style style:name=\"{lname}\">"));
            for lvl in 1..=9u8 {
                match list.kind {
                    ListKind::Ordered => {
                        let fmt = if num_fmt == "bullet" || num_fmt == "decimal" {
                            "1"
                        } else {
                            num_fmt
                        };
                        self.auto_styles_xml.push_str(&format!(
                            "<text:list-level-style-number text:level=\"{lvl}\" style:num-suffix=\".\" style:num-format=\"{}\"/>",
                            escape_attribute(fmt)
                        ));
                    }
                    ListKind::Unordered | ListKind::Task => {
                        self.auto_styles_xml.push_str(&format!(
                            "<text:list-level-style-bullet text:level=\"{lvl}\" text:bullet-char=\"•\"/>"
                        ));
                    }
                }
            }
            self.auto_styles_xml.push_str("</text:list-style>");
            owned_style = lname;
            (
                format!(" text:style-name=\"{owned_style}\""),
                Some(owned_style.as_str()),
            )
        };
        xml.push_str(&format!("\n      <text:list{style_attr}>"));
        for item in &list.items {
            xml.push_str("<text:list-item>");
            let has_text = item
                .spans
                .iter()
                .any(|s| !s.text.is_empty() || s.footnote_ref.is_some());
            if has_text || item.nested_list.is_none() {
                xml.push_str("<text:p>");
                self.write_spans(xml, &item.id, &item.spans);
                xml.push_str("</text:p>");
            }
            if let Some(nested) = &item.nested_list {
                self.write_list(xml, nested, active_style);
            }
            xml.push_str("</text:list-item>");
        }
        xml.push_str("</text:list>");
    }

    fn write_table(&mut self, xml: &mut String, tbl: &TableBlock) {
        let tname = escape_attribute(&tbl.id);
        xml.push_str(&format!("\n      <table:table table:name=\"{tname}\">"));
        if !tbl.column_widths_pt.is_empty() {
            for w in &tbl.column_widths_pt {
                let cname = format!("Col{}", self.col_seq);
                self.col_seq += 1;
                self.auto_styles_xml.push_str(&format!(
                    "\n    <style:style style:name=\"{cname}\" style:family=\"table-column\"><style:table-column-properties style:column-width=\"{}\"/></style:style>",
                    format_length_cm(*w)
                ));
                xml.push_str(&format!(
                    "<table:table-column table:style-name=\"{cname}\"/>"
                ));
            }
        } else if let Some(first_row) = tbl.rows.first() {
            let total_cols: u32 = first_row.cells.iter().map(|c| c.col_span.max(1)).sum();
            xml.push_str(&format!(
                "<table:table-column table:number-columns-repeated=\"{}\"/>",
                total_cols.max(1)
            ));
        }

        let mut in_header = false;
        let mut active_rowspans: BTreeMap<usize, (u32, u32)> = BTreeMap::new();

        for (r_idx, row) in tbl.rows.iter().enumerate() {
            let is_hdr = row.is_header || (r_idx as u32) < tbl.repeat_header_rows;
            if is_hdr && !in_header {
                xml.push_str("<table:table-header-rows>");
                in_header = true;
            } else if !is_hdr && in_header {
                xml.push_str("</table:table-header-rows>");
                in_header = false;
            }
            xml.push_str("<table:table-row>");
            let mut col_idx = 0usize;
            for cell in &row.cells {
                while let Some((rem, cspan)) = active_rowspans.get(&col_idx).copied() {
                    for _ in 0..cspan {
                        xml.push_str("<table:covered-table-cell/>");
                    }
                    if rem <= 1 {
                        active_rowspans.remove(&col_idx);
                    } else {
                        active_rowspans.insert(col_idx, (rem - 1, cspan));
                    }
                    col_idx += cspan as usize;
                }
                let mut cell_attrs = String::from(" office:value-type=\"string\"");
                if let Some(bg) = cell.background_color {
                    let cs_name = format!("Cell{}", self.cell_seq);
                    self.cell_seq += 1;
                    self.auto_styles_xml.push_str(&format!(
                        "\n    <style:style style:name=\"{cs_name}\" style:family=\"table-cell\"><style:table-cell-properties fo:background-color=\"#{}\"/></style:style>",
                        color_hex_rgb(bg)
                    ));
                    cell_attrs.push_str(&format!(" table:style-name=\"{cs_name}\""));
                }
                if cell.col_span > 1 {
                    cell_attrs.push_str(&format!(
                        " table:number-columns-spanned=\"{}\"",
                        cell.col_span
                    ));
                }
                if cell.row_span > 1 {
                    cell_attrs
                        .push_str(&format!(" table:number-rows-spanned=\"{}\"", cell.row_span));
                    active_rowspans.insert(col_idx, (cell.row_span - 1, cell.col_span));
                }
                xml.push_str(&format!("<table:table-cell{cell_attrs}>"));
                for b in &cell.blocks {
                    self.write_block(xml, b);
                }
                xml.push_str("</table:table-cell>");
                for _ in 1..cell.col_span {
                    xml.push_str("<table:covered-table-cell/>");
                }
                col_idx += cell.col_span as usize;
            }
            let trailing: Vec<usize> = active_rowspans.range(col_idx..).map(|(k, _)| *k).collect();
            for c in trailing {
                if let Some((rem, cspan)) = active_rowspans.get(&c).copied() {
                    for _ in 0..cspan {
                        xml.push_str("<table:covered-table-cell/>");
                    }
                    if rem <= 1 {
                        active_rowspans.remove(&c);
                    } else {
                        active_rowspans.insert(c, (rem - 1, cspan));
                    }
                }
            }
            xml.push_str("</table:table-row>");
        }
        if in_header {
            xml.push_str("</table:table-header-rows>");
        }
        xml.push_str("</table:table>");
    }

    fn write_image(&mut self, xml: &mut String, img: &ImageBlock) {
        let Some(asset) = self.assets.get(&img.asset_ref) else {
            self.report.note(
                Severity::Loss,
                "odt.export.missing-image-asset",
                &format!(
                    "Referenced image asset '{}' not provided in AssetMap",
                    img.asset_ref
                ),
                Some(&img.id),
            );
            xml.push_str(&format!(
                "\n      <text:p>[Image: {}]</text:p>",
                escape_text(&img.alt_text)
            ));
            return;
        };
        let info = sniff_image(&asset.bytes);
        let (ext, mime) = match &info {
            Some(i) => (i.extension.to_string(), i.media_type.to_string()),
            None => ("png".to_string(), asset.media_type.clone()),
        };
        let seq = self.img_seq;
        self.img_seq += 1;
        let pic_path = format!("Pictures/image{seq}.{ext}");
        self.pictures
            .insert(pic_path.clone(), (mime, asset.bytes.clone()));

        let w = format_length_cm(
            img.display_width_pt
                .unwrap_or(img.intrinsic_width_pt)
                .max(1.0),
        );
        let h = format_length_cm(
            img.display_height_pt
                .unwrap_or(img.intrinsic_height_pt)
                .max(1.0),
        );
        xml.push_str(&format!(
            "\n      <text:p><draw:frame draw:style-name=\"fr1\" draw:name=\"Image{seq}\" text:anchor-type=\"as-char\" svg:width=\"{w}\" svg:height=\"{h}\"><draw:image xlink:href=\"{pic_path}\" xlink:type=\"simple\" xlink:show=\"embed\" xlink:actuate=\"onLoad\"/><svg:title>{}</svg:title></draw:frame></text:p>",
            escape_text(&img.alt_text)
        ));
    }

    fn write_spans(&mut self, xml: &mut String, block_id: &str, spans: &[TextSpan]) {
        let anchors = self
            .comments_by_block
            .get(block_id)
            .cloned()
            .unwrap_or_default();
        let mut started = Vec::new();
        let mut finished = Vec::new();
        let mut cursor = 0usize;

        for (c_idx, start, _) in &anchors {
            if *start == 0 {
                self.write_annotation_start(xml, *c_idx);
                started.push(*c_idx);
            }
        }
        for span in spans {
            let span_len = span.text.chars().count();
            for (c_idx, start, _) in &anchors {
                if !started.contains(c_idx) && *start <= cursor {
                    self.write_annotation_start(xml, *c_idx);
                    started.push(*c_idx);
                }
            }
            self.write_single_span(xml, span);
            cursor += span_len;
            for (c_idx, _, end) in &anchors {
                if started.contains(c_idx) && !finished.contains(c_idx) && *end <= cursor {
                    xml.push_str(&format!(
                        "<office:annotation-end office:name=\"Ann{c_idx}\"/>"
                    ));
                    finished.push(*c_idx);
                }
            }
        }
        for (c_idx, _, _) in &anchors {
            if !started.contains(c_idx) {
                self.write_annotation_start(xml, *c_idx);
            }
            if !finished.contains(c_idx) {
                xml.push_str(&format!(
                    "<office:annotation-end office:name=\"Ann{c_idx}\"/>"
                ));
            }
        }
    }

    fn write_annotation_start(&self, xml: &mut String, c_idx: usize) {
        let Some(c) = self.doc.review.comments.get(c_idx) else {
            return;
        };
        let resolved_attr = if c.resolved {
            " loext:resolved=\"true\""
        } else {
            ""
        };
        xml.push_str(&format!(
            "<office:annotation office:name=\"Ann{c_idx}\"{resolved_attr}><dc:creator>{}</dc:creator><dc:date>{}</dc:date><text:p>{}</text:p></office:annotation>",
            escape_text(&c.author),
            escape_text(&c.timestamp),
            escape_text(&c.content)
        ));
        for reply in &c.replies {
            xml.push_str(&format!(
                "<office:annotation loext:parent-name=\"Ann{c_idx}\"><dc:creator>{}</dc:creator><dc:date>{}</dc:date><text:p>{}</text:p></office:annotation>",
                escape_text(&reply.author),
                escape_text(&reply.timestamp),
                escape_text(&reply.content)
            ));
        }
    }

    fn write_single_span(&mut self, xml: &mut String, span: &TextSpan) {
        if let Some(fn_id) = &span.footnote_ref {
            let num = self.fn_seq;
            self.fn_seq += 1;
            xml.push_str(&format!(
                "<text:note text:id=\"ftn{num}\" text:note-class=\"footnote\"><text:note-citation>{num}</text:note-citation><text:note-body>"
            ));
            if let Some(def) = self.doc.footnotes.get(fn_id) {
                for b in &def.blocks {
                    self.write_block(xml, b);
                }
            } else {
                xml.push_str("<text:p/>");
            }
            xml.push_str("</text:note-body></text:note>");
            if span.text.is_empty() {
                return;
            }
        }

        let tc = span
            .tracked_change_id
            .as_ref()
            .and_then(|id| self.tracked_by_id.get(id).copied());

        if let Some(change) = tc
            && change.change_type == ChangeType::Delete
        {
            xml.push_str(&format!(
                "<text:change text:change-id=\"{}\"/>",
                escape_attribute(&change.change_id)
            ));
            return;
        }

        if span.text == "{page}" {
            xml.push_str("<text:page-number text:select-page=\"current\">1</text:page-number>");
            return;
        }
        if span.text == "{total_pages}" {
            xml.push_str("<text:page-count>1</text:page-count>");
            return;
        }

        if let Some(change) = tc {
            xml.push_str(&format!(
                "<text:change-start text:change-id=\"{}\"/>",
                escape_attribute(&change.change_id)
            ));
        }
        let mut link_close = "";
        if let Some(url) = &span.link_url
            && let Some(safe) = safe_hyperlink(url)
        {
            xml.push_str(&format!(
                "<text:a xlink:type=\"simple\" xlink:href=\"{}\">",
                escape_attribute(&safe)
            ));
            link_close = "</text:a>";
        }

        let attrs = format_text_props_attrs(
            span.font_family.as_deref(),
            span.font_size_pt,
            &span.style,
            span.color,
            span.background_color,
        );
        if !attrs.is_empty() {
            let tname = format!("T{}", self.t_seq);
            self.t_seq += 1;
            self.auto_styles_xml.push_str(&format!(
                "\n    <style:style style:name=\"{tname}\" style:family=\"text\"><style:text-properties{attrs}/></style:style>"
            ));
            xml.push_str(&format!("<text:span text:style-name=\"{tname}\">"));
            write_odt_text_chars(xml, &span.text);
            xml.push_str("</text:span>");
        } else {
            write_odt_text_chars(xml, &span.text);
        }

        xml.push_str(link_close);
        if let Some(change) = tc {
            xml.push_str(&format!(
                "<text:change-end text:change-id=\"{}\"/>",
                escape_attribute(&change.change_id)
            ));
        }
    }
}

fn write_odt_text_chars(xml: &mut String, text: &str) {
    let mut space_run = 0usize;
    let flush_spaces = |xml: &mut String, run: &mut usize| {
        if *run == 1 {
            xml.push(' ');
        } else if *run > 1 {
            xml.push(' ');
            xml.push_str(&format!("<text:s text:c=\"{}\"/>", *run - 1));
        }
        *run = 0;
    };
    let mut buf = String::new();
    let flush_buf = |xml: &mut String, buf: &mut String| {
        if !buf.is_empty() {
            xml.push_str(&escape_text(buf));
            buf.clear();
        }
    };
    for ch in text.chars() {
        match ch {
            ' ' => {
                flush_buf(xml, &mut buf);
                space_run += 1;
            }
            '\t' => {
                flush_buf(xml, &mut buf);
                flush_spaces(xml, &mut space_run);
                xml.push_str("<text:tab/>");
            }
            '\n' => {
                flush_buf(xml, &mut buf);
                flush_spaces(xml, &mut space_run);
                xml.push_str("<text:line-break/>");
            }
            other => {
                flush_spaces(xml, &mut space_run);
                buf.push(other);
            }
        }
    }
    flush_buf(xml, &mut buf);
    flush_spaces(xml, &mut space_run);
}
