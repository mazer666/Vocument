//! Traversal helpers over the `FlowDocument` block tree.

use vocument_flow::model::{Block, FlowDocument};

/// Visits every block, including blocks nested in table cells, callouts and footnotes, in
/// document order: header, body, footer of each section, then the footnote definitions.
pub fn visit_blocks<'a>(document: &'a FlowDocument, visitor: &mut impl FnMut(&'a Block)) {
    for section in &document.sections {
        for block in section
            .header_blocks
            .iter()
            .chain(&section.blocks)
            .chain(&section.footer_blocks)
        {
            visit_block(block, visitor);
        }
    }
    for footnote in document.footnotes.values() {
        for block in &footnote.blocks {
            visit_block(block, visitor);
        }
    }
}

/// Visits a block and everything nested below it.
pub fn visit_block<'a>(block: &'a Block, visitor: &mut impl FnMut(&'a Block)) {
    visitor(block);
    match block {
        Block::Table(table) => {
            for row in &table.rows {
                for cell in &row.cells {
                    for nested in &cell.blocks {
                        visit_block(nested, visitor);
                    }
                }
            }
        }
        Block::Callout(callout) => {
            for nested in &callout.blocks {
                visit_block(nested, visitor);
            }
        }
        Block::Paragraph(_)
        | Block::Heading(_)
        | Block::List(_)
        | Block::Image(_)
        | Block::ThematicBreak(_) => {}
    }
}

/// Mutable traversal for post-processing imported content.
pub fn visit_blocks_mut(blocks: &mut [Block], visitor: &mut impl FnMut(&mut Block)) {
    for block in blocks {
        visitor(block);
        match block {
            Block::Table(table) => {
                for row in &mut table.rows {
                    for cell in &mut row.cells {
                        visit_blocks_mut(&mut cell.blocks, visitor);
                    }
                }
            }
            Block::Callout(callout) => visit_blocks_mut(&mut callout.blocks, visitor),
            _ => {}
        }
    }
}
