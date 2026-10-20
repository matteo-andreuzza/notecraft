//! Ink and the Draw tab (decision D4): tools, pens, strokes, erasers, lasso, object commands on
//! ink, undo, clipboard and hostile input.

use notecraft_doc::{Id, InkTool, PageObject};
use serde_json::json;

use crate::selection::PageSel;
use crate::tests::{blank_page, run, sample};
use crate::{DrawTool, EraserKind, Session};

fn strokes(s: &Session) -> Vec<notecraft_doc::Ink> {
    s.page().unwrap().objects.iter().filter_map(|o| o.as_ink().cloned()).collect()
}

/// A blank page with one horizontal stroke from (50, 200) to (250, 200); returns its id.
fn with_line() -> (Session, Id) {
    let mut s = sample();
    blank_page(&mut s);
    let pts: Vec<_> = (0..=20).map(|i| json!([50.0 + i as f32 * 10.0, 200.0, 0.5])).collect();
    let r = run(&mut s, "draw.stroke", json!({ "points": pts }));
    (s, Id::parse(r["ink"].as_str().unwrap()).unwrap())
}

#[test]
fn tools_and_pens_are_session_state_without_undo() {
    let mut s = sample();
    assert_eq!(s.draw.tool, DrawTool::Type);
    let undo = s.undo_labels().len();
    run(&mut s, "draw.lasso", json!({}));
    assert_eq!(s.draw.tool, DrawTool::Lasso, "R.Draw.Tools.LassoSelect");
    run(&mut s, "draw.pan", json!({}));
    assert_eq!(s.draw.tool, DrawTool::Pan, "R.Draw.Tools.PanningHand");
    run(&mut s, "draw.eraserSmall", json!({}));
    assert_eq!((s.draw.tool, s.draw.eraser), (DrawTool::Eraser, EraserKind::Small), "R.Draw.Tools.SmallEraser");
    run(&mut s, "draw.eraserMedium", json!({}));
    run(&mut s, "draw.eraserLarge", json!({}));
    assert_eq!(s.draw.eraser, EraserKind::Large);
    run(&mut s, "draw.type", json!({}));
    run(&mut s, "draw.eraser", json!({}));
    assert_eq!((s.draw.tool, s.draw.eraser), (DrawTool::Eraser, EraserKind::Large), "Eraser (last used)");
    run(&mut s, "draw.eraserStroke", json!({}));
    run(&mut s, "draw.pen", json!({"index": 3}));
    assert_eq!((s.draw.tool, s.draw.current_pen().tool), (DrawTool::Pen, InkTool::Highlighter), "R.Draw.Tools.InkStylesGallery");
    assert!(s.run("draw.pen", json!({"index": 99})).is_err());
    let n = s.draw.pens.len();
    run(&mut s, "draw.addPen", json!({"tool": "pen", "color": "#00ff00", "width": 3.0}));
    assert_eq!((s.draw.pens.len(), s.draw.pen), (n + 1, n), "R.Draw.Tools.AddPen");
    run(&mut s, "draw.colorThickness", json!({"color": "#123456", "width": 500.0}));
    let pen = s.draw.current_pen();
    assert_eq!(
        (pen.color.to_hex(), pen.width),
        ("#123456".to_string(), notecraft_doc::ink::MAX_INK_WIDTH),
        "R.Draw.Tools.MoreColorAndThicknessOptions"
    );
    assert!(s.run("draw.colorThickness", json!({"color": "nope"})).is_err());
    assert_eq!(s.undo_labels().len(), undo, "choosing tools and pens never adds an undo step");
    for _ in 0..crate::draw_cmds::MAX_PENS {
        let _ = s.run("draw.addPen", json!({}));
    }
    assert_eq!(s.draw.pens.len(), crate::draw_cmds::MAX_PENS);
}

