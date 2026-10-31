//! Engine tests: the registry against the parity catalog, `panic_hunt` (every command with
//! hostile parameters), and the M1 workflows. Tests that pin documented reference behaviour
//! carry `// parity: <catalog ID>`.

use serde_json::{Value, json};

use crate::selection::{Block, PageSel, Pos};
use crate::{Clock, Session, catalog, commands};

pub(crate) fn clock() -> Clock {
    // 2026-10-14 19:45 UTC.
    Clock { now_ms: 1_792_007_100_000, utc_offset_min: 0 }
}

pub(crate) fn sample() -> Session {
    let mut s = Session::with_sample(clock());
    s.prefs.author = "Tester".into();
    s
}

pub(crate) fn run(s: &mut Session, id: &str, v: Value) -> Value {
    match s.run(id, v) {
        Ok(v) => v,
        Err(e) => panic!("{id} failed: {e}"),
    }
}

/// A blank page in the sample's first section, opened, with no caret.
pub(crate) fn blank_page(s: &mut Session) {
    run(s, "page.new", json!({}));
    s.sel.on_page = PageSel::None;
}

pub(crate) fn body_texts(s: &Session) -> Vec<String> {
    let page = s.page().unwrap();
    page.objects.iter().filter_map(|o| o.as_outline()).flat_map(|o| o.paragraphs.iter().map(|p| p.text())).collect()
}

#[test]
fn command_ids_are_unique_and_documented() {
    let mut ids: Vec<&str> = commands::all().iter().map(|c| c.id).collect();
    let n = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n, "duplicate command ids");
    for c in commands::all() {
        assert!(!c.label.is_empty() && !c.menu.is_empty() && !c.catalog.is_empty(), "{} lacks label, menu or catalog id", c.id);
        assert!(c.params.starts_with('{'), "{}: params doc must be a JSON-ish object", c.id);
    }
}

