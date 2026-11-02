//! ODT inline content parsing (`text:span`, `text:s`, `text:tab`, `text:line-break`, `text:a`,
//! `text:note`, `draw:frame`, `office:annotation`, `text:change-start`/`end`/`change`).

use std::collections::BTreeMap;

use vocument_flow::model::{FootnoteDefinition, ImageBlock};
use vocument_flow::review::{ChangeType, FlowCommentReply};
use vocument_flow::styles::TextSpan;

use crate::convert::ImportedAsset;
use crate::error::OfficeError;
use crate::odt::import::{OdtAnnotation, OdtImportContext};
use crate::odt::styles::{
    NS_DC, NS_DRAW, NS_LOEXT, NS_OFFICE, NS_SVG, NS_TEXT, NS_XLINK, OdtTextProps,
};
use crate::report::Severity;
use crate::util::{content_addressed_path, parse_length_pt, safe_hyperlink, sniff_image};
use crate::xml::{Element, Node};

impl OdtImportContext<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn collect_inline_nodes(
        &mut self,
        container: &Element,
        base_text: &OdtTextProps,
        direct_text: &OdtTextProps,
        link_url: Option<&str>,
        active_change: &mut Option<String>,
        active_annotations: &mut Vec<String>,
        block_id: &str,
        spans: &mut Vec<TextSpan>,
        inline_images: &mut Vec<ImageBlock>,
    ) -> Result<(), OfficeError> {
        let mut eff = base_text.clone();
        eff.overlay(direct_text);
        if eff.hidden {
            self.report.note(
                Severity::PreservedUnrendered,
                "odt.text.hidden",
                "Hidden text span (text:display=\"none\") omitted from visible text",
                Some(block_id),
            );
            return Ok(());
        }

        for node in &container.children {
            match node {
                Node::Text(t) => {
                    if !t.is_empty() {
                        self.push_odt_span(
                            t.clone(),
                            &eff,
                            direct_text,
                            link_url,
                            active_change.as_deref(),
                            active_annotations,
                            spans,
                        );
                    }
                }
                Node::Element(el) => {
                    self.collect_inline_element(
                        el,
                        &eff,
                        direct_text,
                        link_url,
                        active_change,
                        active_annotations,
                        block_id,
                        spans,
                        inline_images,
                    )?;
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_inline_element(
        &mut self,
        el: &Element,
        eff: &OdtTextProps,
        direct_text: &OdtTextProps,
        link_url: Option<&str>,
        active_change: &mut Option<String>,
        active_annotations: &mut Vec<String>,
        block_id: &str,
        spans: &mut Vec<TextSpan>,
        inline_images: &mut Vec<ImageBlock>,
    ) -> Result<(), OfficeError> {
        match (el.namespace.as_str(), el.local.as_str()) {
            (NS_TEXT, "span") => {
                let mut child_direct = direct_text.clone();
                if let Some(sname) = el.attribute(NS_TEXT, "style-name")
                    && let Some(tp) = self.styles.text_styles.get(sname)
                {
                    child_direct.overlay(tp);
                }
                self.collect_inline_nodes(
                    el,
                    eff,
                    &child_direct,
                    link_url,
                    active_change,
                    active_annotations,
                    block_id,
                    spans,
                    inline_images,
                )?;
            }
            (NS_TEXT, "s") => {
                let count = el
                    .attribute(NS_TEXT, "c")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, 256);
                self.push_odt_span(
                    " ".repeat(count),
                    eff,
                    direct_text,
                    link_url,
                    active_change.as_deref(),
                    active_annotations,
                    spans,
                );
            }
            (NS_TEXT, "tab") => {
                self.push_odt_span(
                    "\t".to_string(),
                    eff,
                    direct_text,
                    link_url,
                    active_change.as_deref(),
                    active_annotations,
                    spans,
                );
            }
            (NS_TEXT, "line-break") => {
                self.push_odt_span(
                    "\n".to_string(),
                    eff,
                    direct_text,
                    link_url,
                    active_change.as_deref(),
                    active_annotations,
                    spans,
                );
            }
            (NS_TEXT, "soft-page-break") => {}
            (NS_TEXT, "page-number") => {
                spans.push(TextSpan::plain("{page}"));
            }
            (NS_TEXT, "page-count") => {
                spans.push(TextSpan::plain("{total_pages}"));
            }
            (NS_TEXT, "a") => {
                let raw_href = el.attribute(NS_XLINK, "href").unwrap_or("");
                let safe = if let Some(s) = safe_hyperlink(raw_href) {
                    Some(s)
                } else {
                    self.report.note(
                        Severity::Security,
                        "odt.hyperlink.unsafe-scheme",
                        "Unsafe or local hyperlink scheme stripped during ODT import",
                        Some(block_id),
                    );
                    None
                };
                self.collect_inline_nodes(
                    el,
                    eff,
                    direct_text,
                    safe.as_deref(),
                    active_change,
                    active_annotations,
                    block_id,
                    spans,
                    inline_images,
                )?;
            }
            (NS_TEXT, "note") => {
                let is_endnote = el.attribute(NS_TEXT, "note-class") == Some("endnote");
                if is_endnote {
                    self.report.note(
                        Severity::Approximated,
                        "odt.endnote.to-footnote",
                        "ODF endnote imported as document footnote",
                        Some(block_id),
                    );
                }
                if let Some(body) = el.child(NS_TEXT, "note-body") {
                    let fn_id = self.ids.next("fn");
                    let number = (self.footnotes.len() as u32) + 1;
                    let mut blocks = Vec::new();
                    for child in body.child_elements() {
                        self.convert_block(child, &mut blocks)?;
                    }
                    self.footnotes.insert(
                        fn_id.clone(),
                        FootnoteDefinition::new(fn_id.clone(), number, blocks),
                    );
                    spans.push(TextSpan::plain("").with_footnote(fn_id));
                }
            }
            (NS_DRAW, "frame") => {
                if let Some(img) = self.extract_draw_frame(el, block_id) {
                    inline_images.push(img);
                }
            }
            (NS_OFFICE, "annotation") => {
                self.parse_annotation(el, block_id, spans, active_annotations);
            }
            (NS_OFFICE, "annotation-end") => {
                if let Some(name) = el.attribute(NS_OFFICE, "name") {
                    let offset = char_len_of_spans(spans);
                    if let Some(ann) = self.annotations.iter_mut().find(|a| a.name == name) {
                        ann.char_end = Some(offset);
                    }
                    active_annotations.retain(|n| n != name);
                }
            }
            (NS_TEXT, "change-start") => {
                if let Some(cid) = el.attribute(NS_TEXT, "change-id")
                    && let Some(tc_id) = self.change_id_map.get(cid).cloned()
                {
                    if let Some(tc) = self
                        .tracked_changes
                        .iter_mut()
                        .find(|t| t.change_id == tc_id)
                        && tc.target_block_id.is_none()
                    {
                        tc.target_block_id = Some(block_id.to_string());
                    }
                    if let Some(snippet) = self.deleted_texts.get(cid).cloned()
                        && !snippet.is_empty()
                    {
                        self.push_odt_span(
                            snippet,
                            eff,
                            direct_text,
                            link_url,
                            Some(&tc_id),
                            active_annotations,
                            spans,
                        );
                    } else {
                        *active_change = Some(tc_id);
                    }
                }
            }
            (NS_TEXT, "change-end") => {
                *active_change = None;
            }
            (NS_TEXT, "change") => {
                if let Some(cid) = el.attribute(NS_TEXT, "change-id")
                    && let Some(tc_id) = self.change_id_map.get(cid).cloned()
                {
                    let snippet = self.deleted_texts.get(cid).cloned().unwrap_or_default();
                    if let Some(tc) = self
                        .tracked_changes
                        .iter_mut()
                        .find(|t| t.change_id == tc_id)
                    {
                        tc.target_block_id = Some(block_id.to_string());
                    }
                    if !snippet.is_empty() {
                        self.push_odt_span(
                            snippet,
                            eff,
                            direct_text,
                            link_url,
                            Some(&tc_id),
                            active_annotations,
                            spans,
                        );
                    }
                }
            }
            _ => {
                let fallback = el.text_content();
                if !fallback.is_empty() {
                    self.push_odt_span(
                        fallback,
                        eff,
                        direct_text,
                        link_url,
                        active_change.as_deref(),
                        active_annotations,
                        spans,
                    );
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn push_odt_span(
        &mut self,
        text: String,
        eff: &OdtTextProps,
        direct: &OdtTextProps,
        link_url: Option<&str>,
        tracked_id: Option<&str>,
        active_annotations: &[String],
        spans: &mut Vec<TextSpan>,
    ) {
        for name in active_annotations {
            if let Some(ann) = self.annotations.iter_mut().find(|a| &a.name == name) {
                ann.anchor_text.push_str(&text);
            }
        }
        let mut style = eff.to_font_style();
        let mut tracked_change_id = None;
        let mut original_style = None;

        if let Some(tc_id) = tracked_id {
            tracked_change_id = Some(tc_id.to_string());
            if let Some(tc) = self.tracked_changes.iter().find(|t| t.change_id == tc_id) {
                match tc.change_type {
                    ChangeType::Insert => style.underline = true,
                    ChangeType::Delete => style.strikethrough = true,
                    ChangeType::FormatChange => {
                        original_style = Some(vocument_flow::styles::FontStyle::PLAIN);
                    }
                }
            }
        }

        spans.push(TextSpan {
            text,
            style,
            font_size_pt: direct.size_pt,
            font_family: direct.family.clone(),
            font_weight: direct.bold.map(|b| if b { 700 } else { 400 }),
            color: eff.color,
            background_color: eff.background,
            link_url: link_url.map(str::to_string),
            footnote_ref: None,
            tracked_change_id,
            original_style,
            extra: BTreeMap::new(),
        });
    }

    pub(super) fn extract_draw_frame(
        &mut self,
        frame: &Element,
        block_id: &str,
    ) -> Option<ImageBlock> {
        let width_pt = frame
            .attribute(NS_SVG, "width")
            .and_then(parse_length_pt)
            .map(|v| v.max(1.0));
        let height_pt = frame
            .attribute(NS_SVG, "height")
            .and_then(parse_length_pt)
            .map(|v| v.max(1.0));
        let alt_text = frame
            .child(NS_SVG, "title")
            .or_else(|| frame.child(NS_SVG, "desc"))
            .map(|e| e.text_content().trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                frame
                    .attribute(NS_DRAW, "name")
                    .map(str::to_string)
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or_else(|| "Image".to_string());

        let Some(img_el) = frame.child(NS_DRAW, "image") else {
            if frame.child(NS_DRAW, "object").is_some()
                || frame.child(NS_DRAW, "object-ole").is_some()
            {
                self.report.note(
                    Severity::Loss,
                    "odt.draw.object",
                    "Embedded ODF object/chart inside draw:frame cannot be rendered",
                    Some(block_id),
                );
            }
            return None;
        };

        let href = img_el.attribute(NS_XLINK, "href")?;
        let href = href.trim().trim_start_matches("./");
        if href.contains("://")
            || href.starts_with('/')
            || href.starts_with("..")
            || href.starts_with("file:")
        {
            self.report.note(
                Severity::Security,
                "odt.image.external-href",
                "External or unsafe image xlink:href blocked for security",
                Some(block_id),
            );
            return None;
        }
        let Some(bytes) = self.package.part(href) else {
            self.report.note(
                Severity::Loss,
                "odt.image.missing-part",
                &format!("Image binary '{href}' missing from ODT archive"),
                Some(block_id),
            );
            return None;
        };
        let info = sniff_image(bytes);
        let (ext, media_type, px_size) = match &info {
            Some(i) => (i.extension, i.media_type.to_string(), i.pixel_size),
            None => ("bin", "application/octet-stream".to_string(), None),
        };
        let asset_ref = content_addressed_path(ext, bytes);
        self.assets.insert(
            asset_ref.clone(),
            ImportedAsset {
                media_type,
                bytes: bytes.to_vec(),
            },
        );
        let intrinsic_w = px_size
            .map(|(w, _)| (w as f32 * 0.75).max(1.0))
            .or(width_pt)
            .unwrap_or(180.0);
        let intrinsic_h = px_size
            .map(|(_, h)| (h as f32 * 0.75).max(1.0))
            .or(height_pt)
            .unwrap_or(120.0);

        Some(ImageBlock {
            id: self.ids.next("img"),
            asset_ref,
            alt_text,
            caption: None,
            intrinsic_width_pt: intrinsic_w,
            intrinsic_height_pt: intrinsic_h,
            display_width_pt: width_pt,
            display_height_pt: height_pt,
            extra: BTreeMap::new(),
        })
    }

    fn parse_annotation(
        &mut self,
        ann_el: &Element,
        block_id: &str,
        spans: &[TextSpan],
        active_annotations: &mut Vec<String>,
    ) {
        let name = ann_el
            .attribute(NS_OFFICE, "name")
            .map(str::to_string)
            .unwrap_or_else(|| self.ids.next("ann"));
        let author = ann_el
            .child(NS_DC, "creator")
            .map(|e| e.text_content().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Author".to_string());
        let date = ann_el
            .child(NS_DC, "date")
            .map(|e| e.text_content().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
        let resolved = matches!(
            ann_el
                .attribute(NS_LOEXT, "resolved")
                .or_else(|| ann_el.attribute(NS_OFFICE, "display")),
            Some("true" | "1")
        );
        let parent_name = ann_el
            .attribute(NS_LOEXT, "parent-name")
            .map(str::to_string);

        let mut paragraphs = Vec::new();
        for p in ann_el.child_elements().filter(|e| e.is(NS_TEXT, "p")) {
            let t = p.text_content();
            if !t.is_empty() {
                paragraphs.push(t);
            }
        }
        let content = paragraphs.join("\n");

        if let Some(parent) = parent_name
            && let Some(parent_ann) = self.annotations.iter_mut().find(|a| a.name == parent)
        {
            parent_ann.replies.push(FlowCommentReply::new(
                self.ids.next("reply"),
                author,
                date,
                content,
            ));
            return;
        }

        let offset = char_len_of_spans(spans);
        self.annotations.push(OdtAnnotation {
            name: name.clone(),
            author,
            date,
            content,
            block_id: block_id.to_string(),
            char_start: offset,
            char_end: None,
            anchor_text: String::new(),
            resolved,
            replies: Vec::new(),
        });
        if !active_annotations.contains(&name) {
            active_annotations.push(name);
        }
    }
}

fn char_len_of_spans(spans: &[TextSpan]) -> usize {
    spans.iter().map(|s| s.text.chars().count()).sum()
}
