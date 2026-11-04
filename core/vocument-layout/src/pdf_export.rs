//! Tagged PDF, PDF/A-2b and PDF/UA-1 export engine (`ADR-0020`, `ROADMAP.md` M4.4).
//!
//! Generates PDF 1.7 documents that target ISO 19005-2 (PDF/A-2b) and ISO 14289-1 (PDF/UA-1):
//! an embedded TrueType font with matching `/Widths`, an sRGB output intent, XMP metadata with the
//! identification schemas, a logical structure tree (`/StructTreeRoot`, `/ParentTree`) and comment
//! annotations that are tagged and carry appearance streams.
//!
//! Conformance is claimed only after an external validator (veraPDF) has accepted the output;
//! see `scripts/verify-pdfa-compliance.sh` and `PdfExportOptions::certified_by_external_tooling`.

mod font_embedding;
mod image_xobject;
mod page_content;
mod structure;
mod unicode_fallback;
mod xmp;

pub use image_xobject::{ImagePreparationError, MAX_IMAGE_DIMENSION_PX, MAX_IMAGE_PAYLOAD_BYTES};
pub use unicode_fallback::{FALLBACK_FONT_RESOURCE_NAME, UnicodeFallbackRegistry};

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as FmtWrite;
use std::io::Write as IoWrite;

use vocument_flow::model::{Block, FlowDocument};
use vocument_flow::review::FlowComment;

use crate::font::FontRegistry;
use crate::paginator::{PaginatorOptions, paginate_document};
use crate::tree::LayoutDocument;

use font_embedding::{EmbeddedFontMetrics, EmbeddedFontObjectIds, encode_pdf_text_string};
use page_content::{FONT_RESOURCE_NAME, RenderedPage, render_page};
use structure::{StructureChild, StructureRoles, StructureTree};
use unicode_fallback::FallbackFontObjectIds;

/// Side length of the square comment marker annotation in points.
const COMMENT_MARKER_SIZE_PT: f32 = 20.0;

/// Distance of the comment marker from the right page edge in points.
const COMMENT_MARKER_RIGHT_INSET_PT: f32 = 30.0;

/// Errors occurring during Tagged PDF, PDF/A-2b, and PDF/UA-1 export.
#[derive(Debug, thiserror::Error, PartialEq, Eq, Clone)]
pub enum PdfExportError {
    #[error("referenced image asset '{0}' was not found in export assets catalog")]
    MissingAsset(String),
    #[error("failed to process image asset '{asset_ref}': {error}")]
    InvalidImage {
        asset_ref: String,
        error: image_xobject::ImagePreparationError,
    },
    #[error("layout or export processing error: {0}")]
    Processing(String),
}

/// Configuration options for PDF/A and PDF/UA export.
#[derive(Debug, Clone)]
pub struct PdfExportOptions {
    /// When true, renders margin comments as PDF Text Annotations.
    pub include_comments: bool,
    /// When true, renders tracked insertions and deletions visually. When false, exports clean text.
    pub include_tracked_changes: bool,
    /// Embed PDF/A-2b metadata and conformance indicators.
    pub pdfa_conformance: bool,
    /// Embed PDF/UA-1 metadata, /MarkInfo, and semantic tags.
    pub pdfua_conformance: bool,
    /// Natural language code (e.g. "en-US", "de-DE").
    pub language: String,
    /// Declares whether this export has been independently certified by external tooling (veraPDF / PAC).
    /// Defaults to false (experimental draft profile; official certification pending).
    pub certified_by_external_tooling: bool,
    /// In-memory asset catalog for embedded raster images keyed by asset reference path (`M4.4`).
    pub assets: BTreeMap<String, Vec<u8>>,
    /// When false (default), missing or corrupt images return an explicit error.
    /// When true, allows placeholder images as lossy fallback if explicitly confirmed.
    pub allow_lossy_image_fallbacks: bool,
}

impl Default for PdfExportOptions {
    fn default() -> Self {
        Self {
            include_comments: true,
            include_tracked_changes: true,
            pdfa_conformance: true,
            pdfua_conformance: true,
            language: "de-DE".to_string(),
            certified_by_external_tooling: false,
            assets: BTreeMap::new(),
            allow_lossy_image_fallbacks: false,
        }
    }
}

impl PdfExportOptions {
    /// Adds a single image asset by reference path and raw byte contents.
    pub fn with_asset(mut self, asset_ref: impl Into<String>, bytes: Vec<u8>) -> Self {
        self.assets.insert(asset_ref.into(), bytes);
        self
    }

