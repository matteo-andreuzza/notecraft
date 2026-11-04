use notecraft_doc::color::Rgba;
use notecraft_doc::{Id, InkTool};
use proptest::prelude::*;

use super::*;

fn line(y: f32) -> Ink {
    let pts: Vec<(Point, f32)> = (0..=10).map(|i| (Point::new(10.0 + i as f32 * 10.0, y), 0.5)).collect();
    Ink::new(Id(1), &pts, Rgba::BLACK, 2.0, InkTool::Pen)
}

#[test]
fn samples_are_cleaned_and_simplified() {
    let mut s: Vec<(Point, f32)> = (0..=1000).map(|i| (Point::new(i as f32 * 0.1, 5.0), 0.5)).collect();
    s.push((Point::new(f32::NAN, 1.0), 0.5));
    s.push((Point::new(100.0, 50.0), 7.0));
    let c = clean_samples(&s);
    assert_eq!(c.len(), 3, "a straight run keeps its ends, plus the corner: {c:?}");
    assert_eq!(c[2], (Point::new(100.0, 50.0), 1.0));
    assert!(clean_samples(&[]).is_empty());
}

#[test]
fn hits_and_touches() {
    let k = line(50.0);
    assert!(hit(&k, Point::new(55.0, 51.5), 1.0));
    assert!(!hit(&k, Point::new(55.0, 60.0), 1.0));
    assert!(swept(&k, &[Point::new(50.0, 0.0), Point::new(50.0, 100.0)], 2.0), "a fast vertical drag still crosses the line");
    assert!(!swept(&k, &[Point::new(500.0, 0.0), Point::new(500.0, 100.0)], 2.0));
}

#[test]
fn partial_erase_cuts_the_stroke() {
    let k = line(50.0);
    let pieces = erase(&k, &[Point::new(60.0, 40.0), Point::new(60.0, 60.0)], 3.0).unwrap();
    assert_eq!(pieces.len(), 2);
    let left_end = pieces[0].last().unwrap().0;
    let right_start = pieces[1][0].0;
    assert!((left_end.x - 56.0).abs() < 0.01 && (right_start.x - 64.0).abs() < 0.01, "{left_end:?} {right_start:?}");
    assert!(erase(&k, &[Point::new(60.0, 80.0)], 3.0).is_none());
    assert_eq!(erase(&k, &[Point::new(0.0, 50.0), Point::new(200.0, 50.0)], 3.0), Some(vec![]));
}

#[test]
fn lasso_needs_most_of_the_stroke() {
    let k = line(50.0);
    let all = [Point::new(0.0, 0.0), Point::new(200.0, 0.0), Point::new(200.0, 100.0), Point::new(0.0, 100.0)];
    assert!(lasso_picks(&k, &all), "BEH.Ink.Lasso");
    let half = [Point::new(0.0, 0.0), Point::new(60.0, 0.0), Point::new(60.0, 100.0), Point::new(0.0, 100.0)];
    assert!(!lasso_picks(&k, &half));
}

#[test]
fn turns_and_flips_move_points() {
    let mut k = line(50.0);
    let c = k.bounds().center();
    rotate(&mut k, c, 90.0);
    let b = k.bounds();
    assert!(b.height() > b.width(), "a horizontal line turned 90° stands up: {b:?}");
    let mut f = Ink::new(Id(2), &[(Point::new(0.0, 0.0), 0.5), (Point::new(10.0, 10.0), 0.5)], Rgba::BLACK, 1.0, InkTool::Pen);
    flip(&mut f, Point::new(5.0, 5.0), true);
    let pts: Vec<Point> = f.page_points().collect();
    assert_eq!(pts, vec![Point::new(10.0, 0.0), Point::new(0.0, 10.0)]);
    scale(&mut f, Point::ZERO, 2.0, f32::NAN);
    assert_eq!(f.page_points().last(), Some(Point::new(0.0, 10.0)));
    assert_eq!(f.page_points().next(), Some(Point::new(20.0, 0.0)));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn hostile_gestures_never_panic(
        pts in prop::collection::vec((prop::num::f32::ANY, prop::num::f32::ANY, prop::num::f32::ANY), 0..30),
        path in prop::collection::vec((prop::num::f32::ANY, prop::num::f32::ANY), 0..6),
        r in prop::num::f32::ANY,
    ) {
        let samples: Vec<(Point, f32)> = pts.iter().map(|(x, y, p)| (Point::new(*x, *y), *p)).collect();
        let cleaned = clean_samples(&samples);
        let mut k = Ink::new(Id(3), &cleaned, Rgba::BLACK, r, InkTool::Pen);
        let path: Vec<Point> = path.iter().map(|(x, y)| Point::new(*x, *y).sanitized()).collect();
        let r = if r.is_finite() { r.clamp(0.0, 50.0) } else { 3.0 };
        if let Some(pieces) = erase(&k, &path, r) {
            for p in pieces {
                let piece = Ink::new(Id(4), &p, Rgba::BLACK, 1.0, InkTool::Pen);
                prop_assert!(piece.points.iter().all(|q| q.x.is_finite() && q.y.is_finite()));
            }
        }
        let _ = swept(&k, &path, r);
        let _ = lasso_picks(&k, &path);
        rotate(&mut k, Point::new(1.0, 1.0), 45.0);
        flip(&mut k, Point::ZERO, false);
        scale(&mut k, Point::ZERO, r, r);
        prop_assert!(k.points.len() <= MAX_INK_POINTS);
    }
}

#[test]
fn shapes_are_strokes_inside_their_box() {
    let (a, b) = (Point::new(10.0, 20.0), Point::new(110.0, 70.0));
    for kind in ShapeKind::ALL {
        let pts = shape_points(kind, a, b);
        assert!(pts.len() >= 2, "{kind:?}");
        assert_eq!(ShapeKind::parse(kind.name()), Some(kind));
        if !kind.is_line() {
            assert_eq!(pts.first(), pts.last(), "{kind:?} is closed");
            assert!(pts.iter().all(|p| p.x >= 9.99 && p.x <= 110.01 && p.y >= 19.99 && p.y <= 70.01), "{kind:?} stays in its box");
        }
    }
    assert_eq!(ShapeKind::parse("double-arrow"), Some(ShapeKind::DoubleArrow));
    assert_eq!(ShapeKind::parse("blob"), None);
    let arrow = shape_points(ShapeKind::Arrow, a, b);
    assert_eq!(arrow[1], b, "the shaft ends at the tip");
    assert!(shape_points(ShapeKind::Oval, Point::new(f32::NAN, 0.0), Point::new(f32::INFINITY, 1.0)).iter().all(|p| p.x.is_finite()));
}
