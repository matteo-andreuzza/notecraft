//! Ink to Shape (`R.Draw.Convert.InkToShape`): a hand-drawn stroke recognised as a line,
//! triangle, rectangle or square, diamond, other four-sided shape, pentagon, hexagon, ellipse
//! or circle, and redrawn neatly.
//!
//! The reference recognises shapes drawn in one stroke and gives the neat shape the pen's
//! colour (Microsoft's "Convert ink to shapes" pages); how close a drawing must be, and how
//! much it is straightened, is not documented. The rules here (unverified) are:
//!
//! - an open stroke that stays within 8% of its length from the line between its ends becomes
//!   that line;
//! - a closed stroke (its ends within a quarter of its size) is compared with the polygon of
//!   its corners and with the ellipse in its box, and becomes the closer one when that one is
//!   close enough; polygons keep the corners drawn, except that a four-sided shape with nearly
//!   level and upright sides becomes a rectangle (a square when its sides are within 10%), and
//!   one with nearly level and upright diagonals a diamond; an ellipse whose axes are within
//!   12% becomes a circle;
//! - anything smaller than 8 pt, or that fits neither, stays as drawn (handwriting must not
//!   turn into shapes).

use notecraft_doc::geom::Point;
use notecraft_ink_geom as geo;

/// Strokes smaller than this (points, their box's longer side) are never shapes.
pub const MIN_SHAPE_SIZE: f32 = 8.0;
/// At most this many samples are examined (a longer gesture is not a shape).
const MAX_RECOGNIZED: usize = 20_000;

/// A recognised shape: its name and its points as one stroke (closed shapes end where they
/// start).
#[derive(Debug, Clone, PartialEq)]
pub struct Recognized {
    /// `line`, `triangle`, `rectangle`, `square`, `diamond`, `quadrilateral`, `pentagon`,
    /// `hexagon`, `ellipse` or `circle`.
    pub kind: &'static str,
    /// The neat shape, in page points.
    pub points: Vec<Point>,
}

/// The shape `pts` (one stroke, page points) was drawn as, or `None` to keep it as drawn.
pub fn recognize(pts: &[Point]) -> Option<Recognized> {
    let pts: Vec<Point> = pts.iter().copied().filter(|p| p.x.is_finite() && p.y.is_finite()).collect();
    if pts.len() < 2 || pts.len() > MAX_RECOGNIZED {
        return None;
    }
    let b = geo::bounds(&pts)?;
    let size = b.width().max(b.height());
    if !size.is_finite() || size < MIN_SHAPE_SIZE {
        return None;
    }
    let (first, last) = (*pts.first()?, *pts.last()?);
    if dist(first, last) > size * 0.25 {
        return line(&pts, first, last);
    }
    closed(&pts, size)
}

fn dist(a: Point, b: Point) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn line(pts: &[Point], a: Point, b: Point) -> Option<Recognized> {
    let len = dist(a, b);
    if len < MIN_SHAPE_SIZE {
        return None;
    }
    let far = pts.iter().map(|p| geo::seg_dist(*p, a, b)).fold(0.0f32, f32::max);
    (far <= len * 0.08).then(|| Recognized { kind: "line", points: vec![a, b] })
}

/// The turn at `b` between `a→b` and `b→c`, in degrees (0 = straight on).
fn turn(a: Point, b: Point, c: Point) -> f32 {
    let (u, v) = ((b.x - a.x, b.y - a.y), (c.x - b.x, c.y - b.y));
    let cross = u.0 * v.1 - u.1 * v.0;
    let dot = u.0 * v.0 + u.1 * v.1;
    cross.atan2(dot).abs().to_degrees()
}

