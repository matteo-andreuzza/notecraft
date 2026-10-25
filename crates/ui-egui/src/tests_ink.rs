//! Ink on the canvas (decision D4): drawing with the pen tool through real pointer events,
//! pressure from touch force, the erasers, the lasso and moving its selection, clicking a stroke,
//! the ink context menu and the Draw tab's pen gallery and Color & Thickness dialog.

use egui::{Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use notecraft_engine::DrawTool;
use notecraft_engine::doc::{Ink, InkTool};
use notecraft_engine::selection::PageSel;
use serde_json::json;

use crate::NoteApp;
use crate::tests::{app, harness};
use crate::tests_canvas::{click, drag, press, right_click};

/// A new blank page, the Draw tab showing, `tool` chosen.
fn drawing(tool: &str) -> Harness<'static, NoteApp> {
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    a.session.run(tool, json!({})).unwrap();
    a.ui.tab = "Draw".into();
    let mut h = harness(a);
    h.run();
    h
}

fn strokes(h: &Harness<'static, NoteApp>) -> Vec<Ink> {
    h.state().session.page().unwrap().objects.iter().filter_map(|o| o.as_ink().cloned()).collect()
}

/// Page → screen with the canvas's current origin and zoom (a copy: it doesn't borrow `h`).
fn map(h: &Harness<'static, NoteApp>) -> impl Fn(f32, f32) -> Pos2 + use<> {
    let (o, z) = (h.state().canvas.origin, h.state().canvas.zoom.max(0.1) * crate::canvas::PT);
    move |x, y| pos2(o.x + x * z, o.y + y * z)
}

/// Drag through `pts` (screen), one frame per point.
fn trace(h: &mut Harness<'static, NoteApp>, pts: &[Pos2]) {
    let Some((first, rest)) = pts.split_first() else { return };
    press(h, *first, true);
    for p in rest {
        h.event(egui::Event::PointerMoved(*p));
        h.run();
    }
    press(h, *pts.last().unwrap_or(first), false);
}

#[test]
fn the_pen_draws_a_stroke_through_the_pointer_samples() {
    // parity: NC.Draw.Stroke
    let mut h = drawing("draw.pen");
    let at = map(&h);
    let pts: Vec<Pos2> = (0..12).map(|i| at(100.0 + i as f32 * 12.0, 200.0 + if i % 2 == 0 { 0.0 } else { 15.0 })).collect();
    let undo = h.state().session.undo_labels().len();
    trace(&mut h, &pts);
    let k = strokes(&h);
    assert_eq!(k.len(), 1, "NC.Draw.Stroke: one stroke");
    assert!(k[0].points.len() >= 10, "a zig-zag keeps its corners: {}", k[0].points.len());
    let b = k[0].bounds();
    assert!((b.min.x - 100.0).abs() < 2.0 && (b.max.y - 215.0).abs() < 2.0, "{b:?}");
    assert_eq!(h.state().session.undo_labels().len(), undo + 1, "one undo step per stroke");
    assert_eq!(h.state().canvas.ink.layout.len(), 1);
    // A tap is a dot.
    click(&mut h, at(400.0, 400.0));
    assert_eq!(strokes(&h).len(), 2);
    assert_eq!(strokes(&h)[1].points.len(), 1);
}

#[test]
fn touch_force_becomes_pressure() {
    // BEH.Ink.PressureTilt
    let mut h = drawing("draw.pen");
    let at = map(&h);
    let (a, b) = (at(100.0, 300.0), at(200.0, 300.0));
    let touch = |p, phase| egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(1), phase, pos: p, force: Some(0.9) };
    h.event(egui::Event::PointerMoved(a));
    h.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.event(touch(a, egui::TouchPhase::Start));
    // One frame per sample, as a pen delivers them: a touch held still for many frames would be
    // egui's long press (the touch-screen context menu) instead of a stroke.
    h.run_steps(1);
    for i in 1..=6 {
        let p = a + (b - a) * (i as f32 / 6.0);
        h.event(touch(p, egui::TouchPhase::Move));
        h.event(egui::Event::PointerMoved(p + egui::vec2(0.0, if i % 2 == 0 { 6.0 } else { 0.0 })));
        h.run_steps(1);
    }
    h.event(touch(b, egui::TouchPhase::End));
    press(&mut h, b, false);
    let k = strokes(&h);
    assert_eq!(k.len(), 1, "a touch drag draws");
    assert!(k[0].points.iter().all(|p| (p.pressure - 0.9).abs() < 0.01), "BEH.Ink.PressureTilt: {:?}", k[0].points);
    assert!(k[0].width_at(0.9) > k[0].width, "harder pressure draws wider");
}

