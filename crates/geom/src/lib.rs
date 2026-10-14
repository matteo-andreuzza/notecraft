//! NoteCraft geometry (layer **L0**: depends on nothing in the workspace).
//!
//! Every position on a OneNote-style page is a point in **page space**: the origin is the page's
//! top-left corner, `x` grows to the right, `y` grows downwards, and the unit is the typographic
//! **point** (1/72 inch). The free-form canvas, note containers, ink and pictures all live in
//! this space; the UI converts to screen pixels with the zoom factor and the display scale.
//!
//! Why points: the reference product's file format ([MS-ONE]) stores layout offsets in
//! half-inch units and font sizes in half-points, and both convert exactly to points
//! ([`HALF_INCH`]); points also match the font sizes users type.
//!
//! Numbers arriving from files, commands or agents are hostile: [`finite`] and
//! [`Point::sanitized`] clamp NaN/∞ and absurd magnitudes before they reach layout, so a
//! corrupt offset can never produce an infinite canvas or a NaN-poisoned hit test.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use serde::{Deserialize, Serialize};

/// Points per inch.
pub const INCH: f32 = 72.0;
/// Points per half inch: the layout unit of [MS-ONE] (`OffsetFromParentHoriz`,
/// `LayoutMaxWidth`, …).
pub const HALF_INCH: f32 = 36.0;
/// Points per centimetre.
pub const CM: f32 = 72.0 / 2.54;

/// The largest coordinate magnitude NoteCraft accepts, in points (about 35 m). OneNote pages
/// grow as content is added but stay finite; anything beyond this is treated as corruption and
/// clamped, which keeps `f32` arithmetic exact enough for hit testing (≈ 1/250 pt resolution).
pub const MAX_COORD: f32 = 100_000.0;

/// Clamp a hostile number into `[-MAX_COORD, MAX_COORD]`; NaN becomes `0`.
///
/// ```
/// assert_eq!(notecraft_geom::finite(f32::NAN), 0.0);
/// assert_eq!(notecraft_geom::finite(f32::INFINITY), notecraft_geom::MAX_COORD);
/// assert_eq!(notecraft_geom::finite(12.5), 12.5);
/// ```
pub fn finite(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(-MAX_COORD, MAX_COORD) }
}

/// A non-negative length in points, sanitized like [`finite`].
pub fn length(v: f32) -> f32 {
    finite(v).max(0.0)
}

/// A position in page space (points).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point {
    /// Horizontal offset from the page's left edge.
    pub x: f32,
    /// Vertical offset from the page's top edge (grows downwards).
    pub y: f32,
}

impl Point {
    /// The page origin.
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    /// A point at `(x, y)`.
    pub const fn new(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    /// This point with each coordinate passed through [`finite`].
    pub fn sanitized(self) -> Point {
        Point { x: finite(self.x), y: finite(self.y) }
    }

    /// Translate by `v`.
    pub fn offset(self, v: Vec2) -> Point {
        Point { x: self.x + v.x, y: self.y + v.y }.sanitized()
    }

    /// The vector from `other` to `self`.
    pub fn minus(self, other: Point) -> Vec2 {
        Vec2 { x: self.x - other.x, y: self.y - other.y }
    }

    /// Euclidean distance to `other`.
    pub fn distance(self, other: Point) -> f32 {
        self.minus(other).length()
    }
}

/// A displacement in page space (points).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Vec2 {
    /// Horizontal component.
    pub x: f32,
    /// Vertical component.
    pub y: f32,
}

impl Vec2 {
    /// The zero vector.
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    /// A vector `(x, y)`.
    pub const fn new(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }

    /// Length of the vector.
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Scale both components.
    pub fn scale(self, k: f32) -> Vec2 {
        Vec2 { x: self.x * k, y: self.y * k }
    }
}

/// A width and height in points (never negative once sanitized).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Size {
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Size {
    /// A size `w × h`, sanitized with [`length`].
    pub fn new(w: f32, h: f32) -> Size {
        Size { w: length(w), h: length(h) }
    }
}

/// An axis-aligned rectangle in page space, stored as its min (top-left) and max
/// (bottom-right) corners. Invariant after [`Rect::from_points`]/[`Rect::new`]: `min <= max`
/// on both axes.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    /// Top-left corner.
    pub min: Point,
    /// Bottom-right corner.
    pub max: Point,
}

impl Rect {
    /// The empty rectangle at the origin.
    pub const ZERO: Rect = Rect { min: Point::ZERO, max: Point::ZERO };

    /// A rectangle from its top-left corner and size.
    pub fn new(origin: Point, size: Size) -> Rect {
        let o = origin.sanitized();
        let s = Size::new(size.w, size.h);
        Rect::from_points(o, Point::new(o.x + s.w, o.y + s.h))
    }

    /// The rectangle spanned by two corners, in any order.
    pub fn from_points(a: Point, b: Point) -> Rect {
        let (a, b) = (a.sanitized(), b.sanitized());
        Rect { min: Point::new(a.x.min(b.x), a.y.min(b.y)), max: Point::new(a.x.max(b.x), a.y.max(b.y)) }
    }

    /// Width.
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    /// Height.
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    /// Size.
    pub fn size(&self) -> Size {
        Size::new(self.width(), self.height())
    }