/// The corners of a closed stroke: Ramer–Douglas–Peucker at 8% of its size, then corners
/// that barely turn (the arbitrary start of the loop, wobbles) or sit on top of a neighbour
/// merged away. Bounded: at most `pts.len()` passes, each removing a corner.
fn corners(pts: &[Point], size: f32) -> Vec<Point> {
    let keep = geo::simplify(pts, size * 0.08);
    let mut c: Vec<Point> = keep.iter().filter_map(|&i| pts.get(i).copied()).collect();
    // The loop's two ends are one corner.
    if c.len() >= 2 && c.first().zip(c.last()).is_some_and(|(a, b)| dist(*a, *b) <= size * 0.25) {
        c.pop();
    }
    for _ in 0..pts.len() {
        let n = c.len();
        if n < 3 {
            break;
        }
        let weakest = (0..n)
            .filter_map(|i| {
                let (a, p, z) = (c.get((i + n - 1) % n)?, c.get(i)?, c.get((i + 1) % n)?);
                let short = dist(*a, *p).min(dist(*p, *z)) < size * 0.12;
                let t = turn(*a, *p, *z);
                (t < 30.0 || short && t < 60.0).then_some((i, t))
            })
            .min_by(|x, y| x.1.total_cmp(&y.1));
        match weakest {
            Some((i, _)) => {
                c.remove(i);
            }
            None => break,
        }
    }
    c
}

/// Mean distance of `pts` from the closed polygon `poly`.
fn polygon_error(pts: &[Point], poly: &[Point]) -> f32 {
    let mut ring = poly.to_vec();
    if let Some(f) = poly.first() {
        ring.push(*f);
    }
    pts.iter().map(|p| geo::polyline_dist(&ring, *p)).sum::<f32>() / pts.len().max(1) as f32
}

/// Mean distance (approximately) of `pts` from the ellipse inscribed in their box.
fn ellipse_error(pts: &[Point], c: Point, rx: f32, ry: f32) -> f32 {
    let mean_r = (rx + ry) / 2.0;
    pts.iter()
        .map(|p| {
            let (dx, dy) = ((p.x - c.x) / rx, (p.y - c.y) / ry);
            ((dx * dx + dy * dy).sqrt() - 1.0).abs() * mean_r
        })
        .sum::<f32>()
        / pts.len().max(1) as f32
}

/// Whether the direction `a→b` is within `deg` degrees of level or upright.
fn axis_aligned(a: Point, b: Point, deg: f32) -> bool {
    let ang = (b.y - a.y).atan2(b.x - a.x).to_degrees().rem_euclid(90.0);
    ang <= deg || ang >= 90.0 - deg
}

fn closed(pts: &[Point], size: f32) -> Option<Recognized> {
    let b = geo::bounds(pts)?;
    let (rx, ry) = (b.width() / 2.0, b.height() / 2.0);
    if rx < 1.0 || ry < 1.0 {
        return None;
    }
    let c = Point::new(b.min.x + rx, b.min.y + ry);
    let poly = corners(pts, size);
    let e_poly = if (3..=6).contains(&poly.len()) { polygon_error(pts, &poly) } else { f32::INFINITY };
    let e_ell = ellipse_error(pts, c, rx, ry);
    let tolerance = size * 0.06;
    if e_ell <= e_poly && e_ell <= tolerance {
        let round = (rx - ry).abs() <= rx.max(ry) * 0.12;
        let (rx, ry, kind) = if round { ((rx + ry) / 2.0, (rx + ry) / 2.0, "circle") } else { (rx, ry, "ellipse") };
        let (a, z) = (Point::new(c.x - rx, c.y - ry), Point::new(c.x + rx, c.y + ry));
        return Some(Recognized { kind, points: crate::shape_points(crate::ShapeKind::Oval, a, z) });
    }
    if e_poly > tolerance {
        return None;
    }
    let close = |mut v: Vec<Point>| {
        if let Some(f) = v.first().copied() {
            v.push(f);
        }
        v
    };
    let kind = match poly.len() {
        3 => "triangle",
        5 => "pentagon",
        6 => "hexagon",
        _ => return Some(quad(&poly, b).unwrap_or_else(|| Recognized { kind: "quadrilateral", points: close(poly.clone()) })),
    };
    Some(Recognized { kind, points: close(poly) })
}

