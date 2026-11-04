//! The Draw tab's ruler (decision D7, `docs/plan.md` §4.12): a straightedge lying over the
//! page, [`WIDTH`] × [`LENGTH`] points, centred on `center` and turned by `angle`. A stroke that
//! starts near one of its long edges is laid along that edge ([`Ruler::snap`]), as a pen drawn
//! against a real ruler would be.

use notecraft_doc::geom::Point;

/// The ruler's width, points (unverified).
pub const WIDTH: f32 = 54.0;
/// The ruler's length, points (longer than most windows are wide, as in the reference;
/// unverified).
pub const LENGTH: f32 = 900.0;
/// A stroke that starts within this distance of a long edge, inside or outside, is laid along
/// it, points (unverified).
pub const SNAP: f32 = 12.0;

/// Where the ruler lies: session state, never notebook data.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Ruler {
    /// Its centre, page points.
    pub center: Point,
    /// How far its long edges are turned from horizontal, degrees clockwise on screen (y grows
    /// down), in `0..360`.
    pub angle: f32,
}

impl Ruler {
    /// A ruler at `center`, turned `angle` degrees (both made safe).
    pub fn new(center: Point, angle: f32) -> Ruler {
        let mut r = Ruler { center, angle };
        r.normalize();
        r
    }

    /// Finite coordinates and an angle in `0..360` (NaN → 0).
    pub fn normalize(&mut self) {
        self.center = self.center.sanitized();
        self.angle = if self.angle.is_finite() { self.angle.rem_euclid(360.0) } else { 0.0 };
        // `rem_euclid` can round up to exactly 360 for tiny negative angles.
        if self.angle >= 360.0 {
            self.angle = 0.0;
        }
    }

    /// The unit vectors along the ruler and across it (towards its bottom edge at 0°).
    fn axes(&self) -> ((f32, f32), (f32, f32)) {
        let (s, c) = self.angle.to_radians().sin_cos();
        ((c, s), (-s, c))
    }

    /// A page point in the ruler's own coordinates: (along, across) from its centre.
    pub fn local(&self, p: Point) -> (f32, f32) {
        let (d, n) = self.axes();
        let (x, y) = (p.x - self.center.x, p.y - self.center.y);
        (x * d.0 + y * d.1, x * n.0 + y * n.1)
    }

    /// The page point at (along, across) in the ruler's coordinates.
    pub fn page_point(&self, along: f32, across: f32) -> Point {
        let (d, n) = self.axes();
        Point::new(self.center.x + d.0 * along + n.0 * across, self.center.y + d.1 * along + n.1 * across)
    }

    /// Whether `p` lies on the ruler.
    pub fn contains(&self, p: Point) -> bool {
        let (u, v) = self.local(p);
        u.abs() <= LENGTH / 2.0 && v.abs() <= WIDTH / 2.0
    }

    /// Its corners, clockwise from the top left (as drawn at 0°).
    pub fn corners(&self) -> [Point; 4] {
        let (l, w) = (LENGTH / 2.0, WIDTH / 2.0);
        [self.page_point(-l, -w), self.page_point(l, -w), self.page_point(l, w), self.page_point(-l, w)]
    }

    /// The angle the ruler shows, as a protractor reads it: 0–179°, counterclockwise from
    /// horizontal (a ruler rising to the right reads 45°; unverified).
    pub fn shown_angle(&self) -> u32 {
        let a = (360.0 - self.angle).rem_euclid(180.0).round() as u32;
        a % 180
    }

