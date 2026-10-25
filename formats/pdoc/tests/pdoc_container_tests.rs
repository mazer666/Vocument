//! Comprehensive container and roundtrip tests for `.pdoc` package specification (`ADR-0020`, `QA-003`, `CR-0002`).

use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use vocument_core::config::PackageLimitsConfig;
use vocument_flow::metadata::DocumentMetadata;
use vocument_flow::model::{
    Block, CalloutBlock, FlowDocument, HeadingBlock, ParagraphBlock, Section, SectionGeometry,
    TableBlock, TableCell, TableRow,
};
use vocument_flow::styles::{Alignment, ColorRgba, Stylesheet, TextSpan};
use vocument_pdoc::container::{PdocContainer, PdocPackage};
use vocument_pdoc::error::PdocError;
use vocument_pdoc::review::{
    PdocChangeType, PdocFlowComment, PdocFlowCommentReply, PdocReviewData, PdocTrackedChange,
};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn test_limits() -> PackageLimitsConfig {
    PackageLimitsConfig {
        max_pdoc_archive_size_mb: 10,
        max_zip_compression_ratio: 50,
        max_zip_entries: 100,
    }
}

fn create_sample_flow_document() -> FlowDocument {
    let mut meta = DocumentMetadata::new("doc_legal_001", "Joint Venture NDA Agreement");
    meta.subtitle = Some("Confidential Draft".to_string());
    meta.authors
        .push(vocument_flow::metadata::Author::new("Counsel Alice"));
    meta.authors
        .push(vocument_flow::metadata::Author::new("Counsel Bob"));
    meta.extra.insert(
        "matter_number".to_string(),
        serde_json::json!("2026-VOC-778"),
    );

    let mut doc = FlowDocument::new(meta);
    doc.stylesheet = Stylesheet::standard_legal();
    doc.extra.insert(
        "document_classification".to_string(),
        serde_json::json!("RESTRICTED"),
    );

    let section_geo = SectionGeometry::a4_portrait();

    // 1. Heading
    let h1 = Block::Heading(HeadingBlock::new(
        "h_01",
        1,
        vec![TextSpan::bold("1. Definitions and Obligations")],
    ));

    // 2. Paragraph with mixed spans
    let mut span1 = TextSpan::plain("The Receiving Party shall maintain ");
    span1.font_size_pt = Some(11.0);
    let mut span2 = TextSpan::bold("strict confidentiality");
    span2.color = Some(ColorRgba::rgb(180, 20, 20));
    let mut span3 = TextSpan::italic(" in accordance with applicable laws.");
    span3.font_size_pt = Some(11.0);
    let mut para = ParagraphBlock::new("p_01", vec![span1, span2, span3]);
    para.alignment = Some(Alignment::Justified);
    let p_block = Block::Paragraph(para);

    // 3. Table
    let header_cells = vec![
        TableCell::new(
            "tc_h1",
            vec![ParagraphBlock::new("p_th1", vec![TextSpan::bold("Term")]).into()],
        ),
        TableCell::new(
            "tc_h2",
            vec![ParagraphBlock::new("p_th2", vec![TextSpan::bold("Duration")]).into()],
        ),
    ];
    let row1_cells = vec![
        TableCell::new(
            "tc_r1c1",
            vec![ParagraphBlock::new("p_r1c1", vec![TextSpan::plain("Proprietary Data")]).into()],
        ),
        TableCell::new(
            "tc_r1c2",
            vec![ParagraphBlock::new("p_r1c2", vec![TextSpan::plain("5 Years")]).into()],
        ),
    ];
    let table = TableBlock::new(
        "tbl_01",
        vec![TableRow::header(header_cells), TableRow::new(row1_cells)],
    );

    // 4. Callout
    let callout = CalloutBlock {
        id: "call_01".to_string(),
        callout_type: vocument_flow::model::CalloutType::LegalNotice,
        title: Some("Notice".to_string()),
        blocks: vec![
            ParagraphBlock::new(
                "p_c1",
                vec![TextSpan::plain(
                    "Notice: Immediate injunctive relief permitted.",
                )],
            )
            .into(),
        ],
        extra: BTreeMap::new(),
    };

    let section = Section::new(
        "sec_01",
        section_geo,
        vec![h1, p_block, Block::Table(table), Block::Callout(callout)],
    );
    doc.sections.push(section);
    doc
}