    /// Sets the asset catalog from a pre-populated map.
    pub fn with_assets(mut self, assets: BTreeMap<String, Vec<u8>>) -> Self {
        self.assets = assets;
        self
    }
}

/// Exports a `FlowDocument` to a PDF byte stream (Tagged PDF/A-2b and PDF/UA-1 profile).
pub fn export_flow_document_to_pdf(
    doc: &FlowDocument,
    font_registry: &FontRegistry,
    options: &PdfExportOptions,
) -> Result<Vec<u8>, PdfExportError> {
    let clean_doc;
    let effective_doc = if !options.include_tracked_changes {
        clean_doc = sanitize_clean_flow_document(doc);
        &clean_doc
    } else {
        doc
    };

    let layout = paginate_document(effective_doc, font_registry, &PaginatorOptions::default());
    export_layout_to_pdf(&layout, effective_doc, options)
}

/// Exports an existing `LayoutDocument` and its semantic `FlowDocument` to PDF bytes.
pub fn export_layout_to_pdf(
    layout: &LayoutDocument,
    doc: &FlowDocument,
    options: &PdfExportOptions,
) -> Result<Vec<u8>, PdfExportError> {
    let mut pdf = PdfWriter::new();
    let metrics = EmbeddedFontMetrics::load();

    let catalog_id = pdf.alloc_id();
    let pages_id = pdf.alloc_id();
    let struct_tree_id = pdf.alloc_id();
    let metadata_id = pdf.alloc_id();
    let output_intent_id = pdf.alloc_id();
    let icc_profile_id = pdf.alloc_id();
    let font_ids = EmbeddedFontObjectIds {
        font: pdf.alloc_id(),
        descriptor: pdf.alloc_id(),
        font_program: pdf.alloc_id(),
        to_unicode: pdf.alloc_id(),
    };

    let mut page_ids = Vec::with_capacity(layout.pages.len());
    let mut content_ids = Vec::with_capacity(layout.pages.len());
    for _ in &layout.pages {
        page_ids.push(pdf.alloc_id());
        content_ids.push(pdf.alloc_id());
    }

    pdf.write_embedded_font(&font_ids, &metrics);

    // Render every page first: that builds the structure tree the page dictionaries refer to.
    let roles = StructureRoles::from_document(doc);
    let mut tree = StructureTree::new();
    let mut fallback_registry = UnicodeFallbackRegistry::new();
    let rendered_pages: Vec<RenderedPage> = layout
        .pages
        .iter()
        .enumerate()
        .map(|(page_index, page)| {
            render_page(
                page,
                page_index,
                &roles,
                &mut tree,
                &metrics,
                &mut fallback_registry,
            )
        })
        .collect();
    for (page_index, rendered) in rendered_pages.iter().enumerate() {
        pdf.write_stream_object(content_ids[page_index], rendered.content_stream.as_bytes());
    }

    let fallback_font_id = if !fallback_registry.is_empty() {
        let fb_ids = FallbackFontObjectIds {
            font: pdf.alloc_id(),
            char_procs: pdf.alloc_id(),
            to_unicode: pdf.alloc_id(),
        };
        pdf.write_fallback_type3_font(&fb_ids, &fallback_registry);
        Some(fb_ids.font)
    } else {
        None
    };

    // Comment annotations. Each annotation is tagged through its own /Annot structure element and
    // reachable from the parent tree through its /StructParent key.
    let first_annotation_key = layout.pages.len();
    let mut annotation_struct_elements: Vec<usize> = Vec::new();
    let mut annotation_ids_per_page: Vec<Vec<usize>> = vec![Vec::new(); layout.pages.len()];
    if options.include_comments {
        let mut placed_comment_ids = HashSet::new();
        for (page_index, page) in layout.pages.iter().enumerate() {
            let mut markers_on_anchor_line: Vec<(String, u32)> = Vec::new();
            for comment in &doc.review.comments {
                let Some(anchor_block_id) = comment.anchor_block_id.as_deref() else {
                    continue;
                };
                if placed_comment_ids.contains(&comment.comment_id) {
                    continue;
                }
                let Some(anchor) = rendered_pages[page_index]
                    .line_anchors
                    .iter()
                    .find(|anchor| anchor.block_id == anchor_block_id)
                else {
                    continue;
                };
                placed_comment_ids.insert(comment.comment_id.clone());

                let stacked_markers = markers_on_anchor_line
                    .iter()
                    .filter(|(block_id, _)| block_id == anchor_block_id)
                    .count() as u32;
                markers_on_anchor_line.push((anchor_block_id.to_string(), stacked_markers));

                let lower_left_x = page.size.width
                    - COMMENT_MARKER_RIGHT_INSET_PT
                    - stacked_markers as f32 * (COMMENT_MARKER_SIZE_PT + 4.0);
                let lower_left_y = (anchor.pdf_top_y - COMMENT_MARKER_SIZE_PT).max(0.0);
                let annotation_id = pdf.alloc_id();
                let appearance_id = pdf.alloc_id();
                let structure_key = first_annotation_key + annotation_struct_elements.len();
                pdf.write_comment_annotation(
                    annotation_id,
                    appearance_id,
                    page_ids[page_index],
                    structure_key,
                    comment,
                    (lower_left_x, lower_left_y),
                );
                annotation_ids_per_page[page_index].push(annotation_id);
                annotation_struct_elements.push(tree.add_annotation(annotation_id, page_index));
            }
        }
    }

    // Prepare and write all raster image XObjects referenced on pages.
    let mut image_object_ids: HashMap<String, usize> = HashMap::new();
    let mut image_mappings_per_page: Vec<Vec<(String, usize)>> =
        vec![Vec::new(); layout.pages.len()];
    for (page_index, rendered) in rendered_pages.iter().enumerate() {
        for placement in &rendered.images {
            let img_id = match image_object_ids.get(&placement.asset_ref) {
                Some(existing_id) => *existing_id,
                None => {
                    let image_obj = match options.assets.get(&placement.asset_ref) {
                        Some(bytes) => match image_xobject::prepare_image(bytes) {
                            Ok(obj) => obj,
                            Err(err) => {
                                if options.allow_lossy_image_fallbacks {
                                    image_xobject::placeholder_image()
                                } else {
                                    return Err(PdfExportError::InvalidImage {
                                        asset_ref: placement.asset_ref.clone(),
                                        error: err,
                                    });
                                }
                            }
                        },
                        None => {
                            if options.allow_lossy_image_fallbacks {
                                image_xobject::placeholder_image()
                            } else {
                                return Err(PdfExportError::MissingAsset(
                                    placement.asset_ref.clone(),
                                ));
                            }
                        }
                    };
                    let allocated_id = pdf.alloc_id();
                    let smask_id = if let Some(ref smask_bytes) = image_obj.smask_bytes {
                        let sid = pdf.alloc_id();
                        pdf.write_smask_xobject(
                            sid,
                            image_obj.width,
                            image_obj.height,
                            smask_bytes,
                        );
                        Some(sid)
                    } else {
                        None
                    };
                    pdf.write_image_xobject(allocated_id, &image_obj, smask_id);
                    image_object_ids.insert(placement.asset_ref.clone(), allocated_id);
                    allocated_id
                }
            };
            image_mappings_per_page[page_index].push((placement.resource_name.clone(), img_id));
        }
    }

    for (page_index, page) in layout.pages.iter().enumerate() {
        pdf.write_page_object(
            page_ids[page_index],
            pages_id,
            content_ids[page_index],
            font_ids.font,
            fallback_font_id,
            &image_mappings_per_page[page_index],
            &annotation_ids_per_page[page_index],
            (page.size.width, page.size.height),
            (!rendered_pages[page_index].mcid_owners.is_empty()).then_some(page_index),
        );
    }
    pdf.write_pages_object(pages_id, &page_ids);

    // Structure tree: one object per element, then the root with its parent tree.
    let struct_element_ids: Vec<usize> = tree.nodes.iter().map(|_| pdf.alloc_id()).collect();
    pdf.write_structure_elements(&tree, &struct_element_ids, struct_tree_id, &page_ids);
    pdf.write_struct_tree_root(
        struct_tree_id,
        struct_element_ids[StructureTree::ROOT_INDEX],
        &rendered_pages,
        &annotation_struct_elements,
        &struct_element_ids,
    );

    pdf.write_xmp_metadata(metadata_id, &xmp::build_xmp_packet(doc, options));
    pdf.write_output_intent(output_intent_id, icc_profile_id);
    pdf.write_catalog(
        catalog_id,
        pages_id,
        metadata_id,
        struct_tree_id,
        output_intent_id,
        &options.language,
    );

    Ok(pdf.finish(catalog_id))
}

