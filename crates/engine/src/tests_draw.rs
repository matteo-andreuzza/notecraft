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
fn a_draw_state_read_from_outside_is_repaired() {
    // What the desktop settings file gives back: keys it lacks take their defaults, and
    // hostile values are clamped instead of refused.
    let partial: crate::DrawState = serde_json::from_value(json!({"pen": 2, "eraser": "Small"})).unwrap();
    assert_eq!((partial.pen, partial.eraser, partial.pens.len()), (2, EraserKind::Small, 5));
    let mut d: crate::DrawState = serde_json::from_value(json!({"tool": "Pen", "pen": 40, "pens": []})).unwrap();
    d.normalize();
    assert_eq!((d.tool, d.pen, d.pens.len()), (DrawTool::Type, 0, 5), "no pens: the default gallery");
    let mut many = crate::DrawState { pens: vec![d.pens[0]; 100], ..Default::default() };
    many.pens[0].width = f32::NAN;
    many.pens[1].width = 1e9;
    many.normalize();
    assert_eq!(many.pens.len(), crate::draw_cmds::MAX_PENS);
    assert!(many.pens.iter().all(|p| (notecraft_doc::ink::MIN_INK_WIDTH..=notecraft_doc::ink::MAX_INK_WIDTH).contains(&p.width)));
}

#[test]
fn the_ruler_lays_strokes_along_its_edge() {
    // parity: NC.Draw.RulerPlace
    let mut s = sample();
    blank_page(&mut s);
    let undo = s.undo_labels().len();
    assert!(s.run("draw.rulerPlace", json!({"dx": 5})).is_err(), "no ruler to move yet");
    run(&mut s, "draw.ruler", json!({"x": 400, "y": 300}));
    assert!(s.draw.ruler.is_some(), "R.Draw.Tools.Ruler");
    run(&mut s, "draw.rulerPlace", json!({"turn": -45}));
    assert_eq!(s.draw.ruler.unwrap().angle, 315.0, "turned counterclockwise");
    let r = run(&mut s, "draw.rulerPlace", json!({"angle": 0, "dx": 10, "dy": -10}));
    assert_eq!((r["x"].as_f64(), r["y"].as_f64(), r["shown"].as_u64()), (Some(410.0), Some(290.0), Some(0)));
    assert_eq!(s.undo_labels().len(), undo, "showing, moving and turning the ruler never adds an undo step");
    // The top edge is at y = 290 − 27: a wobbly stroke started there runs along it, its side
    // touching the edge (half of the 2 pt width above it).
    run(&mut s, "draw.stroke", json!({"points": [[300, 260], [350, 250], [420, 266]], "width": 2.0}));
    let k = &strokes(&s)[0];
    assert!(k.points.iter().all(|p| (k.pos.y + p.y - 262.0).abs() < 0.01), "NC.Draw.Stroke along the ruler: {k:?}");
    assert_eq!(k.points.len(), 2, "a straight line keeps its two ends");
    // Away from the edges, or with "free", a stroke is drawn as it is.
    run(&mut s, "draw.stroke", json!({"points": [[300, 100], [350, 120]]}));
    run(&mut s, "draw.stroke", json!({"points": [[300, 260], [350, 250]], "free": true}));
    assert!(strokes(&s)[1..].iter().all(|k| k.bounds().height() > 5.0));
    run(&mut s, "draw.ruler", json!({}));
    assert!(s.draw.ruler.is_none(), "the button hides it again");
}