#[test]
fn test_pdoc_lossless_roundtrip_with_assets_and_review() {
    let limits = test_limits();
    let doc = create_sample_flow_document();
    let mut package = PdocPackage::from_flow_document(doc.clone());

    // Add assets
    let png_bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDRtest_signature_data_bytes".to_vec();
    let asset1_path = package
        .add_asset("assets/signature.png", png_bytes.clone(), Some("image/png"))
        .expect("add asset");
    assert_eq!(asset1_path, "assets/signature.png");

    let font_bytes = b"\x00\x01\x00\x00embedded_opentype_font_binary_data".to_vec();
    let font_asset_path = package
        .add_content_addressed_asset("ttf", font_bytes.clone(), Some("font/ttf"))
        .expect("add font");
    assert!(font_asset_path.starts_with("assets/"));
    assert!(font_asset_path.ends_with(".ttf"));

    // Add review data (comments & tracked changes)
    let reply = PdocFlowCommentReply {
        reply_id: "rep_01".to_string(),
        author: "Partner Bob".to_string(),
        timestamp: "unix:1700000050".to_string(),
        content: "Agreed, 5 years is standard.".to_string(),
        extra: BTreeMap::new(),
    };
    let comment = PdocFlowComment {
        comment_id: "comm_01".to_string(),
        author: "Associate Alice".to_string(),
        timestamp: "unix:1700000000".to_string(),
        content: "Please confirm duration term.".to_string(),
        anchor_block_id: Some("p_r1c2".to_string()),
        anchor_char_start: Some(0),
        anchor_char_end: Some(7),
        anchor_text: Some("5 years".to_string()),
        resolved: false,
        replies: vec![reply],
        extra: BTreeMap::new(),
    };
    let change = PdocTrackedChange {
        change_id: "chg_01".to_string(),
        change_type: PdocChangeType::Insert,
        author: "Counsel Alice".to_string(),
        timestamp: "unix:1700000010".to_string(),
        target_block_id: Some("p_01".to_string()),
        text_snippet: Some("strict confidentiality".to_string()),
        details: Some("Inserted 'strict confidentiality'".to_string()),
        extra: BTreeMap::new(),
    };
    package.review = Some(PdocReviewData {
        comments: vec![comment],
        tracked_changes: vec![change],
        extra: BTreeMap::new(),
    });

    // Add forward-compatible unmapped entry
    package.unmapped_entries.insert(
        "vendor_extension.xml".to_string(),
        b"<custom:metadata>ProprietaryData</custom:metadata>".to_vec(),
    );

    // Serialize to bytes
    let archive_bytes = PdocContainer::write_to_bytes(&package).expect("write .pdoc");
    assert!(!archive_bytes.is_empty());
    assert!(archive_bytes.starts_with(b"PK\x03\x04"));

    // Deserialize back
    let restored = PdocContainer::read_from_bytes(&archive_bytes, &limits).expect("read .pdoc");

    // Verify 100% losslessness
    assert_eq!(restored.document.metadata.id, doc.metadata.id);
    assert_eq!(restored.document.metadata.title, doc.metadata.title);
    assert_eq!(restored.document.metadata.subtitle, doc.metadata.subtitle);
    assert_eq!(restored.document.metadata.authors, doc.metadata.authors);
    assert_eq!(restored.document.metadata.extra, doc.metadata.extra);
    assert_eq!(restored.document.extra, doc.extra);
    assert_eq!(restored.document.sections.len(), 1);
    assert_eq!(restored.document.sections[0].blocks.len(), 4);

    // Verify assets
    assert_eq!(
        restored.get_asset("assets/signature.png"),
        Some(png_bytes.as_slice())
    );
    assert_eq!(
        restored.get_asset(&font_asset_path),
        Some(font_bytes.as_slice())
    );

    // Verify review
    let restored_review = restored.review.expect("review data");
    assert_eq!(restored_review.comments.len(), 1);
    assert_eq!(restored_review.comments[0].author, "Associate Alice");
    assert_eq!(restored_review.comments[0].replies.len(), 1);
    assert_eq!(restored_review.comments[0].replies[0].author, "Partner Bob");
    assert_eq!(restored_review.tracked_changes.len(), 1);
    assert_eq!(
        restored_review.tracked_changes[0].change_type,
        PdocChangeType::Insert
    );

    // Verify unmapped file preserved
    assert_eq!(
        restored.unmapped_entries.get("vendor_extension.xml"),
        Some(&b"<custom:metadata>ProprietaryData</custom:metadata>".to_vec())
    );
}

