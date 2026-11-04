use std::io::Read;
use std::sync::Arc;

use notecraft_doc::{Bullet, CharFormat, Id, ListFormat, Numbering, ParaStyle, Paragraph, Run, TagSymbol, TagUse};

use super::*;
use crate::export::{ExportPage, Report, Resource};
use crate::html::Span;

fn para(text: &str) -> Paragraph {
    Paragraph::with_text(Id::NIL, text)
}

fn export(items: Vec<Item>, resources: Vec<Resource>) -> Export {
    Export {
        pages: vec![ExportPage { title: "Plan & <notes>".into(), date: "Thursday".into(), level: 0, items }],
        resources,
        report: Report::default(),
    }
}

/// The package's parts by name.
fn parts(bytes: &[u8]) -> HashMap<String, Vec<u8>> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut out = HashMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let mut b = Vec::new();
        f.read_to_end(&mut b).unwrap();
        out.insert(f.name().unwrap().to_string(), b);
    }
    out
}

/// Every element opened is closed in order (a small well-formedness check).
fn balanced(xml: &str) {
    let mut stack: Vec<String> = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find('<') {
        rest = &rest[i + 1..];
        let end = rest.find('>').unwrap();
        let tag = &rest[..end];
        rest = &rest[end + 1..];
        if tag.starts_with('?') || tag.ends_with('/') {
            continue;
        }
        let name = tag.split_whitespace().next().unwrap();
        if let Some(n) = name.strip_prefix('/') {
            assert_eq!(stack.pop().as_deref(), Some(n), "closing {n}");
        } else {
            stack.push(name.to_string());
        }
    }
    assert!(stack.is_empty(), "unclosed: {stack:?}");
}

fn document(e: &Export) -> (String, HashMap<String, Vec<u8>>, DocxReport) {
    let (bytes, report) = write(e, "Plan").unwrap();
    let p = parts(&bytes);
    let doc = String::from_utf8(p["word/document.xml"].clone()).unwrap();
    for (name, b) in &p {
        if name.ends_with(".xml") || name.ends_with(".rels") {
            balanced(&String::from_utf8(b.clone()).unwrap());
        }
    }
    (doc, p, report)
}

#[test]
fn the_package_has_every_part_and_escapes_text() {
    // parity: FMT.Export.Docx
    let (doc, p, _) = document(&export(vec![Item::Paragraphs(vec![para("a < b\tc\nd")])], Vec::new()));
    for part in ["[Content_Types].xml", "_rels/.rels", "docProps/core.xml", "word/styles.xml", "word/numbering.xml", "word/_rels/document.xml.rels"] {
        assert!(p.contains_key(part), "{part}");
    }
    assert!(doc.contains("<w:pStyle w:val=\"Title\"/></w:pPr><w:r><w:t xml:space=\"preserve\">Plan &amp; &lt;notes&gt;</w:t>"), "{doc}");
    assert!(doc.contains(">a &lt; b</w:t><w:tab/><w:t xml:space=\"preserve\">c</w:t><w:br/>"), "{doc}");
    let styles = String::from_utf8(p["word/styles.xml"].clone()).unwrap();
    assert!(styles.contains("w:styleId=\"Heading1\"><w:name w:val=\"heading 1\"/>"), "{styles}");
}

#[test]
fn formats_links_styles_and_tags() {
    let mut p = para("");
    let f = CharFormat {
        bold: Some(true),
        size: Some(14.0),
        color: Some(Rgba::rgb(255, 0, 0)),
        script: Some(Script::Super),
        link: Some("https://example.org/?a=1&b=2".into()),
        ..Default::default()
    };
    p.runs = vec![Run::new("hi", f)];
    p.style = ParaStyle::Heading2;
    p.align = Align::Center;
    let mut todo = para("milk");
    todo.tags = vec![TagUse { key: "t".into(), name: "To Do".into(), symbol: TagSymbol::Checkbox, checked: false }];
    let (doc, parts, _) = document(&export(vec![Item::Paragraphs(vec![p, todo])], Vec::new()));
    assert!(doc.contains("<w:pPr><w:pStyle w:val=\"Heading2\"/><w:jc w:val=\"center\"/></w:pPr>"), "{doc}");
    // rPr children in schema order: rStyle, b, color, sz, vertAlign.
    assert!(
        doc.contains("<w:hyperlink r:id=\"rId3\" w:history=\"1\"><w:r><w:rPr><w:rStyle w:val=\"Hyperlink\"/><w:b/><w:color w:val=\"FF0000\"/><w:sz w:val=\"28\"/><w:szCs w:val=\"28\"/><w:vertAlign w:val=\"superscript\"/></w:rPr>"),
        "{doc}"
    );
    assert!(doc.contains(">☐ </w:t>"), "{doc}");
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap();
    assert!(rels.contains("Id=\"rId3\"") && rels.contains("Target=\"https://example.org/?a=1&amp;b=2\" TargetMode=\"External\""), "{rels}");
}