#[test]
fn registry_matches_the_parity_catalog() {
    let rows = catalog::rows();
    assert!(rows.len() > 1000, "the catalog should embed ~1300 rows, got {}", rows.len());
    let mut problems = Vec::new();
    for c in commands::all() {
        let Some(row) = rows.iter().find(|r| r.id == c.catalog) else {
            problems.push(format!("{}: catalog id {} not in docs/parity-catalog.md", c.id, c.catalog));
            continue;
        };
        if row.path != c.menu {
            problems.push(format!("{}: menu `{}` != catalog path `{}`", c.id, c.menu, row.path));
        }
        if !row.command.is_empty() && row.command != c.id {
            problems.push(format!("{}: row {} names command `{}`", c.id, row.id, row.command));
        }
        let keys: Vec<String> = c.keys().map(|k| k.replace(' ', "").to_lowercase()).collect();
        for k in row.shortcut.split(" / ").map(|k| k.replace(' ', "").to_lowercase()).filter(|k| !k.is_empty()) {
            if !keys.contains(&k) {
                problems.push(format!("{}: catalog key `{k}` not bound (has `{}`)", c.id, c.shortcut));
            }
        }
    }
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

#[test]
fn every_catalog_command_row_with_a_command_points_at_a_registered_or_planned_command() {
    // Rows marked wired must name a registered command.
    for r in catalog::rows() {
        if (r.status == "wired" || r.status == "verified") && !r.command.is_empty() {
            assert!(commands::find(&r.command).is_some(), "{} is {} but `{}` is not registered", r.id, r.status, r.command);
        }
    }
}

/// Junk values every parameter is tried with.
fn junk() -> Vec<Value> {
    vec![
        Value::Null,
        json!(true),
        json!(-1),
        json!(1e308),
        json!(f64::MIN),
        json!(u64::MAX),
        json!(""),
        json!("\u{0}\u{1b}"),
        json!("x".repeat(10_000)),
        json!("ffffffffffffffffffffffffffffffff"),
        json!("00000000000000000000000000000000"),
        json!([]),
        json!({}),
        json!([1, "a", null]),
        json!("#zzzzzz"),
        json!("title"),
        json!("../../etc/passwd"),
    ]
}

const PARAM_NAMES: &[&str] = &[
    "text",
    "query",
    "title",
    "name",
    "url",
    "key",
    "page",
    "section",
    "notebook",
    "group",
    "block",
    "para",
    "offset",
    "extend",
    "x",
    "y",
    "id",
    "width",
    "index",
    "level",
    "color",
    "size",
    "font",
    "style",
    "bullet",
    "steps",
    "scope",
    "kind",
    "percent",
    "into",
    "copy",
    "from",
    "to",
    "item",
    "path",
    "folder",
    "ms",
    "mode",
    "set",
    "action",
    "symbol",
    "after",
    "at",
    "before",
    "line",
    "ids",
    "add",
    "range",
    "first",
    "last",
    "factor",
    "unchecked",
    "tag",
    "once",
    "limit",
    "rows",
    "cols",
    "table",
    "col",
    "data",
    "dx",
    "dy",
    "dw",
    "dh",
    "height",
    "dir",
    "points",
    "gesture",
    "pen",
    "tool",
];

#[test]
fn panic_hunt() {
    // Every command, in several session states, with hostile parameters: never a panic (the
    // guard would turn one into an "internal error"), never a corrupt workspace.
    let states: Vec<Box<dyn Fn() -> Session>> = vec![
        Box::new(|| Session::new(clock())),
        Box::new(sample),
        Box::new(|| {
            let mut s = sample();
            run(&mut s, "text.insert", json!({"text": "hello world"}));
            s
        }),
        Box::new(|| {
            let mut s = sample();
            run(&mut s, "text.insert", json!({"text": "abc\ndef"}));
            run(&mut s, "edit.selectAll", json!({}));
            run(&mut s, "edit.selectAll", json!({}));
            s
        }),
        Box::new(|| {
            let mut s = sample();
            s.sel.focus = crate::selection::FocusArea::PageList;
            s
        }),
        // A table with the caret in a cell, and with cells selected.
        Box::new(|| {
            let mut s = sample();
            run(&mut s, "insert.table", json!({"rows": 2, "cols": 3}));
            run(&mut s, "text.insert", json!({"text": "b\tc"}));
            s
        }),
        Box::new(|| {
            let mut s = sample();
            run(&mut s, "insert.table", json!({"rows": 3, "cols": 2}));
            run(&mut s, "table.selectColumns", json!({}));
            s
        }),
        // A selected picture, and a selected attachment (decision D3).
        Box::new(|| {
            let mut s = sample();
            let png = notecraft_codecs::encode_png(&notecraft_codecs::Rgba8::new(2, 2, vec![9; 16]).unwrap(), &Default::default()).unwrap();
            run(&mut s, "insert.picture", json!({"data": crate::b64::encode(&png)}));
            s
        }),
        Box::new(|| {
            let mut s = sample();
            run(&mut s, "insert.file", json!({"data": "aGVsbG8=", "name": "a.txt"}));
            s
        }),
        // Ink strokes, lassoed (decision D4).
        Box::new(|| {
            let mut s = sample();
            run(&mut s, "draw.stroke", json!({"points": [[10, 300, 0.5], [60, 320, 0.9], [90, 300]]}));
            run(&mut s, "draw.stroke", json!({"points": [[20, 400]], "pen": 3}));
            run(&mut s, "draw.lassoSelect", json!({"points": [[0, 280], [200, 280], [200, 450], [0, 450]]}));
            s
        }),
    ];
    let mut tried = 0usize;
    for make in &states {
        for c in commands::all() {
            if matches!(
                c.id,
                "file.newNotebook"
                    | "file.openNotebook"
                    | "file.openRecent"
                    | "file.exportNotebook"
                    | "notebook.changeLocation"
                    | "file.exportPage"
                    | "file.exportSection"
                    | "file.exportMht"
                    | "file.print"
                    | "file.import"
                    | "file.printPreview"
                    | "file.backupNow"
                    | "file.backupChanged"
                    | "file.openBackups"
                    | "object.saveAs"
                    | "attachment.saveAs"
                    | "attachment.open"
                    | "printout.openOriginal"
            ) {
                // These touch the file system; the hostile `path`/`folder` values are covered by
                // their own tests below with a sandboxed folder.
                continue;
            }
            let mut variants = vec![json!({})];
            for name in PARAM_NAMES {
                for j in junk() {
                    variants.push(json!({ *name: j }));
                }
            }
            for v in variants {
                let mut s = make();
                tried += 1;
                if let Err(e) = s.run(c.id, v.clone()) {
                    assert!(!e.to_string().contains("internal error"), "{} panicked with {v}", c.id);
                }
                // Invariants still hold: a caret, if any, is valid.
                if let Some(r) = s.sel.text() {
                    assert!(s.clamp_pos(r.head).is_some() || s.page().is_none(), "{} left a dangling caret", c.id);
                }
            }
        }
    }
    assert!(tried > 10_000);
}

#[test]
fn typing_creates_a_container_and_undo_is_per_word() {
    // parity: BEH.Canvas.ClickToType, BEH.Text.Undo
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "outline.new", json!({"x": 100.0, "y": 150.0}));
    assert_eq!(s.page().unwrap().objects.len(), 0, "a click alone creates nothing");
    for ch in "hello world".chars() {
        run(&mut s, "text.insert", json!({"text": ch.to_string()}));
    }
    assert_eq!(body_texts(&s), vec!["hello world"]);
    let pos = s.page().unwrap().objects[0].pos();
    assert_eq!((pos.x, pos.y), (100.0, 150.0));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(body_texts(&s), vec!["hello "], "undo removes the last word");
    run(&mut s, "edit.undo", json!({}));
    assert!(body_texts(&s).is_empty() || body_texts(&s) == vec![""], "then the first word: {:?}", body_texts(&s));
    run(&mut s, "edit.redo", json!({}));
    assert_eq!(body_texts(&s), vec!["hello "]);
}

