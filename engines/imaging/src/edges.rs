//! Detection of a document outline in a camera frame (`ARCHITECTURE.md` §21).
//!
//! The document in a camera frame is usually **tilted**: its four edges are not
//! parallel to the image axes. A detection that only looks at the centre row
//! and the centre column therefore cannot find the outline of a tilted page —
//! it can only ever return an axis aligned rectangle.
//!
//! The detection here works on the **boundary of the page**:
//!
//! 1. A coarse luminance grid of the frame is sampled.
//! 2. The background is estimated from the border of the frame, and the
//!    threshold between background and page is derived from it.
//! 3. For every row of the grid, the leftmost and the rightmost cell of the
//!    page are determined. For every column, the topmost and the bottommost
//!    one. These are the boundary points of the four edges.
//! 4. A line is fitted through the boundary points of every edge, and the four
//!    lines are intersected to obtain the four corners.
//!
//! Text inside the page does not disturb the detection, because the boundary
//! is determined by the *outermost* page cell of a row or a column.

use crate::geometry::{Point2D, QuadCorners};

/// Resolution of the coarse luminance grid that the detection works on.
const GRID_SIZE: usize = 64;

/// A straight line in the normalized image space, given as `x = a * y + b`.
///
/// The document edges of a camera frame are close to vertical or close to
/// horizontal, so a line of this form describes the left and the right edge
/// well. The top and the bottom edge are described by the transposed form.
#[derive(Debug, Clone, Copy)]
struct Line {
    slope: f32,
    offset: f32,
}

impl Line {
    /// Fits a line through the points `(t, v)` with the Theil–Sen estimator.
    ///
    /// The estimator takes the median of the slopes of all pairs of points and
    /// the median of the offsets. It therefore tolerates up to 29 % wrong
    /// points without moving the line, which matters because the corners of a
    /// photographed page are often covered by a shadow and are then not part
    /// of the page boundary.
    fn fit(points: &[(f32, f32)]) -> Option<Self> {
        if points.len() < 3 {
            return None;
        }
        let mut slopes: Vec<f32> = Vec::with_capacity(points.len() * points.len() / 2);
        for (index, &(t1, v1)) in points.iter().enumerate() {
            for &(t2, v2) in points.iter().skip(index + 1) {
                let delta = t2 - t1;
                if delta.abs() > 1e-6 {
                    slopes.push((v2 - v1) / delta);
                }
            }
        }
        if slopes.len() < 3 {
            return None;
        }
        slopes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let slope = slopes[slopes.len() / 2];
        let mut offsets: Vec<f32> = points.iter().map(|&(t, v)| v - slope * t).collect();
        offsets.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let offset = offsets[offsets.len() / 2];
        Some(Self { slope, offset })
    }

    /// Returns the `x` coordinate of the line at the given `y`.
    fn x_at(&self, y: f32) -> f32 {
        self.slope * y + self.offset
    }
}

/// Returns the luminance grid of a frame.
fn luminance_grid(frame: &crate::ScanFrame) -> Vec<f32> {
    let mut grid = vec![0.0f32; GRID_SIZE * GRID_SIZE];
    for y in 0..GRID_SIZE {
        for x in 0..GRID_SIZE {
            let u = x as f32 / (GRID_SIZE - 1) as f32;
            let v = y as f32 / (GRID_SIZE - 1) as f32;
            grid[y * GRID_SIZE + x] = frame.sample_luminance(u, v);
        }
    }
    grid
}

/// Returns the threshold between the background and the page.
///
/// The background is estimated from the border of the frame, the page level
/// from the brighter half of the image. The threshold lies in between, so that
/// a dark background and a bright page are separated, and a dark page on a
/// bright background as well.
fn page_threshold(grid: &[f32]) -> f32 {
    let border: Vec<f32> = grid
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            let x = index % GRID_SIZE;
            let y = index / GRID_SIZE;
            x <= 1 || y <= 1 || x >= GRID_SIZE - 2 || y >= GRID_SIZE - 2
        })
        .map(|(_, value)| *value)
        .collect();
    let mut sorted: Vec<f32> = grid.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let background = border.iter().sum::<f32>() / border.len().max(1) as f32;
    let page_level = sorted[sorted.len() * 3 / 4];
    // The page is the brighter part of the image. If the image is uniformly
    // bright, the threshold falls back to the mean.
    let threshold = if page_level > background {
        background + (page_level - background) * 0.5
    } else {
        sorted[sorted.len() / 2]
    };
    threshold.clamp(0.02, 0.98)
}

