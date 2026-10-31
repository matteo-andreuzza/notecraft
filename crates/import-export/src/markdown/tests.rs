use notecraft_doc::{Bullet, CharFormat, Id, ListFormat, Numbering, ParaStyle, Paragraph, Run, TagSymbol, TagUse};

use super::*;
use crate::export::{ExportPage, Item, Report, Resource};

fn para(text: &str) -> Paragraph {
    Paragraph::with_text(Id::NIL, text)
}

fn doc(items: Vec<Item>) -> Export {
    Export {
        pages: vec![ExportPage { title: "Notes".into(), date: "Thursday, 29 October 2026   10:15".into(), level: 0, items }],
        resources: Vec::new(),
        report: Report::default(),
    }
}

fn body(items: Vec<Item>) -> String {
    let md = write(&doc(items), "Notes_files");
    md.split_once("10:15*\n\n").map(|(_, b)| b.to_string()).unwrap_or(md)
}

#[test]
fn inline_formats_and_escapes() {
    // parity: FMT.Export.Markdown
    let mut p = para("");
    let b = CharFormat { bold: Some(true), ..Default::default() };
    let i = CharFormat { italic: Some(true), ..Default::default() };
    let l = CharFormat { link: Some("https://example.org/a b".into()), ..Default::default() };
    p.runs = vec![
        Run::new("Plain *star* ", CharFormat::default()),
        Run::new("bold ", b.clone()),
        Run::new("too", b),
        Run::new(" it", i),
        Run::new(" link", l),
    ];
    assert_eq!(body(vec![Item::Paragraphs(vec![p])]), "Plain \\*star\\* **bold too** *it* [link](<https://example.org/a b>)\n");
    // What would start a block is escaped at a line start only.
    let md = body(vec![Item::Paragraphs(vec![para("# not a heading"), para("1. not a list"), para("a - b # c")])]);
    assert_eq!(md, "\\# not a heading\n\n1\\. not a list\n\na - b # c\n");
}

#[test]
fn headings_quotes_code_and_breaks() {
    let mut h = para("Title\nmore");
    h.style = ParaStyle::Heading2;
    let mut q = para("said\nthis");
    q.style = ParaStyle::Quote;
    let mut c1 = para("let x = 1;");
    c1.style = ParaStyle::Code;
    let mut c2 = para("*raw*");
    c2.style = ParaStyle::Code;
    let md = body(vec![Item::Paragraphs(vec![h, q, c1, c2, para("after")])]);
    assert_eq!(md, "## Title more\n\n> said\\\n> this\n\n```\nlet x = 1;\n*raw*\n```\n\nafter\n");
}

#[test]
fn nested_lists_and_todo_tags() {
    let mut a = para("one");
    a.list = Some(ListFormat::Bullet(Bullet::Disc));
    let mut b = para("two");
    b.list = Some(ListFormat::Number { style: Numbering::Decimal, restart: Some(3) });
    b.level = 1;
    let mut c = para("three");
    c.list = Some(ListFormat::Bullet(Bullet::Disc));
    let mut todo = para("buy milk");
    todo.tags = vec![TagUse { key: "todo".into(), name: "To Do".into(), symbol: TagSymbol::Checkbox, checked: true }];
    let md = body(vec![Item::Paragraphs(vec![para("intro"), a, b, c, todo])]);
    assert_eq!(md, "intro\n\n- one\n    3. two\n- three\n- [x] buy milk\n");
}

#[test]
fn tables_pictures_and_files() {
    let cells = vec![vec![vec![para("H|1")], vec![para("H2")]], vec![vec![para("a"), para("b")]]];
    let mut e = doc(vec![
        Item::Table { cells: TableBlock::plain(cells), widths: vec![72.0; 2], borders: true, shading: Vec::new() },
        Item::Picture { res: 0, alt: "map [1]".into(), width: 10.0, height: 10.0 },
        Item::File { res: 1, name: "plan.pdf".into() },
    ]);
    e.resources = vec![
        Resource { name: "image001.png".into(), mime: "image/png".into(), bytes: std::sync::Arc::from(&b"x"[..]) },
        Resource { name: "plan.pdf".into(), mime: "application/octet-stream".into(), bytes: std::sync::Arc::from(&b"y"[..]) },
    ];
    let md = write(&e, "Notes files");
    assert!(md.contains("| H\\|1 | H2 |\n| --- | --- |\n| a<br>b |  |\n"), "{md}");
    assert!(md.contains("![map \\[1\\]](Notes%20files/image001.png)\n"), "{md}");
    assert!(md.contains("[plan.pdf](Notes%20files/plan.pdf)\n"), "{md}");
}

#[test]
fn pages_are_separated_by_rules() {
    let mut e = doc(vec![Item::Paragraphs(vec![para("x")])]);
    e.pages.push(ExportPage { title: "Second".into(), date: "d".into(), level: 1, items: Vec::new() });
    let md = write(&e, "f");
    assert!(md.starts_with("# Notes\n\n*Thursday, 29 October 2026   10:15*\n\nx\n\n---\n\n# Second\n"), "{md}");
}
