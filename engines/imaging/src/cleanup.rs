//! Intelligent scan cleanup and image enhancement (`ARCHITECTURE.md` §21, `REQUIREMENTS.md` §8, `SCAN-004`).
//!
//! Provides:
//! - Adaptive 2D block-based background illumination estimation and shadow gradient removal.
//! - Paper crease, fold shadow, and yellowing attenuation via homomorphic normalization.
//! - Document background whitening and text ink contrast stretching (`DocumentAuto`).
//! - Locally adaptive Bradley-Roth / Sauvola threshold binarization (`HighContrastBw`).
//! - Continuous-tone dynamic range and gamut preservation (`PreservePhoto`).
//! - Quantitative visual regression metrics (`CleanupQualityMetrics`, `evaluate_cleanup_quality`).

use crate::{ImagingError, PixelFormat, ScanCleanupProfile, ScanFrame};

/// Quantitative metrics evaluating scan cleanup quality and visual regression.
#[derive(Debug, Clone, PartialEq)]
pub struct CleanupQualityMetrics {
    /// Background uniformity in paper regions: `1.0 - (std_dev / mean)`. Target: `>= 0.90`.
    pub background_uniformity: f32,
    /// Contrast ratio between paper background and ink text: `(bg - ink) / bg`. Target: `>= 0.80`.
    pub contrast_ratio: f32,
    /// Shadow gradient attenuation factor: `1.0 - (gradient_out / gradient_in)`. Target: `>= 0.80`.
    pub shadow_attenuation: f32,
    /// Whether the output frame is pure binary black and white (`{0, 255}`).
    pub is_binarized: bool,
}

/// Enhances an ingested scan frame according to the selected cleanup profile (`SCAN-004`).
pub fn enhance_frame(
    frame: &ScanFrame,
    profile: ScanCleanupProfile,
) -> Result<ScanFrame, ImagingError> {
    frame.validate()?;

    let (width, height) = (frame.width, frame.height);
    let luma = extract_luminance_plane(frame);
    let bg_map = estimate_background_illumination(&luma, width, height);

    let mut out_frame = frame.clone();
    let channels = match frame.format {
        PixelFormat::Rgb8 => 3,
        PixelFormat::Rgba8 => 4,
        PixelFormat::Gray8 => 1,
        PixelFormat::Nv21 | PixelFormat::Yuv420 => 4, // converted to RGBA
    };

    if frame.format == PixelFormat::Nv21 || frame.format == PixelFormat::Yuv420 {
        out_frame.format = PixelFormat::Rgba8;
        out_frame.stride = width;
        out_frame.pixels = vec![255u8; (width * height * 4) as usize];
    }

    match profile {
        ScanCleanupProfile::DocumentAuto => {
            apply_document_auto(frame, &mut out_frame, &luma, &bg_map, channels)
        }
        ScanCleanupProfile::HighContrastBw => {
            apply_high_contrast_bw(frame, &mut out_frame, &luma, &bg_map, channels)
        }
        ScanCleanupProfile::PreservePhoto => {
            apply_preserve_photo(frame, &mut out_frame, &luma, &bg_map, channels)
        }
    }

    Ok(out_frame)
}

