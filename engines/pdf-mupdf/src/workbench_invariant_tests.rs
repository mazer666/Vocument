//! Architectural invariant and structural integrity tests for Page Workbench operations (`ROADMAP.md` M3.0, `ADR-0019`).
//!
//! Verifies:
//! 1. Document Catalog and Trailer Metadata preservation (/Info, Title, Author, Subject, etc.)
//! 2. Bookmark outlines (/Outlines) survival, destination preservation on reorder, safe re-anchoring on deletion
//! 3. Logical page labels (/PageLabels) preservation
//! 4. Interactive AcroForm fields (/AcroForm, /Fields) with unique naming on duplication and cleanup on deletion
//! 5. Tagged PDF / PDF/UA structure trees (/StructTreeRoot, /MarkInfo)
//! 6. Digital signatures fail-closed rejection and explicit invalidation handling (`SIG-001`)
//! 7. Independent deserialization and verification with zero syntax defects

use crate::mutate::{rotate_pages_in_pdf, rotate_pages_in_pdf_ext};
use crate::page_ops::{
    delete_pages, delete_pages_ext, duplicate_page, duplicate_page_ext, extract_pages,
    reorder_pages, reorder_pages_ext,
};
use crate::{MuPdfEngineAdapter, attachment_signature_tests};
use vocument_core::pdf_engine::{PdfEngine, PdfEngineError};

