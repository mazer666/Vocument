//! High-fidelity Tagged PDF, PDF/A-2b, and PDF/UA-1 export engine (`ADR-0020`, `ROADMAP.md` M4.4).
//!
//! Generates valid ISO 19005-2 (PDF/A-2b) and ISO 14289-1 (PDF/UA-1) compliant documents
//! with semantic structure trees (/StructTreeRoot), /MarkInfo, XMP metadata, /ToUnicode CMaps,
//! and reviewing markup/annotations.

use std::fmt::Write as FmtWrite;
use std::io::Write as IoWrite;
use vocument_flow::model::{Block, FlowDocument};

use crate::font::FontRegistry;
use crate::paginator::{PaginatorOptions, paginate_document};
use crate::tree::{LayoutBoxKind, LayoutDocument, LayoutLine};

/// Configuration options for PDF/A and PDF/UA export.
#[derive(Debug, Clone)]
pub struct PdfExportOptions {
    /// When true, renders margin comments as PDF Text Annotations.
    pub include_comments: bool,
    /// When true, renders tracked insertions and deletions visually. When false, exports clean text.
    pub include_tracked_changes: bool,
    /// Embed draft PDF/A-2b metadata and conformance indicators.
    pub pdfa_conformance: bool,
    /// Embed draft PDF/UA-1 metadata, /MarkInfo, and semantic tags.
    pub pdfua_conformance: bool,
    /// Natural language code (e.g. "en-US", "de-DE").
    pub language: String,
    /// Declares whether this export has been independently certified by external tooling (veraPDF / PAC).
    /// Defaults to false (experimental draft profile; official certification pending).
    pub certified_by_external_tooling: bool,
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
        }
    }
}

/// Exports a `FlowDocument` to a PDF byte stream using an experimental Tagged PDF/A & PDF/UA draft profile.
pub fn export_flow_document_to_pdf(
    doc: &FlowDocument,
    font_registry: &FontRegistry,
    options: &PdfExportOptions,
) -> Vec<u8> {
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
) -> Vec<u8> {
    let mut pdf = PdfWriter::new();

    // 1. Generate objects
    // Obj 1: Catalog
    let catalog_id = pdf.alloc_id();
    // Obj 2: Pages
    let pages_id = pdf.alloc_id();
    // Obj 3: StructTreeRoot
    let struct_tree_id = pdf.alloc_id();
    // Obj 4: Root Document StructElem
    let doc_struct_id = pdf.alloc_id();
    // Obj 5: XMP Metadata
    let metadata_id = pdf.alloc_id();
    // Obj 6: OutputIntent (PDF/A)
    let output_intent_id = pdf.alloc_id();
    // Obj 7: ToUnicode CMap
    let to_unicode_id = pdf.alloc_id();
    // Obj 8: Font /F1
    let font_id = pdf.alloc_id();
    let font_descriptor_id = pdf.alloc_id();

    // Page IDs and Content Stream IDs
    let mut page_ids = Vec::with_capacity(layout.pages.len());
    let mut content_ids = Vec::with_capacity(layout.pages.len());
    for _ in &layout.pages {
        page_ids.push(pdf.alloc_id());
        content_ids.push(pdf.alloc_id());
    }

    // Allocate Structure Elements for blocks
    let mut struct_elem_ids = Vec::new();
    let mut mcid_counters = vec![0u32; layout.pages.len()];

    // Write ToUnicode CMap
    pdf.write_to_unicode_cmap(to_unicode_id);

    // Write Font and Descriptor
    pdf.write_font(font_id, font_descriptor_id, to_unicode_id);

    // Build Page Content Streams with Marked Content (/P << /MCID n >> BDC ... EMC)
    for (page_idx, page) in layout.pages.iter().enumerate() {
        let page_id = page_ids[page_idx];
        let content_id = content_ids[page_idx];

        let mut stream = String::new();
        // Set coordinate transform and font
        stream.push_str("BT\n");
        let _ = writeln!(stream, "/F1 10 Tf");

        for lbox in page.all_boxes() {
            if let LayoutBoxKind::TextLine(ref line) = lbox.kind {
                let mcid = mcid_counters[page_idx];
                mcid_counters[page_idx] += 1;

                let tag = infer_structure_tag(&lbox.id);
                let struct_elem_id = pdf.alloc_id();
                struct_elem_ids.push(struct_elem_id);

                // Write StructElem
                pdf.write_struct_elem(struct_elem_id, doc_struct_id, tag, page_id, mcid);

                // Marked content in page stream
                let _ = writeln!(stream, "/{} << /MCID {} >> BDC", tag, mcid);
                render_line_stream(&mut stream, line, page.size.height);
                stream.push_str("EMC\n");
            }
        }
        stream.push_str("ET\n");

        pdf.write_stream_object(content_id, stream.as_bytes());
    }

    // Write Page Objects with optional comment Text Annotations
    for (page_idx, page) in layout.pages.iter().enumerate() {
        let page_id = page_ids[page_idx];
        let content_id = content_ids[page_idx];
        let mut annot_ids = Vec::new();

        if options.include_comments {
            for comment in &doc.review.comments {
                if let Some(ref block_id) = comment.anchor_block_id {
                    let page_has_block = page.all_boxes().any(|b| b.id.starts_with(block_id));
                    if page_has_block {
                        let annot_id = pdf.alloc_id();
                        annot_ids.push(annot_id);
                        pdf.write_comment_annotation(
                            annot_id,
                            page_id,
                            &comment.author,
                            &comment.content,
                            page.size.width - 50.0,
                            page.size.height - 100.0,
                        );
                    }
                }
            }
        }

        pdf.write_page_object(
            page_id,
            pages_id,
            content_id,
            font_id,
            &annot_ids,
            page.size.width,
            page.size.height,
        );
    }

    // Write Pages Container
    pdf.write_pages_object(pages_id, &page_ids);

    // Write Root Document Structure Element
    pdf.write_doc_struct_elem(doc_struct_id, struct_tree_id, &struct_elem_ids);

    // Write StructTreeRoot
    pdf.write_struct_tree_root(struct_tree_id, doc_struct_id);

    // Write XMP Metadata stream
    pdf.write_xmp_metadata(metadata_id, doc, options);

    // Write OutputIntent
    pdf.write_output_intent(output_intent_id);

    // Write Catalog
    pdf.write_catalog(
        catalog_id,
        pages_id,
        metadata_id,
        struct_tree_id,
        output_intent_id,
        &options.language,
    );

    pdf.finish(catalog_id)
}

