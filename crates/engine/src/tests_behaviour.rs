//! Documented behaviours of the reference (catalog `BEH.*` rows) pinned through the commands
//! that implement them. Each assertion's message is the row it pins.

use serde_json::json;

use crate::Session;
use crate::tests::{blank_page, run, sample};

fn outline(s: &Session) -> notecraft_doc::Id {
    s.page().unwrap().objects.iter().find_map(|o| o.as_outline()).unwrap().id
}

fn paras(s: &Session) -> Vec<notecraft_doc::Paragraph> {
    s.page().unwrap().objects.iter().find_map(|o| o.as_outline()).unwrap().paragraphs.clone()
}

#[test]
fn a_copied_page_link_navigates_back_to_the_page() {
    // parity: BEH.Links.CopyLink
    let mut s = sample();
    let target = s.page().unwrap().id;
    let link = run(&mut s, "page.copyLink", json!({}))["link"].as_str().unwrap().to_string();
    blank_page(&mut s);
    assert_ne!(s.page().unwrap().id, target);
    run(&mut s, "link.open", json!({"url": link}));
    assert_eq!(s.page().unwrap().id, target, "BEH.Links.CopyLink");
}

#[test]
fn a_new_notebook_starts_with_one_section_and_one_untitled_page() {
    let mut s = sample();
    run(&mut s, "file.newNotebook", json!({"name": "Recipes"}));
    let nb = s.notebook().unwrap();
    assert_eq!(nb.name, "Recipes");
    let sections = nb.sections();
    assert_eq!(sections.len(), 1, "BEH.Notebooks.Default");
    assert_eq!(sections[0].name, "New Section 1");
    assert_eq!(sections[0].pages.len(), 1);
    assert_eq!(sections[0].pages[0].title.text(), "", "an Untitled page");
}

#[test]
fn numbered_lists_renumber_and_use_1_a_i_by_level() {
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "one\ntwo\nthree"}));
    let o = outline(&s);
    for i in 0..3 {
        run(&mut s, "caret.set", json!({"block": o.to_string(), "para": i, "offset": 0}));
        run(&mut s, "para.numbering", json!({}));
    }
    let labels = notecraft_text::list_labels(&paras(&s));
    assert_eq!(labels, ["1.", "2.", "3."], "BEH.Outline.NumberingSequence");
    // Indenting the second item renumbers the third and gives the nested one letters.
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 1, "offset": 0}));
    run(&mut s, "para.indent", json!({}));
    run(&mut s, "para.numbering", json!({}));
    run(&mut s, "para.numbering", json!({}));
    let labels = notecraft_text::list_labels(&paras(&s));
    assert_eq!(labels, ["1.", "a.", "2."], "BEH.Outline.NumberingSequence");
    assert_eq!(notecraft_doc::text::Numbering::for_level(2).label(1), "i.");
}

#[test]
fn subpages_go_two_levels_deep_move_with_their_parent_and_collapse() {
    let mut s = sample();
    run(&mut s, "page.new", json!({"title": "Parent"}));
    let parent = s.page().unwrap().id;
    run(&mut s, "page.new", json!({"title": "Child"}));
    run(&mut s, "page.demote", json!({}));
    run(&mut s, "page.new", json!({"title": "Grandchild"}));
    run(&mut s, "page.demote", json!({}));
    run(&mut s, "page.demote", json!({}));
    let levels = |s: &Session| s.section().unwrap().pages.iter().map(|p| (p.title.text(), p.level)).collect::<Vec<_>>();
    let tail: Vec<_> = levels(&s).into_iter().rev().take(3).collect();
    assert_eq!(tail, [("Grandchild".to_string(), 2), ("Child".to_string(), 1), ("Parent".to_string(), 0)], "BEH.Pages.Subpages");
    run(&mut s, "page.new", json!({"title": "Deeper"}));
    run(&mut s, "page.demote", json!({}));
    run(&mut s, "page.demote", json!({}));
    assert!(s.run("page.demote", json!({})).is_err(), "BEH.Pages.Subpages: at most two levels below a top page");
    run(&mut s, "page.delete", json!({}));
    // The family moves together: Parent to the top of the section.
    run(&mut s, "page.move", json!({"page": parent.to_string(), "index": 0}));
    let head: Vec<_> = levels(&s).into_iter().take(3).map(|(t, _)| t).collect();
    assert_eq!(head, ["Parent", "Child", "Grandchild"], "BEH.Pages.Subpages: subpages move with their parent");
    // Collapsing the parent hides its subpages in the page list.
    run(&mut s, "page.collapseSubpages", json!({"page": parent.to_string()}));
    let sec = s.section().unwrap();
    assert!(sec.pages[0].collapsed, "BEH.Pages.Collapse");
    run(&mut s, "page.expandSubpages", json!({"page": parent.to_string()}));
    assert!(!s.section().unwrap().pages[0].collapsed, "BEH.Pages.Collapse");
}