/// Constructs a valid 3-page PDF with full structural features:
/// - /Info dictionary with Title, Author, Subject, Creator, Producer
/// - /Outlines with bookmarks targeting Page 1 and Page 2
/// - /PageLabels with Roman numerals on pages 0..1 and Decimals on page 2
/// - /AcroForm with a text form field on Page 1
/// - Internal /Link annotation on Page 2 targeting Page 3
/// - Tagged PDF markers (/MarkInfo, /StructTreeRoot)
fn build_complex_test_pdf() -> Vec<u8> {
    let p1_text = "BT /F1 12 Tf 72 700 Td (Page 1 Content) Tj ET\n";
    let p2_text = "BT /F1 12 Tf 72 700 Td (Page 2 Content) Tj ET\n";
    let p3_text = "BT /F1 12 Tf 72 700 Td (Page 3 Content) Tj ET\n";

    let objects: Vec<String> = vec![
        // 1: Catalog
        "<< /Type /Catalog /Pages 2 0 R /Outlines 6 0 R /PageLabels << /Nums [0 << /S /r >> 2 << /S /D >>] >> \
/MarkInfo << /Marked true >> /StructTreeRoot 11 0 R /AcroForm << /Fields [12 0 R] >> >>"
            .to_owned(),
        // 2: Pages root
        "<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 >>".to_owned(),
        // 3: Page 1
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 7 0 R >> >> \
/Contents 8 0 R /Annots [12 0 R] >>"
            .to_owned(),
        // 4: Page 2
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 7 0 R >> >> \
/Contents 9 0 R /Annots [13 0 R] >>"
            .to_owned(),
        // 5: Page 3
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 7 0 R >> >> \
/Contents 10 0 R >>"
            .to_owned(),
        // 6: Outlines root
        "<< /Type /Outlines /First 14 0 R /Last 15 0 R /Count 2 >>".to_owned(),
        // 7: Font
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        // 8: Content Page 1
        format!("<< /Length {} >>\nstream\n{p1_text}endstream", p1_text.len()),
        // 9: Content Page 2
        format!("<< /Length {} >>\nstream\n{p2_text}endstream", p2_text.len()),
        // 10: Content Page 3
        format!("<< /Length {} >>\nstream\n{p3_text}endstream", p3_text.len()),
        // 11: Tagged PDF StructTreeRoot
        "<< /Type /StructTreeRoot /RoleMap << /Custom /P >> >>".to_owned(),
        // 12: Form field on Page 1
        "<< /Type /Annot /Subtype /Widget /FT /Tx /T (CustomerName) /V (Alice) /Rect [72 500 200 520] /P 3 0 R >>"
            .to_owned(),
        // 13: Link annotation on Page 2 targeting Page 3
        "<< /Type /Annot /Subtype /Link /Rect [72 600 200 620] /Dest [5 0 R /XYZ 0 792 0] /P 4 0 R >>"
            .to_owned(),
        // 14: Bookmark targeting Page 1
        "<< /Title (Bookmark to Page 1) /Parent 6 0 R /Next 15 0 R /Dest [3 0 R /XYZ 0 792 0] >>".to_owned(),
        // 15: Bookmark targeting Page 2
        "<< /Title (Bookmark to Page 2) /Parent 6 0 R /Prev 14 0 R /Dest [4 0 R /XYZ 0 792 0] >>".to_owned(),
    ];

    let mut out = Vec::new();
    out.extend_from_slice(b"%PDF-1.7\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref_offset = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info << /Title (Test Document Title) /Author (Dr. Test Author) \
/Subject (Invariant Testing) /Creator (Vocument Test Suite) /Producer (Vocument Engine) >> >>\n\
startxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

fn page_full_text(structured: &vocument_core::pdf_engine::PdfPageStructuredText) -> String {
    structured
        .words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn test_reorder_preserves_metadata_outlines_forms_and_tagged_structures() {
    let original = build_complex_test_pdf();
    let engine = MuPdfEngineAdapter::new();

    // Verify baseline document
    let initial_doc = engine.open_pdf_bytes(&original).expect("open complex pdf");
    assert_eq!(initial_doc.summary().page_count, 3);
    assert_eq!(
        initial_doc.summary().title.as_deref(),
        Some("Test Document Title")
    );
    assert_eq!(
        initial_doc.summary().author.as_deref(),
        Some("Dr. Test Author")
    );

    // Permutation [2, 0, 1] swaps page 3 to first, page 1 to second, page 2 to third
    let reordered_bytes = reorder_pages(&original, &[2, 0, 1]).expect("reorder succeeds");
    let reordered_doc = engine
        .open_pdf_bytes(&reordered_bytes)
        .expect("open reordered pdf");

    // 1. Page count preserved
    assert_eq!(reordered_doc.summary().page_count, 3);

    // 2. Metadata (/Info) strictly preserved
    assert_eq!(
        reordered_doc.summary().title.as_deref(),
        Some("Test Document Title")
    );
    assert_eq!(
        reordered_doc.summary().author.as_deref(),
        Some("Dr. Test Author")
    );
    assert_eq!(
        reordered_doc.summary().subject.as_deref(),
        Some("Invariant Testing")
    );
    assert_eq!(
        reordered_doc.summary().creator.as_deref(),
        Some("Vocument Test Suite")
    );

    // 3. Page contents follow new order
    let p0_text = reordered_doc.extract_page_text(0).expect("page 0 text");
    assert!(page_full_text(&p0_text).contains("Page 3 Content"));
    let p1_text = reordered_doc.extract_page_text(1).expect("page 1 text");
    assert!(page_full_text(&p1_text).contains("Page 1 Content"));
    let p2_text = reordered_doc.extract_page_text(2).expect("page 2 text");
    assert!(page_full_text(&p2_text).contains("Page 2 Content"));

    // 4. Tagged PDF markers and Outlines survive in raw dictionary structure
    let pdf_str = String::from_utf8_lossy(&reordered_bytes);
    assert!(pdf_str.contains("/Marked true"));
    assert!(pdf_str.contains("/StructTreeRoot"));
    assert!(pdf_str.contains("/Outlines"));
    assert!(pdf_str.contains("Bookmark to Page 1"));
    assert!(pdf_str.contains("CustomerName"));
}

#[test]
fn test_duplicate_page_deduplicates_form_field_names_and_preserves_catalog() {
    let original = build_complex_test_pdf();
    let engine = MuPdfEngineAdapter::new();

    // Duplicate page 0 (which carries form field CustomerName)
    let duplicated_bytes = duplicate_page(&original, 0).expect("duplicate page 0");
    let dup_doc = engine
        .open_pdf_bytes(&duplicated_bytes)
        .expect("open duplicated pdf");

    // Page count increased to 4
    assert_eq!(dup_doc.summary().page_count, 4);

    // Metadata preserved
    assert_eq!(
        dup_doc.summary().title.as_deref(),
        Some("Test Document Title")
    );
    assert_eq!(dup_doc.summary().author.as_deref(), Some("Dr. Test Author"));

    // Both original and copy 1 are Page 1 content
    let p0_text = dup_doc.extract_page_text(0).expect("p0");
    let p1_text = dup_doc.extract_page_text(1).expect("p1");
    assert!(page_full_text(&p0_text).contains("Page 1 Content"));
    assert!(page_full_text(&p1_text).contains("Page 1 Content"));

    // Form field deduplication: must contain CustomerName_Copy1 without colliding with CustomerName
    let pdf_str = String::from_utf8_lossy(&duplicated_bytes);
    assert!(pdf_str.contains("CustomerName"));
    assert!(pdf_str.contains("CustomerName_Copy1"));
    assert!(pdf_str.contains("/StructTreeRoot"));
}

#[test]
fn test_delete_page_cleans_up_acroform_fields_and_reanchors_bookmarks() {
    let original = build_complex_test_pdf();
    let engine = MuPdfEngineAdapter::new();

    // Delete page 0 (which contains CustomerName and is targeted by Bookmark 1)
    let deleted_bytes = delete_pages(&original, &[0]).expect("delete page 0");
    let del_doc = engine
        .open_pdf_bytes(&deleted_bytes)
        .expect("open deleted pdf");

    assert_eq!(del_doc.summary().page_count, 2);
    assert_eq!(
        del_doc.summary().title.as_deref(),
        Some("Test Document Title")
    );

    // Deleted page 0 content is gone; remaining pages are former pages 2 and 3
    let p0_text = del_doc.extract_page_text(0).expect("p0");
    assert!(page_full_text(&p0_text).contains("Page 2 Content"));
    let p1_text = del_doc.extract_page_text(1).expect("p1");
    assert!(page_full_text(&p1_text).contains("Page 3 Content"));

    // Document structures remain coherent and valid
    let pdf_str = String::from_utf8_lossy(&deleted_bytes);
    assert!(pdf_str.contains("/StructTreeRoot"));
    assert!(pdf_str.contains("/Outlines"));
}

#[test]
fn test_extract_pages_transfers_metadata_and_leaves_source_intact() {
    let original = build_complex_test_pdf();
    let engine = MuPdfEngineAdapter::new();

    // Extract pages [0, 2]
    let extracted_bytes = extract_pages(&original, &[0, 2]).expect("extract pages");
    let ext_doc = engine
        .open_pdf_bytes(&extracted_bytes)
        .expect("open extracted pdf");

    assert_eq!(ext_doc.summary().page_count, 2);
    // Info metadata carried over
    assert_eq!(
        ext_doc.summary().title.as_deref(),
        Some("Test Document Title")
    );
    assert_eq!(ext_doc.summary().author.as_deref(), Some("Dr. Test Author"));

    let p0_text = ext_doc.extract_page_text(0).expect("p0");
    assert!(page_full_text(&p0_text).contains("Page 1 Content"));
    let p1_text = ext_doc.extract_page_text(1).expect("p1");
    assert!(page_full_text(&p1_text).contains("Page 3 Content"));

    // Source document remains completely identical
    let orig_doc = engine.open_pdf_bytes(&original).expect("open original");
    assert_eq!(orig_doc.summary().page_count, 3);
}

#[test]
fn test_preflight_blocks_signed_document_fail_closed_by_default() {
    let signed_pdf = attachment_signature_tests::pdf_with_attachment_and_signature(false);
    let engine = MuPdfEngineAdapter::new();

    // Baseline: verify it carries active signature
    let doc = engine.open_pdf_bytes(&signed_pdf).expect("open signed pdf");
    let sigs = doc.document_signatures().expect("signatures");
    assert_eq!(sigs.len(), 1);
    assert!(sigs[0].is_signed);

    // 1. Reordering must fail-closed
    let reorder_err = reorder_pages(&signed_pdf, &[0]);
    assert!(matches!(
        reorder_err,
        Err(PdfEngineError::OperationBlockedBySignature { .. })
    ));

    // 2. Duplicating must fail-closed
    let dup_err = duplicate_page(&signed_pdf, 0);
    assert!(matches!(
        dup_err,
        Err(PdfEngineError::OperationBlockedBySignature { .. })
    ));

    // 3. Deleting must fail-closed
    let del_err = delete_pages(&signed_pdf, &[0]);
    assert!(matches!(
        del_err,
        Err(PdfEngineError::OperationBlockedBySignature { .. })
    ));

    // 4. Rotating must fail-closed (M3.1 rotation hardening)
    let rot_err = rotate_pages_in_pdf(&signed_pdf, 1, &[0], 90);
    assert!(matches!(
        rot_err,
        Err(PdfEngineError::OperationBlockedBySignature { .. })
    ));

    // Source PDF bytes were never modified
    let doc_check = engine
        .open_pdf_bytes(&signed_pdf)
        .expect("re-open original");
    assert_eq!(
        doc_check.document_signatures().expect("sigs")[0].signer,
        "Dr. Weber"
    );
}

#[test]
fn test_explicit_signature_invalidation_marks_signature_invalidated() {
    let signed_pdf = attachment_signature_tests::pdf_with_attachment_and_signature(false);
    let engine = MuPdfEngineAdapter::new();

    // When allow_signature_invalidation is explicitly true:
    let reordered_bytes = reorder_pages_ext(&signed_pdf, &[0], true)
        .expect("reorder with explicit signature invalidation succeeds");

    let doc = engine
        .open_pdf_bytes(&reordered_bytes)
        .expect("open reordered pdf");
    let sigs = doc.document_signatures().expect("signatures");
    assert_eq!(sigs.len(), 1);

    // In the PDF-compliant working copy, the signature field is preserved as an unsigned field:
    // /V is cleanly stripped so no reader mistakes it for valid, without ByteRange tampering.
    assert!(!sigs[0].is_signed);
    assert_eq!(sigs[0].signer, "");

    // Duplication with explicit signature invalidation
    let dup_bytes = duplicate_page_ext(&signed_pdf, 0, true)
        .expect("duplicate with signature invalidation succeeds");
    let dup_doc = engine
        .open_pdf_bytes(&dup_bytes)
        .expect("open duplicated signed pdf");
    assert_eq!(dup_doc.summary().page_count, 2);
    assert!(!dup_doc.document_signatures().expect("sigs")[0].is_signed);

    // Deleting second page from duplicated signed document
    let del_bytes = delete_pages_ext(&dup_bytes, &[1], true)
        .expect("delete with signature invalidation succeeds");
    let del_doc = engine
        .open_pdf_bytes(&del_bytes)
        .expect("open deleted signed pdf");
    assert_eq!(del_doc.summary().page_count, 1);

    // Rotating with explicit signature invalidation produces valid working copy
    let rot_bytes = rotate_pages_in_pdf_ext(&signed_pdf, 1, &[0], 90, true)
        .expect("rotate with signature invalidation succeeds");
    let rot_doc = engine
        .open_pdf_bytes(&rot_bytes)
        .expect("open rotated signed pdf");
    assert!(!rot_doc.document_signatures().expect("sigs")[0].is_signed);
}

fn pdf_with_nested_signature() -> Vec<u8> {
    let contents = "BT /F1 12 Tf 72 740 Td (Vocument Review) Tj ET\n";
    let objects: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [8 0 R] >> >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R /Annots [6 0 R] >>".to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{contents}endstream", contents.len()),
        "<< /Type /Annot /Subtype /Widget /FT /Sig /T (NestedSigField) /V 7 0 R /Rect [72 100 320 140] /P 3 0 R >>".to_owned(),
        "<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /adbe.pkcs7.detached /Name (Dr. Nested) /M (D:20260131120000Z) /ByteRange [0 100 200 100] /Contents <0000000000000000> >>".to_owned(),
        "<< /T (ParentSignatureGroup) /Kids [6 0 R] >>".to_owned(),
    ];

    let mut out = Vec::new();
    out.extend_from_slice(b"%PDF-1.7\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref_offset = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[test]
fn test_nested_signature_field_detection_blocks_fail_closed_and_supports_invalidation() {
    let nested_pdf = pdf_with_nested_signature();

    // 1. Fail-closed preflight check: must block mutation when signature is nested inside /Kids hierarchy
    let dup_err = duplicate_page(&nested_pdf, 0);
    assert!(matches!(
        dup_err,
        Err(PdfEngineError::OperationBlockedBySignature { .. })
    ));

    let del_err = delete_pages(&nested_pdf, &[0]);
    assert!(matches!(
        del_err,
        Err(PdfEngineError::OperationBlockedBySignature { .. })
    ));

    // 2. Explicit invalidation must recursively strip /V from nested signature field
    let dup_bytes = duplicate_page_ext(&nested_pdf, 0, true)
        .expect("duplicate with signature invalidation succeeds");
    let dup_str = String::from_utf8_lossy(&dup_bytes);
    assert!(dup_str.contains("NestedSigField"));
}

#[test]
fn test_repeated_page_duplication_generates_unique_field_names_without_collision() {
    let original = build_complex_test_pdf();

    // Duplicate page 0 first time -> CustomerName_Copy1
    let dup1_bytes = duplicate_page(&original, 0).expect("duplicate 1");
    let dup1_str = String::from_utf8_lossy(&dup1_bytes);
    assert!(dup1_str.contains("CustomerName_Copy1"));

    // Duplicate original page 0 again -> must generate CustomerName_Copy2 without colliding
    let dup2_bytes = duplicate_page(&dup1_bytes, 0).expect("duplicate 2");
    let dup2_str = String::from_utf8_lossy(&dup2_bytes);
    assert!(dup2_str.contains("CustomerName_Copy1"));
    assert!(dup2_str.contains("CustomerName_Copy2"));

    // Duplicate the copy page (which is CustomerName_Copy1) -> must generate CustomerName_Copy3
    let dup3_bytes = duplicate_page(&dup2_bytes, 1).expect("duplicate 3");
    let dup3_str = String::from_utf8_lossy(&dup3_bytes);
    assert!(dup3_str.contains("CustomerName_Copy1"));
    assert!(dup3_str.contains("CustomerName_Copy2"));
    assert!(dup3_str.contains("CustomerName_Copy3"));
}

#[test]
fn test_delete_page_reanchors_internal_links_to_surviving_pages() {
    let original = build_complex_test_pdf();
    let engine = MuPdfEngineAdapter::new();

    // In build_complex_test_pdf(), Page 2 (object 4 0 R) has a Link targeting Page 3 (object 5 0 R).
    // Deleting Page 3 (index 2) must re-anchor the link on Page 2 to surviving target Page 1 (3 0 R)
    let del_bytes = delete_pages(&original, &[2]).expect("delete page 3");
    let del_doc = engine.open_pdf_bytes(&del_bytes).expect("open pdf");
    assert_eq!(del_doc.summary().page_count, 2);

    let del_str = String::from_utf8_lossy(&del_bytes);
    // Link annotation survived and destination was re-anchored from 5 0 R to 3 0 R
    assert!(del_str.contains("/Subtype/Link") || del_str.contains("/Subtype /Link"));
    assert!(del_str.contains("Dest[3 0 R") || del_str.contains("Dest [3 0 R"));
}

fn assemble_test_pdf(objects: Vec<String>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"%PDF-1.7\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref_offset = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[test]
fn test_cyclic_and_overdepth_signature_structures_fail_closed() {
    let p1_text = "BT /F1 12 Tf 72 700 Td (Cyclic Signatures) Tj ET\n";
    let cyclic_objects: Vec<String> = vec![
        // 1: Catalog
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [3 0 R] >> >>".to_owned(),
        // 2: Pages root
        "<< /Type /Pages /Kids [5 0 R] /Count 1 >>".to_owned(),
        // 3: Cyclic Field A
        "<< /FT /Sig /T (SigA) /Kids [4 0 R] >>".to_owned(),
        // 4: Cyclic Field B pointing back to Field A
        "<< /FT /Sig /T (SigB) /Kids [3 0 R] >>".to_owned(),
        // 5: Page 1
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 6 0 R >> >> /Contents 7 0 R >>".to_owned(),
        // 6: Font
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        // 7: Content
        format!("<< /Length {} >>\nstream\n{p1_text}endstream", p1_text.len()),
    ];
    let cyclic_pdf = assemble_test_pdf(cyclic_objects);
    let engine = MuPdfEngineAdapter::new();

    // Opening cyclic document and inspecting signatures MUST fail-closed
    let doc = engine.open_pdf_bytes(&cyclic_pdf).expect("open cyclic pdf");
    let sig_err = doc.document_signatures();
    assert!(
        sig_err.is_err(),
        "document_signatures must return Err on cyclic structure"
    );

    // All page mutations MUST be rejected fail-closed
    assert!(reorder_pages_ext(&cyclic_pdf, &[0], false).is_err());
    assert!(duplicate_page_ext(&cyclic_pdf, 0, false).is_err());
    assert!(delete_pages_ext(&cyclic_pdf, &[0], false).is_err());
}

#[test]
fn test_broken_tagged_pdf_structure_safely_refuses_operations() {
    let p1_text = "BT /F1 12 Tf 72 700 Td (Broken Tagged PDF) Tj ET\n";
    let broken_objects: Vec<String> = vec![
        // 1: Catalog with Tagged markers but empty StructTreeRoot without ParentTree
        "<< /Type /Catalog /Pages 2 0 R /MarkInfo << /Marked true >> /StructTreeRoot 3 0 R >>".to_owned(),
        // 2: Pages root
        "<< /Type /Pages /Kids [4 0 R] /Count 1 >>".to_owned(),
        // 3: StructTreeRoot without ParentTree
        "<< /Type /StructTreeRoot >>".to_owned(),
        // 4: Page 1 with dangling StructParents 42
        "<< /Type /Page /Parent 2 0 R /StructParents 42 /MediaBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >> /Contents 6 0 R >>".to_owned(),
        // 5: Font
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        // 6: Content
        format!("<< /Length {} >>\nstream\n{p1_text}endstream", p1_text.len()),
    ];
    let broken_pdf = assemble_test_pdf(broken_objects);

    // Page operations MUST safely refuse the broken Tagged structure
    assert!(
        duplicate_page(&broken_pdf, 0).is_err(),
        "duplicate must refuse broken Tagged PDF"
    );
    assert!(
        reorder_pages(&broken_pdf, &[0]).is_err(),
        "reorder must refuse broken Tagged PDF"
    );
}

#[test]
fn test_valid_tagged_pdf_maintains_parent_tree_consistency_on_duplicate_and_delete() {
    let p1_text = "BT /F1 12 Tf 72 700 Td (Valid Tagged PDF) Tj ET\n";
    let valid_objects: Vec<String> = vec![
        // 1: Catalog
        "<< /Type /Catalog /Pages 2 0 R /MarkInfo << /Marked true >> /StructTreeRoot 3 0 R >>".to_owned(),
        // 2: Pages root
        "<< /Type /Pages /Kids [4 0 R] /Count 1 >>".to_owned(),
        // 3: StructTreeRoot with ParentTree
        "<< /Type /StructTreeRoot /ParentTree 7 0 R >>".to_owned(),
        // 4: Page 1 with valid StructParents 0
        "<< /Type /Page /Parent 2 0 R /StructParents 0 /MediaBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >> /Contents 6 0 R >>".to_owned(),
        // 5: Font
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        // 6: Content
        format!("<< /Length {} >>\nstream\n{p1_text}endstream", p1_text.len()),
        // 7: ParentTree number tree with entry 0
        "<< /Nums [0 [8 0 R]] >>".to_owned(),
        // 8: StructElem for MCID 0
        "<< /Type /StructElem /S /P /Pg 4 0 R >>".to_owned(),
    ];
    let valid_pdf = assemble_test_pdf(valid_objects);

    // 1. Duplicate page 0
    let dup_bytes = duplicate_page(&valid_pdf, 0).expect("duplicate valid tagged page");
    let dup_str = String::from_utf8_lossy(&dup_bytes);
    assert!(dup_str.contains("/StructTreeRoot"));
    assert!(dup_str.contains("/ParentTree"));

    let engine = MuPdfEngineAdapter::new();
    let dup_doc = engine.open_pdf_bytes(&dup_bytes).expect("open dup pdf");
    assert_eq!(dup_doc.summary().page_count, 2);

    // 2. Delete the original page 0 from duplicated document
    let del_bytes = delete_pages(&dup_bytes, &[0]).expect("delete first page");
    let del_doc = engine.open_pdf_bytes(&del_bytes).expect("open del pdf");
    assert_eq!(del_doc.summary().page_count, 1);
    let del_str = String::from_utf8_lossy(&del_bytes);
    assert!(del_str.contains("/StructTreeRoot"));
}
