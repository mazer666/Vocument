//! Shared helpers of the DOCX and ODT adapters: units, link safety, image sniffing, identifiers.

use sha2::{Digest, Sha256};
use vocument_flow::styles::ColorRgba;

/// Points per twip (1/20 pt), the unit of WordprocessingML lengths.
const POINTS_PER_TWIP: f32 = 1.0 / 20.0;
/// Points per EMU (English Metric Unit), the unit of DrawingML extents.
const POINTS_PER_EMU: f32 = 1.0 / 12_700.0;

pub fn twips_to_pt(twips: i64) -> f32 {
    twips as f32 * POINTS_PER_TWIP
}

pub fn pt_to_twips(points: f32) -> i64 {
    (points * 20.0).round() as i64
}

pub fn emu_to_pt(emu: i64) -> f32 {
    emu as f32 * POINTS_PER_EMU
}

pub fn pt_to_emu(points: f32) -> i64 {
    (points * 12_700.0).round() as i64
}

/// Parses an ODF/CSS length such as `2.54cm`, `10mm`, `1in`, `12pt`, `6pc`, `16px` into points.
pub fn parse_length_pt(text: &str) -> Option<f32> {
    let text = text.trim();
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+')))
        .unwrap_or(text.len());
    let value: f32 = text[..split].parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    let factor = match &text[split..] {
        "pt" | "" => 1.0,
        "cm" => 72.0 / 2.54,
        "mm" => 72.0 / 25.4,
        "in" => 72.0,
        "pc" => 12.0,
        "px" => 0.75,
        _ => return None,
    };
    Some(value * factor)
}

/// Formats points as an ODF length in centimetres with fixed precision (deterministic output).
pub fn format_length_cm(points: f32) -> String {
    format!("{:.3}cm", points * 2.54 / 72.0)
}

/// Formats points with at most three decimals and no trailing zeros.
pub fn format_pt(points: f32) -> String {
    let text = format!("{points:.3}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Parses `RRGGBB` or `#RRGGBB`; the OOXML value `auto` yields `None`.
pub fn parse_hex_color(text: &str) -> Option<ColorRgba> {
    let hex = text.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    ColorRgba::from_hex(&format!("#{hex}"))
}

/// Hex digits `RRGGBB` of a colour, upper case, without alpha.
pub fn color_hex_rgb(color: ColorRgba) -> String {
    format!("{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

/// Returns the link when its scheme is safe to store and open, else `None`.
///
/// Allowed: `http`, `https`, `mailto`, `tel` and in-document anchors (`#name`). Everything else
/// — `file:`, `javascript:`, `data:`, `smb:`, UNC paths, relative file paths, application
/// schemes — is refused: a document must not be able to point the reader at local resources.
pub fn safe_hyperlink(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() || url.len() > 4096 || url.chars().any(|c| c.is_control()) {
        return None;
    }
    if url.starts_with('#') {
        return Some(url.to_string());
    }
    let (scheme, _) = url.split_once(':')?;
    match scheme.to_ascii_lowercase().as_str() {
        "http" | "https" | "mailto" | "tel" => Some(url.to_string()),
        _ => None,
    }
}

/// Facts recovered from an image header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInfo {
    pub media_type: &'static str,
    pub extension: &'static str,
    /// Pixel size, when the header yields one.
    pub pixel_size: Option<(u32, u32)>,
}

/// Identifies an image by its magic bytes. Content, not the file name, decides.
pub fn sniff_image(bytes: &[u8]) -> Option<ImageInfo> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24 {
        let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        return Some(ImageInfo {
            media_type: "image/png",
            extension: "png",
            pixel_size: Some((width, height)),
        });
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return Some(ImageInfo {
            media_type: "image/jpeg",
            extension: "jpg",
            pixel_size: jpeg_size(bytes),
        });
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        let size = (bytes.len() >= 10).then(|| {
            (
                u32::from(u16::from_le_bytes([bytes[6], bytes[7]])),
                u32::from(u16::from_le_bytes([bytes[8], bytes[9]])),
            )
        });
        return Some(ImageInfo {
            media_type: "image/gif",
            extension: "gif",
            pixel_size: size,
        });
    }
    if bytes.starts_with(b"BM") && bytes.len() >= 26 {
        let width = i32::from_le_bytes(bytes[18..22].try_into().ok()?);
        let height = i32::from_le_bytes(bytes[22..26].try_into().ok()?);
        return Some(ImageInfo {
            media_type: "image/bmp",
            extension: "bmp",
            pixel_size: u32::try_from(width)
                .ok()
                .map(|w| (w, height.unsigned_abs())),
        });
    }
    if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        return Some(ImageInfo {
            media_type: "image/tiff",
            extension: "tiff",
            pixel_size: None,
        });
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
    if head.contains("<svg") {
        return Some(ImageInfo {
            media_type: "image/svg+xml",
            extension: "svg",
            pixel_size: None,
        });
    }
    if bytes.starts_with(&[0xD7, 0xCD, 0xC6, 0x9A]) {
        return Some(ImageInfo {
            media_type: "image/x-wmf",
            extension: "wmf",
            pixel_size: None,
        });
    }
    if bytes.len() > 44 && bytes[0..4] == [1, 0, 0, 0] && &bytes[40..44] == b" EMF" {
        return Some(ImageInfo {
            media_type: "image/x-emf",
            extension: "emf",
            pixel_size: None,
        });
    }
    None
}

fn jpeg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut index = 2;
    while index + 9 < bytes.len() {
        if bytes[index] != 0xFF {
            index += 1;
            continue;
        }
        let marker = bytes[index + 1];
        if marker == 0xFF {
            index += 1;
            continue;
        }
        // Start-of-frame markers carry the dimensions.
        if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            let height = u32::from(u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]));
            let width = u32::from(u16::from_be_bytes([bytes[index + 7], bytes[index + 8]]));
            return Some((width, height));
        }
        let length = usize::from(u16::from_be_bytes([bytes[index + 2], bytes[index + 3]]));
        index += 2 + length.max(2);
    }
    None
}