#[test]
fn pages_show_and_change_their_date_and_time() {
    let mut s = sample();
    blank_page(&mut s);
    let created = s.page().unwrap().created;
    assert_eq!(s.page().unwrap().shown_time, None, "BEH.Pages.DateTime: the creation time is shown");
    assert_eq!(created, s.now());
    let when = notecraft_doc::Timestamp::from_civil(2024, 5, 1, 9, 30, 0);
    run(&mut s, "page.setDateTime", json!({"ms": when.0}));
    assert_eq!(s.page().unwrap().shown_time, Some(when), "BEH.Pages.DateTime");
}

#[test]
fn a_page_dragged_onto_another_section_moves_there() {
    let mut s = sample();
    let pid = s.page().unwrap().id;
    let from = s.section().unwrap().id;
    let to = s.notebook().unwrap().sections().iter().map(|x| x.id).find(|&x| x != from).unwrap();
    run(&mut s, "page.move", json!({"page": pid.to_string(), "section": to.to_string(), "index": 0}));
    assert!(s.ws.section(from).unwrap().pages.iter().all(|p| p.id != pid));
    assert_eq!(s.ws.section(to).unwrap().pages[0].id, pid, "BEH.Pages.DragReorder");
}

#[test]
fn emptying_the_recycle_bin_deletes_for_good() {
    let mut s = sample();
    run(&mut s, "page.delete", json!({}));
    assert!(!s.notebook().unwrap().recycle.is_empty());
    run(&mut s, "history.emptyRecycleBin", json!({}));
    assert!(s.notebook().unwrap().recycle.is_empty(), "BEH.Recycle.Empty");
    assert!(s.run("history.emptyRecycleBin", json!({})).is_err(), "nothing left to empty");
}

#[test]
fn section_groups_nest_sections_and_groups() {
    let mut s = sample();
    let outer = run(&mut s, "sectionGroup.new", json!({"name": "School"}))["group"].as_str().unwrap().to_string();
    let inner = run(&mut s, "sectionGroup.new", json!({"name": "Maths", "group": outer}))["group"].as_str().unwrap().to_string();
    run(&mut s, "section.new", json!({"name": "Algebra", "group": inner}));
    let nb = s.notebook().unwrap();
    let outer_g = nb.group(notecraft_doc::Id::parse(&outer).unwrap()).unwrap();
    let inner_g = nb.group(notecraft_doc::Id::parse(&inner).unwrap()).unwrap();
    assert!(outer_g.children.iter().any(|c| c.id().to_string() == inner), "BEH.Sections.Groups");
    assert!(inner_g.children.iter().any(|c| matches!(c, notecraft_doc::NavNode::Section(x) if x.name == "Algebra")), "BEH.Sections.Groups");
}

