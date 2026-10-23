//! Integration and invariant tests for Multi-Document Operations (`ROADMAP.md` M3.2, `ADR-0019`).

use crate::MuPdfEngineAdapter;
use crate::attachment_signature_tests;
use crate::multidoc_ops::{insert_pages_ext, merge_documents_ext, split_document};
use vocument_core::pdf_engine::{PdfEngine, PdfEngineError};

fn create_sample_pdf(title: &str, pages: &[&str]) -> Vec<u8> {
    let mut objects: Vec<String> = Vec::new();
    let num_pages = pages.len();

    // 1: Catalog
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_owned());

    // 2: Pages root
    let mut kids_refs = String::new();
    for i in 0..num_pages {
        kids_refs.push_str(&format!("{} 0 R ", 3 + i * 2));
    }
    objects.push(format!(
        "<< /Type /Pages /Kids [ {kids_refs}] /Count {num_pages} >>"
    ));

    // For each page: page object and content stream
    for (i, &content_text) in pages.iter().enumerate() {
        let page_obj_id = 3 + i * 2;
        let content_obj_id = page_obj_id + 1;

        let content_stream = format!("BT /F1 12 Tf 72 700 Td ({content_text}) Tj ET\n");
        let stream_len = content_stream.len();

        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {content_obj_id} 0 R /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> >>"
        ));
        objects.push(format!(
            "<< /Length {stream_len} >>\nstream\n{content_stream}endstream"
        ));
    }

    // Info dict
    let info_obj_id = 3 + num_pages * 2;
    objects.push(format!(
        "<< /Title ({title}) /Author (Vocument Test) /Creator (Vocument Core) >>"
    ));

    assemble_pdf(&objects, info_obj_id)
}

fn assemble_pdf(objects: &[String], info_obj_id: usize) -> Vec<u8> {
    let mut buffer = Vec::new();
    buffer.extend_from_slice(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n");

    let mut offsets = Vec::new();
    for (idx, obj_str) in objects.iter().enumerate() {
        let obj_num = idx + 1;
        offsets.push(buffer.len());
        buffer.extend_from_slice(format!("{obj_num} 0 obj\n{obj_str}\nendobj\n").as_bytes());
    }

    let xref_offset = buffer.len();
    buffer.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    buffer.extend_from_slice(b"0000000000 65535 f \n");
    for off in offsets {
        buffer.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }

    buffer.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info {info_obj_id} 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );

    buffer
}

#[test]
fn test_merge_documents_unifies_pages_and_metadata() {
    let doc_a = create_sample_pdf("Document A", &["Doc A Page 1", "Doc A Page 2"]);
    let doc_b = create_sample_pdf(
        "Document B",
        &["Doc B Page 1", "Doc B Page 2", "Doc B Page 3"],
    );
    let engine = MuPdfEngineAdapter::new();

    let merged_bytes =
        merge_documents_ext(&[&doc_a, &doc_b], false).expect("merge documents should succeed");

    let doc = engine
        .open_pdf_bytes(&merged_bytes)
        .expect("open merged pdf");
    assert_eq!(doc.summary().page_count, 5);
    assert_eq!(doc.summary().title.as_deref(), Some("Document A"));

    // Verify content on all 5 pages
    let p0 = doc.extract_page_text(0).expect("p0 text");
    assert!(p0.lines[0].text.contains("Doc A Page 1"));

    let p2 = doc.extract_page_text(2).expect("p2 text");
    assert!(p2.lines[0].text.contains("Doc B Page 1"));

    let p4 = doc.extract_page_text(4).expect("p4 text");
    assert!(p4.lines[0].text.contains("Doc B Page 3"));
}