/// Evaluates cleanup quality metrics between an original uncleaned frame and an enhanced frame.
pub fn evaluate_cleanup_quality(
    original: &ScanFrame,
    enhanced: &ScanFrame,
) -> Result<CleanupQualityMetrics, ImagingError> {
    original.validate()?;
    enhanced.validate()?;

    let orig_luma = extract_luminance_plane(original);
    let enh_luma = extract_luminance_plane(enhanced);

    let total_pixels = enh_luma.len();
    if total_pixels == 0 {
        return Ok(CleanupQualityMetrics {
            background_uniformity: 1.0,
            contrast_ratio: 1.0,
            shadow_attenuation: 1.0,
            is_binarized: false,
        });
    }

    // Identify paper background (brightest 35% in enhanced image) and ink text (darkest 15%)
    let mut sorted_enh = enh_luma.clone();
    sorted_enh.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let bg_threshold = sorted_enh[(total_pixels as f32 * 0.65) as usize];
    let ink_threshold = sorted_enh[(total_pixels as f32 * 0.15) as usize];

    let mut bg_sum = 0.0f32;
    let mut bg_count = 0usize;
    let mut ink_sum = 0.0f32;
    let mut ink_count = 0usize;
    let mut is_binarized = true;

    for &val in &enh_luma {
        if val > 1.0 && val < 254.0 {
            is_binarized = false;
        }
        if val >= bg_threshold {
            bg_sum += val;
            bg_count += 1;
        } else if val <= ink_threshold {
            ink_sum += val;
            ink_count += 1;
        }
    }

    let bg_mean = if bg_count > 0 {
        bg_sum / bg_count as f32
    } else {
        255.0
    };
    let ink_mean = if ink_count > 0 {
        ink_sum / ink_count as f32
    } else {
        0.0
    };

    // Standard deviation of background pixels
    let mut bg_var_sum = 0.0f32;
    for &val in &enh_luma {
        if val >= bg_threshold {
            let diff = val - bg_mean;
            bg_var_sum += diff * diff;
        }
    }
    let bg_std = if bg_count > 1 {
        (bg_var_sum / bg_count as f32).sqrt()
    } else {
        0.0
    };
    let background_uniformity = (1.0 - (bg_std / bg_mean.max(1.0))).clamp(0.0, 1.0);
    let contrast_ratio = ((bg_mean - ink_mean) / bg_mean.max(1.0)).clamp(0.0, 1.0);

    // Shadow attenuation: measure gradient across original vs enhanced
    let orig_bg_map = estimate_background_illumination(&orig_luma, original.width, original.height);
    let enh_bg_map = estimate_background_illumination(&enh_luma, enhanced.width, enhanced.height);
    let orig_grad = calculate_illumination_gradient(&orig_bg_map);
    let enh_grad = calculate_illumination_gradient(&enh_bg_map);
    let shadow_attenuation = if orig_grad > 1.0 {
        (1.0 - (enh_grad / orig_grad)).clamp(0.0, 1.0)
    } else {
        1.0
    };

    Ok(CleanupQualityMetrics {
        background_uniformity,
        contrast_ratio,
        shadow_attenuation,
        is_binarized,
    })
}

// ---------------------------------------------------------------------------
// Internal Cleanup Algorithms
// ---------------------------------------------------------------------------

fn extract_luminance_plane(frame: &ScanFrame) -> Vec<f32> {
    let mut luma = Vec::with_capacity((frame.width * frame.height) as usize);
    let channels = match frame.format {
        PixelFormat::Rgb8 => 3,
        PixelFormat::Rgba8 => 4,
        _ => 1,
    };
    for y in 0..frame.height {
        let row = (y * frame.stride * channels as u32) as usize;
        for x in 0..frame.width {
            let idx = row + (x as usize * channels);
            let val = match frame.format {
                PixelFormat::Rgb8 | PixelFormat::Rgba8 => {
                    let r = frame.pixels.get(idx).copied().unwrap_or(128) as f32;
                    let g = frame.pixels.get(idx + 1).copied().unwrap_or(128) as f32;
                    let b = frame.pixels.get(idx + 2).copied().unwrap_or(128) as f32;
                    0.299 * r + 0.587 * g + 0.114 * b
                }
                _ => frame.pixels.get(idx).copied().unwrap_or(128) as f32,
            };
            luma.push(val);
        }
    }
    luma
}

