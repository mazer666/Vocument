//! Integration, security, roundtrip, and headless LibreOffice interoperability tests for
//! `vocument-office` (`OFFICE-001`–`005`, `M4.5`).

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use vocument_flow::metadata::{Author, DocumentMetadata};
use vocument_flow::model::{
    Block, FlowDocument, FootnoteDefinition, HeadingBlock, ImageBlock, ListBlock, ListItem,
    ListKind, PageOrientation, ParagraphBlock, Section, SectionGeometry, TableBlock, TableCell,
    TableRow,
};
use vocument_flow::review::{ChangeType, FlowComment, FlowCommentReply, TrackedChange};
use vocument_flow::schema::validate_document;
use vocument_flow::styles::{Alignment, ColorRgba, TextSpan};
use vocument_office::{
    AssetMap, ImportedAsset, OfficeError, OfficeFormat, OfficeLimits, Severity, detect_format,
    evaluate_fidelity, export_docx, export_odt, import_docx, import_odt, package::PackageWriter,
};

// Valid 1x1 RGB PNG (with valid zlib stream and CRC32 checksums for libpng/LibreOffice).
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

fn build_rich_legal_document() -> (FlowDocument, AssetMap) {
    let mut meta = DocumentMetadata::new("doc_legal_m4_5", "Master Services Agreement §14");
    meta.subtitle = Some("Cross-Border Data Processing Addendum".to_string());
    meta.description =
        Some("Bilingual German/English legal contract with tables and review".to_string());
    meta.language = "de-DE".to_string();
    meta.authors = vec![
        Author::new("Dr. Hannah Müller"),
        Author::new("Jean-Luc Picard"),
    ];
    meta.keywords = vec![
        "GDPR".to_string(),
        "AGPLv3".to_string(),
        "Vertrag".to_string(),
    ];
    meta.custom_attributes
        .insert("MatterNumber".to_string(), "DE-2026-9941".to_string());

    let asset_ref = vocument_office::util::content_addressed_path("png", TINY_PNG);
    let mut assets = AssetMap::new();
    assets.insert(
        asset_ref.clone(),
        ImportedAsset {
            media_type: "image/png".to_string(),
            bytes: TINY_PNG.to_vec(),
        },
    );

    let mut h1 = HeadingBlock::new(
        "h1",
        1,
        vec![TextSpan::plain("1. Vertragsgegenstand & Geltungsbereich")],
    );
    h1.extra.insert(
        "office.page_break_before".to_string(),
        serde_json::Value::Bool(true),
    );

    let h2 = HeadingBlock::new("h2", 2, vec![TextSpan::plain("1.1 Begriffsbestimmungen")]);

    let mut bold_span = TextSpan::bold("Wichtiger Hinweis:");
    bold_span.color = Some(ColorRgba::rgb(0xAA, 0x00, 0x00));
    let mut italic_span = TextSpan::italic(" Vertrauliche Unterlagen");
    italic_span.background_color = Some(ColorRgba::rgb(0xFF, 0xFF, 0x00));
    let mut link_span =
        TextSpan::plain(" https://example.org/agb").with_link("https://example.org/agb");
    link_span.style.underline = true;
    let fn_ref_span = TextSpan::plain("").with_footnote("fn1");
    let mut ins_span = TextSpan::plain(" (ergänzte Klausel)");
    ins_span.style.underline = true;
    ins_span.tracked_change_id = Some("chg_ins_1".to_string());
    let mut del_span = TextSpan::plain(" (gestrichene Altregelung)");
    del_span.style.strikethrough = true;
    del_span.tracked_change_id = Some("chg_del_1".to_string());

    let mut p1 = ParagraphBlock::new(
        "p_intro",
        vec![
            bold_span,
            italic_span,
            link_span,
            fn_ref_span,
            ins_span,
            del_span,
        ],
    );
    p1.alignment = Some(Alignment::Justified);
    p1.space_before_pt = Some(6.0);
    p1.space_after_pt = Some(12.0);
    p1.extra.insert(
        "office.docx.pPr_unmapped".to_string(),
        serde_json::json!([r#"<w:SuppressAutoHyphens xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#]),
    );

    // Nested ordered list
    let nested_list = ListBlock::new(
        "list_sub",
        ListKind::Ordered,
        vec![
            ListItem::new("li_sub_1", vec![TextSpan::plain("Unterpunkt Alpha")]),
            ListItem::new("li_sub_2", vec![TextSpan::plain("Unterpunkt Beta")]),
        ],
    );
    let mut item_1 = ListItem::new(
        "li_1",
        vec![TextSpan::plain("Hauptpflicht des Auftragnehmers")],
    );
    item_1.nested_list = Some(Box::new(nested_list));
    let item_2 = ListItem::new(
        "li_2",
        vec![TextSpan::plain("Mitwirkungspflicht des Auftraggebers")],
    );
    let list_block = ListBlock::new("list_main", ListKind::Ordered, vec![item_1, item_2]);

    // Table with header row, col_span, and row_span (vMerge)
    let hdr_row = TableRow::header(vec![
        TableCell::new(
            "c_h1",
            vec![Block::Paragraph(ParagraphBlock::new(
                "p_ch1",
                vec![TextSpan::bold("Kategorie")],
            ))],
        ),
        TableCell::new(
            "c_h2",
            vec![Block::Paragraph(ParagraphBlock::new(
                "p_ch2",
                vec![TextSpan::bold("Details & SLA")],
            ))],
        )
        .with_col_span(2),
    ]);
    let body_row_1 = TableRow::new(vec![
        TableCell::new(
            "c_r1_1",
            vec![Block::Paragraph(ParagraphBlock::new(
                "p_r1_1",
                vec![TextSpan::plain("Verfügbarkeit")],
            ))],
        )
        .with_row_span(2)
        .with_background_color(ColorRgba::rgb(0xEE, 0xF4, 0xFA)),
        TableCell::new(
            "c_r1_2",
            vec![Block::Paragraph(ParagraphBlock::new(
                "p_r1_2",
                vec![TextSpan::plain("Kernzeit")],
            ))],
        ),
        TableCell::new(
            "c_r1_3",
            vec![Block::Paragraph(ParagraphBlock::new(
                "p_r1_3",
                vec![TextSpan::plain("99.95%")],
            ))],
        ),
    ]);
    let body_row_2 = TableRow::new(vec![
        TableCell::new(
            "c_r2_2",
            vec![Block::Paragraph(ParagraphBlock::new(
                "p_r2_2",
                vec![TextSpan::plain("Nebenzeit")],
            ))],
        ),
        TableCell::new(
            "c_r2_3",
            vec![Block::Paragraph(ParagraphBlock::new(
                "p_r2_3",
                vec![TextSpan::plain("99.50%")],
            ))],
        ),
    ]);
    let table_block = TableBlock::new("tbl_sla", vec![hdr_row, body_row_1, body_row_2])
        .with_caption("Tabelle 1: Service-Level-Kennzahlen")
        .with_column_widths(vec![140.0, 160.0, 140.0])
        .with_repeat_header_rows(1);

    let img_block = ImageBlock {
        id: "img_seal".to_string(),
        asset_ref,
        alt_text: "Firmenstempel".to_string(),
        caption: None,
        intrinsic_width_pt: 96.0,
        intrinsic_height_pt: 48.0,
        display_width_pt: Some(96.0),
        display_height_pt: Some(48.0),
        extra: BTreeMap::new(),
    };

    let mut geom = SectionGeometry::a4_portrait();
    geom.orientation = PageOrientation::Portrait;
    let mut section = Section::new(
        "sec_main",
        geom,
        vec![
            Block::Heading(h1),
            Block::Heading(h2),
            Block::Paragraph(p1),
            Block::List(list_block),
            Block::Table(table_block),
            Block::Image(img_block),
        ],
    );
    section.header_blocks = vec![Block::Paragraph(ParagraphBlock::new(
        "p_hdr",
        vec![TextSpan::plain("Vertraulich — MSA §14")],
    ))];
    section.footer_blocks = vec![Block::Paragraph(ParagraphBlock::new(
        "p_ftr",
        vec![
            TextSpan::plain("Seite "),
            TextSpan::plain("{page}"),
            TextSpan::plain(" von "),
            TextSpan::plain("{total_pages}"),
        ],
    ))];

    let mut doc = FlowDocument::new(meta).with_section(section);
    doc.footnotes.insert(
        "fn1".to_string(),
        FootnoteDefinition::new(
            "fn1",
            1,
            vec![Block::Paragraph(ParagraphBlock::new(
                "p_fn1",
                vec![TextSpan::plain("Siehe Anhang B zur Auftragsverarbeitung.")],
            ))],
        ),
    );

    let mut comment = FlowComment::new(
        "c1",
        "Dr. Hannah Müller",
        "2026-10-30T10:15:00Z",
        "Bitte Haftungshöchstgrenze mit dem Vorstand abstimmen.",
        Some("p_intro".to_string()),
        Some((0, 18)),
        Some("Wichtiger Hinweis:".to_string()),
    );
    comment.resolved = true;
    comment.replies.push(FlowCommentReply::new(
        "r1",
        "Jean-Luc Picard",
        "2026-10-30T11:00:00Z",
        "Freigabe durch Rechtsabteilung liegt vor.",
    ));
    doc.review.comments.push(comment);

    let mut tc_ins = TrackedChange::new(
        "chg_ins_1",
        ChangeType::Insert,
        "Dr. Hannah Müller",
        "2026-10-30T09:00:00Z",
    );
    tc_ins.target_block_id = Some("p_intro".to_string());
    tc_ins.text_snippet = Some(" (ergänzte Klausel)".to_string());

    let mut tc_del = TrackedChange::new(
        "chg_del_1",
        ChangeType::Delete,
        "Jean-Luc Picard",
        "2026-10-30T09:30:00Z",
    );
    tc_del.target_block_id = Some("p_intro".to_string());
    tc_del.text_snippet = Some(" (gestrichene Altregelung)".to_string());

    doc.review.tracked_changes = vec![tc_ins, tc_del];
    validate_document(&doc).expect("seed document must be valid");
    (doc, assets)
}

#[test]
fn docx_roundtrip_preserves_tier1_tier2_tier3_and_review() {
    let (source_doc, source_assets) = build_rich_legal_document();
    let exported = export_docx(&source_doc, &source_assets).expect("export DOCX");
    assert_eq!(
        detect_format(&exported.bytes, &OfficeLimits::default()),
        Some(OfficeFormat::Docx)
    );

    let imported = import_docx(&exported.bytes).expect("re-import DOCX");
    validate_document(&imported.document).expect("imported DOCX must validate");

    let eval = evaluate_fidelity(
        &source_doc,
        &imported.document,
        &imported.report,
        &exported.report,
    );
    assert!(
        eval.passes_m4_5_gate(),
        "DOCX roundtrip must pass M4.5 fidelity gate, got: {eval:#?}"
    );
    assert_eq!(eval.tier1.plain_text_similarity, 1.0);
    assert_eq!(eval.tier1.heading_preservation, 1.0);
    assert_eq!(eval.tier1.character_style_preservation, 1.0);
    assert_eq!(eval.tier1.footnote_preservation, 1.0);
    assert_eq!(eval.tier2.table_cell_preservation, 1.0);
    assert_eq!(eval.tier2.nested_list_preservation, 1.0);
    assert_eq!(eval.tier2.image_dimension_preservation, 1.0);
    assert!(
        eval.tier3.passthrough_entries_roundtrip >= 2,
        "Tier 3 unmapped XML and page_break_before must roundtrip"
    );
    assert_eq!(imported.document.review.comments.len(), 1);
    assert!(imported.document.review.comments[0].resolved);
    assert_eq!(imported.document.review.comments[0].replies.len(), 1);
    assert_eq!(imported.document.review.tracked_changes.len(), 2);
}

#[test]
fn odt_roundtrip_preserves_tier1_tier2_and_review() {
    let (source_doc, source_assets) = build_rich_legal_document();
    let exported = export_odt(&source_doc, &source_assets).expect("export ODT");
    assert_eq!(
        detect_format(&exported.bytes, &OfficeLimits::default()),
        Some(OfficeFormat::Odt)
    );

    // Verify ODF packaging requirement: `mimetype` is the first entry and stored uncompressed.
    let reader = std::io::Cursor::new(&exported.bytes);
    let mut zip = zip::ZipArchive::new(reader).expect("open ODT zip");
    let first_entry = zip.by_index(0).expect("first zip entry");
    assert_eq!(first_entry.name(), "mimetype");
    assert_eq!(first_entry.compression(), zip::CompressionMethod::Stored);
    drop(first_entry);

    let imported = import_odt(&exported.bytes).expect("re-import ODT");
    validate_document(&imported.document).expect("imported ODT must validate");

    let eval = evaluate_fidelity(
        &source_doc,
        &imported.document,
        &imported.report,
        &exported.report,
    );
    assert!(
        eval.passes_m4_5_gate(),
        "ODT roundtrip must pass M4.5 fidelity gate, got: {eval:#?}"
    );
    assert_eq!(eval.tier1.plain_text_similarity, 1.0);
    assert_eq!(eval.tier1.heading_preservation, 1.0);
    assert_eq!(eval.tier1.character_style_preservation, 1.0);
    assert_eq!(eval.tier1.footnote_preservation, 1.0);
    assert_eq!(eval.tier2.table_cell_preservation, 1.0);
    assert_eq!(eval.tier2.nested_list_preservation, 1.0);
    assert_eq!(eval.tier2.image_dimension_preservation, 1.0);
    assert_eq!(imported.document.review.comments.len(), 1);
    assert!(imported.document.review.comments[0].resolved);
    assert_eq!(imported.document.review.comments[0].replies.len(), 1);
    assert_eq!(imported.document.review.tracked_changes.len(), 2);
}

#[test]
fn cross_format_docx_to_odt_to_docx_roundtrip_passes_gate() {
    let (source_doc, source_assets) = build_rich_legal_document();
    let docx1 = export_docx(&source_doc, &source_assets).expect("export DOCX 1");
    let from_docx = import_docx(&docx1.bytes).expect("import DOCX 1");
    let odt1 = export_odt(&from_docx.document, &from_docx.assets).expect("export ODT");
    let from_odt = import_odt(&odt1.bytes).expect("import ODT");
    let docx2 = export_docx(&from_odt.document, &from_odt.assets).expect("export DOCX 2");
    let final_doc = import_docx(&docx2.bytes).expect("import DOCX 2");

    let eval = evaluate_fidelity(
        &source_doc,
        &final_doc.document,
        &final_doc.report,
        &docx2.report,
    );
    assert!(
        eval.passes_m4_5_gate(),
        "DOCX -> ODT -> DOCX cross-format roundtrip must pass M4.5 gate: {eval:#?}"
    );
}

#[test]
fn security_neutralizes_macros_external_links_and_hostile_archives() {
    // 1. DOCX with VBA macro, external image relationship, and javascript: hyperlink
    let mut pkg = PackageWriter::new();
    pkg.add(
        "_rels/.rels",
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    )
    .expect("add rels");
    pkg.add("word/vbaProject.bin", b"FAKE_VBA_MACRO_PAYLOAD")
        .expect("add vba");
    pkg.add(
        "word/_rels/document.xml.rels",
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdExtImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="http://evil.example/beacon.png" TargetMode="External"/>
  <Relationship Id="rIdBadLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="javascript:alert(1)" TargetMode="External"/>
</Relationships>"#,
    )
    .expect("add doc rels");
    pkg.add(
        "word/document.xml",
        br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <w:body>
    <w:p>
      <w:hyperlink r:id="rIdBadLink"><w:r><w:t>Click me</w:t></w:r></w:hyperlink>
      <w:r><w:drawing><a:blip r:embed="rIdExtImg"/></w:drawing></w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .expect("add document.xml");
    let hostile_docx = pkg.finish().expect("finish hostile docx");

    let imported = import_docx(&hostile_docx).expect("import hostile docx safely");
    assert!(
        imported.report.count_at_least(Severity::Security) >= 3,
        "Expected macro, external rel, and unsafe link security findings: {:?}",
        imported.report.entries
    );
    // Verify the javascript: link was stripped from the span
    let Block::Paragraph(p) = &imported.document.sections[0].blocks[0] else {
        panic!("expected paragraph");
    };
    assert!(p.spans[0].link_url.is_none());

    // 2. Encrypted ODT (manifest:encryption-data) must be refused cleanly
    let mut enc_odt = PackageWriter::new();
    enc_odt
        .add_stored("mimetype", b"application/vnd.oasis.opendocument.text")
        .expect("mimetype");
    enc_odt
        .add(
            "META-INF/manifest.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0">
  <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml">
    <manifest:encryption-data/>
  </manifest:file-entry>
</manifest:manifest>"#,
        )
        .expect("manifest");
    enc_odt
        .add(
            "content.xml",
            br#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"/>"#,
        )
        .expect("content");
    let enc_bytes = enc_odt.finish().expect("finish enc odt");
    assert!(matches!(
        import_odt(&enc_bytes),
        Err(OfficeError::EncryptedEntry { .. })
    ));
}

#[test]
fn libreoffice_headless_cross_conversion_verification() {
    let soffice = Path::new("/Applications/LibreOffice.app/Contents/MacOS/soffice");
    if !soffice.exists() {
        eprintln!(
            "SKIPPED: LibreOffice not installed at {}",
            soffice.display()
        );
        return;
    }

    let (source_doc, source_assets) = build_rich_legal_document();
    let tmp_dir = std::env::temp_dir().join(format!("vocument_lo_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).expect("create temp dir");

    // 1. Export DOCX from Vocument -> convert to ODT with LibreOffice -> import ODT in Vocument
    let docx_path = tmp_dir.join("vocument_out.docx");
    let exported_docx = export_docx(&source_doc, &source_assets).expect("export docx");
    std::fs::write(&docx_path, &exported_docx.bytes).expect("write docx");

    let status = Command::new(soffice)
        .args([
            "--headless",
            "--convert-to",
            "odt",
            "--outdir",
            tmp_dir.to_str().expect("utf8 path"),
            docx_path.to_str().expect("utf8 path"),
        ])
        .status()
        .expect("run soffice docx->odt");
    assert!(
        status.success(),
        "LibreOffice failed to convert Vocument DOCX to ODT"
    );

    let lo_odt_bytes = std::fs::read(tmp_dir.join("vocument_out.odt")).expect("read lo odt");
    let lo_odt_imported = import_odt(&lo_odt_bytes).expect("import LibreOffice-generated ODT");
    validate_document(&lo_odt_imported.document).expect("validate LO ODT");
    let eval_docx_via_lo = evaluate_fidelity(
        &source_doc,
        &lo_odt_imported.document,
        &lo_odt_imported.report,
        &exported_docx.report,
    );
    assert!(
        eval_docx_via_lo.tier1.plain_text_similarity >= 0.95,
        "Text similarity after LibreOffice DOCX->ODT conversion too low: {:?}",
        eval_docx_via_lo
    );

    // 2. Export ODT from Vocument -> convert to DOCX with LibreOffice -> import DOCX in Vocument
    let odt_path = tmp_dir.join("vocument_out2.odt");
    let exported_odt = export_odt(&source_doc, &source_assets).expect("export odt");
    std::fs::write(&odt_path, &exported_odt.bytes).expect("write odt");

    let status2 = Command::new(soffice)
        .args([
            "--headless",
            "--convert-to",
            "docx",
            "--outdir",
            tmp_dir.to_str().expect("utf8 path"),
            odt_path.to_str().expect("utf8 path"),
        ])
        .status()
        .expect("run soffice odt->docx");
    assert!(
        status2.success(),
        "LibreOffice failed to convert Vocument ODT to DOCX"
    );

    let lo_docx_bytes = std::fs::read(tmp_dir.join("vocument_out2.docx")).expect("read lo docx");
    let lo_docx_imported = import_docx(&lo_docx_bytes).expect("import LibreOffice-generated DOCX");
    validate_document(&lo_docx_imported.document).expect("validate LO DOCX");
    let eval_odt_via_lo = evaluate_fidelity(
        &source_doc,
        &lo_docx_imported.document,
        &lo_docx_imported.report,
        &exported_odt.report,
    );
    assert!(
        eval_odt_via_lo.tier1.plain_text_similarity >= 0.95,
        "Text similarity after LibreOffice ODT->DOCX conversion too low: {:?}",
        eval_odt_via_lo
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
