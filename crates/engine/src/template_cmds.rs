//! Page templates (decision D10, `docs/plan.md` §4.15): NoteCraft's own built-in templates (their
//! content is NoteCraft's, not the reference's), the user's own ("My Templates", kept with the
//! preferences), the recently used ones, and each section's default template, which new pages
//! of that section start from.
//!
//! A template id is `builtin:<slug>` or `my:<name>`.

use notecraft_doc::color::Rgba;
use notecraft_doc::geom::Point;
use notecraft_doc::tags::TagUse;
use notecraft_doc::{Id, Outline, Page, PageObject, ParaStyle, Paragraph, RuleLines, Table};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ops::{self, DEFAULT_LEFT, need_page, no_page};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// Most templates of the user's own.
pub const MAX_TEMPLATES: usize = 200;
/// Longest template name, in characters.
pub const MAX_NAME: usize = 100;
/// Recently used templates remembered.
pub const MAX_RECENT: usize = 5;
/// The category of the user's own templates.
pub const MY_TEMPLATES: &str = "My Templates";

/// A template of the user's own: a page saved by name (pictures and attachments are left out).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedTemplate {
    /// The name shown in the Templates pane.
    pub name: String,
    /// The page new pages copy.
    pub page: Page,
}

/// One built-in template.
pub struct BuiltIn {
    /// Its id after `builtin:`.
    pub slug: &'static str,
    /// Its name.
    pub name: &'static str,
    /// Its category in the Templates pane.
    pub category: &'static str,
    /// One line about it.
    pub description: &'static str,
}