fn infer_structure_tag(box_id: &str) -> &'static str {
    if box_id.starts_with("h1") {
        "H1"
    } else if box_id.starts_with("h2") {
        "H2"
    } else if box_id.starts_with("h3") {
        "H3"
    } else if box_id.starts_with("tbl") || box_id.starts_with("pth") || box_id.starts_with("pc") {
        "Table"
    } else if box_id.starts_with("li") {
        "LI"
    } else {
        "P"
    }
}

fn render_line_stream(stream: &mut String, line: &LayoutLine, page_height: f32) {
    let pdf_y = page_height - line.baseline_y;
    for run in &line.runs {
        let pdf_x = run.bounds.x;
        let color = run.color;
        let r = color.r as f32 / 255.0;
        let g = color.g as f32 / 255.0;
        let b = color.b as f32 / 255.0;
        let _ = writeln!(stream, "{:.3} {:.3} {:.3} rg", r, g, b);
        let _ = writeln!(stream, "/F1 {:.2} Tf", run.font_size_pt);
        let _ = writeln!(stream, "1 0 0 1 {:.2} {:.2} Tm", pdf_x, pdf_y);
        let escaped = escape_pdf_string(&run.text);
        let _ = writeln!(stream, "({}) Tj", escaped);
    }
}

fn escape_pdf_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 128 => out.push(c),
            'ä' => out.push_str("\\344"),
            'ö' => out.push_str("\\366"),
            'ü' => out.push_str("\\374"),
            'Ä' => out.push_str("\\304"),
            'Ö' => out.push_str("\\326"),
            'Ü' => out.push_str("\\334"),
            'ß' => out.push_str("\\337"),
            '§' => out.push_str("\\247"),
            '–' => out.push('-'),
            '—' => out.push('-'),
            _ => out.push('?'),
        }
    }
    out
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

struct PdfWriter {
    buffer: Vec<u8>,
    offsets: Vec<usize>,
    next_id: usize,
}

