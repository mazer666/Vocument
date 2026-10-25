use crate::geometry::Rect;
use crate::tree::{LayoutBoxKind, LayoutLine, LayoutPage};

/// A resolved visual cursor location within a document block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorLocation {
    pub block_id: String,
    pub char_offset: usize,
}

/// Hit-tests a 2D point on a layout page to find the nearest block ID and character offset.
pub fn hit_test_page(page: &LayoutPage, x_pt: f32, y_pt: f32) -> Option<CursorLocation> {
    // 1. Find line closest to y_pt
    let mut closest_line: Option<(&str, &LayoutLine, f32)> = None;

    for lbox in page.all_boxes() {
        if let LayoutBoxKind::TextLine(ref line) = lbox.kind {
            let block_id = lbox.id.split(':').next().unwrap_or(&lbox.id);
            let y_dist = if y_pt < line.bounds.y {
                line.bounds.y - y_pt
            } else if y_pt > line.bounds.y + line.bounds.height {
                y_pt - (line.bounds.y + line.bounds.height)
            } else {
                0.0
            };

            match closest_line {
                None => closest_line = Some((block_id, line, y_dist)),
                Some((_, _, min_dist)) if y_dist < min_dist => {
                    closest_line = Some((block_id, line, y_dist));
                }
                _ => {}
            }
        }
    }

    let (block_id, line, _) = closest_line?;
    let char_offset = hit_test_line(line, x_pt);
    Some(CursorLocation {
        block_id: block_id.to_string(),
        char_offset,
    })
}

/// Maps an x-coordinate along a line to a character offset.
pub fn hit_test_line(line: &LayoutLine, x_pt: f32) -> usize {
    if line.runs.is_empty() {
        return 0;
    }

    let mut accumulated_chars = 0;

    for run in &line.runs {
        let run_start_x = run.bounds.x;
        let run_end_x = run.bounds.x + run.bounds.width;
        let char_count = run.text.chars().count();

        if x_pt <= run_start_x {
            return accumulated_chars;
        }

        if x_pt >= run_end_x {
            accumulated_chars += char_count;
            continue;
        }

        // Inside this run: interpolate character index based on width
        if char_count > 0 && run.bounds.width > 0.0 {
            let char_width = run.bounds.width / char_count as f32;
            let rel_x = x_pt - run_start_x;
            let idx = (rel_x / char_width).round() as usize;
            return accumulated_chars + idx.min(char_count);
        }

        accumulated_chars += char_count;
    }

    accumulated_chars
}

/// Computes the visual cursor rectangle `(x, y, width, height)` for a given block and character offset.
pub fn cursor_rect_for_offset(
    page: &LayoutPage,
    block_id: &str,
    target_offset: usize,
) -> Option<Rect> {
    let mut current_block_chars = 0;

    for lbox in page.all_boxes() {
        if let LayoutBoxKind::TextLine(ref line) = lbox.kind {
            let box_block_id = lbox.id.split(':').next().unwrap_or(&lbox.id);
            if box_block_id != block_id {
                continue;
            }

            let line_chars: usize = line.runs.iter().map(|r| r.text.chars().count()).sum();
            let line_end_chars = current_block_chars + line_chars;

            if target_offset <= line_end_chars || line_chars == 0 {
                // The cursor is on this line
                let offset_in_line = target_offset.saturating_sub(current_block_chars);
                let x = compute_x_in_line(line, offset_in_line);
                return Some(Rect::new(
                    x,
                    line.bounds.y,
                    2.0,
                    line.bounds.height.max(12.0),
                ));
            }

            current_block_chars = line_end_chars;
        }
    }

    None
}

fn compute_x_in_line(line: &LayoutLine, mut offset_in_line: usize) -> f32 {
    if line.runs.is_empty() {
        return line.bounds.x;
    }

    for run in &line.runs {
        let count = run.text.chars().count();
        if offset_in_line <= count {
            if count == 0 || run.bounds.width <= 0.0 {
                return run.bounds.x;
            }
            let char_width = run.bounds.width / count as f32;
            return run.bounds.x + (offset_in_line as f32 * char_width);
        }
        offset_in_line -= count;
    }

    // At the end of the line
    line.runs
        .last()
        .map(|r| r.bounds.x + r.bounds.width)
        .unwrap_or(line.bounds.x)
}

/// Computes visual selection highlight rectangles for a text range on a page.
pub fn selection_rects_for_range(
    page: &LayoutPage,
    block_id: &str,
    start_offset: usize,
    end_offset: usize,
) -> Vec<Rect> {
    let mut rects = Vec::new();
    let min_offset = start_offset.min(end_offset);
    let max_offset = start_offset.max(end_offset);

    if min_offset == max_offset {
        return rects;
    }

    let mut current_block_chars = 0;

    for lbox in page.all_boxes() {
        if let LayoutBoxKind::TextLine(ref line) = lbox.kind {
            let box_block_id = lbox.id.split(':').next().unwrap_or(&lbox.id);
            if box_block_id != block_id {
                continue;
            }

            let line_chars: usize = line.runs.iter().map(|r| r.text.chars().count()).sum();
            let line_start = current_block_chars;
            let line_end = current_block_chars + line_chars;

            if max_offset > line_start && min_offset < line_end {
                // This line intersects the selection
                let sel_start_in_line = min_offset.saturating_sub(line_start).min(line_chars);
                let sel_end_in_line = max_offset.saturating_sub(line_start).min(line_chars);

                let x1 = compute_x_in_line(line, sel_start_in_line);
                let x2 = compute_x_in_line(line, sel_end_in_line);

                let sel_x = x1.min(x2);
                let sel_w = (x2 - x1).abs().max(4.0);
                rects.push(Rect::new(sel_x, line.bounds.y, sel_w, line.bounds.height));
            }

            current_block_chars = line_end;
        }
    }

    rects
}
