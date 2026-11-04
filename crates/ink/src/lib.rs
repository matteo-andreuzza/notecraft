//! NoteCraft ink operations (layer **L2**: depends on the `doc` model and the L0 `ink-geom`).
//!
//! What the Draw tab does to strokes (decision D4, `docs/plan.md` §4.10), as pure functions
//! over [`notecraft_doc::Ink`] so the engine's commands stay thin and every rule is tested
//! here: building a stored stroke from pointer samples (sanitized, simplified, capped), hit
//! tests, the stroke eraser and the partial erasers (a stroke cut into the pieces left outside
//! the eraser), the lasso's rule for picking a stroke, and the turns and flips of the Rotate
//! menu (ink is turned by moving its points; pictures turn as a view property instead).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use notecraft_doc::geom::{Point, Rect};
use notecraft_doc::ink::{MAX_INK_POINTS, clean_pressure};
use notecraft_doc::{Ink, InkPoint};
use notecraft_ink_geom as geo;

/// A finished stroke is simplified with this tolerance, points (invisible at any zoom the
/// canvas offers, and it typically keeps one sample in five).
pub const SIMPLIFY_TOLERANCE: f32 = 0.2;
/// At most this many raw samples are read from one gesture (a hostile agent can't make the
/// simplification allocate without bound); later samples are ignored.
pub const MAX_SAMPLES: usize = 200_000;
/// Pieces shorter than this (points) left by an eraser are dropped as crumbs.
pub const MIN_PIECE: f32 = 0.3;
/// A stroke is picked by the lasso when at least this share of its points is inside the loop
/// (unverified).
pub const LASSO_SHARE: f32 = 0.75;

/// Pointer samples (page point, pressure) cleaned for storage: non-finite points dropped,
/// pressure clamped, at most [`MAX_SAMPLES`], then simplified and cut to
/// [`MAX_INK_POINTS`]. Empty when nothing usable is left.
pub fn clean_samples(samples: &[(Point, f32)]) -> Vec<(Point, f32)> {
    let raw: Vec<(Point, f32)> = samples
        .iter()
        .take(MAX_SAMPLES)
        .filter(|(p, _)| p.x.is_finite() && p.y.is_finite())
        .map(|(p, pr)| (p.sanitized(), clean_pressure(*pr)))
        .collect();
    let pts: Vec<Point> = raw.iter().map(|(p, _)| *p).collect();
    let mut keep = geo::simplify(&pts, SIMPLIFY_TOLERANCE);
    keep.truncate(MAX_INK_POINTS);
    keep.into_iter().filter_map(|i| raw.get(i).copied()).collect()
}

/// The page points of a stroke.
fn points(ink: &Ink) -> Vec<Point> {
    ink.page_points().collect()
}

/// Whether `p` is on the drawn stroke, give or take `slop` points.
pub fn hit(ink: &Ink, p: Point, slop: f32) -> bool {
    geo::polyline_dist(&points(ink), p) <= ink.width / 2.0 + slop.max(0.0)
}

/// Whether an eraser disc (`c`, radius `r`) touches the drawn stroke (the Stroke Eraser).
pub fn touches(ink: &Ink, c: Point, r: f32) -> bool {
    hit(ink, c, r)
}

/// At most this many eraser positions are tried per gesture: a hostile 100,000-pt path is
/// swept with wider steps instead of costing seconds.
pub const MAX_SWEEP: usize = 2_000;

/// The eraser's centre positions along `path`, at most `r / 2` apart (wider on very long
/// paths, see [`MAX_SWEEP`]), so a fast drag doesn't skip over a stroke between two pointer
/// samples.
fn sweep(path: &[Point], r: f32) -> Vec<Point> {
    let total: f32 = path.windows(2).map(|w| w[0].distance(w[1])).sum();
    let step = (r / 2.0).max(0.25).max(total / MAX_SWEEP as f32);
    let mut out = Vec::new();
    if let Some(first) = path.first() {
        out.push(*first);
    }
    for w in path.windows(2) {
        let n = ((w[0].distance(w[1]) / step).ceil() as usize).clamp(1, MAX_SWEEP);
        for k in 1..=n {
            if out.len() > MAX_SWEEP + path.len() {
                return out;
            }
            let t = k as f32 / n as f32;
            out.push(Point::new(w[0].x + (w[1].x - w[0].x) * t, w[0].y + (w[1].y - w[0].y) * t));
        }
    }
    out
}

/// Whether the eraser dragged along `path` with radius `r` touches the stroke.
pub fn swept(ink: &Ink, path: &[Point], r: f32) -> bool {
    let b = ink.bounds().inflate(r.max(0.0));
    sweep(path, r).into_iter().any(|c| b.contains(c) && touches(ink, c, r))
}

