use super::MuPdfEngineAdapter;
use super::attachment_signature_tests;
use super::geometry_ops::*;
use super::tests::sample_two_page_pdf_bytes;
use std::path::PathBuf;
use vocument_core::pdf_engine::{PdfEngine, PdfEngineError, PdfRectMilliPt, PdfTileRenderRequest};

#[test]
fn test_get_page_boxes_reads_standard_boxes_and_rotation() {
    let pdf_bytes = sample_two_page_pdf_bytes();
    let boxes = get_page_boxes(&pdf_bytes, 0).expect("read page 0 boxes");

    assert_eq!(boxes.media_box, [0.0, 0.0, 595.0, 842.0]);
    assert_eq!(boxes.crop_box, [0.0, 0.0, 595.0, 842.0]);
    assert_eq!(boxes.bleed_box, [0.0, 0.0, 595.0, 842.0]);
    assert_eq!(boxes.trim_box, [0.0, 0.0, 595.0, 842.0]);
    assert_eq!(boxes.art_box, [0.0, 0.0, 595.0, 842.0]);
    assert_eq!(boxes.rotation_degrees, 0);

    let boxes_page1 = get_page_boxes(&pdf_bytes, 1).expect("read page 1 boxes");
    assert_eq!(boxes_page1.media_box, [0.0, 0.0, 595.0, 842.0]);
}

#[test]
fn test_set_page_boxes_updates_all_five_boxes() {
    let pdf_bytes = sample_two_page_pdf_bytes();

    let new_media = [0.0, 0.0, 600.0, 800.0];
    let new_crop = [10.0, 10.0, 590.0, 790.0];
    let new_bleed = [15.0, 15.0, 585.0, 785.0];
    let new_trim = [20.0, 20.0, 580.0, 780.0];
    let new_art = [30.0, 30.0, 570.0, 770.0];

    let updated = set_page_boxes_ext(
        &pdf_bytes,
        &[0],
        Some(new_media),
        Some(new_crop),
        Some(new_bleed),
        Some(new_trim),
        Some(new_art),
        false,
    )
    .expect("set page boxes on page 0");

    let read_back = get_page_boxes(&updated, 0).expect("read back updated boxes");
    assert_eq!(read_back.media_box, new_media);
    assert_eq!(read_back.crop_box, new_crop);
    assert_eq!(read_back.bleed_box, new_bleed);
    assert_eq!(read_back.trim_box, new_trim);
    assert_eq!(read_back.art_box, new_art);

    // Page 1 should remain unaffected
    let page1_boxes = get_page_boxes(&updated, 1).expect("read back page 1 boxes");
    assert_eq!(page1_boxes.media_box, [0.0, 0.0, 595.0, 842.0]);
}

#[test]
fn test_display_crop_is_non_destructive() {
    let pdf_bytes = sample_two_page_pdf_bytes();
    let crop_rect = [50.0, 50.0, 500.0, 700.0];

    let cropped_bytes = apply_display_crop_ext(&pdf_bytes, &[0], crop_rect, false)
        .expect("apply non-destructive display crop");

    let boxes = get_page_boxes(&cropped_bytes, 0).expect("read back cropped page 0");
    // MediaBox remains unchanged (non-destructive)
    assert_eq!(boxes.media_box, [0.0, 0.0, 595.0, 842.0]);
    // CropBox reflects the cropped area
    assert_eq!(boxes.crop_box, crop_rect);

    // Verify engine can render cropped page
    let adapter = MuPdfEngineAdapter::new();
    let doc = adapter
        .open_pdf_bytes(&cropped_bytes)
        .expect("open cropped PDF");
    let dl = doc.create_display_list(0).expect("create display list");
    let tile = dl
        .render_tile(PdfTileRenderRequest {
            page_index: 0,
            scale_milli: 1000,
            tile_x_px: 0,
            tile_y_px: 0,
            tile_width_px: 64,
            tile_height_px: 64,
        })
        .expect("render tile");
    assert_eq!(tile.width_px, 64);
}

#[test]
fn test_hard_crop_clips_and_prunes() {
    let pdf_bytes = sample_two_page_pdf_bytes();
    let crop_rect = [50.0, 50.0, 400.0, 600.0];

    let hard_cropped = apply_hard_crop_ext(&pdf_bytes, &[0], crop_rect, false)
        .expect("apply destructive hard crop");

    let boxes = get_page_boxes(&hard_cropped, 0).expect("read back hard cropped page 0");
    // Both MediaBox and CropBox are clamped to the hard crop rect
    assert_eq!(boxes.media_box, crop_rect);
    assert_eq!(boxes.crop_box, crop_rect);

    // Verify PDF remains valid and renderable
    let adapter = MuPdfEngineAdapter::new();
    let doc = adapter
        .open_pdf_bytes(&hard_cropped)
        .expect("open hard-cropped PDF");
    let dl = doc.create_display_list(0).expect("create display list");
    let tile = dl
        .render_tile(PdfTileRenderRequest {
            page_index: 0,
            scale_milli: 1000,
            tile_x_px: 0,
            tile_y_px: 0,
            tile_width_px: 64,
            tile_height_px: 64,
        })
        .expect("render tile on hard-cropped PDF");
    assert_eq!(tile.width_px, 64);
}

