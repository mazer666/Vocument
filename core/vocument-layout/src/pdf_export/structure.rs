//! Logical structure tree (`/StructTreeRoot`) construction for Tagged PDF and PDF/UA-1 export
//! (`ADR-0020`, `ROADMAP.md` M4.4).
//!
//! Every text line of the layout carries the identifier of the `FlowDocument` block it came from.
//! [`StructureRoles`] resolves that identifier back to the semantic path of the block
//! (for example `Table > TR > TD > P`), and [`StructureTree`] merges the paths of consecutive
//! lines into one tree so that a multi-line paragraph becomes a single `/P` element.

use std::collections::HashMap;

use vocument_flow::model::{Block, FlowDocument, ListBlock, TableBlock};

/// Suffix the layout engine appends to a block identifier to name one of its text lines.
const LINE_ID_SEPARATOR: &str = ":line_";

/// One step of a semantic path: the standard structure type and a key that is equal for all lines
/// that belong to the same structure element.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct StructureStep {
    pub(super) structure_type: &'static str,
    pub(super) key: String,
    /// Optional `/A` table attribute entries such as `/Scope /Column`.
    pub(super) table_attributes: Option<String>,
}

impl StructureStep {
    fn new(structure_type: &'static str, key: String) -> Self {
        Self {
            structure_type,
            key,
            table_attributes: None,
        }
    }
}

/// Maps `FlowDocument` block identifiers to their semantic structure path.
#[derive(Debug, Default)]
pub(super) struct StructureRoles {
    paths_by_block_id: HashMap<String, Vec<StructureStep>>,
}

impl StructureRoles {
    /// Walks the whole document once and records the path of every text-bearing block.
    pub(super) fn from_document(doc: &FlowDocument) -> Self {
        let mut roles = Self::default();
        for section in &doc.sections {
            for block in &section.blocks {
                roles.record_block(block, &[]);
            }
        }
        roles
    }

    /// Semantic path for a layout box identifier. Unknown boxes (footnotes, captions, callout
    /// titles) fall back to a paragraph of their own so no text is left untagged.
    pub(super) fn path_for_box_id(&self, box_id: &str) -> Vec<StructureStep> {
        let block_id = box_id
            .rsplit_once(LINE_ID_SEPARATOR)
            .map_or(box_id, |(block_id, _)| block_id);
        self.paths_by_block_id
            .get(block_id)
            .cloned()
            .unwrap_or_else(|| vec![StructureStep::new("P", format!("P:{block_id}"))])
    }

    fn record_block(&mut self, block: &Block, parent_path: &[StructureStep]) {
        match block {
            Block::Paragraph(paragraph) => {
                let step = StructureStep::new("P", format!("P:{}", paragraph.id));
                self.record_leaf(&paragraph.id, parent_path, step);
            }
            Block::Heading(heading) => {
                let structure_type = heading_structure_type(heading.level);
                let step = StructureStep::new(structure_type, format!("H:{}", heading.id));
                self.record_leaf(&heading.id, parent_path, step);
            }
            Block::List(list) => self.record_list(list, parent_path),
            Block::Table(table) => self.record_table(table, parent_path),
            Block::Callout(callout) => {
                let mut path = parent_path.to_vec();
                path.push(StructureStep::new("Sect", format!("Sect:{}", callout.id)));
                self.paths_by_block_id
                    .insert(format!("{}:title", callout.id), {
                        let mut title_path = path.clone();
                        title_path.push(StructureStep::new("P", format!("P:{}:title", callout.id)));
                        title_path
                    });
                for nested in &callout.blocks {
                    self.record_block(nested, &path);
                }
            }
            Block::Image(_) | Block::ThematicBreak(_) => {}
        }
    }

    fn record_leaf(&mut self, block_id: &str, parent_path: &[StructureStep], step: StructureStep) {
        let mut path = parent_path.to_vec();
        path.push(step);
        self.paths_by_block_id.insert(block_id.to_string(), path);
    }

    fn record_list(&mut self, list: &ListBlock, parent_path: &[StructureStep]) {
        let mut list_path = parent_path.to_vec();
        list_path.push(StructureStep::new("L", format!("L:{}", list.id)));
        for item in &list.items {
            let mut item_path = list_path.clone();
            item_path.push(StructureStep::new("LI", format!("LI:{}", item.id)));
            item_path.push(StructureStep::new("LBody", format!("LBody:{}", item.id)));
            self.paths_by_block_id
                .insert(item.id.clone(), item_path.clone());
            if let Some(nested_list) = &item.nested_list {
                // Nested lists live inside the list item, next to its body.
                let item_only_path = &item_path[..item_path.len() - 1];
                self.record_list(nested_list, item_only_path);
            }
        }
    }

    fn record_table(&mut self, table: &TableBlock, parent_path: &[StructureStep]) {
        let mut table_path = parent_path.to_vec();
        table_path.push(StructureStep::new("Table", format!("Table:{}", table.id)));
        if table.caption.is_some() {
            let caption_id = format!("{}:caption", table.id);
            let mut caption_path = table_path.clone();
            caption_path.push(StructureStep::new(
                "Caption",
                format!("Caption:{}", table.id),
            ));
            caption_path.push(StructureStep::new("P", format!("P:{caption_id}")));
            self.paths_by_block_id.insert(caption_id, caption_path);
        }
        for (row_index, row) in table.rows.iter().enumerate() {
            let mut row_path = table_path.clone();
            row_path.push(StructureStep::new(
                "TR",
                format!("TR:{}:{}", table.id, row_index),
            ));
            for cell in &row.cells {
                let cell_type = if row.is_header { "TH" } else { "TD" };
                let mut cell_step =
                    StructureStep::new(cell_type, format!("{cell_type}:{}", cell.id));
                cell_step.table_attributes = Some(table_cell_attributes(
                    row.is_header,
                    cell.col_span,
                    cell.row_span,
                ));
                let mut cell_path = row_path.clone();
                cell_path.push(cell_step);
                for nested in &cell.blocks {
                    self.record_block(nested, &cell_path);
                }
            }
        }
    }
}