fn sanitize_clean_flow_document(doc: &FlowDocument) -> FlowDocument {
    let mut clean = doc.clone();
    for section in &mut clean.sections {
        for block in &mut section.blocks {
            if let Block::Paragraph(p) = block {
                p.spans.retain(|s| !s.style.strikethrough);
                for span in &mut p.spans {
                    span.tracked_change_id = None;
                    span.style.underline = false;
                }
            }
        }
    }
    clean.review.tracked_changes.clear();
    clean
}

/// Annotation text of a comment thread: the comment, its replies and its resolution state.
fn comment_thread_text(comment: &FlowComment) -> String {
    let mut text = comment.content.clone();
    for reply in &comment.replies {
        let _ = write!(text, "\n{}: {}", reply.author, reply.content);
    }
    if comment.resolved {
        text.push_str("\n[resolved]");
    }
    text
}

/// Deterministic 128-bit file identifier for the trailer `/ID` (required by ISO 19005-2, 6.1.3):
/// two FNV-1a 64-bit hashes with different offset bases over the file body. It only has to tell
/// files apart and be reproducible across platforms (`ADR-0020`); it is not a security feature.
fn file_identifier_hex(file_body: &[u8]) -> String {
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    const FNV_OFFSET_BASES: [u64; 2] = [0xCBF2_9CE4_8422_2325, 0x84222325_CBF29CE4];
    let mut identifier = String::with_capacity(32);
    for offset_base in FNV_OFFSET_BASES {
        let hash = file_body.iter().fold(offset_base, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
        });
        let _ = write!(identifier, "{hash:016X}");
    }
    identifier
}