#[test]
fn lists_get_word_numbering() {
    let mut a = para("one");
    a.list = Some(ListFormat::Number { style: Numbering::Decimal, restart: None });
    let mut b = para("sub");
    b.list = Some(ListFormat::Bullet(Bullet::Circle));
    b.level = 1;
    let mut c = para("two");
    c.list = Some(ListFormat::Number { style: Numbering::Decimal, restart: None });
    let mut d = para("five");
    d.list = Some(ListFormat::Number { style: Numbering::Decimal, restart: Some(5) });
    let (doc, parts, _) = document(&export(vec![Item::Paragraphs(vec![a, b, c, para("break"), d])], Vec::new()));
    // "one" and "two" share a list; "five" after a plain paragraph starts another, at 5.
    assert_eq!(doc.matches("<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"101\"/></w:numPr>").count(), 1 + 1, "{doc}");
    assert!(doc.contains("<w:numPr><w:ilvl w:val=\"1\"/><w:numId w:val=\"2\"/></w:numPr>"), "the circle bullet: {doc}");
    assert!(doc.contains("<w:numId w:val=\"102\"/>"), "{doc}");
    let numbering = String::from_utf8(parts["word/numbering.xml"].clone()).unwrap();
    assert!(
        numbering.contains("<w:num w:numId=\"102\"><w:abstractNumId w:val=\"8\"/><w:lvlOverride w:ilvl=\"0\"><w:startOverride w:val=\"5\"/>"),
        "{numbering}"
    );
    // Every abstract list comes before the first list instance.
    assert!(numbering.rfind("<w:abstractNum ").unwrap() < numbering.find("<w:num ").unwrap());
}

#[test]
fn tables_with_widths_shading_and_merged_cells() {
    let cells = vec![
        vec![vec![para("wide")], vec![], vec![para("c")]],
        vec![vec![para("tall")], vec![para("x")], vec![para("y")]],
        vec![vec![], vec![para("p")], vec![para("q")]],
    ];
    let t = TableBlock { cells, spans: vec![Span { row: 0, col: 0, rows: 1, cols: 2 }, Span { row: 1, col: 0, rows: 2, cols: 1 }] };
    let shading = vec![vec![None, None, Some(Rgba::rgb(0xff, 0xee, 0x00))]];
    let item = Item::Table { cells: t, widths: vec![50.0, 60.0, 70.0], borders: false, shading };
    let (doc, _, _) = document(&export(vec![item], Vec::new()));
    assert!(doc.contains("<w:gridCol w:w=\"1000\"/><w:gridCol w:w=\"1200\"/><w:gridCol w:w=\"1400\"/>"), "{doc}");
    assert!(doc.contains("<w:tcW w:w=\"2200\" w:type=\"dxa\"/><w:gridSpan w:val=\"2\"/>"), "{doc}");
    assert!(doc.contains("<w:vMerge w:val=\"restart\"/>") && doc.contains("<w:vMerge/></w:tcPr><w:p/>"), "{doc}");
    assert!(doc.contains("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"FFEE00\"/>"), "{doc}");
    assert!(doc.contains("<w:top w:val=\"nil\"/>"), "hidden borders");
    assert_eq!(doc.matches("<w:tr>").count(), 3);
    // Row one has two cells (the merged pair and "c"), the others three.
    let rows: Vec<usize> = doc.split("<w:tr>").skip(1).map(|r| r.matches("<w:tc>").count()).collect();
    assert_eq!(rows, [2, 3, 3]);
}

#[test]
fn pictures_are_embedded_and_files_named() {
    let png = notecraft_codecs::encode_png(&notecraft_codecs::Rgba8::new(2, 1, vec![9; 8]).unwrap(), &notecraft_codecs::Limits::PICTURE).unwrap();
    let res = vec![
        Resource { name: "image001.png".into(), mime: "image/png".into(), bytes: Arc::from(png) },
        Resource { name: "broken.bin".into(), mime: "application/octet-stream".into(), bytes: Arc::from(&b"nope"[..]) },
        Resource { name: "a.pdf".into(), mime: "application/octet-stream".into(), bytes: Arc::from(&b"%PDF"[..]) },
    ];
    let items = vec![
        Item::Picture { res: 0, alt: "dog \"rex\"".into(), width: 936.0, height: 100.0 },
        Item::Picture { res: 0, alt: String::new(), width: 10.0, height: 10.0 },
        Item::Picture { res: 1, alt: String::new(), width: 10.0, height: 10.0 },
        Item::File { res: 2, name: "a.pdf".into() },
    ];
    let (doc, parts, report) = document(&export(items, res));
    assert_eq!(report, DocxReport { files_dropped: 1, pictures_dropped: 1 });
    // Scaled to the text width (468 pt), height in proportion.
    assert!(doc.contains("<wp:extent cx=\"5943600\" cy=\"635000\"/>"), "{doc}");
    assert!(doc.contains("descr=\"dog &quot;rex&quot;\""));
    assert_eq!(parts.keys().filter(|k| k.starts_with("word/media/")).count(), 1, "one picture used twice is stored once");
    assert!(parts.contains_key("word/media/image1.png"));
    assert!(doc.contains("Attached file: a.pdf"));
}

#[test]
fn pages_are_separated_by_page_breaks() {
    let mut e = export(vec![], Vec::new());
    e.pages.push(ExportPage { title: "Two".into(), date: "d".into(), level: 0, items: Vec::new() });
    let (doc, _, _) = document(&e);
    assert_eq!(doc.matches("<w:br w:type=\"page\"/>").count(), 1);
}
