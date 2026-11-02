//! DOCX paragraph run, hyperlink, drawing, footnote reference, and tracked-change parsing.

use std::collections::BTreeMap;

use vocument_flow::model::{FootnoteDefinition, ImageBlock};
use vocument_flow::review::{ChangeType, TrackedChange};
use vocument_flow::styles::{FontStyle, TextSpan};

use crate::convert::ImportedAsset;
use crate::docx::import::{CommentAnchorInfo, ImportContext};
use crate::docx::props::{RunProps, attr, child_w, is_w};
use crate::error::OfficeError;
use crate::report::Severity;
use crate::util::{content_addressed_path, emu_to_pt, safe_hyperlink, sniff_image};
use crate::xml::Element;

impl ImportContext<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn collect_paragraph_runs(
        &mut self,
        container: &Element,
        para_style_id: Option<&str>,
        link_url: Option<&str>,
        tracked_ctx: Option<(&str, ChangeType)>,
        block_id: &str,
        spans: &mut Vec<TextSpan>,
        inline_images: &mut Vec<ImageBlock>,
        active_comments: &mut Vec<String>,
    ) -> Result<(), OfficeError> {
        let mut in_page_field = false;
        let mut skip_field_result = false;
        for child in container.child_elements() {
            match child.local.as_str() {
                "r" if is_w(child, "r") => {
                    for rc in child.child_elements() {
                        if is_w(rc, "fldChar") {
                            match attr(rc, "fldCharType") {
                                Some("begin") => {
                                    in_page_field = false;
                                    skip_field_result = false;
                                }
                                Some("separate") => {
                                    if in_page_field {
                                        skip_field_result = true;
                                    }
                                }
                                Some("end") => {
                                    in_page_field = false;
                                    skip_field_result = false;
                                }
                                _ => {}
                            }
                        } else if is_w(rc, "instrText") {
                            let instr = rc.text_content().to_ascii_uppercase();
                            if instr.contains("PAGE")
                                || instr.contains("NUMPAGES")
                                || instr.contains("SECTIONPAGES")
                            {
                                in_page_field = true;
                            }
                        }
                    }
                    if skip_field_result {
                        continue;
                    }
                    for rc in child.child_elements() {
                        if is_w(rc, "fldChar") {
                            match attr(rc, "fldCharType") {
                                Some("begin") => {
                                    in_page_field = false;
                                    skip_field_result = false;
                                }
                                Some("separate") => {
                                    if in_page_field {
                                        skip_field_result = true;
                                    }
                                }
                                Some("end") => {
                                    in_page_field = false;
                                    skip_field_result = false;
                                }
                                _ => {}
                            }
                        } else if is_w(rc, "instrText") {
                            let instr = rc.text_content().to_ascii_uppercase();
                            if instr.contains("PAGE")
                                || instr.contains("NUMPAGES")
                                || instr.contains("SECTIONPAGES")
                            {
                                in_page_field = true;
                            }
                        }
                    }
                    if skip_field_result {
                        continue;
                    }
                    self.convert_run(
                        child,
                        para_style_id,
                        link_url,
                        tracked_ctx,
                        block_id,
                        spans,
                        inline_images,
                        active_comments,
                    )?;
                }
                "hyperlink" if is_w(child, "hyperlink") => {
                    let resolved_url = self.resolve_hyperlink(child, block_id);
                    self.collect_paragraph_runs(
                        child,
                        para_style_id,
                        resolved_url.as_deref(),
                        tracked_ctx,
                        block_id,
                        spans,
                        inline_images,
                        active_comments,
                    )?;
                }
                "fldSimple" if is_w(child, "fldSimple") => {
                    let instr = attr(child, "instr")
                        .unwrap_or_default()
                        .to_ascii_uppercase();
                    if instr.contains("NUMPAGES") || instr.contains("SECTIONPAGES") {
                        spans.push(TextSpan::plain("{total_pages}"));
                    } else if instr.contains("PAGE") {
                        spans.push(TextSpan::plain("{page}"));
                    } else {
                        self.report.note(
                            Severity::Approximated,
                            "docx.field.simple",
                            "Word simple field imported as static result text",
                            Some(block_id),
                        );
                        self.collect_paragraph_runs(
                            child,
                            para_style_id,
                            link_url,
                            tracked_ctx,
                            block_id,
                            spans,
                            inline_images,
                            active_comments,
                        )?;
                    }
                }
                "ins" | "moveTo" if is_w(child, &child.local) => {
                    let chg_id = self.ids.next("chg");
                    let author = attr(child, "author").unwrap_or("Author").to_string();
                    let date = attr(child, "date")
                        .unwrap_or("1970-01-01T00:00:00Z")
                        .to_string();
                    let mut tc = TrackedChange::new(&chg_id, ChangeType::Insert, author, date);
                    tc.target_block_id = Some(block_id.to_string());
                    tc.text_snippet = Some(child.text_content());
                    self.tracked_changes.push(tc);
                    self.collect_paragraph_runs(
                        child,
                        para_style_id,
                        link_url,
                        Some((&chg_id, ChangeType::Insert)),
                        block_id,
                        spans,
                        inline_images,
                        active_comments,
                    )?;
                }
                "del" | "moveFrom" if is_w(child, &child.local) => {
                    let chg_id = self.ids.next("chg");
                    let author = attr(child, "author").unwrap_or("Author").to_string();
                    let date = attr(child, "date")
                        .unwrap_or("1970-01-01T00:00:00Z")
                        .to_string();
                    let mut tc = TrackedChange::new(&chg_id, ChangeType::Delete, author, date);
                    tc.target_block_id = Some(block_id.to_string());
                    tc.text_snippet = Some(child.text_content());
                    self.tracked_changes.push(tc);
                    self.collect_paragraph_runs(
                        child,
                        para_style_id,
                        link_url,
                        Some((&chg_id, ChangeType::Delete)),
                        block_id,
                        spans,
                        inline_images,
                        active_comments,
                    )?;
                }
                "commentRangeStart" if is_w(child, "commentRangeStart") => {
                    if let Some(cid) = attr(child, "id") {
                        let offset = char_len_of_spans(spans);
                        self.comment_anchors.insert(
                            cid.to_string(),
                            CommentAnchorInfo {
                                block_id: block_id.to_string(),
                                char_start: offset,
                                char_end: None,
                                text: String::new(),
                            },
                        );
                        if !active_comments.iter().any(|c| c == cid) {
                            active_comments.push(cid.to_string());
                        }
                    }
                }
                "commentRangeEnd" if is_w(child, "commentRangeEnd") => {
                    if let Some(cid) = attr(child, "id") {
                        let offset = char_len_of_spans(spans);
                        if let Some(anchor) = self.comment_anchors.get_mut(cid) {
                            anchor.char_end = Some(offset);
                        }
                        active_comments.retain(|c| c != cid);
                    }
                }
                "sdt" if is_w(child, "sdt") => {
                    if let Some(content) = child_w(child, "sdtContent") {
                        self.collect_paragraph_runs(
                            content,
                            para_style_id,
                            link_url,
                            tracked_ctx,
                            block_id,
                            spans,
                            inline_images,
                            active_comments,
                        )?;
                    }
                }
                "smartTag" if is_w(child, "smartTag") => {
                    self.collect_paragraph_runs(
                        child,
                        para_style_id,
                        link_url,
                        tracked_ctx,
                        block_id,
                        spans,
                        inline_images,
                        active_comments,
                    )?;
                }
                "oMath" | "oMathPara" => {
                    let math_text = child.text_content();
                    if !math_text.is_empty() {
                        spans.push(TextSpan::plain(math_text));
                    }
                    self.report.note(
                        Severity::Approximated,
                        "docx.math.omml",
                        "Office Math equation imported as plain text",
                        Some(block_id),
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn resolve_hyperlink(&mut self, hyperlink: &Element, block_id: &str) -> Option<String> {
        if let Some(anchor) = attr(hyperlink, "anchor")
            && !anchor.trim().is_empty()
        {
            return safe_hyperlink(&format!("#{}", anchor.trim()));
        }
        let rel_id = hyperlink.attribute_any_namespace("id")?;
        let rel = self.rels.get(rel_id)?;
        if let Some(safe) = safe_hyperlink(&rel.target) {
            Some(safe)
        } else {
            self.report.note(
                Severity::Security,
                "docx.hyperlink.unsafe-scheme",
                "Unsafe or local hyperlink scheme stripped during import",
                Some(block_id),
            );
            None
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn convert_run(
        &mut self,
        run_el: &Element,
        para_style_id: Option<&str>,
        link_url: Option<&str>,
        tracked_ctx: Option<(&str, ChangeType)>,
        block_id: &str,
        spans: &mut Vec<TextSpan>,
        inline_images: &mut Vec<ImageBlock>,
        active_comments: &mut [String],
    ) -> Result<(), OfficeError> {
        let direct_rpr = child_w(run_el, "rPr")
            .map(RunProps::from_element)
            .unwrap_or_default();
        if direct_rpr.hidden == Some(true) {
            self.report.note(
                Severity::PreservedUnrendered,
                "docx.run.vanish",
                "Hidden run (w:vanish) omitted from visible text",
                Some(block_id),
            );
            return Ok(());
        }

        let mut format_change_id: Option<(String, FontStyle)> = None;
        if let Some(rpr_el) = child_w(run_el, "rPr")
            && let Some(rpr_chg) = child_w(rpr_el, "rPrChange")
        {
            let chg_id = self.ids.next("chg");
            let author = attr(rpr_chg, "author").unwrap_or("Author").to_string();
            let date = attr(rpr_chg, "date")
                .unwrap_or("1970-01-01T00:00:00Z")
                .to_string();
            let prev_rpr = child_w(rpr_chg, "rPr")
                .map(RunProps::from_element)
                .unwrap_or_default();
            let prev_style = FontStyle {
                bold: prev_rpr.bold.unwrap_or(false),
                italic: prev_rpr.italic.unwrap_or(false),
                underline: prev_rpr.underline.unwrap_or(false),
                strikethrough: prev_rpr.strike.unwrap_or(false),
                code: false,
                subscript: prev_rpr.subscript.unwrap_or(false),
                superscript: prev_rpr.superscript.unwrap_or(false),
            };
            let mut tc = TrackedChange::new(&chg_id, ChangeType::FormatChange, author, date);
            tc.target_block_id = Some(block_id.to_string());
            self.tracked_changes.push(tc);
            format_change_id = Some((chg_id, prev_style));
        }

        let mut eff_r = self
            .styles
            .effective_run(para_style_id, direct_rpr.character_style.as_deref());
        eff_r.overlay(&direct_rpr);

        let mut text = String::new();
        for child in run_el.child_elements() {
            match child.local.as_str() {
                "t" | "delText" if is_w(child, &child.local) => {
                    text.push_str(&child.text_content());
                }
                "tab" if is_w(child, "tab") => text.push('\t'),
                "br" | "cr" if is_w(child, &child.local) => text.push('\n'),
                "softHyphen" if is_w(child, "softHyphen") => text.push('\u{00AD}'),
                "noBreakHyphen" if is_w(child, "noBreakHyphen") => text.push('\u{2011}'),
                "instrText" if is_w(child, "instrText") => {
                    let instr = child.text_content().to_ascii_uppercase();
                    if instr.contains("NUMPAGES") || instr.contains("SECTIONPAGES") {
                        text.push_str("{total_pages}");
                    } else if instr.contains("PAGE") {
                        text.push_str("{page}");
                    }
                }
                "footnoteReference" if is_w(child, "footnoteReference") => {
                    if !text.is_empty() {
                        self.push_styled_span(
                            std::mem::take(&mut text),
                            &eff_r,
                            &direct_rpr,
                            link_url,
                            tracked_ctx,
                            format_change_id.as_ref(),
                            spans,
                            active_comments,
                        );
                    }
                    if let Some(xml_id) = attr(child, "id")
                        && let Some(fn_id) = self.materialize_note(xml_id, false)?
                    {
                        spans.push(TextSpan::plain("").with_footnote(fn_id));
                    }
                }
                "endnoteReference" if is_w(child, "endnoteReference") => {
                    if !text.is_empty() {
                        self.push_styled_span(
                            std::mem::take(&mut text),
                            &eff_r,
                            &direct_rpr,
                            link_url,
                            tracked_ctx,
                            format_change_id.as_ref(),
                            spans,
                            active_comments,
                        );
                    }
                    if let Some(xml_id) = attr(child, "id")
                        && let Some(fn_id) = self.materialize_note(xml_id, true)?
                    {
                        self.report.note(
                            Severity::Approximated,
                            "docx.endnote.to-footnote",
                            "Endnote imported as document footnote",
                            Some(block_id),
                        );
                        spans.push(TextSpan::plain("").with_footnote(fn_id));
                    }
                }
                "commentReference" if is_w(child, "commentReference") => {
                    if let Some(cid) = attr(child, "id") {
                        let offset = char_len_of_spans(spans) + text.chars().count();
                        self.comment_anchors
                            .entry(cid.to_string())
                            .or_insert_with(|| CommentAnchorInfo {
                                block_id: block_id.to_string(),
                                char_start: offset,
                                char_end: Some(offset),
                                text: String::new(),
                            });
                    }
                }
                "drawing" | "pict" if is_w(child, &child.local) => {
                    self.extract_drawing(child, block_id, inline_images, &mut text);
                }
                "AlternateContent" => {
                    let branch = child
                        .child_elements()
                        .find(|e| e.local == "Choice")
                        .or_else(|| child.child_elements().next());
                    if let Some(b) = branch {
                        for inner in b.child_elements() {
                            if inner.local == "drawing" || inner.local == "pict" {
                                self.extract_drawing(inner, block_id, inline_images, &mut text);
                            }
                        }
                    }
                }
                "object" if is_w(child, "object") => {
                    self.report.note(
                        Severity::Loss,
                        "docx.run.ole-object",
                        "Embedded OLE object cannot be rendered",
                        Some(block_id),
                    );
                }
                _ => {}
            }
        }

        if !text.is_empty() {
            self.push_styled_span(
                text,
                &eff_r,
                &direct_rpr,
                link_url,
                tracked_ctx,
                format_change_id.as_ref(),
                spans,
                active_comments,
            );
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn push_styled_span(
        &mut self,
        mut text: String,
        eff_r: &RunProps,
        direct_r: &RunProps,
        link_url: Option<&str>,
        tracked_ctx: Option<(&str, ChangeType)>,
        format_change: Option<&(String, FontStyle)>,
        spans: &mut Vec<TextSpan>,
        active_comments: &mut [String],
    ) {
        if eff_r.caps == Some(true) {
            text = text.to_uppercase();
        }
        for cid in active_comments.iter() {
            if let Some(anchor) = self.comment_anchors.get_mut(cid) {
                anchor.text.push_str(&text);
            }
        }

        let mut style = FontStyle {
            bold: eff_r.bold.unwrap_or(false),
            italic: eff_r.italic.unwrap_or(false),
            underline: eff_r.underline.unwrap_or(false),
            strikethrough: eff_r.strike.unwrap_or(false),
            code: false,
            subscript: eff_r.subscript.unwrap_or(false),
            superscript: eff_r.superscript.unwrap_or(false),
        };
        let mut tracked_change_id = None;
        let mut original_style = None;

        if let Some((cid, kind)) = tracked_ctx {
            tracked_change_id = Some(cid.to_string());
            match kind {
                ChangeType::Insert => style.underline = true,
                ChangeType::Delete => style.strikethrough = true,
                ChangeType::FormatChange => {}
            }
        } else if let Some((cid, prev_style)) = format_change {
            tracked_change_id = Some(cid.clone());
            original_style = Some(*prev_style);
        }

        let span = TextSpan {
            text,
            style,
            font_size_pt: direct_r.size_pt.map(|s| s.clamp(4.0, 200.0)),
            font_family: direct_r.family.clone(),
            font_weight: direct_r.bold.map(|b| if b { 700 } else { 400 }),
            color: eff_r.color.flatten(),
            background_color: eff_r.background,
            link_url: link_url.map(str::to_string),
            footnote_ref: None,
            tracked_change_id,
            original_style,
            extra: BTreeMap::new(),
        };
        spans.push(span);
    }

    fn extract_drawing(
        &mut self,
        drawing: &Element,
        block_id: &str,
        inline_images: &mut Vec<ImageBlock>,
        fallback_text: &mut String,
    ) {
        let mut found_blip = false;
        let mut width_pt = None;
        let mut height_pt = None;
        let mut alt_text = String::new();
        let mut embed_id: Option<String> = None;
        let mut external_link = false;

        let mut stack = vec![drawing];
        while let Some(el) = stack.pop() {
            match el.local.as_str() {
                "extent" => {
                    if let Some(cx) = el
                        .attribute_any_namespace("cx")
                        .and_then(|v| v.parse::<i64>().ok())
                    {
                        width_pt = Some(emu_to_pt(cx).max(1.0));
                    }
                    if let Some(cy) = el
                        .attribute_any_namespace("cy")
                        .and_then(|v| v.parse::<i64>().ok())
                    {
                        height_pt = Some(emu_to_pt(cy).max(1.0));
                    }
                }
                "docPr" => {
                    if let Some(descr) = el
                        .attribute_any_namespace("descr")
                        .or_else(|| el.attribute_any_namespace("title"))
                        && !descr.is_empty()
                    {
                        alt_text = descr.to_string();
                    }
                }
                "blip" | "imagedata" => {
                    found_blip = true;
                    if let Some(r_embed) = el
                        .attribute_any_namespace("embed")
                        .or_else(|| el.attribute_any_namespace("id"))
                    {
                        embed_id = Some(r_embed.to_string());
                    } else if el.attribute_any_namespace("link").is_some() {
                        external_link = true;
                    }
                }
                "chart" => {
                    self.report.note(
                        Severity::Loss,
                        "docx.drawing.chart",
                        "Embedded DrawingML chart cannot be rendered",
                        Some(block_id),
                    );
                }
                "txbxContent" => {
                    let t = el.text_content();
                    if !t.is_empty() {
                        if !fallback_text.is_empty() {
                            fallback_text.push(' ');
                        }
                        fallback_text.push_str(&t);
                        self.report.note(
                            Severity::Approximated,
                            "docx.drawing.textbox",
                            "Text box content extracted into paragraph text",
                            Some(block_id),
                        );
                    }
                    continue;
                }
                _ => {}
            }
            stack.extend(el.child_elements());
        }

        if external_link {
            self.report.note(
                Severity::Security,
                "docx.image.external-link",
                "External linked image blocked for security",
                Some(block_id),
            );
            return;
        }
        let Some(rel_id) = embed_id else {
            if found_blip {
                self.report.note(
                    Severity::Loss,
                    "docx.image.missing-rel",
                    "Image element has no relationship reference",
                    Some(block_id),
                );
            }
            return;
        };
        let Some(rel) = self.rels.get(&rel_id) else {
            self.report.note(
                Severity::Loss,
                "docx.image.unresolved-rel",
                "Image relationship target not found in .rels",
                Some(block_id),
            );
            return;
        };
        if rel.is_external {
            self.report.note(
                Severity::Security,
                "docx.image.external-rel",
                "External image relationship blocked for security",
                Some(block_id),
            );
            return;
        }
        let part_path = rel.resolved_part_path(self.base_dir);
        let Some(img_bytes) = self.package.part(&part_path) else {
            self.report.note(
                Severity::Loss,
                "docx.image.missing-part",
                &format!("Image binary part '{part_path}' missing from archive"),
                Some(block_id),
            );
            return;
        };

        let info = sniff_image(img_bytes);
        let (ext, media_type, px_size) = match &info {
            Some(i) => (i.extension, i.media_type.to_string(), i.pixel_size),
            None => ("bin", "application/octet-stream".to_string(), None),
        };
        let asset_ref = content_addressed_path(ext, img_bytes);
        self.assets.insert(
            asset_ref.clone(),
            ImportedAsset {
                media_type,
                bytes: img_bytes.to_vec(),
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
        let img_block = ImageBlock {
            id: self.ids.next("img"),
            asset_ref,
            alt_text: if alt_text.is_empty() {
                "Image".to_string()
            } else {
                alt_text
            },
            caption: None,
            intrinsic_width_pt: intrinsic_w,
            intrinsic_height_pt: intrinsic_h,
            display_width_pt: width_pt,
            display_height_pt: height_pt,
            extra: BTreeMap::new(),
        };
        inline_images.push(img_block);
    }

    fn materialize_note(
        &mut self,
        xml_id: &str,
        is_endnote: bool,
    ) -> Result<Option<String>, OfficeError> {
        let source_el = if is_endnote {
            self.endnote_xml_map.get(xml_id).cloned()
        } else {
            self.footnote_xml_map.get(xml_id).cloned()
        };
        let Some(note_el) = source_el else {
            return Ok(None);
        };
        let fn_id = self.ids.next("fn");
        let number = (self.footnotes.len() as u32) + 1;
        let mut items = Vec::new();
        for child in note_el.child_elements() {
            self.convert_block_element(child, &mut items)?;
        }
        let blocks = self.flush_block_items(items);
        self.footnotes.insert(
            fn_id.clone(),
            FootnoteDefinition::new(fn_id.clone(), number, blocks),
        );
        Ok(Some(fn_id))
    }
}

fn char_len_of_spans(spans: &[TextSpan]) -> usize {
    spans.iter().map(|s| s.text.chars().count()).sum()
}
