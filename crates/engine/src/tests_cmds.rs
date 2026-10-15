//! One focused test per command family: every command runs in a meaningful state and its
//! observable effect is checked. (`tests.rs` holds the cross-cutting tests and workflows.)

use notecraft_doc::{Align, ListFormat, Numbering, ParaStyle, RuleLines, Script};
use serde_json::{Value, json};

use crate::selection::{Block, FocusArea, PageSel, Pos};
use crate::tests::{blank_page, body_texts, run, sample};
use crate::{ScrollRequest, Session};

/// A blank page holding `text` (newlines = paragraphs); the caret ends at the end.
fn page_with(text: &str) -> (Session, notecraft_doc::Id) {
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({ "text": text }));
    let o = s.page().unwrap().objects[0].as_outline().unwrap().id;
    (s, o)
}

fn caret(s: &mut Session, o: notecraft_doc::Id, para: usize, offset: usize) {
    run(s, "caret.set", json!({"block": o.to_string(), "para": para, "offset": offset}));
}

fn select(s: &mut Session, o: notecraft_doc::Id, para: usize, a: usize, b: usize) {
    caret(s, o, para, a);
    run(s, "caret.set", json!({"block": o.to_string(), "para": para, "offset": b, "extend": true}));
}

fn para(s: &Session, o: notecraft_doc::Id, i: usize) -> notecraft_doc::Paragraph {
    s.page().unwrap().outline(o).unwrap().paragraphs[i].clone()
}

fn eff(s: &Session, o: notecraft_doc::Id, i: usize, off: usize) -> notecraft_doc::CharFormat {
    para(s, o, i).effective_fmt_at(off)
}

#[test]
fn history_list_undo_redo() {
    let (mut s, _) = page_with("a");
    run(&mut s, "text.insert", json!({"text": " b"}));
    run(&mut s, "text.insert", json!({"text": " c"}));
    let r = run(&mut s, "edit.undoTo", json!({"steps": 2}));
    assert_eq!(r["undone"], 2);
    assert!(s.can_redo());
    let r = run(&mut s, "edit.redoTo", json!({"steps": 5}));
    assert!(r["redone"].as_u64().unwrap() >= 2);
    assert_eq!(body_texts(&s), vec!["a b c"]);
}

#[test]
fn cut_and_paste_variants() {
    // parity: R.Home.Clipboard.Cut
    let (mut s, o) = page_with("hello world");
    select(&mut s, o, 0, 0, 5);
    run(&mut s, "format.italic", json!({}));
    let r = run(&mut s, "edit.cut", json!({}));
    assert_eq!(r["text"], "hello");
    assert_eq!(body_texts(&s), vec![" world"]);
    run(&mut s, "edit.pasteKeepSource", json!({}));
    assert_eq!(eff(&s, o, 0, 1).italic, Some(true), "keep source formatting");
    run(&mut s, "caret.lineEnd", json!({}));
    run(&mut s, "edit.pasteMerge", json!({}));
    let p = para(&s, o, 0);
    assert_eq!(p.text(), "hello worldhello");
    assert_eq!(eff(&s, o, 0, p.len()).italic, Some(true), "merge keeps emphasis");
    run(&mut s, "edit.pasteOptions", json!({"mode": "text"}));
    let p = para(&s, o, 0);
    assert_eq!(eff(&s, o, 0, p.len()).italic, Some(true), "text-only takes the destination's format (italic at the caret)");
    assert!(s.run("edit.pasteOptions", json!({"mode": "bogus"})).is_err());
}

#[test]
fn format_painter_copies_and_pastes_format() {
    // parity: KB.Format.CopyFormat, KB.Format.PasteFormat
    let (mut s, o) = page_with("aaa bbb");
    select(&mut s, o, 0, 0, 3);
    run(&mut s, "format.underline", json!({}));
    caret(&mut s, o, 0, 1);
    run(&mut s, "edit.formatPainter", json!({}));
    select(&mut s, o, 0, 4, 7);
    run(&mut s, "format.pasteFormat", json!({"once": true}));
    assert_eq!(eff(&s, o, 0, 5).underline, Some(true));
    assert!(s.format_painter.is_none(), "the ribbon painter paints once");
    run(&mut s, "format.copyFormat", json!({}));
    assert!(s.format_painter.is_some());
}

#[test]
fn line_break_delete_forward_and_word_right() {
    // parity: KB.General.LineBreak, KB.General.DeleteChar, KB.General.DeleteWordRight
    let (mut s, o) = page_with("one two three");
    caret(&mut s, o, 0, 3);
    run(&mut s, "text.lineBreak", json!({}));
    assert_eq!(para(&s, o, 0).text(), "one\n two three");
    run(&mut s, "text.deleteForward", json!({}));
    assert_eq!(para(&s, o, 0).text(), "one\ntwo three");
    run(&mut s, "text.deleteWordRight", json!({}));
    assert_eq!(para(&s, o, 0).text(), "one\nthree");
}

