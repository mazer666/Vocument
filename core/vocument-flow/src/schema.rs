//! Schema validation rules, invariants, and structural integrity checks (`ADR-0020`, `AUTHOR-001`–`005`).

use std::collections::HashSet;

use crate::error::FlowError;
use crate::model::{Block, FlowDocument, ListBlock, Section, TableBlock};
use crate::styles::TextSpan;

/// Current supported schema version for FlowDocument containers.
pub const FLOW_SCHEMA_VERSION: &str = "1.0";

/// Minimum supported heading level.
pub const MIN_HEADING_LEVEL: u8 = 1;

/// Maximum supported heading level.
pub const MAX_HEADING_LEVEL: u8 = 6;

/// Comprehensive schema validator enforcing document-level invariants.
pub fn validate_document(doc: &FlowDocument) -> Result<(), FlowError> {
    // 1. Schema version verification
    if doc.schema_version != FLOW_SCHEMA_VERSION {
        return Err(FlowError::UnsupportedSchemaVersion {
            version: doc.schema_version.clone(),
            expected: FLOW_SCHEMA_VERSION.to_string(),
        });
    }

    // 2. Metadata verification
    doc.metadata.validate()?;

    // 3. Stylesheet verification
    doc.stylesheet.validate()?;

    // 4. Section presence
    if doc.sections.is_empty() {
        return Err(FlowError::EmptyDocument);
    }

    // 5. Identifier uniqueness and structural traversal
    let mut seen_ids = HashSet::new();

    // Verify footnote definitions
    for (fn_id, footnote) in &doc.footnotes {
        check_id(&footnote.id, "footnote", &mut seen_ids)?;
        if footnote.id != *fn_id {
            return Err(FlowError::InvalidMetadata {
                field: format!("footnotes[{fn_id}]"),
                details: format!("key '{fn_id}' does not match footnote id '{}'", footnote.id),
            });
        }
        if footnote.blocks.is_empty() {
            return Err(FlowError::EmptyFootnote {
                footnote_id: footnote.id.clone(),
            });
        }
        for block in &footnote.blocks {
            validate_block(block, &mut seen_ids, doc)?;
        }
    }

    // Verify sections and their blocks
    for section in &doc.sections {
        validate_section(section, &mut seen_ids, doc)?;
    }

    Ok(())
}

fn check_id(id: &str, element_type: &str, seen_ids: &mut HashSet<String>) -> Result<(), FlowError> {
    if id.trim().is_empty() {
        return Err(FlowError::EmptyId {
            element_type: element_type.to_string(),
            id: id.to_string(),
        });
    }
    if !seen_ids.insert(id.to_string()) {
        return Err(FlowError::DuplicateId { id: id.to_string() });
    }
    Ok(())
}

fn validate_section(
    section: &Section,
    seen_ids: &mut HashSet<String>,
    doc: &FlowDocument,
) -> Result<(), FlowError> {
    check_id(&section.id, "section", seen_ids)?;

    // Geometrical boundaries and margins verification
    section.geometry.validate(&section.id)?;

    if section.blocks.is_empty() {
        return Err(FlowError::EmptySection {
            section_id: section.id.clone(),
        });
    }

    for block in &section.header_blocks {
        validate_block(block, seen_ids, doc)?;
    }
    for block in &section.blocks {
        validate_block(block, seen_ids, doc)?;
    }
    for block in &section.footer_blocks {
        validate_block(block, seen_ids, doc)?;
    }

    Ok(())
}