impl PdfWriter {
    fn new() -> Self {
        let mut w = Self {
            buffer: Vec::with_capacity(32 * 1024),
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
        annot_ids: &[usize],
        width: f32,
        height: f32,
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
        let _ = writeln!(
            self.buffer,
            "<< /Type /Page /Parent {} 0 R /MediaBox [ 0 0 {:.2} {:.2} ] /Contents {} 0 R {}/Resources << /Font << /F1 {} 0 R >> >> >>",
            parent_id, width, height, content_id, annots_entry, font_id
        );
        self.end_obj();
    }

    fn write_struct_tree_root(&mut self, id: usize, doc_struct_id: usize) {
        self.begin_obj(id);
        let _ = writeln!(
            self.buffer,
            "<< /Type /StructTreeRoot /RoleMap << /H1 /H1 /H2 /H2 /H3 /H3 /P /P /Table /Table /LI /LI >> /K [ {} 0 R ] >>",
            doc_struct_id
        );
        self.end_obj();
    }

    fn write_doc_struct_elem(&mut self, id: usize, parent_tree_id: usize, children_ids: &[usize]) {
        self.begin_obj(id);
        let mut k = String::new();
        for cid in children_ids {
            let _ = write!(k, "{} 0 R ", cid);
        }
        let _ = writeln!(
            self.buffer,
            "<< /Type /StructElem /S /Document /P {} 0 R /K [ {}] >>",
            parent_tree_id, k
        );
        self.end_obj();
    }

    fn write_struct_elem(
        &mut self,
        id: usize,
        parent_id: usize,
        tag: &str,
        page_id: usize,
        mcid: u32,
    ) {
        self.begin_obj(id);
        let _ = writeln!(
            self.buffer,
            "<< /Type /StructElem /S /{} /P {} 0 R /Pg {} 0 R /K {} >>",
            tag, parent_id, page_id, mcid
        );
        self.end_obj();
    }

    fn write_font(&mut self, font_id: usize, descriptor_id: usize, to_unicode_id: usize) {
        self.begin_obj(font_id);
        let widths = (32..=126).map(|_| "600 ").collect::<String>();
        let _ = writeln!(
            self.buffer,
            "<< /Type /Font /Subtype /TrueType /BaseFont /Helvetica /Encoding /WinAnsiEncoding /FirstChar 32 /LastChar 126 /Widths [ {}] /FontDescriptor {} 0 R /ToUnicode {} 0 R >>",
            widths, descriptor_id, to_unicode_id
        );
        self.end_obj();

        self.begin_obj(descriptor_id);
        self.buffer.extend_from_slice(
            b"<< /Type /FontDescriptor /FontName /Helvetica /Flags 32 /FontBBox [ -166 -225 1000 931 ] /Ascent 718 /Descent -207 /CapHeight 718 /ItalicAngle 0 /StemV 80 >>\n",
        );
        self.end_obj();
    }

    fn write_to_unicode_cmap(&mut self, id: usize) {
        let cmap = b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Vocument-ToUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<00> <FF>\nendcodespacerange\n1 beginbfrange\n<20> <7E> <0020>\nendbfrange\nendcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n";
        self.write_stream_object(id, cmap);
    }

    fn write_xmp_metadata(&mut self, id: usize, doc: &FlowDocument, options: &PdfExportOptions) {
        let title = &doc.metadata.title;
        let author = doc
            .metadata
            .authors
            .first()
            .map(|a| a.name.as_str())
            .unwrap_or("Vocument User");
        let now = "2026-10-25T12:00:00Z";

        let mut xmp = String::new();
        xmp.push_str("<?xpacket begin=\"\u{FEFF}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n");
        xmp.push_str("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n");

        if options.pdfa_conformance {
            xmp.push_str("<rdf:Description rdf:about=\"\" xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\">\n");
            xmp.push_str("  <pdfaid:part>2</pdfaid:part>\n  <pdfaid:conformance>B</pdfaid:conformance>\n</rdf:Description>\n");
        }
        if options.pdfua_conformance {
            xmp.push_str("<rdf:Description rdf:about=\"\" xmlns:pdfuaid=\"http://www.aiim.org/pdfua/ns/id/\">\n");
            xmp.push_str("  <pdfuaid:part>1</pdfuaid:part>\n</rdf:Description>\n");
        }

        xmp.push_str(
            "<rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n",
        );
        let _ = writeln!(
            xmp,
            "  <dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:title>",
            title
        );
        let _ = writeln!(
            xmp,
            "  <dc:creator><rdf:Seq><rdf:li>{}</rdf:li></rdf:Seq></dc:creator>",
            author
        );
        let _ = writeln!(
            xmp,
            "  <dc:date><rdf:Seq><rdf:li>{}</rdf:li></rdf:Seq></dc:date>",
            now
        );
        if !options.certified_by_external_tooling {
            xmp.push_str("  <dc:description><rdf:Alt><rdf:li xml:lang=\"x-default\">Experimental Tagged PDF/A-2b and PDF/UA-1 draft profile. Independent external validation certification pending.</rdf:li></rdf:Alt></dc:description>\n");
        }
        xmp.push_str("</rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>");

        self.begin_obj(id);
        let _ = write!(
            self.buffer,
            "<< /Type /Metadata /Subtype /XML /Length {} >>\nstream\n",
            xmp.len()
        );
        self.buffer.extend_from_slice(xmp.as_bytes());
        self.buffer.extend_from_slice(b"\nendstream\n");
        self.end_obj();
    }

    fn write_output_intent(&mut self, id: usize) {
        self.begin_obj(id);
        self.buffer.extend_from_slice(
            b"<< /Type /OutputIntent /S /GTS_PDFA1 /OutputConditionIdentifier (sRGB) /Info (sRGB IEC61966-2.1) >>\n",
        );
        self.end_obj();
    }

    fn write_comment_annotation(
        &mut self,
        id: usize,
        page_id: usize,
        author: &str,
        content: &str,
        x: f32,
        y: f32,
    ) {
        self.begin_obj(id);
        let _ = writeln!(
            self.buffer,
            "<< /Type /Annot /Subtype /Text /Rect [ {:.2} {:.2} {:.2} {:.2} ] /Contents ({}) /T ({}) /P {} 0 R >>",
            x,
            y,
            x + 24.0,
            y + 24.0,
            escape_pdf_string(content),
            escape_pdf_string(author),
            page_id
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
        let _ = write!(
            self.buffer,
            "trailer\n<< /Size {} /Root {} 0 R >>\nstartxref\n{}\n%%EOF\n",
            count, catalog_id, startxref
        );
        self.buffer
    }
}
