use crate::geometry::{Point2D, QuadCorners};
use crate::{ImagingError, PixelFormat, ScanFrame};

/// 3x3 Projective Homography Mapping from normalized unit square `[0, 1]^2` to arbitrary convex quadrilateral.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectiveMapping {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
    g: f32,
    h: f32,
}

impl ProjectiveMapping {
    /// Creates a mapping from destination unit square `[0, 1]^2` to quadrilateral corners `(p0, p1, p2, p3)`.
    ///
    /// Corner ordering: `p0` = top-left, `p1` = top-right, `p2` = bottom-right, `p3` = bottom-left.
    /// Based on Heckbert's projective mapping parameterization (1989).
    pub fn from_quad(corners: &QuadCorners) -> Result<Self, ImagingError> {
        let p0 = corners.top_left;
        let p1 = corners.top_right;
        let p2 = corners.bottom_right;
        let p3 = corners.bottom_left;

        let dx1 = p1.x - p2.x;
        let dx2 = p3.x - p2.x;
        let dx3 = p0.x - p1.x + p2.x - p3.x;

        let dy1 = p1.y - p2.y;
        let dy2 = p3.y - p2.y;
        let dy3 = p0.y - p1.y + p2.y - p3.y;

        let det = dx1 * dy2 - dx2 * dy1;
        if det.abs() < 1e-7 {
            // Affine mapping (parallelogram / rectangle)
            let a = p1.x - p0.x;
            let b = p3.x - p0.x;
            let c = p0.x;
            let d = p1.y - p0.y;
            let e = p3.y - p0.y;
            let f = p0.y;
            return Ok(Self {
                a,
                b,
                c,
                d,
                e,
                f,
                g: 0.0,
                h: 0.0,
            });
        }

        let g = (dx3 * dy2 - dx2 * dy3) / det;
        let h = (dx1 * dy3 - dx3 * dy1) / det;

        let a = p1.x - p0.x + g * p1.x;
        let b = p3.x - p0.x + h * p3.x;
        let c = p0.x;

        let d = p1.y - p0.y + g * p1.y;
        let e = p3.y - p0.y + h * p3.y;
        let f = p0.y;

        Ok(Self {
            a,
            b,
            c,
            d,
            e,
            f,
            g,
            h,
        })
    }

    /// Evaluates the mapping at normalized destination coordinates `(u, v) in [0, 1]^2`, returning source coordinates.
    #[inline]
    pub fn map(&self, u: f32, v: f32) -> Point2D {
        let denom = self.g * u + self.h * v + 1.0;
        let denom_safe = if denom.abs() < 1e-7 { 1e-7 } else { denom };
        let x = (self.a * u + self.b * v + self.c) / denom_safe;
        let y = (self.d * u + self.e * v + self.f) / denom_safe;
        Point2D::new(x, y)
    }
}

/// Result of deskewing and orientation correction.
#[derive(Debug, Clone, PartialEq)]
pub struct DeskewResult {
    pub frame: ScanFrame,
    pub skew_angle_degrees: f32,
    pub rotation_degrees: u32,
}

