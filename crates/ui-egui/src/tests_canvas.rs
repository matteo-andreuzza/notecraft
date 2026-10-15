//! Canvas tests: the paragraph handle, containers growing with their text and the page
//! growing with its content, driven by pointer events through the real canvas.

use egui::{Pos2, pos2};
use egui_kittest::Harness;
use notecraft_engine::doc::Id;
use notecraft_engine::selection::Block;
use serde_json::json;

use crate::NoteApp;
use crate::tests::app;

fn harness(app: NoteApp) -> Harness<'static, NoteApp> {
    Harness::builder().with_size(egui::vec2(1280.0, 800.0)).build_ui_state(|ui, app: &mut NoteApp| app.ui(ui), app)
}

/// A new page with one container at (60, 120) pt holding `text` (newlines = paragraphs).
pub(crate) fn page_with(text: &str) -> (NoteApp, Id) {
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    a.session.run("outline.new", json!({"x": 60.0, "y": 120.0})).unwrap();
    a.session.run("text.insert", json!({"text": text})).unwrap();
    let id = a.session.page().unwrap().objects[0].id();
    a.canvas.focused = true;
    (a, id)
}

pub(crate) fn texts(h: &Harness<'static, NoteApp>, id: Id) -> Vec<(String, u8)> {
    h.state().session.page().unwrap().outline(id).unwrap().paragraphs.iter().map(|p| (p.text(), p.level)).collect()
}

fn handle(h: &Harness<'static, NoteApp>, id: Id, para: usize) -> Pos2 {
    h.state().canvas.para(Block::Outline(id), para).unwrap().handle.center()
}

pub(crate) fn press(h: &mut Harness<'static, NoteApp>, p: Pos2, pressed: bool) {
    h.event(egui::Event::PointerMoved(p));
    h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    h.run();
}

pub(crate) fn click(h: &mut Harness<'static, NoteApp>, p: Pos2) {
    h.event(egui::Event::PointerMoved(p));
    h.run_steps(60);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
}