#[test]
fn caret_commands_move_as_documented() {
    // parity: KB.General.WordLeft, KB.Select.ToLineEnd, KB.Pages.NextParagraph, KB.Pages.PrevParagraph
    let (mut s, o) = page_with("ab cd\nef\ngh");
    caret(&mut s, o, 0, 5);
    run(&mut s, "caret.wordLeft", json!({}));
    assert_eq!(s.sel.caret().unwrap().offset, 3);
    run(&mut s, "caret.left", json!({}));
    assert_eq!(s.sel.caret().unwrap().offset, 2);
    run(&mut s, "caret.selectLineEnd", json!({}));
    let r = s.text_range().unwrap();
    assert_eq!((r.anchor.offset, r.head.offset), (2, 5));
    run(&mut s, "edit.cancelSelection", json!({}));
    assert!(s.text_range().unwrap().is_caret());
    run(&mut s, "caret.down", json!({}));
    assert_eq!(s.sel.caret().unwrap().para, 1);
    run(&mut s, "caret.up", json!({}));
    assert_eq!(s.sel.caret().unwrap().para, 0);
    run(&mut s, "caret.selectDown", json!({}));
    assert!(!s.text_range().unwrap().is_caret());
    run(&mut s, "caret.nextParagraph", json!({}));
    assert_eq!(s.sel.caret().unwrap(), Pos::new(Block::Outline(o), 2, 0));
    run(&mut s, "caret.prevParagraph", json!({}));
    assert_eq!(s.sel.caret().unwrap().para, 1);
    // A UI-computed target wins over logical lines.
    run(&mut s, "caret.down", json!({"para": 2, "offset": 1}));
    assert_eq!(s.sel.caret().unwrap(), Pos::new(Block::Outline(o), 2, 1));
}

#[test]
fn character_formats() {
    // parity: R.Home.BasicText.Italic, R.Home.BasicText.Underline, R.Home.BasicText.Strikethrough, R.Home.BasicText.Subscript, R.Home.BasicText.Superscript
    let (mut s, o) = page_with("formula");
    select(&mut s, o, 0, 0, 7);
    for (cmd, check) in [
        ("format.italic", Box::new(|f: &notecraft_doc::CharFormat| f.italic == Some(true)) as Box<dyn Fn(&notecraft_doc::CharFormat) -> bool>),
        ("format.underline", Box::new(|f| f.underline == Some(true))),
        ("format.strikethrough", Box::new(|f| f.strike == Some(true))),
        ("format.subscript", Box::new(|f| f.script == Some(Script::Sub))),
        ("format.superscript", Box::new(|f| f.script == Some(Script::Super))),
    ] {
        run(&mut s, cmd, json!({}));
        assert!(check(&eff(&s, o, 0, 2)), "{cmd}");
    }
    run(&mut s, "format.superscript", json!({}));
    assert_eq!(eff(&s, o, 0, 2).script, Some(Script::Normal), "toggling superscript off");
    run(&mut s, "format.font", json!({"font": "Georgia"}));
    assert_eq!(eff(&s, o, 0, 2).font.as_deref(), Some("Georgia"));
    run(&mut s, "format.size", json!({"size": 13.3}));
    assert_eq!(eff(&s, o, 0, 2).size, Some(13.5), "sizes round to half points");
    assert!(s.run("format.size", json!({"size": 5000})).is_err());
    run(&mut s, "format.growFont", json!({}));
    assert_eq!(eff(&s, o, 0, 2).size, Some(14.0));
    run(&mut s, "format.shrinkFont", json!({}));
    run(&mut s, "format.shrinkFont", json!({}));
    assert_eq!(eff(&s, o, 0, 2).size, Some(11.0), "14 → 12 → 11 down the ladder");
    run(&mut s, "format.color", json!({"color": "#c00000"}));
    assert_eq!(eff(&s, o, 0, 2).color.map(|c| c.to_hex()), Some("#c00000".into()));
    run(&mut s, "format.highlight", json!({}));
    assert!(eff(&s, o, 0, 2).highlight.is_some());
    run(&mut s, "format.highlightNone", json!({}));
    assert!(eff(&s, o, 0, 2).highlight.is_none());
    run(&mut s, "format.highlight", json!({"color": "#00ff00"}));
    assert_eq!(eff(&s, o, 0, 2).highlight.map(|c| c.to_hex()), Some("#00ff00".into()));
}

#[test]
fn lists_and_numbering() {
    // parity: R.Home.BasicText.BulletsGallery, R.Home.BasicText.NumberingGallery, CM.Number.NumberListRestart
    let (mut s, o) = page_with("a\nb\nc");
    select(&mut s, o, 0, 0, 0);
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 2, "offset": 1, "extend": true}));
    run(&mut s, "para.bullets", json!({}));
    assert!((0..3).all(|i| matches!(para(&s, o, i).list, Some(ListFormat::Bullet(_)))));
    run(&mut s, "para.bullets", json!({}));
    assert!((0..3).all(|i| para(&s, o, i).list.is_none()), "Ctrl+. toggles off");
    run(&mut s, "para.numbering", json!({"style": "upperRoman"}));
    assert_eq!(para(&s, o, 1).list, Some(ListFormat::Number { style: Numbering::UpperRoman, restart: None }));
    caret(&mut s, o, 2, 0);
    run(&mut s, "para.restartNumbering", json!({"at": 5}));
    assert_eq!(notecraft_text::list_labels(&s.page().unwrap().outline(o).unwrap().paragraphs)[2], "V.");
    run(&mut s, "para.continueNumbering", json!({}));
    assert_eq!(notecraft_text::list_labels(&s.page().unwrap().outline(o).unwrap().paragraphs)[2], "III.");
    run(&mut s, "para.numberingRemove", json!({}));
    assert!(para(&s, o, 2).list.is_none());
    caret(&mut s, o, 0, 0);
    run(&mut s, "para.bullets", json!({"bullet": "star"}));
    run(&mut s, "para.bulletRemove", json!({}));
    assert!(para(&s, o, 0).list.is_none());
}

