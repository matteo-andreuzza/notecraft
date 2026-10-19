//! Pictures and attachments on the canvas (decision D3), driven by pointer events through the
//! real canvas: drawing through the texture cache, select, move, resize by a handle, and the
//! picture and attachment context menus. One right click per harness (see `tests_canvas.rs`).

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use notecraft_engine::doc::Id;
use notecraft_engine::selection::PageSel;
use serde_json::json;

use crate::NoteApp;
use crate::tests::app;
use crate::tests_canvas::{drag, right_click};

fn harness(app: NoteApp) -> Harness<'static, NoteApp> {
    Harness::builder().with_size(egui::vec2(1280.0, 800.0)).build_ui_state(|ui, app: &mut NoteApp| app.ui(ui), app)
}

/// A PNG made here: `w × h`, a gradient.
pub(crate) fn png(w: u32, h: u32) -> Vec<u8> {
    let px = (0..w * h).flat_map(|i| [(i % w * 255 / w) as u8, 120, (i / w * 255 / h) as u8, 255]).collect();
    notecraft_codecs::encode_png(&notecraft_codecs::Rgba8::new(w, h, px).unwrap(), &Default::default()).unwrap()
}

/// A new page with a 160×80 px picture at (60, 120) pt, nothing selected, on the View tab (no
/// ribbon label clashes with menu labels).
fn page_with_picture() -> (NoteApp, Id) {
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    let r = a.session.run("insert.picture", json!({"data": notecraft_engine::b64::encode(&png(160, 80)), "x": 60.0, "y": 120.0})).unwrap();
    let id = Id::parse(r["picture"].as_str().unwrap()).unwrap();
    a.session.sel.on_page = PageSel::None;
    a.ui.tab = "View".into();
    a.canvas.focused = true;
    (a, id)
}

fn rect(h: &Harness<'static, NoteApp>, id: Id) -> egui::Rect {
    h.state().canvas.media.iter().find(|m| m.id == id).unwrap().rect
}

#[test]
fn a_picture_is_drawn_selected_moved_and_resized() {
    let (a, id) = page_with_picture();
    let mut h = harness(a);
    h.run();
    assert_eq!(h.state().canvas.textures.stats().0, 1, "decoded once into the texture cache");
    let r = rect(&h, id);
    assert!((r.width() - 120.0 * crate::canvas::PT).abs() < 0.5, "160 px at 96 dpi = 120 pt");
    // Click selects; dragging the body moves it.
    crate::tests_canvas::click(&mut h, r.center());
    assert_eq!(h.state().session.sel.on_page, PageSel::Objects { ids: vec![id] });
    drag(&mut h, r.center(), r.center() + egui::vec2(40.0, 20.0));
    let im = h.state().session.page().unwrap().image(id).unwrap().clone();
    assert!(
        (im.pos.x - (60.0 + 40.0 / crate::canvas::PT)).abs() < 1.0 && (im.pos.y - (120.0 + 20.0 / crate::canvas::PT)).abs() < 1.0,
        "{:?}",
        im.pos
    );
    // The bottom-right handle keeps the proportions.
    let r = rect(&h, id);
    drag(&mut h, r.right_bottom(), r.right_bottom() + egui::vec2(60.0, 0.0));
    let im = h.state().session.page().unwrap().image(id).unwrap().clone();
    assert!(im.width > 150.0, "{}", im.width);
    assert!((im.height / im.width - 0.5).abs() < 0.01, "a corner handle keeps the proportions");
    // Arrow keys nudge the selected picture (CM.Picture.ObjectMove).
    let x0 = im.pos.x;
    h.key_press(egui::Key::ArrowRight);
    h.run();
    assert!((h.state().session.page().unwrap().image(id).unwrap().pos.x - x0 - 1.0).abs() < 0.01);
}

#[test]
fn right_click_on_a_picture_opens_the_picture_menu() {
    let (mut a, id) = page_with_picture();
    a.session.run("object.resize", json!({"id": id.to_string(), "width": 300.0})).unwrap();
    a.session.sel.on_page = PageSel::None;
    let mut h = harness(a);
    h.run();
    let r = rect(&h, id);
    right_click(&mut h, r.center());
    assert_eq!(h.state().session.sel.on_page, PageSel::Objects { ids: vec![id] }, "the right click selects the picture");
    assert!(h.query_by_label_contains("Order").is_some(), "CM.Picture.OrderObjectsMenu");
    assert!(h.query_by_label_contains("Rotate").is_some(), "CM.Picture.RotateInkMenu");
    h.get_by_label("Restore to Original Size").click();
    h.run();
    let im = h.state().session.page().unwrap().image(id).unwrap().clone();
    assert_eq!((im.width, im.height), (120.0, 60.0), "CM.Picture.RestoreImageSize from the menu");
}

#[test]
fn right_click_on_an_attachment_opens_the_attachment_menu() {
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    let r = a.session.run("insert.file", json!({"data": "aGVsbG8=", "name": "notes.txt", "x": 80.0, "y": 140.0})).unwrap();
    let id = Id::parse(r["attachment"].as_str().unwrap()).unwrap();
    a.session.sel.on_page = PageSel::None;
    a.ui.tab = "View".into();
    let mut h = harness(a);
    h.run();
    let r = rect(&h, id);
    right_click(&mut h, r.center());
    assert!(h.query_by_label("Save As…").is_some(), "CM.FileAttachment.FileAttachmentSaveAs");
    h.get_by_label("Rename").click();
    h.run();
    assert!(matches!(h.state().ui.dialog, Some(crate::dialogs::Dialog::RenameAttachment(ref n)) if n == "notes.txt"));
}