#[test]
fn test_zip_slip_traversal_attacks_strictly_rejected() {
    let limits = test_limits();
    let attacks = vec![
        "../../etc/passwd",
        "../escaped.txt",
        "assets/../../secret.key",
        "/etc/shadow",
        "\\Windows\\System32\\calc.exe",
        "C:\\autoexec.bat",
        "assets/nested/../../../evil.so",
    ];

    for malicious_name in attacks {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buffer);
            zip.start_file(malicious_name, SimpleFileOptions::default())
                .expect("start malicious entry");
            zip.write_all(b"malicious payload").expect("write payload");
            zip.finish().expect("finish malicious zip");
        }
        let malicious_bytes = buffer.into_inner();
        let result = PdocContainer::read_from_bytes(&malicious_bytes, &limits);
        match result {
            Err(PdocError::ZipSlipDetected { path, reason }) => {
                assert!(
                    path.contains(malicious_name) || !reason.is_empty(),
                    "Expected zip-slip detection on '{malicious_name}', got path='{path}', reason='{reason}'"
                );
            }
            other => panic!("Expected ZipSlipDetected for '{malicious_name}', but got {other:?}"),
        }
    }
}

#[test]
fn test_decompression_bomb_ratio_rejected() {
    let limits = test_limits(); // max ratio: 50
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("manifest.json", opts).expect("start file");
        // Highly compressible zeros (1 MB zeros compress down to a few hundred bytes)
        let zeros = vec![0u8; 1_048_576];
        zip.write_all(&zeros).expect("write zeros");
        zip.finish().expect("finish zip");
    }
    let bomb_bytes = buffer.into_inner();
    let result = PdocContainer::read_from_bytes(&bomb_bytes, &limits);
    match result {
        Err(PdocError::DecompressionBomb { details }) => {
            assert!(
                details.contains("compression ratio"),
                "Unexpected details: {details}"
            );
        }
        other => panic!("Expected DecompressionBomb error, got: {other:?}"),
    }
}

#[test]
fn test_max_entry_count_exceeded_rejected() {
    let mut limits = test_limits();
    limits.max_zip_entries = 5;

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for i in 0..6 {
            zip.start_file(format!("file_{i}.txt"), opts)
                .expect("start file");
            zip.write_all(b"dummy").expect("write dummy");
        }
        zip.finish().expect("finish zip");
    }
    let bytes = buffer.into_inner();
    let result = PdocContainer::read_from_bytes(&bytes, &limits);
    match result {
        Err(PdocError::TooManyEntries { count, max }) => {
            assert_eq!(count, 6);
            assert_eq!(max, 5);
        }
        other => panic!("Expected TooManyEntries, got: {other:?}"),
    }
}

#[test]
fn test_archive_too_large_rejected() {
    let mut limits = test_limits();
    limits.max_pdoc_archive_size_mb = 1; // 1 MB limit

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("large.bin", opts).expect("start file");
        // 1.5 MB uncompressed data
        let chunk = vec![42u8; 1024];
        for _ in 0..1500 {
            zip.write_all(&chunk).expect("write chunk");
        }
        zip.finish().expect("finish zip");
    }
    let bytes = buffer.into_inner();
    let result = PdocContainer::read_from_bytes(&bytes, &limits);
    match result {
        Err(PdocError::ArchiveTooLarge {
            bytes: b,
            max_bytes: m,
        }) => {
            assert!(b > m);
        }
        other => panic!("Expected ArchiveTooLarge, got: {other:?}"),
    }
}

