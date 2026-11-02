//! Turns raw adapter output into a document that satisfies the `FlowDocument` invariants.
//!
//! Real-world files contain empty paragraphs, empty table cells and sections without body text.
//! The model forbids those shapes (`validate_document`), so they are normalised here — with an
//! empty text span or an empty paragraph, never by dropping content — and then validated.

use vocument_flow::model::{Block, FlowDocument, ParagraphBlock};
use vocument_flow::schema::validate_document;
use vocument_flow::styles::TextSpan;

use crate::error::OfficeError;
use crate::util::IdGenerator;
use crate::walk::visit_blocks_mut;

/// Normalises and validates an imported document.
pub fn finalize_document(
    document: &mut FlowDocument,
    ids: &mut IdGenerator,
) -> Result<(), OfficeError> {
    for section in &mut document.sections {
        ensure_non_empty(&mut section.blocks, ids);
        normalise(&mut section.blocks, ids);
        normalise(&mut section.header_blocks, ids);
        normalise(&mut section.footer_blocks, ids);
    }
    for footnote in document.footnotes.values_mut() {
        ensure_non_empty(&mut footnote.blocks, ids);
        normalise(&mut footnote.blocks, ids);
    }
    validate_document(document)?;
    Ok(())
}

fn ensure_non_empty(blocks: &mut Vec<Block>, ids: &mut IdGenerator) {
    if blocks.is_empty() {
        blocks.push(empty_paragraph(ids));
    }
}

fn empty_paragraph(ids: &mut IdGenerator) -> Block {
    Block::Paragraph(ParagraphBlock::new(
        ids.next("empty"),
        vec![TextSpan::plain("")],
    ))
}

fn normalise(blocks: &mut [Block], ids: &mut IdGenerator) {
    visit_blocks_mut(blocks, &mut |block| match block {
        Block::Paragraph(paragraph) if paragraph.spans.is_empty() => {
            paragraph.spans.push(TextSpan::plain(""));
        }
        Block::Heading(heading) if heading.spans.is_empty() => {
            heading.spans.push(TextSpan::plain(""));
        }
        Block::Table(table) => {
            for row in &mut table.rows {
                for cell in &mut row.cells {
                    ensure_non_empty(&mut cell.blocks, ids);
                }
            }
        }
        Block::Callout(callout) => ensure_non_empty(&mut callout.blocks, ids),
        Block::List(list) => {
            let mut stack: Vec<&mut vocument_flow::model::ListBlock> = vec![list];
            while let Some(current) = stack.pop() {
                for item in &mut current.items {
                    if item.spans.is_empty() {
                        item.spans.push(TextSpan::plain(""));
                    }
                    if let Some(nested) = item.nested_list.as_mut() {
                        stack.push(nested);
                    }
                }
            }
        }
        _ => {}
    });
}
