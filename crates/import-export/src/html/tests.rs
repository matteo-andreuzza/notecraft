use super::*;
use proptest::prelude::*;

fn texts(blocks: &[Block]) -> Vec<String> {
    blocks
        .iter()
        .map(|b| match b {
            Block::Para(p) => p.text(),
            Block::Table(rows) => rows
                .iter()
                .map(|r| r.iter().map(|c| c.iter().map(Paragraph::text).collect::<Vec<_>>().join("/")).collect::<Vec<_>>().join("|"))
                .collect::<Vec<_>>()
                .join("\n"),
        })
        .collect()
}

fn para(b: &Block) -> &Paragraph {
    match b {
        Block::Para(p) => p,
        Block::Table(_) => panic!("expected a paragraph"),
    }
}

#[test]
fn paragraphs_formatting_and_links() {
    let html = r#"<html><head><style>p{color:red}</style><title>x</title></head><body>
        <p>Hello <b>bold</b> and <i>it<u>alic</u></i> &amp; <a href="https://example.org/?a=1&amp;b=2">a link</a></p>
        <h2>Heading</h2><div>Second&nbsp;line<br>soft</div>
        <p><span style="font-weight:700; color:#C00000">red bold</span> H<sub>2</sub>O x<sup>2</sup> <s>gone</s></p>
        <script>alert(1)</script></body></html>"#;
    let b = parse(html);
    assert_eq!(texts(&b), vec!["Hello bold and italic & a link", "Heading", "Second line\nsoft", "red bold H2O x2 gone"]);
    let p0 = para(&b[0]);
    let fmt_of = |p: &Paragraph, word: &str| p.runs.iter().find(|r| r.text.contains(word)).unwrap().fmt.clone();
    assert_eq!(fmt_of(p0, "bold").bold, Some(true));
    assert_eq!(fmt_of(p0, "alic").underline, Some(true));
    assert_eq!(fmt_of(p0, "alic").italic, Some(true));
    assert_eq!(fmt_of(p0, "link").link.as_deref(), Some("https://example.org/?a=1&b=2"));
    assert_eq!(para(&b[1]).style, ParaStyle::Heading2);
    let p3 = para(&b[3]);
    assert_eq!(fmt_of(p3, "red").bold, Some(true));
    assert_eq!(fmt_of(p3, "red").color, Some(Rgba::rgb(0xC0, 0, 0)));
    assert_eq!(fmt_of(p3, "2").script, Some(Script::Sub));
    assert_eq!(fmt_of(p3, "gone").strike, Some(true));
}

#[test]
fn nested_lists_become_levels() {
    let b = parse("<ul><li>one<ul><li>one.a</li></ul></li><li><p>two</p></li></ul><ol><li>first</li></ol><p>after</p>");
    assert_eq!(texts(&b), vec!["one", "one.a", "two", "first", "after"]);
    let lv: Vec<(u8, bool, bool)> = b
        .iter()
        .map(para)
        .map(|p| (p.level, matches!(p.list, Some(ListFormat::Bullet(_))), matches!(p.list, Some(ListFormat::Number { .. }))))
        .collect();
    assert_eq!(lv, vec![(0, true, false), (1, true, false), (0, true, false), (0, false, true), (0, false, false)]);
}

#[test]
fn tables_are_tables() {
    let b = parse("<p>Before</p><table><tr><th>Item</th><th>Qty</th></tr><tr><td>Pears</td><td><p>4</p><p>ripe</p></td></tr></table><p>After</p>");
    assert_eq!(texts(&b), vec!["Before", "Item|Qty\nPears|4/ripe", "After"]);
    let Block::Table(rows) = &b[1] else { panic!() };
    assert_eq!(rows[0][0][0].runs[0].fmt.bold, Some(true), "header cells are bold");
    // Unclosed cells and rows, a nested table flattened into its cell, a ragged row.
    let b = parse("<table><tr><td>a<td>b<tr><td><table><tr><td>x</td><td>y</td></tr></table></table>");
    assert_eq!(texts(&b), vec!["a|b\nx y"]);
}

#[test]
fn malformed_input_is_text_never_a_panic() {
    for html in [
        "",
        "<",
        "<<>>",
        "a < b > c",
        "<p",
        "</p></div></table>",
        "<!-- unterminated",
        "&#xFFFFFFFF; &#0; &bogus; &",
        "<a href=>x</a>",
        "<td>cell</td>",
        "<li>item",
    ] {
        let _ = parse(html);
    }
    assert_eq!(texts(&parse("a < b and c > d")), vec!["a < b and c > d"]);
    assert_eq!(texts(&parse("<li>item")), vec!["item"]);
    // javascript: links are dropped, the text kept.
    let b = parse(r#"<a href="javascript:alert(1)">click</a>"#);
    assert_eq!(para(&b[0]).runs[0].fmt.link, None);
}

#[test]
fn hostile_sizes_are_bounded() {
    // Deep nesting: tags beyond MAX_DEPTH are ignored, the text is kept.
    let deep = format!("{}deep{}", "<div><b>".repeat(5_000), "</b></div>".repeat(5_000));
    assert_eq!(texts(&parse(&deep)), vec!["deep"]);
    // A huge table stops at the cell limit.
    let row = format!("<tr>{}</tr>", "<td>x</td>".repeat(200));
    let b = parse(&format!("<table>{}</table>", row.repeat(400)));
    let Block::Table(rows) = &b[0] else { panic!() };
    assert!(rows.iter().all(|r| r.len() <= MAX_COLS));
    assert!(rows.iter().map(Vec::len).sum::<usize>() <= MAX_CELLS);
    // Many paragraphs stop at the paragraph limit.
    let many = "<p>p</p>".repeat(MAX_PARAGRAPHS + 10);
    assert_eq!(parse(&many).len(), MAX_PARAGRAPHS);
}

#[test]
fn written_html_reads_back() {
    let src = parse(
        "<h1>Title</h1><p>plain <b>bold</b> <a href=\"https://x.org\">link</a></p><ul><li>a<ul><li>b</li></ul></li></ul><ol><li>n</li></ol>\
         <table><tr><td>1</td><td>2</td></tr></table><p>end &lt;tag&gt;</p>",
    );
    let html = write(&src);
    assert!(html.contains("<table") && html.contains("<ul>") && html.contains("<ol>"), "{html}");
    let back = parse(&html);
    assert_eq!(back, src, "{html}");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn random_markup_never_panics(s in "(<[a-z/!]{0,4}[^<>]{0,8}>?|&[a-z#0-9]{0,6};?|[a-z <>\"'=]{0,6}){0,60}") {
        let _ = parse(&s);
    }
}
