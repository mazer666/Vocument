//! Visual bitmap pixel comparison and bounding box change clustering (`COMPARE-003`).

use crate::pdf_engine::PdfRectMilliPt;

use super::model::{ComparisonOptions, PageDiffResult};

/// Compares two RGBA8 page bitmaps and identifies visual changes, differences, and bounding boxes.
#[allow(clippy::too_many_arguments, clippy::manual_div_ceil)]
pub fn compare_page_bitmaps(
    page_a: u32,
    page_b: u32,
    rgba_a: &[u8],
    width_a: u32,
    height_a: u32,
    rgba_b: &[u8],
    width_b: u32,
    height_b: u32,
    page_width_pt: f32,
    page_height_pt: f32,
    options: &ComparisonOptions,
) -> PageDiffResult {
    let width = width_a.max(width_b);
    let height = height_a.max(height_b);
    let total_pixels = width * height;

    if total_pixels == 0 {
        return PageDiffResult {
            page_a,
            page_b,
            pixel_difference_percentage: 0.0,
            changed_pixel_count: 0,
            total_pixel_count: 0,
            difference_bounding_boxes: Vec::new(),
            diff_mask_png: None,
        };
    }

    let mut changed_pixels = 0u32;
    let threshold = i32::from(options.visual_pixel_threshold);

    // Grid for spatial clustering (cell size in pixels)
    let cell_size = 16u32;
    let grid_cols = (width + cell_size - 1) / cell_size;
    let grid_rows = (height + cell_size - 1) / cell_size;
    let mut grid = vec![false; (grid_cols * grid_rows) as usize];

    for y in 0..height {
        for x in 0..width {
            let px_a = get_pixel_rgba(rgba_a, width_a, height_a, x, y);
            let px_b = get_pixel_rgba(rgba_b, width_b, height_b, x, y);

            let dr = (i32::from(px_a[0]) - i32::from(px_b[0])).abs();
            let dg = (i32::from(px_a[1]) - i32::from(px_b[1])).abs();
            let db = (i32::from(px_a[2]) - i32::from(px_b[2])).abs();
            let da = (i32::from(px_a[3]) - i32::from(px_b[3])).abs();

            let delta = dr.max(dg).max(db).max(da);
            if delta > threshold {
                changed_pixels += 1;
                let c_col = x / cell_size;
                let c_row = y / cell_size;
                grid[(c_row * grid_cols + c_col) as usize] = true;
            }
        }
    }

    let diff_percentage = (changed_pixels as f32 / total_pixels as f32) * 100.0;

    // Cluster active grid cells into bounding boxes
    let bounding_boxes = cluster_active_cells(
        &grid,
        grid_cols,
        grid_rows,
        cell_size,
        width,
        height,
        page_width_pt,
        page_height_pt,
    );

    PageDiffResult {
        page_a,
        page_b,
        pixel_difference_percentage: diff_percentage,
        changed_pixel_count: changed_pixels,
        total_pixel_count: total_pixels,
        difference_bounding_boxes: bounding_boxes,
        diff_mask_png: None,
    }
}

fn get_pixel_rgba(buffer: &[u8], width: u32, height: u32, x: u32, y: u32) -> [u8; 4] {
    if x >= width || y >= height {
        return [255, 255, 255, 0]; // Transparent white for out-of-bounds
    }
    let offset = ((y * width + x) * 4) as usize;
    if offset + 4 <= buffer.len() {
        [
            buffer[offset],
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
        ]
    } else {
        [255, 255, 255, 0]
    }
}