#[test]
fn erasers_work_on_the_canvas() {
    // parity: NC.Draw.Erase
    let mut h = drawing("draw.pen");
    let at = map(&h);
    let line: Vec<Pos2> = (0..8).map(|i| at(100.0 + i as f32 * 20.0, 250.0)).collect();
    trace(&mut h, &line);
    trace(&mut h, &[at(100.0, 350.0), at(180.0, 350.0), at(260.0, 350.0)]);
    assert_eq!(strokes(&h).len(), 2);
    // The small eraser across the first line cuts it in two.
    h.state_mut().session.run("draw.eraserSmall", json!({})).unwrap();
    h.run();
    let undo = h.state().session.undo_labels().len();
    drag(&mut h, at(170.0, 230.0), at(170.0, 270.0));
    assert_eq!(strokes(&h).len(), 3, "R.Draw.Tools.SmallEraser");
    assert_eq!(h.state().session.undo_labels().len(), undo + 1, "one drag, one undo step");
    // The stroke eraser removes the second line whole.
    h.state_mut().session.run("draw.eraserStroke", json!({})).unwrap();
    h.run();
    drag(&mut h, at(200.0, 330.0), at(200.0, 370.0));
    assert_eq!(strokes(&h).len(), 2, "R.Draw.Tools.StrokeEraserOneNote");
}

#[test]
fn the_lasso_selects_and_the_selection_moves() {
    // BEH.Ink.Lasso
    let mut h = drawing("draw.pen");
    let at = map(&h);
    trace(&mut h, &[at(150.0, 200.0), at(200.0, 220.0), at(250.0, 200.0)]);
    let id = strokes(&h)[0].id;
    h.state_mut().session.run("draw.lasso", json!({})).unwrap();
    h.run();
    let ring: Vec<Pos2> = [(120.0, 170.0), (290.0, 170.0), (290.0, 250.0), (120.0, 250.0), (120.0, 175.0)].iter().map(|(x, y)| at(*x, *y)).collect();
    trace(&mut h, &ring);
    assert_eq!(h.state().session.sel.on_page, PageSel::Objects { ids: vec![id] }, "BEH.Ink.Lasso");
    assert!(crate::ink::selection_box(h.state()).is_some());
    // Dragging inside the selection box moves the stroke (one undo step).
    let undo = h.state().session.undo_labels().len();
    let x0 = strokes(&h)[0].pos.x;
    drag(&mut h, at(200.0, 215.0), at(260.0, 215.0));
    let x1 = strokes(&h)[0].pos.x;
    assert!((x1 - x0 - 60.0).abs() < 2.0, "moved by the drag: {x0} → {x1}");
    assert_eq!(h.state().session.undo_labels().len(), undo + 1);
}

#[test]
fn type_tool_selects_a_stroke_and_its_menu_opens() {
    let mut h = drawing("draw.pen");
    let at = map(&h);
    trace(&mut h, &[at(150.0, 200.0), at(220.0, 200.0), at(300.0, 200.0)]);
    let id = strokes(&h)[0].id;
    h.state_mut().session.run("draw.type", json!({})).unwrap();
    h.state_mut().ui.tab = "View".into();
    h.run();
    right_click(&mut h, at(220.0, 200.0));
    h.run();
    assert_eq!(h.state().session.sel.on_page, PageSel::Objects { ids: vec![id] });
    assert!(h.query_by_label_contains("Rotate").is_some(), "CM.InkDrawing.RotateInkMenu");
    assert!(h.query_by_label_contains("Order").is_some(), "CM.InkDrawing.OrderObjectsMenu");
}

#[test]
fn clicking_a_stroke_with_the_type_tool_selects_it() {
    let mut h = drawing("draw.pen");
    let at = map(&h);
    trace(&mut h, &[at(150.0, 200.0), at(220.0, 200.0), at(300.0, 200.0)]);
    let id = strokes(&h)[0].id;
    h.state_mut().session.run("draw.type", json!({})).unwrap();
    h.run();
    click(&mut h, at(220.0, 200.5));
    assert_eq!(h.state().session.sel.on_page, PageSel::Objects { ids: vec![id] }, "R.Draw.Tools.SelectAndTypeMode");
    // Away from it, a click is a click on the page again.
    click(&mut h, at(220.0, 400.0));
    assert_ne!(h.state().session.sel.on_page, PageSel::Objects { ids: vec![id] });
}