#[test]
fn indent_outdent_alignment_spacing_direction() {
    // parity: R.Home.BasicText.AlignLeft, R.Home.BasicText.AlignCenter, R.Home.BasicText.AlignRight, R.Home.BasicText.OutdentClassic
    let (mut s, o) = page_with("a\nb");
    caret(&mut s, o, 1, 0);
    run(&mut s, "para.indent", json!({}));
    assert_eq!(para(&s, o, 1).level, 1);
    run(&mut s, "para.outdent", json!({}));
    assert_eq!(para(&s, o, 1).level, 0);
    run(&mut s, "para.alignCenter", json!({}));
    assert_eq!(para(&s, o, 1).align, Align::Center);
    run(&mut s, "para.alignRight", json!({}));
    assert_eq!(para(&s, o, 1).align, Align::Right);
    run(&mut s, "para.alignLeft", json!({}));
    assert_eq!(para(&s, o, 1).align, Align::Left);
    run(&mut s, "para.spacingDialog", json!({"before": 6, "after": 12, "line": 18}));
    let p = para(&s, o, 1);
    assert_eq!((p.space_before, p.space_after, p.line_spacing), (6.0, 12.0, 18.0));
    assert!(s.run("para.spacingDialog", json!({})).is_err());
    run(&mut s, "para.rtl", json!({}));
    assert!(para(&s, o, 1).rtl);
    run(&mut s, "para.ltr", json!({}));
    assert!(!para(&s, o, 1).rtl);
    // Tab inside text inserts a tab character instead of indenting.
    caret(&mut s, o, 0, 1);
    run(&mut s, "para.indent", json!({"key": "tab"}));
    assert_eq!(para(&s, o, 0).text(), "a\t");
}

#[test]
fn every_style_applies() {
    let (mut s, o) = page_with("x");
    for (cmd, style) in [
        ("style.heading2", ParaStyle::Heading2),
        ("style.heading3", ParaStyle::Heading3),
        ("style.heading4", ParaStyle::Heading4),
        ("style.heading5", ParaStyle::Heading5),
        ("style.heading6", ParaStyle::Heading6),
        ("style.pageTitle", ParaStyle::PageTitle),
        ("style.citation", ParaStyle::Citation),
        ("style.quote", ParaStyle::Quote),
        ("style.code", ParaStyle::Code),
        ("style.normal", ParaStyle::Normal),
    ] {
        run(&mut s, cmd, json!({}));
        assert_eq!(para(&s, o, 0).style, style, "{cmd}");
    }
    run(&mut s, "style.apply", json!({"style": "Quote"}));
    assert_eq!(para(&s, o, 0).style, ParaStyle::Quote);
    assert!(s.run("style.apply", json!({"style": "Fancy"})).is_err());
}

#[test]
fn every_shortcut_tag_and_tag_menu_command() {
    // parity: KB.Tag.Important, KB.Tag.Question, KB.Tag.RememberForLater, KB.Tag.Definition, KB.Tag.Highlight, KB.Tag.Contact, KB.Tag.Address, KB.Tag.PhoneNumber, KB.Tag.RemoveAll
    let (mut s, o) = page_with("note");
    for (cmd, key) in [
        ("tag.important", "important"),
        ("tag.question", "question"),
        ("tag.rememberForLater", "remember-for-later"),
        ("tag.definition", "definition"),
        ("tag.highlight", "highlight"),
        ("tag.contact", "contact"),
        ("tag.address", "address"),
        ("tag.phoneNumber", "phone-number"),
    ] {
        run(&mut s, cmd, json!({}));
        assert!(para(&s, o, 0).tags.iter().any(|t| t.key == key), "{cmd}");
    }
    run(&mut s, "tag.important", json!({}));
    assert!(!para(&s, o, 0).tags.iter().any(|t| t.key == "important"), "pressing again clears the tag");
    run(&mut s, "tag.remove", json!({"key": "question"}));
    assert!(!para(&s, o, 0).tags.iter().any(|t| t.key == "question"));
    run(&mut s, "tag.removeAll", json!({}));
    assert!(para(&s, o, 0).tags.is_empty());
    run(&mut s, "tag.toggle", json!({"key": "call-back"}));
    run(&mut s, "tag.check", json!({}));
    assert!(para(&s, o, 0).tags[0].checked);
    run(&mut s, "tag.uncheck", json!({}));
    assert!(!para(&s, o, 0).tags[0].checked);
    run(&mut s, "tag.checkAll", json!({}));
    assert!(para(&s, o, 0).tags[0].checked);
    run(&mut s, "tag.clearAll", json!({}));
    assert!(!para(&s, o, 0).tags[0].checked);
    let r = run(&mut s, "tag.click", json!({"block": o.to_string(), "para": 0}));
    assert_eq!(r["checked"], true);
    assert!(s.run("tag.toggle", json!({"key": "nope"})).is_err());
}