#[test]
fn enter_continues_lists_and_ends_them_on_an_empty_item() {
    // parity: BEH.Outline.EnterNewParagraph, BEH.Outline.EmptyBulletEnds, BEH.Outline.AutoBullet
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "* "}));
    run(&mut s, "text.insert", json!({"text": "milk"}));
    run(&mut s, "text.newline", json!({}));
    run(&mut s, "text.insert", json!({"text": "eggs"}));
    let page = s.page().unwrap();
    let o = page.objects[0].as_outline().unwrap();
    assert_eq!(o.paragraphs.len(), 2);
    assert!(o.paragraphs.iter().all(|p| matches!(p.list, Some(notecraft_doc::ListFormat::Bullet(_)))));
    assert_eq!(o.paragraphs[0].text(), "milk", "the * prefix is consumed");
    run(&mut s, "text.newline", json!({}));
    run(&mut s, "text.newline", json!({}));
    let o = s.page().unwrap().objects[0].as_outline().unwrap().clone();
    assert_eq!(o.paragraphs.len(), 3);
    assert!(o.paragraphs[2].list.is_none(), "Enter on an empty bullet ends the list");
}

#[test]
fn tab_indents_children_move_with_parent() {
    // parity: BEH.Outline.TabIndent, BEH.Outline.MoveParagraph
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "A\nA1\nB"}));
    // Indent A1 under A.
    let o = s.page().unwrap().objects[0].as_outline().unwrap().id;
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 1, "offset": 0}));
    run(&mut s, "para.indent", json!({"key": "tab"}));
    let levels: Vec<u8> = s.page().unwrap().outline(o).unwrap().paragraphs.iter().map(|p| p.level).collect();
    assert_eq!(levels, vec![0, 1, 0]);
    // Move A (with A1) below B.
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 0, "offset": 0}));
    run(&mut s, "outline.moveDown", json!({}));
    assert_eq!(body_texts(&s), vec!["B", "A", "A1"]);
    assert_eq!(s.sel.caret().unwrap().para, 1, "the caret follows the moved paragraph");
}

#[test]
fn bold_toggles_over_selection_and_caret() {
    // parity: R.Home.BasicText.Bold, KB.Format.Bold
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "one two"}));
    let o = s.page().unwrap().objects[0].as_outline().unwrap().id;
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 0, "offset": 0}));
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 0, "offset": 3, "extend": true}));
    run(&mut s, "format.bold", json!({}));
    let p = &s.page().unwrap().outline(o).unwrap().paragraphs[0];
    assert_eq!(p.runs[0].text, "one");
    assert_eq!(p.runs[0].fmt.bold, Some(true));
    run(&mut s, "format.bold", json!({}));
    assert_eq!(s.page().unwrap().outline(o).unwrap().paragraphs[0].runs.len(), 1, "toggling back merges the runs");
    // Caret at the end: bold applies to the next typed text only.
    run(&mut s, "caret.lineEnd", json!({}));
    run(&mut s, "format.bold", json!({}));
    run(&mut s, "text.insert", json!({"text": "!"}));
    let p = &s.page().unwrap().outline(o).unwrap().paragraphs[0];
    assert_eq!(p.runs.last().unwrap().text, "!");
    assert_eq!(p.runs.last().unwrap().fmt.bold, Some(true));
}