#[test]
fn the_pen_gallery_and_color_thickness() {
    let mut h = drawing("draw.type");
    assert!(h.query_by_label("Pen 2").is_some(), "R.Draw.Tools.InkStylesGallery");
    h.get_by_label("Pen 2").click();
    h.run();
    assert_eq!((h.state().session.draw.tool, h.state().session.draw.pen), (DrawTool::Pen, 1));
    h.get_by_label("Highlighter 4").click();
    h.run();
    assert_eq!(h.state().session.draw.current_pen().tool, InkTool::Highlighter);
    h.get_by_label("Color & Thickness").click();
    h.run();
    assert!(matches!(h.state().ui.dialog, Some(crate::dialogs::Dialog::ColorThickness { highlighter: true, .. })), "UI.Panes.ColorThickness");
    h.get_by_label("Pen").click();
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().session.draw.current_pen().tool, InkTool::Pen, "the dialog's choice applies to the pen");
}

#[test]
fn the_draw_tab_and_lasso_menus_act_on_selected_ink() {
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    a.session.run("draw.stroke", json!({"points": [[10, 10], [60, 30]]})).unwrap();
    a.session.run("draw.stroke", json!({"points": [[10, 50], [60, 70]]})).unwrap();
    a.session.run("draw.lassoSelect", json!({"points": [[0, 0], [100, 0], [100, 100], [0, 100]]})).unwrap();
    let on = |id: &str| crate::ribbon::enabled(&a, id);
    assert!(on("R.Draw.Edit.ObjectRotateRight90") && on("R.Draw.Edit.RotateRight45OneNote"), "R.Draw.Edit.RotateMenuOneNote");
    assert!(on("R.Draw.Edit.ObjectBringToFront") && on("R.Draw.Edit.ObjectSendToBack"), "R.Draw.Edit.ObjectsArrangeMenu");
    assert!(on("CM.LassoSelection.ObjectFlipVertical") && on("CM.LassoSelection.RotateLeft45OneNote"), "CM.LassoSelection.RotateInkMenu");
}

#[test]
fn a_corner_handle_resizes_selected_ink() {
    let mut h = drawing("draw.pen");
    let at = map(&h);
    trace(&mut h, &[at(150.0, 200.0), at(220.0, 230.0), at(300.0, 200.0)]);
    h.state_mut().session.run("draw.type", json!({})).unwrap();
    click(&mut h, at(220.0, 230.0));
    let b = crate::ink::selection_box(h.state()).expect("the stroke is selected");
    let w0 = strokes(&h)[0].bounds().width();
    let undo = h.state().session.undo_labels().len();
    // Drag the bottom-right corner outwards: the ink grows in proportion, as one undo step.
    drag(&mut h, b.right_bottom(), b.right_bottom() + egui::vec2(150.0, 0.0));
    let w1 = strokes(&h)[0].bounds().width();
    assert!(w1 > w0 * 1.5, "CM.InkDrawing.ObjectResize: {w0} → {w1}");
    assert_eq!(h.state().session.undo_labels().len(), undo + 1);
}

#[test]
fn insert_space_by_dragging_down() {
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    a.session.run("draw.stroke", json!({"points": [[60, 300], [200, 300]]})).unwrap();
    a.session.run("insert.space", json!({})).unwrap();
    a.ui.tab = "Insert".into();
    let mut h = harness(a);
    h.run();
    assert!(h.query_by_label("Insert Space").is_some(), "R.Insert.Insert.PenInsertSpace");
    let at = map(&h);
    drag(&mut h, at(300.0, 250.0), at(300.0, 330.0));
    let y = strokes(&h)[0].pos.y;
    assert!((y - 380.0).abs() < 2.0, "BEH.Canvas.InsertSpace: {y}");
    assert_eq!(h.state().session.draw.tool, DrawTool::Type, "back to typing");
}

#[test]
fn dragging_with_the_shape_tool_draws_the_shape() {
    let mut h = drawing("draw.type");
    h.state_mut().session.run("draw.shapes", json!({"kind": "oval"})).unwrap();
    h.run();
    let at = map(&h);
    drag(&mut h, at(100.0, 150.0), at(220.0, 230.0));
    let k = strokes(&h);
    assert_eq!(k.len(), 1, "NC.Draw.Shape");
    let b = k[0].bounds();
    assert!((b.width() - 120.0).abs() < 4.0 && (b.height() - 80.0).abs() < 4.0, "{b:?}");
    assert_eq!(h.state().session.draw.tool, DrawTool::Type, "without Lock Drawing Mode the tool goes back to Type");
    // The gallery lists the shapes.
    h.state_mut().ui.tab = "Draw".into();
    h.state_mut().session.run("ui.ribbonMode", json!({"mode": "classic"})).unwrap();
    h.run();
    h.get_by_label("Shapes").click();
    h.run();
    h.get_by_label("Diamond").click();
    h.run();
    assert_eq!(h.state().session.draw.shape, notecraft_ink::ShapeKind::Diamond, "R.Draw.Shapes.InkShapeGallery");
}