    /// Lay a stroke along the long edge it starts near: every sample moves onto the line
    /// `half` points outside that edge (so the stroke's side touches the edge), within the
    /// ruler's length; pressure is kept. `None` when the first sample isn't near an edge.
    pub fn snap(&self, samples: &[(Point, f32)], half: f32) -> Option<Vec<(Point, f32)>> {
        let first = samples.first()?.0;
        let (u, v) = self.local(first);
        let edge = WIDTH / 2.0;
        if u.abs() > LENGTH / 2.0 || (v.abs() - edge).abs() > SNAP {
            return None;
        }
        let half = if half.is_finite() { half.clamp(0.0, SNAP) } else { 0.0 };
        let across = if v < 0.0 { -(edge + half) } else { edge + half };
        Some(
            samples
                .iter()
                .map(|(p, pr)| {
                    let (u, _) = self.local(*p);
                    (self.page_point(u.clamp(-LENGTH / 2.0, LENGTH / 2.0), across), *pr)
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn near(a: Point, b: Point) -> bool {
        a.distance(b) < 1e-3
    }

    #[test]
    fn a_stroke_near_an_edge_is_laid_along_it() {
        let r = Ruler::new(Point::new(400.0, 300.0), 0.0);
        // Starts 5 pt above the top edge (y = 273), wobbles, runs past the right end.
        let s = [(Point::new(300.0, 268.0), 0.3), (Point::new(350.0, 262.0), 0.6), (Point::new(2000.0, 250.0), 0.9)];
        let out = r.snap(&s, 1.0).unwrap();
        assert!(out.iter().all(|(p, _)| (p.y - 272.0).abs() < 1e-3), "on the line 1 pt above the edge: {out:?}");
        assert_eq!(out.iter().map(|(p, _)| p.x).collect::<Vec<_>>(), vec![300.0, 350.0, 850.0], "kept within the ruler's length");
        assert_eq!(out.iter().map(|(_, pr)| *pr).collect::<Vec<_>>(), vec![0.3, 0.6, 0.9], "pressure kept");
        // Just inside the bottom edge (y = 327) counts too, and lands below it.
        let out = r.snap(&[(Point::new(400.0, 320.0), 0.5)], 2.0).unwrap();
        assert!(near(out[0].0, Point::new(400.0, 329.0)));
        // Far from both edges, or beyond an end: drawn as it is.
        assert!(r.snap(&[(Point::new(400.0, 300.0), 0.5)], 1.0).is_none());
        assert!(r.snap(&[(Point::new(400.0, 240.0), 0.5)], 1.0).is_none());
        assert!(r.snap(&[(Point::new(900.0, 273.0), 0.5)], 1.0).is_none());
        assert!(r.snap(&[], 1.0).is_none());
    }

    #[test]
    fn a_turned_ruler_lays_strokes_along_its_turned_edge() {
        // Turned 90° clockwise: the "top" edge is now the right-hand side, x = 400 + 27.
        let r = Ruler::new(Point::new(400.0, 300.0), 90.0);
        let out = r.snap(&[(Point::new(430.0, 200.0), 0.5), (Point::new(440.0, 260.0), 0.5)], 0.0).unwrap();
        assert!(out.iter().all(|(p, _)| (p.x - 427.0).abs() < 1e-3), "{out:?}");
        assert!(r.contains(Point::new(400.0, 700.0)) && !r.contains(Point::new(800.0, 300.0)));
        assert_eq!(Ruler::new(Point::ZERO, 315.0).shown_angle(), 45, "rising to the right");
        assert_eq!(Ruler::new(Point::ZERO, 90.0).shown_angle(), 90);
        assert_eq!(Ruler::new(Point::ZERO, 0.0).shown_angle(), 0);
        assert_eq!(Ruler::new(Point::ZERO, 180.0).shown_angle(), 0);
    }

    #[test]
    fn hostile_values_are_made_safe() {
        let r = Ruler::new(Point::new(f32::NAN, f32::INFINITY), f32::NAN);
        assert!(r.center.x.is_finite() && r.center.y.is_finite() && r.angle == 0.0);
        assert_eq!(Ruler::new(Point::ZERO, -90.0).angle, 270.0);
        assert!(Ruler::new(Point::ZERO, 1e30).angle.is_finite());
        let out = r.snap(&[(r.page_point(0.0, -27.0), 0.5)], f32::NAN).unwrap();
        assert!(out[0].0.x.is_finite() && out[0].0.y.is_finite());
    }

    proptest! {
        #[test]
        fn local_and_page_points_round_trip(x in -1e4f32..1e4, y in -1e4f32..1e4, a in -720f32..720.0) {
            let r = Ruler::new(Point::new(123.0, -45.0), a);
            let (u, v) = r.local(Point::new(x, y));
            let p = r.page_point(u, v);
            prop_assert!((p.x - x).abs() < 0.05 && (p.y - y).abs() < 0.05);
        }
    }
}