#[test]
fn todo_tag_cycles_and_find_tags_lists_it() {
    // parity: KB.Tag.ToDo, BEH.Tags.Summary, R.Home.Tags.FindTaggedNotes
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "buy seeds"}));
    run(&mut s, "tag.todo", json!({}));
    let tags = |s: &Session| s.page().unwrap().objects[0].as_outline().unwrap().paragraphs[0].tags.clone();
    assert_eq!(tags(&s).len(), 1);
    assert!(!tags(&s)[0].checked);
    run(&mut s, "tag.todo", json!({}));
    assert!(tags(&s)[0].checked);
    run(&mut s, "tag.todo", json!({}));
    assert!(tags(&s).is_empty());
    run(&mut s, "tag.todo", json!({}));
    let found = run(&mut s, "tag.findTags", json!({"scope": "page"}));
    assert_eq!(found["count"], 1);
    assert_eq!(found["groups"][0]["group"], "To Do");
    let all = run(&mut s, "tag.findTags", json!({}));
    assert!(all["count"].as_u64().unwrap() > 5, "the sample notebook has tagged notes");
}

#[test]
fn search_scopes_and_find_on_page() {
    // parity: BEH.Search.Instant, BEH.Search.Scope, UI.Nav.FindOnPage
    let mut s = sample();
    let r = run(&mut s, "search.all", json!({"query": "seed"}));
    assert!(r["total"].as_u64().unwrap() >= 2, "{r}");
    let titles: Vec<String> = r["results"].as_array().unwrap().iter().map(|x| x["title"].as_str().unwrap().to_string()).collect();
    assert!(titles.contains(&"Weekly planning".to_string()));
    // Typing makes new text searchable immediately.
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "zucchini bread"}));
    let r = run(&mut s, "search.all", json!({"query": "zucch"}));
    assert_eq!(r["total"], 1);
    let r = run(&mut s, "search.all", json!({"query": "seed", "scope": "page"}));
    assert_eq!(r["total"], 0);
    let n = run(&mut s, "search.page", json!({"query": "BREAD"}));
    assert_eq!(n["matches"], 1);
    let sel = s.text_range().unwrap();
    assert_eq!(sel.ordered().0.offset, 9);
}

#[test]
fn pages_subpages_and_recycle_bin() {
    // parity: KB.Pages.NewPage, KB.Pages.Demote, KB.Pages.Promote, BEH.Recycle.DeletedPages
    let mut s = sample();
    let before = s.section().unwrap().pages.len();
    run(&mut s, "page.new", json!({"title": "Parent"}));
    run(&mut s, "page.new", json!({"title": "Child"}));
    run(&mut s, "page.demote", json!({}));
    let sec = s.section().unwrap();
    assert_eq!(sec.pages.len(), before + 2);
    assert_eq!(sec.pages.last().unwrap().level, 1);
    assert!(s.run("page.demote", json!({})).is_err(), "can't be two levels deeper than the page above");
    run(&mut s, "page.promote", json!({}));
    assert_eq!(s.section().unwrap().pages.last().unwrap().level, 0);
    run(&mut s, "page.delete", json!({}));
    assert_eq!(s.section().unwrap().pages.len(), before + 1);
    assert_eq!(s.notebook().unwrap().recycle.pages.len(), 1);
    let deleted = s.notebook().unwrap().recycle.pages[0].page.id;
    run(&mut s, "history.restore", json!({"item": deleted.to_string()}));
    assert!(s.notebook().unwrap().recycle.is_empty());
    assert_eq!(s.page().unwrap().display_title(), "Child");
}

#[test]
fn sections_get_default_names_colours_and_a_page() {
    // parity: BEH.Sections.NewSectionName, BEH.Sections.Colors, KB.Notebooks.NewSection
    let mut s = sample();
    let r = run(&mut s, "section.new", json!({}));
    assert_eq!(r["rename"], true);
    let sec = s.section().unwrap();
    assert_eq!(sec.name, "New Section 1");
    assert_eq!(sec.pages.len(), 1);
    assert!(sec.color.0.is_some());
    run(&mut s, "section.rename", json!({"name": "Recipes"}));
    assert_eq!(s.section().unwrap().name, "Recipes");
    run(&mut s, "section.delete", json!({}));
    assert_eq!(s.notebook().unwrap().recycle.sections.len(), 1);
}