/// Warps a convex document quadrilateral in `frame` into an upright rectangular scan frame (`SCAN-002`, `SCAN-005`).
pub fn warp_perspective(
    frame: &ScanFrame,
    corners: &QuadCorners,
    target_aspect_ratio: Option<f32>,
) -> Result<ScanFrame, ImagingError> {
    frame.validate()?;
    if !corners.is_valid_document_quad() {
        return Err(ImagingError::InvalidQuad {
            details: "Corners do not form a valid convex document quadrilateral".to_owned(),
        });
    }

    let p0 = corners.top_left;
    let p1 = corners.top_right;
    let p2 = corners.bottom_right;
    let p3 = corners.bottom_left;

    let fw = frame.width as f32;
    let fh = frame.height as f32;

    // Estimate natural dimensions in pixels
    let w_top = ((p1.x - p0.x).powi(2) + (p1.y - p0.y).powi(2)).sqrt() * fw;
    let w_bottom = ((p2.x - p3.x).powi(2) + (p2.y - p3.y).powi(2)).sqrt() * fw;
    let h_left = ((p3.x - p0.x).powi(2) + (p3.y - p0.y).powi(2)).sqrt() * fh;
    let h_right = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt() * fh;

    let dest_w = (w_top.max(w_bottom).round() as u32).clamp(16, 4096);
    let mut dest_h = (h_left.max(h_right).round() as u32).clamp(16, 4096);

    if let Some(target_ratio) = target_aspect_ratio.filter(|&r| (0.05..20.0).contains(&r)) {
        dest_h = ((dest_w as f32 / target_ratio).round() as u32).clamp(16, 4096);
    }

    let mapping = ProjectiveMapping::from_quad(corners)?;
    let channels: usize = match frame.format {
        PixelFormat::Rgb8 => 3,
        PixelFormat::Rgba8 => 4,
        PixelFormat::Gray8 => 1,
        PixelFormat::Nv21 | PixelFormat::Yuv420 => 4, // Convert to RGBA
    };

    let dest_pixels_len = (dest_w * dest_h * channels as u32) as usize;
    let mut dest_pixels = vec![255u8; dest_pixels_len];

    let dest_w_f = dest_w as f32;
    let dest_h_f = dest_h as f32;

    for y_dst in 0..dest_h {
        let v = (y_dst as f32 + 0.5) / dest_h_f;
        let row_offset = (y_dst * dest_w * channels as u32) as usize;

        for x_dst in 0..dest_w {
            let u = (x_dst as f32 + 0.5) / dest_w_f;
            let src_norm = mapping.map(u, v);

            let src_x = (src_norm.x * fw).clamp(0.0, fw - 1.0);
            let src_y = (src_norm.y * fh).clamp(0.0, fh - 1.0);

            let out_idx = row_offset + (x_dst * channels as u32) as usize;

            sample_bilinear(
                frame,
                src_x,
                src_y,
                &mut dest_pixels[out_idx..out_idx + channels],
            );
        }
    }

    let output_format = match frame.format {
        PixelFormat::Rgb8 => PixelFormat::Rgb8,
        PixelFormat::Gray8 => PixelFormat::Gray8,
        _ => PixelFormat::Rgba8,
    };

    Ok(ScanFrame {
        width: dest_w,
        height: dest_h,
        stride: dest_w,
        format: output_format,
        orientation_degrees: 0,
        pixels: dest_pixels,
        metadata: frame.metadata.clone(),
    })
}

