//! ODT package supporting parts (`META-INF/manifest.xml`, `meta.xml`, `styles.xml`).

use vocument_flow::metadata::DocumentMetadata;
use vocument_flow::model::{FlowDocument, PageOrientation, SectionGeometry};
use vocument_flow::styles::{Alignment, FontStyle, ParagraphStyle};

use crate::odt::styles::{
    NS_DC, NS_DRAW, NS_FO, NS_LOEXT, NS_MANIFEST, NS_META, NS_OFFICE, NS_STYLE, NS_SVG, NS_TABLE,
    NS_TEXT, NS_XLINK,
};
use crate::util::{color_hex_rgb, format_length_cm, format_pt};
use crate::xml::{escape_attribute, escape_text};

pub(super) const ODT_MIMETYPE: &str = "application/vnd.oasis.opendocument.text";

pub(super) fn build_manifest_xml(pictures: &[(String, String)]) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="{NS_MANIFEST}" manifest:version="1.2">
  <manifest:file-entry manifest:full-path="/" manifest:version="1.2" manifest:media-type="{ODT_MIMETYPE}"/>
  <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
  <manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>
  <manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/>"#
    );
    for (path, mime) in pictures {
        xml.push_str(&format!(
            "\n  <manifest:file-entry manifest:full-path=\"{}\" manifest:media-type=\"{}\"/>",
            escape_attribute(path),
            escape_attribute(mime)
        ));
    }
    xml.push_str("\n</manifest:manifest>\n");
    xml
}

pub(super) fn build_meta_xml(meta: &DocumentMetadata) -> String {
    let creators: Vec<&str> = meta.authors.iter().map(|a| a.name.as_str()).collect();
    let creator_str = creators.join("; ");
    let subject = meta.subtitle.as_deref().unwrap_or("");
    let description = meta.description.as_deref().unwrap_or("");

    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-meta xmlns:office="{NS_OFFICE}" xmlns:dc="{NS_DC}" xmlns:meta="{NS_META}" office:version="1.2">
  <office:meta>
    <dc:title>{}</dc:title>
    <dc:subject>{}</dc:subject>
    <dc:description>{}</dc:description>
    <dc:language>{}</dc:language>
    <dc:creator>{}</dc:creator>
    <meta:initial-creator>{}</meta:initial-creator>
    <meta:creation-date>{}</meta:creation-date>
    <dc:date>{}</dc:date>"#,
        escape_text(&meta.title),
        escape_text(subject),
        escape_text(description),
        escape_text(&meta.language),
        escape_text(&creator_str),
        escape_text(&creator_str),
        escape_text(&meta.created_at_utc),
        escape_text(&meta.modified_at_utc),
    );
    for kw in &meta.keywords {
        xml.push_str(&format!(
            "\n    <meta:keyword>{}</meta:keyword>",
            escape_text(kw)
        ));
    }
    for (k, v) in &meta.custom_attributes {
        xml.push_str(&format!(
            "\n    <meta:user-defined meta:name=\"{}\">{}</meta:user-defined>",
            escape_attribute(k),
            escape_text(v)
        ));
    }
    for (k, v) in &meta.extra {
        let text = match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        xml.push_str(&format!(
            "\n    <meta:user-defined meta:name=\"{}\">{}</meta:user-defined>",
            escape_attribute(k),
            escape_text(&text)
        ));
    }
    xml.push_str("\n  </office:meta>\n</office:document-meta>\n");
    xml
}

pub(super) fn build_styles_xml(doc: &FlowDocument, header_xml: &str, footer_xml: &str) -> String {
    let geom = doc
        .sections
        .first()
        .map(|s| s.geometry.clone())
        .unwrap_or_else(SectionGeometry::a4_portrait);

    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles xmlns:office="{NS_OFFICE}" xmlns:style="{NS_STYLE}" xmlns:text="{NS_TEXT}" xmlns:table="{NS_TABLE}" xmlns:draw="{NS_DRAW}" xmlns:fo="{NS_FO}" xmlns:xlink="{NS_XLINK}" xmlns:dc="{NS_DC}" xmlns:svg="{NS_SVG}" xmlns:loext="{NS_LOEXT}" office:version="1.2">
  <office:styles>
    <style:default-style style:family="paragraph">
      <style:paragraph-properties fo:hyphenation-ladder-count="no-limit" style:line-break="strict"/>
      <style:text-properties style:font-name="Liberation Sans" fo:font-family="'Liberation Sans'" fo:font-size="11pt"/>
    </style:default-style>"#
    );

    for style in doc.stylesheet.paragraph_styles.values() {
        write_odt_para_style(&mut xml, style);
    }
    for level in 1..=6u8 {
        let hid = format!("Heading_{level}");
        if !doc.stylesheet.paragraph_styles.contains_key(&hid) {
            let pt = match level {
                1 => 18,
                2 => 15,
                3 => 13,
                4 => 12,
                5 => 11,
                _ => 10,
            };
            xml.push_str(&format!(
                "\n    <style:style style:name=\"{hid}\" style:display-name=\"Heading {level}\" style:family=\"paragraph\" style:default-outline-level=\"{level}\"><style:paragraph-properties fo:margin-top=\"0.423cm\" fo:margin-bottom=\"0.212cm\" fo:keep-with-next=\"always\"/><style:text-properties fo:font-size=\"{pt}pt\" fo:font-weight=\"bold\"/></style:style>"
            ));
        }
    }
    for cstyle in doc.stylesheet.character_styles.values() {
        xml.push_str(&format!(
            "\n    <style:style style:name=\"{}\" style:display-name=\"{}\" style:family=\"text\"><style:text-properties{}/></style:style>",
            escape_attribute(&cstyle.id),
            escape_attribute(&cstyle.name),
            format_text_props_attrs(
                cstyle.font_family.as_deref(),
                cstyle.font_size_pt,
                &cstyle.font_style,
                cstyle.color,
                None
            )
        ));
    }

    xml.push_str("\n  </office:styles>\n  <office:automatic-styles>");
    write_page_layout(&mut xml, "pm1", &geom);
    xml.push_str("\n  </office:automatic-styles>\n  <office:master-styles>\n    <style:master-page style:name=\"Standard\" style:page-layout-name=\"pm1\">");
    if !header_xml.is_empty() {
        xml.push_str("\n      <style:header>");
        xml.push_str(header_xml);
        xml.push_str("\n      </style:header>");
    }
    if !footer_xml.is_empty() {
        xml.push_str("\n      <style:footer>");
        xml.push_str(footer_xml);
        xml.push_str("\n      </style:footer>");
    }
    xml.push_str(
        "\n    </style:master-page>\n  </office:master-styles>\n</office:document-styles>\n",
    );
    xml
}