#[test]
fn test_deskew_angle_detection_and_matrix_deskew() {
    let pdf_bytes = sample_two_page_pdf_bytes();

    let detected_angle = detect_deskew_angle(&pdf_bytes, 0).expect("detect deskew angle");
    assert!(
        detected_angle.abs() < 1.0,
        "clean synthetic document should have negligible skew: {detected_angle}"
    );

    // Apply deskew rotation
    let deskewed = apply_deskew_ext(&pdf_bytes, &[0], 3.0, false).expect("apply deskew rotation");

    let adapter = MuPdfEngineAdapter::new();
    let doc = adapter
        .open_pdf_bytes(&deskewed)
        .expect("open deskewed PDF");
    let dl = doc.create_display_list(0).expect("create display list");
    let tile = dl
        .render_tile(PdfTileRenderRequest {
            page_index: 0,
            scale_milli: 1000,
            tile_x_px: 0,
            tile_y_px: 0,
            tile_width_px: 64,
            tile_height_px: 64,
        })
        .expect("render tile on deskewed PDF");
    assert_eq!(tile.width_px, 64);
}

#[test]
fn test_auto_crop_content_margins() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/m0_3_navigation_and_search.pdf");
    let bytes = std::fs::read(&fixture_path).expect("read M0.3 fixture");

    let original_boxes = get_page_boxes(&bytes, 0).expect("original boxes");
    let safety_margin = 15.0;
    let content_margins =
        detect_content_margins(&bytes, 0, safety_margin).expect("detect content margins");

    // Margins should be valid non-empty rect within MediaBox
    assert!(content_margins[0] < content_margins[2]);
    assert!(content_margins[1] < content_margins[3]);
    assert!(content_margins[0] >= original_boxes.media_box[0]);
    assert!(content_margins[2] <= original_boxes.media_box[2]);

    // Apply auto crop
    let auto_cropped =
        apply_auto_crop_ext(&bytes, &[0], safety_margin, false).expect("apply auto crop");

    let auto_boxes = get_page_boxes(&auto_cropped, 0).expect("auto cropped boxes");
    assert_eq!(auto_boxes.crop_box, content_margins);
}

#[test]
fn test_geometry_operations_fail_closed_on_signed_pdf() {
    let signed_pdf = attachment_signature_tests::pdf_with_attachment_and_signature(false);

    // 1. set_page_boxes fail-closed
    let err_set = set_page_boxes_ext(
        &signed_pdf,
        &[0],
        None,
        Some([10.0, 10.0, 500.0, 700.0]),
        None,
        None,
        None,
        false,
    );
    assert!(matches!(
        err_set,
        Err(PdfEngineError::OperationBlockedBySignature { .. })
    ));

    // 2. apply_hard_crop fail-closed
    let err_crop = apply_hard_crop_ext(&signed_pdf, &[0], [10.0, 10.0, 500.0, 700.0], false);
    assert!(matches!(
        err_crop,
        Err(PdfEngineError::OperationBlockedBySignature { .. })
    ));

    // 3. apply_deskew fail-closed
    let err_deskew = apply_deskew_ext(&signed_pdf, &[0], 2.5, false);
    assert!(matches!(
        err_deskew,
        Err(PdfEngineError::OperationBlockedBySignature { .. })
    ));

    // Explicit signature invalidation succeeds
    let ok_set = set_page_boxes_ext(
        &signed_pdf,
        &[0],
        None,
        Some([10.0, 10.0, 500.0, 700.0]),
        None,
        None,
        None,
        true,
    )
    .expect("succeed with signature invalidation");
    let boxes = get_page_boxes(&ok_set, 0).expect("read boxes");
    assert_eq!(boxes.crop_box, [10.0, 10.0, 500.0, 700.0]);
}

#[test]
fn test_coordinate_conversion_roundtrip() {
    let media_box = [0.0f32, 0.0f32, 595.0f32, 842.0f32];
    let rect_milli = PdfRectMilliPt {
        x0_milli_pt: 50_000,
        y0_milli_pt: 100_000,
        x1_milli_pt: 450_000,
        y1_milli_pt: 700_000,
    };

    let pts = milli_pt_rect_to_pdf_points(rect_milli, media_box);
    assert_eq!(pts[0], 50.0);
    assert_eq!(pts[2], 450.0);
    assert_eq!(pts[1], 842.0 - 700.0);
    assert_eq!(pts[3], 842.0 - 100.0);

    let roundtripped = pdf_points_to_milli_pt_rect(pts, media_box);
    assert_eq!(roundtripped, rect_milli);
}