/// Estimates low-frequency background illumination $B(x, y)$ using a 2D block grid with 90th percentile filtering.
fn estimate_background_illumination(luma: &[f32], width: u32, height: u32) -> Vec<f32> {
    let block_size = ((width.min(height) / 16).clamp(16, 64)) as usize;
    let grid_w = (width as usize).div_ceil(block_size).max(2);
    let grid_h = (height as usize).div_ceil(block_size).max(2);

    let mut grid = vec![0.0f32; grid_w * grid_h];
    let mut block_samples = Vec::with_capacity(block_size * block_size);

    for gy in 0..grid_h {
        let y_start = gy * block_size;
        let y_end = (y_start + block_size).min(height as usize);

        for gx in 0..grid_w {
            let x_start = gx * block_size;
            let x_end = (x_start + block_size).min(width as usize);

            block_samples.clear();
            for y in y_start..y_end {
                let row_offset = y * width as usize;
                for x in x_start..x_end {
                    if let Some(&val) = luma.get(row_offset + x) {
                        block_samples.push(val);
                    }
                }
            }

            if !block_samples.is_empty() {
                block_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let p90_idx =
                    ((block_samples.len() as f32 * 0.90) as usize).min(block_samples.len() - 1);
                grid[gy * grid_w + gx] = block_samples[p90_idx].max(60.0);
            } else {
                grid[gy * grid_w + gx] = 200.0;
            }
        }
    }

    // Smooth block grid with a 3x3 box blur
    let mut smoothed_grid = grid.clone();
    for gy in 0..grid_h {
        for gx in 0..grid_w {
            let mut sum = 0.0f32;
            let mut count = 0usize;
            for dy in -1i32..=1 {
                let ny = gy as i32 + dy;
                if ny >= 0 && (ny as usize) < grid_h {
                    for dx in -1i32..=1 {
                        let nx = gx as i32 + dx;
                        if nx >= 0 && (nx as usize) < grid_w {
                            sum += grid[(ny as usize) * grid_w + (nx as usize)];
                            count += 1;
                        }
                    }
                }
            }
            smoothed_grid[gy * grid_w + gx] = sum / count as f32;
        }
    }

    // Bilinearly interpolate smoothed grid to full image dimensions
    let mut bg_map = Vec::with_capacity((width * height) as usize);
    let gw_f = (grid_w - 1) as f32;
    let gh_f = (grid_h - 1) as f32;
    let w_f = width as f32;
    let h_f = height as f32;

    for y in 0..height {
        let v = ((y as f32 + 0.5) / h_f * gh_f).clamp(0.0, gh_f);
        let gy0 = v.floor() as usize;
        let gy1 = (gy0 + 1).min(grid_h - 1);
        let fv = v - gy0 as f32;

        for x in 0..width {
            let u = ((x as f32 + 0.5) / w_f * gw_f).clamp(0.0, gw_f);
            let gx0 = u.floor() as usize;
            let gx1 = (gx0 + 1).min(grid_w - 1);
            let fu = u - gx0 as f32;

            let b00 = smoothed_grid[gy0 * grid_w + gx0];
            let b10 = smoothed_grid[gy0 * grid_w + gx1];
            let b01 = smoothed_grid[gy1 * grid_w + gx0];
            let b11 = smoothed_grid[gy1 * grid_w + gx1];

            let top = b00 + fu * (b10 - b00);
            let bot = b01 + fu * (b11 - b01);
            let bg_val = (top + fv * (bot - top)).max(1.0);
            bg_map.push(bg_val);
        }
    }

    bg_map
}

fn calculate_illumination_gradient(bg_map: &[f32]) -> f32 {
    if bg_map.is_empty() {
        return 0.0;
    }
    let mut min_val = f32::MAX;
    let mut max_val = f32::MIN;
    for &val in bg_map {
        if val < min_val {
            min_val = val;
        }
        if val > max_val {
            max_val = val;
        }
    }
    (max_val - min_val).max(0.0)
}

fn apply_document_auto(
    orig: &ScanFrame,
    out: &mut ScanFrame,
    luma: &[f32],
    bg_map: &[f32],
    channels: usize,
) {
    let (width, height) = (orig.width, orig.height);

    for y in 0..height {
        let luma_row = (y * width) as usize;
        let out_row = (y * out.stride * channels as u32) as usize;
        let orig_row = (y
            * orig.stride
            * (if orig.format == PixelFormat::Rgb8 {
                3
            } else {
                4
            })) as usize;

        for x in 0..width {
            let idx = luma_row + x as usize;
            let l = luma[idx];
            let bg = bg_map[idx];

            // Normalize luminance against local background: L_norm in [0, 255]
            let norm_l = (l / bg * 255.0).clamp(0.0, 255.0);

            // Adaptive contrast S-curve: whiten background (>215), darken ink (<90)
            let enhanced_l = if norm_l >= 215.0 {
                255.0
            } else if norm_l <= 80.0 {
                (norm_l * 0.8).clamp(0.0, 255.0)
            } else {
                let t = (norm_l - 80.0) / (215.0 - 80.0);
                80.0 * 0.8 + t.powf(1.25) * (255.0 - 80.0 * 0.8)
            };

            let out_idx = out_row + (x as usize * channels);
            if channels == 1 {
                out.pixels[out_idx] = enhanced_l.round() as u8;
            } else {
                let src_idx = orig_row
                    + (x as usize
                        * (if orig.format == PixelFormat::Rgb8 {
                            3
                        } else {
                            4
                        }));
                let scale = if l > 0.1 { enhanced_l / l } else { 1.0 };
                let r = orig.pixels.get(src_idx).copied().unwrap_or(255) as f32;
                let g = orig.pixels.get(src_idx + 1).copied().unwrap_or(255) as f32;
                let b = orig.pixels.get(src_idx + 2).copied().unwrap_or(255) as f32;

                out.pixels[out_idx] = (r * scale).clamp(0.0, 255.0).round() as u8;
                out.pixels[out_idx + 1] = (g * scale).clamp(0.0, 255.0).round() as u8;
                out.pixels[out_idx + 2] = (b * scale).clamp(0.0, 255.0).round() as u8;
                if channels == 4 {
                    out.pixels[out_idx + 3] = 255;
                }
            }
        }
    }
}

