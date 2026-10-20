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

// ---- The rich clipboard (HTML and pictures from other applications). ----

#[test]
fn copy_gives_html_with_formatting_and_tables() {
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "plain bold"}));
    let o = s.page().unwrap().objects[0].id();
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 0, "offset": 6}));
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 0, "offset": 10, "extend": true}));
    run(&mut s, "format.bold", json!({}));
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 0, "offset": 0}));
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 0, "offset": 10, "extend": true}));
    let r = run(&mut s, "edit.copy", json!({}));
    assert_eq!(r["text"], "plain bold");
    assert!(r["html"].as_str().unwrap().contains("plain <b>bold</b>"), "{}", r["html"]);
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 2}));
    run(&mut s, "text.insert", json!({"text": "a\tb"}));
    run(&mut s, "table.selectTable", json!({}));
    let r = run(&mut s, "edit.copy", json!({}));
    assert!(r["text"].as_str().unwrap().starts_with("a\tb"), "{}", r["text"]);
    assert!(r["html"].as_str().unwrap().contains("<table"), "{}", r["html"]);
}

#[test]
fn pasting_html_from_another_application() {
    // BEH.Paste.HTML: formatting, lists and tables survive; a table becomes a table object.
    let mut s = sample();
    blank_page(&mut s);
    let html = "<p>Intro <b>bold</b></p><ul><li>one</li><li>two</li></ul><table><tr><td>Item</td><td>Qty</td></tr><tr><td>Pears</td><td>4</td></tr></table><p>After</p>";
    run(&mut s, "edit.paste", json!({"text": "Intro bold\none\ntwo\nItem\tQty\nPears\t4\nAfter", "html": html}));
    let page = s.page().unwrap();
    let outlines: Vec<_> = page.objects.iter().filter_map(|o| o.as_outline()).collect();
    let first: Vec<String> = outlines[0].paragraphs.iter().map(|p| p.text()).collect();
    assert_eq!(first, vec!["Intro bold", "one", "two"]);
    assert!(outlines[0].paragraphs[0].runs.iter().any(|r| r.text == "bold" && r.fmt.bold == Some(true)));
    assert!(matches!(outlines[0].paragraphs[1].list, Some(notecraft_doc::ListFormat::Bullet(_))));
    let t = page.objects.iter().find_map(|o| o.as_table()).unwrap();
    assert_eq!(t.plain_text(), "Item\tQty\nPears\t4");
    assert_eq!(outlines[1].paragraphs[0].text(), "After");
    assert!(outlines[1].pos.y > t.pos.y && t.pos.y > outlines[0].pos.y, "stacked in reading order");
    // One undo step takes the whole paste away.
    run(&mut s, "edit.undo", json!({}));
    assert!(s.page().unwrap().objects.iter().all(|o| o.as_table().is_none()));
    // Keep Text Only ignores the HTML.
    run(&mut s, "edit.pasteTextOnly", json!({"text": "just text", "html": html}));
    assert!(s.page().unwrap().objects.iter().all(|o| o.as_table().is_none()));
}

#[test]
fn pasting_a_picture_and_copied_tables() {
    // BEH.Paste.Images: a picture on the system clipboard (no text) becomes a picture.
    let mut s = sample();
    blank_page(&mut s);
    let r = run(&mut s, "edit.paste", json!({"image": crate::b64::encode(&png(20, 10))}));
    let id = Id::parse(r["picture"].as_str().unwrap()).unwrap();
    assert_eq!(image(&s, id).pixels, (20, 10));
    assert!(s.run("edit.paste", json!({"image": data(b"not a picture")})).is_err());
    // A whole table copied inside NoteCraft pastes back as a table, with fresh ids.
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 3}));
    run(&mut s, "text.insert", json!({"text": "x"}));
    run(&mut s, "table.selectTable", json!({}));
    let tid = s.page().unwrap().objects.iter().find_map(|o| o.as_table()).unwrap().id;
    run(&mut s, "object.select", json!({"ids": [tid.to_string()]}));
    let copied = run(&mut s, "edit.copy", json!({}));
    run(&mut s, "edit.paste", json!({"text": copied["text"]}));
    let tables: Vec<_> = s.page().unwrap().objects.iter().filter_map(|o| o.as_table()).cloned().collect();
    assert_eq!(tables.len(), 2);
    assert_ne!(tables[0].rows[0].cells[0].id, tables[1].rows[0].cells[0].id);
    assert_eq!(tables[1].plain_text(), tables[0].plain_text());
}