struct PdfWriter {
    buffer: Vec<u8>,
    offsets: Vec<usize>,
    next_id: usize,
}

impl PdfWriter {
    fn new() -> Self {
        let mut w = Self {
            buffer: Vec::with_capacity(160 * 1024),
            offsets: vec![0], // Object 0 is dummy
            next_id: 1,
        };
        w.buffer.extend_from_slice(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n");
        w
    }

    fn alloc_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.offsets.push(0);
        id
    }

    fn begin_obj(&mut self, id: usize) {
        self.offsets[id] = self.buffer.len();
        let _ = writeln!(self.buffer, "{} 0 obj", id);
    }

    fn end_obj(&mut self) {
        self.buffer.extend_from_slice(b"endobj\n");
    }

    fn write_stream_object(&mut self, id: usize, data: &[u8]) {
        self.begin_obj(id);
        let _ = write!(self.buffer, "<< /Length {} >>\nstream\n", data.len());
        self.buffer.extend_from_slice(data);
        self.buffer.extend_from_slice(b"\nendstream\n");
        self.end_obj();
    }

    fn write_catalog(
        &mut self,
        id: usize,
        pages_id: usize,
        metadata_id: usize,
        struct_tree_id: usize,
        output_intent_id: usize,
        lang: &str,
    ) {
        self.begin_obj(id);
        let _ = writeln!(
            self.buffer,
            "<< /Type /Catalog /Pages {} 0 R /Metadata {} 0 R /StructTreeRoot {} 0 R /MarkInfo << /Marked true >> /ViewerPreferences << /DisplayDocTitle true >> /Lang ({}) /OutputIntents [ {} 0 R ] >>",
            pages_id, metadata_id, struct_tree_id, lang, output_intent_id
        );
        self.end_obj();
    }

    fn write_pages_object(&mut self, id: usize, page_ids: &[usize]) {
        self.begin_obj(id);
        let mut kids = String::new();
        for pid in page_ids {
            let _ = write!(kids, "{} 0 R ", pid);
        }
        let _ = writeln!(
            self.buffer,
            "<< /Type /Pages /Kids [ {}] /Count {} >>",
            kids,
            page_ids.len()
        );
        self.end_obj();
    }