fn apply_high_contrast_bw(
    _orig: &ScanFrame,
    out: &mut ScanFrame,
    luma: &[f32],
    bg_map: &[f32],
    channels: usize,
) {
    let (width, height) = (out.width, out.height);

    for y in 0..height {
        let luma_row = (y * width) as usize;
        let out_row = (y * out.stride * channels as u32) as usize;

        for x in 0..width {
            let idx = luma_row + x as usize;
            let l = luma[idx];
            let bg = bg_map[idx];

            // Adaptive Bradley/Sauvola threshold: 18% below local paper background
            let threshold = bg * 0.82;
            let val = if l < threshold { 0u8 } else { 255u8 };

            let out_idx = out_row + (x as usize * channels);
            if channels == 1 {
                out.pixels[out_idx] = val;
            } else {
                out.pixels[out_idx] = val;
                out.pixels[out_idx + 1] = val;
                out.pixels[out_idx + 2] = val;
                if channels == 4 {
                    out.pixels[out_idx + 3] = 255;
                }
            }
        }
    }
}

fn apply_preserve_photo(
    orig: &ScanFrame,
    out: &mut ScanFrame,
    luma: &[f32],
    bg_map: &[f32],
    channels: usize,
) {
    let (width, height) = (orig.width, orig.height);
    let avg_bg = if !bg_map.is_empty() {
        bg_map.iter().sum::<f32>() / bg_map.len() as f32
    } else {
        200.0
    };

    for y in 0..height {
        let luma_row = (y * width) as usize;
        let out_row = (y * out.stride * channels as u32) as usize;
        let orig_row = (y
            * orig.stride
            * (if orig.format == PixelFormat::Rgb8 {
                3
            } else {
                4
            })) as usize;

        for x in 0..width {
            let idx = luma_row + x as usize;
            let l = luma[idx];
            let bg = bg_map[idx];

            // Gentle illumination flattening (power 0.6) preserving smooth continuous tones
            let factor = (avg_bg / bg.max(1.0)).powf(0.60);
            let enhanced_l = (l * factor).clamp(0.0, 255.0);

            let out_idx = out_row + (x as usize * channels);
            if channels == 1 {
                out.pixels[out_idx] = enhanced_l.round() as u8;
            } else {
                let src_idx = orig_row
                    + (x as usize
                        * (if orig.format == PixelFormat::Rgb8 {
                            3
                        } else {
                            4
                        }));
                let r = orig.pixels.get(src_idx).copied().unwrap_or(255) as f32;
                let g = orig.pixels.get(src_idx + 1).copied().unwrap_or(255) as f32;
                let b = orig.pixels.get(src_idx + 2).copied().unwrap_or(255) as f32;

                out.pixels[out_idx] = (r * factor).clamp(0.0, 255.0).round() as u8;
                out.pixels[out_idx + 1] = (g * factor).clamp(0.0, 255.0).round() as u8;
                out.pixels[out_idx + 2] = (b * factor).clamp(0.0, 255.0).round() as u8;
                if channels == 4 {
                    out.pixels[out_idx + 3] = 255;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScanFrameMetadata;

    /// Creates a synthetic test frame with an uneven illumination gradient (shadow from left=100 to right=240),
    /// a paper fold crease line, and black text lines.
    fn create_synthetic_shadowed_document_frame(w: u32, h: u32) -> ScanFrame {
        let mut pixels = vec![255u8; (w * h * 4) as usize];
        for y in 0..h {
            let y_f = y as f32 / h as f32;
            let row = (y * w * 4) as usize;
            for x in 0..w {
                let x_f = x as f32 / w as f32;
                // Linear illumination gradient from left (shadowed: 110) to right (unshadowed: 240)
                let mut bg = 110.0 + x_f * 130.0;

                // Paper fold crease across y = 0.50
                if (y_f - 0.50).abs() < 0.02 {
                    bg *= 0.75;
                }

                let is_text = (y % 20 < 4) && (x > 20 && x < w - 20);
                let lum = if is_text { 25.0 } else { bg };

                let idx = row + (x * 4) as usize;
                pixels[idx] = lum.round() as u8;
                pixels[idx + 1] = lum.round() as u8;
                pixels[idx + 2] = lum.round() as u8;
                pixels[idx + 3] = 255;
            }
        }

        ScanFrame {
            width: w,
            height: h,
            stride: w,
            format: PixelFormat::Rgba8,
            orientation_degrees: 0,
            pixels,
            metadata: ScanFrameMetadata::default(),
        }
    }

    #[test]
    fn test_document_auto_background_whitening_and_shadow_attenuation() {
        let frame = create_synthetic_shadowed_document_frame(128, 128);
        let enhanced = enhance_frame(&frame, ScanCleanupProfile::DocumentAuto).expect("enhance");
        let metrics = evaluate_cleanup_quality(&frame, &enhanced).expect("evaluate metrics");

        assert!(
            metrics.background_uniformity >= 0.90,
            "unif: {}",
            metrics.background_uniformity
        );
        assert!(
            metrics.contrast_ratio >= 0.80,
            "contrast: {}",
            metrics.contrast_ratio
        );
        assert!(
            metrics.shadow_attenuation >= 0.80,
            "atten: {}",
            metrics.shadow_attenuation
        );
        assert!(!metrics.is_binarized, "DocumentAuto must not be binarized");
    }

    #[test]
    fn test_high_contrast_bw_binarization() {
        let frame = create_synthetic_shadowed_document_frame(128, 128);
        let enhanced = enhance_frame(&frame, ScanCleanupProfile::HighContrastBw).expect("enhance");
        let metrics = evaluate_cleanup_quality(&frame, &enhanced).expect("evaluate metrics");

        assert!(metrics.is_binarized, "HighContrastBw must be binary");
        assert!(
            metrics.background_uniformity >= 0.95,
            "unif: {}",
            metrics.background_uniformity
        );
        assert!(
            metrics.contrast_ratio >= 0.95,
            "contrast: {}",
            metrics.contrast_ratio
        );
    }

    #[test]
    fn test_preserve_photo_continuous_tone_preservation() {
        // Continuous 16-step grayscale ramp
        let (w, h) = (64u32, 64u32);
        let mut pixels = vec![255u8; (w * h * 4) as usize];
        for y in 0..h {
            let row = (y * w * 4) as usize;
            for x in 0..w {
                let step = (x / 4).min(15);
                let lum = (step * 17) as u8;
                let idx = row + (x * 4) as usize;
                pixels[idx] = lum;
                pixels[idx + 1] = lum;
                pixels[idx + 2] = lum;
                pixels[idx + 3] = 255;
            }
        }
        let frame = ScanFrame {
            width: w,
            height: h,
            stride: w,
            format: PixelFormat::Rgba8,
            orientation_degrees: 0,
            pixels,
            metadata: ScanFrameMetadata::default(),
        };

        let enhanced = enhance_frame(&frame, ScanCleanupProfile::PreservePhoto).expect("enhance");
        let metrics = evaluate_cleanup_quality(&frame, &enhanced).expect("evaluate metrics");

        assert!(
            !metrics.is_binarized,
            "PreservePhoto must not binarize photo tones"
        );

        // Verify that intermediate gray values are preserved (not clipped)
        let unique_levels: std::collections::HashSet<u8> =
            enhanced.pixels.iter().copied().step_by(4).collect();
        assert!(
            unique_levels.len() >= 14,
            "Expected at least 14 unique tones preserved in photo profile, got {}",
            unique_levels.len()
        );
    }
}