#[test]
fn a_stroke_is_one_object_and_one_undo_step() {
    // parity: NC.Draw.Stroke
    let (mut s, id) = with_line();
    let k = &strokes(&s)[0];
    assert_eq!(k.id, id);
    assert_eq!(k.points.len(), 2, "a straight run is simplified to its ends");
    assert_eq!((k.pos.x, k.pos.y), (50.0, 200.0));
    assert_eq!(k.tool, InkTool::Pen);
    run(&mut s, "draw.pen", json!({"index": 3}));
    run(&mut s, "draw.stroke", json!({"points": [[10, 10], [20, 30]]}));
    assert_eq!(strokes(&s)[1].tool, InkTool::Highlighter, "the selected pen draws");
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(strokes(&s).len(), 1, "undo removes one stroke");
    assert!(s.run("draw.stroke", json!({"points": []})).is_err());
    assert!(s.run("draw.stroke", json!({"points": [[1, "x"]]})).is_err());
    assert!(s.run("draw.stroke", json!({"points": [[f64::MAX, 1.0]]})).is_ok(), "huge numbers are clamped, not refused");
}

#[test]
fn pressure_shapes_the_width() {
    // BEH.Ink.PressureTilt: per-point pressure is stored and widens pen strokes.
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "draw.stroke", json!({"points": [[0, 0, 0.1], [10, 0, 1.0], [10, 10, 0.5]]}));
    let k = &strokes(&s)[0];
    assert!(k.width_at(k.points[1].pressure) > k.width_at(k.points[0].pressure), "BEH.Ink.PressureTilt");
    assert_eq!(k.width_at(k.points[2].pressure), k.width, "0.5 (a mouse) draws the nominal width");
}

#[test]
fn erasers_cut_or_remove_and_coalesce_by_gesture() {
    // parity: NC.Draw.Erase
    let (mut s, id) = with_line();
    let undo = s.undo_labels().len();
    // A miss changes nothing and records nothing.
    let r = run(&mut s, "draw.erase", json!({"points": [[100, 400], [120, 400]], "kind": "small"}));
    assert_eq!(r["erased"], 0);
    assert_eq!(s.undo_labels().len(), undo);
    // The small eraser across the middle cuts the line in two.
    run(&mut s, "draw.erase", json!({"points": [[150, 190], [150, 210]], "kind": "small", "gesture": 1}));
    let k = strokes(&s);
    assert_eq!(k.len(), 2, "R.Draw.Tools.SmallEraser cuts the stroke");
    assert_eq!(k[0].id, id, "the first piece keeps the id");
    assert!(k[0].bounds().max.x < 150.0 && k[1].bounds().min.x > 150.0);
    // A second call in the same gesture joins the same undo step.
    run(&mut s, "draw.erase", json!({"points": [[200, 190], [200, 210]], "kind": "small", "gesture": 1}));
    assert_eq!(strokes(&s).len(), 3);
    assert_eq!(s.undo_labels().len(), undo + 1, "one drag is one undo step");
    // The stroke eraser removes whole strokes it touches.
    run(&mut s, "draw.erase", json!({"points": [[60, 200]], "kind": "stroke", "gesture": 2}));
    assert_eq!(strokes(&s).len(), 2, "R.Draw.Tools.StrokeEraserOneNote removes the stroke");
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(strokes(&s).len(), 1, "undo restores the whole line");
}

