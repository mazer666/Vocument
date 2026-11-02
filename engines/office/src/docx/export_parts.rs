//! Supporting XML part generators for DOCX export (`[Content_Types].xml`, `.rels`, `styles.xml`,
//! `numbering.xml`, `footnotes.xml`, `comments.xml`, `commentsExtended.xml`, `docProps/*`).

use vocument_flow::metadata::DocumentMetadata;
use vocument_flow::model::{FlowDocument, ListKind};
use vocument_flow::styles::{Alignment, FontStyle, ParagraphStyle};

use crate::docx::export::ExportRel;
use crate::util::pt_to_twips;
use crate::xml::{escape_attribute, escape_text};

pub(super) const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
pub(super) const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub(super) const NS_WP: &str =
    "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
pub(super) const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub(super) const NS_PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
pub(super) const NS_W15: &str = "http://schemas.microsoft.com/office/word/2012/wordml";

pub(super) fn build_root_rels(has_custom_props: bool) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>"#,
    );
    if has_custom_props {
        xml.push_str(
            r#"
  <Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties" Target="docProps/custom.xml"/>"#,
        );
    }
    xml.push_str("\n</Relationships>\n");
    xml
}

pub(super) fn build_content_types(
    has_numbering: bool,
    has_footnotes: bool,
    has_comments: bool,
    has_custom_props: bool,
    header_parts: &[String],
    footer_parts: &[String],
    media_extensions: &[(&str, &str)],
) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>"#,
    );
    for (ext, mime) in media_extensions {
        xml.push_str(&format!(
            "\n  <Default Extension=\"{}\" ContentType=\"{}\"/>",
            escape_attribute(ext),
            escape_attribute(mime)
        ));
    }
    xml.push_str(
        r#"
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>"#,
    );
    if has_custom_props {
        xml.push_str(
            "\n  <Override PartName=\"/docProps/custom.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.custom-properties+xml\"/>",
        );
    }
    if has_numbering {
        xml.push_str(
            "\n  <Override PartName=\"/word/numbering.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml\"/>",
        );
    }
    if has_footnotes {
        xml.push_str(
            "\n  <Override PartName=\"/word/footnotes.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml\"/>",
        );
    }
    if has_comments {
        xml.push_str(
            "\n  <Override PartName=\"/word/comments.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml\"/>",
        );
        xml.push_str(
            "\n  <Override PartName=\"/word/commentsExtended.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtended+xml\"/>",
        );
    }
    for hdr in header_parts {
        xml.push_str(&format!(
            "\n  <Override PartName=\"/word/{hdr}\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml\"/>"
        ));
    }
    for ftr in footer_parts {
        xml.push_str(&format!(
            "\n  <Override PartName=\"/word/{ftr}\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml\"/>"
        ));
    }
    xml.push_str("\n</Types>\n");
    xml
}

pub(super) fn build_document_rels(rels: &[ExportRel]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    for rel in rels {
        if rel.external {
            xml.push_str(&format!(
                "\n  <Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\" TargetMode=\"External\"/>",
                escape_attribute(&rel.id),
                escape_attribute(&rel.rel_type),
                escape_attribute(&rel.target)
            ));
        } else {
            xml.push_str(&format!(
                "\n  <Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"/>",
                escape_attribute(&rel.id),
                escape_attribute(&rel.rel_type),
                escape_attribute(&rel.target)
            ));
        }
    }
    xml.push_str("\n</Relationships>\n");
    xml
}

pub(super) fn build_core_props(meta: &DocumentMetadata) -> String {
    let keywords = meta.keywords.join(", ");
    let creators: Vec<&str> = meta.authors.iter().map(|a| a.name.as_str()).collect();
    let creator_str = creators.join("; ");
    let subject = meta.subtitle.as_deref().unwrap_or("");
    let description = meta.description.as_deref().unwrap_or("");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <dc:title>{}</dc:title>
  <dc:subject>{}</dc:subject>
  <dc:creator>{}</dc:creator>
  <cp:keywords>{}</cp:keywords>
  <dc:description>{}</dc:description>
  <dc:language>{}</dc:language>
  <dcterms:created xsi:type="dcterms:W3CDTF">{}</dcterms:created>
  <dcterms:modified xsi:type="dcterms:W3CDTF">{}</dcterms:modified>
</cp:coreProperties>
"#,
        escape_text(&meta.title),
        escape_text(subject),
        escape_text(&creator_str),
        escape_text(&keywords),
        escape_text(description),
        escape_text(&meta.language),
        escape_text(&meta.created_at_utc),
        escape_text(&meta.modified_at_utc),
    )
}

