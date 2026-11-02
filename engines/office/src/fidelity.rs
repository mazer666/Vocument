//! Objective 3-tier roundtrip fidelity evaluation (`OFFICE-001`–`005`, `M4.5`).
//!
//! Evaluates a pair of [`FlowDocument`] instances (original import vs. re-imported export)
//! against the M4.5 multi-tiered fidelity gates:
//! - **Tier 1 (Target 100%):** Plain text, headings (levels 1–6), character styles, inline footnotes.
//! - **Tier 2 (Target >= 95%):** Table cells (including `col_span`/`row_span`/`is_header`), nested lists, image dimensions.
//! - **Tier 3:** Unmapped XML pass-through preservation and explicit [`ConversionReport`] accounting.

use serde::{Deserialize, Serialize};
use vocument_flow::model::{Block, FlowDocument, ImageBlock, ListBlock, ListKind, TableBlock};
use vocument_flow::styles::{FontStyle, TextSpan};

use crate::report::{ConversionReport, Severity};
use crate::walk::visit_blocks;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tier1Metrics {
    pub plain_text_similarity: f64,
    pub heading_preservation: f64,
    pub character_style_preservation: f64,
    pub footnote_preservation: f64,
}

impl Tier1Metrics {
    pub fn score(&self) -> f64 {
        (self.plain_text_similarity
            + self.heading_preservation
            + self.character_style_preservation
            + self.footnote_preservation)
            / 4.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tier2Metrics {
    pub table_cell_preservation: f64,
    pub nested_list_preservation: f64,
    pub image_dimension_preservation: f64,
}

impl Tier2Metrics {
    pub fn score(&self) -> f64 {
        (self.table_cell_preservation
            + self.nested_list_preservation
            + self.image_dimension_preservation)
            / 3.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tier3Metrics {
    pub passthrough_entries_source: usize,
    pub passthrough_entries_roundtrip: usize,
    pub reported_approximations: usize,
    pub reported_losses: usize,
    pub security_stripped: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FidelityEvaluation {
    pub tier1: Tier1Metrics,
    pub tier2: Tier2Metrics,
    pub tier3: Tier3Metrics,
    pub comments_source: usize,
    pub comments_roundtrip: usize,
    pub tracked_changes_source: usize,
    pub tracked_changes_roundtrip: usize,
}

impl FidelityEvaluation {
    /// Returns `true` when Tier 1 achieves 100% (within floating-point epsilon) and Tier 2 >= 95%.
    pub fn passes_m4_5_gate(&self) -> bool {
        self.tier1.plain_text_similarity >= 0.999
            && self.tier1.heading_preservation >= 0.999
            && self.tier1.character_style_preservation >= 0.999
            && self.tier1.footnote_preservation >= 0.999
            && self.tier2.score() >= 0.95
    }
}

/// Evaluates the structural and textual fidelity between `source` and `roundtrip`.
pub fn evaluate_fidelity(
    source: &FlowDocument,
    roundtrip: &FlowDocument,
    import_report: &ConversionReport,
    export_report: &ConversionReport,
) -> FidelityEvaluation {
    let src_text = normalize_ws(&extract_all_text(source));
    let rt_text = normalize_ws(&extract_all_text(roundtrip));
    let plain_text_similarity = char_multiset_jaccard(&src_text, &rt_text);

    let src_headings = collect_headings(source);
    let rt_headings = collect_headings(roundtrip);
    let heading_preservation = exact_slice_ratio(&src_headings, &rt_headings);

    let src_styled = collect_styled_tokens(source);
    let rt_styled = collect_styled_tokens(roundtrip);
    let character_style_preservation = exact_slice_ratio(&src_styled, &rt_styled);

    let src_fn = collect_footnotes(source);
    let rt_fn = collect_footnotes(roundtrip);
    let footnote_preservation = exact_slice_ratio(&src_fn, &rt_fn);

    let src_cells = collect_table_cells(source);
    let rt_cells = collect_table_cells(roundtrip);
    let table_cell_preservation = exact_slice_ratio(&src_cells, &rt_cells);

    let src_lists = collect_list_items(source);
    let rt_lists = collect_list_items(roundtrip);
    let nested_list_preservation = exact_slice_ratio(&src_lists, &rt_lists);

    let src_imgs = collect_images(source);
    let rt_imgs = collect_images(roundtrip);
    let image_dimension_preservation = compare_images(&src_imgs, &rt_imgs);

    let count_sev = |r: &ConversionReport, s: Severity| -> usize {
        r.entries_with(s).map(|e| e.occurrences as usize).sum()
    };
    let tier3 = Tier3Metrics {
        passthrough_entries_source: count_passthrough_entries(source),
        passthrough_entries_roundtrip: count_passthrough_entries(roundtrip),
        reported_approximations: count_sev(import_report, Severity::Approximated)
            + count_sev(export_report, Severity::Approximated),
        reported_losses: count_sev(import_report, Severity::Loss)
            + count_sev(export_report, Severity::Loss),
        security_stripped: count_sev(import_report, Severity::Security)
            + count_sev(export_report, Severity::Security),
    };

    FidelityEvaluation {
        tier1: Tier1Metrics {
            plain_text_similarity,
            heading_preservation,
            character_style_preservation,
            footnote_preservation,
        },
        tier2: Tier2Metrics {
            table_cell_preservation,
            nested_list_preservation,
            image_dimension_preservation,
        },
        tier3,
        comments_source: source.review.comments.len(),
        comments_roundtrip: roundtrip.review.comments.len(),
        tracked_changes_source: source.review.tracked_changes.len(),
        tracked_changes_roundtrip: roundtrip.review.tracked_changes.len(),
    }
}

fn extract_all_text(doc: &FlowDocument) -> String {
    let mut out = String::new();
    visit_blocks(doc, &mut |block| match block {
        Block::Paragraph(p) => append_spans_text(&mut out, &p.spans),
        Block::Heading(h) => append_spans_text(&mut out, &h.spans),
        Block::List(l) => collect_list_text_top(&mut out, l),
        _ => {}
    });
    out
}

fn collect_list_text_top(out: &mut String, list: &ListBlock) {
    for item in &list.items {
        append_spans_text(out, &item.spans);
        if let Some(nested) = &item.nested_list {
            collect_list_text_top(out, nested);
        }
    }
}

fn append_spans_text(out: &mut String, spans: &[TextSpan]) {
    for s in spans {
        out.push_str(&s.text);
    }
    out.push('\n');
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn char_multiset_jaccard(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let words_a: Vec<&str> = a.split_whitespace().collect();
    let words_b: Vec<&str> = b.split_whitespace().collect();
    let max_len = words_a.len().max(words_b.len());
    if max_len == 0 {
        return 1.0;
    }
    let mut matched = 0usize;
    let mut used = vec![false; words_b.len()];
    for wa in &words_a {
        if let Some((idx, _)) = words_b
            .iter()
            .enumerate()
            .find(|(i, wb)| !used[*i] && *wb == wa)
        {
            used[idx] = true;
            matched += 1;
        }
    }
    (matched as f64) / (max_len as f64)
}

fn exact_slice_ratio<T: PartialEq>(src: &[T], rt: &[T]) -> f64 {
    if src.is_empty() && rt.is_empty() {
        return 1.0;
    }
    if src.is_empty() || rt.is_empty() {
        return 0.0;
    }
    let max_len = src.len().max(rt.len());
    let matches = src.iter().zip(rt.iter()).filter(|(a, b)| a == b).count();
    (matches as f64) / (max_len as f64)
}

fn collect_headings(doc: &FlowDocument) -> Vec<(u8, String)> {
    let mut out = Vec::new();
    visit_blocks(doc, &mut |block| {
        if let Block::Heading(h) = block {
            let text: String = h.spans.iter().map(|s| s.text.as_str()).collect();
            out.push((h.level, normalize_ws(&text)));
        }
    });
    out
}

fn collect_styled_tokens(doc: &FlowDocument) -> Vec<(String, FontStyle)> {
    let mut out = Vec::new();
    let mut visit_spans = |spans: &[TextSpan]| {
        for s in spans {
            let trimmed = normalize_ws(&s.text);
            if !trimmed.is_empty() && s.style != FontStyle::PLAIN {
                for word in trimmed.split_whitespace() {
                    out.push((word.to_string(), s.style));
                }
            }
        }
    };
    visit_blocks(doc, &mut |block| match block {
        Block::Paragraph(p) => visit_spans(&p.spans),
        Block::Heading(h) => visit_spans(&h.spans),
        Block::List(l) => {
            fn walk_list(l: &ListBlock, f: &mut dyn FnMut(&[TextSpan])) {
                for item in &l.items {
                    f(&item.spans);
                    if let Some(nested) = &item.nested_list {
                        walk_list(nested, f);
                    }
                }
            }
            walk_list(l, &mut visit_spans);
        }
        _ => {}
    });
    out
}

fn collect_footnotes(doc: &FlowDocument) -> Vec<String> {
    doc.footnotes
        .values()
        .map(|f| {
            let mut text = String::new();
            for b in &f.blocks {
                if let Block::Paragraph(p) = b {
                    append_spans_text(&mut text, &p.spans);
                }
            }
            normalize_ws(&text)
        })
        .collect()
}

fn collect_table_cells(doc: &FlowDocument) -> Vec<(usize, u32, u32, bool, String)> {
    let mut out = Vec::new();
    visit_blocks(doc, &mut |block| {
        if let Block::Table(tbl) = block {
            collect_table(tbl, &mut out);
        }
    });
    out
}

fn collect_table(tbl: &TableBlock, out: &mut Vec<(usize, u32, u32, bool, String)>) {
    for (r_idx, row) in tbl.rows.iter().enumerate() {
        let is_hdr = row.is_header || (r_idx as u32) < tbl.repeat_header_rows;
        for cell in &row.cells {
            let mut text = String::new();
            for b in &cell.blocks {
                if let Block::Paragraph(p) = b {
                    append_spans_text(&mut text, &p.spans);
                } else if let Block::Heading(h) = b {
                    append_spans_text(&mut text, &h.spans);
                }
            }
            out.push((
                r_idx,
                cell.col_span,
                cell.row_span,
                is_hdr,
                normalize_ws(&text),
            ));
        }
    }
}

fn collect_list_items(doc: &FlowDocument) -> Vec<(u8, ListKind, String)> {
    let mut out = Vec::new();
    fn walk(list: &ListBlock, depth: u8, out: &mut Vec<(u8, ListKind, String)>) {
        for item in &list.items {
            let t: String = item.spans.iter().map(|s| s.text.as_str()).collect();
            let norm = normalize_ws(&t);
            if !norm.is_empty() {
                out.push((depth, list.kind, norm));
            }
            if let Some(nested) = &item.nested_list {
                walk(nested, depth + 1, out);
            }
        }
    }
    visit_blocks(doc, &mut |block| {
        if let Block::List(l) = block {
            walk(l, 0, &mut out);
        }
    });
    out
}

fn collect_images(doc: &FlowDocument) -> Vec<ImageBlock> {
    let mut out = Vec::new();
    visit_blocks(doc, &mut |block| {
        if let Block::Image(img) = block {
            out.push(img.clone());
        }
    });
    out
}

fn compare_images(src: &[ImageBlock], rt: &[ImageBlock]) -> f64 {
    if src.is_empty() && rt.is_empty() {
        return 1.0;
    }
    if src.is_empty() || rt.is_empty() {
        return 0.0;
    }
    let max_len = src.len().max(rt.len());
    let matches = src
        .iter()
        .zip(rt.iter())
        .filter(|(a, b)| {
            let wa = a.display_width_pt.unwrap_or(a.intrinsic_width_pt);
            let wb = b.display_width_pt.unwrap_or(b.intrinsic_width_pt);
            let ha = a.display_height_pt.unwrap_or(a.intrinsic_height_pt);
            let hb = b.display_height_pt.unwrap_or(b.intrinsic_height_pt);
            a.asset_ref == b.asset_ref && (wa - wb).abs() <= 1.0 && (ha - hb).abs() <= 1.0
        })
        .count();
    (matches as f64) / (max_len as f64)
}

fn count_passthrough_entries(doc: &FlowDocument) -> usize {
    let mut count = doc.extra.len() + doc.metadata.extra.len();
    visit_blocks(doc, &mut |block| match block {
        Block::Paragraph(p) => count += p.extra.len(),
        Block::Heading(h) => count += h.extra.len(),
        Block::List(l) => count += l.extra.len(),
        Block::Table(t) => count += t.extra.len(),
        Block::Image(i) => count += i.extra.len(),
        _ => {}
    });
    count
}