    #[allow(clippy::too_many_arguments)]
    fn write_page_object(
        &mut self,
        id: usize,
        parent_id: usize,
        content_id: usize,
        font_id: usize,
        fallback_font_id: Option<usize>,
        image_mappings: &[(String, usize)],
        annot_ids: &[usize],
        (width, height): (f32, f32),
        parent_tree_key: Option<usize>,
    ) {
        self.begin_obj(id);
        let annots_entry = if annot_ids.is_empty() {
            String::new()
        } else {
            let mut s = String::from("/Annots [ ");
            for aid in annot_ids {
                let _ = write!(s, "{} 0 R ", aid);
            }
            s.push_str("] ");
            s
        };
        // Pages without marked content have no parent-tree entry and therefore no key.
        let structure_parents = parent_tree_key
            .map(|key| format!("/StructParents {key} "))
            .unwrap_or_default();
        let xobjects_entry = if !image_mappings.is_empty() {
            let mut x_str = String::from(" /XObject << ");
            for (res_name, img_id) in image_mappings {
                let _ = write!(x_str, "/{res_name} {img_id} 0 R ");
            }
            x_str.push_str(">> /ProcSet [ /PDF /Text /ImageC ]");
            x_str
        } else {
            String::new()
        };
        let font_dict_entry = if let Some(fb_id) = fallback_font_id {
            format!(
                "/{} {} 0 R /{} {} 0 R",
                FONT_RESOURCE_NAME, font_id, FALLBACK_FONT_RESOURCE_NAME, fb_id
            )
        } else {
            format!("/{} {} 0 R", FONT_RESOURCE_NAME, font_id)
        };
        let _ = writeln!(
            self.buffer,
            "<< /Type /Page /Parent {} 0 R /MediaBox [ 0 0 {:.2} {:.2} ] /Contents {} 0 R {}{}/Tabs /S /Resources << /Font << {} >>{} >> >>",
            parent_id,
            width,
            height,
            content_id,
            annots_entry,
            structure_parents,
            font_dict_entry,
            xobjects_entry
        );
        self.end_obj();
    }

    fn write_image_xobject(
        &mut self,
        id: usize,
        image: &image_xobject::PdfImageObject,
        smask_id: Option<usize>,
    ) {
        self.begin_obj(id);
        let smask_entry = smask_id
            .map(|sid| format!(" /SMask {sid} 0 R"))
            .unwrap_or_default();
        let _ = write!(
            self.buffer,
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace {} /BitsPerComponent 8 /Filter {}{} /Length {} >>\nstream\n",
            image.width,
            image.height,
            image.color_space,
            image.filter,
            smask_entry,
            image.stream_bytes.len()
        );
        self.buffer.extend_from_slice(&image.stream_bytes);
        self.buffer.extend_from_slice(b"\nendstream\n");
        self.end_obj();
    }

    fn write_smask_xobject(&mut self, id: usize, width: u32, height: u32, smask_bytes: &[u8]) {
        self.begin_obj(id);
        let _ = write!(
            self.buffer,
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
            width,
            height,
            smask_bytes.len()
        );
        self.buffer.extend_from_slice(smask_bytes);
        self.buffer.extend_from_slice(b"\nendstream\n");
        self.end_obj();
    }

    fn write_xmp_metadata(&mut self, id: usize, packet: &str) {
        self.begin_obj(id);
        let _ = write!(
            self.buffer,
            "<< /Type /Metadata /Subtype /XML /Length {} >>\nstream\n",
            packet.len()
        );
        self.buffer.extend_from_slice(packet.as_bytes());
        self.buffer.extend_from_slice(b"\nendstream\n");
        self.end_obj();
    }

    /// Writes a Text annotation with an appearance stream. PDF/A requires the print flag and an
    /// appearance dictionary; PDF/UA requires `/Contents` and a tagged `/Annot` element, which the
    /// `/StructParent` key links to.
    fn write_comment_annotation(
        &mut self,
        id: usize,
        appearance_id: usize,
        page_id: usize,
        structure_key: usize,
        comment: &FlowComment,
        (x, y): (f32, f32),
    ) {
        let size = COMMENT_MARKER_SIZE_PT;
        let appearance = format!(
            "1.000 0.820 0.200 rg\n0 0 {size:.2} {size:.2} re\nf\n0.300 0.250 0.000 RG\n1 w\n0.5 0.5 {:.2} {:.2} re\nS\n",
            size - 1.0,
            size - 1.0
        );
        self.begin_obj(appearance_id);
        let _ = write!(
            self.buffer,
            "<< /Type /XObject /Subtype /Form /BBox [ 0 0 {size:.2} {size:.2} ] /Length {} >>\nstream\n",
            appearance.len()
        );
        self.buffer.extend_from_slice(appearance.as_bytes());
        self.buffer.extend_from_slice(b"\nendstream\n");
        self.end_obj();

        self.begin_obj(id);
        let _ = writeln!(
            self.buffer,
            "<< /Type /Annot /Subtype /Text /Name /Comment /F 28 /Rect [ {:.2} {:.2} {:.2} {:.2} ] /Contents {} /T {} /P {} 0 R /StructParent {} /AP << /N {} 0 R >> >>",
            x,
            y,
            x + size,
            y + size,
            encode_pdf_text_string(&comment_thread_text(comment)),
            encode_pdf_text_string(&comment.author),
            page_id,
            structure_key,
            appearance_id
        );
        self.end_obj();
    }