#[test]
fn tag_summary_page_and_custom_tags() {
    // parity: BEH.Tags.SummaryPage, BEH.Tags.Custom
    let mut s = sample();
    let r = run(&mut s, "tag.summaryPage", json!({"group": "section"}));
    assert!(r["count"].as_u64().unwrap() > 3);
    assert_eq!(s.page().unwrap().display_title(), "Tags Summary");
    let r = run(&mut s, "tag.customize", json!({"action": "add", "name": "Recipe", "symbol": "book", "highlight": "#ffe0a0"}));
    let key = r["key"].as_str().unwrap().to_string();
    assert!(s.tag_defs().iter().any(|d| d.key == key));
    run(&mut s, "tag.customize", json!({"action": "modify", "key": key, "name": "Recipe idea"}));
    assert!(s.tag_defs().iter().any(|d| d.name == "Recipe idea"));
    run(&mut s, "tag.customize", json!({"action": "remove", "key": key}));
    assert!(!s.tag_defs().iter().any(|d| d.name == "Recipe idea"));
    assert!(s.run("tag.customize", json!({"action": "modify", "key": "todo"})).is_err(), "built-ins can't be modified");
}

#[test]
fn outline_levels_collapse_and_moves() {
    // parity: KB.Outline.Level1, KB.Outline.ExpandAll, KB.Outline.CollapseHeading, KB.Outline.ExpandHeading, KB.Select.MoveUp, KB.Select.Subordinates
    let (mut s, o) = page_with("A\nA1\nA1a\nB");
    caret(&mut s, o, 1, 0);
    run(&mut s, "para.indent", json!({}));
    caret(&mut s, o, 2, 0);
    run(&mut s, "para.indent", json!({}));
    run(&mut s, "para.indent", json!({}));
    let levels: Vec<u8> = s.page().unwrap().outline(o).unwrap().paragraphs.iter().map(|p| p.level).collect();
    assert_eq!(levels, vec![0, 1, 2, 0]);
    run(&mut s, "outline.level1", json!({}));
    assert!(para(&s, o, 0).collapsed);
    assert!(s.page().unwrap().outline(o).unwrap().is_hidden(1));
    assert_eq!(s.sel.caret().unwrap().para, 0, "the caret leaves hidden paragraphs");
    run(&mut s, "outline.expandAll", json!({}));
    assert!(!para(&s, o, 0).collapsed);
    for (n, cmd) in [
        "outline.level2",
        "outline.level3",
        "outline.level4",
        "outline.level5",
        "outline.level6",
        "outline.level7",
        "outline.level8",
        "outline.level9",
    ]
    .iter()
    .enumerate()
    {
        run(&mut s, cmd, json!({}));
        let hidden_a1a = s.page().unwrap().outline(o).unwrap().is_hidden(2);
        assert_eq!(hidden_a1a, n == 0, "{cmd}");
    }
    caret(&mut s, o, 0, 0);
    run(&mut s, "outline.collapse", json!({}));
    assert!(para(&s, o, 0).collapsed);
    run(&mut s, "outline.expand", json!({}));
    assert!(!para(&s, o, 0).collapsed);
    run(&mut s, "outline.toggleCollapse", json!({"block": o.to_string(), "para": 0}));
    assert!(para(&s, o, 0).collapsed);
    run(&mut s, "outline.toggleCollapse", json!({}));
    run(&mut s, "outline.selectBranch", json!({}));
    let r = s.text_range().unwrap();
    assert_eq!((r.ordered().0.para, r.ordered().1.para), (0, 2));
    caret(&mut s, o, 3, 0);
    run(&mut s, "outline.moveUp", json!({}));
    assert_eq!(body_texts(&s), vec!["B", "A", "A1", "A1a"]);
}

#[test]
fn containers_move_resize_and_paragraph_drag() {
    // parity: BEH.Canvas.ContainerMove, BEH.Canvas.ContainerResize, BEH.Outline.DragParagraph
    let (mut s, o) = page_with("one\ntwo");
    run(&mut s, "outline.move", json!({"id": o.to_string(), "x": 200.0, "y": -50.0}));
    let pos = s.page().unwrap().outline(o).unwrap().pos;
    assert_eq!((pos.x, pos.y), (200.0, 0.0), "containers stay on the page");
    run(&mut s, "outline.resize", json!({"id": o.to_string(), "width": 120.0}));
    assert_eq!(s.page().unwrap().outline(o).unwrap().width, notecraft_doc::OutlineWidth::Fixed(120.0));
    run(&mut s, "outline.resize", json!({"id": o.to_string(), "width": null}));
    assert_eq!(s.page().unwrap().outline(o).unwrap().width, notecraft_doc::OutlineWidth::Auto);
    run(&mut s, "outline.moveParagraph", json!({"from": o.to_string(), "para": 1, "to": o.to_string(), "index": 0}));
    assert_eq!(body_texts(&s), vec!["two", "one"]);
    run(&mut s, "object.select", json!({"ids": [o.to_string()]}));
    assert!(matches!(&s.sel.on_page, PageSel::Objects { ids } if ids == &vec![o]));
}

