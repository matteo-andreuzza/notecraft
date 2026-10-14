//! Navigation: Back/Forward, next/previous page and section, jumping to the title or the next
//! container, following `notecraft:` links, and the focus shortcuts.

use notecraft_doc::Id;
use serde_json::{Value, json};

use crate::ops::{need_page, no_page};
use crate::selection::{Block, FocusArea, Pos};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn need_section(s: &Session) -> Option<&'static str> {
    if s.section().is_some() { None } else { Some("Select a section first") }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // QAT.ViewGoBack (Back on the Quick Access Toolbar) — Alt+Left.
        CommandSpec::new("nav.back", "QAT.ViewGoBack", "Back", "Quick Access Toolbar/Back", back)
            .key("Alt+Left")
            .pure()
            .when(|s| if s.nav_back.is_empty() { Some("No previous page") } else { None }),
        CommandSpec::new("nav.forward", "QAT.ViewGoForward", "Forward", "Quick Access Toolbar/Forward", forward)
            .key("Alt+Right")
            .pure()
            .when(|s| if s.nav_forward.is_empty() { Some("No next page") } else { None }),
        CommandSpec::new(
            "nav.nextPage",
            "QAT.NR.NextPageOneNote",
            "Next Page",
            "Quick Access Toolbar/Commands Not in the Ribbon/Next Page",
            |s, _| page_step(s, 1),
        )
        .key("Mod+PageDown")
        .pure()
        .when(need_section),
        CommandSpec::new(
            "nav.prevPage",
            "QAT.NR.PreviousPageOneNote",
            "Previous Page",
            "Quick Access Toolbar/Commands Not in the Ribbon/Previous Page",
            |s, _| page_step(s, -1),
        )
        .key("Mod+PageUp")
        .pure()
        .when(need_section),
        CommandSpec::new(
            "nav.firstPage",
            "KB.Notebooks.FirstPage",
            "First Page",
            "Keyboard/Notebooks and sections/Go to the first page in the section",
            |s, _| page_edge(s, true),
        )
        .key("Alt+Home")
        .pure()
        .when(need_section),
        CommandSpec::new(
            "nav.lastPage",
            "KB.Notebooks.LastPage",
            "Last Page",
            "Keyboard/Notebooks and sections/Go to the last page in the section",
            |s, _| page_edge(s, false),
        )
        .key("Alt+End")
        .pure()
        .when(need_section),
        // KB.Pages.FirstVisible / LastVisible — the page list shows every page, so these equal
        // first/last page until the list scrolls (the UI passes the visible range).
        CommandSpec::new(
            "nav.firstVisiblePage",
            "KB.Pages.FirstVisible",
            "First Visible Page",
            "Keyboard/Pages/Go to the first page in the visible set",
            |s, v| visible_edge(s, v, true),
        )
        .key("Alt+PageUp")
        .params(r#"{"first"?: page id}"#)
        .pure()
        .when(need_section),
        CommandSpec::new(
            "nav.lastVisiblePage",
            "KB.Pages.LastVisible",
            "Last Visible Page",
            "Keyboard/Pages/Go to the last page in the visible set",
            |s, v| visible_edge(s, v, false),
        )
        .key("Alt+PageDown")
        .params(r#"{"last"?: page id}"#)
        .pure()
        .when(need_section),
        CommandSpec::new(
            "nav.nextSection",
            "KB.Notebooks.NextSection",
            "Next Section",
            "Keyboard/Notebooks and sections/Go to the next section",
            |s, _| section_step(s, 1),
        )
        .key("Mod+Tab")
        .pure()
        .when(need_section),
        CommandSpec::new(
            "nav.prevSection",
            "KB.Notebooks.PrevSection",
            "Previous Section",
            "Keyboard/Notebooks and sections/Go to the previous section",
            |s, _| section_step(s, -1),
        )
        .key("Mod+Shift+Tab")
        .pure()
        .when(need_section),
        // KB.Select.Title — Ctrl+Shift+T selects the title.
        CommandSpec::new("nav.pageTitle", "KB.Select.Title", "Go to Page Title", "Keyboard/Select/Jump to the page title and select it", page_title)
            .key("Mod+Shift+T")
            .pure()
            .when(need_page),
        CommandSpec::new(
            "nav.nextContainer",
            "KB.Pages.NextContainer",
            "Next Note Container",
            "Keyboard/Pages/Go to the next note container",
            next_container,
        )
        .key("Alt+Down")
        .pure()
        .when(need_page),
        CommandSpec::new(
            "nav.focusPageTab",
            "KB.Notebooks.FocusPageTab",
            "Focus Page Tab",
            "Keyboard/Notebooks and sections/Put focus on the current page tab",
            |s, _| focus(s, FocusArea::PageList),
        )
        .key("Mod+Alt+G")
        .pure(),
        CommandSpec::new(
            "nav.focusSectionTab",
            "KB.Notebooks.FocusSectionTab",
            "Focus Section Tab",
            "Keyboard/Notebooks and sections/Put focus on the current section tab",
            |s, _| focus(s, FocusArea::SectionTabs),
        )
        .key("Mod+Shift+G")
        .pure(),
        CommandSpec::new(
            "nav.notebookList",
            "KB.Notebooks.SwitchNotebook",
            "Switch Notebook",
            "Keyboard/Notebooks and sections/Switch notebook on the navigation bar",
            |s, _| focus(s, FocusArea::NotebookPane),
        )
        .key("Mod+G")
        .pure(),
        // NC.Page.Select — click a page tab.
        CommandSpec::new("nav.goto", "NC.Page.Select", "Go to Page", "Mouse/Page tab", goto)
            .params(r#"{"page"?: id, "section"?: id, "add"?: bool, "range"?: bool}"#)
            .pure(),
        // KB.Format.OpenLink — Enter on a link, or a click: notecraft: links navigate, others are
        // handed back to the UI to open in the browser.
        CommandSpec::new(
            "link.open",
            "KB.Format.OpenLink",
            "Open Link",
            "Keyboard/Format notes/Open a hyperlink (Enter on the link text)",
            open_link,
        )
        .key("Enter")
        .params(r#"{"url"?: string}"#)
        .pure(),
    ]
}

fn back(s: &mut Session, _: &Value) -> CmdResult {
    while let Some(prev) = s.nav_back.pop() {
        if let Some((sid, _)) = s.ws.locate_page(prev) {
            if let Some(cur) = s.sel.page {
                s.last_sel.insert(cur, s.sel.on_page.clone());
                s.nav_forward.push(cur);
            }
            let nb = s.ws.notebook_of(sid).map(|n| n.id);
            s.show_page(nb, sid, prev);
            return Ok(Value::Null);
        }
    }
    Err(CmdError::Disabled("no previous page".into()))
}

fn forward(s: &mut Session, _: &Value) -> CmdResult {
    while let Some(next) = s.nav_forward.pop() {
        if let Some((sid, _)) = s.ws.locate_page(next) {
            if let Some(cur) = s.sel.page {
                s.last_sel.insert(cur, s.sel.on_page.clone());
                s.nav_back.push(cur);
            }
            let nb = s.ws.notebook_of(sid).map(|n| n.id);
            s.show_page(nb, sid, next);
            return Ok(Value::Null);
        }
    }
    Err(CmdError::Disabled("no next page".into()))
}

/// Visible pages of the current section (collapsed subpages skipped).
fn visible_pages(s: &Session) -> Vec<Id> {
    let Some(sec) = s.section() else { return Vec::new() };
    (0..sec.pages.len()).filter(|&i| !sec.page_hidden(i)).filter_map(|i| sec.pages.get(i).map(|p| p.id)).collect()
}

fn page_step(s: &mut Session, d: i32) -> CmdResult {
    let pages = visible_pages(s);
    let cur = s.sel.page.and_then(|p| pages.iter().position(|x| *x == p));
    let next = match cur {
        Some(i) => {
            let j = i as i64 + d as i64;
            if j < 0 || j as usize >= pages.len() {
                return Ok(Value::Null);
            }
            pages.get(j as usize).copied()
        }
        None => pages.first().copied(),
    };
    if let Some(n) = next {
        s.goto_page(n)?;
    }
    Ok(Value::Null)
}

fn page_edge(s: &mut Session, first: bool) -> CmdResult {
    let pages = visible_pages(s);
    let target = if first { pages.first() } else { pages.last() };
    if let Some(t) = target.copied() {
        s.goto_page(t)?;
    }
    Ok(Value::Null)
}

fn visible_edge(s: &mut Session, v: &Value, first: bool) -> CmdResult {
    let hint = p::id(v, if first { "first" } else { "last" })?;
    match hint {
        Some(h) if s.ws.locate_page(h).is_some() => {
            s.goto_page(h)?;
            Ok(Value::Null)
        }
        _ => page_edge(s, first),
    }
}

fn section_step(s: &mut Session, d: i32) -> CmdResult {
    let nb = s.notebook().ok_or_else(no_page)?;
    let ids: Vec<Id> = nb.sections().iter().map(|x| x.id).collect();
    let Some(cur) = s.sel.section.and_then(|c| ids.iter().position(|x| *x == c)) else { return Ok(Value::Null) };
    let n = ids.len() as i64;
    if n == 0 {
        return Ok(Value::Null);
    }
    // Ctrl+Tab wraps around the notebook's sections.
    let j = (cur as i64 + d as i64).rem_euclid(n) as usize;
    if let Some(next) = ids.get(j).copied() {
        s.goto_section(next)?;
    }
    Ok(Value::Null)
}

fn page_title(s: &mut Session, _: &Value) -> CmdResult {
    let len = s.page().map(|p| p.title.len()).ok_or_else(no_page)?;
    s.sel.focus = FocusArea::Canvas;
    s.set_range(Pos::new(Block::Title, 0, 0), Pos::new(Block::Title, 0, len));
    Ok(Value::Null)
}

fn next_container(s: &mut Session, _: &Value) -> CmdResult {
    let page = s.page().ok_or_else(no_page)?;
    let ids: Vec<Id> = page.objects.iter().filter_map(|o| o.as_outline().map(|ol| ol.id)).collect();
    if ids.is_empty() {
        return Ok(Value::Null);
    }
    let cur = match s.sel.caret() {
        Some(Pos { block: Block::Outline(id), .. }) => ids.iter().position(|x| *x == id),
        _ => None,
    };
    let next = match cur {
        Some(i) => ids.get((i + 1) % ids.len()),
        None => ids.first(),
    };
    if let Some(n) = next.copied() {
        s.set_caret(Pos::new(Block::Outline(n), 0, 0));
    }
    Ok(Value::Null)
}

fn focus(s: &mut Session, f: FocusArea) -> CmdResult {
    s.sel.focus = f;
    Ok(Value::Null)
}

fn goto(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(page) = p::id(v, "page")? {
        let add = p::bool(v, "add").unwrap_or(false);
        let range = p::bool(v, "range").unwrap_or(false);
        if (add || range) && s.sel.page.is_some() {
            // Ctrl+click adds to the page selection; Shift+click selects a range.
            let sec = s.section().ok_or_else(no_page)?;
            if range {
                let a = s.sel.page.and_then(|p| sec.page_index(p));
                let b = sec.page_index(page);
                if let (Some(a), Some(b)) = (a, b) {
                    let (lo, hi) = (a.min(b), a.max(b));
                    s.sel.pages = (lo..=hi).filter_map(|i| sec.pages.get(i).map(|p| p.id)).collect();
                }
            } else if !s.sel.pages.contains(&page) {
                if s.sel.pages.is_empty()
                    && let Some(cur) = s.sel.page
                {
                    s.sel.pages.push(cur);
                }
                s.sel.pages.push(page);
            }
            s.sel.focus = FocusArea::PageList;
            return Ok(Value::Null);
        }
        s.goto_page(page)?;
        s.sel.focus = FocusArea::PageList;
        return Ok(Value::Null);
    }
    if let Some(section) = p::id(v, "section")? {
        s.goto_section(section)?;
        return Ok(Value::Null);
    }
    Err(CmdError::Params("give `page` or `section`".into()))
}

/// Parse a `notecraft:` link: `notecraft:page/<id>`, `notecraft:section/<id>`,
/// `notecraft:group/<id>`, `notecraft:notebook/<id>`, `notecraft:para/<page>/<para id>`.
pub(crate) fn parse_link(url: &str) -> Option<(&str, Vec<Id>)> {
    let rest = url.strip_prefix("notecraft:")?;
    let mut parts = rest.split('/');
    let kind = parts.next()?;
    let ids: Option<Vec<Id>> = parts.map(Id::parse).collect();
    Some((kind, ids?))
}

fn link_at_caret(s: &Session) -> Option<String> {
    let c = s.sel.caret()?;
    let page = s.page()?;
    let para = Session::block_paras(page, c.block)?.get(c.para)?;
    // The run under the caret, or the one just before it.
    para.fmt_at(c.offset).link.or_else(|| para.fmt_at(c.offset + 1).link)
}

fn open_link(s: &mut Session, v: &Value) -> CmdResult {
    let url = match p::str(v, "url") {
        Some(u) => u.to_string(),
        None => link_at_caret(s).ok_or_else(|| CmdError::Disabled("the caret is not on a link".into()))?,
    };
    match parse_link(&url) {
        Some(("page", ids)) => {
            let page = *ids.first().ok_or_else(|| CmdError::Params("bad link".into()))?;
            s.goto_page(page)?;
            Ok(json!({"navigated": true}))
        }
        Some(("section" | "group", ids)) => {
            let id = *ids.first().ok_or_else(|| CmdError::Params("bad link".into()))?;
            let first =
                s.ws.section(id).map(|x| x.id).or_else(|| s.ws.notebook_of(id).and_then(|n| n.group(id)).and_then(|g| first_section(&g.children)));
            match first {
                Some(sid) => s.goto_section(sid)?,
                None => return Err(CmdError::Failed("the linked section no longer exists".into())),
            }
            Ok(json!({"navigated": true}))
        }
        Some(("notebook", ids)) => {
            let id = *ids.first().ok_or_else(|| CmdError::Params("bad link".into()))?;
            let first =
                s.ws.notebook(id)
                    .and_then(|n| n.sections().first().map(|x| x.id))
                    .ok_or_else(|| CmdError::Failed("the linked notebook is not open".into()))?;
            s.goto_section(first)?;
            Ok(json!({"navigated": true}))
        }
        Some(("para", ids)) => {
            let page = *ids.first().ok_or_else(|| CmdError::Params("bad link".into()))?;
            let para = *ids.get(1).ok_or_else(|| CmdError::Params("bad link".into()))?;
            s.goto_page(page)?;
            let found =
                s.page().and_then(|p| p.objects.iter().find_map(|o| o.as_outline().and_then(|ol| ol.paragraph_index(para).map(|i| (ol.id, i)))));
            if let Some((oid, i)) = found {
                s.set_caret(Pos::new(Block::Outline(oid), i, 0));
                s.scroll_request = Some(crate::ScrollRequest::Caret);
            }
            Ok(json!({"navigated": true}))
        }
        Some(_) => Err(CmdError::Params("unknown notecraft link".into())),
        None => {
            // External links are opened by the UI (system browser); never by the engine.
            let lower = url.to_ascii_lowercase();
            if ["http://", "https://", "mailto:", "ftp://", "file://"].iter().any(|p| lower.starts_with(p)) {
                Ok(json!({"open_url": url}))
            } else {
                Err(CmdError::Params("unsupported link".into()))
            }
        }
    }
}

fn first_section(nodes: &[notecraft_doc::NavNode]) -> Option<Id> {
    for n in nodes {
        match n {
            notecraft_doc::NavNode::Section(s) => return Some(s.id),
            notecraft_doc::NavNode::Group(g) => {
                if let Some(x) = first_section(&g.children) {
                    return Some(x);
                }
            }
        }
    }
    None
}