#[test]
fn lasso_selects_ink_and_objects_then_object_commands_apply() {
    // parity: BEH.Ink.Lasso
    let (mut s, id) = with_line();
    run(&mut s, "outline.new", json!({"x": 60.0, "y": 300.0}));
    run(&mut s, "text.insert", json!({"text": "note"}));
    let loop_all = json!({"points": [[0, 150], [700, 150], [700, 400], [0, 400]]});
    let r = run(&mut s, "draw.lassoSelect", loop_all);
    assert_eq!(r["ids"].as_array().unwrap().len(), 2, "the stroke and the container: {r}");
    let r = run(&mut s, "draw.lassoSelect", json!({"points": [[0, 150], [700, 150], [700, 250], [0, 250]]}));
    assert_eq!(r["ids"], json!([id.to_string()]));
    assert_eq!(s.sel.on_page, PageSel::Objects { ids: vec![id] });
    // Move, rotate, flip, copy, paste, delete — the object commands work on ink.
    run(&mut s, "object.move", json!({"dx": 10.0, "dy": 5.0}));
    assert_eq!(strokes(&s)[0].pos.y, 205.0);
    run(&mut s, "object.rotateRight90", json!({}));
    let b = strokes(&s)[0].bounds();
    assert!(b.height() > b.width(), "CM.InkDrawing.ObjectRotateRight90: {b:?}");
    run(&mut s, "object.rotateLeft45", json!({}));
    let b45 = strokes(&s)[0].bounds();
    assert!((b45.width() - b45.height()).abs() < 1.0, "R.Draw.Edit.RotateLeft45OneNote: a diagonal: {b45:?}");
    run(&mut s, "object.flipHorizontal", json!({}));
    run(&mut s, "object.flipVertical", json!({}));
    run(&mut s, "edit.copy", json!({}));
    run(&mut s, "edit.paste", json!({}));
    assert_eq!(strokes(&s).len(), 2, "CM.InkDrawing.CopyOneNote: the copy has its own id");
    assert_ne!(strokes(&s)[0].id, strokes(&s)[1].id);
    run(&mut s, "draw.lassoSelect", json!({"points": [[0, 0], [2000, 0], [2000, 2000], [0, 2000]]}));
    let n = s.page().unwrap().objects.len();
    run(&mut s, "edit.cut", json!({}));
    assert!(strokes(&s).is_empty(), "CM.LassoSelection.Cut");
    run(&mut s, "edit.paste", json!({}));
    assert_eq!(s.page().unwrap().objects.len(), n, "cut ink pastes back");
    run(&mut s, "draw.lassoSelect", json!({"points": [[0, 0], [2000, 0], [2000, 2000], [0, 2000]]}));
    run(&mut s, "edit.delete", json!({}));
    assert!(strokes(&s).is_empty(), "CM.LassoSelection.Delete");
    // Nothing inside: the selection clears.
    run(&mut s, "draw.lassoSelect", json!({"points": [[0, 0], [1, 0], [1, 1]]}));
    assert_eq!(s.sel.on_page, PageSel::None);
    assert!(s.run("object.rotateRight45", json!({})).is_err(), "45° turns need ink");
}

#[test]
fn ink_stays_on_the_page_when_turned_at_the_edge() {
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "draw.stroke", json!({"points": [[0, 0], [100, 0]]}));
    run(&mut s, "draw.lassoSelect", json!({"points": [[-10, -10], [200, -10], [200, 20], [-10, 20]]}));
    run(&mut s, "object.rotateRight45", json!({}));
    let b = strokes(&s)[0].bounds();
    assert!(b.min.x >= -0.01 && b.min.y >= -0.01, "{b:?}");
    let o = s.page().unwrap().objects.iter().find(|o| matches!(o, PageObject::Ink(_))).unwrap();
    assert!(o.pos().x >= 0.0);
}

#[test]
fn ink_resizes_from_one_rectangle_to_another() {
    let (mut s, id) = with_line();
    run(&mut s, "draw.lassoSelect", json!({"points": [[0, 150], [400, 150], [400, 250], [0, 250]]}));
    let b = strokes(&s)[0].bounds();
    let (x, y, w, h) = (b.min.x, b.min.y, b.width(), b.height());
    let undo = s.undo_labels().len();
    // Two steps of one handle drag: twice as wide, then three times, from the left edge.
    run(&mut s, "object.resize", json!({"from": [x, y, w, h], "to": [x, y, w * 2.0, h]}));
    run(&mut s, "object.resize", json!({"from": [x, y, w * 2.0, h], "to": [x, y, w * 3.0, h]}));
    let k = s.page().unwrap().ink(id).unwrap().clone();
    let pts: Vec<_> = k.page_points().collect();
    assert!((pts[1].x - pts[0].x - 600.0).abs() < 0.5, "CM.InkDrawing.ObjectResize: {pts:?}");
    assert_eq!(s.undo_labels().len(), undo + 1, "one drag, one undo step");
    // dw from the menu-less API; no params only explains.
    run(&mut s, "object.resize", json!({"dw": 10.0}));
    run(&mut s, "object.resize", json!({}));
    assert!(s.run("object.resize", json!({"from": [0, 0, 0, 0], "to": [0, 0, 1e30, -5]})).is_ok(), "hostile rectangles are ignored, not fatal");
}

