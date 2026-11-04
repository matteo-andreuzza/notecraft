use notecraft_geom::Point;
use proptest::prelude::*;

use super::*;

fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

#[test]
fn distances() {
    assert_eq!(seg_dist(pt(5.0, 3.0), pt(0.0, 0.0), pt(10.0, 0.0)), 3.0);
    assert_eq!(seg_dist(pt(-4.0, 3.0), pt(0.0, 0.0), pt(10.0, 0.0)), 5.0);
    assert_eq!(seg_dist(pt(3.0, 4.0), pt(0.0, 0.0), pt(0.0, 0.0)), 5.0);
    assert_eq!(polyline_dist(&[], pt(0.0, 0.0)), f32::INFINITY);
    assert_eq!(polyline_dist(&[pt(1.0, 1.0)], pt(4.0, 5.0)), 5.0);
    assert_eq!(polyline_dist(&[pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0)], pt(12.0, 5.0)), 2.0);
}

#[test]
fn polygon_and_bounds() {
    let sq = [pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0), pt(0.0, 10.0)];
    assert!(in_polygon(&sq, pt(5.0, 5.0)));
    assert!(!in_polygon(&sq, pt(15.0, 5.0)));
    assert!(!in_polygon(&sq[..2], pt(5.0, 0.0)));
    let b = bounds(&[pt(3.0, -1.0), pt(-2.0, 4.0)]).unwrap();
    assert_eq!((b.min, b.max), (pt(-2.0, -1.0), pt(3.0, 4.0)));
    assert!(bounds(&[]).is_none());
}

#[test]
fn simplify_keeps_corners_and_drops_collinear_points() {
    let line: Vec<Point> = (0..=100).map(|i| pt(i as f32, 0.0)).collect();
    assert_eq!(simplify(&line, 0.1), vec![0, 100]);
    let l = [pt(0.0, 0.0), pt(5.0, 0.01), pt(10.0, 0.0), pt(10.0, 10.0)];
    assert_eq!(simplify(&l, 0.1), vec![0, 2, 3]);
    assert_eq!(simplify(&l[..1], 0.1), vec![0]);
}

#[test]
fn cut_circle_splits_a_line_in_two() {
    let line = [pt(0.0, 0.0), pt(10.0, 0.0), pt(20.0, 0.0)];
    let pieces = cut_circle(&line, pt(10.0, 0.0), 2.0).unwrap();
    assert_eq!(pieces.len(), 2);
    let ends: Vec<(Point, Point)> = pieces.iter().map(|p| (point_at(&line, *p.first().unwrap()), point_at(&line, *p.last().unwrap()))).collect();
    assert_eq!(ends[0], (pt(0.0, 0.0), pt(8.0, 0.0)));
    assert_eq!(ends[1], (pt(12.0, 0.0), pt(20.0, 0.0)));
    // Untouched, wholly erased, and a dot.
    assert!(cut_circle(&line, pt(10.0, 9.0), 2.0).is_none());
    assert_eq!(cut_circle(&line, pt(10.0, 0.0), 50.0), Some(vec![]));
    assert_eq!(cut_circle(&[pt(1.0, 1.0)], pt(1.0, 2.0), 2.0), Some(vec![]));
    // Cutting the start leaves one piece that starts at the cut.
    let p = cut_circle(&line, pt(0.0, 0.0), 3.0).unwrap();
    assert_eq!(p.len(), 1);
    assert_eq!(point_at(&line, p[0][0]), pt(3.0, 0.0));
    assert_eq!(point_at(&line, *p[0].last().unwrap()), pt(20.0, 0.0));
}

#[test]
fn outline_offsets_by_the_half_width() {
    let o = outline(&[pt(0.0, 0.0), pt(10.0, 0.0)], &[1.0, 2.0]);
    assert_eq!(o[0], (pt(0.0, 1.0), pt(0.0, -1.0)));
    assert_eq!(o[1], (pt(10.0, 2.0), pt(10.0, -2.0)));
    assert_eq!(outline(&[pt(1.0, 1.0)], &[]).len(), 1);
}

#[test]
fn rotate_quarter_turn() {
    let p = rotate(pt(10.0, 0.0), pt(0.0, 0.0), 90.0);
    assert!((p.x).abs() < 1e-5 && (p.y - 10.0).abs() < 1e-5, "{p:?}");
}

fn any_pts() -> impl Strategy<Value = Vec<Point>> {
    prop::collection::vec((prop::num::f32::ANY, prop::num::f32::ANY).prop_map(|(x, y)| pt(x, y)), 0..40)
}

proptest! {
    #[test]
    fn hostile_input_never_panics(pts in any_pts(), cx in prop::num::f32::ANY, cy in prop::num::f32::ANY, r in prop::num::f32::ANY) {
        let c = pt(cx, cy);
        let _ = polyline_dist(&pts, c);
        let _ = bounds(&pts);
        let _ = in_polygon(&pts, c);
        let kept = simplify(&pts, r);
        prop_assert!(kept.len() <= pts.len());
        if let Some(pieces) = cut_circle(&pts, c, r) {
            for piece in &pieces {
                prop_assert!(piece.len() >= 2);
                for a in piece {
                    let _ = point_at(&pts, *a);
                }
            }
        }
        prop_assert_eq!(outline(&pts, &[r]).len(), pts.len());
    }

    #[test]
    fn pieces_stay_outside_the_disc(xs in prop::collection::vec(-100.0f32..100.0, 2..30), cx in -100.0f32..100.0, r in 0.5f32..30.0) {
        let pts: Vec<Point> = xs.iter().enumerate().map(|(i, y)| pt(i as f32 * 5.0, *y)).collect();
        let c = pt(cx, 0.0);
        if let Some(pieces) = cut_circle(&pts, c, r) {
            for piece in pieces {
                for a in &piece[1..piece.len() - 1] {
                    prop_assert!(point_at(&pts, *a).distance(c) >= r - 1e-3);
                }
            }
        }
    }
}
