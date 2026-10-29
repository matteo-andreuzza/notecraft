use std::sync::Arc;

use notecraft_doc::geom::Point;
use notecraft_doc::{Attachment, Blob, Id, IdGen, Image, Ink, InkTool, Outline, Page, PageObject, Paragraph, Table, Timestamp};

use super::*;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

fn page(g: &mut IdGen, title: &str) -> Page {
    let mut p = Page::new(g.next_id(), g.next_id(), Timestamp(1_792_007_100_000), "Tester");
    p.title = Paragraph::with_text(p.title.id, title);
    p
}

fn outline(g: &mut IdGen, y: f32, texts: &[&str]) -> PageObject {
    let mut o = Outline::new(g.next_id(), Point::new(36.0, y), g.next_id());
    o.paragraphs = texts.iter().map(|t| Paragraph::with_text(g.next_id(), t)).collect();
    PageObject::Outline(o)
}

/// A page with, out of order on the canvas: text at the bottom, a picture at the top, a table,
/// an attachment, a stroke.
fn sample(g: &mut IdGen) -> Page {
    let mut p = page(g, "Trip <plan>");
    p.objects.push(outline(g, 400.0, &["Last"]));
    let blob = Blob::new(g.next_id(), Arc::from(PNG));
    let mut im = Image::new(g.next_id(), Point::new(36.0, 80.0), blob, (4, 2), 40.0, 20.0);
    im.alt_text = "A \"map\"".into();
    p.objects.push(PageObject::Image(im));
    let mut t = Table::new(g.next_id(), Point::new(36.0, 200.0), 2, 2, &mut || g.next_id());
    if let Some(c) = t.cell_mut(0, 0) {
        c.paragraphs = vec![Paragraph::with_text(Id::NIL, "A1")];
    }
    p.objects.push(PageObject::Table(t));
    p.objects.push(PageObject::Attachment(Attachment {
        id: g.next_id(),
        pos: Point::new(36.0, 300.0),
        name: "notes v2.txt".into(),
        blob: Blob::new(g.next_id(), Arc::from(&b"hello"[..])),
        alt_text: String::new(),
        link: None,
    }));
    p.objects.push(PageObject::Ink(Ink::new(
        g.next_id(),
        &[(Point::new(0.0, 0.0), 0.5), (Point::new(9.0, 9.0), 0.5)],
        notecraft_doc::color::Rgba::rgb(0, 0, 0),
        2.0,
        InkTool::Pen,
    )));
    p
}

#[test]
fn collect_reads_top_to_bottom_and_reports_ink() {
    // parity: FMT.Export.Html
    let mut g = IdGen::new(7);
    let p = sample(&mut g);
    let e = collect(&[&p], 0);
    assert_eq!(e.pages.len(), 1);
    let kinds: Vec<&str> = e.pages[0]
        .items
        .iter()
        .map(|i| match i {
            Item::Paragraphs(_) => "text",
            Item::Table(_) => "table",
            Item::Picture { .. } => "picture",
            Item::File { .. } => "file",
        })
        .collect();
    assert_eq!(kinds, ["picture", "table", "file", "text"]);
    assert_eq!(e.report, Report { pages: 1, pictures: 1, files: 1, tables: 1, ink_lost: 1, files_lost: 0 });
    let names: Vec<&str> = e.resources.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["image001.png", "notes v2.txt"]);
    assert_eq!(e.resources[0].mime, "image/png");
    assert!(e.pages[0].date.contains("2026"), "{}", e.pages[0].date);
}

#[test]
fn a_blob_on_two_pages_is_one_file_and_names_never_collide() {
    let mut g = IdGen::new(8);
    let a = sample(&mut g);
    let mut b = a.clone();
    b.id = g.next_id();
    // Same name, other bytes: a second file.
    if let Some(PageObject::Attachment(at)) = b.objects.iter_mut().find(|o| matches!(o, PageObject::Attachment(_))) {
        at.blob = Blob::new(g.next_id(), Arc::from(&b"other"[..]));
    }
    let e = collect(&[&a, &b], 0);
    let names: Vec<&str> = e.resources.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["image001.png", "notes v2.txt", "notes v2 (2).txt"]);
    assert_eq!(e.report.pictures, 1);
    assert_eq!(e.report.files, 2);
}

#[test]
fn lost_bytes_and_hostile_names_are_handled() {
    let mut g = IdGen::new(9);
    let mut p = page(&mut g, "");
    p.objects.push(PageObject::Attachment(Attachment {
        id: g.next_id(),
        pos: Point::new(0.0, 0.0),
        name: "../../etc/passwd".into(),
        blob: Blob::new(g.next_id(), Arc::from(&b"x"[..])),
        alt_text: String::new(),
        link: None,
    }));
    let mut lost = Blob::new(g.next_id(), Arc::from(&b""[..]));
    lost.len = 10;
    p.objects.push(PageObject::Image(Image::new(g.next_id(), Point::new(0.0, 9.0), lost, (1, 1), 1.0, 1.0)));
    let e = collect(&[&p], 0);
    assert_eq!(e.resources[0].name, "_.._etc_passwd");
    assert_eq!(e.report.files_lost, 1);
    assert_eq!(e.pages[0].title, notecraft_doc::page::UNTITLED);
}

#[test]
fn html_document_is_escaped_and_links_files() {
    let mut g = IdGen::new(10);
    let p = sample(&mut g);
    let e = collect(&[&p], 0);
    let html = html_document(&e, "Trip <plan>", Links::Inline);
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<title>Trip &lt;plan&gt;</title>"));
    assert!(html.contains("<img src=\"data:image/png;base64,"), "{html}");
    assert!(html.contains("alt=\"A &quot;map&quot;\" style=\"width:40pt;height:20pt\""), "{html}");
    assert!(html.contains("<td><p>A1</p></td>"), "{html}");
    let rel = html_document(&e, "x", Links::Relative(Some("Trip files")));
    assert!(rel.contains("href=\"Trip%20files/notes%20v2.txt\""), "{rel}");
    // The page's own content reads back through the clipboard reader.
    let back = html::parse(&html);
    assert!(back.iter().any(|b| matches!(b, Block::Para(p) if p.text() == "Last")));
}

#[test]
fn mht_is_multipart_related_with_every_file() {
    // parity: BS.Export.FormatMht
    let mut g = IdGen::new(11);
    let p = sample(&mut g);
    let e = collect(&[&p], 0);
    let bytes = mht(&e, "Trip ✈");
    let s = String::from_utf8(bytes).unwrap();
    assert!(s.starts_with("From: <Saved by NoteCraft>\r\nSubject: =?utf-8?B?"), "{s}");
    assert!(s.contains("Content-Type: multipart/related;"));
    assert_eq!(s.matches("------=_NextPart_NoteCraft_0000\r\n").count(), 3, "html + picture + file");
    assert!(s.ends_with("------=_NextPart_NoteCraft_0000--\r\n"));
    assert!(s.contains("Content-Location: file:///notecraft-export/image001.png"));
    assert!(s.contains("Content-Location: file:///notecraft-export/notes%20v2.txt"));
    assert!(s.lines().all(|l| l.len() <= 998), "RFC 5322 line limit");
    // The HTML part decodes and names the picture relatively.
    let start = s.find("index.html\r\n\r\n").unwrap() + "index.html\r\n\r\n".len();
    let end = s[start..].find("\r\n\r\n").unwrap() + start;
    let html = String::from_utf8(b64::decode(&s[start..end]).unwrap()).unwrap();
    assert!(html.contains("<img src=\"image001.png\""), "{html}");
}
