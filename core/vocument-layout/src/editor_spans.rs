//! Low-level inline TextSpan mutations, splitting, and tracking attribution (`ADR-0020`).

use std::sync::atomic::{AtomicU64, Ordering};
use vocument_flow::model::Block;
use vocument_flow::styles::{ColorRgba, TextSpan};

pub(crate) fn block_id(block: &Block) -> String {
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

pub(crate) fn uuid_fast() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:08x}{:08x}", std::process::id(), id)
}

pub(crate) fn insert_into_spans(spans: &mut Vec<TextSpan>, target_offset: usize, text: &str) {
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

    if let Some(last) = spans.last_mut() {
        last.text.push_str(text);
    }
}

pub(crate) fn insert_span_into_spans(
    spans: &mut Vec<TextSpan>,
    target_offset: usize,
    new_span: TextSpan,
) {
    if spans.is_empty() {
        spans.push(new_span);
        return;
    }
    let mut accumulated = 0;
    for idx in 0..spans.len() {
        let count = spans[idx].text.chars().count();
        if target_offset <= accumulated + count {
            let offset_in_span = target_offset.saturating_sub(accumulated);
            if offset_in_span == 0 {
                spans.insert(idx, new_span);
                return;
            } else if offset_in_span == count {
                spans.insert(idx + 1, new_span);
                return;
            } else {
                let tail_spans = split_spans_at(spans, target_offset);
                spans.push(new_span);
                spans.extend(tail_spans);
                return;
            }
        }
        accumulated += count;
    }
    spans.push(new_span);
}

pub(crate) fn mark_char_before_deleted_in_spans(
    spans: &mut Vec<TextSpan>,
    target_offset: usize,
    change_id: &str,
) {
    if target_offset == 0 || spans.is_empty() {
        return;
    }
    let mut accumulated = 0;
    for idx in 0..spans.len() {
        let count = spans[idx].text.chars().count();
        if target_offset <= accumulated + count {
            let offset_in_span = target_offset.saturating_sub(accumulated);
            if offset_in_span > 0 {
                let char_idx = offset_in_span - 1;
                let ch_str = spans[idx]
                    .text
                    .chars()
                    .nth(char_idx)
                    .map(|c| c.to_string())
                    .unwrap_or_default();
                if ch_str.is_empty() {
                    return;
                }
                if spans[idx].tracked_change_id.as_deref() == Some(change_id) {
                    return;
                }

                let byte_start = spans[idx]
                    .text
                    .char_indices()
                    .nth(char_idx)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let char_len = ch_str.len();
                spans[idx]
                    .text
                    .replace_range(byte_start..byte_start + char_len, "");

                let mut del_span = TextSpan::plain(ch_str);
                del_span.style.strikethrough = true;
                del_span.color = Some(ColorRgba::rgb(204, 0, 0));
                del_span.tracked_change_id = Some(change_id.to_string());

                spans.insert(idx + 1, del_span);
                return;
            }
        }
        accumulated += count;
    }
}

pub(crate) fn delete_char_before_in_spans(spans: &mut [TextSpan], target_offset: usize) {
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

pub(crate) fn delete_char_at_in_spans(spans: &mut [TextSpan], target_offset: usize) {
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

pub(crate) fn split_spans_at(spans: &mut Vec<TextSpan>, target_offset: usize) -> Vec<TextSpan> {
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

        while spans.len() > idx + 1 {
            tail.push(spans.remove(idx + 1));
        }
    }

    if tail.is_empty() {
        tail.push(TextSpan::plain(""));
    }

    tail
}

pub(crate) fn for_each_span_in_block_mut<F: FnMut(&mut TextSpan)>(block: &mut Block, f: &mut F) {
    match block {
        Block::Paragraph(p) => {
            for span in &mut p.spans {
                f(span);
            }
        }
        Block::Heading(h) => {
            for span in &mut h.spans {
                f(span);
            }
        }
        Block::List(l) => {
            fn walk_list<F: FnMut(&mut TextSpan)>(
                list: &mut vocument_flow::model::ListBlock,
                f: &mut F,
            ) {
                for item in &mut list.items {
                    for span in &mut item.spans {
                        f(span);
                    }
                    if let Some(nested) = &mut item.nested_list {
                        walk_list(nested, f);
                    }
                }
            }
            walk_list(l, f);
        }
        Block::Table(t) => {
            for row in &mut t.rows {
                for cell in &mut row.cells {
                    for b in &mut cell.blocks {
                        for_each_span_in_block_mut(b, f);
                    }
                }
            }
        }
        Block::Callout(c) => {
            for b in &mut c.blocks {
                for_each_span_in_block_mut(b, f);
            }
        }
        Block::Image(_) | Block::ThematicBreak(_) => {}
    }
}

pub(crate) fn retain_spans_in_block<F: FnMut(&TextSpan) -> bool>(block: &mut Block, f: &mut F) {
    match block {
        Block::Paragraph(p) => {
            p.spans.retain(|s| f(s));
            if p.spans.is_empty() {
                p.spans.push(TextSpan::plain(""));
            }
        }
        Block::Heading(h) => {
            h.spans.retain(|s| f(s));
            if h.spans.is_empty() {
                h.spans.push(TextSpan::plain(""));
            }
        }
        Block::List(l) => {
            fn walk_list<F: FnMut(&TextSpan) -> bool>(
                list: &mut vocument_flow::model::ListBlock,
                f: &mut F,
            ) {
                for item in &mut list.items {
                    item.spans.retain(|s| f(s));
                    if item.spans.is_empty() {
                        item.spans.push(TextSpan::plain(""));
                    }
                    if let Some(nested) = &mut item.nested_list {
                        walk_list(nested, f);
                    }
                }
            }
            walk_list(l, f);
        }
        Block::Table(t) => {
            for row in &mut t.rows {
                for cell in &mut row.cells {
                    for b in &mut cell.blocks {
                        retain_spans_in_block(b, f);
                    }
                }
            }
        }
        Block::Callout(c) => {
            for b in &mut c.blocks {
                retain_spans_in_block(b, f);
            }
        }
        Block::Image(_) | Block::ThematicBreak(_) => {}
    }
}
