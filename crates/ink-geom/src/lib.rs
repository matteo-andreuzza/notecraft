//! NoteCraft ink geometry (layer **L0**: depends only on `notecraft-geom`).
//!
//! The pure geometry under ink strokes (decision D4, `docs/plan.md` §4.10), kept apart from the
//! document model so the canvas, the engine and a future renderer share one tested set of
//! routines: distances from a point to a polyline (hit tests, the stroke eraser), polygon
//! containment (the lasso), Ramer–Douglas–Peucker simplification (a finished stroke is stored
//! with far fewer points than the pointer delivered), cutting a polyline by a disc (the partial
//! erasers) and the left/right outline of a variable-width stroke (drawing with pressure).
//!
//! Everything works in page points. Inputs come from pointers, files and agents, so every
//! routine accepts any slice (empty, one point, repeated points, huge coordinates) without
//! panicking, recursing or allocating more than a constant factor of its input; NaN inputs
//! are the caller's to sanitize (`geom::finite`), but they still never panic here.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use notecraft_geom::{Point, Rect};

/// Distance from `p` to the segment `a`–`b`.
pub fn seg_dist(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > f32::EPSILON { (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(Point::new(a.x + t * dx, a.y + t * dy))
}

/// Distance from `p` to the polyline `pts` (a single point counts as a dot);
/// `f32::INFINITY` for an empty slice.
pub fn polyline_dist(pts: &[Point], p: Point) -> f32 {
    match pts {
        [] => f32::INFINITY,
        [a] => p.distance(*a),
        _ => pts.windows(2).map(|w| seg_dist(p, w[0], w[1])).fold(f32::INFINITY, f32::min),
    }
}

/// The smallest rectangle holding every point, or `None` for an empty slice.
pub fn bounds(pts: &[Point]) -> Option<Rect> {
    let first = *pts.first()?;
    Some(pts.iter().fold(Rect::from_points(first, first), |r, p| {
        Rect::from_points(Point::new(r.min.x.min(p.x), r.min.y.min(p.y)), Point::new(r.max.x.max(p.x), r.max.y.max(p.y)))
    }))
}

/// Whether `p` is inside the closed polygon `poly` (even–odd rule; the last point joins the
/// first). Fewer than three points enclose nothing.
pub fn in_polygon(poly: &[Point], p: Point) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for (i, a) in poly.iter().enumerate() {
        let b = poly[j];
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if p.x < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Ramer–Douglas–Peucker: the indices of the points to keep so that no dropped point is
/// farther than `tolerance` from the simplified line. The first and last points are always
/// kept. Iterative (an explicit stack), so a hostile 10^6-point stroke can't overflow the
/// call stack.
pub fn simplify(pts: &[Point], tolerance: f32) -> Vec<usize> {
    let n = pts.len();
    if n <= 2 {
        return (0..n).collect();
    }
    let tol = if tolerance.is_finite() { tolerance.max(0.0) } else { 0.0 };
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((s, e)) = stack.pop() {
        if e <= s + 1 {
            continue;
        }
        let (mut far, mut at) = (-1.0f32, s);
        for (k, p) in pts.iter().enumerate().take(e).skip(s + 1) {
            let d = seg_dist(*p, pts[s], pts[e]);
            if d > far {
                (far, at) = (d, k);
            }
        }
        if far > tol {
            keep[at] = true;
            stack.push((s, at));
            stack.push((at, e));
        }
    }
    keep.iter().enumerate().filter_map(|(i, k)| k.then_some(i)).collect()
}

/// A position along a polyline: segment `seg` (from point `seg` to point `seg + 1`) at
/// parameter `t` in 0–1. The caller interpolates its own per-point data (pressure) with it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct At {
    /// Segment index.
    pub seg: usize,
    /// Parameter along the segment, 0–1.
    pub t: f32,
}

/// Where a segment runs inside the disc (`c`, `r`), as a parameter interval clipped to 0–1.
fn seg_in_disc(a: Point, b: Point, c: Point, r: f32) -> Option<(f32, f32)> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (fx, fy) = (a.x - c.x, a.y - c.y);
    let qa = dx * dx + dy * dy;
    let qc = fx * fx + fy * fy - r * r;
    if qa <= f32::EPSILON {
        return (qc <= 0.0).then_some((0.0, 1.0));
    }
    let qb = 2.0 * (fx * dx + fy * dy);
    let disc = qb * qb - 4.0 * qa * qc;
    // NaN (hostile input) fails the test as well as a negative discriminant.
    if disc.is_nan() || disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    let (t0, t1) = ((-qb - sq) / (2.0 * qa), (-qb + sq) / (2.0 * qa));
    if t0 > 1.0 || t1 < 0.0 { None } else { Some((t0.max(0.0), t1.min(1.0))) }
}

/// Cut the polyline `pts` with the disc of centre `c` and radius `r` (a partial eraser).
/// Returns `None` when the disc touches nothing, otherwise the pieces left outside it, in
/// order, each as the positions to keep (its cut ends and the original points between them).
/// Every piece has at least two positions; an empty list means the whole stroke went.
pub fn cut_circle(pts: &[Point], c: Point, r: f32) -> Option<Vec<Vec<At>>> {
    let n = pts.len();
    match n {
        0 => return None,
        1 => return (pts[0].distance(c) <= r).then(Vec::new),
        _ => {}
    }
    // Removed intervals along the arc parameter s = seg + t, in order; touching ones merge.
    let mut removed: Vec<(f32, f32)> = Vec::new();
    for (i, w) in pts.windows(2).enumerate() {
        if let Some((t0, t1)) = seg_in_disc(w[0], w[1], c, r) {
            let (s0, s1) = (i as f32 + t0, i as f32 + t1);
            match removed.last_mut() {
                Some(last) if s0 <= last.1 + 1e-6 => last.1 = last.1.max(s1),
                _ => removed.push((s0, s1)),
            }
        }
    }
    if removed.is_empty() {
        return None;
    }
    let end = (n - 1) as f32;
    let at = |s: f32| {
        let seg = (s.floor().max(0.0) as usize).min(n - 2);
        At { seg, t: (s - seg as f32).clamp(0.0, 1.0) }
    };
    let mut pieces = Vec::new();
    let mut from = 0.0f32;
    for (s0, s1) in removed.iter().copied().chain(std::iter::once((end + 1.0, end + 1.0))) {
        let to = s0.min(end);
        if to - from > 1e-4 {
            let mut piece = vec![at(from)];
            let first_vertex = from.floor() as usize + 1;
            for k in first_vertex..n {
                if k as f32 >= to {
                    break;
                }
                piece.push(At { seg: k.min(n - 2), t: if k == n - 1 { 1.0 } else { 0.0 } });
            }
            piece.push(at(to));
            pieces.push(piece);
        }
        from = s1;
        if from >= end {
            break;
        }
    }
    Some(pieces)
}

/// The point at `at` along `pts` (clamped to the polyline).
pub fn point_at(pts: &[Point], at: At) -> Point {
    let Some(a) = pts.get(at.seg).or(pts.last()) else { return Point::ZERO };
    let b = pts.get(at.seg + 1).unwrap_or(a);
    Point::new(a.x + (b.x - a.x) * at.t, a.y + (b.y - a.y) * at.t)
}

/// The left and right edges of a stroke drawn along `pts` with `half_widths[i]` on each side
/// of point `i` (a missing width counts as the last one, or 0.5): one pair per point, ready for
/// a triangle strip. Repeated points reuse the previous direction.
pub fn outline(pts: &[Point], half_widths: &[f32]) -> Vec<(Point, Point)> {
    let n = pts.len();
    let mut out = Vec::with_capacity(n);
    let mut dir = (1.0f32, 0.0f32);
    for i in 0..n {
        let prev = pts[i.saturating_sub(1)];
        let next = pts[(i + 1).min(n - 1)];
        let (dx, dy) = (next.x - prev.x, next.y - prev.y);
        let len = (dx * dx + dy * dy).sqrt();
        if len > 1e-6 {
            dir = (dx / len, dy / len);
        }
        let hw = half_widths.get(i).or(half_widths.last()).copied().unwrap_or(0.5);
        let (nx, ny) = (-dir.1 * hw, dir.0 * hw);
        let p = pts[i];
        out.push((Point::new(p.x + nx, p.y + ny), Point::new(p.x - nx, p.y - ny)));
    }
    out
}

/// `p` turned `degrees` clockwise (on screen, y down) about `c`.
pub fn rotate(p: Point, c: Point, degrees: f32) -> Point {
    let (s, co) = degrees.to_radians().sin_cos();
    let (x, y) = (p.x - c.x, p.y - c.y);
    Point::new(c.x + x * co - y * s, c.y + x * s + y * co)
}

#[cfg(test)]
mod tests;