#[test]
fn the_ink_menu_tags_a_stroke() {
    let mut h = drawing("draw.pen");
    let at = map(&h);
    trace(&mut h, &[at(150.0, 200.0), at(220.0, 200.0), at(300.0, 200.0)]);
    h.state_mut().session.run("draw.type", json!({})).unwrap();
    h.state_mut().ui.tab = "View".into();
    h.run();
    right_click(&mut h, at(220.0, 200.0));
    h.run();
    h.get_by_label_contains("Tags ⏵").click();
    h.run();
    assert!(h.query_by_label("Customize Tags").is_some(), "CM.InkDrawing.TagsCustomize");
    assert_eq!(h.query_all_by_label("Remove Tag").count(), 2, "CM.InkDrawing.TagsRemoveAll in the submenu and CM.InkDrawing.TagsRemoveAll2 below it");
    h.get_by_label_contains("Important").click();
    h.run();
    let tags: Vec<String> = strokes(&h)[0].tags.iter().map(|t| t.key.clone()).collect();
    assert_eq!(tags, vec!["important".to_string()], "CM.InkDrawing.TagsGallery");
}

#[test]
fn a_checkbox_tag_on_ink_toggles_on_click_and_find_tags_selects_the_ink() {
    let mut h = drawing("draw.pen");
    let at = map(&h);
    trace(&mut h, &[at(150.0, 200.0), at(220.0, 200.0), at(300.0, 200.0)]);
    let id = strokes(&h)[0].id;
    let s = &mut h.state_mut().session;
    s.run("draw.type", json!({})).unwrap();
    s.run("object.select", json!({"ids": [id.to_string()]})).unwrap();
    s.run("tag.todo", json!({})).unwrap();
    s.run("tag.findTags", json!({})).unwrap();
    h.run();
    // The icon sits left of the stroke's box, level with its top.
    let (r, z) = (h.state().canvas.ink.layout[0].rect, h.state().canvas.zoom);
    let size = (14.0 * z).clamp(8.0, 40.0);
    click(&mut h, pos2(r.min.x - crate::canvas::TAG_W * crate::canvas::PT * z + size / 2.0, r.min.y + size / 2.0));
    assert!(strokes(&h)[0].tags[0].checked, "BEH.Tags.CheckboxClick on ink");
    // The Tags pane lists the drawing; clicking it selects the stroke.
    h.state_mut().session.run("caret.set", json!({"block": "title", "para": 0, "offset": 0})).ok();
    h.run();
    let item = h.get_by_label("Ink drawing").rect().center();
    click(&mut h, item);
    assert_eq!(h.state().session.sel.on_page, PageSel::Objects { ids: vec![id] });
}

#[test]
fn the_ruler_is_shown_turned_moved_and_drawn_along() {
    let mut h = drawing("draw.pen");
    h.state_mut().session.run("ui.ribbonMode", json!({"mode": "classic"})).unwrap();
    h.run();
    h.get_by_label("Ruler").click();
    h.run();
    let r = h.state().session.draw.ruler.expect("R.Draw.Tools.Ruler");
    let c = h.state().canvas.to_screen(r.center);
    assert!(c.distance(h.state().canvas.viewport.center()) < 2.0, "shown in the middle of the page's view");
    // The wheel over it turns it a degree a notch, and the page doesn't scroll.
    let origin = h.state().canvas.origin;
    h.event(egui::Event::PointerMoved(c));
    h.run();
    for _ in 0..3 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, 1.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        h.run();
    }
    assert_eq!(h.state().session.draw.ruler.unwrap().angle, 357.0, "three notches up turn it 3° counterclockwise");
    assert_eq!(h.state().canvas.origin, origin, "the page didn't scroll");
    h.state_mut().session.run("draw.rulerPlace", json!({"angle": 0})).unwrap();
    h.run();
    // A drag that starts on it (away from its edges) moves it; nothing is drawn.
    let at = map(&h);
    let (x, y) = (r.center.x, r.center.y);
    drag(&mut h, at(x, y), at(x + 40.0, y + 30.0));
    let moved = h.state().session.draw.ruler.unwrap().center;
    assert!((moved.x - x - 40.0).abs() < 1.0 && (moved.y - y - 30.0).abs() < 1.0, "moved by the drag: {moved:?}");
    assert!(strokes(&h).is_empty(), "moving the ruler draws nothing");
    // The pen started just above its top edge draws along it.
    let top = moved.y - notecraft_ink::ruler::WIDTH / 2.0;
    trace(&mut h, &[at(moved.x - 100.0, top - 4.0), at(moved.x - 40.0, top - 9.0), at(moved.x + 60.0, top - 2.0)]);
    let k = strokes(&h);
    assert_eq!(k.len(), 1);
    let half = h.state().session.draw.current_pen().width / 2.0;
    assert!(k[0].points.iter().all(|p| (k[0].pos.y + p.y - (top - half)).abs() < 0.05), "drawn along the edge: {:?}", k[0]);
}

