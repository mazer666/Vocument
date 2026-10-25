use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use vocument_core::config::PackageLimitsConfig;
use vocument_flow::metadata::DocumentMetadata;
use vocument_flow::model::{
    Block, FlowDocument, ImageBlock, ParagraphBlock, Section, SectionGeometry,
};
use vocument_flow::styles::TextSpan;
use vocument_pdoc::{
    PdocContainer, PdocDocumentContent, PdocError, PdocPackage, PdocRecoveryStore,
};
use zip::write::SimpleFileOptions;

fn test_temp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("voc_pdoc_sec_{}_{}", std::process::id(), suffix));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn test_limits() -> PackageLimitsConfig {
    PackageLimitsConfig {
        max_zip_entries: 50,
        max_pdoc_archive_size_mb: 5,
        max_zip_compression_ratio: 50,
    }
}

fn create_valid_flow_document(id: &str) -> FlowDocument {
    let meta = DocumentMetadata::new(id, "Test Document");
    let mut doc = FlowDocument::new(meta);
    let section = Section::new(
        "sec_1",
        SectionGeometry::a4_portrait(),
        vec![Block::Paragraph(ParagraphBlock::new(
            "p_1",
            vec![TextSpan::plain("Valid paragraph content")],
        ))],
    );
    doc.sections.push(section);
    doc
}

#[test]
fn test_duplicate_exact_entry_rejected() {
    let limits = test_limits();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default();

    zip.start_file("manifest.json", opts).expect("start 1");
    zip.write_all(b"{}").expect("write 1");
    // ZipWriter permits ./manifest.json because the string differs, but our normalization catches the collision
    zip.start_file("./manifest.json", opts).expect("start 2");
    zip.write_all(b"{}").expect("write 2");

    let bytes = zip.finish().expect("finish").into_inner();
    let result = PdocContainer::read_from_bytes(&bytes, &limits);

    match result {
        Err(PdocError::DuplicateEntry { details, .. }) => {
            assert!(details.contains("manifest.json"));
        }
        other => panic!("Expected DuplicateEntry error, got: {other:?}"),
    }
}

#[test]
fn test_case_colliding_entry_rejected() {
    let limits = test_limits();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default();

    zip.start_file("document.json", opts).expect("start 1");
    zip.write_all(b"{}").expect("write 1");
    zip.start_file("DOCUMENT.JSON", opts).expect("start 2");
    zip.write_all(b"{}").expect("write 2");

    let bytes = zip.finish().expect("finish").into_inner();
    let result = PdocContainer::read_from_bytes(&bytes, &limits);

    match result {
        Err(PdocError::DuplicateEntry { details, .. }) => {
            assert!(details.contains("document.json"));
        }
        other => panic!("Expected DuplicateEntry error for case collision, got: {other:?}"),
    }
}

#[test]
fn test_forged_header_streaming_decompression_bomb_rejected() {
    let limits = test_limits();
    // Build raw zip entry with 1.5 MB of zeros compressed with Deflate (compressed ~3 KB > 1024, ratio ~500 > 50)
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("large.bin", opts).expect("start large");
    let zeros = vec![0u8; 1_500_000];
    zip.write_all(&zeros).expect("write zeros");

    let bytes = zip.finish().expect("finish").into_inner();
    let result = PdocContainer::read_from_bytes(&bytes, &limits);

    match result {
        Err(PdocError::DecompressionBomb { .. }) => {}
        other => panic!("Expected DecompressionBomb error, got: {other:?}"),
    }
}

