//! Tests of the document outline detection (`ARCHITECTURE.md` §21).
//!
//! The fixture is a photographed page: tilted, shadowed and distorted. The
//! detection has to find the outline of the page, not a rectangle around it.

use super::*;
use crate::geometry::Point2D;

/// The corners of the document in the camera distorted corpus fixture.
///
/// The values come from the generator of the fixture and describe the page
/// inside the frame.
fn reference_corners() -> QuadCorners {
    QuadCorners {
        top_left: Point2D::new(233.32 / 1500.0, 82.95 / 2050.0),
        top_right: Point2D::new(1465.35 / 1500.0, 223.33 / 2050.0),
        bottom_right: Point2D::new(1266.68 / 1500.0, 1967.05 / 2050.0),
        bottom_left: Point2D::new(34.65 / 1500.0, 1826.67 / 2050.0),
        confidence_milli: 900,
    }
}

fn corpus_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/ocr/deu_vertrag_vertraulichkeit_camera_distorted.pgm")
}

/// Loads the corpus frame and converts it into a scan frame.
///
/// The parser is a small local one: the imaging crate must not depend on the
/// OCR crate, because the OCR crate already depends on this one.
fn corpus_frame() -> crate::ScanFrame {
    let bytes = std::fs::read(corpus_path()).expect("the corpus frame must be readable");
    assert!(
        &bytes[0..2] == b"P5",
        "the corpus frame must be a binary P5 PGM"
    );
    let mut index = 2usize;
    let mut tokens: Vec<std::ops::Range<usize>> = Vec::new();
    while tokens.len() < 3 && index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b'#' {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        let start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        tokens.push(start..index);
    }
    let width: u32 = std::str::from_utf8(&bytes[tokens[0].clone()])
        .expect("width")
        .parse()
        .expect("width is a number");
    let height: u32 = std::str::from_utf8(&bytes[tokens[1].clone()])
        .expect("height")
        .parse()
        .expect("height is a number");
    index += 1;
    let pixels = bytes[index..].to_vec();
    assert_eq!(pixels.len(), (width * height) as usize);

    crate::ScanFrame {
        width,
        height,
        stride: width,
        format: crate::PixelFormat::Gray8,
        orientation_degrees: 0,
        pixels,
        metadata: crate::ScanFrameMetadata {
            timestamp_ms: 0,
            device_angle_degrees: 0.0,
            brightness_hint: 0.0,
            is_steady: true,
        },
    }
}

#[test]
fn the_detection_finds_the_tilted_outline_of_the_page() {
    let frame = corpus_frame();
    let detected = detect_outline(&frame).expect("the page must be found in the frame");
    let reference = reference_corners();

    // Every corner has to be close to the reference. A detection that only
    // returns a rectangle around the page would be off by far more than this.
    let tolerance = 0.05;
    for (name, found, expected) in [
        ("top_left", detected.top_left, reference.top_left),
        ("top_right", detected.top_right, reference.top_right),
        (
            "bottom_right",
            detected.bottom_right,
            reference.bottom_right,
        ),
        ("bottom_left", detected.bottom_left, reference.bottom_left),
    ] {
        let dx = (found.x - expected.x).abs();
        let dy = (found.y - expected.y).abs();
        assert!(
            dx <= tolerance && dy <= tolerance,
            "{name} is off: found ({:.3},{:.3}), expected ({:.3},{:.3})",
            found.x,
            found.y,
            expected.x,
            expected.y
        );
    }
}

#[test]
fn the_detection_is_not_a_rectangle_around_the_page() {
    let frame = corpus_frame();
    let detected = detect_outline(&frame).expect("the page must be found in the frame");
    let reference = reference_corners();

    // The page in the fixture is sheared: the top edge and the bottom edge are
    // shifted against each other. A detection that ignores the tilt would
    // return a symmetric rectangle.
    let reference_shear = (reference.top_left.x - reference.bottom_left.x).abs();
    let detected_shear = (detected.top_left.x - detected.bottom_left.x).abs();
    assert!(
        (reference_shear - detected_shear).abs() < 0.08,
        "the detection must reproduce the shear of the page: reference {reference_shear:.3}, detected {detected_shear:.3}"
    );
}
