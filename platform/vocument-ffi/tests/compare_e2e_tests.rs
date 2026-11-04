//! End-to-end integration tests for Document Comparison across the UniFFI boundary (`ROADMAP.md` M3.7, `COMPARE-001`–`004`).

use vocument_ffi::compare_dto::{
    FfiComparisonOptions, FfiDiffGranularity, FfiDiffOperation, document_core_compare_documents,
    document_core_compare_flow_editor_sessions, document_core_compare_pages_visual,
    document_core_compare_text_streams,
};
use vocument_ffi::document_core_open_brokered_file;
use vocument_ffi::editor_dto::{
    document_core_create_editor_session, document_core_editor_insert_text,
};
use vocument_ffi::session_dto::{FfiBrokeredFileDescriptor, FfiFileAccessOrigin};

mod common;

const SAMPLE_PDF: &[u8] = include_bytes!("../../../tests/corpus/m0_1_sample_two_pages.pdf");

fn open_session(name: &str) -> String {
    let descriptor = FfiBrokeredFileDescriptor {
        source_token: format!("compare://{name}"),
        display_name: name.to_string(),
        access_origin: FfiFileAccessOrigin::MacOsScopedUrl,
        os_file_id: Some(format!("cmp-{name}")),
        modified_unix_ms: Some(1_700_000_000_000),
        bytes: SAMPLE_PDF.to_vec(),
    };
    let snap = document_core_open_brokered_file(descriptor).expect("open brokered file");
    snap.session_id
}

#[test]
fn test_ffi_compare_text_streams_identical() {
    let text = "§ 1 Anwendungsbereich\nDieser Vertrag gilt verbindlich.";
    let opts = FfiComparisonOptions::default();

    let report = document_core_compare_text_streams(
        "Doc A".to_string(),
        text.to_string(),
        "Doc B".to_string(),
        text.to_string(),
        opts,
    );

    assert_eq!(report.summary.total_changes, 0);
    assert!(report.summary.similarity_ratio > 0.99);
    assert!(report.changes.is_empty());
    assert!(report.markdown_report.contains("Doc A"));
    assert!(report.html_report.contains("<!DOCTYPE html>"));
    assert!(report.json_report.contains("\"total_changes\": 0"));
}

#[test]
fn test_ffi_compare_text_streams_with_changes_and_clauses() {
    let text_a =
        "§ 1 Gegenstand\nDie Partei A liefert Software.\n§ 2 Vergütung\n100 Euro je Stunde.";
    let text_b = "§ 1 Gegenstand\nDie Partei A liefert moderne Software.\n§ 2 Vergütung\n150 Euro je Stunde.\n§ 3 Haftung\nDie Haftung ist beschränkt.";
    let opts = FfiComparisonOptions {
        granularity: FfiDiffGranularity::Word,
        ignore_whitespace: true,
        ignore_case: false,
        visual_pixel_threshold: 16,
        visual_cluster_distance_pt: 10.0,
        align_clauses: true,
    };

    let report = document_core_compare_text_streams(
        "Vertrag v1".to_string(),
        text_a.to_string(),
        "Vertrag v2".to_string(),
        text_b.to_string(),
        opts,
    );

    assert!(report.summary.total_changes >= 2);
    assert!(report.summary.insertions_count >= 1);
    assert!(
        report
            .changes
            .iter()
            .any(|c| c.operation == FfiDiffOperation::Insert)
    );
    assert!(report.changes.iter().any(|c| c.clause_hint.is_some()));
}

#[test]
fn test_ffi_compare_pdf_documents_identical_session() {
    let session_a = open_session("doc_a.pdf");
    let opts = FfiComparisonOptions::default();

    let report = document_core_compare_documents(session_a.clone(), session_a, opts)
        .expect("compare same document");

    assert_eq!(report.summary.total_changes, 0);
    assert!(report.summary.similarity_ratio > 0.99);
    assert_eq!(report.page_count_a, 2);
    assert_eq!(report.page_count_b, 2);
    assert_eq!(report.page_alignments.len(), 2);
}

#[test]
fn test_ffi_compare_pages_visual() {
    let session_a = open_session("vis_a.pdf");
    let session_b = open_session("vis_b.pdf");
    let opts = FfiComparisonOptions::default();

    let diff = document_core_compare_pages_visual(session_a, 0, session_b, 0, opts)
        .expect("visual comparison");

    assert_eq!(diff.page_a, 0);
    assert_eq!(diff.page_b, 0);
    // Identical pages produce negligible pixel difference
    assert!(diff.pixel_difference_percentage < 0.01);
}

#[test]
fn test_ffi_compare_flow_editor_sessions() {
    let s1 = document_core_create_editor_session("Schriftsatz v1".to_string())
        .expect("create editor session 1");
    let s2 = document_core_create_editor_session("Schriftsatz v2".to_string())
        .expect("create editor session 2");

    let _ = document_core_editor_insert_text(
        s1.editor_id.clone(),
        "§ 1 Sachverhalt: Der Kläger beantragt Abweisung.".to_string(),
    )
    .expect("insert s1");
    let _ = document_core_editor_insert_text(
        s2.editor_id.clone(),
        "§ 1 Sachverhalt: Der Kläger beantragt vollumfängliche Verurteilung.".to_string(),
    )
    .expect("insert s2");

    let opts = FfiComparisonOptions::default();
    let report = document_core_compare_flow_editor_sessions(s1.editor_id, s2.editor_id, opts)
        .expect("compare editor sessions");

    assert!(report.summary.total_changes >= 1);
    assert!(
        report
            .changes
            .iter()
            .any(|c| c.clause_hint.as_deref() == Some("§ 1 Sachverhalt"))
    );
}