#[test]
fn insert_space_pushes_and_pulls_what_follows() {
    // parity: BEH.Canvas.InsertSpace
    let (mut s, id) = with_line();
    run(&mut s, "outline.new", json!({"x": 60.0, "y": 100.0}));
    run(&mut s, "text.insert", json!({"text": "above"}));
    run(&mut s, "insert.space", json!({}));
    assert_eq!(s.draw.tool, DrawTool::Space, "R.Insert.Insert.PenInsertSpace");
    let undo = s.undo_labels().len();
    let r = run(&mut s, "canvas.insertSpace", json!({"y": 150.0, "dy": 80.0}));
    assert_eq!(r["moved"], 1, "only what starts below the line");
    assert_eq!(strokes(&s)[0].pos.y, 280.0);
    assert_eq!(s.undo_labels().len(), undo + 1);
    // Removing space stops at the line.
    let r = run(&mut s, "canvas.insertSpace", json!({"y": 150.0, "dy": -500.0}));
    assert_eq!(r["by"], -130.0);
    assert_eq!(strokes(&s)[0].pos.y, 150.0);
    // Sideways, and nothing to move.
    run(&mut s, "canvas.insertSpace", json!({"x": 40.0, "dx": 30.0}));
    assert_eq!(s.page().unwrap().ink(id).unwrap().pos.x, 80.0);
    let n = s.undo_labels().len();
    assert_eq!(run(&mut s, "canvas.insertSpace", json!({"y": 5000.0, "dy": 10.0}))["moved"], 0);
    assert_eq!(s.undo_labels().len(), n, "nothing moved, no undo step");
    assert!(s.run("canvas.insertSpace", json!({"y": 1.0})).is_err());
    run(&mut s, "insert.space", json!({}));
    assert_eq!(s.draw.tool, DrawTool::Type, "the toggle turns it off");
}

#[test]
fn shapes_draw_as_ink_with_the_selected_pen() {
    // parity: NC.Draw.Shape
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "draw.shapes", json!({"kind": "rectangle"}));
    assert_eq!((s.draw.tool, s.draw.shape), (DrawTool::Shape, notecraft_ink::ShapeKind::Rectangle), "R.Draw.Shapes.InkShapeGallery");
    run(&mut s, "draw.pen", json!({"index": 1}));
    run(&mut s, "draw.shapes", json!({}));
    let undo = s.undo_labels().len();
    run(&mut s, "draw.shape", json!({"from": [100, 100], "to": [40, 60]}));
    let k = &strokes(&s)[0];
    assert_eq!(k.points.len(), 5, "a closed rectangle");
    assert_eq!((k.pos.x, k.pos.y), (40.0, 60.0));
    assert_eq!(k.color, s.draw.pens[1].color);
    assert_eq!(s.undo_labels().len(), undo + 1);
    run(&mut s, "draw.shape", json!({"kind": "double arrow", "from": [0, 0], "to": [100, 0]}));
    assert_eq!(strokes(&s).len(), 2);
    assert!(s.run("draw.shape", json!({"kind": "blob", "from": [0, 0], "to": [1, 1]})).is_err());
    assert!(s.run("draw.shape", json!({"from": [0, 0]})).is_err());
    assert_eq!(run(&mut s, "draw.lockDrawingMode", json!({}))["locked"], true, "R.Draw.Shapes.ShapesDuplicate");
}
