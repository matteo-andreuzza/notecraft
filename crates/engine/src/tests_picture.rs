//! Pictures and attachments (decision D3): insert, the picture and attachment context-menu
//! commands, copy/paste of objects, and hostile input.

use std::sync::Arc;

use notecraft_codecs::{Limits, Rgba8, encode_png};
use notecraft_doc::Id;
use serde_json::{Value, json};

use crate::Session;
use crate::selection::PageSel;
use crate::tests::{blank_page, run, sample};

/// A `w × h` PNG made here (no fixture files).
fn png(w: u32, h: u32) -> Vec<u8> {
    let px = (0..w * h).flat_map(|i| [(i * 7) as u8, 90, (i * 3) as u8, 255]).collect();
    encode_png(&Rgba8::new(w, h, px).unwrap(), &Limits::PICTURE).unwrap()
}

fn data(bytes: &[u8]) -> Value {
    json!(crate::b64::encode(bytes))
}

/// A blank page with one `w × h` picture, selected; returns its id.
fn with_picture(w: u32, h: u32) -> (Session, Id) {
    let mut s = sample();
    blank_page(&mut s);
    let r = run(&mut s, "insert.picture", json!({"data": data(&png(w, h)), "x": 50.0, "y": 120.0}));
    let id = Id::parse(r["picture"].as_str().unwrap()).unwrap();
    (s, id)
}