/// A four-sided shape straightened into a rectangle, square or diamond, when it is close to one.
fn quad(q: &[Point], b: notecraft_doc::geom::Rect) -> Option<Recognized> {
    let [p0, p1, p2, p3] = <[Point; 4]>::try_from(q).ok()?;
    let sides = [(p0, p1), (p1, p2), (p2, p3), (p3, p0)];
    if sides.iter().all(|(a, z)| axis_aligned(*a, *z, 12.0)) {
        let (x0, y0) = (b.min.x, b.min.y);
        let (mut w, mut h) = (b.width(), b.height());
        let kind = if (w - h).abs() <= w.max(h) * 0.1 {
            let side = (w + h) / 2.0;
            (w, h) = (side, side);
            "square"
        } else {
            "rectangle"
        };
        let a = Point::new(x0, y0);
        let z = Point::new(x0 + w, y0 + h);
        return Some(Recognized { kind, points: crate::shape_points(crate::ShapeKind::Rectangle, a, z) });
    }
    if axis_aligned(p0, p2, 12.0) && axis_aligned(p1, p3, 12.0) {
        return Some(Recognized { kind: "diamond", points: crate::shape_points(crate::ShapeKind::Diamond, b.min, b.max) });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand-drawn trace of the closed polygon `corners`, starting halfway along its first
    /// side, `per_side` samples a side, wobbling by up to `wobble` points.
    fn traced(corners: &[(f32, f32)], per_side: usize, wobble: f32) -> Vec<Point> {
        let n = corners.len();
        let mut out = Vec::new();
        for k in 0..=n {
            let (a, b) = (corners[k % n], corners[(k + 1) % n]);
            let (from, to) = if k == 0 {
                (per_side / 2, per_side)
            } else if k == n {
                (0, per_side / 2)
            } else {
                (0, per_side)
            };
            for i in from..to {
                let t = i as f32 / per_side as f32;
                let j = out.len() as f32;
                out.push(Point::new(a.0 + (b.0 - a.0) * t + wobble * (j * 0.7).sin(), a.1 + (b.1 - a.1) * t + wobble * (j * 1.3).cos()));
            }
        }
        out
    }

    fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32, wobble: f32, sweep: f32) -> Vec<Point> {
        (0..=120)
            .map(|i| {
                let t = i as f32 / 120.0 * sweep;
                Point::new(cx + rx * t.cos() + wobble * (i as f32 * 0.9).sin(), cy + ry * t.sin() + wobble * (i as f32 * 1.7).cos())
            })
            .collect()
    }

    fn kind(pts: &[Point]) -> Option<&'static str> {
        recognize(pts).map(|r| r.kind)
    }

    #[test]
    fn closed_strokes_become_their_shapes() {
        // The recognised set (R.Draw.Convert.InkToShape; the reference's thresholds are unverified).
        assert_eq!(kind(&traced(&[(0.0, 0.0), (200.0, 0.0), (200.0, 100.0), (0.0, 100.0)], 40, 1.5)), Some("rectangle"));
        assert_eq!(kind(&traced(&[(0.0, 0.0), (100.0, 3.0), (98.0, 104.0), (-2.0, 100.0)], 40, 1.5)), Some("square"));
        assert_eq!(kind(&traced(&[(50.0, 0.0), (100.0, 90.0), (0.0, 90.0)], 40, 1.5)), Some("triangle"));
        assert_eq!(kind(&traced(&[(60.0, 0.0), (120.0, 50.0), (60.0, 100.0), (0.0, 50.0)], 40, 1.5)), Some("diamond"));
        assert_eq!(kind(&traced(&[(30.0, 0.0), (160.0, 0.0), (130.0, 80.0), (0.0, 80.0)], 40, 1.0)), Some("quadrilateral"));
        let penta: Vec<(f32, f32)> = (0..5)
            .map(|i| i as f32 * std::f32::consts::TAU / 5.0 - std::f32::consts::FRAC_PI_2)
            .map(|t| (100.0 + 80.0 * t.cos(), 100.0 + 80.0 * t.sin()))
            .collect();
        assert_eq!(kind(&traced(&penta, 40, 1.0)), Some("pentagon"));
        let hexa: Vec<(f32, f32)> =
            (0..6).map(|i| i as f32 * std::f32::consts::TAU / 6.0).map(|t| (100.0 + 80.0 * t.cos(), 100.0 + 80.0 * t.sin())).collect();
        assert_eq!(kind(&traced(&hexa, 40, 1.0)), Some("hexagon"));
        assert_eq!(kind(&ellipse(100.0, 100.0, 60.0, 58.0, 1.5, std::f32::consts::TAU)), Some("circle"));
        assert_eq!(kind(&ellipse(100.0, 100.0, 120.0, 50.0, 1.5, std::f32::consts::TAU)), Some("ellipse"));
    }

    #[test]
    fn the_neat_shape_fills_the_drawing_and_closes() {
        let r = recognize(&traced(&[(10.0, 20.0), (210.0, 20.0), (210.0, 120.0), (10.0, 120.0)], 40, 1.5)).unwrap();
        assert_eq!(r.points.first(), r.points.last(), "closed");
        let b = geo::bounds(&r.points).unwrap();
        assert!((b.min.x - 10.0).abs() < 4.0 && (b.max.y - 120.0).abs() < 4.0, "{b:?}");
        let c = recognize(&ellipse(100.0, 100.0, 60.0, 56.0, 1.0, std::f32::consts::TAU)).unwrap();
        let b = geo::bounds(&c.points).unwrap();
        assert!((b.width() - b.height()).abs() < 0.5, "a circle is round");
    }

    #[test]
    fn straight_strokes_become_lines() {
        let wobbly: Vec<Point> = (0..60).map(|i| Point::new(i as f32 * 3.0, 50.0 + (i as f32).sin() * 2.0)).collect();
        let r = recognize(&wobbly).unwrap();
        assert_eq!(r.kind, "line");
        assert_eq!(r.points, vec![wobbly[0], wobbly[59]]);
    }

    #[test]
    fn writing_scribbles_and_hostile_input_stay_as_drawn() {
        // An S curve, an open arc, a tiny loop, a zigzag.
        let s_curve: Vec<Point> = (0..80).map(|i| i as f32 / 79.0).map(|t| Point::new(40.0 * (t * std::f32::consts::TAU).sin(), t * 120.0)).collect();
        assert_eq!(kind(&s_curve), None);
        assert_eq!(kind(&ellipse(100.0, 100.0, 60.0, 60.0, 0.5, std::f32::consts::PI)), None, "half a circle");
        assert_eq!(kind(&ellipse(10.0, 10.0, 3.0, 3.0, 0.0, std::f32::consts::TAU)), None, "smaller than MIN_SHAPE_SIZE");
        let zigzag: Vec<Point> = (0..20).map(|i| Point::new(i as f32 * 10.0, if i % 2 == 0 { 0.0 } else { 40.0 })).collect();
        assert_eq!(kind(&zigzag), None);
        let star: Vec<(f32, f32)> = (0..10)
            .map(|i| (i, i as f32 * std::f32::consts::TAU / 10.0))
            .map(|(i, t)| {
                let r = if i % 2 == 0 { 90.0 } else { 35.0 };
                (100.0 + r * t.cos(), 100.0 + r * t.sin())
            })
            .collect();
        assert_eq!(kind(&traced(&star, 12, 0.5)), None, "ten corners is no shape");
        assert_eq!(recognize(&[]), None);
        assert_eq!(recognize(&[Point::new(f32::NAN, 1.0), Point::new(f32::INFINITY, 2.0)]), None);
        let huge: Vec<Point> = (0..MAX_RECOGNIZED + 1).map(|i| Point::new(i as f32, 0.0)).collect();
        assert_eq!(recognize(&huge), None);
    }
}