#[test]
fn checkboxes_toggle_on_click_and_tags_stack_and_toggle() {
    // parity: BEH.Tags.CheckboxClick
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "call the plumber"}));
    run(&mut s, "tag.todo", json!({}));
    let o = outline(&s).to_string();
    assert_eq!(run(&mut s, "tag.click", json!({"block": o, "para": 0}))["checked"], true, "BEH.Tags.CheckboxClick");
    assert_eq!(run(&mut s, "tag.click", json!({"block": o, "para": 0}))["checked"], false, "BEH.Tags.CheckboxClick");
    // Several tags on one paragraph.
    run(&mut s, "tag.important", json!({}));
    run(&mut s, "tag.question", json!({}));
    let keys: Vec<String> = paras(&s)[0].tags.iter().map(|t| t.key.clone()).collect();
    assert_eq!(keys.len(), 3, "BEH.Tags.MultipleTags");
    // Applying a tag that is already there removes it.
    run(&mut s, "tag.important", json!({}));
    let keys: Vec<String> = paras(&s)[0].tags.iter().map(|t| t.key.clone()).collect();
    assert_eq!(keys.len(), 2, "BEH.Tags.Toggle");
    assert!(!keys.contains(&"important".to_string()), "BEH.Tags.Toggle");
}

#[test]
fn new_text_uses_the_default_font_and_styles_replace_character_formatting() {
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "Heading text"}));
    let p = &paras(&s)[0];
    let f = p.effective_fmt_at(0);
    assert_eq!((f.font.as_deref(), f.size), (Some(notecraft_doc::text::DEFAULT_FONT), Some(11.0)), "BEH.Text.DefaultFont");
    let o = outline(&s);
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 0, "offset": 0}));
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 0, "offset": 7, "extend": true}));
    run(&mut s, "format.bold", json!({}));
    assert!(paras(&s)[0].runs.iter().any(|r| r.fmt.bold == Some(true)));
    run(&mut s, "style.apply", json!({"style": "Heading 1"}));
    let p = &paras(&s)[0];
    assert!(p.runs.iter().all(|r| r.fmt.bold.is_none()), "BEH.Text.Styles: the style's formatting replaces the run's");
    assert_eq!(p.style.def().name, "Heading 1", "BEH.Text.Styles");
}

#[test]
fn zoom_runs_from_10_to_400_percent_through_fixed_stops() {
    let mut s = sample();
    run(&mut s, "view.zoom", json!({"percent": 100}));
    run(&mut s, "view.zoomIn", json!({}));
    assert_eq!(s.view.zoom, 1.25, "BEH.View.Zoom");
    for _ in 0..20 {
        run(&mut s, "view.zoomIn", json!({}));
    }
    assert_eq!(s.view.zoom, 4.0, "BEH.View.Zoom: at most 400%");
    for _ in 0..30 {
        run(&mut s, "view.zoomOut", json!({}));
    }
    assert_eq!(s.view.zoom, 0.1, "BEH.View.Zoom: at least 10%");
    assert!(s.run("view.zoom", json!({"percent": 401})).is_err(), "BEH.View.Zoom");
    assert!(s.run("view.zoom", json!({"percent": 9})).is_err(), "BEH.View.Zoom");
}

#[test]
fn an_expression_before_equals_is_worked_out_on_space_or_enter() {
    let mut s = sample();
    blank_page(&mut s);
    for c in ["9", "5", "+", "8", "3", "+", "4", "1", "6", "=", " "] {
        run(&mut s, "text.insert", json!({"text": c}));
    }
    assert_eq!(paras(&s)[0].text(), "95+83+416=594 ", "BEH.Text.MathCalc");
    let c = s.sel.caret().unwrap();
    assert_eq!(c.offset, "95+83+416=594 ".len(), "the caret follows the answer");
    run(&mut s, "text.insert", json!({"text": "sqrt(16)="}));
    run(&mut s, "text.newline", json!({}));
    let texts: Vec<String> = paras(&s).iter().map(|p| p.text()).collect();
    assert_eq!(texts, ["95+83+416=594 sqrt(16)=4", ""], "BEH.Text.MathCalc: Enter puts the answer, then a new line");
    // Prose is left alone, and the option turns it off.
    run(&mut s, "text.insert", json!({"text": "Total = "}));
    assert_eq!(paras(&s)[1].text(), "Total = ");
    run(&mut s, "file.options", json!({"set": {"calculate_math": false}}));
    run(&mut s, "text.insert", json!({"text": "1+1= "}));
    assert_eq!(paras(&s)[1].text(), "Total = 1+1= ", "OPT.Advanced.CalculateMath");
}

