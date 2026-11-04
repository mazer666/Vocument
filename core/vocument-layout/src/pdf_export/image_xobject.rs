//! Image XObject embedding for Tagged PDF/A-2b and PDF/UA-1 export (`M4.4`, `ADR-0020`).
//!
//! Handles JPEG (via `/DCTDecode`) and PNG / raster image data (via deflated RGB streams).

use miniz_oxide::deflate::compress_to_vec_zlib;

/// Maximum allowable pixel dimension for embedded raster images (resource limit: 16k x 16k).
pub const MAX_IMAGE_DIMENSION_PX: u32 = 16_384;

/// Maximum payload size in bytes for an embedded image (50 MB).
pub const MAX_IMAGE_PAYLOAD_BYTES: usize = 50 * 1024 * 1024;

/// Maximum allowable memory footprint for decompressed raster bitmap data (64 MB).
/// Protects against decompression bombs and excessive memory consumption.
pub const MAX_DECOMPRESSED_IMAGE_BYTES: usize = 64 * 1024 * 1024;

/// Errors occurring during image decoding, validation, or PDF XObject preparation.
#[derive(Debug, thiserror::Error, PartialEq, Eq, Clone)]
pub enum ImagePreparationError {
    #[error("unsupported image format (only JPEG and PNG raster images are supported)")]
    UnsupportedFormat,
    #[error(
        "image exceeds maximum allowed dimensions: {width}x{height} (limit: {MAX_IMAGE_DIMENSION_PX}x{MAX_IMAGE_DIMENSION_PX})"
    )]
    DimensionsExceeded { width: u32, height: u32 },
    #[error(
        "image payload exceeds size limit ({size_bytes} bytes, limit: {MAX_IMAGE_PAYLOAD_BYTES} bytes)"
    )]
    PayloadTooLarge { size_bytes: usize },
    #[error("corrupt or malformed image data: {reason}")]
    CorruptData { reason: String },
}

/// Prepared raster image XObject ready to be written to a PDF stream.
#[derive(Debug, Clone)]
pub struct PdfImageObject {
    pub width: u32,
    pub height: u32,
    pub color_space: &'static str,
    pub filter: &'static str,
    pub stream_bytes: Vec<u8>,
    pub smask_bytes: Option<Vec<u8>>,
}

/// Prepares raw image bytes (JPEG, PNG) into a PDF-compliant XObject with strict resource limits.
pub fn prepare_image(bytes: &[u8]) -> Result<PdfImageObject, ImagePreparationError> {
    if bytes.len() > MAX_IMAGE_PAYLOAD_BYTES {
        return Err(ImagePreparationError::PayloadTooLarge {
            size_bytes: bytes.len(),
        });
    }

    if bytes.starts_with(&[0xFF, 0xD8]) {
        return prepare_jpeg(bytes);
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return prepare_png(bytes);
    }

    Err(ImagePreparationError::UnsupportedFormat)
}

/// Fallback 8x8 neutral gray raster placeholder image when asset is missing or invalid.
pub fn placeholder_image() -> PdfImageObject {
    let width = 8u32;
    let height = 8u32;
    let mut raw_rgb = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            let val = if (x + y) % 2 == 0 { 210u8 } else { 235u8 };
            raw_rgb.extend_from_slice(&[val, val, val]);
        }
    }
    let compressed = compress_to_vec_zlib(&raw_rgb, 6);
    PdfImageObject {
        width,
        height,
        color_space: "/DeviceRGB",
        filter: "/FlateDecode",
        stream_bytes: compressed,
        smask_bytes: None,
    }
}

fn prepare_jpeg(bytes: &[u8]) -> Result<PdfImageObject, ImagePreparationError> {
    let (width, height, is_gray) =
        parse_jpeg_dimensions(bytes).ok_or_else(|| ImagePreparationError::CorruptData {
            reason: "unable to parse JPEG Start-of-Frame dimensions".to_string(),
        })?;

    if width > MAX_IMAGE_DIMENSION_PX || height > MAX_IMAGE_DIMENSION_PX {
        return Err(ImagePreparationError::DimensionsExceeded { width, height });
    }

    let estimated_decompressed = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(if is_gray { 1 } else { 3 });
    if estimated_decompressed > MAX_DECOMPRESSED_IMAGE_BYTES {
        return Err(ImagePreparationError::PayloadTooLarge {
            size_bytes: estimated_decompressed,
        });
    }

    Ok(PdfImageObject {
        width,
        height,
        color_space: if is_gray { "/DeviceGray" } else { "/DeviceRGB" },
        filter: "/DCTDecode",
        stream_bytes: bytes.to_vec(),
        smask_bytes: None,
    })
}