#[test]
fn page_creation_variants() {
    // parity: KB.Pages.NewSubpage, KB.Pages.NewPageBelow
    let mut s = sample();
    let start = s.sel.page.unwrap();
    run(&mut s, "page.newSubpage", json!({}));
    let sec = s.section().unwrap().clone();
    let i = sec.page_index(s.sel.page.unwrap()).unwrap();
    assert_eq!(sec.pages[i].level, 1);
    assert_eq!(sec.pages[i - 1].id, start);
    run(&mut s, "page.newBelow", json!({}));
    let sec = s.section().unwrap().clone();
    let j = sec.page_index(s.sel.page.unwrap()).unwrap();
    assert_eq!(sec.pages[j].level, 1, "below at the same level");
    run(&mut s, "page.newAfterGroup", json!({}));
    let sec = s.section().unwrap().clone();
    let k = sec.page_index(s.sel.page.unwrap()).unwrap();
    assert_eq!(sec.pages[k].level, 0);
    run(&mut s, "page.rename", json!({"title": "Renamed"}));
    assert_eq!(s.page().unwrap().display_title(), "Renamed");
}

#[test]
fn page_list_operations() {
    // parity: CM.Page.IndentedPagesCollapse, CM.Page.IndentedPagesShow, KB.Pages.ToggleGroup, KB.Pages.SelectPage
    let mut s = sample();
    let first = s.sel.page.unwrap();
    // The sample's first section: Welcome (0) with a subpage (1).
    run(&mut s, "page.collapseSubpages", json!({"page": first.to_string()}));
    assert!(s.section().unwrap().pages[0].collapsed);
    run(&mut s, "page.expandSubpages", json!({"page": first.to_string()}));
    assert!(!s.section().unwrap().pages[0].collapsed);
    run(&mut s, "page.toggleSubpages", json!({}));
    assert!(s.section().unwrap().pages[0].collapsed);
    run(&mut s, "page.toggleSubpages", json!({}));
    run(&mut s, "page.new", json!({"title": "Z"}));
    run(&mut s, "page.moveUp", json!({}));
    assert_eq!(s.section().unwrap().pages[0].display_title(), "Z", "moves past the whole family above");
    run(&mut s, "page.moveDown", json!({}));
    assert_eq!(s.section().unwrap().pages.last().unwrap().display_title(), "Z");
    let z = s.sel.page.unwrap();
    run(&mut s, "page.move", json!({"page": z.to_string(), "index": 0}));
    assert_eq!(s.section().unwrap().pages[0].id, z);
    let link = run(&mut s, "page.copyLink", json!({}));
    assert_eq!(link["link"], format!("notecraft:page/{z}"));
    run(&mut s, "page.select", json!({}));
    assert_eq!(s.sel.focus, FocusArea::PageList);
    run(&mut s, "edit.copy", json!({}));
    run(&mut s, "page.paste", json!({}));
    assert_eq!(s.section().unwrap().pages.iter().filter(|p| p.display_title() == "Z").count(), 2);
    run(&mut s, "page.setDateTime", json!({"ms": 0}));
    assert_eq!(s.page().unwrap().shown_time(), notecraft_doc::Timestamp(0));
}

#[test]
fn section_operations() {
    // parity: CM.Section.SectionColorMenu, CM.Section.SectionsMerge, CM.Section.SectionMove
    let mut s = sample();
    let secs: Vec<_> = s.notebook().unwrap().sections().iter().map(|x| x.id).collect();
    run(&mut s, "section.select", json!({"section": secs[1].to_string()}));
    assert_eq!(s.sel.section, Some(secs[1]));
    run(&mut s, "section.color", json!({"color": 7}));
    assert_eq!(s.section().unwrap().color.0, Some(7));
    run(&mut s, "section.color", json!({"color": null}));
    assert_eq!(s.section().unwrap().color.0, None);
    assert!(s.run("section.color", json!({"color": 16})).is_err());
    let link = run(&mut s, "section.copyLink", json!({}));
    assert!(link["link"].as_str().unwrap().starts_with("notecraft:section/"));
    let r = run(&mut s, "section.moveOrCopy", json!({"copy": true, "index": 0}));
    let copy: notecraft_doc::Id = notecraft_doc::Id::parse(r["section"].as_str().unwrap()).unwrap();
    assert_eq!(s.notebook().unwrap().sections()[0].id, copy);
    let pages_before = s.ws.section(secs[0]).unwrap().pages.len();
    let copied_pages = s.ws.section(copy).unwrap().pages.len();
    run(&mut s, "section.merge", json!({"section": copy.to_string(), "into": secs[0].to_string()}));
    assert_eq!(s.ws.section(secs[0]).unwrap().pages.len(), pages_before + copied_pages);
    assert!(s.ws.section(copy).is_none());
}

#[test]
fn section_groups() {
    let mut s = sample();
    let r = run(&mut s, "sectionGroup.new", json!({"name": "Archive"}));
    let g = notecraft_doc::Id::parse(r["group"].as_str().unwrap()).unwrap();
    run(&mut s, "sectionGroup.rename", json!({"group": g.to_string(), "name": "Old"}));
    assert_eq!(s.notebook().unwrap().group(g).unwrap().name, "Old");
    let link = run(&mut s, "sectionGroup.copyLink", json!({"group": g.to_string()}));
    assert!(link["link"].as_str().unwrap().starts_with("notecraft:group/"));
    let projects = s.notebook().unwrap().children.iter().find(|n| n.name() == "Projects").unwrap().id();
    run(&mut s, "sectionGroup.move", json!({"group": g.to_string(), "into": projects.to_string()}));
    assert!(s.notebook().unwrap().group(projects).unwrap().children.iter().any(|c| c.id() == g));
    assert!(s.run("sectionGroup.move", json!({"group": projects.to_string(), "into": g.to_string()})).is_err(), "no cycles");
    run(&mut s, "sectionGroup.delete", json!({"group": projects.to_string()}));
    assert!(s.notebook().unwrap().group(projects).is_none());
    assert!(!s.notebook().unwrap().recycle.sections.is_empty());
}