/// A touch of finger `id` (no pressure, as fingers report).
fn finger(id: u64, phase: egui::TouchPhase, pos: Pos2) -> egui::Event {
    egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(id), phase, pos, force: None }
}

#[test]
fn two_fingers_turn_and_move_the_ruler() {
    use egui::TouchPhase::{End, Move, Start};
    let mut h = drawing("draw.pen");
    h.state_mut().session.run("draw.ruler", json!({"x": 400, "y": 300})).unwrap();
    h.run();
    let c = h.state().canvas.to_screen(h.state().session.draw.ruler.unwrap().center);
    let arm = |deg: f32| egui::Vec2::angled(deg.to_radians()) * 60.0;
    // As egui-winit sends them: each finger a touch, the first one also the pointer.
    let fingers = |h: &mut Harness<'static, NoteApp>, phase: egui::TouchPhase, deg: f32, shift: egui::Vec2| {
        let (one, two) = (c - arm(deg) + shift, c + arm(deg) + shift);
        h.event(finger(1, phase, one));
        h.event(finger(2, phase, two));
        match phase {
            Start => press(h, one, true),
            End => press(h, one, false),
            _ => {
                h.event(egui::Event::PointerMoved(one));
                h.run();
            }
        }
    };
    fingers(&mut h, Start, 0.0, egui::Vec2::ZERO);
    // Turning both fingers 10° clockwise about the ruler's middle turns it, in place.
    fingers(&mut h, Move, 5.0, egui::Vec2::ZERO);
    fingers(&mut h, Move, 10.0, egui::Vec2::ZERO);
    let r = h.state().session.draw.ruler.unwrap();
    assert!((r.angle - 10.0).abs() < 0.1, "turned 10°: {r:?}");
    assert!((r.center.x - 400.0).abs() < 0.1 && (r.center.y - 300.0).abs() < 0.1, "about the point between the fingers: {r:?}");
    // Sliding both fingers moves it with them.
    let slide = egui::vec2(30.0, 20.0);
    fingers(&mut h, Move, 10.0, slide);
    fingers(&mut h, End, 10.0, slide);
    let moved = h.state().session.draw.ruler.unwrap().center;
    let want = h.state().canvas.to_page(c + slide);
    assert!((moved.x - want.x).abs() < 0.1 && (moved.y - want.y).abs() < 0.1, "moved with the fingers: {moved:?} vs {want:?}");
    assert!(strokes(&h).is_empty(), "two fingers draw nothing");
}

#[test]
fn with_draw_with_touch_off_a_finger_moves_the_page_instead_of_drawing() {
    // parity: R.Draw.Tools.FingerPaintingMode
    use egui::TouchPhase::{End, Start};
    let mut h = drawing("draw.pen");
    h.state_mut().session.run("ui.ribbonMode", json!({"mode": "classic"})).unwrap();
    h.run();
    h.get_by_label("Draw with Touch").click();
    h.run();
    assert!(!h.state().session.draw.with_touch, "draw.withTouch");
    let at = map(&h);
    let (a, b) = (at(100.0, 300.0), at(220.0, 200.0));
    // egui-winit sends a finger as a touch and as the pointer.
    let finger_drag = |h: &mut Harness<'static, NoteApp>| {
        h.event(finger(1, Start, a));
        trace(h, &[a, pos2(a.x + 40.0, a.y - 30.0), b]);
        h.event(finger(1, End, b));
        h.run();
    };
    finger_drag(&mut h);
    assert!(strokes(&h).is_empty(), "a finger doesn't draw");
    // A mouse (or a pen) still draws.
    trace(&mut h, &[a, pos2(a.x + 40.0, a.y - 30.0), b]);
    assert_eq!(strokes(&h).len(), 1);
    // Switched back on, a finger draws.
    h.get_by_label("Draw with Touch").click();
    h.run();
    finger_drag(&mut h);
    assert_eq!(strokes(&h).len(), 2);
}