    /// Whether `p` lies inside (edges inclusive).
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    /// Whether the two rectangles overlap (touching edges count).
    pub fn intersects(&self, o: &Rect) -> bool {
        self.min.x <= o.max.x && o.min.x <= self.max.x && self.min.y <= o.max.y && o.min.y <= self.max.y
    }

    /// The smallest rectangle containing both.
    pub fn union(&self, o: &Rect) -> Rect {
        Rect { min: Point::new(self.min.x.min(o.min.x), self.min.y.min(o.min.y)), max: Point::new(self.max.x.max(o.max.x), self.max.y.max(o.max.y)) }
    }

    /// Grow (or shrink, for negative `d`) on every side; never inverts.
    pub fn inflate(&self, d: f32) -> Rect {
        let d = finite(d);
        let c = self.center();
        let hw = (self.width() / 2.0 + d).max(0.0);
        let hh = (self.height() / 2.0 + d).max(0.0);
        Rect::from_points(Point::new(c.x - hw, c.y - hh), Point::new(c.x + hw, c.y + hh))
    }

    /// Move by `v`.
    pub fn translate(&self, v: Vec2) -> Rect {
        Rect::from_points(self.min.offset(v), self.max.offset(v))
    }

    /// The centre point.
    pub fn center(&self) -> Point {
        Point::new((self.min.x + self.max.x) / 2.0, (self.min.y + self.max.y) / 2.0)
    }
}

/// Measurement units the user can pick for dialogs (paper size, margins).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Unit {
    /// Inches (the reference's default in en-US).
    #[default]
    Inches,
    /// Centimetres.
    Centimeters,
    /// Millimetres.
    Millimeters,
    /// Points.
    Points,
    /// Picas (12 pt).
    Picas,
}

impl Unit {
    /// Points per one of this unit.
    pub fn points(self) -> f32 {
        match self {
            Unit::Inches => INCH,
            Unit::Centimeters => CM,
            Unit::Millimeters => CM / 10.0,
            Unit::Points => 1.0,
            Unit::Picas => 12.0,
        }
    }

    /// Convert `v` of this unit to points (sanitized).
    pub fn to_points(self, v: f32) -> f32 {
        finite(v * self.points())
    }

    /// Convert `pt` points to this unit.
    pub fn from_points(self, pt: f32) -> f32 {
        finite(pt) / self.points()
    }

    /// Short suffix shown after numbers (`"`, `cm`, …).
    pub fn suffix(self) -> &'static str {
        match self {
            Unit::Inches => "\"",
            Unit::Centimeters => " cm",
            Unit::Millimeters => " mm",
            Unit::Points => " pt",
            Unit::Picas => " pi",
        }
    }
}

/// Convert a [MS-ONE] half-inch layout value to points.
pub fn half_inches_to_points(v: f32) -> f32 {
    finite(v * HALF_INCH)
}

/// Convert points to [MS-ONE] half-inch layout units.
pub fn points_to_half_inches(pt: f32) -> f32 {
    finite(pt) / HALF_INCH
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn hostile_numbers_are_clamped() {
        assert_eq!(finite(f32::NAN), 0.0);
        assert_eq!(finite(f32::NEG_INFINITY), -MAX_COORD);
        assert_eq!(length(-5.0), 0.0);
        let r = Rect::new(Point::new(f32::NAN, 1e30), Size::new(f32::INFINITY, -3.0));
        assert!(r.min.x.is_finite() && r.max.y.is_finite());
        assert!(r.width() >= 0.0 && r.height() >= 0.0);
    }

    #[test]
    fn rect_ops() {
        let a = Rect::from_points(Point::new(10.0, 10.0), Point::new(0.0, 0.0));
        assert_eq!(a.min, Point::ZERO);
        assert!(a.contains(Point::new(10.0, 10.0)));
        let b = a.translate(Vec2::new(5.0, 5.0));
        assert!(a.intersects(&b));
        assert_eq!(a.union(&b).max, Point::new(15.0, 15.0));
        assert_eq!(a.inflate(-100.0).width(), 0.0);
        assert_eq!(a.inflate(2.0).width(), 14.0);
    }

    #[test]
    fn units_round_trip() {
        assert_eq!(half_inches_to_points(2.0), 72.0);
        assert_eq!(points_to_half_inches(72.0), 2.0);
        for u in [Unit::Inches, Unit::Centimeters, Unit::Millimeters, Unit::Points, Unit::Picas] {
            assert!((u.from_points(u.to_points(3.25)) - 3.25).abs() < 1e-4, "{u:?}");
        }
    }

    #[test]
    fn serde_round_trip() {
        let r = Rect::new(Point::new(1.0, 2.0), Size::new(3.0, 4.0));
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(serde_json::from_str::<Rect>(&s).unwrap(), r);
    }

    proptest! {
        // Property: whatever numbers come in, a rectangle is finite and never inverted.
        #[test]
        fn rect_invariant(x in any::<f32>(), y in any::<f32>(), w in any::<f32>(), h in any::<f32>(), d in any::<f32>()) {
            let r = Rect::new(Point::new(x, y), Size::new(w, h)).inflate(d);
            prop_assert!(r.min.x <= r.max.x && r.min.y <= r.max.y);
            prop_assert!(r.min.x.is_finite() && r.max.x.is_finite() && r.min.y.is_finite() && r.max.y.is_finite());
        }
    }
}
