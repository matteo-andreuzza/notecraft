use super::*;
use notecraft_doc::ListFormat;

fn texts(i: &Imported) -> Vec<String> {
    i.blocks
        .iter()
        .map(|b| match b {
            Block::Para(p) => p.text(),
            Block::Table(t) => format!("table {}x{}", t.cells.len(), t.cells.first().map_or(0, Vec::len)),
            Block::Image { alt, .. } => format!("image {alt}"),
        })
        .collect()
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

#[test]
fn markdown_keeps_what_a_note_holds() {
    // parity: FMT.Import.Markdown
    let md = "# Trip\n\nPack **socks** and ~~ties~~.\n\n- one\n  - nested\n- [x] booked\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n![map](map.png) ![web](https://example.org/x.png) ![gone](missing.png)\n\n```\ncode *raw*\n```\n";
    let mut asked = Vec::new();
    let i = markdown(md, &mut |p| {
        asked.push(p.to_string());
        (p == "map.png").then(|| PNG.to_vec())
    });
    assert_eq!(i.title.as_deref(), Some("Trip"));
    assert_eq!(asked, ["map.png", "missing.png"], "web pictures aren't fetched");
    assert_eq!(i.pictures_lost, 2);
    let t = texts(&i);
    assert_eq!(t[0], "Pack socks and ties.");
    assert!(t.contains(&"one".to_string()) && t.contains(&"nested".to_string()));
    assert!(t.contains(&"☑ booked".to_string()), "{t:?}");
    assert!(t.contains(&"table 2x2".to_string()), "{t:?}");
    assert!(t.contains(&"image map".to_string()), "{t:?}");
    assert!(t.iter().any(|x| x.contains("code *raw*")), "{t:?}");
    let Block::Para(p) = &i.blocks[0] else { panic!() };
    assert!(p.runs.iter().any(|r| r.text == "socks" && r.fmt.bold == Some(true)));
    let nested = i.blocks.iter().find_map(|b| match b {
        Block::Para(p) if p.text() == "nested" => Some(p.clone()),
        _ => None,
    });
    assert!(nested.is_some_and(|p| p.level == 1 && matches!(p.list, Some(ListFormat::Bullet(_)))));
    let img = i.blocks.iter().find_map(|b| match b {
        Block::Image { src, .. } => Some(src.clone()),
        _ => None,
    });
    assert!(img.is_some_and(|s| s.starts_with("data:image/png;base64,")));
}

#[test]
fn plain_text_lines_and_tabs() {
    // parity: FMT.Import.Text
    let i = plain_text("first\r\n\tsecond\n\t\t\t\t\t\t\t\t\t\t\t\tdeep\n");
    assert_eq!(texts(&i), ["first", "second", "deep"]);
    let levels: Vec<u8> = i.blocks.iter().map(|b| if let Block::Para(p) = b { p.level } else { 99 }).collect();
    assert_eq!(levels, [0, 1, MAX_LEVEL]);
    assert_eq!(decode_text(b"\xEF\xBB\xBFhi"), "hi");
    assert_eq!(decode_text(b"caf\xe9"), "café", "Latin-1 fallback");
}

#[test]
fn html_files_and_hostile_input() {
    // parity: FMT.Import.Html
    let i = html_document("<html><head><title>x</title><script>alert(1)</script></head><body><h1>Notes</h1><p>Hi</p></body></html>");
    assert_eq!(i.title.as_deref(), Some("Notes"));
    assert_eq!(texts(&i), ["Hi"]);
    // A forged reference in plain HTML is not a picture.
    let i = html_document("<p>a</p><img src=\"nc-res:0\">");
    assert_eq!(texts(&i), ["a"]);
    // Deep nesting and junk never panic.
    let deep = "> ".repeat(5000) + "x" + &"[".repeat(5000) + &"*".repeat(5000);
    let _ = markdown(&deep, &mut |_| None);
    let _ = plain_text(&"\t".repeat(100_000));
}
