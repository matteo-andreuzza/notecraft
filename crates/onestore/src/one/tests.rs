use notecraft_doc::id::IdGen;
use notecraft_doc::page::{OutlineWidth, PageObject};
use notecraft_doc::time::Timestamp;

use super::map::LINE;
use super::*;
use crate::error::OneError;
use crate::writer::section::{FixtureOutline, FixturePage, write};

fn import(file: &[u8]) -> Import {
    read_section(file, "Imported", &mut IdGen::new(7), Timestamp(1_000)).unwrap()
}

fn outline(paragraphs: &[(u8, &str)]) -> FixtureOutline {
    FixtureOutline { x: 2.0, y: 4.0, paragraphs: paragraphs.iter().map(|(l, t)| (*l, t.to_string())).collect(), ..FixtureOutline::default() }
}

fn texts(page: &notecraft_doc::page::Page) -> Vec<(u8, String)> {
    page.objects.iter().filter_map(PageObject::as_outline).flat_map(|o| o.paragraphs.iter().map(|p| (p.level, p.text()))).collect()
}

#[test]
fn pages_titles_and_levels_round_trip() {
    // parity: FMT.One.ReadPages
    let pages = vec![
        FixturePage { title: "Groceries".into(), level: 1, ..FixturePage::default() },
        FixturePage { title: "Monday".into(), level: 2, ..FixturePage::default() },
        FixturePage { title: "Tuesday".into(), level: 3, modified: 1_000, ..FixturePage::default() },
    ];
    let i = import(&write(&pages));
    let s = &i.section;
    assert_eq!(s.name, "Imported");
    let got: Vec<_> = s.pages.iter().map(|p| (p.title.text(), p.level)).collect();
    assert_eq!(got, vec![("Groceries".to_string(), 0), ("Monday".to_string(), 1), ("Tuesday".to_string(), 2)]);
    assert_eq!(s.pages[2].modified, Timestamp(315_532_800_000 + 1_000_000));
    assert_eq!(i.report.kept.get("pages"), Some(&3));
    assert!(i.report.lost.is_empty() && i.report.warnings.is_empty(), "{:?}", i.report);
}

#[test]
fn outlines_keep_text_levels_position_and_width() {
    let mut fixed = outline(&[(0, "Notes")]);
    fixed.width = Some(10.0);
    let pages = vec![FixturePage {
        title: "Plan".into(),
        outlines: vec![outline(&[(0, "Trip"), (1, "Tickets"), (2, "Train"), (1, "Hotel"), (0, "Budget")]), fixed],
        ..FixturePage::default()
    }];
    let i = import(&write(&pages));
    let page = &i.section.pages[0];
    assert_eq!(
        texts(page),
        [(0, "Trip"), (1, "Tickets"), (2, "Train"), (1, "Hotel"), (0, "Budget"), (0, "Notes")].map(|(l, t)| (l, t.to_string())).to_vec()
    );
    let outlines: Vec<_> = page.objects.iter().filter_map(PageObject::as_outline).collect();
    assert_eq!(outlines.len(), 2);
    assert_eq!((outlines[0].pos.x, outlines[0].pos.y), (72.0, 144.0), "half inches become points");
    assert_eq!(outlines[0].width, OutlineWidth::Auto);
    assert_eq!(outlines[1].width, OutlineWidth::Fixed(360.0));
    assert_eq!(i.report.kept.get("paragraphs"), Some(&(6 + 1)), "FMT.One.ReadOutlines");
}

#[test]
fn unicode_and_extended_ascii_text() {
    let mut ascii = outline(&[(0, "café")]);
    ascii.ascii = true;
    let pages =
        vec![FixturePage { title: "Ünïcödé 📝".into(), outlines: vec![outline(&[(0, "日本語 — ok")]), ascii], ..FixturePage::default() }];
    let i = import(&write(&pages));
    let page = &i.section.pages[0];
    assert_eq!(page.title.text(), "Ünïcödé 📝", "FMT.One.Read");
    assert_eq!(texts(page), vec![(0, "日本語 — ok".to_string()), (0, "café".to_string())]);
}

#[test]
fn an_empty_section_imports_empty() {
    let i = import(&write(&[]));
    assert!(i.section.pages.is_empty());
    assert_eq!(i.report.summary(), "imported nothing");
}

#[test]
fn a_notebook_toc_is_not_a_section() {
    let file = crate::writer::FileBuilder::new(crate::header::FileKind::Toc)
        .finish(&[crate::writer::node::object_space_manifest_root(crate::types::ExtendedGuid::NIL)]);
    let e = read_section(&file, "x", &mut IdGen::new(1), Timestamp(0)).unwrap_err();
    assert!(matches!(e, OneError::Unsupported(m) if m.contains(".onetoc2")));
}

#[test]
fn the_report_reads_as_a_sentence() {
    let mut r = Report::default();
    r.kept.insert("pages", 2);
    r.lost.insert("pictures", 1);
    r.warnings.push("x".into());
    assert_eq!(r.summary(), "imported 2 pages; not imported yet: 1 pictures; 1 damaged objects skipped");
}