/// Detects the document outline in a camera frame.
///
/// Returns `None` when no page can be found in the frame at all. The returned
/// quad carries a confidence that grows with the quality of the line fits.
pub fn detect_outline(frame: &crate::ScanFrame) -> Option<QuadCorners> {
    let grid = luminance_grid(frame);
    let threshold = page_threshold(&grid);
    let cell = 1.0 / (GRID_SIZE - 1) as f32;

    // The boundary points of the four edges, in normalized coordinates.
    let mut left_points: Vec<(f32, f32)> = Vec::new();
    let mut right_points: Vec<(f32, f32)> = Vec::new();
    let mut top_points: Vec<(f32, f32)> = Vec::new();
    let mut bottom_points: Vec<(f32, f32)> = Vec::new();

    for y in 0..GRID_SIZE {
        let v = y as f32 * cell;
        let mut first: Option<usize> = None;
        let mut last: Option<usize> = None;
        for x in 0..GRID_SIZE {
            if grid[y * GRID_SIZE + x] >= threshold {
                if first.is_none() {
                    first = Some(x);
                }
                last = Some(x);
            }
        }
        if let (Some(first), Some(last)) = (first, last) {
            // A row that only contains a few bright cells is text, not the
            // page. It does not contribute to the boundary.
            if last - first >= GRID_SIZE / 8 {
                left_points.push((v, first as f32 * cell));
                right_points.push((v, last as f32 * cell));
            }
        }
    }

    for x in 0..GRID_SIZE {
        let u = x as f32 * cell;
        let mut first: Option<usize> = None;
        let mut last: Option<usize> = None;
        for y in 0..GRID_SIZE {
            if grid[y * GRID_SIZE + x] >= threshold {
                if first.is_none() {
                    first = Some(y);
                }
                last = Some(y);
            }
        }
        if let (Some(first), Some(last)) = (first, last)
            && last - first >= GRID_SIZE / 8
        {
            top_points.push((u, first as f32 * cell));
            bottom_points.push((u, last as f32 * cell));
        }
    }

    if left_points.len() < 4
        || right_points.len() < 4
        || top_points.len() < 4
        || bottom_points.len() < 4
    {
        return None;
    }

    // The left and the right edge are described as `x = a * y + b`, the top and
    // the bottom edge as `y = a * x + b`.
    let left = Line::fit(&left_points)?;
    let right = Line::fit(&right_points)?;
    let top = Line::fit(&top_points)?;
    let bottom = Line::fit(&bottom_points)?;

    // The corners are the intersections of the four edges.
    let top_left = intersect_vertical_horizontal(&left, &top)?;
    let top_right = intersect_vertical_horizontal(&right, &top)?;
    let bottom_right = intersect_vertical_horizontal(&right, &bottom)?;
    let bottom_left = intersect_vertical_horizontal(&left, &bottom)?;

    let quad = QuadCorners {
        top_left,
        top_right,
        bottom_right,
        bottom_left,
        confidence_milli: 0,
    };
    if !quad.is_valid_document_quad() {
        return None;
    }

    // The confidence grows with the quality of the four line fits: a page whose
    // boundary is a straight line in every row is a good candidate. The
    // deviation is the mean over the four edges, so that the confidence does
    // not depend on the number of edges.
    let deviation = (mean_deviation(&left_points, &left)
        + mean_deviation(&right_points, &right)
        + mean_deviation(&top_points, &top)
        + mean_deviation(&bottom_points, &bottom))
        / 4.0;
    // One grid cell is `1 / 63`. A deviation of half a cell is still a good
    // boundary, one cell is already uncertain.
    let quality = (1.0 - deviation * 8.0).clamp(0.0, 1.0);

    Some(QuadCorners {
        confidence_milli: (500.0 + quality * 450.0) as u32,
        ..quad
    })
}

/// Intersects the line `x = a * y + b` with the line `y = c * x + d`.
fn intersect_vertical_horizontal(vertical: &Line, horizontal: &Line) -> Option<Point2D> {
    // `x = a * y + b` and `y = c * x + d`:
    // `y = c * (a * y + b) + d`  =>  `y * (1 - a * c) = c * b + d`
    let denominator = 1.0 - vertical.slope * horizontal.slope;
    if denominator.abs() < 1e-6 {
        return None;
    }
    let y = (horizontal.slope * vertical.offset + horizontal.offset) / denominator;
    let x = vertical.x_at(y);
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some(Point2D::new(x, y))
}

/// Returns the mean distance of the boundary points from their line.
fn mean_deviation(points: &[(f32, f32)], line: &Line) -> f32 {
    if points.is_empty() {
        return 1.0;
    }
    let mut total = 0.0f32;
    let mut count = 0.0f32;
    for &(y, x) in points {
        if y <= 0.02 || y >= 0.98 || x <= 0.02 || x >= 0.98 {
            continue;
        }
        total += (x - line.x_at(y)).abs();
        count += 1.0;
    }
    if count == 0.0 {
        return 1.0;
    }
    total / count
}

#[cfg(test)]
#[path = "edges/edges_tests.rs"]
mod edges_tests;