#[test]
fn notebook_operations() {
    let mut s = sample();
    run(&mut s, "file.newNotebook", json!({"name": "Second"}));
    let second = s.sel.notebook.unwrap();
    run(&mut s, "notebook.rename", json!({"name": "Work"}));
    assert_eq!(s.notebook().unwrap().name, "Work");
    let r = run(&mut s, "notebook.properties", json!({"color": 3}));
    assert_eq!(r["color"], 3);
    run(&mut s, "notebook.moveUp", json!({}));
    assert_eq!(s.ws.notebooks[0].id, second);
    run(&mut s, "notebook.moveDown", json!({}));
    assert_eq!(s.ws.notebooks[1].id, second);
    let link = run(&mut s, "notebook.copyLink", json!({}));
    assert!(link["link"].as_str().unwrap().starts_with("notecraft:notebook/"));
    let first = s.ws.notebooks[0].id;
    run(&mut s, "notebook.select", json!({"notebook": first.to_string()}));
    assert_eq!(s.sel.notebook, Some(first));
    run(&mut s, "notebook.close", json!({"notebook": second.to_string()}));
    assert_eq!(s.ws.notebooks.len(), 1);
    assert!(run(&mut s, "file.save", json!({}))["saved"].as_bool().unwrap());
    let prefs = run(&mut s, "file.options", json!({"set": {"author": "Ada", "auto_bullets": false}}));
    assert_eq!(prefs["author"], "Ada");
    assert!(!s.prefs.auto_bullets);
    assert!(s.run("file.options", json!({"set": {"nope": 1}})).is_err());
}

#[test]
fn recycle_bin_view_and_empty() {
    // parity: R.History.History.RecycleBinMenu, R.History.History.EmptyRecycleBin
    let mut s = sample();
    run(&mut s, "page.delete", json!({}));
    let r = run(&mut s, "history.recycleBin", json!({}));
    assert_eq!(r["pages"].as_array().unwrap().len(), 1);
    run(&mut s, "history.emptyRecycleBin", json!({}));
    assert!(s.notebook().unwrap().recycle.is_empty());
    assert!(s.run("history.emptyRecycleBin", json!({})).is_err(), "disabled when empty");
}

#[test]
fn page_and_section_navigation() {
    // parity: KB.Notebooks.FirstPage, KB.Notebooks.LastPage, KB.Notebooks.NextSection, KB.Notebooks.PrevSection, KB.Pages.NextContainer
    let mut s = sample();
    let first = s.sel.page.unwrap();
    run(&mut s, "nav.lastPage", json!({}));
    assert_ne!(s.sel.page.unwrap(), first);
    run(&mut s, "nav.prevPage", json!({}));
    assert_eq!(s.sel.page.unwrap(), first);
    run(&mut s, "nav.lastVisiblePage", json!({}));
    run(&mut s, "nav.firstVisiblePage", json!({}));
    run(&mut s, "nav.firstPage", json!({}));
    assert_eq!(s.sel.page.unwrap(), first);
    let sec = s.sel.section;
    run(&mut s, "nav.nextSection", json!({}));
    assert_ne!(s.sel.section, sec);
    run(&mut s, "nav.prevSection", json!({}));
    assert_eq!(s.sel.section, sec);
    run(&mut s, "nav.prevSection", json!({}));
    assert_eq!(s.sel.section, s.notebook().unwrap().sections().last().map(|x| x.id), "Ctrl+Shift+Tab wraps");
    run(&mut s, "nav.goto", json!({"page": first.to_string()}));
    run(&mut s, "nav.nextContainer", json!({}));
    assert!(matches!(s.sel.caret().unwrap().block, Block::Outline(_)));
    run(&mut s, "nav.focusPageTab", json!({}));
    assert_eq!(s.sel.focus, FocusArea::PageList);
    run(&mut s, "nav.focusSectionTab", json!({}));
    assert_eq!(s.sel.focus, FocusArea::SectionTabs);
    run(&mut s, "nav.notebookList", json!({}));
    assert_eq!(s.sel.focus, FocusArea::NotebookPane);
    // Ctrl+click adds pages to the selection.
    let other = s.section().unwrap().pages[1].id;
    run(&mut s, "nav.goto", json!({"page": other.to_string(), "add": true}));
    assert_eq!(s.sel.pages.len(), 2);
}