/// Bilinear interpolation sampler across scan frame pixel data.
#[inline]
fn sample_bilinear(frame: &ScanFrame, src_x: f32, src_y: f32, out: &mut [u8]) {
    let x0 = src_x.floor() as u32;
    let y0 = src_y.floor() as u32;
    let x1 = (x0 + 1).min(frame.width - 1);
    let y1 = (y0 + 1).min(frame.height - 1);

    let fx = src_x - x0 as f32;
    let fy = src_y - y0 as f32;

    let w00 = (1.0 - fx) * (1.0 - fy);
    let w10 = fx * (1.0 - fy);
    let w01 = (1.0 - fx) * fy;
    let w11 = fx * fy;

    match frame.format {
        PixelFormat::Rgba8 => {
            let stride = frame.stride as usize * 4;
            let c00 = &frame.pixels[y0 as usize * stride + x0 as usize * 4..];
            let c10 = &frame.pixels[y0 as usize * stride + x1 as usize * 4..];
            let c01 = &frame.pixels[y1 as usize * stride + x0 as usize * 4..];
            let c11 = &frame.pixels[y1 as usize * stride + x1 as usize * 4..];

            for ch in 0..4 {
                let v = c00[ch] as f32 * w00
                    + c10[ch] as f32 * w10
                    + c01[ch] as f32 * w01
                    + c11[ch] as f32 * w11;
                out[ch] = v.round() as u8;
            }
        }
        PixelFormat::Rgb8 => {
            let stride = frame.stride as usize * 3;
            let c00 = &frame.pixels[y0 as usize * stride + x0 as usize * 3..];
            let c10 = &frame.pixels[y0 as usize * stride + x1 as usize * 3..];
            let c01 = &frame.pixels[y1 as usize * stride + x0 as usize * 3..];
            let c11 = &frame.pixels[y1 as usize * stride + x1 as usize * 3..];

            for ch in 0..3 {
                let v = c00[ch] as f32 * w00
                    + c10[ch] as f32 * w10
                    + c01[ch] as f32 * w01
                    + c11[ch] as f32 * w11;
                out[ch] = v.round() as u8;
            }
        }
        PixelFormat::Gray8 => {
            let stride = frame.stride as usize;
            let v00 = frame.pixels[y0 as usize * stride + x0 as usize] as f32;
            let v10 = frame.pixels[y0 as usize * stride + x1 as usize] as f32;
            let v01 = frame.pixels[y1 as usize * stride + x0 as usize] as f32;
            let v11 = frame.pixels[y1 as usize * stride + x1 as usize] as f32;

            let v = v00 * w00 + v10 * w10 + v01 * w01 + v11 * w11;
            out[0] = v.round() as u8;
        }
        PixelFormat::Nv21 | PixelFormat::Yuv420 => {
            let y_stride = frame.stride as usize;
            let y = frame.pixels[y0 as usize * y_stride + x0 as usize];
            out[0] = y;
            out[1] = y;
            out[2] = y;
            if out.len() >= 4 {
                out[3] = 255;
            }
        }
    }
}

