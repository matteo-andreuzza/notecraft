//! Ink strokes (decision D4, `docs/plan.md` §4.10): one stroke is one page object.
//!
//! A stroke stores its points relative to its own position (the top-left of the points' box),
//! so moving it is one assignment and copies stay cheap. Pressure is per point, 0–1; a mouse
//! (or any device that reports none) gives 0.5, which draws exactly the nominal width. Every
//! number can come from a hostile file or agent: [`Ink::normalize`] restores the invariants.

use notecraft_color::Rgba;
use notecraft_geom::{Point, Rect, finite};
use serde::{Deserialize, Serialize};

use crate::Id;
use crate::tags::TagUse;

/// At most this many points per stroke (longer input is simplified, then cut, before it is
/// stored).
pub const MAX_INK_POINTS: usize = 20_000;
/// Thinnest nominal tip width, points.
pub const MIN_INK_WIDTH: f32 = 0.1;
/// Widest nominal tip width, points.
pub const MAX_INK_WIDTH: f32 = 72.0;
/// Pressure when the device reports none.
pub const DEFAULT_PRESSURE: f32 = 0.5;

/// What drew a stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum InkTool {
    /// An opaque pen whose width follows pressure.
    #[default]
    Pen,
    /// A translucent highlighter of constant width.
    Highlighter,
}

/// One sampled point, relative to the stroke's position; stored as `[x, y, pressure]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "[f32; 3]", into = "[f32; 3]")]
pub struct InkPoint {
    /// Horizontal offset from the stroke's position, points.
    pub x: f32,
    /// Vertical offset from the stroke's position, points.
    pub y: f32,
    /// Pen pressure, 0–1.
    pub pressure: f32,
}

impl From<[f32; 3]> for InkPoint {
    fn from(a: [f32; 3]) -> InkPoint {
        InkPoint { x: a[0], y: a[1], pressure: a[2] }
    }
}

impl From<InkPoint> for [f32; 3] {
    fn from(p: InkPoint) -> [f32; 3] {
        [p.x, p.y, p.pressure]
    }
}

/// Round to 1/100 pt: invisible on screen, and it keeps `section.json` short.
fn centi(v: f32) -> f32 {
    (finite(v) * 100.0).round() / 100.0
}

/// A pressure from anywhere: clamped to 0–1, NaN → [`DEFAULT_PRESSURE`].
pub fn clean_pressure(p: f32) -> f32 {
    if p.is_nan() { DEFAULT_PRESSURE } else { p.clamp(0.0, 1.0) }
}

/// An ink stroke on the page ([MS-ONE] ink data; one stroke per object, decision D4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ink {
    /// Stable id.
    pub id: Id,
    /// Top-left of the points' box in page space (the pen width spills over it).
    pub pos: Point,
    /// The points, relative to `pos`.
    pub points: Vec<InkPoint>,
    /// Colour (a highlighter's is drawn translucent).
    pub color: Rgba,
    /// Nominal tip width, points.
    pub width: f32,
    /// Pen or highlighter.
    #[serde(default)]
    pub tool: InkTool,
    /// A hyperlink on the stroke (Link… on its menu).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// Note tags on the stroke (decision D5: a tagged drawing carries its tags on one of its
    /// strokes; absent in files written before tags on ink, so format 4 is unchanged).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagUse>,
}

impl Ink {
    /// A stroke through `page_points` (`(page point, pressure)`), normalised.
    pub fn new(id: Id, page_points: &[(Point, f32)], color: Rgba, width: f32, tool: InkTool) -> Ink {
        let points = page_points.iter().map(|(p, pr)| InkPoint { x: p.x, y: p.y, pressure: *pr }).collect();
        let mut ink = Ink { id, pos: Point::ZERO, points, color, width, tool, link: None, tags: Vec::new() };
        ink.normalize();
        ink
    }

    /// The drawn width at `pressure`: pens `width × (0.4 + 1.2 × pressure)` (0.5 draws `width`
    /// exactly; the curve is unverified), highlighters `width` whatever the pressure.
    pub fn width_at(&self, pressure: f32) -> f32 {
        match self.tool {
            InkTool::Pen => self.width * (0.4 + 1.2 * clean_pressure(pressure)),
            InkTool::Highlighter => self.width,
        }
    }

