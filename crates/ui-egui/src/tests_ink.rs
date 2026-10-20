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
use crate::tests::app;
use crate::tests_canvas::{click, drag, press, right_click};

fn harness(app: NoteApp) -> Harness<'static, NoteApp> {
    Harness::builder().with_size(egui::vec2(1280.0, 800.0)).build_ui_state(|ui, app: &mut NoteApp| app.ui(ui), app)
}

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