#[test]
fn snap_to_grid_puts_shapes_on_the_grid_and_draw_with_touch_switches() {
    // parity: R.Draw.Shapes.GridSnapTo
    let mut s = sample();
    blank_page(&mut s);
    let ends = |s: &Session| {
        let k = strokes(s).pop().unwrap();
        let (a, b) = (k.points[0], k.points[k.points.len() - 1]);
        ((k.pos.x + a.x, k.pos.y + a.y), (k.pos.x + b.x, k.pos.y + b.y))
    };
    let undo = s.undo_labels().len();
    assert_eq!(run(&mut s, "draw.snapToGrid", json!({}))["snap"], true);
    assert_eq!(s.undo_labels().len(), undo, "a setting, not an edit");
    // No grid on the page: 1/8 in (9 pt).
    run(&mut s, "draw.shape", json!({"kind": "line", "from": [13, 22], "to": [100, 47]}));
    assert_eq!(ends(&s), ((9.0, 18.0), (99.0, 45.0)));
    // A page with a large grid: its spacing (27 pt).
    run(&mut s, "view.ruleLines", json!({"kind": "gridLarge"}));
    run(&mut s, "draw.shape", json!({"kind": "line", "from": [13, 22], "to": [100, 47]}));
    assert_eq!(ends(&s), ((0.0, 27.0), (108.0, 54.0)));
    // Off: where the pointer was; freehand strokes never snap.
    run(&mut s, "draw.snapToGrid", json!({"on": false}));
    run(&mut s, "draw.shape", json!({"kind": "line", "from": [13, 22], "to": [100, 47]}));
    assert_eq!(ends(&s), ((13.0, 22.0), (100.0, 47.0)));
    run(&mut s, "draw.snapToGrid", json!({}));
    run(&mut s, "draw.stroke", json!({"points": [[13, 22], [100, 47]]}));
    assert_eq!(ends(&s), ((13.0, 22.0), (100.0, 47.0)));
    // Draw with Touch (R.Draw.Tools.FingerPaintingMode) is on until switched off.
    assert!(s.draw.with_touch);
    assert_eq!(run(&mut s, "draw.withTouch", json!({}))["touch"], false);
    assert_eq!(run(&mut s, "draw.withTouch", json!({"on": true}))["touch"], true);
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

#[test]
fn tags_go_on_selected_ink_and_find_tags_lists_them() {
    // Decision D5: a tagged drawing carries the tag on its leftmost stroke.
    let (mut s, a) = with_line();
    let r = run(&mut s, "draw.stroke", json!({"points": [[20, 220], [40, 260]]}));
    let b = Id::parse(r["ink"].as_str().unwrap()).unwrap();
    let tags = |s: &Session, id: Id| s.page().unwrap().ink(id).unwrap().tags.iter().map(|t| (t.key.clone(), t.checked)).collect::<Vec<_>>();
    let enabled = |s: &Session, id: &str| (crate::commands::find(id).unwrap().enabled)(s);
    run(&mut s, "object.select", json!({"ids": [a.to_string(), b.to_string()]}));
    assert_eq!(enabled(&s, "tag.removeAll"), Some("The ink has no tag"));
    let undo = s.undo_labels().len();
    run(&mut s, "tag.toggle", json!({"key": "important"}));
    assert_eq!(s.undo_labels().len(), undo + 1, "one undo step");
    assert_eq!(tags(&s, b), vec![("important".to_string(), false)], "CM.InkDrawing.TagsGallery");
    assert!(tags(&s, a).is_empty(), "one tag per drawing, on its leftmost stroke");
    // Ctrl+1 cycles a to-do on ink as on text; a click on its checkbox toggles it.
    run(&mut s, "tag.todo", json!({}));
    assert_eq!(tags(&s, b)[1], ("todo".to_string(), false));
    run(&mut s, "tag.todo", json!({}));
    assert_eq!(tags(&s, b)[1], ("todo".to_string(), true));
    assert_eq!(run(&mut s, "tag.click", json!({"block": b.to_string()}))["checked"], false);
    run(&mut s, "tag.checkAll", json!({}));
    assert!(tags(&s, b)[1].1, "CM.InkDrawing.TagsMarkAll");
    // Find Tags lists the stroke once per tag, marked as ink.
    let found = run(&mut s, "tag.findTags", json!({"scope": "page"}));
    assert_eq!(found["count"], 2);
    let item = &found["groups"][0]["items"][0];
    assert_eq!((item["ink"].as_bool(), item["block"].as_str()), (Some(true), Some(b.to_string().as_str())));
    let summary = run(&mut s, "tag.summaryPage", json!({"scope": "section"}));
    assert_eq!(summary["count"], 2, "the summary page lists the ink's tags too");
    run(&mut s, "nav.goto", json!({"page": found["groups"][0]["items"][0]["page"]}));
    run(&mut s, "object.select", json!({"ids": [a.to_string(), b.to_string()]}));
    // A tag the drawing has toggles off on every stroke; Remove Tag clears them all.
    run(&mut s, "tag.toggle", json!({"key": "important"}));
    assert_eq!(tags(&s, b).len(), 1);
    run(&mut s, "tag.removeAll", json!({}));
    assert!(tags(&s, b).is_empty(), "CM.InkDrawing.TagsRemoveAll");
    assert!(tags(&s, b).is_empty(), "CM.InkDrawing.TagsRemoveAll2");
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(tags(&s, b).len(), 1, "undo brings the tags back");
    // Hostile clicks: an unknown block, a stroke without a checkbox.
    assert!(s.run("tag.click", json!({"block": a.to_string()})).is_err());
    assert!(s.run("tag.click", json!({"block": "nope"})).is_err());
}

#[test]
fn a_selection_without_text_or_ink_cant_be_tagged() {
    let (mut s, a) = with_line();
    let img = notecraft_codecs::Rgba8 { width: 2, height: 2, pixels: vec![255; 16] };
    let png = notecraft_codecs::encode_png(&img, &notecraft_codecs::Limits::PICTURE).unwrap();
    let r = run(&mut s, "insert.picture", json!({"data": crate::b64::encode(&png), "x": 300.0, "y": 300.0}));
    run(&mut s, "object.select", json!({"ids": [r["picture"].as_str().unwrap()]}));
    let enabled = |s: &Session, id: &str| (crate::commands::find(id).unwrap().enabled)(s);
    assert_eq!(enabled(&s, "tag.toggle"), Some("Select text or ink to tag"));
    assert_eq!(enabled(&s, "tag.todo"), Some("Select text or ink to tag"));
    run(&mut s, "object.select", json!({"ids": [a.to_string()], "add": true}));
    assert_eq!(enabled(&s, "tag.toggle"), None, "ink in the selection is enough");
}

#[test]
fn ink_to_shape_turns_a_drawn_box_into_a_rectangle_in_the_same_pen() {
    // parity: R.Draw.Convert.InkToShape
    let mut s = sample();
    blank_page(&mut s);
    // A wobbly box, started halfway along its top side.
    let mut pts = Vec::new();
    let corners = [(100.0f32, 100.0f32), (300.0, 100.0), (300.0, 200.0), (100.0, 200.0)];
    for k in 0..=4usize {
        let (a, b) = (corners[k % 4], corners[(k + 1) % 4]);
        let (from, to) = if k == 0 {
            (10, 20)
        } else if k == 4 {
            (0, 10)
        } else {
            (0, 20)
        };
        for i in from..to {
            let t = i as f32 / 20.0;
            let j = pts.len() as f32;
            pts.push(json!([a.0 + (b.0 - a.0) * t + (j * 0.7).sin(), a.1 + (b.1 - a.1) * t + (j * 1.3).cos()]));
        }
    }
    let r = run(&mut s, "draw.stroke", json!({"points": pts, "color": "#E81123"}));
    assert!(r.get("shape").is_none(), "off: the stroke stays as drawn");
    let undo = s.undo_labels().len();
    assert_eq!(run(&mut s, "draw.inkToShape", json!({}))["inkToShape"], true);
    assert_eq!(s.undo_labels().len(), undo, "the toggle is not an undo step");
    let r = run(&mut s, "draw.stroke", json!({"points": pts, "color": "#E81123"}));
    assert_eq!(r["shape"], "rectangle");
    let ink = strokes(&s).pop().unwrap();
    assert_eq!(ink.points.len(), 5, "four corners, closed");
    assert_eq!(ink.color, notecraft_doc::color::Rgba::rgb(0xE8, 0x11, 0x23), "the pen's colour");
    // A free stroke and scribbles stay as drawn.
    let r = run(&mut s, "draw.stroke", json!({"points": pts, "free": true}));
    assert!(r.get("shape").is_none());
    let r = run(&mut s, "draw.stroke", json!({"points": [[10, 10], [14, 30], [18, 12], [22, 28], [26, 9]]}));
    assert!(r.get("shape").is_none(), "writing is not a shape");
    assert_eq!(run(&mut s, "draw.inkToShape", json!({"on": false}))["inkToShape"], false);
}