/// Two clicks, well after any earlier click. The harness clock advances by whole frames and
/// spreads queued events over frames, so the double-click window is widened for the test and
/// earlier clicks are pushed out of the triple-click window first.
pub(crate) fn double_click(h: &mut Harness<'static, NoteApp>, p: Pos2) {
    h.ctx.options_mut(|o| o.input_options.max_double_click_delay = 2.0);
    h.event(egui::Event::PointerMoved(p));
    h.run_steps(400);
    for pressed in [true, false, true, false] {
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
}

pub(crate) fn drag(h: &mut Harness<'static, NoteApp>, from: Pos2, to: Pos2) {
    press(h, from, true);
    for k in 1..=6 {
        let t = k as f32 / 6.0;
        h.event(egui::Event::PointerMoved(from + (to - from) * t));
        h.run();
    }
    press(h, to, false);
}

#[test]
fn the_paragraph_handle_selects_moves_and_collapses() {
    // parity: UI.Canvas.ParagraphHandle
    let (a, o) = page_with("Alpha\nBeta\nGamma");
    let mut h = harness(a);
    h.run();
    // Click: the paragraph (with its children) is selected.
    let p = handle(&h, o, 1);
    click(&mut h, p);
    let r = h.state().session.text_range().unwrap();
    assert_eq!((r.ordered().0.para, r.ordered().1.para, r.ordered().1.offset), (1, 1, 4), "UI.Canvas.ParagraphHandle: click selects");
    // Drag: Gamma goes above Alpha, as one undo step.
    let undo = h.state().session.undo_labels().len();
    let from = handle(&h, o, 2);
    let to = h.state().canvas.para(Block::Outline(o), 0).unwrap().pos + egui::vec2(2.0, 1.0);
    drag(&mut h, from, to);
    assert_eq!(texts(&h, o).iter().map(|t| t.0.as_str()).collect::<Vec<_>>(), vec!["Gamma", "Alpha", "Beta"], "BEH.Outline.DragParagraph");
    assert_eq!(h.state().session.undo_labels().len(), undo + 1);
    // Dragging to the right of the paragraph above indents it under that paragraph.
    let from = handle(&h, o, 2);
    let beta = h.state().canvas.para(Block::Outline(o), 2).unwrap().clone();
    drag(&mut h, from, pos2(beta.pos.x + 40.0, beta.pos.y + 2.0));
    assert_eq!(texts(&h, o)[2], ("Beta".to_string(), 1), "dropped one level deeper");
    // Double-click on a parent's handle collapses its children; again expands them.
    let p = handle(&h, o, 1);
    double_click(&mut h, p);
    let page = h.state().session.page().unwrap().clone();
    assert!(page.outline(o).unwrap().paragraphs[1].collapsed, "BEH.Outline.CollapseExpand");
    assert!(h.state().canvas.para(Block::Outline(o), 2).is_none(), "the child is hidden");
    double_click(&mut h, p);
    assert!(!h.state().session.page().unwrap().outline(o).unwrap().paragraphs[1].collapsed);
    // Dropped far from every container: a new container there.
    let from = handle(&h, o, 0);
    drag(&mut h, from, pos2(900.0, 650.0));
    let page = h.state().session.page().unwrap().clone();
    assert_eq!(page.objects.len(), 2, "a new container holds the dropped paragraph");
    assert_eq!(page.objects[1].as_outline().unwrap().paragraphs[0].text(), "Gamma");
}

#[test]
fn containers_grow_with_their_text_and_the_page_grows_with_them() {
    // parity: BEH.Canvas.ContainerAutoWidth, BEH.Canvas.PageGrows
    let (a, o) = page_with("Hi");
    let mut h = harness(a);
    h.run();
    let w0 = h.state().canvas.outlines[0].rect.width();
    h.state_mut().session.run("text.insert", json!({"text": " there, a longer line of text"})).unwrap();
    h.run();
    let w1 = h.state().canvas.outlines[0].rect.width();
    assert!(w1 > w0 + 50.0, "BEH.Canvas.ContainerAutoWidth: the container widens with its text ({w0} → {w1})");
    // Up to the maximum width, then the text wraps and the container grows down.
    let long = "word ".repeat(200);
    h.state_mut().session.run("text.insert", json!({"text": long})).unwrap();
    h.run();
    let z = crate::canvas::PT;
    let r = h.state().canvas.outlines[0].rect;
    let max = notecraft_engine::doc::page::AUTO_WIDTH_MAX * z;
    assert!(r.width() <= max + 16.0 * z + 12.0, "capped at the maximum width: {} > {max}", r.width());
    assert!(r.height() > 80.0, "wrapped lines make it taller: {}", r.height());
    assert_eq!(h.state().canvas.paras.iter().filter(|p| p.block == Block::Outline(o)).count(), 1);
    // The page grows right and down: a container moved far away is still reachable.
    let id = o.to_string();
    h.state_mut().session.run("outline.move", json!({"id": id, "x": 3000.0, "y": 2500.0})).unwrap();
    h.run();
    let ext = crate::canvas::page_extent(h.state());
    assert!(ext.x > 3000.0 * z && ext.y > 2500.0 * z, "BEH.Canvas.PageGrows: extent {ext:?}");
}

#[test]
fn the_container_bar_moves_and_its_edge_resizes() {
    // parity: UI.Canvas.Container
    let (a, o) = page_with("Some text");
    let mut h = harness(a);
    h.run();
    // Hovering shows the frame with the move bar and the resize edge.
    let ol = h.state().canvas.outlines[0];
    h.event(egui::Event::PointerMoved(ol.rect.center()));
    h.run();
    // The bar moves the container (one undo step for the whole drag).
    let undo = h.state().session.undo_labels().len();
    let before = h.state().session.page().unwrap().outline(o).unwrap().pos;
    drag(&mut h, ol.bar.center(), ol.bar.center() + egui::vec2(120.0, 60.0));
    let after = h.state().session.page().unwrap().outline(o).unwrap().pos;
    let z = crate::canvas::PT;
    assert!(
        (after.x - before.x - 120.0 / z).abs() < 2.0 && (after.y - before.y - 60.0 / z).abs() < 2.0,
        "UI.Canvas.Container: {before:?} → {after:?}"
    );
    assert_eq!(h.state().session.undo_labels().len(), undo + 1);
    // The right edge sets a fixed width.
    let ol = h.state().canvas.outlines[0];
    drag(&mut h, ol.resize.center(), ol.resize.center() + egui::vec2(80.0, 0.0));
    let w = h.state().session.page().unwrap().outline(o).unwrap().width;
    assert!(matches!(w, notecraft_engine::doc::OutlineWidth::Fixed(_)), "{w:?}");
    // A click in the text still places the caret.
    let pl = h.state().canvas.para(Block::Outline(o), 0).unwrap().clone();
    click(&mut h, pl.pos + egui::vec2(3.0, 4.0));
    let c = h.state().session.sel.caret().unwrap();
    assert_eq!((c.block, c.para, c.offset), (Block::Outline(o), 0, 0));
}