fn validate_block(
    block: &Block,
    seen_ids: &mut HashSet<String>,
    doc: &FlowDocument,
) -> Result<(), FlowError> {
    check_id(block.id(), "block", seen_ids)?;

    match block {
        Block::Paragraph(p) => {
            if let Some(style_id) = &p.style_id
                && !doc.stylesheet.paragraph_styles.contains_key(style_id)
            {
                return Err(FlowError::UnknownStyle {
                    style_type: "paragraph".to_string(),
                    style_id: style_id.clone(),
                    block_id: p.id.clone(),
                });
            }
            if let Some(before) = p.space_before_pt
                && (!before.is_finite() || before < 0.0)
            {
                return Err(FlowError::InvalidTypography {
                    context: format!("paragraph '{}'", p.id),
                    details: format!("space_before_pt must be finite and >= 0, got {}", before),
                });
            }
            if let Some(after) = p.space_after_pt
                && (!after.is_finite() || after < 0.0)
            {
                return Err(FlowError::InvalidTypography {
                    context: format!("paragraph '{}'", p.id),
                    details: format!("space_after_pt must be finite and >= 0, got {}", after),
                });
            }
            if p.spans.is_empty() {
                return Err(FlowError::EmptyParagraph {
                    block_id: p.id.clone(),
                });
            }
            validate_spans(&p.id, &p.spans, doc)?;
        }
        Block::Heading(h) => {
            if h.level < MIN_HEADING_LEVEL || h.level > MAX_HEADING_LEVEL {
                return Err(FlowError::InvalidHeadingLevel {
                    block_id: h.id.clone(),
                    level: h.level,
                });
            }
            if let Some(style_id) = &h.style_id
                && !doc.stylesheet.paragraph_styles.contains_key(style_id)
            {
                return Err(FlowError::UnknownStyle {
                    style_type: "paragraph".to_string(),
                    style_id: style_id.clone(),
                    block_id: h.id.clone(),
                });
            }
            if h.spans.is_empty() {
                return Err(FlowError::EmptyHeading {
                    block_id: h.id.clone(),
                });
            }
            validate_spans(&h.id, &h.spans, doc)?;
        }
        Block::List(list) => {
            if let Some(style_id) = &list.style_id
                && !doc.stylesheet.list_styles.contains_key(style_id)
            {
                return Err(FlowError::UnknownStyle {
                    style_type: "list".to_string(),
                    style_id: style_id.clone(),
                    block_id: list.id.clone(),
                });
            }
            validate_list(list, seen_ids, doc)?;
        }
        Block::Table(table) => {
            if let Some(style_id) = &table.style_id
                && !doc.stylesheet.table_styles.contains_key(style_id)
            {
                return Err(FlowError::UnknownStyle {
                    style_type: "table".to_string(),
                    style_id: style_id.clone(),
                    block_id: table.id.clone(),
                });
            }
            validate_table(table, seen_ids, doc)?;
        }
        Block::Image(img) => {
            if img.asset_ref.trim().is_empty() {
                return Err(FlowError::InvalidMetadata {
                    field: format!("image[{}].asset_ref", img.id),
                    details: "image asset reference cannot be empty".to_string(),
                });
            }
            if !img.intrinsic_width_pt.is_finite()
                || img.intrinsic_width_pt <= 0.0
                || !img.intrinsic_height_pt.is_finite()
                || img.intrinsic_height_pt <= 0.0
            {
                return Err(FlowError::InvalidImageDimensions {
                    image_id: img.id.clone(),
                    details: "intrinsic dimensions must be finite and > 0".to_string(),
                });
            }
            if let Some(disp_w) = img.display_width_pt
                && (!disp_w.is_finite() || disp_w <= 0.0)
            {
                return Err(FlowError::InvalidImageDimensions {
                    image_id: img.id.clone(),
                    details: format!("display_width_pt must be finite and > 0, got {}", disp_w),
                });
            }
            if let Some(disp_h) = img.display_height_pt
                && (!disp_h.is_finite() || disp_h <= 0.0)
            {
                return Err(FlowError::InvalidImageDimensions {
                    image_id: img.id.clone(),
                    details: format!("display_height_pt must be finite and > 0, got {}", disp_h),
                });
            }
        }
        Block::Callout(c) => {
            if c.blocks.is_empty() {
                return Err(FlowError::EmptyCallout {
                    block_id: c.id.clone(),
                });
            }
            for child in &c.blocks {
                validate_block(child, seen_ids, doc)?;
            }
        }
        Block::ThematicBreak(_) => {}
    }

    Ok(())
}

fn validate_list(
    list: &ListBlock,
    seen_ids: &mut HashSet<String>,
    doc: &FlowDocument,
) -> Result<(), FlowError> {
    if list.items.is_empty() {
        return Err(FlowError::EmptyList {
            block_id: list.id.clone(),
        });
    }

    for item in &list.items {
        check_id(&item.id, "list_item", seen_ids)?;
        if item.spans.is_empty() {
            return Err(FlowError::EmptyParagraph {
                block_id: item.id.clone(),
            });
        }
        validate_spans(&item.id, &item.spans, doc)?;

        if let Some(nested) = &item.nested_list {
            check_id(&nested.id, "nested_list", seen_ids)?;
            validate_list(nested, seen_ids, doc)?;
        }
    }

    Ok(())
}