#[test]
fn find_on_page_steps_and_scopes() {
    // parity: KB.Search.Next, KB.Search.Prev, KB.Search.Dismiss, CM.SearchScope.SearchScopeSection
    let (mut s, _) = page_with("pea pea pea");
    let r = run(&mut s, "search.page", json!({"query": "pea"}));
    assert_eq!(r["matches"], 3);
    let r = run(&mut s, "search.next", json!({}));
    assert_eq!(r["current"], 2);
    let r = run(&mut s, "search.prev", json!({}));
    assert_eq!(r["current"], 1);
    run(&mut s, "search.prev", json!({}));
    assert_eq!(s.view.find.current, 2, "wraps around");
    run(&mut s, "search.dismiss", json!({}));
    assert!(!s.view.find.active);
    for (cmd, kind) in [
        ("search.scopePage", crate::ScopeKind::Page),
        ("search.scopeSection", crate::ScopeKind::Section),
        ("search.scopeSectionGroup", crate::ScopeKind::SectionGroup),
        ("search.scopeNotebook", crate::ScopeKind::Notebook),
        ("search.scopeAll", crate::ScopeKind::AllNotebooks),
    ] {
        run(&mut s, cmd, json!({}));
        assert_eq!(s.view.scope, kind, "{cmd}");
    }
    run(&mut s, "search.scopeSection", json!({}));
    run(&mut s, "search.setDefaultScope", json!({}));
    assert_eq!(s.prefs.default_scope, crate::ScopeKind::Section);
    run(&mut s, "search.resultsPane", json!({}));
    let hit = run(&mut s, "search.all", json!({"query": "garden", "scope": "all"}));
    let page = hit["results"][0]["page"].as_str().unwrap().to_string();
    run(&mut s, "search.openResult", json!({"page": page, "query": "garden"}));
    assert!(!s.view.find.matches.is_empty(), "the query is highlighted on the opened page");
}

#[test]
fn view_commands() {
    // parity: R.View.Zoom.ZoomIn, R.View.Zoom.ZoomOut, R.View.Zoom.ZoomCurrent100, R.View.PageSetup.RuleLinesMenu, R.View.PageSetup.PageTitleHide, R.View.Views.FullPageView, KB.Format.RuleLines
    let mut s = sample();
    run(&mut s, "view.zoomIn", json!({}));
    assert_eq!(s.view.zoom, 1.25);
    run(&mut s, "view.zoomOut", json!({}));
    run(&mut s, "view.zoomOut", json!({}));
    assert_eq!(s.view.zoom, 0.75);
    run(&mut s, "view.zoom", json!({"percent": 150}));
    assert_eq!(s.view.zoom, 1.5);
    assert!(s.run("view.zoom", json!({"percent": 5000})).is_err());
    run(&mut s, "view.zoom100", json!({}));
    assert_eq!(s.view.zoom, 1.0);
    run(&mut s, "view.zoom75", json!({}));
    assert_eq!(s.view.zoom, 0.75);
    run(&mut s, "view.pageWidth", json!({"factor": 1.1}));
    assert!((s.view.zoom - 1.1).abs() < 1e-6);
    run(&mut s, "view.ruleLines", json!({}));
    assert_eq!(s.page().unwrap().setup.rule_lines, RuleLines::Standard);
    run(&mut s, "view.ruleLines", json!({}));
    assert_eq!(s.page().unwrap().setup.rule_lines, RuleLines::None);
    run(&mut s, "view.ruleLines", json!({"kind": "gridSmall"}));
    assert_eq!(s.page().unwrap().setup.rule_lines, RuleLines::GridSmall);
    run(&mut s, "view.ruleLineColor", json!({"color": "#ff0000"}));
    assert!(s.page().unwrap().setup.rule_color.is_some());
    run(&mut s, "view.newPagesWithRuleLines", json!({}));
    assert!(s.prefs.new_pages_rule_lines.is_some());
    run(&mut s, "page.new", json!({}));
    assert_eq!(s.page().unwrap().setup.rule_lines, RuleLines::GridSmall, "new pages get rule lines");
    run(&mut s, "view.hidePageTitle", json!({}));
    assert!(s.page().unwrap().setup.hide_title);
    run(&mut s, "view.pageColor", json!({"color": "#dce9f7"}));
    assert!(s.page().unwrap().setup.color.is_some());
    run(&mut s, "view.pageColor", json!({"color": null}));
    assert!(s.page().unwrap().setup.color.is_none());
    run(&mut s, "view.fullPage", json!({}));
    assert!(s.view.full_page);
    run(&mut s, "view.normal", json!({}));
    assert!(!s.view.full_page);
    for (cmd, req) in [
        ("view.scrollPageUp", ScrollRequest::PageUp),
        ("view.scrollPageDown", ScrollRequest::PageDown),
        ("view.scrollTop", ScrollRequest::Top),
        ("view.scrollBottom", ScrollRequest::Bottom),
    ] {
        run(&mut s, cmd, json!({}));
        assert_eq!(s.scroll_request, Some(req), "{cmd}");
    }
    run(&mut s, "ui.collapseRibbon", json!({}));
    assert!(s.view.ribbon_collapsed);
    // UI.Ribbon.ModeSwitch: simplified by default; toggles, or takes the mode by name.
    assert_eq!(s.view.ribbon_mode, crate::RibbonMode::Simplified);
    run(&mut s, "ui.ribbonMode", json!({}));
    assert_eq!(s.view.ribbon_mode, crate::RibbonMode::Classic);
    run(&mut s, "ui.ribbonMode", json!({"mode": "simplified"}));
    assert_eq!(s.view.ribbon_mode, crate::RibbonMode::Simplified);
    assert!(s.run("ui.ribbonMode", json!({"mode": "wide"})).is_err(), "unknown modes are refused");
    let w = s.view.page_list_width;
    run(&mut s, "ui.pageListWider", json!({}));
    assert!(s.view.page_list_width > w);
    run(&mut s, "ui.pageListNarrower", json!({}));
    assert_eq!(s.view.page_list_width, w);
    run(&mut s, "view.notebookPane", json!({}));
    assert!(s.view.notebook_pane_collapsed);
    // R.View.Views.NavigationPanes / UI.Nav.Toggle: ☰ shows the navigation again.
    run(&mut s, "view.navigationPanes", json!({}));
    assert!(!s.view.notebook_pane_collapsed);
}