// ---- File › Options: each option changes what it says. ----

fn temp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("notecraft-options-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn the_default_font_size_and_colour_apply_to_new_text() {
    let mut s = sample();
    run(&mut s, "file.options", json!({"set": {"default_font": "Georgia", "default_size": 14.0, "default_color": "#0A141E"}}));
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "Hello\nWorld"}));
    for p in paras(&s) {
        let f = p.effective_fmt_at(1);
        assert_eq!((f.font.as_deref(), f.size), (Some("Georgia"), Some(14.0)), "OPT.Advanced.DefaultFont");
        assert_eq!(f.color, Some(notecraft_doc::color::Rgba { r: 10, g: 20, b: 30, a: 255 }), "OPT.Advanced.DefaultFont");
    }
    // The defaults store nothing extra.
    run(&mut s, "file.options", json!({"set": {"default_font": "Calibri", "default_size": 11.0, "default_color": null}}));
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "plain"}));
    assert!(paras(&s)[0].runs.iter().all(|r| r.fmt.is_plain()));
}

#[test]
fn auto_lists_tables_on_tab_and_wiki_links_follow_their_options() {
    let mut s = sample();
    run(&mut s, "file.options", json!({"set": {"auto_bullets": false, "auto_numbering": false}}));
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "* item"}));
    assert!(paras(&s)[0].list.is_none(), "OPT.Advanced.AutoBulleting");
    run(&mut s, "text.newline", json!({}));
    run(&mut s, "text.insert", json!({"text": "1. item"}));
    assert!(paras(&s)[1].list.is_none(), "OPT.Advanced.AutoNumbering");
    run(&mut s, "file.options", json!({"set": {"auto_bullets": true, "auto_numbering": true}}));
    run(&mut s, "text.newline", json!({}));
    run(&mut s, "text.insert", json!({"text": "* item"}));
    assert!(paras(&s)[2].list.is_some(), "OPT.Advanced.AutoBulleting");
    // Tab after text: a table only with the option on.
    run(&mut s, "file.options", json!({"set": {"create_table_on_tab": false}}));
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "Name"}));
    let _ = s.run("para.indent", json!({"key": "tab"}));
    assert!(s.page().unwrap().objects.iter().all(|o| o.as_table().is_none()), "OPT.Advanced.CreateTableOnTab");
    run(&mut s, "file.options", json!({"set": {"create_table_on_tab": true}}));
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "Name"}));
    run(&mut s, "para.indent", json!({"key": "tab"}));
    assert!(s.page().unwrap().objects.iter().any(|o| o.as_table().is_some()), "OPT.Advanced.CreateTableOnTab");
    // Wiki links.
    run(&mut s, "file.options", json!({"set": {"wiki_links": false}}));
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "[[Plans]]"}));
    assert_eq!(paras(&s)[0].text(), "[[Plans]]", "OPT.Advanced.WikiLinks");
    run(&mut s, "file.options", json!({"set": {"wiki_links": true, "author": "Ada", "initials": "AL"}}));
    run(&mut s, "text.insert", json!({"text": " [[Plans]]"}));
    assert!(paras(&s)[0].runs.iter().any(|r| r.fmt.link.is_some()), "OPT.Advanced.WikiLinks");
    assert_eq!(paras(&s)[0].author, "Ada", "OPT.General.UserName");
    assert_eq!(s.prefs.initials, "AL", "OPT.General.Initials");
}