#[allow(clippy::needless_range_loop)]
fn validate_table(
    table: &TableBlock,
    seen_ids: &mut HashSet<String>,
    doc: &FlowDocument,
) -> Result<(), FlowError> {
    let num_rows = table.rows.len();
    if num_rows == 0 {
        return Err(FlowError::EmptyTable {
            table_id: table.id.clone(),
        });
    }

    // 2D grid matrix mapping row_idx -> col_idx -> Option<cell_id>
    let mut grid: Vec<Vec<Option<String>>> = vec![Vec::new(); num_rows];

    for (row_idx, row) in table.rows.iter().enumerate() {
        let mut col_cursor = 0;

        for cell in &row.cells {
            check_id(&cell.id, "table_cell", seen_ids)?;

            if cell.col_span == 0 || cell.row_span == 0 {
                return Err(FlowError::InvalidCellSpan {
                    table_id: table.id.clone(),
                    cell_id: cell.id.clone(),
                    row_span: cell.row_span,
                    col_span: cell.col_span,
                });
            }

            let row_span = cell.row_span as usize;
            let col_span = cell.col_span as usize;

            if row_idx + row_span > num_rows {
                return Err(FlowError::RowSpanOutOfBounds {
                    table_id: table.id.clone(),
                    cell_id: cell.id.clone(),
                    row_index: row_idx,
                    row_span: cell.row_span,
                    total_rows: num_rows,
                });
            }

            // Find next unallocated column index in the current row
            while col_cursor < grid[row_idx].len() && grid[row_idx][col_cursor].is_some() {
                col_cursor += 1;
            }

            // Allocate span slots across row_span x col_span
            let target_col_end = col_cursor + col_span;
            for r in row_idx..(row_idx + row_span) {
                while grid[r].len() < target_col_end {
                    grid[r].push(None);
                }
                for c in col_cursor..target_col_end {
                    if let Some(existing_cell) = &grid[r][c] {
                        return Err(FlowError::OverlappingTableCells {
                            table_id: table.id.clone(),
                            cell_id: format!("'{}' overlaps with '{}'", cell.id, existing_cell),
                            row_index: r,
                            col_index: c,
                        });
                    }
                    grid[r][c] = Some(cell.id.clone());
                }
            }

            col_cursor = target_col_end;

            // Validate cell block contents
            if cell.blocks.is_empty() {
                return Err(FlowError::EmptyTableCell {
                    cell_id: cell.id.clone(),
                });
            }
            for cell_block in &cell.blocks {
                validate_block(cell_block, seen_ids, doc)?;
            }
        }
    }

    // Compute expected total columns
    let expected_cols = grid.iter().map(|r| r.len()).max().unwrap_or(0);
    if expected_cols == 0 {
        return Err(FlowError::InvalidTableDimensions {
            table_id: table.id.clone(),
            row_index: 0,
            expected_cols: 1,
            actual_cols: 0,
        });
    }

    // Verify grid completeness and rectangular consistency
    for (r, row_slots) in grid.iter().enumerate() {
        if row_slots.len() != expected_cols {
            return Err(FlowError::InvalidTableDimensions {
                table_id: table.id.clone(),
                row_index: r,
                expected_cols,
                actual_cols: row_slots.len(),
            });
        }
        for (c, slot) in row_slots.iter().enumerate() {
            if slot.is_none() {
                return Err(FlowError::InvalidTableGrid {
                    table_id: table.id.clone(),
                    details: format!("unoccupied grid slot at row {}, column {}", r, c),
                });
            }
        }
    }

    // Column widths check
    if !table.column_widths_pt.is_empty() {
        if table.column_widths_pt.len() != expected_cols {
            return Err(FlowError::InvalidTableGrid {
                table_id: table.id.clone(),
                details: format!(
                    "column_widths_pt count ({}) does not match table columns ({})",
                    table.column_widths_pt.len(),
                    expected_cols
                ),
            });
        }
        for (col_idx, &w) in table.column_widths_pt.iter().enumerate() {
            if !w.is_finite() || w <= 0.0 {
                return Err(FlowError::InvalidTableGrid {
                    table_id: table.id.clone(),
                    details: format!(
                        "column width at index {} must be finite and > 0, got {}",
                        col_idx, w
                    ),
                });
            }
        }
    }

    // Repeat header rows check
    if table.repeat_header_rows > 0 && table.repeat_header_rows as usize > num_rows {
        return Err(FlowError::InvalidTableGrid {
            table_id: table.id.clone(),
            details: format!(
                "repeat_header_rows ({}) exceeds total rows ({})",
                table.repeat_header_rows, num_rows
            ),
        });
    }

    Ok(())
}

fn validate_spans(block_id: &str, spans: &[TextSpan], doc: &FlowDocument) -> Result<(), FlowError> {
    for span in spans {
        span.validate(block_id)?;

        if let Some(ref_id) = &span.footnote_ref
            && !doc.footnotes.contains_key(ref_id)
        {
            return Err(FlowError::InvalidFootnoteReference {
                block_id: block_id.to_string(),
                ref_id: ref_id.clone(),
            });
        }
    }
    Ok(())
}