#[test]
fn test_missing_referenced_asset_rejected() {
    let mut doc = create_valid_flow_document("doc_missing_img");
    let img = ImageBlock {
        id: "img_missing".to_string(),
        asset_ref: "assets/non_existent.png".to_string(),
        alt_text: "Missing chart".to_string(),
        caption: None,
        intrinsic_width_pt: 100.0,
        intrinsic_height_pt: 100.0,
        display_width_pt: None,
        display_height_pt: None,
        extra: BTreeMap::new(),
    };
    doc.sections[0].blocks.push(Block::Image(img));

    let pkg = PdocPackage::from_flow_document(doc);
    let result = PdocContainer::write_to_bytes(&pkg);

    match result {
        Err(PdocError::MissingReferencedAsset {
            asset_ref,
            referrer,
        }) => {
            assert_eq!(asset_ref, "assets/non_existent.png");
            assert!(referrer.contains("img_missing"));
        }
        other => panic!("Expected MissingReferencedAsset error, got: {other:?}"),
    }
}

#[test]
fn test_undefined_style_reference_rejected() {
    let mut doc = create_valid_flow_document("doc_bad_style");
    let mut para = ParagraphBlock::new("p_bad_style", vec![TextSpan::plain("Sample text")]);
    para.style_id = Some("NonExistentHeadingStyle".to_string());
    doc.sections[0].blocks.push(Block::Paragraph(para));

    let pkg = PdocPackage::from_flow_document(doc);
    let result = PdocContainer::write_to_bytes(&pkg);

    match result {
        Err(PdocError::UndefinedStyleReference {
            style_id,
            element_id,
        }) => {
            assert_eq!(style_id, "NonExistentHeadingStyle");
            assert_eq!(element_id, "p_bad_style");
        }
        Err(PdocError::Flow(vocument_flow::error::FlowError::UnknownStyle {
            style_id,
            block_id,
            ..
        })) => {
            assert_eq!(style_id, "NonExistentHeadingStyle");
            assert_eq!(block_id, "p_bad_style");
        }
        other => panic!("Expected style error, got: {other:?}"),
    }
}

#[test]
fn test_incompatible_major_version_rejected() {
    let limits = test_limits();
    let doc = create_valid_flow_document("doc_v2");
    let mut pkg = PdocPackage::from_flow_document(doc.clone());
    pkg.manifest.format_version = "2.0".to_string();

    // Construct raw ZIP with major version "2.0" in manifest.json
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default();

    zip.start_file("manifest.json", opts).expect("start");
    zip.write_all(&serde_json::to_vec(&pkg.manifest).expect("json"))
        .expect("write");
    zip.start_file("metadata.json", opts).expect("start");
    zip.write_all(&serde_json::to_vec(&doc.metadata).expect("json"))
        .expect("write");
    zip.start_file("styles.json", opts).expect("start");
    zip.write_all(&serde_json::to_vec(&doc.stylesheet).expect("json"))
        .expect("write");
    zip.start_file("document.json", opts).expect("start");
    let content = PdocDocumentContent {
        schema_version: doc.schema_version.clone(),
        sections: doc.sections.clone(),
        footnotes: doc.footnotes.clone(),
        extra: doc.extra.clone(),
    };
    zip.write_all(&serde_json::to_vec(&content).expect("json"))
        .expect("write");

    let bytes = zip.finish().expect("finish").into_inner();
    let result = PdocContainer::read_from_bytes(&bytes, &limits);

    match result {
        Err(PdocError::IncompatibleMajorVersion {
            version,
            supported_major,
        }) => {
            assert_eq!(version, "2.0");
            assert_eq!(supported_major, "1");
        }
        other => panic!("Expected IncompatibleMajorVersion, got: {other:?}"),
    }
}