#[test]
fn test_insert_pages_into_target_document() {
    let target = create_sample_pdf("Target", &["Target P1", "Target P2"]);
    let source = create_sample_pdf("Source", &["Source P1", "Source P2", "Source P3"]);
    let engine = MuPdfEngineAdapter::new();

    // Insert source pages 0 and 2 at index 1 of target
    let inserted_bytes =
        insert_pages_ext(&target, &source, &[0, 2], 1, false).expect("insert pages should succeed");

    let doc = engine
        .open_pdf_bytes(&inserted_bytes)
        .expect("open inserted pdf");
    assert_eq!(doc.summary().page_count, 4);

    let t0 = doc.extract_page_text(0).expect("t0");
    assert!(t0.lines[0].text.contains("Target P1"));

    let t1 = doc.extract_page_text(1).expect("t1");
    assert!(t1.lines[0].text.contains("Source P1"));

    let t2 = doc.extract_page_text(2).expect("t2");
    assert!(t2.lines[0].text.contains("Source P3"));

    let t3 = doc.extract_page_text(3).expect("t3");
    assert!(t3.lines[0].text.contains("Target P2"));
}

#[test]
fn test_split_document_partitions_into_standalone_pdfs() {
    let doc = create_sample_pdf("Big Doc", &["P1", "P2", "P3", "P4", "P5"]);
    let engine = MuPdfEngineAdapter::new();

    let partitions = vec![
        vec![0, 1], // Part 1: Pages 1-2
        vec![2],    // Part 2: Page 3
        vec![3, 4], // Part 3: Pages 4-5
    ];

    let split_results = split_document(&doc, &partitions).expect("split document should succeed");

    assert_eq!(split_results.len(), 3);

    let part1 = engine.open_pdf_bytes(&split_results[0]).expect("part 1");
    assert_eq!(part1.summary().page_count, 2);

    let part2 = engine.open_pdf_bytes(&split_results[1]).expect("part 2");
    assert_eq!(part2.summary().page_count, 1);

    let part3 = engine.open_pdf_bytes(&split_results[2]).expect("part 3");
    assert_eq!(part3.summary().page_count, 2);
}

#[test]
fn test_multidoc_operations_refuse_on_signed_documents_fail_closed() {
    let signed_pdf = attachment_signature_tests::pdf_with_attachment_and_signature(false);
    let plain_pdf = create_sample_pdf("Plain", &["Plain Page 1"]);
    let engine = MuPdfEngineAdapter::new();

    // 1. Merge must fail-closed if any source is signed
    let merge_err = merge_documents_ext(&[&plain_pdf, &signed_pdf], false);
    assert!(matches!(
        merge_err,
        Err(PdfEngineError::OperationBlockedBySignature { .. })
    ));

    let merge_err2 = merge_documents_ext(&[&signed_pdf, &plain_pdf], false);
    assert!(matches!(
        merge_err2,
        Err(PdfEngineError::OperationBlockedBySignature { .. })
    ));

    // 2. Insert must fail-closed if target is signed
    let insert_err = insert_pages_ext(&signed_pdf, &plain_pdf, &[0], 0, false);
    assert!(matches!(
        insert_err,
        Err(PdfEngineError::OperationBlockedBySignature { .. })
    ));

    // 3. Insert must fail-closed if source is signed
    let insert_err2 = insert_pages_ext(&plain_pdf, &signed_pdf, &[0], 0, false);
    assert!(matches!(
        insert_err2,
        Err(PdfEngineError::OperationBlockedBySignature { .. })
    ));

    // 4. Merge with explicit signature invalidation succeeds and strips /V
    let merged_inval = merge_documents_ext(&[&signed_pdf, &plain_pdf], true)
        .expect("merge with explicit signature invalidation succeeds");
    let merged_doc = engine.open_pdf_bytes(&merged_inval).expect("open merged");
    assert_eq!(merged_doc.summary().page_count, 2);
    assert!(!merged_doc.document_signatures().expect("sigs")[0].is_signed);

    // 5. Insert with explicit signature invalidation succeeds and strips /V
    let insert_inval = insert_pages_ext(&signed_pdf, &plain_pdf, &[0], 0, true)
        .expect("insert with explicit signature invalidation succeeds");
    let inserted_doc = engine.open_pdf_bytes(&insert_inval).expect("open inserted");
    assert_eq!(inserted_doc.summary().page_count, 2);
    assert!(!inserted_doc.document_signatures().expect("sigs")[0].is_signed);
}