/// The page's printout pictures linked to `att`, in order.
fn printout_pages(s: &Session, att: Id) -> Vec<notecraft_doc::Image> {
    s.page().unwrap().objects.iter().filter_map(|o| o.as_image().filter(|im| im.printout == Some(att)).cloned()).collect()
}

#[test]
fn file_printouts_attach_the_file_and_one_picture_per_page() {
    let mut s = sample();
    blank_page(&mut s);
    let pdf = notecraft_codecs::pdf::sample_pdf(&[(200.0, 100.0, [255, 0, 0]), (100.0, 300.0, [0, 0, 255])]);
    let undo = s.undo_labels().len();
    let r = run(&mut s, "insert.printout", json!({"data": data(&pdf), "name": "Report.pdf", "x": 40.0, "y": 150.0}));
    let att = Id::parse(r["attachment"].as_str().unwrap()).unwrap();
    assert_eq!(r["total"], 2);
    let a = s.page().unwrap().attachment(att).unwrap().clone();
    assert_eq!(a.name, "Report.pdf");
    assert_eq!(&a.blob.bytes[..], &pdf[..], "the attachment keeps the file unchanged");
    let pages = printout_pages(&s, att);
    assert_eq!(pages.len(), 2, "FMT.Import.Pdf");
    assert_eq!((pages[0].width, pages[0].height), (200.0, 100.0), "pages come in at their print size");
    assert_eq!((pages[1].width, pages[1].height), (100.0, 300.0));
    assert!(pages[0].pos.y > a.pos.y && pages[1].pos.y > pages[0].pos.y + 100.0, "stacked below the attachment");
    assert_eq!(pages[0].pixels, (300, 150), "rendered at 108 dpi");
    assert_eq!(s.undo_labels().len(), undo + 1, "one undo step");
    // Open Original from a printed page writes the file for the host to open.
    run(&mut s, "object.select", json!({"ids": [pages[1].id.to_string()]}));
    let dir = temp_dir("printout");
    let r = run(&mut s, "printout.openOriginal", json!({"dir": dir.to_str().unwrap()}));
    assert_eq!(std::fs::read(r["path"].as_str().unwrap()).unwrap(), pdf, "CM.Picture.ViewOriginalPrintout");
    // Delete Printout Pages, then Insert Printout again from the attachment's menu.
    run(&mut s, "object.select", json!({"ids": [att.to_string()]}));
    assert_eq!(run(&mut s, "printout.delete", json!({}))["deleted"], 2, "CM.FileAttachment.JotDeletePrintoutPages");
    assert!(printout_pages(&s, att).is_empty());
    assert!(s.run("printout.delete", json!({})).is_err(), "nothing left to delete");
    run(&mut s, "attachment.insertPrintout", json!({}));
    run(&mut s, "attachment.insertPrintout", json!({}));
    assert_eq!(printout_pages(&s, att).len(), 2, "CM.FileAttachment.FilePrintoutInsert replaces earlier pages");
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(printout_pages(&s, att).len(), 2, "undo brings the first pages back");
}