fn parse_jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32, bool)> {
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
            let components = bytes[index + 9];
            let is_gray = components == 1;
            return Some((width, height, is_gray));
        }
        let length = usize::from(u16::from_be_bytes([bytes[index + 2], bytes[index + 3]]));
        index += 2 + length.max(2);
    }
    None
}

fn prepare_png(bytes: &[u8]) -> Result<PdfImageObject, ImagePreparationError> {
    if bytes.len() < 33 {
        return Err(ImagePreparationError::CorruptData {
            reason: "PNG stream truncated before IHDR header".to_string(),
        });
    }
    // Parse IHDR
    let width = u32::from_be_bytes(bytes[16..20].try_into().map_err(|_| {
        ImagePreparationError::CorruptData {
            reason: "failed to read PNG width".to_string(),
        }
    })?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().map_err(|_| {
        ImagePreparationError::CorruptData {
            reason: "failed to read PNG height".to_string(),
        }
    })?);
    let bit_depth = bytes[24];
    let color_type = bytes[25];

    if width == 0 || height == 0 {
        return Err(ImagePreparationError::CorruptData {
            reason: "PNG dimensions must be non-zero".to_string(),
        });
    }
    if width > MAX_IMAGE_DIMENSION_PX || height > MAX_IMAGE_DIMENSION_PX {
        return Err(ImagePreparationError::DimensionsExceeded { width, height });
    }
    if bit_depth != 8 {
        return Err(ImagePreparationError::UnsupportedFormat);
    }

    let channels: usize = match color_type {
        0 => 1,
        2 => 3,
        6 => 4,
        _ => return Err(ImagePreparationError::UnsupportedFormat),
    };

    let expected_scanline = (width as usize).saturating_mul(channels).saturating_add(1);
    let expected_decompressed = expected_scanline.saturating_mul(height as usize);
    if expected_decompressed > MAX_DECOMPRESSED_IMAGE_BYTES {
        return Err(ImagePreparationError::PayloadTooLarge {
            size_bytes: expected_decompressed,
        });
    }

    // Collect all IDAT chunks
    let mut idat_data = Vec::new();
    let mut offset = 8;
    while offset + 8 <= bytes.len() {
        let chunk_len = u32::from_be_bytes(bytes[offset..offset + 4].try_into().map_err(|_| {
            ImagePreparationError::CorruptData {
                reason: "failed to read PNG chunk length".to_string(),
            }
        })?) as usize;
        let chunk_type = &bytes[offset + 4..offset + 8];
        if chunk_type == b"IDAT" && offset + 8 + chunk_len <= bytes.len() {
            idat_data.extend_from_slice(&bytes[offset + 8..offset + 8 + chunk_len]);
        }
        offset += 12 + chunk_len;
    }

    if idat_data.is_empty() {
        return Err(ImagePreparationError::CorruptData {
            reason: "PNG contains no IDAT data chunks".to_string(),
        });
    }

    let decompressed = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
        &idat_data,
        MAX_DECOMPRESSED_IMAGE_BYTES,
    )
    .map_err(|e| ImagePreparationError::CorruptData {
        reason: format!(
            "zlib inflate failed on PNG IDAT (limit {MAX_DECOMPRESSED_IMAGE_BYTES} bytes): {e:?}"
        ),
    })?;

    match color_type {
        // Grayscale 8-bit
        0 => {
            let uncompressed = unfilter_png(&decompressed, width as usize, height as usize, 1)
                .ok_or_else(|| ImagePreparationError::CorruptData {
                    reason: "PNG row filter reconstruction failed".to_string(),
                })?;
            let compressed = compress_to_vec_zlib(&uncompressed, 6);
            Ok(PdfImageObject {
                width,
                height,
                color_space: "/DeviceGray",
                filter: "/FlateDecode",
                stream_bytes: compressed,
                smask_bytes: None,
            })
        }
        // Truecolor (RGB) 8-bit
        2 => {
            let uncompressed = unfilter_png(&decompressed, width as usize, height as usize, 3)
                .ok_or_else(|| ImagePreparationError::CorruptData {
                    reason: "PNG row filter reconstruction failed".to_string(),
                })?;
            let compressed = compress_to_vec_zlib(&uncompressed, 6);
            Ok(PdfImageObject {
                width,
                height,
                color_space: "/DeviceRGB",
                filter: "/FlateDecode",
                stream_bytes: compressed,
                smask_bytes: None,
            })
        }
        // Truecolor with alpha (RGBA) 8-bit
        6 => {
            let uncompressed = unfilter_png(&decompressed, width as usize, height as usize, 4)
                .ok_or_else(|| ImagePreparationError::CorruptData {
                    reason: "PNG row filter reconstruction failed".to_string(),
                })?;
            let pixel_count = (width * height) as usize;
            let mut rgb = Vec::with_capacity(pixel_count * 3);
            let mut alpha = Vec::with_capacity(pixel_count);
            for chunk in uncompressed.chunks_exact(4) {
                rgb.extend_from_slice(&chunk[0..3]);
                alpha.push(chunk[3]);
            }
            let compressed_rgb = compress_to_vec_zlib(&rgb, 6);
            let compressed_alpha = compress_to_vec_zlib(&alpha, 6);
            Ok(PdfImageObject {
                width,
                height,
                color_space: "/DeviceRGB",
                filter: "/FlateDecode",
                stream_bytes: compressed_rgb,
                smask_bytes: Some(compressed_alpha),
            })
        }
        _ => Err(ImagePreparationError::UnsupportedFormat),
    }
}