/// Every truncation and many single-byte corruptions of a small section give a result or an
/// error, never a panic.
#[test]
fn damaged_files_never_panic() {
    let pages = vec![FixturePage { title: "T".into(), level: 1, outlines: vec![outline(&[(0, "a"), (1, "b")])], ..FixturePage::default() }];
    let good = write(&pages);
    for len in (0..good.len()).step_by(7) {
        let _ = read_section(&good[..len], "x", &mut IdGen::new(1), Timestamp(0));
    }
    for at in (0..good.len()).step_by(3) {
        for flip in [0x01u8, 0x80, 0xFF] {
            let mut bad = good.clone();
            bad[at] ^= flip;
            let _ = read_section(&bad, "x", &mut IdGen::new(1), Timestamp(0));
        }
    }
}

mod prop_tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn random_bytes_after_a_valid_header_never_panic(tail in proptest::collection::vec(any::<u8>(), 0..2048), cut in 0usize..4096) {
            let mut file = write(&[FixturePage { title: "T".into(), ..FixturePage::default() }]);
            let at = cut.min(file.len());
            file.truncate(at.max(crate::header::HEADER_SIZE));
            file.extend_from_slice(&tail);
            let _ = read_section(&file, "x", &mut IdGen::new(1), Timestamp(0));
        }
    }
}

#[test]
fn text_runs_keep_their_formats_styles_and_alignment() {
    use crate::writer::section::{FixtureFormat, FixtureStyle};
    use notecraft_color::Rgba;
    use notecraft_doc::text::{Align, ParaStyle, Script};
    // "Bold red, italic 14pt Arial, x²" with a hidden run that must not show.
    let text = "Bold red, italic x2";
    let runs = vec![
        (8, FixtureFormat { bold: true, color: Some(0x0000_00FF), ..FixtureFormat::default() }),
        (10, FixtureFormat::default()),
        (
            16,
            FixtureFormat {
                italic: true,
                font: Some("Arial".into()),
                half_points: Some(28),
                highlight: Some(0x0000_FFFF),
                ..FixtureFormat::default()
            },
        ),
        (17, FixtureFormat::default()),
        (18, FixtureFormat { hidden: true, ..FixtureFormat::default() }),
        (0, FixtureFormat { superscript: true, ..FixtureFormat::default() }),
    ];
    let mut o = outline(&[(0, text), (0, "Heading")]);
    o.styles = vec![
        FixtureStyle { runs, style_id: Some("p".into()), align: Some(1), ..FixtureStyle::default() },
        FixtureStyle { style_id: Some("h2".into()), ..FixtureStyle::default() },
    ];
    let i = import(&write(&[FixturePage { title: "F".into(), outlines: vec![o], ..FixturePage::default() }]));
    let page = &i.section.pages[0];
    let paras: Vec<_> = page.objects.iter().filter_map(PageObject::as_outline).flat_map(|o| o.paragraphs.clone()).collect();
    let p = &paras[0];
    assert_eq!(p.text(), "Bold red, italic 2", "the hidden run is dropped");
    let runs: Vec<_> = p.runs.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(runs, vec!["Bold red", ", ", "italic", " ", "2"]);
    assert_eq!(p.runs[0].fmt.bold, Some(true));
    assert_eq!(p.runs[0].fmt.color, Some(Rgba::rgb(255, 0, 0)), "COLORREF is 0x00BBGGRR");
    assert_eq!(p.runs[2].fmt.italic, Some(true));
    assert_eq!(p.runs[2].fmt.font.as_deref(), Some("Arial"));
    assert_eq!(p.runs[2].fmt.size, Some(14.0), "half points");
    assert_eq!(p.runs[2].fmt.highlight, Some(Rgba::rgb(255, 255, 0)));
    assert_eq!(p.runs[4].fmt.script, Some(Script::Super));
    assert_eq!((p.style, p.align), (ParaStyle::Normal, Align::Center));
    assert_eq!(paras[1].style, ParaStyle::Heading2);
    assert_eq!(i.report.lost.get("hidden text runs"), Some(&1), "FMT.One.ReadRichText");
}

#[test]
fn forged_run_positions_are_clamped() {
    use crate::writer::section::{FixtureFormat, FixtureStyle};
    let mut o = outline(&[(0, "abc")]);
    let bold = FixtureFormat { bold: true, ..FixtureFormat::default() };
    o.styles = vec![FixtureStyle { runs: vec![(99, bold.clone()), (1, bold.clone()), (0, bold)], ..FixtureStyle::default() }];
    let i = import(&write(&[FixturePage { title: "F".into(), outlines: vec![o], ..FixturePage::default() }]));
    let p = &i.section.pages[0].objects[0].as_outline().unwrap().paragraphs[0];
    assert_eq!(p.text(), "abc");
}