#[test]
fn new_notebooks_go_to_the_default_location_and_quick_notes_to_the_chosen_notebook() {
    let d = temp("location");
    let mut s = sample();
    run(&mut s, "file.options", json!({"set": {"default_notebook_location": d.to_str().unwrap()}}));
    let r = run(&mut s, "file.newNotebook", json!({"name": "Garden"}));
    let id = notecraft_doc::Id::parse(r["notebook"].as_str().unwrap()).unwrap();
    assert_eq!(s.notebook_path(id).as_deref(), Some(d.join("Garden").as_path()), "OPT.SaveBackup.DefaultNotebookLocation");
    let r = run(&mut s, "file.newNotebook", json!({"name": "Scratch", "memory": true}));
    let id = notecraft_doc::Id::parse(r["notebook"].as_str().unwrap()).unwrap();
    assert_eq!(s.notebook_path(id), None);
    // Quick Notes go to the notebook named in the Options.
    run(&mut s, "file.options", json!({"set": {"quick_notes_notebook": "garden"}}));
    run(&mut s, "nav.quickNotes", json!({}));
    assert_eq!(s.notebook().unwrap().name, "Garden", "OPT.SaveBackup.QuickNotesSection");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn backups_use_the_folder_interval_and_copies_of_the_options() {
    let d = temp("backup");
    let mut s = sample();
    run(&mut s, "file.newNotebook", json!({"name": "Work", "path": d.join("Work").to_str().unwrap()}));
    assert!(s.run("file.backupNow", json!({})).is_err(), "no folder yet");
    run(&mut s, "file.options", json!({"set": {"backup_folder": d.join("bk").to_str().unwrap(), "backup_copies": 1}}));
    assert!(s.backup_due(), "OPT.SaveBackup.AutoBackup: never backed up");
    let r = run(&mut s, "file.backupNow", json!({}));
    assert_eq!(r["backups"].as_array().unwrap().len(), 1, "OPT.SaveBackup.BackupFolder");
    assert!(r["backups"][0].as_str().unwrap().contains("Work"), "OPT.SaveBackup.BackupAllNow");
    assert!(!s.backup_due(), "OPT.SaveBackup.AutoBackup: just done");
    s.clock.now_ms += 25 * 3_600_000;
    assert!(s.backup_due(), "OPT.SaveBackup.AutoBackup");
    // Nothing changed since: Back Up Changed Files Now skips the notebook.
    let r = run(&mut s, "file.backupChanged", json!({}));
    assert_eq!(r["backups"].as_array().unwrap().len(), 0, "OPT.SaveBackup.BackupChangedNow");
    s.clock.now_ms += 60_000;
    run(&mut s, "text.insert", json!({"text": "changed"}));
    let r = run(&mut s, "file.backupChanged", json!({}));
    assert_eq!(r["backups"].as_array().unwrap().len(), 1, "OPT.SaveBackup.BackupChangedNow");
    let kept = std::fs::read_dir(d.join("bk").join("Work")).unwrap().count();
    assert_eq!(kept, 1, "OPT.SaveBackup.BackupCopies");
    run(&mut s, "file.options", json!({"set": {"auto_backup_minutes": 0}}));
    assert!(!s.backup_due());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn selected_words_link_to_the_page_of_that_title() {
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "text.insert", json!({"text": "see Garden plans soon"}));
    let o = outline(&s);
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 0, "offset": 4}));
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 0, "offset": 16, "extend": true}));
    let pages = s.section().unwrap().pages.len();
    let r = run(&mut s, "link.toPage", json!({}));
    assert_eq!(s.section().unwrap().pages.len(), pages + 1, "CM.Text.ToPageLink: a new page");
    let target = notecraft_doc::Id::parse(r["page"].as_str().unwrap()).unwrap();
    assert_eq!(s.ws.page(target).unwrap().title.text(), "Garden plans");
    assert_eq!(paras(&s)[0].fmt_at(6).link.as_deref(), Some(r["link"].as_str().unwrap()), "CM.Text.ToPageLink");
    // Again on the same words: the existing page, no new one.
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 0, "offset": 4}));
    run(&mut s, "caret.set", json!({"block": o.to_string(), "para": 0, "offset": 16, "extend": true}));
    run(&mut s, "link.toPage", json!({}));
    assert_eq!(s.section().unwrap().pages.len(), pages + 1);
}