#[test]
fn printouts_refuse_what_they_cannot_print() {
    let mut s = sample();
    blank_page(&mut s);
    let n = s.page().unwrap().objects.len();
    let e = s.run("insert.printout", json!({"data": data(b"plain text, not printable"), "name": "a.txt"})).unwrap_err();
    assert!(e.to_string().contains("PDF files and pictures"), "{e}");
    assert!(s.run("insert.printout", json!({"data": data(b"%PDF-1.7 truncated")})).is_err());
    assert_eq!(s.page().unwrap().objects.len(), n, "nothing is inserted");
    // A picture prints as itself.
    let r = run(&mut s, "insert.printout", json!({"data": data(&png(8, 4)), "name": "dot.png"}));
    assert_eq!(r["total"], 1);
    // A text attachment can't be printed later either.
    run(&mut s, "insert.file", json!({"data": data(b"hello"), "name": "a.txt"}));
    assert!(s.run("attachment.insertPrintout", json!({})).is_err());
}

#[test]
fn links_on_pictures_attachments_and_ink() {
    // The picture, attachment and ink menus' Link… / Edit Link… / Open Link / Copy Link /
    // Remove Link act on the selected objects.
    let (mut s, pic) = with_picture(8, 4);
    assert!(s.run("link.open", json!({})).is_err(), "no link yet");
    run(&mut s, "insert.link", json!({"url": "https://example.org/a"}));
    assert_eq!(image(&s, pic).link.as_deref(), Some("https://example.org/a"), "CM.Picture.HyperlinkCreate");
    assert_eq!(s.current_link().as_deref(), Some("https://example.org/a"));
    assert_eq!(run(&mut s, "link.copy", json!({}))["text"], "https://example.org/a", "CM.Picture.LinkCopyOneNote");
    assert_eq!(run(&mut s, "link.open", json!({}))["open_url"], "https://example.org/a", "CM.Picture.HyperlinkOpen");
    run(&mut s, "link.edit", json!({"url": "https://example.org/b"}));
    assert_eq!(image(&s, pic).link.as_deref(), Some("https://example.org/b"), "CM.Picture.LinkEdit");
    run(&mut s, "link.remove", json!({}));
    assert_eq!(image(&s, pic).link, None, "CM.Picture.LinkRemove");
    // An attachment and a stroke.
    let r = run(&mut s, "insert.file", json!({"data": data(b"hello"), "name": "a.txt"}));
    let att = Id::parse(r["attachment"].as_str().unwrap()).unwrap();
    run(&mut s, "insert.link", json!({"url": "notecraft:page/0000000000000000000000000000000a"}));
    assert!(s.page().unwrap().attachment(att).unwrap().link.is_some(), "CM.FileAttachment.HyperlinkCreate");
    let r = run(&mut s, "draw.stroke", json!({"points": [[10, 10], [40, 40]]}));
    let ink = Id::parse(r["ink"].as_str().unwrap()).unwrap();
    run(&mut s, "object.select", json!({"ids": [ink.to_string()]}));
    run(&mut s, "insert.link", json!({"url": "mailto:me@example.org"}));
    assert_eq!(s.page().unwrap().ink(ink).unwrap().link.as_deref(), Some("mailto:me@example.org"), "CM.InkDrawing.HyperlinkCreate");
    assert!(s.run("insert.link", json!({"url": "bad\u{0}url"})).is_err());
}

#[test]
fn a_screen_clipping_is_a_picture_with_when_it_was_taken() {
    let mut s = sample();
    blank_page(&mut s);
    assert!(s.run("insert.screenClipping", json!({})).is_err(), "the host captures; the engine alone can't");
    let r = run(&mut s, "insert.screenClipping", json!({"data": data(&png(40, 20)), "x": 50.0, "y": 100.0}));
    let id = Id::parse(r["picture"].as_str().unwrap()).unwrap();
    assert_eq!(image(&s, id).pixels, (40, 20), "R.Insert.Images.ScreenCaptureInsert");
    assert!(s.page().unwrap().plain_text().contains("Screen clipping taken: "), "{}", s.page().unwrap().plain_text());
    run(&mut s, "edit.undo", json!({}));
    assert!(s.page().unwrap().objects.is_empty(), "one undo step");
}