pub(super) fn build_app_props() -> String {
    String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>Vocument</Application>
</Properties>
"#,
    )
}

pub(super) fn build_custom_props(meta: &DocumentMetadata) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">"#,
    );
    let mut pid = 2usize;
    for (key, val) in &meta.custom_attributes {
        xml.push_str(&format!(
            "\n  <property fmtid=\"{{D5CDD505-2E9C-101B-9397-08002B2CF9AE}}\" pid=\"{pid}\" name=\"{}\"><vt:lpwstr>{}</vt:lpwstr></property>",
            escape_attribute(key),
            escape_text(val)
        ));
        pid += 1;
    }
    for (key, val) in &meta.extra {
        let text = match val {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        xml.push_str(&format!(
            "\n  <property fmtid=\"{{D5CDD505-2E9C-101B-9397-08002B2CF9AE}}\" pid=\"{pid}\" name=\"{}\"><vt:lpwstr>{}</vt:lpwstr></property>",
            escape_attribute(key),
            escape_text(&text)
        ));
        pid += 1;
    }
    xml.push_str("\n</Properties>\n");
    xml
}

pub(super) fn build_styles_xml(doc: &FlowDocument) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="{NS_W}">
  <w:docDefaults>
    <w:rPrDefault>
      <w:rPr>
        <w:rFonts w:ascii="Liberation Sans" w:hAnsi="Liberation Sans" w:cs="Liberation Sans"/>
        <w:sz w:val="22"/>
        <w:szCs w:val="22"/>
      </w:rPr>
    </w:rPrDefault>
  </w:docDefaults>"#
    );

    for style in doc.stylesheet.paragraph_styles.values() {
        write_paragraph_style(&mut xml, style);
    }

    for level in 1..=6u8 {
        let hid = format!("Heading{level}");
        if !doc.stylesheet.paragraph_styles.contains_key(&hid) {
            let half_pts = match level {
                1 => 36,
                2 => 30,
                3 => 26,
                4 => 24,
                5 => 22,
                _ => 20,
            };
            xml.push_str(&format!(
                r#"
  <w:style w:type="paragraph" w:styleId="{hid}">
    <w:name w:val="heading {level}"/>
    <w:basedOn w:val="body"/>
    <w:pPr>
      <w:keepNext/>
      <w:spacing w:before="240" w:after="120"/>
      <w:outlineLvl w:val="{}"/>
    </w:pPr>
    <w:rPr>
      <w:b/>
      <w:sz w:val="{half_pts}"/>
    </w:rPr>
  </w:style>"#,
                level - 1
            ));
        }
    }

    xml.push_str("\n</w:styles>\n");
    xml
}

fn write_paragraph_style(xml: &mut String, style: &ParagraphStyle) {
    let is_default = if style.id == "body" || style.id == "Normal" {
        " w:default=\"1\""
    } else {
        ""
    };
    xml.push_str(&format!(
        "\n  <w:style w:type=\"paragraph\"{} w:styleId=\"{}\">",
        is_default,
        escape_attribute(&style.id)
    ));
    xml.push_str(&format!(
        "\n    <w:name w:val=\"{}\"/>",
        escape_attribute(&style.name)
    ));
    let jc = match style.alignment {
        Alignment::Left => "left",
        Alignment::Center => "center",
        Alignment::Right => "right",
        Alignment::Justified => "both",
    };
    let before = pt_to_twips(style.space_before_pt).max(0);
    let after = pt_to_twips(style.space_after_pt).max(0);
    let line = ((style.line_spacing_mult * 240.0).round() as i64).clamp(120, 2400);
    let first_line = pt_to_twips(style.first_line_indent_pt).max(0);
    xml.push_str(&format!(
        "\n    <w:pPr><w:jc w:val=\"{jc}\"/><w:spacing w:before=\"{before}\" w:after=\"{after}\" w:line=\"{line}\" w:lineRule=\"auto\"/><w:ind w:firstLine=\"{first_line}\"/>"
    ));
    if style.keep_with_next {
        xml.push_str("<w:keepNext/>");
    }
    if let Some(lvl) = style
        .id
        .strip_prefix("Heading")
        .or_else(|| style.id.strip_prefix("heading_"))
        .and_then(|s| s.parse::<u8>().ok())
        .filter(|l| (1..=6).contains(l))
    {
        xml.push_str(&format!("<w:outlineLvl w:val=\"{}\"/>", lvl - 1));
    }
    xml.push_str("</w:pPr>");

    let half_pt = ((style.font_size_pt * 2.0).round() as i64).clamp(8, 400);
    xml.push_str(&format!(
        "\n    <w:rPr><w:rFonts w:ascii=\"{fam}\" w:hAnsi=\"{fam}\" w:cs=\"{fam}\"/><w:sz w:val=\"{half_pt}\"/><w:szCs w:val=\"{half_pt}\"/></w:rPr>\n  </w:style>",
        fam = escape_attribute(&style.font_family)
    ));
}