/// Partial erasing: the pieces of the stroke left outside the eraser dragged along `path`
/// with radius `r` (the stroke's own half width added, so touching the drawn ink erases it).
/// `None` when the eraser misses the stroke; an empty list when nothing is left. Each piece
/// is a list of (page point, pressure) ready for [`Ink::new`]; crumbs shorter than
/// [`MIN_PIECE`] are dropped.
pub fn erase(ink: &Ink, path: &[Point], r: f32) -> Option<Vec<Vec<(Point, f32)>>> {
    let reach = r.max(0.0) + ink.width / 2.0;
    let b = ink.bounds().inflate(r.max(0.0));
    let mut pieces: Vec<Vec<(Point, f32)>> = vec![ink.page_points().zip(ink.points.iter().map(|p| p.pressure)).collect()];
    let mut changed = false;
    for c in sweep(path, r).into_iter().filter(|c| b.contains(*c)) {
        let mut next = Vec::with_capacity(pieces.len());
        for piece in pieces {
            let pts: Vec<Point> = piece.iter().map(|(p, _)| *p).collect();
            match geo::cut_circle(&pts, c, reach) {
                None => next.push(piece),
                Some(cuts) => {
                    changed = true;
                    for cut in cuts {
                        next.push(cut.iter().map(|at| (geo::point_at(&pts, *at), pressure_at(&piece, *at))).collect());
                    }
                }
            }
        }
        pieces = next;
    }
    if !changed {
        return None;
    }
    pieces.retain(|p| p.windows(2).map(|w| w[0].0.distance(w[1].0)).sum::<f32>() >= MIN_PIECE);
    Some(pieces)
}

fn pressure_at(piece: &[(Point, f32)], at: geo::At) -> f32 {
    let a = piece.get(at.seg).or(piece.last()).map_or(0.5, |p| p.1);
    let b = piece.get(at.seg + 1).map_or(a, |p| p.1);
    a + (b - a) * at.t
}

/// Whether the lasso loop `poly` (page points) picks the stroke: at least [`LASSO_SHARE`] of
/// its points inside (unverified).
pub fn lasso_picks(ink: &Ink, poly: &[Point]) -> bool {
    if ink.points.is_empty() || poly.len() < 3 {
        return false;
    }
    let inside = ink.page_points().filter(|p| geo::in_polygon(poly, *p)).count();
    inside as f32 >= ink.points.len() as f32 * LASSO_SHARE
}

/// The box around several strokes (their drawn bounds), or `None` for none.
pub fn union_bounds<'a>(inks: impl IntoIterator<Item = &'a Ink>) -> Option<Rect> {
    inks.into_iter().map(Ink::bounds).reduce(|a, b| a.union(&b))
}

/// Move every point of `ink` (page space) through `f`, then re-anchor.
fn map_points(ink: &mut Ink, f: impl Fn(Point) -> Point) {
    let pos = ink.pos;
    for p in &mut ink.points {
        let q = f(Point::new(pos.x + p.x, pos.y + p.y));
        *p = InkPoint { x: q.x, y: q.y, pressure: p.pressure };
    }
    ink.pos = Point::ZERO;
    ink.normalize();
}

/// Turn the stroke `degrees` clockwise about `c` (page space). The result is kept on the page
/// (nothing left of or above the origin) by the caller shifting the whole selection.
pub fn rotate(ink: &mut Ink, c: Point, degrees: f32) {
    map_points(ink, |p| geo::rotate(p, c, degrees));
}

/// Mirror the stroke about the vertical (`horizontal`) or horizontal line through `c`.
pub fn flip(ink: &mut Ink, c: Point, horizontal: bool) {
    map_points(ink, |p| if horizontal { Point::new(2.0 * c.x - p.x, p.y) } else { Point::new(p.x, 2.0 * c.y - p.y) });
}

/// Scale the stroke's points by `kx`, `ky` about `origin` (the width stays).
pub fn scale(ink: &mut Ink, origin: Point, kx: f32, ky: f32) {
    let (kx, ky) = (if kx.is_finite() { kx } else { 1.0 }, if ky.is_finite() { ky } else { 1.0 });
    map_points(ink, |p| Point::new(origin.x + (p.x - origin.x) * kx, origin.y + (p.y - origin.y) * ky));
}

pub mod recognize;
pub mod ruler;

#[cfg(test)]
mod tests;