fn unfilter_png(data: &[u8], width: usize, height: usize, bpp: usize) -> Option<Vec<u8>> {
    let row_stride = width * bpp;
    let expected_len = height * (1 + row_stride);
    if data.len() < expected_len {
        return None;
    }

    let mut out = vec![0u8; width * height * bpp];
    let mut prev_row = vec![0u8; row_stride];

    for y in 0..height {
        let src_offset = y * (1 + row_stride);
        let filter_type = data[src_offset];
        let row_src = &data[src_offset + 1..src_offset + 1 + row_stride];
        let dst_offset = y * row_stride;
        let row_dst = &mut out[dst_offset..dst_offset + row_stride];

        match filter_type {
            // None
            0 => {
                row_dst.copy_from_slice(row_src);
            }
            // Sub
            1 => {
                for i in 0..row_stride {
                    let a = if i >= bpp { row_dst[i - bpp] } else { 0 };
                    row_dst[i] = row_src[i].wrapping_add(a);
                }
            }
            // Up
            2 => {
                for i in 0..row_stride {
                    row_dst[i] = row_src[i].wrapping_add(prev_row[i]);
                }
            }
            // Average
            3 => {
                for i in 0..row_stride {
                    let a = if i >= bpp { row_dst[i - bpp] } else { 0 };
                    let b = prev_row[i];
                    let avg = ((u16::from(a) + u16::from(b)) / 2) as u8;
                    row_dst[i] = row_src[i].wrapping_add(avg);
                }
            }
            // Paeth
            4 => {
                for i in 0..row_stride {
                    let a = if i >= bpp { row_dst[i - bpp] } else { 0 };
                    let b = prev_row[i];
                    let c = if i >= bpp { prev_row[i - bpp] } else { 0 };
                    row_dst[i] = row_src[i].wrapping_add(paeth_predictor(a, b, c));
                }
            }
            _ => return None,
        }
        prev_row.copy_from_slice(row_dst);
    }

    Some(out)
}

fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let p = i32::from(a) + i32::from(b) - i32::from(c);
    let pa = (p - i32::from(a)).abs();
    let pb = (p - i32::from(b)).abs();
    let pc = (p - i32::from(c)).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}
