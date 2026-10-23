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
    assert!(h.query_by_label("notes.txt").is_some(), "CM.FileAttachment.FileAttachmentName");
    assert!(h.query_by_label("5 bytes").is_some(), "CM.FileAttachment.FileAttachmentSize");
    h.get_by_label("Rename").click();
    h.run();
    assert!(matches!(h.state().ui.dialog, Some(crate::dialogs::Dialog::RenameAttachment(ref n)) if n == "notes.txt"));
}

// ---- The rich system clipboard, through a host with a fake clipboard. ----

#[derive(Default)]
struct Clip {
    text: Option<String>,
    html: Option<String>,
    image: Option<notecraft_codecs::Rgba8>,
}

struct ClipHost(std::sync::Arc<std::sync::Mutex<Clip>>);

impl crate::Host for ClipHost {
    fn clock(&self) -> notecraft_engine::Clock {
        notecraft_engine::Clock { now_ms: 1_792_007_100_000, utc_offset_min: 0 }
    }
    fn read_clipboard(&mut self, image: bool) -> crate::RichClip {
        let c = self.0.lock().unwrap();
        crate::RichClip { text: c.text.clone(), html: c.html.clone(), image: if image { c.image.clone() } else { None } }
    }
    fn write_clipboard(&mut self, text: &str, html: &str, image: Option<&notecraft_codecs::Rgba8>) -> bool {
        let mut c = self.0.lock().unwrap();
        *c = Clip { text: (!text.is_empty()).then(|| text.to_string()), html: (!html.is_empty()).then(|| html.to_string()), image: image.cloned() };
        true
    }
}

fn clip_app() -> (NoteApp, std::sync::Arc<std::sync::Mutex<Clip>>) {
    let clip = std::sync::Arc::new(std::sync::Mutex::new(Clip::default()));
    let mut s = notecraft_engine::Session::with_sample(notecraft_engine::Clock { now_ms: 1_792_007_100_000, utc_offset_min: 0 });
    s.prefs.author = "Tester".into();
    let mut a = NoteApp::new(s, Box::new(ClipHost(clip.clone())));
    a.session.run("page.new", json!({})).unwrap();
    a.session.sel.on_page = PageSel::None;
    a.ui.tab = "View".into();
    a.canvas.focused = true;
    (a, clip)
}

/// Ctrl+V as egui-winit delivers it: a paste event when the clipboard has text, otherwise
/// only the key release.
fn ctrl_v(h: &mut Harness<'static, NoteApp>, text: Option<&str>) {
    let m = egui::Modifiers::COMMAND;
    if let Some(t) = text {
        h.event(egui::Event::Paste(t.to_string()));
    }
    h.event(egui::Event::Key { key: egui::Key::V, physical_key: None, pressed: false, repeat: false, modifiers: m });
    h.run();
}

#[test]
fn copying_a_picture_puts_its_pixels_on_the_system_clipboard() {
    // UI.Window.Clipboard: a copied picture goes out as a picture; pasting it back while the
    // system clipboard still holds it pastes the object (alt text kept), not a new picture.
    let (mut a, clip) = clip_app();
    let r = a.session.run("insert.picture", json!({"data": notecraft_engine::b64::encode(&png(30, 20))})).unwrap();
    let id = Id::parse(r["picture"].as_str().unwrap()).unwrap();
    a.session.run("object.altText", json!({"text": "kept"})).unwrap();
    let mut h = harness(a);
    h.run();
    let ctx = h.ctx.clone();
    h.state_mut().run(&ctx, "edit.copy", json!({}));
    let img = clip.lock().unwrap().image.clone();
    assert!(img.is_some(), "UI.Window.Clipboard");
    let img = img.unwrap();
    assert_eq!((img.width, img.height), (30, 20));
    ctrl_v(&mut h, None);
    let pics: Vec<_> = h.state().session.page().unwrap().objects.iter().filter_map(|o| o.as_image()).cloned().collect();
    assert_eq!(pics.len(), 2);
    assert!(pics.iter().all(|p| p.alt_text == "kept"), "pasted as the copied object");
    assert!(pics.iter().any(|p| p.id != id));
}

#[test]
fn ctrl_v_with_only_another_programs_picture_pastes_it() {
    // BEH.Paste.Images
    let (a, clip) = clip_app();
    let shot = notecraft_codecs::decode(&png(40, 10), &Default::default()).unwrap();
    clip.lock().unwrap().image = Some(shot);
    let mut h = harness(a);
    h.run();
    ctrl_v(&mut h, None);
    let pics: Vec<_> = h.state().session.page().unwrap().objects.iter().filter_map(|o| o.as_image()).cloned().collect();
    assert_eq!(pics.len(), 1, "BEH.Paste.Images");
    assert_eq!(pics[0].pixels, (40, 10));
}

