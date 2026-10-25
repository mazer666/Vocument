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

    // 3. Section presence
    if doc.sections.is_empty() {
        return Err(FlowError::EmptyDocument);
    }

    // 4. Identifier uniqueness and structural traversal
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
            if h.spans.is_empty() {
                return Err(FlowError::EmptyHeading {
                    block_id: h.id.clone(),
                });
            }
            validate_spans(&h.id, &h.spans, doc)?;
        }
        Block::List(list) => {
            validate_list(list, seen_ids, doc)?;
        }
        Block::Table(table) => {
            validate_table(table, seen_ids, doc)?;
        }
        Block::Image(img) => {
            if img.asset_ref.trim().is_empty() {
                return Err(FlowError::InvalidMetadata {
                    field: format!("image[{}].asset_ref", img.id),
                    details: "image asset reference cannot be empty".to_string(),
                });
            }
            if img.intrinsic_width_pt <= 0.0 || img.intrinsic_height_pt <= 0.0 {
                return Err(FlowError::InvalidMetadata {
                    field: format!("image[{}].intrinsic_dimensions", img.id),
                    details: "intrinsic dimensions must be strictly positive".to_string(),
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

fn validate_table(
    table: &TableBlock,
    seen_ids: &mut HashSet<String>,
    doc: &FlowDocument,
) -> Result<(), FlowError> {
    if table.rows.is_empty() {
        return Err(FlowError::EmptyTable {
            table_id: table.id.clone(),
        });
    }

    // Expected column count is defined by the column span sum of the first row
    let expected_cols: usize = table.rows[0]
        .cells
        .iter()
        .map(|c| c.col_span as usize)
        .sum();

    if expected_cols == 0 {
        return Err(FlowError::InvalidTableDimensions {
            table_id: table.id.clone(),
            row_index: 0,
            expected_cols: 1,
            actual_cols: 0,
        });
    }

    for (row_idx, row) in table.rows.iter().enumerate() {
        let row_cols: usize = row.cells.iter().map(|c| c.col_span as usize).sum();
        if row_cols != expected_cols {
            return Err(FlowError::InvalidTableDimensions {
                table_id: table.id.clone(),
                row_index: row_idx,
                expected_cols,
                actual_cols: row_cols,
            });
        }

        for cell in &row.cells {
            check_id(&cell.id, "table_cell", seen_ids)?;
            if cell.blocks.is_empty() {
                return Err(FlowError::EmptyParagraph {
                    block_id: cell.id.clone(),
                });
            }
            for cell_block in &cell.blocks {
                validate_block(cell_block, seen_ids, doc)?;
            }
        }
    }

    Ok(())
}

fn validate_spans(block_id: &str, spans: &[TextSpan], doc: &FlowDocument) -> Result<(), FlowError> {
    for span in spans {
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
