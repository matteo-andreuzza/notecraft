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