    /// The points in page space.
    pub fn page_points(&self) -> impl Iterator<Item = Point> + '_ {
        self.points.iter().map(|p| Point::new(self.pos.x + p.x, self.pos.y + p.y))
    }

    /// The box the drawn stroke covers in page space (the points' box grown by half the widest
    /// drawn width).
    pub fn bounds(&self) -> Rect {
        let (mut w, mut h, mut widest) = (0.0f32, 0.0f32, 0.0f32);
        for p in &self.points {
            w = w.max(p.x);
            h = h.max(p.y);
            widest = widest.max(self.width_at(p.pressure));
        }
        Rect::from_points(self.pos, Point::new(self.pos.x + w, self.pos.y + h)).inflate(widest / 2.0)
    }

    /// Enforce the invariants (hostile files and agents): finite coordinates rounded to
    /// 1/100 pt, pressure 0–1, at most [`MAX_INK_POINTS`] points, the width in range, tags
    /// deduplicated and capped ([`crate::tags::clean_uses`]), and the points re-anchored so their box starts at `pos`.
    pub fn normalize(&mut self) {
        crate::media::clean_link(&mut self.link);
        crate::tags::clean_uses(&mut self.tags);
        self.points.truncate(MAX_INK_POINTS);
        self.width = if self.width.is_nan() { 1.0 } else { self.width.clamp(MIN_INK_WIDTH, MAX_INK_WIDTH) };
        let pos = self.pos.sanitized();
        let (mut min_x, mut min_y) = (f32::INFINITY, f32::INFINITY);
        for p in &mut self.points {
            p.x = finite(pos.x + finite(p.x));
            p.y = finite(pos.y + finite(p.y));
            p.pressure = (clean_pressure(p.pressure) * 1000.0).round() / 1000.0;
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
        }
        if self.points.is_empty() {
            self.pos = pos;
            return;
        }
        self.pos = Point::new(centi(min_x), centi(min_y));
        for p in &mut self.points {
            p.x = centi(p.x - self.pos.x).max(0.0);
            p.y = centi(p.y - self.pos.y).max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_reanchors_and_cleans() {
        let pts = [(Point::new(10.0, 20.0), 0.5), (Point::new(4.0, 30.0), f32::NAN), (Point::new(f32::INFINITY, 25.0), 7.0)];
        let ink = Ink::new(Id(1), &pts, Rgba::BLACK, f32::NAN, InkTool::Pen);
        assert_eq!(ink.pos, Point::new(4.0, 20.0));
        assert_eq!(ink.points[0], InkPoint { x: 6.0, y: 0.0, pressure: 0.5 });
        assert_eq!(ink.points[1].pressure, DEFAULT_PRESSURE);
        assert_eq!(ink.points[2].pressure, 1.0);
        assert!(ink.points.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        assert_eq!(ink.width, 1.0);
        let json = serde_json::to_string(&ink.points[0]).unwrap();
        assert_eq!(json, "[6.0,0.0,0.5]");
    }

    #[test]
    fn width_follows_pressure_for_pens_only() {
        let mut ink = Ink::new(Id(1), &[(Point::ZERO, 0.5)], Rgba::BLACK, 2.0, InkTool::Pen);
        assert_eq!(ink.width_at(0.5), 2.0);
        assert!(ink.width_at(1.0) > ink.width_at(0.1));
        ink.tool = InkTool::Highlighter;
        assert_eq!(ink.width_at(1.0), 2.0);
        let b = ink.bounds();
        assert_eq!((b.min, b.max), (Point::new(-1.0, -1.0), Point::new(1.0, 1.0)));
    }
    #[test]
    fn tags_are_optional_deduplicated_and_capped() {
        // Strokes written before tags on ink (format 4) load with none and save without the key.
        let ink = Ink::new(Id(1), &[(Point::ZERO, 0.5)], Rgba::BLACK, 1.0, InkTool::Pen);
        let json = serde_json::to_value(&ink).unwrap();
        assert!(json.get("tags").is_none());
        let back: Ink = serde_json::from_value(json).unwrap();
        assert!(back.tags.is_empty());
        // A hostile file: 40 distinct tags, each repeated.
        let mut ink = back;
        ink.tags =
            (0..100).map(|i| TagUse { key: format!("k{}", i % 40), name: "X".into(), symbol: crate::TagSymbol::Star, checked: false }).collect();
        ink.normalize();
        assert_eq!(ink.tags.len(), crate::tags::MAX_OBJECT_TAGS);
        assert_eq!((ink.tags[0].key.as_str(), ink.tags[31].key.as_str()), ("k0", "k31"));
    }
}