#[test]
fn built_in_styles_have_their_documented_sizes() {
    use notecraft_doc::ParaStyle;
    let size = |n: &str| ParaStyle::from_name(n).unwrap().def().char.size;
    let font = |n: &str| ParaStyle::from_name(n).unwrap().def().char.font;
    for (name, pt) in [
        ("Heading 1", 16.0),
        ("Heading 2", 14.0),
        ("Heading 3", 12.0),
        ("Heading 4", 12.0),
        ("Heading 5", 11.0),
        ("Heading 6", 11.0),
        ("Page Title", 20.0),
        ("Citation", 9.0),
        ("Quote", 11.0),
        ("Code", 10.0),
        ("Normal", 11.0),
    ] {
        assert_eq!(size(name), Some(pt), "BEH.Text.HeadingSizes: {name}");
    }
    assert_eq!(font("Code").as_deref(), Some("Consolas"), "BEH.Text.HeadingSizes");
    assert_eq!(ParaStyle::from_name("Quote").unwrap().def().char.italic, Some(true), "BEH.Text.HeadingSizes");
}

#[test]
fn rule_lines_and_page_colours() {
    let mut s = sample();
    blank_page(&mut s);
    for kind in ["narrow", "college", "standard", "wide", "gridSmall", "gridMedium", "gridLarge", "gridVeryLarge", "none"] {
        run(&mut s, "view.ruleLines", json!({"kind": kind}));
    }
    assert_eq!(s.page().unwrap().setup.rule_lines, notecraft_doc::RuleLines::None, "BEH.View.RuleLines");
    run(&mut s, "view.ruleLines", json!({"kind": "college"}));
    assert_eq!(s.page().unwrap().setup.rule_lines, notecraft_doc::RuleLines::College, "BEH.View.RuleLines");
    assert_eq!(notecraft_doc::color::PAGE_COLORS.len(), 16, "BEH.View.PageColor: sixteen colours");
    let hex = notecraft_doc::color::PAGE_COLORS[3].color.to_hex();
    run(&mut s, "view.pageColor", json!({"color": hex}));
    assert_eq!(s.page().unwrap().setup.color.map(|c| c.to_hex()), Some(hex), "BEH.View.PageColor");
    run(&mut s, "view.pageColor", json!({"color": null}));
    assert_eq!(s.page().unwrap().setup.color, None, "BEH.View.PageColor: No Color");
}

#[test]
fn page_links_survive_renames_and_moves() {
    let mut s = sample();
    let target = s.page().unwrap().id;
    let link = run(&mut s, "page.copyLink", json!({}))["link"].as_str().unwrap().to_string();
    run(&mut s, "page.rename", json!({"title": "A brand new title"}));
    let from = s.section().unwrap().id;
    let to = s.notebook().unwrap().sections().iter().map(|x| x.id).find(|&x| x != from).unwrap();
    run(&mut s, "page.move", json!({"page": target.to_string(), "section": to.to_string(), "index": 0}));
    blank_page(&mut s);
    run(&mut s, "link.open", json!({"url": link}));
    assert_eq!(s.page().unwrap().id, target, "BEH.Links.PageRenameTracking");
    assert_eq!(s.page().unwrap().title.text(), "A brand new title");
}