#[test]
fn test_minor_version_forward_compatibility_preserved() {
    let limits = test_limits();
    let doc = create_valid_flow_document("doc_v1_1");
    let mut pkg = PdocPackage::from_flow_document(doc.clone());
    pkg.manifest.format_version = "1.1".to_string();
    pkg.manifest
        .extra
        .insert("future_feature_flag".to_string(), serde_json::json!(true));

    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default();

    zip.start_file("manifest.json", opts).expect("start");
    zip.write_all(&serde_json::to_vec(&pkg.manifest).expect("json"))
        .expect("write");
    zip.start_file("metadata.json", opts).expect("start");
    zip.write_all(&serde_json::to_vec(&doc.metadata).expect("json"))
        .expect("write");
    zip.start_file("styles.json", opts).expect("start");
    zip.write_all(&serde_json::to_vec(&doc.stylesheet).expect("json"))
        .expect("write");
    zip.start_file("document.json", opts).expect("start");
    let content = PdocDocumentContent {
        schema_version: doc.schema_version.clone(),
        sections: doc.sections.clone(),
        footnotes: doc.footnotes.clone(),
        extra: doc.extra.clone(),
    };
    zip.write_all(&serde_json::to_vec(&content).expect("json"))
        .expect("write");

    let bytes = zip.finish().expect("finish").into_inner();
    let loaded = PdocContainer::read_from_bytes(&bytes, &limits).expect("load 1.1 package");

    assert_eq!(loaded.manifest.format_version, "1.1");
    assert_eq!(
        loaded.manifest.extra.get("future_feature_flag"),
        Some(&serde_json::json!(true))
    );
}

#[test]
fn test_external_conflict_detection_and_safe_conflict_copy() {
    let dir = test_temp_dir("conflict");
    let file_path = dir.join("case_brief.pdoc");

    let doc = create_valid_flow_document("doc_conflict_01");
    let package = PdocPackage::from_flow_document(doc);

    // Initial atomic save and capture fingerprint
    let initial_fp =
        PdocContainer::save_with_conflict_check(&package, &file_path, None).expect("initial save");

    // Simulate external process modification on disk
    std::fs::write(&file_path, b"corrupted by another process").expect("external write");

    // Attempting to save with expected initial fingerprint MUST fail with ExternalConflictDetected
    let save_attempt =
        PdocContainer::save_with_conflict_check(&package, &file_path, Some(&initial_fp));
    match save_attempt {
        Err(PdocError::ExternalConflictDetected { path, details }) => {
            assert!(path.contains("case_brief.pdoc"));
            assert!(details.contains("file on disk was modified by an external process"));
        }
        other => panic!("Expected ExternalConflictDetected, got: {other:?}"),
    }

    // Safe conflict copy must succeed without data loss
    let conflict_copy_path =
        PdocContainer::save_conflict_copy(&package, &file_path).expect("save conflict copy");
    assert!(conflict_copy_path.exists());
    assert!(
        conflict_copy_path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("valid filename")
            .contains("conflict")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_autosave_checkpoint_and_crash_recovery() {
    let dir = test_temp_dir("recovery");
    let recovery_dir = dir.join("recovery");
    let store = PdocRecoveryStore::new(&recovery_dir).expect("init recovery store");

    let doc = create_valid_flow_document("doc_recover_99");
    let package = PdocPackage::from_flow_document(doc);

    // 1. Save autosave checkpoint
    let cp_path = store.save_checkpoint(&package).expect("save checkpoint");
    assert!(cp_path.exists());
    assert!(store.has_checkpoint("doc_recover_99"));

    // 2. Load recovery checkpoint
    let limits = test_limits();
    let restored = store
        .load_checkpoint("doc_recover_99", &limits)
        .expect("load checkpoint")
        .expect("checkpoint exists");
    assert_eq!(restored.manifest.document_id, "doc_recover_99");

    // 3. Clean remove after successful save
    assert!(store.remove_checkpoint("doc_recover_99").expect("remove"));
    assert!(!store.has_checkpoint("doc_recover_99"));

    // 4. Stale temp files cleanup
    let dummy_temp = dir.join(".tmp_1234_987654321_doc.pdoc");
    std::fs::write(&dummy_temp, b"leftover").expect("write temp");
    assert!(dummy_temp.exists());
    let cleaned = PdocRecoveryStore::cleanup_stale_temp_files(&dir).expect("cleanup temp");
    assert_eq!(cleaned, 1);
    assert!(!dummy_temp.exists());
    let _ = std::fs::remove_dir_all(&dir);
}