fn write_odt_para_style(xml: &mut String, style: &ParagraphStyle) {
    let align = match style.alignment {
        Alignment::Left => "start",
        Alignment::Center => "center",
        Alignment::Right => "end",
        Alignment::Justified => "justify",
    };
    let lh = ((style.line_spacing_mult * 100.0).round() as i32).clamp(50, 1000);
    let outline_attr = style
        .id
        .strip_prefix("Heading")
        .or_else(|| style.id.strip_prefix("heading_"))
        .and_then(|s| s.parse::<u8>().ok())
        .filter(|l| (1..=6).contains(l))
        .map(|l| format!(" style:default-outline-level=\"{l}\""))
        .unwrap_or_default();

    xml.push_str(&format!(
        "\n    <style:style style:name=\"{}\" style:display-name=\"{}\" style:family=\"paragraph\"{outline_attr}>",
        escape_attribute(&style.id),
        escape_attribute(&style.name)
    ));
    xml.push_str(&format!(
        "\n      <style:paragraph-properties fo:text-align=\"{align}\" fo:margin-top=\"{}\" fo:margin-bottom=\"{}\" fo:line-height=\"{lh}%\" fo:text-indent=\"{}\"{}/>",
        format_length_cm(style.space_before_pt.max(0.0)),
        format_length_cm(style.space_after_pt.max(0.0)),
        format_length_cm(style.first_line_indent_pt),
        if style.keep_with_next { " fo:keep-with-next=\"always\"" } else { "" }
    ));
    xml.push_str(&format!(
        "\n      <style:text-properties style:font-name=\"{fam}\" fo:font-family=\"{fam}\" fo:font-size=\"{}pt\"/>\n    </style:style>",
        format_pt(style.font_size_pt),
        fam = escape_attribute(&style.font_family)
    ));
}

fn write_page_layout(xml: &mut String, name: &str, g: &SectionGeometry) {
    let orient = match g.orientation {
        PageOrientation::Landscape => "landscape",
        PageOrientation::Portrait => "portrait",
    };
    xml.push_str(&format!(
        "\n    <style:page-layout style:name=\"{name}\"><style:page-layout-properties fo:page-width=\"{}\" fo:page-height=\"{}\" style:print-orientation=\"{orient}\" fo:margin-top=\"{}\" fo:margin-bottom=\"{}\" fo:margin-left=\"{}\" fo:margin-right=\"{}\">",
        format_length_cm(g.width_pt),
        format_length_cm(g.height_pt),
        format_length_cm(g.margin_top_pt),
        format_length_cm(g.margin_bottom_pt),
        format_length_cm(g.margin_left_pt),
        format_length_cm(g.margin_right_pt),
    ));
    if g.column_count > 1 {
        xml.push_str(&format!(
            "<style:columns fo:column-count=\"{}\" fo:column-gap=\"{}\"/>",
            g.column_count,
            format_length_cm(g.column_gap_pt)
        ));
    }
    xml.push_str("</style:page-layout-properties></style:page-layout>");
}

pub(super) fn format_text_props_attrs(
    family: Option<&str>,
    size_pt: Option<f32>,
    fs: &FontStyle,
    color: Option<vocument_flow::styles::ColorRgba>,
    bg: Option<vocument_flow::styles::ColorRgba>,
) -> String {
    let mut s = String::new();
    if let Some(fam) = family {
        let e = escape_attribute(fam);
        s.push_str(&format!(" style:font-name=\"{e}\" fo:font-family=\"{e}\""));
    }
    if let Some(sz) = size_pt {
        s.push_str(&format!(" fo:font-size=\"{}pt\"", format_pt(sz)));
    }
    if fs.bold {
        s.push_str(" fo:font-weight=\"bold\"");
    }
    if fs.italic {
        s.push_str(" fo:font-style=\"italic\"");
    }
    if fs.underline {
        s.push_str(" style:text-underline-style=\"solid\" style:text-underline-width=\"auto\"");
    }
    if fs.strikethrough {
        s.push_str(" style:text-line-through-style=\"solid\"");
    }
    if fs.superscript {
        s.push_str(" style:text-position=\"super 58%\"");
    } else if fs.subscript {
        s.push_str(" style:text-position=\"sub 58%\"");
    }
    if let Some(c) = color {
        s.push_str(&format!(" fo:color=\"#{}\"", color_hex_rgb(c)));
    }
    if let Some(b) = bg {
        s.push_str(&format!(" fo:background-color=\"#{}\"", color_hex_rgb(b)));
    }
    s
}