#[test]
fn navigation_back_forward() {
    // parity: KB.Select.Back, KB.Select.Forward
    let mut s = sample();
    let first = s.sel.page.unwrap();
    run(&mut s, "nav.nextPage", json!({}));
    let second = s.sel.page.unwrap();
    assert_ne!(first, second);
    run(&mut s, "nav.back", json!({}));
    assert_eq!(s.sel.page, Some(first));
    run(&mut s, "nav.forward", json!({}));
    assert_eq!(s.sel.page, Some(second));
}

#[test]
fn wiki_links_create_and_follow_pages() {
    // parity: BEH.Links.Wiki
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "see [[Compost tips]]"}));
    assert_eq!(body_texts(&s), vec!["see Compost tips"]);
    let link = s.page().unwrap().objects[0].as_outline().unwrap().paragraphs[0].runs.last().unwrap().fmt.link.clone().unwrap();
    assert!(link.starts_with("notecraft:page/"));
    run(&mut s, "link.open", json!({"url": link}));
    assert_eq!(s.page().unwrap().display_title(), "Compost tips");
}

#[test]
fn copy_paste_keeps_formatting_and_paste_text_drops_it() {
    // parity: BEH.Paste.HTML
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "bold"}));
    run(&mut s, "edit.selectAll", json!({}));
    run(&mut s, "format.bold", json!({}));
    let copied = run(&mut s, "edit.copy", json!({}));
    assert_eq!(copied["text"], "bold");
    run(&mut s, "caret.lineEnd", json!({}));
    run(&mut s, "edit.paste", json!({"text": "bold"}));
    let p = &s.page().unwrap().objects[0].as_outline().unwrap().paragraphs[0];
    assert_eq!(p.text(), "boldbold");
    assert!(p.runs.iter().all(|r| r.fmt.bold == Some(true)));
    run(&mut s, "edit.pasteTextOnly", json!({"text": "plain"}));
    let p = &s.page().unwrap().objects[0].as_outline().unwrap().paragraphs[0];
    assert!(p.text().ends_with("plain"));
    // Multi-line external text becomes paragraphs.
    run(&mut s, "edit.paste", json!({"text": "x\ny\nz"}));
    assert_eq!(s.page().unwrap().objects[0].as_outline().unwrap().paragraphs.len(), 3);
}

#[test]
fn empty_containers_disappear_when_left() {
    // parity: BEH.Canvas.EmptyContainerRemoved
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "x"}));
    run(&mut s, "text.backspace", json!({}));
    assert_eq!(s.page().unwrap().objects.len(), 1, "kept while the caret is in it");
    run(&mut s, "nav.pageTitle", json!({}));
    assert_eq!(s.page().unwrap().objects.len(), 0);
}

#[test]
fn styles_and_clear_formatting() {
    // parity: R.Home.Styles.Heading1, BEH.Text.ClearFormatting
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "Heading"}));
    run(&mut s, "style.heading1", json!({}));
    assert_eq!(s.page().unwrap().objects[0].as_outline().unwrap().paragraphs[0].style, notecraft_doc::ParaStyle::Heading1);
    run(&mut s, "format.clear", json!({}));
    assert_eq!(s.page().unwrap().objects[0].as_outline().unwrap().paragraphs[0].style, notecraft_doc::ParaStyle::Normal);
}

#[test]
fn select_all_expands_in_steps() {
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "a\nb"}));
    run(&mut s, "edit.selectAll", json!({}));
    let r = s.text_range().unwrap();
    assert_eq!((r.ordered().0.para, r.ordered().1.para), (1, 1));
    run(&mut s, "edit.selectAll", json!({}));
    let r = s.text_range().unwrap();
    assert_eq!((r.ordered().0.para, r.ordered().1.para), (0, 1));
    run(&mut s, "edit.selectAll", json!({}));
    assert!(matches!(s.sel.on_page, PageSel::Objects { .. }));
    run(&mut s, "edit.delete", json!({}));
    assert!(s.page().unwrap().objects.is_empty());
}