#[test]
fn test_corrupt_asset_sha256_mismatch_detected() {
    let limits = test_limits();
    let doc = create_sample_flow_document();
    let mut package = PdocPackage::from_flow_document(doc.clone());
    package
        .add_asset(
            "assets/evidence.pdf",
            b"%PDF-valid-data".to_vec(),
            Some("application/pdf"),
        )
        .expect("add asset");

    // Tamper with manifest declared SHA-256
    let entry = package
        .manifest
        .assets
        .get_mut("assets/evidence.pdf")
        .expect("asset entry exists");
    entry.sha256 = "0000000000000000000000000000000000000000000000000000000000000000".to_string();

    // 1. Verify that attempting to serialize a tampered package fails validation
    let write_result = PdocContainer::write_to_bytes(&package);
    match write_result {
        Err(PdocError::AssetCorrupted { asset_path, .. }) => {
            assert_eq!(asset_path, "assets/evidence.pdf");
        }
        other => panic!("Expected AssetCorrupted on write, got: {other:?}"),
    }

    // 2. Build a raw ZIP container with a mismatched manifest hash to test read rejection
    use zip::write::SimpleFileOptions;
    let mut raw_zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().large_file(true);

    let mut tampered_manifest = package.manifest.clone();
    tampered_manifest
        .assets
        .get_mut("assets/evidence.pdf")
        .expect("asset entry")
        .sha256 = "0000000000000000000000000000000000000000000000000000000000000000".to_string();

    raw_zip
        .start_file("manifest.json", opts)
        .expect("start manifest");
    raw_zip
        .write_all(&serde_json::to_vec(&tampered_manifest).expect("json"))
        .expect("write manifest");

    raw_zip
        .start_file("metadata.json", opts)
        .expect("start metadata");
    raw_zip
        .write_all(&serde_json::to_vec(&doc.metadata).expect("json"))
        .expect("write metadata");

    raw_zip
        .start_file("styles.json", opts)
        .expect("start styles");
    raw_zip
        .write_all(&serde_json::to_vec(&doc.stylesheet).expect("json"))
        .expect("write styles");

    raw_zip
        .start_file("document.json", opts)
        .expect("start doc");
    let content = vocument_pdoc::PdocDocumentContent {
        schema_version: doc.schema_version.clone(),
        sections: doc.sections.clone(),
        footnotes: doc.footnotes.clone(),
        extra: doc.extra.clone(),
    };
    raw_zip
        .write_all(&serde_json::to_vec(&content).expect("json"))
        .expect("write doc");

    raw_zip
        .start_file("assets/evidence.pdf", opts)
        .expect("start asset");
    raw_zip.write_all(b"%PDF-valid-data").expect("write asset");

    let tampered_bytes = raw_zip.finish().expect("finish zip").into_inner();

    let read_result = PdocContainer::read_from_bytes(&tampered_bytes, &limits);
    match read_result {
        Err(PdocError::AssetCorrupted { asset_path, .. }) => {
            assert_eq!(asset_path, "assets/evidence.pdf");
        }
        other => panic!("Expected AssetCorrupted on read of tampered archive, got: {other:?}"),
    }
}

#[test]
fn test_missing_manifest_rejected() {
    let limits = test_limits();
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let opts = SimpleFileOptions::default();
        zip.start_file("document.json", opts).expect("start file");
        zip.write_all(b"{}").expect("write json");
        zip.finish().expect("finish zip");
    }
    let bytes = buffer.into_inner();
    let result = PdocContainer::read_from_bytes(&bytes, &limits);
    match result {
        Err(PdocError::MissingRequiredEntry { entry_name }) => {
            assert_eq!(entry_name, "manifest.json");
        }
        other => panic!("Expected MissingRequiredEntry('manifest.json'), got: {other:?}"),
    }
}

#[test]
fn test_atomic_filesystem_save_and_open() {
    let limits = test_limits();
    let doc = create_sample_flow_document();
    let package = PdocPackage::from_flow_document(doc);

    let temp_dir = std::env::temp_dir().join(format!("voc_pdoc_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let test_file = temp_dir.join("test_agreement.pdoc");

    PdocContainer::save(&package, &test_file).expect("atomic save to disk");
    assert!(test_file.exists());

    let loaded = PdocContainer::open(&test_file, &limits).expect("open from disk");
    assert_eq!(loaded.document.metadata.id, package.document.metadata.id);
    assert_eq!(
        loaded.document.sections.len(),
        package.document.sections.len()
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
