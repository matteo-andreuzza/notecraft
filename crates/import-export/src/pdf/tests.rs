use std::sync::Arc;

use notecraft_codecs::pdf::{PdfLimits, page_sizes, render};
use notecraft_doc::geom::Point;
use notecraft_doc::{Attachment, Blob, Id, IdGen, Image, Ink, InkTool, Outline, Page, PageObject, Paragraph, Table, Timestamp};

use super::*;

/// The machine's page fonts, or `None` (then the test has nothing to check).
fn fonts() -> Option<notecraft_fonts::PageFonts> {
    let f = notecraft_fonts::PageFonts::find();
    f.regular.is_some().then_some(f)
}

fn page(g: &mut IdGen, title: &str) -> Page {
    let mut p = Page::new(g.next_id(), g.next_id(), Timestamp(1_792_007_100_000), "Tester");
    p.title = Paragraph::with_text(p.title.id, title);
    p
}

fn outline(g: &mut IdGen, x: f32, y: f32, texts: &[&str]) -> PageObject {
    let mut o = Outline::new(g.next_id(), Point::new(x, y), g.next_id());
    o.paragraphs = texts.iter().map(|t| Paragraph::with_text(g.next_id(), t)).collect();
    PageObject::Outline(o)
}

fn opts() -> PdfOptions {
    PdfOptions { title: "Trip".into(), author: "Tester".into(), utc_offset_min: 0, fit_content: false }
}