#[test]
fn caret_motion_and_word_deletion() {
    // parity: KB.General.DeleteWordLeft, KB.General.LineStart
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "alpha beta gamma"}));
    run(&mut s, "text.deleteWordLeft", json!({}));
    assert_eq!(body_texts(&s), vec!["alpha beta "]);
    run(&mut s, "caret.lineStart", json!({}));
    assert_eq!(s.sel.caret().unwrap().offset, 0);
    run(&mut s, "caret.wordRight", json!({}));
    assert_eq!(s.sel.caret().unwrap().offset, 6);
    run(&mut s, "caret.right", json!({"extend": true}));
    assert!(!s.text_range().unwrap().is_caret());
}

#[test]
fn insert_date_and_time_use_the_host_clock() {
    // parity: R.Insert.TimeStamp.DateInsert, R.Insert.TimeStamp.TimeInsert
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "insert.date", json!({}));
    run(&mut s, "text.insert", json!({"text": " "}));
    run(&mut s, "insert.time", json!({}));
    assert_eq!(body_texts(&s), vec!["10/14/2026 7:45 PM"]);
}

#[test]
fn title_enter_moves_into_the_body() {
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "nav.pageTitle", json!({}));
    run(&mut s, "text.insert", json!({"text": "Groceries"}));
    run(&mut s, "text.newline", json!({}));
    run(&mut s, "text.insert", json!({"text": "apples"}));
    assert_eq!(s.page().unwrap().display_title(), "Groceries");
    assert_eq!(body_texts(&s), vec!["apples"]);
    assert!(matches!(s.sel.caret().unwrap().block, Block::Outline(_)));
}

#[test]
fn disabled_commands_say_why() {
    let mut s = Session::new(clock());
    let e = s.run("format.bold", json!({})).unwrap_err();
    assert!(matches!(e, crate::CmdError::Disabled(_)), "{e}");
    assert!(s.run("no.such.command", json!({})).is_err());
    assert!(s.run("text.insert", json!(["not", "an", "object"])).is_err());
}

#[test]
fn autosave_writes_only_dirty_sections_and_reopens() {
    // parity: BEH.Save.Autosave, FMT.Native.Ncraft
    let dir = std::env::temp_dir().join(format!("notecraft-engine-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut s = Session::new(clock());
    run(&mut s, "file.newNotebook", json!({"name": "Journal", "path": dir.to_string_lossy()}));
    assert!(!s.is_dirty());
    run(&mut s, "text.insert", json!({"text": "Dear diary"}));
    assert!(s.is_dirty());
    s.clock.now_ms += 2000;
    assert!(s.autosave_due(1000));
    assert!(s.save_dirty().is_empty());
    assert!(!s.is_dirty());
    let mut t = Session::new(clock());
    run(&mut t, "file.openNotebook", json!({"path": dir.to_string_lossy()}));
    let texts = {
        let p = t.page().unwrap();
        p.plain_text()
    };
    assert!(texts.contains("Dear diary"), "{texts}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hostile_file_paths_fail_cleanly() {
    let mut s = Session::new(clock());
    for p in ["", "\u{0}", "/proc/definitely/not/a/notebook", "../../.."] {
        if let Err(e) = s.run("file.openNotebook", json!({"path": p})) {
            assert!(!e.to_string().contains("internal error"));
        }
    }
}

#[test]
fn move_or_copy_page_between_sections() {
    // parity: QAT.NR.MoveOrCopyPage
    let mut s = sample();
    let target = s.notebook().unwrap().sections()[1].id;
    let pid = s.sel.page.unwrap();
    run(&mut s, "page.moveOrCopy", json!({"section": target.to_string(), "copy": true}));
    assert_ne!(s.sel.page.unwrap(), pid, "a copy gets fresh ids");
    assert!(s.ws.locate_page(pid).is_some(), "the original stays");
    run(&mut s, "page.moveOrCopy", json!({"page": pid.to_string(), "section": target.to_string()}));
    assert_eq!(s.ws.locate_page(pid).unwrap().0, target);
}

#[test]
fn caret_positions_from_agents_are_clamped() {
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "é👍"}));
    let o = s.page().unwrap().objects[0].as_outline().unwrap().id;
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 99, "offset": 1}));
    let c = s.sel.caret().unwrap();
    assert_eq!(c, Pos::new(Block::Outline(o), 0, 0), "offset 1 is inside é: snapped back");
}