pub(super) fn write_font_style_tags(xml: &mut String, fs: &FontStyle) {
    if fs.bold {
        xml.push_str("<w:b/>");
    }
    if fs.italic {
        xml.push_str("<w:i/>");
    }
    if fs.underline {
        xml.push_str("<w:u w:val=\"single\"/>");
    }
    if fs.strikethrough {
        xml.push_str("<w:strike/>");
    }
    if fs.superscript {
        xml.push_str("<w:vertAlign w:val=\"superscript\"/>");
    } else if fs.subscript {
        xml.push_str("<w:vertAlign w:val=\"subscript\"/>");
    }
}

pub(super) fn build_numbering_xml(list_defs: &[(u32, ListKind, String)]) -> String {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:numbering xmlns:w=\"{NS_W}\">"
    );
    for (num_id, kind, num_fmt) in list_defs {
        let abs_id = *num_id;
        let fmt = match kind {
            ListKind::Unordered | ListKind::Task => "bullet",
            ListKind::Ordered => {
                if num_fmt.is_empty() || num_fmt == "bullet" {
                    "decimal"
                } else {
                    num_fmt.as_str()
                }
            }
        };
        xml.push_str(&format!("\n  <w:abstractNum w:abstractNumId=\"{abs_id}\">"));
        for lvl in 0..=8u8 {
            let indent = (i64::from(lvl) + 1) * 720;
            let lvl_text = if fmt == "bullet" {
                "•".to_string()
            } else {
                format!("%{}.", lvl + 1)
            };
            xml.push_str(&format!(
                "\n    <w:lvl w:ilvl=\"{lvl}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{}\"/><w:lvlText w:val=\"{}\"/><w:pPr><w:ind w:left=\"{indent}\" w:hanging=\"360\"/></w:pPr></w:lvl>",
                escape_attribute(fmt),
                escape_attribute(&lvl_text)
            ));
        }
        xml.push_str("\n  </w:abstractNum>");
    }
    for (num_id, _, _) in list_defs {
        xml.push_str(&format!(
            "\n  <w:num w:numId=\"{num_id}\"><w:abstractNumId w:val=\"{num_id}\"/></w:num>"
        ));
    }
    xml.push_str("\n</w:numbering>\n");
    xml
}

pub(super) fn build_comments_xml(doc: &FlowDocument) -> (String, String) {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:comments xmlns:w=\"{NS_W}\" xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\">"
    );
    let mut ext_xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w15:commentsEx xmlns:w15=\"{NS_W15}\">"
    );

    let mut next_id = 1u32;
    for comment in &doc.review.comments {
        let root_id = next_id;
        next_id += 1;
        let root_para_id = format!("{root_id:08X}");
        xml.push_str(&format!(
            "\n  <w:comment w:id=\"{root_id}\" w:author=\"{}\" w:date=\"{}\"><w:p w14:paraId=\"{root_para_id}\"><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p></w:comment>",
            escape_attribute(&comment.author),
            escape_attribute(&comment.timestamp),
            escape_text(&comment.content)
        ));
        ext_xml.push_str(&format!(
            "\n  <w15:commentEx w15:paraId=\"{root_para_id}\" w15:done=\"{}\"/>",
            if comment.resolved { "1" } else { "0" }
        ));
        for reply in &comment.replies {
            let reply_id = next_id;
            next_id += 1;
            let reply_para_id = format!("{reply_id:08X}");
            xml.push_str(&format!(
                "\n  <w:comment w:id=\"{reply_id}\" w:author=\"{}\" w:date=\"{}\"><w:p w14:paraId=\"{reply_para_id}\"><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p></w:comment>",
                escape_attribute(&reply.author),
                escape_attribute(&reply.timestamp),
                escape_text(&reply.content)
            ));
            ext_xml.push_str(&format!(
                "\n  <w15:commentEx w15:paraId=\"{reply_para_id}\" w15:paraIdParent=\"{root_para_id}\" w15:done=\"0\"/>"
            ));
        }
    }

    xml.push_str("\n</w:comments>\n");
    ext_xml.push_str("\n</w15:commentsEx>\n");
    (xml, ext_xml)
}