    fn finish(mut self, catalog_id: usize) -> Vec<u8> {
        let startxref = self.buffer.len();
        self.buffer.extend_from_slice(b"xref\n");
        let count = self.next_id;
        let _ = writeln!(self.buffer, "0 {}", count);
        self.buffer.extend_from_slice(b"0000000000 65535 f \n");
        for i in 1..count {
            let offset = self.offsets[i];
            let _ = writeln!(self.buffer, "{:010} 00000 n ", offset);
        }
        let file_identifier = file_identifier_hex(&self.buffer);
        let _ = write!(
            self.buffer,
            "trailer\n<< /Size {} /Root {} 0 R /ID [ <{id}> <{id}> ] >>\nstartxref\n{}\n%%EOF\n",
            count,
            catalog_id,
            startxref,
            id = file_identifier
        );
        self.buffer
    }
}

// Structure tree serialisation lives next to the writer so it can use its private state.
impl PdfWriter {
    fn write_structure_elements(
        &mut self,
        tree: &StructureTree,
        element_ids: &[usize],
        struct_tree_id: usize,
        page_ids: &[usize],
    ) {
        for (index, node) in tree.nodes.iter().enumerate() {
            let parent_id = node
                .parent
                .map_or(struct_tree_id, |parent| element_ids[parent]);
            let mut children = String::new();
            for child in &node.children {
                match child {
                    StructureChild::Element(child_index) => {
                        let _ = write!(children, "{} 0 R ", element_ids[*child_index]);
                    }
                    StructureChild::MarkedContent { page_index, mcid } => {
                        let _ = write!(
                            children,
                            "<< /Type /MCR /Pg {} 0 R /MCID {} >> ",
                            page_ids[*page_index], mcid
                        );
                    }
                    StructureChild::Annotation {
                        annotation_object_id,
                        page_index,
                    } => {
                        let _ = write!(
                            children,
                            "<< /Type /OBJR /Obj {} 0 R /Pg {} 0 R >> ",
                            annotation_object_id, page_ids[*page_index]
                        );
                    }
                }
            }
            let attributes = node
                .table_attributes
                .as_ref()
                .map(|attributes| format!(" /A {attributes}"))
                .unwrap_or_default();
            let alt = node
                .alt_text
                .as_ref()
                .map(|alt| format!(" /Alt {}", encode_pdf_text_string(alt)))
                .unwrap_or_default();
            self.begin_obj(element_ids[index]);
            let _ = writeln!(
                self.buffer,
                "<< /Type /StructElem /S /{} /P {} 0 R /K [ {}]{}{} >>",
                node.structure_type, parent_id, children, attributes, alt
            );
            self.end_obj();
        }
    }

    fn write_struct_tree_root(
        &mut self,
        id: usize,
        document_element_id: usize,
        rendered_pages: &[RenderedPage],
        annotation_struct_elements: &[usize],
        element_ids: &[usize],
    ) {
        let mut parent_tree = String::new();
        for (page_index, rendered) in rendered_pages.iter().enumerate() {
            if rendered.mcid_owners.is_empty() {
                continue;
            }
            let owners: String = rendered
                .mcid_owners
                .iter()
                .map(|owner| format!("{} 0 R ", element_ids[*owner]))
                .collect();
            let _ = write!(parent_tree, "{page_index} [ {owners}] ");
        }
        let first_annotation_key = rendered_pages.len();
        for (offset, node_index) in annotation_struct_elements.iter().enumerate() {
            let _ = write!(
                parent_tree,
                "{} {} 0 R ",
                first_annotation_key + offset,
                element_ids[*node_index]
            );
        }
        self.begin_obj(id);
        let _ = writeln!(
            self.buffer,
            "<< /Type /StructTreeRoot /K [ {} 0 R ] /ParentTree << /Nums [ {}] >> /ParentTreeNextKey {} >>",
            document_element_id,
            parent_tree,
            first_annotation_key + annotation_struct_elements.len()
        );
        self.end_obj();
    }
}