#[test]
fn quick_notes_opens_or_creates_the_section() {
    // UI.Nav.QuickNotes: the sample has a Quick Notes section; it is opened, not duplicated.
    let mut s = sample();
    let sections = s.notebook().unwrap().sections().len();
    let r = run(&mut s, "nav.quickNotes", json!({}));
    assert_eq!(r["created"], false);
    assert_eq!(s.section().unwrap().name, "Quick Notes");
    assert_eq!(s.notebook().unwrap().sections().len(), sections);
    // Without one, it is created at the end of the first notebook, with a page, in one undo step.
    let qid = s.section().unwrap().id.to_string();
    run(&mut s, "section.delete", json!({"section": qid}));
    let r = run(&mut s, "nav.quickNotes", json!({}));
    assert_eq!(r["created"], true);
    assert_eq!(s.section().unwrap().name, "Quick Notes");
    assert_eq!(s.section().unwrap().pages.len(), 1);
    run(&mut s, "edit.undo", json!({}));
    assert!(s.notebook().unwrap().sections().iter().all(|x| x.name != "Quick Notes"));
}

#[test]
fn sort_pages_keeps_subpages_with_their_page() {
    // UI.Nav.SortPages: by title A→Z; a subpage stays under its page; one undo step.
    let mut s = sample();
    run(&mut s, "section.new", json!({"name": "Sort"}));
    for t in ["Charlie", "alpha", "Bravo"] {
        run(&mut s, "page.new", json!({"title": t}));
    }
    // "Bravo" becomes a subpage of the page before it, "alpha".
    let bravo = s.page().unwrap().id.to_string();
    run(&mut s, "page.demote", json!({"page": bravo}));
    let titles = |s: &Session| s.section().unwrap().pages.iter().map(|p| p.display_title()).collect::<Vec<_>>();
    let before = titles(&s);
    run(&mut s, "page.sort", json!({"by": "title"}));
    let after = titles(&s);
    let pos = |name: &str| after.iter().position(|t| t == name).unwrap();
    assert!(pos("alpha") < pos("Charlie"), "{after:?}");
    assert_eq!(pos("Bravo"), pos("alpha") + 1, "the subpage follows its page: {after:?}");
    run(&mut s, "page.sort", json!({"by": "title", "descending": true}));
    assert!(titles(&s).iter().position(|t| t == "Charlie") < titles(&s).iter().position(|t| t == "alpha"));
    assert!(s.run("page.sort", json!({"by": "colour"})).is_err());
    assert_eq!(titles(&s).len(), before.len(), "a refused sort keeps every page");
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(titles(&s), before);
}

#[test]
fn links_insert_edit_remove_copy_select() {
    // parity: R.Insert.Links.HyperlinkCreateHere, CM.Text.LinkEdit, CM.Text.LinkRemove, CM.Text.LinkCopyOneNote, CM.Text.LinkSelect
    let (mut s, o) = page_with("see ");
    run(&mut s, "insert.link", json!({"url": "https://example.org", "text": "example"}));
    assert_eq!(para(&s, o, 0).text(), "see example");
    caret(&mut s, o, 0, 6);
    run(&mut s, "link.select", json!({}));
    let r = s.text_range().unwrap();
    assert_eq!((r.ordered().0.offset, r.ordered().1.offset), (4, 11));
    caret(&mut s, o, 0, 6);
    let c = run(&mut s, "link.copy", json!({}));
    assert_eq!(c["text"], "https://example.org");
    run(&mut s, "link.edit", json!({"url": "https://example.com"}));
    assert_eq!(para(&s, o, 0).fmt_at(6).link.as_deref(), Some("https://example.com"));
    let opened = run(&mut s, "link.open", json!({}));
    assert_eq!(opened["open_url"], "https://example.com", "external links go back to the UI");
    run(&mut s, "link.remove", json!({}));
    assert!(para(&s, o, 0).fmt_at(6).link.is_none());
    // Linking a selection.
    select(&mut s, o, 0, 0, 3);
    run(&mut s, "insert.link", json!({"url": "https://a.b"}));
    assert_eq!(para(&s, o, 0).fmt_at(2).link.as_deref(), Some("https://a.b"));
    assert!(s.run("insert.link", json!({"url": "bad\u{0}url"})).is_err());
    // Paragraph links navigate back to the paragraph.
    caret(&mut s, o, 0, 0);
    let l = run(&mut s, "para.copyLink", json!({}));
    let url = l["link"].as_str().unwrap().to_string();
    run(&mut s, "nav.firstPage", json!({}));
    run(&mut s, "link.open", json!({"url": url}));
    assert_eq!(s.sel.caret().unwrap().block, Block::Outline(o));
}

#[test]
fn date_time_stamp() {
    // parity: R.Insert.TimeStamp.DateAndTimeInsertOneNote
    let (mut s, _) = page_with("");
    let r = run(&mut s, "insert.dateTime", json!({}));
    assert_eq!(r["text"], "10/14/2026 7:45 PM");
}

#[test]
fn json_results_are_serialisable() {
    // Agents read every result as JSON; null is fine.
    let mut s = sample();
    let v: Value = run(&mut s, "file.options", json!({}));
    assert!(v.is_object());
}