/// Clusters active grid cells into bounding boxes expressed in PDF points.
#[allow(clippy::too_many_arguments)]
fn cluster_active_cells(
    grid: &[bool],
    grid_cols: u32,
    grid_rows: u32,
    cell_size: u32,
    img_width: u32,
    img_height: u32,
    page_width_pt: f32,
    page_height_pt: f32,
) -> Vec<PdfRectMilliPt> {
    let mut visited = vec![false; grid.len()];
    let mut boxes = Vec::new();

    let scale_x = page_width_pt / img_width as f32;
    let scale_y = page_height_pt / img_height as f32;

    for row in 0..grid_rows {
        for col in 0..grid_cols {
            let idx = (row * grid_cols + col) as usize;
            if !grid[idx] || visited[idx] {
                continue;
            }

            // Flood-fill connected component of active cells
            let mut min_c = col;
            let mut max_c = col;
            let mut min_r = row;
            let mut max_r = row;

            let mut queue = vec![(col, row)];
            visited[idx] = true;

            while let Some((c, r)) = queue.pop() {
                min_c = min_c.min(c);
                max_c = max_c.max(c);
                min_r = min_r.min(r);
                max_r = max_r.max(r);

                // 4-neighborhood
                let neighbors = [
                    (c.wrapping_sub(1), r),
                    (c + 1, r),
                    (c, r.wrapping_sub(1)),
                    (c, r + 1),
                ];

                for (nc, nr) in neighbors {
                    if nc < grid_cols && nr < grid_rows {
                        let n_idx = (nr * grid_cols + nc) as usize;
                        if grid[n_idx] && !visited[n_idx] {
                            visited[n_idx] = true;
                            queue.push((nc, nr));
                        }
                    }
                }
            }

            // Convert cell bounds to pixel bounds, then to milli-points
            let px_x = min_c * cell_size;
            let px_y = min_r * cell_size;
            let px_w = ((max_c - min_c + 1) * cell_size).min(img_width - px_x);
            let px_h = ((max_r - min_r + 1) * cell_size).min(img_height - px_y);

            let pt_x = px_x as f32 * scale_x;
            let pt_y = px_y as f32 * scale_y;
            let pt_w = px_w as f32 * scale_x;
            let pt_h = px_h as f32 * scale_y;

            let milli_x0 = (pt_x * 1000.0) as i32;
            let milli_y0 = (pt_y * 1000.0) as i32;
            let milli_x1 = ((pt_x + pt_w) * 1000.0) as i32;
            let milli_y1 = ((pt_y + pt_h) * 1000.0) as i32;

            boxes.push(PdfRectMilliPt {
                x0_milli_pt: milli_x0,
                y0_milli_pt: milli_y0,
                x1_milli_pt: milli_x1,
                y1_milli_pt: milli_y1,
            });
        }
    }

    boxes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_bitmaps_produce_zero_diff() {
        let white_pixels = vec![255u8; 100 * 100 * 4];
        let options = ComparisonOptions::default();

        let result = compare_page_bitmaps(
            0,
            0,
            &white_pixels,
            100,
            100,
            &white_pixels,
            100,
            100,
            595.0,
            842.0,
            &options,
        );

        assert_eq!(result.changed_pixel_count, 0);
        assert_eq!(result.pixel_difference_percentage, 0.0);
        assert!(result.difference_bounding_boxes.is_empty());
    }

    #[test]
    fn test_pixel_delta_clusters_into_bounding_box() {
        let bitmap_a = vec![255u8; 100 * 100 * 4];
        let mut bitmap_b = vec![255u8; 100 * 100 * 4];

        // Draw a black 20x20 box in bitmap_b at (20, 20)
        for y in 20..40 {
            for x in 20..40 {
                let offset = (y * 100 + x) * 4;
                bitmap_b[offset] = 0;
                bitmap_b[offset + 1] = 0;
                bitmap_b[offset + 2] = 0;
            }
        }

        let options = ComparisonOptions::default();
        let result = compare_page_bitmaps(
            0, 0, &bitmap_a, 100, 100, &bitmap_b, 100, 100, 100.0, 100.0, &options,
        );

        assert!(result.changed_pixel_count >= 400);
        assert!(result.pixel_difference_percentage > 3.0);
        assert_eq!(result.difference_bounding_boxes.len(), 1);

        let bb = &result.difference_bounding_boxes[0];
        // Bounding box should enclose the 20..40 region (in milli-points: 20_000..40_000)
        assert!(bb.x0_milli_pt <= 20_000);
        assert!(bb.y0_milli_pt <= 20_000);
        assert!(bb.x1_milli_pt >= 40_000);
        assert!(bb.y1_milli_pt >= 40_000);
    }
}
