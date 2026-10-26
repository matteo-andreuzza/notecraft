use notecraft_doc::id::IdGen;
use notecraft_doc::page::{OutlineWidth, PageObject};
use notecraft_doc::time::Timestamp;

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
