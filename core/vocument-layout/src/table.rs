//! Table block layout, multi-page splitting, and repeated header pagination (`ADR-0020`, `AUTHOR-004`).

use std::collections::HashSet;
use vocument_flow::model::{Block, TableBlock};
use vocument_flow::styles::{ColorRgba, FontStyle, TableStyle, TextSpan};

use crate::geometry::Rect;
use crate::paragraph::{TypesettingContext, layout_heading, layout_list, layout_paragraph};
use crate::tree::{LayoutBox, LayoutBoxKind};

/// Typeset representation of a single cell within a table row.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionedTableCell {
    pub cell_id: String,
    pub col_index: usize,
    pub col_span: usize,
    pub row_span: usize,
    pub bounds: Rect,
    pub background_color: Option<ColorRgba>,
    pub child_boxes: Vec<LayoutBox>,
    pub footnote_refs: Vec<String>,
}

/// Typeset representation of a single table row.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionedTableRow {
    pub row_index: usize,
    pub is_header: bool,
    pub height_pt: f32,
    pub cells: Vec<PositionedTableCell>,
    pub footnote_refs: Vec<String>,
}

/// Typeset representation of an entire table block before pagination splitting.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionedTable {
    pub block_id: String,
    pub caption_boxes: Vec<LayoutBox>,
    pub caption_height_pt: f32,
    pub rows: Vec<PositionedTableRow>,
    pub column_widths: Vec<f32>,
    pub total_width_pt: f32,
    pub repeat_header_rows: usize,
    pub border_width_pt: f32,
    pub border_color: ColorRgba,
    pub space_before_pt: f32,
    pub space_after_pt: f32,
}

/// A slice of a table positioned to fit on a specific page.
#[derive(Debug, Clone, PartialEq)]
pub struct TableSlice {
    pub start_row: usize,
    pub next_row: usize,
    pub is_complete: bool,
    pub total_height_pt: f32,
    pub boxes: Vec<LayoutBox>,
    pub footnote_refs: Vec<String>,
}

/// Typesets a `TableBlock` into a `PositionedTable`.
#[allow(clippy::needless_range_loop)]
pub fn layout_table(table: &TableBlock, ctx: &TypesettingContext) -> PositionedTable {
    let table_style = resolve_table_style(table.style_id.as_deref(), ctx.stylesheet);
    let num_cols = compute_column_count(table);
    let column_widths = compute_column_widths(table, num_cols, ctx.available_width_pt);
    let total_width: f32 = column_widths.iter().sum();

    // Layout caption if present
    let mut caption_boxes = Vec::new();
    let mut caption_height = 0.0;
    if let Some(caption_text) = &table.caption {
        let caption_span = TextSpan {
            text: caption_text.clone(),
            style: FontStyle::bold(),
            font_size_pt: Some(10.0),
            font_family: Some("Inter".to_string()),
            font_weight: Some(700),
            color: None,
            background_color: None,
            link_url: None,
            footnote_ref: None,
            extra: Default::default(),
        };
        let caption_para = vocument_flow::model::ParagraphBlock::new(
            format!("{}:caption", table.id),
            vec![caption_span],
        );
        let pos_caption = layout_paragraph(&caption_para, ctx);
        caption_height = pos_caption.total_height_pt + 4.0;
        caption_boxes = pos_caption.to_layout_boxes(0.0, 0.0);
    }

    // Grid occupation map to compute column index for cells with rowspans/colspans
    let mut grid_occupied: Vec<Vec<bool>> = Vec::new();
    let mut positioned_rows = Vec::with_capacity(table.rows.len());

    for (row_idx, row) in table.rows.iter().enumerate() {
        while grid_occupied.len() <= row_idx {
            grid_occupied.push(vec![false; num_cols]);
        }

        let mut pos_cells = Vec::new();
        let mut row_footnote_refs = Vec::new();
        let mut curr_col = 0;
        let mut row_content_height: f32 = 18.0; // Minimum row height

        let default_bg = if row.is_header {
            table_style.header_background_color
        } else if row_idx % 2 == 1 {
            table_style.alternating_row_color
        } else {
            None
        };

        for cell in &row.cells {
            // Advance to next unoccupied slot
            while curr_col < num_cols && grid_occupied[row_idx][curr_col] {
                curr_col += 1;
            }
            if curr_col >= num_cols {
                break;
            }

            let c_span = (cell.col_span as usize).max(1);
            let r_span = (cell.row_span as usize).max(1);

            // Mark occupied slots
            for r in row_idx..(row_idx + r_span) {
                while grid_occupied.len() <= r {
                    grid_occupied.push(vec![false; num_cols]);
                }
                for c in curr_col..(curr_col + c_span).min(num_cols) {
                    grid_occupied[r][c] = true;
                }
            }

            // Cell x coordinate and width
            let cell_x: f32 = column_widths[..curr_col].iter().sum();
            let cell_w: f32 = column_widths[curr_col..(curr_col + c_span).min(num_cols)]
                .iter()
                .sum();

            // Typeset cell contents
            let (child_boxes, cell_height, fn_refs) =
                layout_cell_blocks(&cell.blocks, cell_w, table_style.cell_padding_pt, ctx);

            row_footnote_refs.extend(fn_refs);
            if r_span == 1 && cell_height > row_content_height {
                row_content_height = cell_height;
            }

            let cell_bg = cell.background_color.or(default_bg);

            pos_cells.push(PositionedTableCell {
                cell_id: cell.id.clone(),
                col_index: curr_col,
                col_span: c_span,
                row_span: r_span,
                bounds: Rect::new(cell_x, 0.0, cell_w, cell_height),
                background_color: cell_bg,
                child_boxes,
                footnote_refs: Vec::new(),
            });

            curr_col += c_span;
        }

        positioned_rows.push(PositionedTableRow {
            row_index: row_idx,
            is_header: row.is_header,
            height_pt: row_content_height,
            cells: pos_cells,
            footnote_refs: row_footnote_refs,
        });
    }

    PositionedTable {
        block_id: table.id.clone(),
        caption_boxes,
        caption_height_pt: caption_height,
        rows: positioned_rows,
        column_widths,
        total_width_pt: total_width,
        repeat_header_rows: table.repeat_header_rows as usize,
        border_width_pt: table_style.border_width_pt,
        border_color: table_style.border_color,
        space_before_pt: 8.0,
        space_after_pt: 8.0,
    }
}