fn heading_structure_type(level: u8) -> &'static str {
    match level {
        0 | 1 => "H1",
        2 => "H2",
        3 => "H3",
        4 => "H4",
        5 => "H5",
        _ => "H6",
    }
}

/// `/A` attribute dictionary of a table cell: header scope and spans.
fn table_cell_attributes(is_header: bool, column_span: u32, row_span: u32) -> String {
    let mut attributes = String::from("<< /O /Table");
    if is_header {
        attributes.push_str(" /Scope /Column");
    }
    if column_span > 1 {
        attributes.push_str(&format!(" /ColSpan {column_span}"));
    }
    if row_span > 1 {
        attributes.push_str(&format!(" /RowSpan {row_span}"));
    }
    attributes.push_str(" >>");
    attributes
}

/// Content that hangs below a structure element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StructureChild {
    /// Child structure element, by index into [`StructureTree::nodes`].
    Element(usize),
    /// Marked-content sequence on a page.
    MarkedContent { page_index: usize, mcid: u32 },
    /// Annotation object that the element describes.
    Annotation {
        annotation_object_id: usize,
        page_index: usize,
    },
}

/// One structure element of the logical structure tree.
#[derive(Debug)]
pub(super) struct StructureNode {
    pub(super) structure_type: &'static str,
    pub(super) table_attributes: Option<String>,
    pub(super) parent: Option<usize>,
    pub(super) children: Vec<StructureChild>,
}

/// Logical structure tree under a single `/Document` root element (node `0`).
#[derive(Debug)]
pub(super) struct StructureTree {
    pub(super) nodes: Vec<StructureNode>,
    child_lookup: HashMap<(usize, String), usize>,
}

impl StructureTree {
    pub(super) const ROOT_INDEX: usize = 0;

    pub(super) fn new() -> Self {
        Self {
            nodes: vec![StructureNode {
                structure_type: "Document",
                table_attributes: None,
                parent: None,
                children: Vec::new(),
            }],
            child_lookup: HashMap::new(),
        }
    }

    /// Adds a marked-content sequence below the element described by `path`, creating or reusing
    /// the elements along the way. Returns the index of the element that owns the content.
    pub(super) fn add_marked_content(
        &mut self,
        path: &[StructureStep],
        page_index: usize,
        mcid: u32,
    ) -> usize {
        let owner = self.resolve_path(path);
        self.nodes[owner]
            .children
            .push(StructureChild::MarkedContent { page_index, mcid });
        owner
    }

    /// Adds an `/Annot` element below the document root that owns one annotation object.
    pub(super) fn add_annotation(
        &mut self,
        annotation_object_id: usize,
        page_index: usize,
    ) -> usize {
        let index = self.nodes.len();
        self.nodes.push(StructureNode {
            structure_type: "Annot",
            table_attributes: None,
            parent: Some(Self::ROOT_INDEX),
            children: vec![StructureChild::Annotation {
                annotation_object_id,
                page_index,
            }],
        });
        self.nodes[Self::ROOT_INDEX]
            .children
            .push(StructureChild::Element(index));
        index
    }

    fn resolve_path(&mut self, path: &[StructureStep]) -> usize {
        let mut current = Self::ROOT_INDEX;
        for step in path {
            let lookup_key = (current, step.key.clone());
            current = match self.child_lookup.get(&lookup_key) {
                Some(existing) => *existing,
                None => {
                    let index = self.nodes.len();
                    self.nodes.push(StructureNode {
                        structure_type: step.structure_type,
                        table_attributes: step.table_attributes.clone(),
                        parent: Some(current),
                        children: Vec::new(),
                    });
                    self.nodes[current]
                        .children
                        .push(StructureChild::Element(index));
                    self.child_lookup.insert(lookup_key, index);
                    index
                }
            };
        }
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(structure_type: &'static str, key: &str) -> StructureStep {
        StructureStep::new(structure_type, key.to_string())
    }

    #[test]
    fn lines_of_one_paragraph_share_one_element() {
        let mut tree = StructureTree::new();
        let path = vec![step("P", "P:a")];
        let first = tree.add_marked_content(&path, 0, 0);
        let second = tree.add_marked_content(&path, 0, 1);
        assert_eq!(first, second);
        assert_eq!(tree.nodes[first].children.len(), 2);
        assert_eq!(tree.nodes.len(), 2, "document root plus one paragraph");
    }

    #[test]
    fn table_path_nests_row_and_cell() {
        let mut tree = StructureTree::new();
        let path = vec![
            step("Table", "Table:t"),
            step("TR", "TR:t:0"),
            step("TD", "TD:c"),
            step("P", "P:cp"),
        ];
        let owner = tree.add_marked_content(&path, 0, 0);
        let mut chain = vec![tree.nodes[owner].structure_type];
        let mut cursor = tree.nodes[owner].parent;
        while let Some(index) = cursor {
            chain.push(tree.nodes[index].structure_type);
            cursor = tree.nodes[index].parent;
        }
        assert_eq!(chain, ["P", "TD", "TR", "Table", "Document"]);
    }

    #[test]
    fn heading_levels_are_clamped_to_h1_through_h6() {
        assert_eq!(heading_structure_type(0), "H1");
        assert_eq!(heading_structure_type(4), "H4");
        assert_eq!(heading_structure_type(9), "H6");
    }
}