/// The shapes of Draw › Shapes, drawn as ink (the gallery's set; unverified against the
/// reference's exact list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ShapeKind {
    /// A straight line.
    #[default]
    Line,
    /// A line with an arrowhead at its end.
    Arrow,
    /// A line with arrowheads at both ends.
    DoubleArrow,
    /// A rectangle.
    Rectangle,
    /// An ellipse.
    Oval,
    /// An isosceles triangle.
    Triangle,
    /// A right triangle.
    RightTriangle,
    /// A diamond.
    Diamond,
    /// A parallelogram.
    Parallelogram,
}

impl ShapeKind {
    /// Every shape, in gallery order.
    pub const ALL: [ShapeKind; 9] = [
        ShapeKind::Line,
        ShapeKind::Arrow,
        ShapeKind::DoubleArrow,
        ShapeKind::Rectangle,
        ShapeKind::Oval,
        ShapeKind::Triangle,
        ShapeKind::RightTriangle,
        ShapeKind::Diamond,
        ShapeKind::Parallelogram,
    ];

    /// The name shown in the gallery and accepted by commands (`"double arrow"`…).
    pub fn name(self) -> &'static str {
        match self {
            ShapeKind::Line => "Line",
            ShapeKind::Arrow => "Arrow",
            ShapeKind::DoubleArrow => "Double Arrow",
            ShapeKind::Rectangle => "Rectangle",
            ShapeKind::Oval => "Oval",
            ShapeKind::Triangle => "Triangle",
            ShapeKind::RightTriangle => "Right Triangle",
            ShapeKind::Diamond => "Diamond",
            ShapeKind::Parallelogram => "Parallelogram",
        }
    }

    /// The shape named `s` (any case, spaces optional).
    pub fn parse(s: &str) -> Option<ShapeKind> {
        let k: String = s.chars().filter(|c| !c.is_whitespace() && *c != '_' && *c != '-').collect::<String>().to_lowercase();
        ShapeKind::ALL.into_iter().find(|x| x.name().replace(' ', "").to_lowercase() == k)
    }

    /// Whether the shape is a line (dragged end to end) rather than a box.
    pub fn is_line(self) -> bool {
        matches!(self, ShapeKind::Line | ShapeKind::Arrow | ShapeKind::DoubleArrow)
    }
}

/// An arrowhead at `tip` for a line arriving from `from`: the two barbs, as a path that goes
/// out and back through the tip (one stroke).
fn head(from: Point, tip: Point, size: f32) -> [Point; 4] {
    let (dx, dy) = (tip.x - from.x, tip.y - from.y);
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    let (ux, uy) = (dx / len, dy / len);
    let s = size.min(len * 0.5);
    let barb = |side: f32| Point::new(tip.x - s * (ux * 0.866 - side * uy * 0.5), tip.y - s * (uy * 0.866 + side * ux * 0.5));
    [barb(1.0), tip, barb(-1.0), tip]
}

/// The points of `kind` dragged from `a` to `b` (page points), as one stroke: lines run from
/// `a` to `b`, boxes fill the rectangle they span; closed shapes end where they start.
/// Arrowheads are 10 pt (unverified), ovals use 64 segments.
pub fn shape_points(kind: ShapeKind, a: Point, b: Point) -> Vec<Point> {
    let (a, b) = (a.sanitized(), b.sanitized());
    let (x0, y0, x1, y1) = (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let p = Point::new;
    match kind {
        ShapeKind::Line => vec![a, b],
        ShapeKind::Arrow => {
            let mut v = vec![a, b];
            v.extend(head(a, b, 10.0));
            v
        }
        ShapeKind::DoubleArrow => {
            let mut v = vec![a];
            v.extend(head(b, a, 10.0));
            v.push(b);
            v.extend(head(a, b, 10.0));
            v
        }
        ShapeKind::Rectangle => vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1), p(x0, y0)],
        ShapeKind::Oval => {
            let mut v: Vec<Point> = (0..64)
                .map(|i| {
                    let t = i as f32 / 64.0 * std::f32::consts::TAU;
                    p(cx + (x1 - x0) / 2.0 * t.cos(), cy + (y1 - y0) / 2.0 * t.sin())
                })
                .collect();
            // Closed exactly (trigonometry wouldn't land on the first point bit for bit).
            v.push(p(x1, cy));
            v
        }
        ShapeKind::Triangle => vec![p(cx, y0), p(x1, y1), p(x0, y1), p(cx, y0)],
        ShapeKind::RightTriangle => vec![p(x0, y0), p(x0, y1), p(x1, y1), p(x0, y0)],
        ShapeKind::Diamond => vec![p(cx, y0), p(x1, cy), p(cx, y1), p(x0, cy), p(cx, y0)],
        ShapeKind::Parallelogram => {
            let k = (x1 - x0) * 0.25;
            vec![p(x0 + k, y0), p(x1, y0), p(x1 - k, y1), p(x0, y1), p(x0 + k, y0)]
        }
    }
}