/// Content-addressed asset path `assets/<sha256>.<extension>`, the `.pdoc` convention.
pub fn content_addressed_path(extension: &str, bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("assets/{digest:x}.{extension}")
}

/// Allocates stable, unique identifiers with a per-kind counter: `p_1`, `p_2`, `tbl_1` …
#[derive(Debug, Default)]
pub struct IdGenerator {
    counters: std::collections::BTreeMap<&'static str, u32>,
}

impl IdGenerator {
    pub fn next(&mut self, prefix: &'static str) -> String {
        let counter = self.counters.entry(prefix).or_insert(0);
        *counter += 1;
        format!("{prefix}_{counter}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_conversions_are_exact_where_defined() {
        assert_eq!(twips_to_pt(1440), 72.0);
        assert_eq!(pt_to_twips(72.0), 1440);
        assert_eq!(emu_to_pt(914_400), 72.0);
        assert_eq!(pt_to_emu(72.0), 914_400);
        assert!((parse_length_pt("2.54cm").expect("cm") - 72.0).abs() < 0.01);
        assert_eq!(parse_length_pt("1in"), Some(72.0));
        assert_eq!(parse_length_pt("12pt"), Some(12.0));
        assert_eq!(parse_length_pt("3furlong"), None);
        assert_eq!(parse_length_pt("nan"), None);
        assert_eq!(format_pt(12.5), "12.5");
        assert_eq!(format_pt(12.0), "12");
        assert_eq!(format_pt(0.0), "0");
    }

    #[test]
    fn only_safe_link_schemes_survive() {
        for safe in [
            "https://example.org/a?b=c",
            "http://x",
            "mailto:a@b.de",
            "#bookmark",
        ] {
            assert_eq!(safe_hyperlink(safe).as_deref(), Some(safe));
        }
        for unsafe_link in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html;base64,AAAA",
            "\\\\server\\share\\x.docx",
            "smb://host/share",
            "other.docx",
            "ms-word:ofe|u|http://x",
            "",
            "http://a\nb",
        ] {
            assert_eq!(safe_hyperlink(unsafe_link), None, "{unsafe_link}");
        }
    }

    #[test]
    fn sniffs_images_by_content() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&640u32.to_be_bytes());
        png.extend_from_slice(&480u32.to_be_bytes());
        let info = sniff_image(&png).expect("png");
        assert_eq!(info.media_type, "image/png");
        assert_eq!(info.pixel_size, Some((640, 480)));

        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x01, 0xE0, 0x02, 0x80, 0x03, 0x01, 0x22,
            0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01,
        ];
        assert_eq!(
            sniff_image(&jpeg).expect("jpeg").pixel_size,
            Some((640, 480))
        );
        assert!(sniff_image(b"not an image").is_none());
    }

    #[test]
    fn identifiers_are_unique_per_kind() {
        let mut ids = IdGenerator::default();
        assert_eq!(ids.next("p"), "p_1");
        assert_eq!(ids.next("p"), "p_2");
        assert_eq!(ids.next("tbl"), "tbl_1");
    }
}