#[test]
fn pasting_html_from_a_browser_keeps_its_table() {
    // BEH.Paste.HTML
    let (a, clip) = clip_app();
    *clip.lock().unwrap() = Clip {
        text: Some("Fruit\tQty\nPears\t4".into()),
        html: Some("<table><tr><td><b>Fruit</b></td><td>Qty</td></tr><tr><td>Pears</td><td>4</td></tr></table>".into()),
        image: None,
    };
    let mut h = harness(a);
    h.run();
    ctrl_v(&mut h, Some("Fruit\tQty\nPears\t4"));
    let page = h.state().session.page().unwrap().clone();
    let t = page.objects.iter().find_map(|o| o.as_table()).expect("BEH.Paste.HTML: pasted as a table");
    assert_eq!(t.plain_text(), "Fruit\tQty\nPears\t4");
    assert_eq!(t.rows[0].cells[0].paragraphs[0].runs[0].fmt.bold, Some(true));
}

#[derive(Debug)]
struct DroppedPath(std::path::PathBuf);

impl egui::DroppedFile for DroppedPath {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

#[test]
fn dropping_files_inserts_a_picture_and_an_attachment() {
    // BEH.Paste.Files: a dropped picture becomes a picture, another file an attachment.
    let dir = std::env::temp_dir().join(format!("notecraft-drop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (pic, doc) = (dir.join("photo.png"), dir.join("notes.txt"));
    std::fs::write(&pic, png(16, 8)).unwrap();
    std::fs::write(&doc, b"hello").unwrap();
    let (a, _) = clip_app();
    let mut h = harness(a);
    h.run();
    let at = h.state().canvas.viewport.center();
    h.event(egui::Event::PointerMoved(at));
    h.run();
    for p in [&pic, &doc] {
        h.input_mut().dropped_files.push(std::sync::Arc::new(DroppedPath(p.clone())));
    }
    h.run();
    let page = h.state().session.page().unwrap().clone();
    let im = page.objects.iter().find_map(|o| o.as_image()).expect("BEH.Paste.Files");
    assert_eq!(im.pixels, (16, 8));
    let att = page.objects.iter().find_map(|o| o.as_attachment()).unwrap();
    assert_eq!(att.name, "notes.txt");
    let drop_pt = h.state().canvas.to_page(at);
    assert!((im.pos.x - drop_pt.x).abs() < 1.0 && (att.pos.x - drop_pt.x - 24.0).abs() < 1.0, "at the drop point, stacked");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dropping_a_pdf_asks_whether_to_attach_it_or_insert_its_printout() {
    // BEH.Paste.Files: a file NoteCraft can print asks first.
    let dir = std::env::temp_dir().join(format!("notecraft-drop-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pdf = dir.join("report.pdf");
    std::fs::write(&pdf, notecraft_codecs::pdf::sample_pdf(&[(200.0, 100.0, [0, 0, 255]), (200.0, 100.0, [255, 0, 0])])).unwrap();
    for (button, pictures) in [("Insert Printout", 2), ("Attach File", 0)] {
        let (a, _) = clip_app();
        let mut h = harness(a);
        h.run();
        let at = h.state().canvas.viewport.center();
        h.event(egui::Event::PointerMoved(at));
        h.run();
        h.input_mut().dropped_files.push(std::sync::Arc::new(DroppedPath(pdf.clone())));
        h.run();
        assert!(matches!(&h.state().ui.dialog, Some(crate::dialogs::Dialog::InsertFile(f)) if f.len() == 1), "BEH.Paste.Files");
        assert!(h.query_by_label_contains("report.pdf").is_some(), "the question names the file");
        assert!(h.state().session.page().unwrap().objects.is_empty(), "nothing inserted before the answer");
        h.get_by_label(button).click();
        h.run();
        assert!(h.state().ui.dialog.is_none());
        let page = h.state().session.page().unwrap();
        let att = page.objects.iter().find_map(|o| o.as_attachment()).expect("the file is attached either way");
        assert_eq!(att.name, "report.pdf");
        assert_eq!(page.objects.iter().filter(|o| o.as_image().is_some()).count(), pictures, "{button}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edit_link_on_a_picture_starts_from_its_link_and_changes_it() {
    let (mut a, id) = page_with_picture();
    a.session.run("object.select", json!({"ids": [id.to_string()]})).unwrap();
    a.session.run("insert.link", json!({"url": "https://example.org/old"})).unwrap();
    let d = crate::ribbon::dialog_for(&a, "CM.Picture.LinkEdit");
    let Some(crate::dialogs::Dialog::Link { url, .. }) = &d else { panic!("Edit Link… opens the Link dialog: {d:?}") };
    assert_eq!(url, "https://example.org/old", "prefilled with the picture's link");
    a.ui.dialog = Some(crate::dialogs::Dialog::Link { url: "https://example.org/new".into(), text: String::new(), filter: String::new() });
    let mut h = harness(a);
    h.run();
    h.get_by_label("OK").click();
    h.run();
    let link = h.state().session.page().unwrap().image(id).unwrap().link.clone();
    assert_eq!(link.as_deref(), Some("https://example.org/new"), "OK edits the existing link");
    assert_eq!(h.state().session.page().unwrap().objects.iter().filter(|o| o.as_outline().is_some()).count(), 0, "no linked text was typed");
}

#[test]
fn paste_picture_from_the_ribbon_takes_the_picture_beside_text() {
    // R.Home.Clipboard.PastePicture: a browser put both text and a picture on the clipboard.
    let (a, clip) = clip_app();
    {
        let mut c = clip.lock().unwrap();
        c.text = Some("a caption".into());
        c.image = Some(notecraft_codecs::decode(&png(16, 12), &Default::default()).unwrap());
    }
    let mut h = harness(a);
    h.run();
    let ctx = h.ctx.clone();
    assert!(crate::pictures::host_action(h.state_mut(), &ctx, "edit.pastePicture"));
    h.run();
    let page = h.state().session.page().unwrap();
    let pics: Vec<_> = page.objects.iter().filter_map(|o| o.as_image()).collect();
    assert_eq!(pics.iter().map(|p| p.pixels).collect::<Vec<_>>(), vec![(16, 12)], "R.Home.Clipboard.PastePicture");
    assert!(page.objects.iter().filter_map(|o| o.as_outline()).all(|o| o.paragraphs.iter().all(|p| p.text() != "a caption")));
}

#[test]
fn the_ink_menus_paste_options_paste_from_the_system_clipboard() {
    // The menus' Paste Options are the four paste choices, and they read the system clipboard
    // (another program's text here), not only what NoteCraft copied.
    let (mut a, clip) = clip_app();
    a.session.run("draw.stroke", json!({"points": [[60, 300], [200, 300]]})).unwrap();
    a.session.sel.on_page = PageSel::None;
    clip.lock().unwrap().text = Some("From elsewhere".into());
    let mut h = harness(a);
    h.run();
    let (o, z) = (h.state().canvas.origin, h.state().canvas.zoom.max(0.1) * crate::canvas::PT);
    right_click(&mut h, egui::pos2(o.x + 130.0 * z, o.y + 300.0 * z));
    assert!(matches!(h.state().canvas.context_target, Some(crate::canvas::ContextTarget::Ink(_))), "the ink menu is open");
    assert!(h.query_by_label("Paste Options:").is_some(), "CM.InkDrawing.PasteGalleryMini");
    for choice in ["Paste: Keep Source Formatting", "Paste: Merge Formatting", "Paste: Picture"] {
        assert!(h.query_by_label(choice).is_some(), "{choice}");
    }
    let at = h.get_by_label("Paste: Keep Text Only").rect().center();
    crate::tests_canvas::click(&mut h, at);
    let text = h.state().session.page().unwrap().plain_text();
    assert!(text.contains("From elsewhere"), "pasted from the system clipboard: {text:?}");
    assert!(h.query_by_label("Paste Options:").is_none(), "the menu closed");
}

#[test]
fn a_picture_in_front_of_text_is_drawn_and_clicked_over_it() {
    let (mut a, o) = crate::tests_canvas::page_with("Text under or over the picture");
    let r = a.session.run("insert.picture", json!({"data": notecraft_engine::b64::encode(&png(160, 80)), "x": 70.0, "y": 118.0})).unwrap();
    let id = Id::parse(r["picture"].as_str().unwrap()).unwrap();
    a.session.sel.on_page = PageSel::None;
    a.ui.tab = "View".into();
    let mut h = harness(a);
    h.run();
    let m = *h.state().canvas.media.iter().find(|m| m.id == id).unwrap();
    assert!(m.above, "inserted after the container and overlapping it: drawn over its text");
    let text = h.state().canvas.paras.iter().find(|p| p.block == notecraft_engine::selection::Block::Outline(o)).unwrap().rect();
    let p = text.intersect(m.rect).center();
    crate::tests_canvas::click(&mut h, p);
    assert_eq!(h.state().session.sel.on_page, PageSel::Objects { ids: vec![id] }, "a click there selects the picture");
    // Sent to the back, it goes under the text again, and the text takes the click.
    let ctx = h.ctx.clone();
    h.state_mut().run(&ctx, "object.sendToBack", json!({}));
    h.run();
    assert!(!h.state().canvas.media.iter().any(|m| m.above));
    crate::tests_canvas::click(&mut h, p);
    assert!(matches!(h.state().session.sel.caret(), Some(c) if c.block == notecraft_engine::selection::Block::Outline(o)));
}