fn image(s: &Session, id: Id) -> notecraft_doc::Image {
    s.page().unwrap().image(id).unwrap().clone()
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("notecraft-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn insert_picture_from_data_and_path() {
    // R.Insert.Images.PictureFromFileInsert: the picture lands at its 96-dpi size, selected.
    let (mut s, id) = with_picture(80, 40);
    let im = image(&s, id);
    assert_eq!((im.width, im.height), (60.0, 30.0), "80×40 px at 96 dpi");
    assert_eq!(im.pixels, (80, 40));
    assert_eq!((im.pos.x, im.pos.y), (50.0, 120.0));
    assert_eq!(s.sel.on_page, PageSel::Objects { ids: vec![id] });
    run(&mut s, "edit.undo", json!({}));
    assert!(s.page().unwrap().image(id).is_none(), "one undo step removes it");
    // From a file, below everything; wide pictures are scaled to the insert width.
    let dir = temp_dir("pic");
    let path = dir.join("wide.png");
    std::fs::write(&path, png(1000, 100)).unwrap();
    let r = run(&mut s, "insert.picture", json!({"path": path.display().to_string()}));
    let im = image(&s, Id::parse(r["picture"].as_str().unwrap()).unwrap());
    assert_eq!((im.width, im.height), (notecraft_doc::media::INSERT_MAX_WIDTH, notecraft_doc::media::INSERT_MAX_WIDTH / 10.0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hostile_picture_input_is_refused_and_nothing_is_inserted() {
    let mut s = sample();
    blank_page(&mut s);
    let dir = temp_dir("hostile");
    let txt = dir.join("notes.png");
    std::fs::write(&txt, b"not a picture at all").unwrap();
    let cases = [
        json!({"data": data(b"GIF89a truncated")}),
        json!({"data": "@@not base64@@"}),
        json!({"data": ""}),
        json!({}),
        json!({"path": txt.display().to_string()}),
        json!({"path": dir.display().to_string()}),
        json!({"path": dir.join("missing.png").display().to_string()}),
        json!({"data": "A".repeat(90 << 20)}),
    ];
    for v in cases {
        assert!(s.run("insert.picture", v.clone()).is_err(), "{}", v.to_string().chars().take(80).collect::<String>());
    }
    assert!(s.page().unwrap().objects.is_empty());
    assert!(!s.can_undo() || s.undo_labels().iter().all(|l| l != "Pictures"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rotate_flip_and_restore() {
    // parity: CM.Picture.ObjectRotateRight90, CM.Picture.ObjectRotateLeft90
    let (mut s, id) = with_picture(80, 40);
    run(&mut s, "object.rotateRight90", json!({}));
    let im = image(&s, id);
    assert_eq!((im.quarter_turns, im.width, im.height), (1, 30.0, 60.0), "90° swaps the sides");
    assert_eq!((im.pos.x + im.width / 2.0, im.pos.y + im.height / 2.0), (80.0, 135.0), "about its centre");
    run(&mut s, "object.rotateLeft90", json!({}));
    run(&mut s, "object.rotateLeft90", json!({}));
    assert_eq!(image(&s, id).quarter_turns, 3);
    // parity: CM.Picture.ObjectFlipHorizontal, CM.Picture.ObjectFlipVertical
    // Turned an odd number of times, a flip across the screen's vertical axis is the
    // picture's own vertical flip.
    run(&mut s, "object.flipHorizontal", json!({}));
    let im = image(&s, id);
    assert!(im.flip_v && !im.flip_h);
    run(&mut s, "object.rotateRight90", json!({}));
    run(&mut s, "object.flipVertical", json!({}));
    run(&mut s, "object.flipHorizontal", json!({}));
    let im = image(&s, id);
    assert!(im.flip_h && !im.flip_v, "{im:?}");
    // parity: CM.Picture.RestoreImageSize
    run(&mut s, "object.resize", json!({"width": 300.0}));
    assert_eq!((image(&s, id).width, image(&s, id).height), (300.0, 150.0), "one side keeps the proportions");
    run(&mut s, "object.restoreSize", json!({}));
    assert_eq!((image(&s, id).width, image(&s, id).height), (60.0, 30.0));
}

#[test]
fn move_resize_background_and_alt_text() {
    let (mut s, id) = with_picture(80, 40);
    // CM.Picture.ObjectMove / ObjectResize from the menu explain the gestures.
    run(&mut s, "object.move", json!({}));
    run(&mut s, "object.resize", json!({}));
    assert_eq!(s.messages.len(), 2);
    run(&mut s, "object.move", json!({"dx": 10.0, "dy": -500.0}));
    let im = image(&s, id);
    assert_eq!((im.pos.x, im.pos.y), (60.0, 0.0), "never above the page");
    run(&mut s, "object.resize", json!({"dw": 12.0}));
    assert_eq!((image(&s, id).width, image(&s, id).height), (72.0, 36.0));
    run(&mut s, "object.resize", json!({"id": id.to_string(), "width": 1e9, "height": -1.0}));
    let im = image(&s, id);
    assert_eq!((im.width, im.height), (notecraft_doc::media::MAX_PICTURE_SIZE, notecraft_doc::media::MIN_PICTURE_SIZE));
    // CM.Picture.AlternativeText
    run(&mut s, "object.altText", json!({"text": "Sales by month"}));
    assert_eq!(image(&s, id).alt_text, "Sales by month");
    assert!(s.run("object.altText", json!({"text": "x".repeat(5000)})).is_err());
    // CM.Picture.PictureAsBackground: behind everything, deselected; toggles back.
    run(&mut s, "text.insert", json!({"text": "note"}));
    run(&mut s, "object.select", json!({"ids": [id.to_string()]}));
    run(&mut s, "object.bringToFront", json!({}));
    let r = run(&mut s, "object.setAsBackground", json!({}));
    assert_eq!(r["background"], true);
    assert!(image(&s, id).background);
    assert_eq!(s.page().unwrap().objects[0].id(), id);
    assert_eq!(s.sel.on_page, PageSel::None);
    run(&mut s, "object.select", json!({"ids": [id.to_string()]}));
    assert_eq!(run(&mut s, "object.setAsBackground", json!({}))["background"], false);
}

#[test]
fn pictures_copy_and_paste_as_objects_sharing_bytes() {
    // CM.Picture.CopyOneNote / CM.Picture.Cut through edit.copy / edit.cut.
    let (mut s, id) = with_picture(8, 8);
    run(&mut s, "edit.copy", json!({}));
    let r = run(&mut s, "edit.paste", json!({}));
    let pasted = Id::parse(r["objects"][0].as_str().unwrap()).unwrap();
    assert_ne!(pasted, id);
    let (a, b) = (image(&s, id), image(&s, pasted));
    assert!(Arc::ptr_eq(&a.blob.bytes, &b.blob.bytes), "the bytes are shared, not copied");
    assert_eq!(s.sel.on_page, PageSel::Objects { ids: vec![pasted] });
    // Something else on the system clipboard wins over the copied picture.
    let r = s.run("edit.paste", json!({"text": "plain words"}));
    assert!(r.is_ok());
    assert_eq!(s.page().unwrap().objects.iter().filter(|o| o.as_image().is_some()).count(), 2);
    run(&mut s, "object.select", json!({"ids": [id.to_string()]}));
    run(&mut s, "edit.cut", json!({}));
    assert!(s.page().unwrap().image(id).is_none());
    run(&mut s, "object.select", json!({"ids": [pasted.to_string()]}));
    run(&mut s, "edit.delete", json!({}));
    assert!(s.page().unwrap().image(pasted).is_none(), "Delete removes a selected picture");
}

#[test]
fn save_picture_and_attachments() {
    let (mut s, _) = with_picture(4, 4);
    let dir = temp_dir("save");
    let out = dir.join("copy.png");
    run(&mut s, "object.saveAs", json!({"path": out.display().to_string()}));
    assert_eq!(std::fs::read(&out).unwrap(), png(4, 4), "CM.Picture.ImageSaveAs writes the stored bytes");
    // R.Insert.Files.FileAttachmentInsert, from data with a name and from a path.
    let r = run(&mut s, "insert.file", json!({"data": data(b"%PDF-1.7 report"), "name": "../Q3 report.pdf"}));
    let a = Id::parse(r["attachment"].as_str().unwrap()).unwrap();
    assert_eq!(s.page().unwrap().attachment(a).unwrap().name, "Q3 report.pdf", "a hostile name is reduced to a file name");
    assert!(s.page().unwrap().plain_text().contains("Q3 report.pdf"));
    // CM.FileAttachment.RenameSelected
    run(&mut s, "attachment.rename", json!({"name": "Budget.pdf"}));
    assert!(s.run("attachment.rename", json!({"name": "  "})).is_err());
    // CM.FileAttachment.FileAttachmentSaveAs / FileAttachmentOpen
    let saved = dir.join("out.pdf");
    run(&mut s, "attachment.saveAs", json!({"path": saved.display().to_string()}));
    assert_eq!(std::fs::read(&saved).unwrap(), b"%PDF-1.7 report");
    let r = run(&mut s, "attachment.open", json!({"dir": dir.display().to_string()}));
    let opened = std::path::PathBuf::from(r["path"].as_str().unwrap());
    assert!(opened.starts_with(&dir) && opened.ends_with("Budget.pdf"));
    assert_eq!(std::fs::read(&opened).unwrap(), b"%PDF-1.7 report");
    // CM.FileAttachment.AlternativeText shares object.altText.
    run(&mut s, "object.altText", json!({"text": "The budget"}));
    assert_eq!(s.page().unwrap().attachment(a).unwrap().alt_text, "The budget");
    let file = dir.join("data.bin");
    std::fs::write(&file, [0u8, 1, 2]).unwrap();
    let r = run(&mut s, "insert.file", json!({"path": file.display().to_string()}));
    let b = Id::parse(r["attachment"].as_str().unwrap()).unwrap();
    assert_eq!(s.page().unwrap().attachment(b).unwrap().name, "data.bin");
    // Picture commands are disabled on an attachment and vice versa.
    assert!(s.run("object.rotateRight90", json!({})).is_err());
    let first = s.page().unwrap().objects[0].id().to_string();
    run(&mut s, "object.select", json!({"ids": [first]}));
    assert!(s.run("attachment.open", json!({})).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_page_holds_at_most_max_objects() {
    let (mut s, id) = with_picture(2, 2);
    let im = image(&s, id);
    let page = s.page_mut().unwrap();
    page.objects = (0..notecraft_doc::media::MAX_PAGE_OBJECTS)
        .map(|i| {
            let mut c = im.clone();
            c.id = Id(10_000 + i as u128);
            notecraft_doc::PageObject::Image(c)
        })
        .collect();
    assert!(s.run("insert.picture", json!({"data": data(&png(2, 2))})).is_err());
    assert!(s.run("insert.file", json!({"data": data(b"x"), "name": "x"})).is_err());
}