/// Dark pixels in the rectangle (fractions of the sheet) of a rendered sheet.
fn ink_in(px: &notecraft_codecs::Rgba8, x0: f32, y0: f32, x1: f32, y1: f32) -> usize {
    let (w, h) = (px.width as f32, px.height as f32);
    let mut n = 0;
    for y in (y0 * h) as u32..(y1 * h) as u32 {
        for x in (x0 * w) as u32..(x1 * w) as u32 {
            let i = ((y * px.width + x) * 4) as usize;
            if px.pixels.get(i..i + 3).is_some_and(|c| c.iter().all(|v| *v < 128)) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn a_page_becomes_a_letter_sheet_with_its_text() {
    // parity: FMT.Export.Pdf
    let Some(f) = fonts() else { return };
    let mut g = IdGen::new(1);
    let mut p = page(&mut g, "Trip plan");
    p.objects.push(outline(&mut g, 36.0, 100.0, &["Pack the bags and check the tickets twice."]));
    let (bytes, report) = write(&[&p], &f, &opts()).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    assert_eq!((report.pages, report.sheets), (1, 1));
    assert_eq!(page_sizes(&bytes, &PdfLimits::PRINTOUT).unwrap(), vec![(612.0, 792.0)]);
    let sheets = render(&bytes, &PdfLimits::PRINTOUT).unwrap();
    let px = &sheets[0].0;
    // The title near the top left, the text below it, nothing at the bottom.
    assert!(ink_in(px, 0.0, 0.0, 0.5, 0.12) > 50, "title drawn");
    assert!(ink_in(px, 0.0, 0.12, 0.9, 0.25) > 50, "text drawn");
    assert_eq!(ink_in(px, 0.0, 0.6, 1.0, 1.0), 0);
}

#[test]
fn every_object_kind_draws_and_hostile_values_are_clamped() {
    let Some(f) = fonts() else { return };
    let p = all_objects();
    let (bytes, report) = write(&[&p], &f, &opts()).unwrap();
    assert_eq!(report.pictures_lost, 1, "the junk picture is a grey box");
    assert_eq!(report.files, 1, "the attachment rides along");
    let sheets = render(&bytes, &PdfLimits::PRINTOUT).unwrap();
    assert_eq!(sheets.len(), 1);
    let s = String::from_utf8_lossy(&bytes);
    assert!(s.contains("https://example.org"), "the link annotation");
}

/// A page with every kind of object, some with hostile values.
fn all_objects() -> Page {
    let mut g = IdGen::new(2);
    let mut p = page(&mut g, "All");
    p.setup.rule_lines = notecraft_doc::RuleLines::College;
    p.setup.color = Some(Rgba::rgb(0xFF, 0xF8, 0xE0));
    let mut ol = Outline::new(g.next_id(), Point::new(36.0, 100.0), g.next_id());
    let mut a = Paragraph::with_text(g.next_id(), "bullet");
    a.list = Some(ListFormat::Bullet(notecraft_doc::Bullet::Disc));
    let mut b = Paragraph::with_text(g.next_id(), "numbered");
    b.list = Some(ListFormat::Number { style: notecraft_doc::Numbering::Decimal, restart: None });
    b.level = 1;
    let mut c = Paragraph::with_text(g.next_id(), "link\tand\nbreak");
    c.runs.iter_mut().for_each(|r| {
        r.fmt.link = Some("https://example.org".into());
        r.fmt.italic = Some(true);
        r.fmt.bold = Some(true);
    });
    c.tags = vec![notecraft_doc::TagUse { key: "t".into(), name: "To Do".into(), symbol: TagSymbol::Checkbox, checked: true }];
    ol.paragraphs = vec![a, b, c];
    ol.width = OutlineWidth::Fixed(f32::NAN);
    p.objects.push(PageObject::Outline(ol));
    let mut t = Table::new(g.next_id(), Point::new(36.0, 220.0), 2, 2, &mut || g.next_id());
    t.rows[0].cells[0].shading = Some(Rgba::rgb(200, 220, 255));
    t.rows[0].cells[0].col_span = 2;
    t.columns[1] = f32::INFINITY;
    p.objects.push(PageObject::Table(t));
    let png = notecraft_codecs::encode_png(&notecraft_codecs::Rgba8::new(4, 2, vec![30; 32]).unwrap(), &notecraft_codecs::Limits::PICTURE).unwrap();
    let mut im = Image::new(g.next_id(), Point::new(300.0, 300.0), Blob::new(g.next_id(), Arc::from(png)), (4, 2), 80.0, 40.0);
    im.quarter_turns = 1;
    im.flip_h = true;
    p.objects.push(PageObject::Image(im));
    p.objects.push(PageObject::Image(Image::new(
        g.next_id(),
        Point::new(36.0, 360.0),
        Blob::new(g.next_id(), Arc::from(&b"junk"[..])),
        (1, 1),
        20.0,
        20.0,
    )));
    p.objects.push(PageObject::Attachment(Attachment {
        id: g.next_id(),
        pos: Point::new(36.0, 420.0),
        name: "notes.txt".into(),
        blob: Blob::new(g.next_id(), Arc::from(&b"hello"[..])),
        alt_text: String::new(),
        link: None,
    }));
    p.objects.push(PageObject::Ink(Ink::new(
        g.next_id(),
        &[(Point::new(100.0, 500.0), 0.5), (Point::new(200.0, 540.0), 0.9)],
        Rgba::rgb(200, 0, 0),
        3.0,
        InkTool::Highlighter,
    )));
    p
}

#[test]
fn a_page_becomes_an_svg_picture_with_every_object() {
    // parity: FMT.Export.Images
    let Some(f) = fonts() else { return };
    let mut p = all_objects();
    p.title = Paragraph::with_text(p.title.id, "A & <B>");
    let o = PdfOptions { title: "A & <B>".into(), fit_content: true, ..opts() };
    let (svg, report) = svg::write(&p, &f, &o).unwrap();
    assert_eq!((report.pages, report.sheets), (1, 1));
    assert!(svg.starts_with("<?xml") && svg.trim_end().ends_with("</svg>"));
    assert!(svg.contains("<title>A &amp; &lt;B&gt;</title>"), "escaped title");
    assert!(!svg.contains("NaN") && !svg.contains("inf"), "hostile numbers never reach the file");
    assert!(svg.contains("fill=\"#fff8e0\""), "the page colour");
    assert!(svg.contains("stroke=\"#e69a9a\""), "ruled paper's margin line");
    assert!(svg.contains("<defs>") && svg.contains("<use xlink:href=\"#g"), "glyph outlines, used");
    assert_eq!(svg.matches("data:image/png;base64,").count(), 1, "the readable picture embedded once, the junk one left out");
    assert!(svg.contains("<a xlink:href=\"https://example.org\">"), "the link");
    assert!(svg.contains("stroke-linecap=\"round\"") && svg.contains("stroke-opacity="), "ink, see-through highlighter");
    // Every element opened is closed: tags balance.
    for tag in ["svg", "g", "defs", "a"] {
        assert_eq!(
            svg.matches(&format!("<{tag} ")).count() + svg.matches(&format!("<{tag}>")).count(),
            svg.matches(&format!("</{tag}>")).count(),
            "<{tag}>"
        );
    }
    // An empty page is still a picture.
    let mut g = IdGen::new(9);
    let (svg, _) = svg::write(&page(&mut g, ""), &f, &o).unwrap();
    assert!(svg.contains("viewBox=\"0 0 "));
    assert!(svg::write(&p, &notecraft_fonts::PageFonts::default(), &o).unwrap_err().contains("no font"));
}

#[test]
fn a_tall_page_continues_on_more_sheets() {
    let Some(f) = fonts() else { return };
    let mut g = IdGen::new(3);
    let mut p = page(&mut g, "Long");
    let lines: Vec<String> = (0..120).map(|i| format!("Line {i}")).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    p.objects.push(outline(&mut g, 36.0, 100.0, &refs));
    let (bytes, report) = write(&[&p, &page(&mut g, "Short")], &f, &opts()).unwrap();
    assert_eq!(report.pages, 2);
    assert!(report.sheets >= 4, "{report:?}");
    assert_eq!(page_sizes(&bytes, &PdfLimits::PRINTOUT).unwrap().len(), report.sheets);
}

#[test]
fn sheet_breaks_fall_between_lines() {
    // Lines 14 pt tall from 0; a sheet holds 100 pt: the break moves up to a line's top.
    let bands: Vec<(f32, f32)> = (0..30).map(|i| (i as f32 * 14.0, i as f32 * 14.0 + 14.0)).collect();
    let starts = sheet_breaks(0.0, 420.0, 100.0, 100.0, &bands);
    assert_eq!(starts[..3], [0.0, 98.0, 196.0]);
    for s in &starts {
        assert!(bands.iter().all(|(a, b)| !(a < s && b > s)), "{s} cuts a line");
    }
    // A band taller than a sheet can't be kept whole: it is cut.
    let starts = sheet_breaks(0.0, 300.0, 100.0, 100.0, &[(10.0, 290.0)]);
    assert_eq!(starts, [0.0, 100.0, 200.0]);
    // Hostile extents end.
    assert!(sheet_breaks(0.0, f32::MAX, 1.0, 1.0, &[]).len() <= MAX_SHEETS);
}

#[test]
fn without_fonts_the_export_says_so() {
    let mut g = IdGen::new(4);
    let p = page(&mut g, "x");
    let e = write(&[&p], &notecraft_fonts::PageFonts::default(), &opts()).unwrap_err();
    assert!(e.contains("no font"), "{e}");
    let Some(f) = fonts() else { return };
    assert!(write(&[], &f, &opts()).is_err(), "no pages");
    let _ = Id::NIL;
}

#[test]
fn fit_content_is_one_sheet_the_size_of_the_page() {
    let Some(f) = fonts() else { return };
    let mut g = IdGen::new(5);
    let mut p = page(&mut g, "Wide");
    let lines: Vec<String> = (0..80).map(|i| format!("Line {i}")).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    p.objects.push(outline(&mut g, 36.0, 100.0, &refs));
    p.objects.push(outline(&mut g, 900.0, 100.0, &["far right"]));
    let o = PdfOptions { fit_content: true, ..opts() };
    let (bytes, report) = write(&[&p], &f, &o).unwrap();
    assert_eq!(report.sheets, 1);
    let sizes = page_sizes(&bytes, &PdfLimits::PRINTOUT).unwrap();
    assert!(sizes[0].0 > 900.0 && sizes[0].1 > 1000.0, "{sizes:?}");
}