/// Slices a table to fit into the remaining height on a page.
pub fn slice_table(
    table: &PositionedTable,
    start_row: usize,
    max_height_pt: f32,
    is_first_slice: bool,
    x_offset: f32,
    y_offset: f32,
) -> Option<TableSlice> {
    if start_row >= table.rows.len() {
        return None;
    }

    let mut current_y = y_offset;
    let mut slice_boxes = Vec::new();
    let mut footnote_refs = Vec::new();
    let mut seen_fn = HashSet::new();

    // Include caption if first slice
    if is_first_slice && table.caption_height_pt > 0.0 {
        for cb in &table.caption_boxes {
            let mut b = cb.clone();
            b.bounds.x += x_offset;
            b.bounds.y += current_y;
            slice_boxes.push(b);
        }
        current_y += table.caption_height_pt;
    }

    let table_top_y = current_y;
    let repeat_count = if is_first_slice {
        0
    } else {
        table.repeat_header_rows
    };

    // Emit repeated header rows if continuation page
    for r_idx in 0..repeat_count.min(table.rows.len()) {
        let r = &table.rows[r_idx];
        emit_row_boxes(
            r,
            table,
            x_offset,
            current_y,
            &mut slice_boxes,
            &mut footnote_refs,
            &mut seen_fn,
        );
        current_y += r.height_pt;
    }

    let mut next_row = start_row;
    while next_row < table.rows.len() {
        let r = &table.rows[next_row];
        if (current_y + r.height_pt - y_offset) > max_height_pt && next_row > start_row {
            // Cannot fit more rows on this page slice
            break;
        }

        emit_row_boxes(
            r,
            table,
            x_offset,
            current_y,
            &mut slice_boxes,
            &mut footnote_refs,
            &mut seen_fn,
        );
        current_y += r.height_pt;
        next_row += 1;
    }

    // If no progress could be made, return None so paginator advances to a new page
    if next_row == start_row {
        return None;
    }

    let total_slice_height = current_y - y_offset;
    let is_complete = next_row >= table.rows.len();

    // Draw outer border if border width > 0
    if table.border_width_pt > 0.0 {
        let border_box = LayoutBox {
            id: format!("{}:border_{}_{}", table.block_id, start_row, next_row),
            bounds: Rect::new(
                x_offset,
                table_top_y,
                table.total_width_pt,
                current_y - table_top_y,
            ),
            kind: LayoutBoxKind::BorderRect {
                width: table.border_width_pt,
                color: table.border_color,
            },
        };
        slice_boxes.push(border_box);
    }

    Some(TableSlice {
        start_row,
        next_row,
        is_complete,
        total_height_pt: total_slice_height,
        boxes: slice_boxes,
        footnote_refs,
    })
}