#[test]
fn lists_and_note_tags() {
    use crate::writer::section::FixtureStyle;
    use notecraft_doc::tags::TagSymbol;
    use notecraft_doc::text::{Bullet, ListFormat, Numbering};
    let mut o = outline(&[(0, "milk"), (0, "first"), (0, "second"), (0, "call Ann"), (0, "plain")]);
    let style = |list: Option<(Vec<u16>, Option<u32>)>, tags: Vec<(String, u16, bool)>| FixtureStyle { list, tags, ..FixtureStyle::default() };
    o.styles = vec![
        style(Some((vec![0x2022], None)), vec![("To Do".into(), 1, true)]),
        style(Some((vec![0xFFFD, 0, u16::from(b'.')], Some(3))), vec![]),
        style(Some((vec![0xFFFD, 4, u16::from(b')')], None)), vec![("Important".into(), 13, false), ("Ask Bob".into(), 15, false)]),
        style(None, vec![("Call back".into(), 1, false)]),
    ];
    let i = import(&write(&[FixturePage { title: "L".into(), outlines: vec![o], ..FixturePage::default() }]));
    let paras = &i.section.pages[0].objects[0].as_outline().unwrap().paragraphs;
    assert_eq!(paras[0].list, Some(ListFormat::Bullet(Bullet::Disc)), "FMT.One.ReadLists");
    assert_eq!(paras[1].list, Some(ListFormat::Number { style: Numbering::Decimal, restart: Some(3) }));
    assert_eq!(paras[2].list, Some(ListFormat::Number { style: Numbering::LowerAlphaParen, restart: None }));
    assert_eq!(paras[4].list, None);
    let tags = |k: usize| paras[k].tags.iter().map(|t| (t.key.as_str(), t.symbol, t.checked)).collect::<Vec<_>>();
    assert_eq!(tags(0), vec![("todo", TagSymbol::Checkbox, true)], "FMT.One.ReadTags");
    assert_eq!(tags(2), vec![("important", TagSymbol::Star, false), ("onenote-ask-bob", TagSymbol::Question, false)]);
    assert_eq!(paras[2].tags[1].name, "Ask Bob");
    assert_eq!(tags(3), vec![("call-back", TagSymbol::CheckboxPhone, false)], "built-in names keep NoteCraft's icon");
    assert_eq!(i.report.kept.get("note tags"), Some(&4));
    assert_eq!(i.report.kept.get("list items"), Some(&3));
}

/// A small PNG made with the codecs crate (no fixture files).
fn png(w: u32, h: u32) -> Vec<u8> {
    let px = (0..w * h).flat_map(|i| [(i * 7) as u8, 90, 30, 255]).collect();
    notecraft_codecs::encode_png(&notecraft_codecs::Rgba8::new(w, h, px).unwrap(), &notecraft_codecs::Limits::PICTURE).unwrap()
}

#[test]
fn pictures_attachments_and_tables() {
    let mut o = outline(&[(0, "see below"), (0, "and more")]);
    o.inline_pictures = vec![png(4, 2)];
    let page = FixturePage {
        title: "Media".into(),
        outlines: vec![o],
        pictures: vec![(4.0, 6.0, png(16, 8), "a chart".into()), (1.0, 1.0, b"not a picture".to_vec(), String::new())],
        attachments: vec![("report.pdf".into(), b"%PDF-1.7 tiny".to_vec())],
        tables: vec![(2.0, 10.0, vec![vec!["a".into(), "b".into()], vec!["c".into(), "d".into()]], vec![3.0, 5.0])],
        ..FixturePage::default()
    };
    let i = import(&write(&[page]));
    let p = &i.section.pages[0];
    let images: Vec<_> = p.objects.iter().filter_map(PageObject::as_image).collect();
    assert_eq!(images.len(), 2, "FMT.One.ReadImages");
    let chart = images.iter().find(|im| im.alt_text == "a chart").unwrap();
    assert_eq!(chart.pixels, (16, 8));
    assert_eq!((chart.pos.x, chart.pos.y), (144.0, 216.0));
    assert_eq!((chart.width, chart.height), (12.0, 6.0), "96 dpi without a stored size");
    let inline = images.iter().find(|im| im.pixels == (4, 2)).unwrap();
    assert_eq!((inline.pos.x, inline.pos.y), (72.0, 144.0 + 2.0 * LINE), "below the outline's two paragraphs");
    let files: Vec<_> = p.objects.iter().filter_map(PageObject::as_attachment).collect();
    assert_eq!(files.len(), 1, "FMT.One.ReadAttachments");
    assert_eq!((files[0].name.as_str(), &files[0].blob.bytes[..]), ("report.pdf", &b"%PDF-1.7 tiny"[..]));
    let tables: Vec<_> = p.objects.iter().filter_map(PageObject::as_table).collect();
    assert_eq!(tables.len(), 1, "FMT.One.ReadTables");
    let t = tables[0];
    let cells: Vec<Vec<String>> =
        t.rows.iter().map(|r| r.cells.iter().map(|c| c.paragraphs.iter().map(|p| p.text()).collect::<String>()).collect()).collect();
    assert_eq!(cells, vec![vec!["a", "b"], vec!["c", "d"]]);
    assert_eq!(t.columns, vec![108.0, 180.0]);
    assert!(t.borders);
    assert_eq!(i.report.lost.len(), 1, "{:?}", i.report.lost);
}
