//! Fixed-point typography units and 2D geometric primitives (`ADR-0020`, `AUTHOR-004`).

use serde::{Deserialize, Serialize};

/// 26.6 fixed-point scalar representing sub-point coordinates.
/// 1 typographic point = 64 fixed units.
///
/// Using fixed-point integer arithmetic prevents floating-point divergence
/// across x86_64, aarch64, Windows, and Linux hardware architectures.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct FixedPt(pub i32);

impl FixedPt {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(64);

    /// Converts standard floating-point points to 26.6 fixed point.
    #[inline]
    pub fn from_f32(val: f32) -> Self {
        Self((val * 64.0).round() as i32)
    }

    /// Converts 26.6 fixed point back to floating-point points.
    #[inline]
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / 64.0
    }

    #[inline]
    pub fn from_i32(val: i32) -> Self {
        Self(val.saturating_mul(64))
    }

    #[inline]
    pub fn round_to_pt(self) -> f32 {
        (self.0 as f32 / 64.0 * 100.0).round() / 100.0
    }
}

impl std::ops::Add for FixedPt {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl std::ops::AddAssign for FixedPt {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_add(rhs.0);
    }
}

impl std::ops::Sub for FixedPt {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl std::ops::SubAssign for FixedPt {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_sub(rhs.0);
    }
}

impl std::ops::Mul<i32> for FixedPt {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: i32) -> Self {
        Self(self.0.saturating_mul(rhs))
    }
}

impl std::ops::Div<i32> for FixedPt {
    type Output = Self;
    #[inline]
    fn div(self, rhs: i32) -> Self {
        if rhs == 0 {
            Self(0)
        } else {
            Self(self.0 / rhs)
        }
    }
}

/// 2D point in typographic points.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

/// 2D size dimensions in typographic points.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    #[inline]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// 2D rectangle in typographic points.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    #[inline]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[inline]
    pub fn from_origin_size(origin: Point, size: Size) -> Self {
        Self {
            x: origin.x,
            y: origin.y,
            width: size.width,
            height: size.height,
        }
    }

    #[inline]
    pub fn origin(self) -> Point {
        Point::new(self.x, self.y)
    }

    #[inline]
    pub fn size(self) -> Size {
        Size::new(self.width, self.height)
    }

    #[inline]
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    #[inline]
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    #[inline]
    pub fn contains(self, pt: Point) -> bool {
        pt.x >= self.x && pt.x <= self.right() && pt.y >= self.y && pt.y <= self.bottom()
    }

    #[inline]
    pub fn inset(self, insets: Insets) -> Self {
        Self {
            x: self.x + insets.left,
            y: self.y + insets.top,
            width: (self.width - insets.left - insets.right).max(0.0),
            height: (self.height - insets.top - insets.bottom).max(0.0),
        }
    }

    #[inline]
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            width: self.width,
            height: self.height,
        }
    }

    #[inline]
    pub fn union(self, other: Self) -> Self {
        if self.width <= 0.0 || self.height <= 0.0 {
            return other;
        }
        if other.width <= 0.0 || other.height <= 0.0 {
            return self;
        }
        let min_x = self.x.min(other.x);
        let min_y = self.y.min(other.y);
        let max_x = self.right().max(other.right());
        let max_y = self.bottom().max(other.bottom());
        Self {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    }
}

/// 4-sided inset margins or padding.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Insets {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

impl Insets {
    pub const ZERO: Self = Self {
        top: 0.0,
        bottom: 0.0,
        left: 0.0,
        right: 0.0,
    };

    #[inline]
    pub const fn all(val: f32) -> Self {
        Self {
            top: val,
            bottom: val,
            left: val,
            right: val,
        }
    }

    #[inline]
    pub const fn new(top: f32, bottom: f32, left: f32, right: f32) -> Self {
        Self {
            top,
            bottom,
            left,
            right,
        }
    }
}
