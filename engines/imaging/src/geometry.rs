use serde::{Deserialize, Serialize};

/// 2D point in normalized coordinates `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Point2D {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Clamps coordinates to `[0.0, 1.0]`.
    pub fn clamped(&self) -> Self {
        Self {
            x: self.x.clamp(0.0, 1.0),
            y: self.y.clamp(0.0, 1.0),
        }
    }
}

/// Quadrilateral corner coordinates defining a document outline in normalized coordinates `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct QuadCorners {
    pub top_left: Point2D,
    pub top_right: Point2D,
    pub bottom_right: Point2D,
    pub bottom_left: Point2D,
    /// Detection confidence score in milli-units (`0` = 0.0, `1000` = 1.0).
    pub confidence_milli: u32,
}

impl QuadCorners {
    /// Creates a full-frame default quad.
    pub const fn full_frame() -> Self {
        Self {
            top_left: Point2D::new(0.0, 0.0),
            top_right: Point2D::new(1.0, 0.0),
            bottom_right: Point2D::new(1.0, 1.0),
            bottom_left: Point2D::new(0.0, 1.0),
            confidence_milli: 1000,
        }
    }

    /// Creates an inset default quad with margin `inset` (e.g. 0.05).
    pub fn with_inset(inset: f32, confidence_milli: u32) -> Self {
        let i = inset.clamp(0.0, 0.4);
        Self {
            top_left: Point2D::new(i, i),
            top_right: Point2D::new(1.0 - i, i),
            bottom_right: Point2D::new(1.0 - i, 1.0 - i),
            bottom_left: Point2D::new(i, 1.0 - i),
            confidence_milli,
        }
    }

    /// Computes the area of the quadrilateral using the Shoelace formula.
    pub fn area(&self) -> f32 {
        let (p0, p1, p2, p3) = (
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        );
        0.5 * ((p0.x * p1.y - p1.x * p0.y)
            + (p1.x * p2.y - p2.x * p1.y)
            + (p2.x * p3.y - p3.x * p2.y)
            + (p3.x * p0.y - p0.x * p3.y))
            .abs()
    }

    /// Tests if the quadrilateral is strictly convex.
    pub fn is_convex(&self) -> bool {
        let pts = [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ];
        let mut sign = 0.0;
        for i in 0..4 {
            let p1 = pts[i];
            let p2 = pts[(i + 1) % 4];
            let p3 = pts[(i + 2) % 4];
            let dx1 = p2.x - p1.x;
            let dy1 = p2.y - p1.y;
            let dx2 = p3.x - p2.x;
            let dy2 = p3.y - p2.y;
            let cross = dx1 * dy2 - dy1 * dx2;
            if cross.abs() > 1e-5 {
                if sign == 0.0 {
                    sign = cross.signum();
                } else if cross.signum() != sign {
                    return false;
                }
            }
        }
        sign != 0.0
    }

    /// Validates whether the quad is plausibly a document sheet:
    /// - strictly convex
    /// - area between 10% and 98% of total frame
    /// - aspect ratio between 0.4 and 2.5
    pub fn is_valid_document_quad(&self) -> bool {
        if !self.is_convex() {
            return false;
        }
        let a = self.area();
        if !(0.10..=1.0001).contains(&a) {
            return false;
        }
        let top_w = ((self.top_right.x - self.top_left.x).powi(2)
            + (self.top_right.y - self.top_left.y).powi(2))
        .sqrt();
        let left_h = ((self.bottom_left.x - self.top_left.x).powi(2)
            + (self.bottom_left.y - self.top_left.y).powi(2))
        .sqrt();
        if top_w < 0.1 || left_h < 0.1 {
            return false;
        }
        let ratio = left_h / top_w;
        (0.35..=2.8).contains(&ratio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_and_quad_area_and_convexity() {
        let full = QuadCorners::full_frame();
        assert!(full.is_convex());
        assert!((full.area() - 1.0).abs() < 1e-4);
        assert!(full.is_valid_document_quad());

        let inset = QuadCorners::with_inset(0.1, 900);
        assert!(inset.is_convex());
        assert!((inset.area() - 0.64).abs() < 1e-4);
        assert!(inset.is_valid_document_quad());

        let concave = QuadCorners {
            top_left: Point2D::new(0.0, 0.0),
            top_right: Point2D::new(0.8, 0.8),
            bottom_right: Point2D::new(0.2, 0.8),
            bottom_left: Point2D::new(0.8, 0.0),
            confidence_milli: 100,
        };
        assert!(!concave.is_convex());
        assert!(!concave.is_valid_document_quad());
    }
}