/// Estimates document deskew angle and rotates the frame to align text baselines (`SCAN-005`).
pub fn deskew_frame(frame: &ScanFrame) -> Result<DeskewResult, ImagingError> {
    frame.validate()?;

    let mut best_angle = 0.0f32;
    let mut max_variance = 0.0f32;
    let test_angles = [
        -8.0f32, -6.0, -4.0, -2.0, -1.0, 0.0, 1.0, 2.0, 4.0, 6.0, 8.0,
    ];
    for &angle in &test_angles {
        let rad = angle.to_radians();
        let cos_a = rad.cos();
        let sin_a = rad.sin();

        let mut line_means = Vec::with_capacity(16);
        for line_idx in 0..16 {
            let y_rel = (line_idx as f32 + 1.0) / 17.0;
            let mut line_sum = 0.0f32;
            for sample_idx in 0..16 {
                let x_rel = (sample_idx as f32 + 1.0) / 17.0;
                let u = (x_rel - 0.5) * cos_a - (y_rel - 0.5) * sin_a + 0.5;
                let v = (x_rel - 0.5) * sin_a + (y_rel - 0.5) * cos_a + 0.5;
                line_sum += frame.sample_luminance(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
            }
            line_means.push(line_sum / 16.0);
        }

        let mean = line_means.iter().sum::<f32>() / line_means.len() as f32;
        let variance = line_means.iter().map(|&x| (x - mean).powi(2)).sum::<f32>();
        if variance > max_variance {
            max_variance = variance;
            best_angle = angle;
        }
    }

    let deskewed = if best_angle.abs() >= 0.5 {
        rotate_frame_fine(frame, best_angle)?
    } else {
        frame.clone()
    };

    Ok(DeskewResult {
        frame: deskewed,
        skew_angle_degrees: best_angle,
        rotation_degrees: frame.orientation_degrees,
    })
}

/// Rotates frame by a small arbitrary angle (in degrees) around center via bilinear interpolation.
pub fn rotate_frame_fine(frame: &ScanFrame, angle_degrees: f32) -> Result<ScanFrame, ImagingError> {
    frame.validate()?;
    if angle_degrees.abs() < 0.25 {
        return Ok(frame.clone());
    }
    let rad = (-angle_degrees).to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();
    let fw = frame.width as f32;
    let fh = frame.height as f32;
    let channels: usize = match frame.format {
        PixelFormat::Rgb8 => 3,
        PixelFormat::Rgba8 => 4,
        PixelFormat::Gray8 => 1,
        PixelFormat::Nv21 | PixelFormat::Yuv420 => 4,
    };
    let mut dest_pixels = vec![255u8; (frame.width * frame.height * channels as u32) as usize];
    for y_dst in 0..frame.height {
        let v = (y_dst as f32 + 0.5) / fh;
        let row_offset = (y_dst * frame.width * channels as u32) as usize;
        for x_dst in 0..frame.width {
            let u = (x_dst as f32 + 0.5) / fw;
            let src_u = (u - 0.5) * cos_a - (v - 0.5) * sin_a + 0.5;
            let src_v = (u - 0.5) * sin_a + (v - 0.5) * cos_a + 0.5;
            let src_x = (src_u * fw).clamp(0.0, fw - 1.0);
            let src_y = (src_v * fh).clamp(0.0, fh - 1.0);
            let out_idx = row_offset + (x_dst * channels as u32) as usize;
            sample_bilinear(
                frame,
                src_x,
                src_y,
                &mut dest_pixels[out_idx..out_idx + channels],
            );
        }
    }
    let output_format = match frame.format {
        PixelFormat::Rgb8 => PixelFormat::Rgb8,
        PixelFormat::Gray8 => PixelFormat::Gray8,
        _ => PixelFormat::Rgba8,
    };
    Ok(ScanFrame {
        width: frame.width,
        height: frame.height,
        stride: frame.width,
        format: output_format,
        orientation_degrees: frame.orientation_degrees,
        pixels: dest_pixels,
        metadata: frame.metadata.clone(),
    })
}

/// Rotates a scan frame by multiples of 90 degrees clockwise (0, 90, 180, 270).
pub fn rotate_frame_90(frame: &ScanFrame, clockwise_steps: u32) -> Result<ScanFrame, ImagingError> {
    frame.validate()?;
    let steps = clockwise_steps % 4;
    if steps == 0 {
        return Ok(frame.clone());
    }
    let channels: usize = match frame.format {
        PixelFormat::Rgb8 => 3,
        PixelFormat::Rgba8 => 4,
        PixelFormat::Gray8 => 1,
        PixelFormat::Nv21 | PixelFormat::Yuv420 => 4,
    };
    let (dest_w, dest_h) = if steps == 1 || steps == 3 {
        (frame.height, frame.width)
    } else {
        (frame.width, frame.height)
    };
    let mut dest_pixels = vec![0u8; (dest_w * dest_h * channels as u32) as usize];
    for y_src in 0..frame.height {
        for x_src in 0..frame.width {
            let (x_dst, y_dst) = match steps {
                1 => (frame.height - 1 - y_src, x_src),
                2 => (frame.width - 1 - x_src, frame.height - 1 - y_src),
                3 => (y_src, frame.width - 1 - x_src),
                _ => (x_src, y_src),
            };
            let src_idx = (y_src * frame.stride + x_src) as usize * channels;
            let dst_idx = (y_dst * dest_w + x_dst) as usize * channels;
            if src_idx + channels <= frame.pixels.len() && dst_idx + channels <= dest_pixels.len() {
                dest_pixels[dst_idx..dst_idx + channels]
                    .copy_from_slice(&frame.pixels[src_idx..src_idx + channels]);
            }
        }
    }
    let new_orientation = (frame.orientation_degrees + steps * 90) % 360;
    Ok(ScanFrame {
        width: dest_w,
        height: dest_h,
        stride: dest_w,
        format: frame.format,
        orientation_degrees: new_orientation,
        pixels: dest_pixels,
        metadata: frame.metadata.clone(),
    })
}

/// Automatically orients a scan frame to upright portrait based on aspect ratio and orientation hints.
pub fn auto_orient_frame(
    frame: &ScanFrame,
    prefer_portrait: bool,
) -> Result<ScanFrame, ImagingError> {
    frame.validate()?;
    let mut current = frame.clone();
    if current.orientation_degrees != 0 {
        let steps = (360 - (current.orientation_degrees % 360)) / 90;
        if steps > 0 {
            current = rotate_frame_90(&current, steps)?;
            current.orientation_degrees = 0;
        }
    }
    if prefer_portrait && current.width > current.height {
        current = rotate_frame_90(&current, 1)?;
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projective_mapping_unit_square_identity() {
        let quad = QuadCorners {
            top_left: Point2D::new(0.0, 0.0),
            top_right: Point2D::new(1.0, 0.0),
            bottom_right: Point2D::new(1.0, 1.0),
            bottom_left: Point2D::new(0.0, 1.0),
            confidence_milli: 1000,
        };

        let mapping = ProjectiveMapping::from_quad(&quad).expect("mapping creation");
        assert_eq!(mapping.map(0.0, 0.0), Point2D::new(0.0, 0.0));
        assert_eq!(mapping.map(1.0, 0.0), Point2D::new(1.0, 0.0));
        assert_eq!(mapping.map(1.0, 1.0), Point2D::new(1.0, 1.0));
        assert_eq!(mapping.map(0.0, 1.0), Point2D::new(0.0, 1.0));
    }

    #[test]
    fn test_warp_perspective_synthetic_frame() {
        let width = 100;
        let height = 100;
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for y in 20..80 {
            for x in 20..80 {
                let idx = ((y * width + x) * 4) as usize;
                pixels[idx] = 200;
                pixels[idx + 1] = 200;
                pixels[idx + 2] = 200;
                pixels[idx + 3] = 255;
            }
        }

        let frame = ScanFrame {
            width,
            height,
            stride: width,
            format: PixelFormat::Rgba8,
            orientation_degrees: 0,
            pixels,
            metadata: Default::default(),
        };

        let quad = QuadCorners {
            top_left: Point2D::new(0.2, 0.2),
            top_right: Point2D::new(0.8, 0.2),
            bottom_right: Point2D::new(0.8, 0.8),
            bottom_left: Point2D::new(0.2, 0.8),
            confidence_milli: 900,
        };

        let warped = warp_perspective(&frame, &quad, None).expect("warp perspective");
        assert!(warped.width >= 58 && warped.width <= 62);
        assert!(warped.height >= 58 && warped.height <= 62);

        let center_idx = ((warped.height / 2 * warped.width + warped.width / 2) * 4) as usize;
        assert_eq!(warped.pixels[center_idx], 200);
    }

    #[test]
    fn test_deskew_and_rotation_operations() {
        let frame = ScanFrame {
            width: 64,
            height: 64,
            stride: 64,
            format: PixelFormat::Gray8,
            orientation_degrees: 0,
            pixels: vec![128u8; 64 * 64],
            metadata: Default::default(),
        };

        let res = deskew_frame(&frame).expect("deskew");
        assert!(res.skew_angle_degrees.abs() <= 15.0);

        let rot90 = rotate_frame_90(&frame, 1).expect("rotate 90");
        assert_eq!(rot90.width, 64);
        assert_eq!(rot90.height, 64);
        assert_eq!(rot90.orientation_degrees, 90);

        let landscape = ScanFrame {
            width: 80,
            height: 50,
            stride: 80,
            format: PixelFormat::Gray8,
            orientation_degrees: 0,
            pixels: vec![100u8; 80 * 50],
            metadata: Default::default(),
        };
        let oriented = auto_orient_frame(&landscape, true).expect("auto orient");
        assert_eq!(oriented.width, 50);
        assert_eq!(oriented.height, 80);
    }
}