fn emit_row_boxes(
    row: &PositionedTableRow,
    table: &PositionedTable,
    x_offset: f32,
    row_y: f32,
    out_boxes: &mut Vec<LayoutBox>,
    out_footnotes: &mut Vec<String>,
    seen_fn: &mut HashSet<String>,
) {
    for cell in &row.cells {
        let cell_rect = Rect::new(
            x_offset + cell.bounds.x,
            row_y,
            cell.bounds.width,
            row.height_pt,
        );

        // Background box
        if let Some(bg_color) = cell.background_color {
            out_boxes.push(LayoutBox {
                id: format!("{}:bg", cell.cell_id),
                bounds: cell_rect,
                kind: LayoutBoxKind::SolidRect { color: bg_color },
            });
        }

        // Cell border box
        if table.border_width_pt > 0.0 {
            out_boxes.push(LayoutBox {
                id: format!("{}:cell_border", cell.cell_id),
                bounds: cell_rect,
                kind: LayoutBoxKind::BorderRect {
                    width: table.border_width_pt,
                    color: table.border_color,
                },
            });
        }

        // Child content boxes
        for cb in &cell.child_boxes {
            let mut positioned_child = cb.clone();
            positioned_child.bounds.x += cell_rect.x;
            positioned_child.bounds.y += cell_rect.y;
            out_boxes.push(positioned_child);
        }

        // Footnotes
        for fn_id in &cell.footnote_refs {
            if seen_fn.insert(fn_id.clone()) {
                out_footnotes.push(fn_id.clone());
            }
        }
    }

    for fn_id in &row.footnote_refs {
        if seen_fn.insert(fn_id.clone()) {
            out_footnotes.push(fn_id.clone());
        }
    }
}

fn layout_cell_blocks(
    blocks: &[Block],
    cell_width: f32,
    cell_padding: f32,
    ctx: &TypesettingContext,
) -> (Vec<LayoutBox>, f32, Vec<String>) {
    let inner_width = (cell_width - cell_padding * 2.0).max(10.0);
    let mut current_y = cell_padding;
    let mut cell_boxes = Vec::new();
    let mut cell_fn_refs = Vec::new();

    let cell_ctx = TypesettingContext {
        stylesheet: ctx.stylesheet,
        font_registry: ctx.font_registry,
        available_width_pt: inner_width,
        use_knuth_plass: ctx.use_knuth_plass,
    };

    for block in blocks {
        match block {
            Block::Paragraph(p) => {
                let pos = layout_paragraph(p, &cell_ctx);
                current_y += pos.space_before_pt;
                let bxs = pos.to_layout_boxes(cell_padding, current_y);
                cell_boxes.extend(bxs);
                cell_fn_refs.extend(pos.footnote_refs);
                current_y += pos.total_height_pt + pos.space_after_pt;
            }
            Block::Heading(h) => {
                let pos = layout_heading(h, &cell_ctx);
                current_y += pos.space_before_pt;
                let bxs = pos.to_layout_boxes(cell_padding, current_y);
                cell_boxes.extend(bxs);
                cell_fn_refs.extend(pos.footnote_refs);
                current_y += pos.total_height_pt + pos.space_after_pt;
            }
            Block::List(l) => {
                let pos_list = layout_list(l, &cell_ctx, 0);
                for item in pos_list {
                    current_y += item.space_before_pt;
                    let bxs = item.to_layout_boxes(cell_padding, current_y);
                    cell_boxes.extend(bxs);
                    cell_fn_refs.extend(item.footnote_refs);
                    current_y += item.total_height_pt + item.space_after_pt;
                }
            }
            _ => {}
        }
    }

    let total_cell_height = current_y + cell_padding;
    (cell_boxes, total_cell_height, cell_fn_refs)
}

fn compute_column_count(table: &TableBlock) -> usize {
    let mut max_cols = 1;
    for row in &table.rows {
        let mut row_cols = 0;
        for cell in &row.cells {
            row_cols += (cell.col_span as usize).max(1);
        }
        if row_cols > max_cols {
            max_cols = row_cols;
        }
    }
    max_cols
}

fn compute_column_widths(table: &TableBlock, num_cols: usize, available_width: f32) -> Vec<f32> {
    if !table.column_widths_pt.is_empty() && table.column_widths_pt.len() == num_cols {
        return table.column_widths_pt.clone();
    }

    if num_cols == 0 {
        return vec![available_width];
    }

    let even_width = available_width / (num_cols as f32);
    vec![even_width; num_cols]
}

fn resolve_table_style(
    style_id: Option<&str>,
    stylesheet: &vocument_flow::styles::Stylesheet,
) -> TableStyle {
    if let Some(id) = style_id {
        if let Some(s) = stylesheet.table_styles.get(id) {
            return s.clone();
        }
    }
    if let Some(default_tbl) = stylesheet.table_styles.get("table_normal") {
        return default_tbl.clone();
    }
    TableStyle {
        id: "default_table".to_string(),
        name: "Default Table".to_string(),
        border_width_pt: 0.5,
        border_color: ColorRgba::rgb(200, 200, 200),
        cell_padding_pt: 4.0,
        header_background_color: Some(ColorRgba::rgb(240, 240, 240)),
        alternating_row_color: None,
    }
}