/// The built-in templates, by category (NoteCraft's own set and content).
pub const BUILT_IN: &[BuiltIn] = &[
    BuiltIn { slug: "lecture-notes", name: "Lecture notes", category: "Academic", description: "Ruled page with Key points, Questions and Summary" },
    BuiltIn { slug: "study-plan", name: "Study plan", category: "Academic", description: "A table of subjects, topics and dates" },
    BuiltIn { slug: "blank", name: "Blank", category: "Blank", description: "An empty page" },
    BuiltIn { slug: "lined", name: "Lined", category: "Blank", description: "College-ruled lines" },
    BuiltIn { slug: "small-grid", name: "Small grid", category: "Blank", description: "A grid of 1/8 in squares" },
    BuiltIn { slug: "large-grid", name: "Large grid", category: "Blank", description: "A grid of 3/8 in squares" },
    BuiltIn {
        slug: "meeting-notes",
        name: "Meeting notes",
        category: "Business",
        description: "Attendees, Agenda, Notes and Action items to check off",
    },
    BuiltIn {
        slug: "project-overview",
        name: "Project overview",
        category: "Business",
        description: "Goal, and a table of tasks, owners, dates and status",
    },
    BuiltIn { slug: "sky", name: "Sky", category: "Decorative", description: "A light blue page" },
    BuiltIn { slug: "sand", name: "Sand", category: "Decorative", description: "A warm beige page" },
    BuiltIn { slug: "mint", name: "Mint", category: "Decorative", description: "A light green page" },
    BuiltIn { slug: "to-do-list", name: "To-do list", category: "Planners", description: "To Do items for today and this week" },
    BuiltIn { slug: "weekly-planner", name: "Weekly planner", category: "Planners", description: "A table with a row for each day" },
];

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new(
            "insert.pageTemplates",
            "R.Insert.Pages.PageTemplatesSplitButton",
            "Page Templates",
            "Insert/Pages/Page Templates",
            |s, v| {
                s.view.templates_pane = p::bool(v, "on").unwrap_or(!s.view.templates_pane);
                Ok(json!({"open": s.view.templates_pane}))
            },
        )
        .params(r#"{"on"?: bool} (the Templates pane; default: switch)"#)
        .pure(),
        CommandSpec::new(
            "page.newFromTemplate",
            "R.Insert.Pages.NewPageFromStationery",
            "Page from Template",
            "Insert/Pages/Page Templates/Page from Template",
            new_from_template,
        )
        .params(r#"{"template"?: "builtin:<slug>" | "my:<name>" (default: the section's default template, else a blank page), "section"?: id}"#)
        .when(need_section),
        CommandSpec::new(
            "insert.recentTemplate",
            "R.Insert.Pages.StationeryMostRecentlyUsedList",
            "Recently Used Templates",
            "Insert/Pages/Page Templates/Recently Used Templates",
            |s, v| {
                let i = p::usize(v, "index").unwrap_or(0);
                let id = s.prefs.recent_templates.get(i).cloned().ok_or_else(|| CmdError::Params("no recently used template there".into()))?;
                new_from_template(s, &json!({"template": id}))
            },
        )
        .params(r#"{"index"?: int (default 0, the latest)}"#)
        .when(need_recent),
        CommandSpec::new("template.list", "NC.Templates.List", "List Templates", "Window/Panes/Templates", list)
            .params(r#"{} → {"templates": [{"id", "name", "category", "description"}], "default": id | null, "recent": [id]}"#)
            .pure(),
        CommandSpec::new(
            "template.setDefault",
            "NC.Templates.SetDefault",
            "Choose Default Template",
            "Window/Panes/Templates/Choose default template",
            set_default,
        )
        .params(r#"{"template": id | null (none), "section"?: id (default: the current one)}"#)
        .when(need_section),
        CommandSpec::new(
            "template.save",
            "NC.Templates.Save",
            "Save Current Page as a Template",
            "Window/Panes/Templates/Save current page as a template",
            save,
        )
        .params(r#"{"name": string, "default"?: bool (also the section's default)}"#)
        .when(need_page),
        CommandSpec::new("template.delete", "NC.Templates.Delete", "Delete Template", "Window/Panes/Templates/My Templates/Delete", delete)
            .params(r#"{"template": "my:<name>"}"#)
            .pure(),
    ]
}

fn need_section(s: &Session) -> Option<&'static str> {
    if s.sel.section.is_none() { Some("Open a section first") } else { None }
}

fn need_recent(s: &Session) -> Option<&'static str> {
    if s.sel.section.is_none() {
        return Some("Open a section first");
    }
    if s.prefs.recent_templates.is_empty() { Some("No template used yet") } else { None }
}

/// Every template: the built-in ones by category, then the user's own.
fn list(s: &mut Session, _: &Value) -> CmdResult {
    let mut all: Vec<Value> = BUILT_IN
        .iter()
        .map(|b| json!({"id": format!("builtin:{}", b.slug), "name": b.name, "category": b.category, "description": b.description}))
        .collect();
    all.extend(
        s.prefs.templates.iter().map(|t| json!({"id": format!("my:{}", t.name), "name": t.name, "category": MY_TEMPLATES, "description": ""})),
    );
    let default = s.sel.section.and_then(|id| s.ws.section(id)).and_then(|sec| sec.default_template.clone());
    Ok(json!({"templates": all, "default": default, "recent": s.prefs.recent_templates}))
}

/// Whether `id` names a template that exists.
fn exists(s: &Session, id: &str) -> bool {
    match id.split_once(':') {
        Some(("builtin", slug)) => BUILT_IN.iter().any(|b| b.slug == slug),
        Some(("my", name)) => s.prefs.templates.iter().any(|t| t.name == name),
        _ => false,
    }
}

/// A new page from the template `id` (ids come from the session; nothing is inserted).
pub(crate) fn page_from(s: &mut Session, id: &str) -> Result<Page, CmdError> {
    match id.split_once(':') {
        Some(("builtin", slug)) if BUILT_IN.iter().any(|b| b.slug == slug) => Ok(built_in(s, slug)),
        Some(("my", name)) => {
            let tpl = s.prefs.templates.iter().find(|t| t.name == name).map(|t| t.page.clone()).ok_or_else(|| unknown(id))?;
            let now = s.now();
            let fresh = ops::clone_page_fresh(s, &tpl, now);
            let mut page = crate::page_cmds::make_page(s, "");
            page.setup = fresh.setup.clone();
            page.objects = fresh.objects;
            if !fresh.title.is_empty() {
                page.title = Paragraph { id: page.title.id, ..fresh.title };
            }
            Ok(page)
        }
        _ => Err(unknown(id)),
    }
}

fn unknown(id: &str) -> CmdError {
    CmdError::Params(format!("no template `{id}` (see template.list)"))
}

/// A new page for `section`: from its default template when it has one (and it still exists),
/// else blank; `title` replaces the template's.
pub(crate) fn new_page_in(s: &mut Session, section: Id, title: &str) -> Page {
    let default = s.ws.section(section).and_then(|sec| sec.default_template.clone()).filter(|id| exists(s, id));
    let mut page = match default.map(|id| page_from(s, &id)) {
        Some(Ok(p)) => p,
        _ => crate::page_cmds::make_page(s, ""),
    };
    if !title.is_empty() {
        page.title = Paragraph::with_text(page.title.id, title);
        page.title.style = ParaStyle::PageTitle;
    }
    page
}

fn new_from_template(s: &mut Session, v: &Value) -> CmdResult {
    let section = match p::id(v, "section")? {
        Some(id) => id,
        None => s.sel.section.ok_or_else(|| CmdError::Disabled("open a section first".into()))?,
    };
    let sec = s.ws.section(section).ok_or_else(|| CmdError::Params("no such section".into()))?;
    let id = match p::str(v, "template") {
        Some(id) => Some(id.to_string()),
        None => sec.default_template.clone().filter(|id| exists(s, id)),
    };
    let index = sec.pages.len();
    let page = match &id {
        Some(id) => page_from(s, id)?,
        None => crate::page_cmds::make_page(s, ""),
    };
    if let Some(id) = id {
        s.prefs.recent_templates.retain(|r| *r != id);
        s.prefs.recent_templates.insert(0, id);
        s.prefs.recent_templates.truncate(MAX_RECENT);
    }
    crate::page_cmds::insert_and_open(s, section, index, page)
}

fn set_default(s: &mut Session, v: &Value) -> CmdResult {
    let section = match p::id(v, "section")? {
        Some(id) => id,
        None => s.sel.section.ok_or_else(|| CmdError::Disabled("open a section first".into()))?,
    };
    let id = match v.get("template") {
        None | Some(Value::Null) => None,
        Some(Value::String(id)) if exists(s, id) => Some(id.clone()),
        Some(Value::String(id)) => return Err(unknown(id)),
        Some(_) => return Err(CmdError::Params("`template` must be a template id or null".into())),
    };
    let sec = s.ws.section_mut(section).ok_or_else(|| CmdError::Params("no such section".into()))?;
    sec.default_template = id.clone();
    Ok(json!({"default": id}))
}

fn save(s: &mut Session, v: &Value) -> CmdResult {
    let name: String = p::str(v, "name").unwrap_or("").trim().chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
    if name.is_empty() {
        return Err(CmdError::Params("give the template a name".into()));
    }
    let mut page = (**s.page().ok_or_else(no_page)?).clone();
    // Pictures and attachments stay on the page: a template keeps text, tables and ink.
    page.objects.retain(|o| !matches!(o, PageObject::Image(_) | PageObject::Attachment(_)));
    page.level = 0;
    let replacing = s.prefs.templates.iter().any(|t| t.name == name);
    if !replacing && s.prefs.templates.len() >= MAX_TEMPLATES {
        return Err(CmdError::Failed(format!("at most {MAX_TEMPLATES} templates of your own; delete one first")));
    }
    s.prefs.templates.retain(|t| t.name != name);
    s.prefs.templates.push(SavedTemplate { name: name.clone(), page });
    s.prefs.templates.sort_by_key(|t| t.name.to_lowercase());
    let id = format!("my:{name}");
    if p::bool(v, "default") == Some(true)
        && let Some(sec) = s.sel.section.and_then(|sid| s.ws.section_mut(sid))
    {
        sec.default_template = Some(id.clone());
    }
    s.notice(format!("Saved the page as the template “{name}”."));
    Ok(json!({"template": id}))
}

fn delete(s: &mut Session, v: &Value) -> CmdResult {
    let id = p::str(v, "template").unwrap_or("");
    let Some(("my", name)) = id.split_once(':') else { return Err(CmdError::Params("only your own templates (`my:<name>`) can be deleted".into())) };
    let before = s.prefs.templates.len();
    s.prefs.templates.retain(|t| t.name != name);
    if s.prefs.templates.len() == before {
        return Err(unknown(id));
    }
    s.prefs.recent_templates.retain(|r| r != id);
    Ok(Value::Null)
}

/// Build a built-in template's page.
fn built_in(s: &mut Session, slug: &str) -> Page {
    let mut page = crate::page_cmds::make_page(s, "");
    let mut b = Build { s, page: &mut page, y: 90.0 };
    match slug {
        "lined" => b.page.setup.rule_lines = RuleLines::College,
        "small-grid" => b.page.setup.rule_lines = RuleLines::GridSmall,
        "large-grid" => b.page.setup.rule_lines = RuleLines::GridLarge,
        "sky" => b.page.setup.color = Some(Rgba::rgb(0xDD, 0xEE, 0xFA)),
        "sand" => b.page.setup.color = Some(Rgba::rgb(0xF6, 0xEE, 0xDD)),
        "mint" => b.page.setup.color = Some(Rgba::rgb(0xE2, 0xF4, 0xE6)),
        "lecture-notes" => {
            b.page.setup.rule_lines = RuleLines::College;
            let paras = vec![
                b.heading("Topic"),
                b.text(""),
                b.heading("Key points"),
                b.text(""),
                b.heading("Questions"),
                b.tagged("question", ""),
                b.heading("Summary"),
                b.text(""),
            ];
            b.outline(paras);
        }
        "study-plan" => {
            let title = vec![b.heading("Study plan")];
            b.outline(title);
            b.table(&["Subject", "Topic", "Date", "Done"], 4, &[120.0, 180.0, 90.0, 60.0]);
        }
        "meeting-notes" => {
            let paras = vec![
                b.heading("Attendees"),
                b.text(""),
                b.heading("Agenda"),
                b.text(""),
                b.heading("Notes"),
                b.text(""),
                b.heading("Action items"),
                b.tagged("todo", ""),
                b.tagged("todo", ""),
            ];
            b.outline(paras);
        }
        "project-overview" => {
            let goal = vec![b.heading("Goal"), b.text(""), b.heading("Tasks")];
            b.outline(goal);
            b.table(&["Task", "Owner", "Due", "Status"], 4, &[180.0, 100.0, 80.0, 90.0]);
        }
        "to-do-list" => {
            let paras = vec![
                b.heading("Today"),
                b.tagged("todo", ""),
                b.tagged("todo", ""),
                b.tagged("todo", ""),
                b.heading("This week"),
                b.tagged("todo", ""),
                b.tagged("todo", ""),
            ];
            b.outline(paras);
        }
        "weekly-planner" => {
            let days = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
            let t = b.table(&["Day", "Plans", "Notes"], days.len(), &[100.0, 240.0, 180.0]);
            if let Some(PageObject::Table(tb)) = b.page.objects.iter_mut().find(|o| o.id() == t) {
                for (row, day) in tb.rows.iter_mut().skip(1).zip(days) {
                    if let Some(p) = row.cells.first_mut().and_then(|c| c.paragraphs.first_mut()) {
                        *p = Paragraph::with_text(p.id, day);
                    }
                }
            }
        }
        _ => {}
    }
    page
}

/// Builds a template page top to bottom.
struct Build<'a> {
    s: &'a mut Session,
    page: &'a mut Page,
    y: f32,
}

impl Build<'_> {
    fn heading(&mut self, text: &str) -> Paragraph {
        let mut p = Paragraph::with_text(self.s.new_id(), text);
        p.style = ParaStyle::Heading2;
        p
    }

    fn text(&mut self, text: &str) -> Paragraph {
        Paragraph::with_text(self.s.new_id(), text)
    }

    fn tagged(&mut self, key: &str, text: &str) -> Paragraph {
        let mut p = self.text(text);
        if let Some(def) = self.s.tag_defs().into_iter().find(|d| d.key == key) {
            p.tags.push(TagUse::of(&def));
        }
        p
    }

    /// A container of `paras` below what is already there.
    fn outline(&mut self, paras: Vec<Paragraph>) {
        let mut ol = Outline::new(self.s.new_id(), Point::new(DEFAULT_LEFT, self.y), self.s.new_id());
        let n = paras.len();
        ol.paragraphs = paras;
        ol.normalize();
        // About one line a paragraph, plus the heading's extra height (layout decides the rest).
        self.y += n as f32 * 22.0 + 16.0;
        self.page.objects.push(PageObject::Outline(ol));
    }

    /// A table with a header row of `header` and `rows` empty rows below it; returns its id.
    fn table(&mut self, header: &[&str], rows: usize, widths: &[f32]) -> Id {
        let id = self.s.new_id();
        let s = &mut *self.s;
        let mut t = Table::new(id, Point::new(DEFAULT_LEFT, self.y), rows + 1, header.len(), &mut || s.new_id());
        for (w, x) in t.columns.iter_mut().zip(widths) {
            *w = *x;
        }
        if let Some(first) = t.rows.first_mut() {
            for (cell, text) in first.cells.iter_mut().zip(header) {
                if let Some(p) = cell.paragraphs.first_mut() {
                    *p = Paragraph::with_text(p.id, text);
                    for r in &mut p.runs {
                        r.fmt.bold = Some(true);
                    }
                }
                cell.shading = Some(Rgba::rgb(0xE8, 0xEE, 0xF6));
            }
        }
        t.header_row = true;
        t.normalize();
        self.y += (rows + 1) as f32 * 24.0 + 20.0;
        self.page.objects.push(PageObject::Table(t));
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{run, sample};

    #[test]
    fn every_built_in_template_builds_a_page() {
        let mut s = sample();
        for b in BUILT_IN {
            let p = page_from(&mut s, &format!("builtin:{}", b.slug)).unwrap();
            assert!(p.title.is_empty(), "{}: built-in templates leave the title to the user", b.slug);
        }
        assert!(page_from(&mut s, "builtin:nope").is_err());
        assert!(page_from(&mut s, "nope").is_err());
        let _ = run(&mut s, "template.list", json!({}));
    }

    fn texts(s: &Session) -> Vec<String> {
        s.page().unwrap().objects.iter().filter_map(PageObject::as_outline).flat_map(|o| o.paragraphs.iter().map(Paragraph::text)).collect()
    }

    #[test]
    fn pages_come_from_templates_and_sections_have_a_default_one() {
        // parity: R.Insert.Pages.PageTemplatesSplitButton, R.Insert.Pages.NewPageFromStationery, NC.Templates.List
        let mut s = sample();
        assert_eq!(run(&mut s, "insert.pageTemplates", json!({}))["open"], true);
        assert!(s.view.templates_pane);
        let all = run(&mut s, "template.list", json!({}));
        assert_eq!(all["templates"].as_array().unwrap().len(), BUILT_IN.len());
        // Meeting notes: headings and two To Do items, a blank title.
        run(&mut s, "page.newFromTemplate", json!({"template": "builtin:meeting-notes"}));
        assert!(texts(&s).contains(&"Action items".to_string()), "{:?}", texts(&s));
        let todos = s
            .page()
            .unwrap()
            .objects
            .iter()
            .filter_map(PageObject::as_outline)
            .flat_map(|o| o.paragraphs.iter())
            .filter(|p| p.tags.iter().any(|t| t.key == "todo"))
            .count();
        assert_eq!(todos, 2);
        assert!(s.page().unwrap().title.is_empty());
        // The weekly planner's table has a row per day under its header.
        run(&mut s, "page.newFromTemplate", json!({"template": "builtin:weekly-planner"}));
        let t = s.page().unwrap().objects.iter().find_map(PageObject::as_table).unwrap().clone();
        assert_eq!((t.n_rows(), t.header_row, t.cell(7, 0).map(|c| c.text())), (8, true, Some("Sunday".to_string())));
        assert_eq!(s.prefs.recent_templates, vec!["builtin:weekly-planner", "builtin:meeting-notes"]);
        // A section's default template: New Page starts from it, a title given replaces its own.
        run(&mut s, "template.setDefault", json!({"template": "builtin:lined"}));
        run(&mut s, "page.new", json!({"title": "Notes"}));
        assert_eq!((s.page().unwrap().setup.rule_lines, s.page().unwrap().display_title()), (RuleLines::College, "Notes".to_string()));
        run(&mut s, "page.newBelow", json!({}));
        assert_eq!(s.page().unwrap().setup.rule_lines, RuleLines::College);
        assert!(s.run("template.setDefault", json!({"template": "builtin:nope"})).is_err());
        // The current page as a template of the user's own, with fresh ids each time.
        run(&mut s, "text.insert", json!({"text": "Plan"}));
        run(&mut s, "outline.new", json!({"x": 40.0, "y": 120.0}));
        run(&mut s, "text.insert", json!({"text": "kept"}));
        assert_eq!(run(&mut s, "template.save", json!({"name": "  Mine\n"}))["template"], "my:Mine");
        let a = run(&mut s, "page.newFromTemplate", json!({"template": "my:Mine"}))["page"].clone();
        assert!(texts(&s).contains(&"kept".to_string()) && s.page().unwrap().display_title() == "Plan");
        run(&mut s, "insert.recentTemplate", json!({}));
        assert_ne!(run(&mut s, "template.list", json!({}))["recent"][0], json!(null));
        let ids: Vec<_> = s.page().unwrap().objects.iter().map(|o| o.id()).collect();
        run(&mut s, "nav.goto", json!({"page": a}));
        assert!(s.page().unwrap().objects.iter().all(|o| !ids.contains(&o.id())), "each page gets its own ids");
        // Deleting it: a section defaulting to it falls back to a blank page.
        run(&mut s, "template.setDefault", json!({"template": "my:Mine"}));
        run(&mut s, "template.delete", json!({"template": "my:Mine"}));
        run(&mut s, "page.new", json!({}));
        assert!(texts(&s).is_empty() || texts(&s).iter().all(String::is_empty));
        // Hostile input.
        assert!(s.run("template.save", json!({"name": "   "})).is_err());
        assert!(s.run("template.delete", json!({"template": "builtin:blank"})).is_err());
        assert!(s.run("page.newFromTemplate", json!({"template": 7})).is_ok(), "a non-string template is ignored: the default (none) applies");
        assert!(s.run("insert.recentTemplate", json!({"index": 99})).is_err());
    }
}
