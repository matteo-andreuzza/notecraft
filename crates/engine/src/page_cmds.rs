//! Pages: create, rename, delete (to the Notebook Recycle Bin), move or copy, subpages,
//! collapse, reorder, links (the page list and its context menu).

use std::sync::Arc;

use notecraft_doc::{DeletedPage, Id, Page, Paragraph, Section};
use serde_json::{Value, json};

use crate::ops::{self, no_page};
use crate::selection::{Block, FocusArea, PageSel, Pos};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn need_section(s: &Session) -> Option<&'static str> {
    if s.sel.section.is_some() && s.section().is_some() { None } else { Some("Open a section first") }
}

fn need_page_sel(s: &Session) -> Option<&'static str> {
    if s.page().is_some() { None } else { Some("No page is selected") }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // UI.Nav.SortPages — the sort button beside Add page (2024 UI): reorders the section's
        // pages, each page keeping its subpages under it, in one undo step. Titles A→Z, dates
        // newest first unless `descending` says otherwise (unverified).
        CommandSpec::new("page.sort", "UI.Nav.SortPages", "Sort Pages", "Window/Navigation/Sort Pages", sort_pages)
            .params(r#"{"by": "title" | "created" | "modified", "descending"?: bool, "section"?: id}"#)
            .when(need_section),
        // KB.Pages.NewPage — Ctrl+N adds a page at the end of the section.
        CommandSpec::new("page.new", "KB.Pages.NewPage", "New Page", "Keyboard/Pages/Add a new page at the end of the section", new_page)
            .key("Mod+N")
            .params(r#"{"title"?: string, "section"?: id, "after"?: page id}"#)
            .when(need_section),
        // KB.Pages.NewPageBelow — Ctrl+Alt+N: below the current page at the same level.
        CommandSpec::new(
            "page.newBelow",
            "KB.Pages.NewPageBelow",
            "New Page Below",
            "Keyboard/Pages/Create a new page below the current page tab at the same level",
            new_below,
        )
        .key("Mod+Alt+N")
        .when(need_page_sel),
        // KB.Pages.NewSubpage — Ctrl+Shift+Alt+N.
        CommandSpec::new(
            "page.newSubpage",
            "KB.Pages.NewSubpage",
            "New Subpage",
            "Keyboard/Pages/Create a new subpage below the current page",
            new_subpage,
        )
        .key("Mod+Shift+Alt+N")
        .when(need_page_sel),
        // QAT.NR.NewPageAfterCurrentGroup
        CommandSpec::new(
            "page.newAfterGroup",
            "QAT.NR.NewPageAfterCurrentGroup",
            "New Page After Current Group",
            "Quick Access Toolbar/Commands Not in the Ribbon/New Page After Current Group",
            new_after_group,
        )
        .when(need_page_sel),
        // CM.Page.RenamePageOneNote — sets the title text.
        CommandSpec::new("page.rename", "CM.Page.RenamePageOneNote", "Rename", "Context/Page/Rename", rename)
            .params(r#"{"title": string, "page"?: id}"#)
            .when(need_page_sel),
        // QAT.NR.PageDeleteOneNote — to the recycle bin (BEH.Recycle.DeletedPages).
        CommandSpec::new(
            "page.delete",
            "QAT.NR.PageDeleteOneNote",
            "Delete Page",
            "Quick Access Toolbar/Commands Not in the Ribbon/Delete Page",
            delete,
        )
        .params(r#"{"page"?: id}"#)
        .when(need_page_sel),
        // QAT.NR.MoveOrCopyPage — Ctrl+Alt+M; the dialog's choice.
        CommandSpec::new(
            "page.moveOrCopy",
            "QAT.NR.MoveOrCopyPage",
            "Move or Copy Page",
            "Quick Access Toolbar/Commands Not in the Ribbon/Move or Copy Page",
            move_or_copy,
        )
        .key("Mod+Alt+M")
        .params(r#"{"section": id, "copy"?: bool, "page"?: id}"#)
        .when(need_page_sel),
        // KB.Pages.Demote / Promote — Ctrl+Alt+] / [ (also Make Subpage / Promote Subpage).
        CommandSpec::new("page.demote", "KB.Pages.Demote", "Make Subpage", "Keyboard/Pages/Increase the indent level of the current page", |s, v| {
            relevel(s, v, 1)
        })
        .key("Mod+Alt+]")
        .params(r#"{"page"?: id}"#)
        .when(need_page_sel),
        CommandSpec::new(
            "page.promote",
            "KB.Pages.Promote",
            "Promote Subpage",
            "Keyboard/Pages/Decrease the indent level of the current page",
            |s, v| relevel(s, v, -1),
        )
        .key("Mod+Alt+[")
        .params(r#"{"page"?: id}"#)
        .when(need_page_sel),
        CommandSpec::new("page.collapseSubpages", "CM.Page.IndentedPagesCollapse", "Collapse Subpages", "Context/Page/Collapse Subpages", |s, v| {
            set_collapsed(s, v, Some(true))
        })
        .params(r#"{"page"?: id}"#)
        .when(need_page_sel),
        CommandSpec::new("page.expandSubpages", "CM.Page.IndentedPagesShow", "Expand Subpages", "Context/Page/Expand Subpages", |s, v| {
            set_collapsed(s, v, Some(false))
        })
        .params(r#"{"page"?: id}"#)
        .when(need_page_sel),
        CommandSpec::new(
            "page.toggleSubpages",
            "KB.Pages.ToggleGroup",
            "Expand or Collapse Page Group",
            "Keyboard/Pages/Expand or collapse a page group in the page list",
            |s, v| set_collapsed(s, v, None),
        )
        .key("Mod+Shift+*")
        .when(need_page_sel),
        // KB.Pages.MoveUp / MoveDown — Alt+Shift+Up/Down while the page list has focus.
        CommandSpec::new("page.moveUp", "KB.Pages.MoveUp", "Move Page Up", "Keyboard/Pages/Move the selected page up in the page list", |s, _| {
            move_step(s, true)
        })
        .when(need_page_sel),
        CommandSpec::new(
            "page.moveDown",
            "KB.Pages.MoveDown",
            "Move Page Down",
            "Keyboard/Pages/Move the selected page down in the page list",
            |s, _| move_step(s, false),
        )
        .when(need_page_sel),
        // BEH.Pages.DragReorder — drag a page (with its subpages) to an index and level.
        CommandSpec::new("page.move", "BEH.Pages.DragReorder", "Move Page", "Behaviour/Pages", move_to)
            .params(r#"{"page": id, "section"?: id, "index": int, "level"?: int}"#)
            .when(need_page_sel),
        // CM.Page.PageLinkCreate — a notecraft: link (BEH.Links.CopyLink).
        CommandSpec::new("page.copyLink", "CM.Page.PageLinkCreate", "Copy Link to Page", "Context/Page/Copy Link to Page", copy_link)
            .params(r#"{"page"?: id}"#)
            .pure()
            .when(need_page_sel),
        // KB.Pages.SelectPage — Ctrl+Shift+A selects the page tab (again: its group).
        CommandSpec::new("page.select", "KB.Pages.SelectPage", "Select Page", "Keyboard/Pages/Select the current page", select)
            .key("Mod+Shift+A")
            .pure()
            .when(need_page_sel),
        // CM.PageListArea.PastePage
        CommandSpec::new("page.paste", "CM.PageListArea.PastePage", "Paste Page", "Context/Page List Area/Paste Page", |s, _| paste_pages(s)).when(
            |s| {
                if s.clipboard.pages.is_empty() { Some("No pages on the clipboard") } else { need_section(s) }
            },
        ),
        // BEH.Pages.DateTime — change the date/time shown under the title.
        CommandSpec::new("page.setDateTime", "BEH.Pages.DateTime", "Change Page Date and Time", "Behaviour/Pages", set_date_time)
            .params(r#"{"ms": int (Unix ms)}"#)
            .when(need_page_sel),
    ]
}

fn target_page(s: &Session, v: &Value) -> Result<Id, CmdError> {
    match p::id(v, "page")? {
        Some(id) => Ok(id),
        None => s.sel.page.ok_or_else(no_page),
    }
}

/// A new empty page (title empty, rule lines per preference).
fn sort_pages(s: &mut Session, v: &Value) -> CmdResult {
    let sid = match p::id(v, "section")? {
        Some(id) => id,
        None => s.sel.section.ok_or_else(|| CmdError::Disabled("open a section first".into()))?,
    };
    let by = p::req_str(v, "by")?;
    let descending = match v.get("descending") {
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(CmdError::Params("`descending` must be true or false".into())),
        None => by != "title",
    };
    let sec = s.ws.section_mut(sid).ok_or_else(|| CmdError::Params("no such section".into()))?;
    // Families: a page and the subpages under it move together.
    let mut families: Vec<Vec<Arc<Page>>> = Vec::new();
    for page in sec.pages.drain(..) {
        match families.last_mut() {
            Some(f) if page.level > f.first().map_or(0, |h| h.level) => f.push(page),
            _ => families.push(vec![page]),
        }
    }
    let head = |f: &Vec<Arc<Page>>| f.first().cloned();
    match by {
        "title" => families.sort_by_cached_key(|f| head(f).map(|h| h.display_title().to_lowercase()).unwrap_or_default()),
        "created" => families.sort_by_key(|f| head(f).map(|h| h.created)),
        "modified" => families.sort_by_key(|f| head(f).map(|h| h.modified)),
        other => {
            sec.pages = families.into_iter().flatten().collect();
            return Err(CmdError::Params(format!("unknown sort `{other}` (title, created or modified)")));
        }
    }
    if descending {
        families.reverse();
    }
    sec.pages = families.into_iter().flatten().collect();
    Ok(json!({"pages": sec.pages.len()}))
}

pub(crate) fn make_page(s: &mut Session, title: &str) -> Page {
    let id = s.new_id();
    let tid = s.new_id();
    let mut p = Page::new(id, tid, s.now(), &s.author());
    if !title.is_empty() {
        p.title = Paragraph::with_text(tid, title);
        p.title.style = notecraft_doc::ParaStyle::PageTitle;
    }
    if let Some(r) = s.prefs.new_pages_rule_lines {
        p.setup.rule_lines = r;
    }
    p
}

/// Insert `page` into `section` at `index` and open it with the caret in the title.
pub(crate) fn insert_and_open(s: &mut Session, section: Id, index: usize, page: Page) -> CmdResult {
    let pid = page.id;
    let title_len = page.title.len();
    let sec = s.ws.section_mut(section).ok_or_else(|| CmdError::Failed("that section no longer exists".into()))?;
    let at = index.min(sec.pages.len());
    sec.pages.insert(at, Arc::new(page));
    sec.normalize();
    s.goto_page(pid)?;
    s.sel.focus = FocusArea::Canvas;
    s.set_caret(Pos::new(Block::Title, 0, title_len));
    Ok(json!({"page": pid.to_string()}))
}

fn new_page(s: &mut Session, v: &Value) -> CmdResult {
    let section = match p::id(v, "section")? {
        Some(id) => id,
        None => s.sel.section.ok_or_else(|| CmdError::Disabled("open a section first".into()))?,
    };
    let title = p::str(v, "title").unwrap_or("").to_string();
    let page = crate::template_cmds::new_page_in(s, section, &title);
    let sec = s.ws.section(section).ok_or_else(|| CmdError::Params("no such section".into()))?;
    let index = match p::id(v, "after")? {
        Some(after) => sec.page_index(after).map(|i| sec.page_family(i).end).unwrap_or(sec.pages.len()),
        None => sec.pages.len(),
    };
    insert_and_open(s, section, index, page)
}

fn current(s: &Session) -> Result<(Id, usize, Arc<Section>), CmdError> {
    let pid = s.sel.page.ok_or_else(no_page)?;
    let (sid, i) = s.ws.locate_page(pid).ok_or_else(no_page)?;
    let sec = s.ws.section(sid).ok_or_else(no_page)?.clone();
    Ok((sid, i, sec))
}

fn new_below(s: &mut Session, _: &Value) -> CmdResult {
    let (sid, i, sec) = current(s)?;
    let level = sec.pages.get(i).map(|p| p.level).unwrap_or(0);
    let at = sec.page_family(i).end;
    let mut page = crate::template_cmds::new_page_in(s, sid, "");
    page.level = level;
    insert_and_open(s, sid, at, page)
}

fn new_subpage(s: &mut Session, _: &Value) -> CmdResult {
    let (sid, i, sec) = current(s)?;
    let level = sec.pages.get(i).map(|p| p.level).unwrap_or(0);
    let mut page = crate::template_cmds::new_page_in(s, sid, "");
    page.level = (level + 1).min(notecraft_doc::page::MAX_PAGE_LEVEL);
    insert_and_open(s, sid, i + 1, page)
}

fn new_after_group(s: &mut Session, _: &Value) -> CmdResult {
    let (sid, i, sec) = current(s)?;
    // The group is the top-level page family the current page belongs to.
    let mut top = i;
    while top > 0 && sec.pages.get(top).is_some_and(|p| p.level > 0) {
        top -= 1;
    }
    let at = sec.page_family(top).end;
    let page = make_page(s, "");
    insert_and_open(s, sid, at, page)
}

fn rename(s: &mut Session, v: &Value) -> CmdResult {
    let pid = target_page(s, v)?;
    let title = p::req_str(v, "title")?;
    if title.chars().count() > 10_000 {
        return Err(CmdError::Params("title is too long".into()));
    }
    let now = s.now();
    let page = s.ws.page_mut(pid).ok_or_else(|| CmdError::Params("no such page".into()))?;
    let tid = page.title.id;
    page.title = Paragraph::with_text(tid, &title.replace('\n', " "));
    page.title.style = notecraft_doc::ParaStyle::PageTitle;
    page.modified = now;
    Ok(Value::Null)
}

/// Move the selected pages (current page plus multi-selection) to the recycle bin.
pub(crate) fn delete_pages_to_bin(s: &mut Session) -> Result<usize, CmdError> {
    let mut ids = s.sel.pages.clone();
    if let Some(p) = s.sel.page
        && !ids.contains(&p)
    {
        ids.insert(0, p);
    }
    let mut n = 0;
    for id in ids {
        if delete_one(s, id)? {
            n += 1;
        }
    }
    Ok(n)
}

/// Delete one page: a collapsed page takes its subpages along; an expanded page's subpages
/// move up one level (unverified exact rule).
fn delete_one(s: &mut Session, pid: Id) -> Result<bool, CmdError> {
    let Some((sid, i)) = s.ws.locate_page(pid) else { return Ok(false) };
    let now = s.now();
    let nb_id = s.ws.notebook_of(sid).map(|n| n.id).ok_or_else(no_page)?;
    let sec = s.ws.section(sid).ok_or_else(no_page)?.clone();
    let collapsed = sec.pages.get(i).is_some_and(|p| p.collapsed);
    let fam = sec.page_family(i);
    let range = if collapsed { fam.clone() } else { i..i + 1 };
    let section_name = sec.name.clone();
    let secm = s.ws.section_mut(sid).ok_or_else(no_page)?;
    let removed: Vec<Arc<Page>> = secm.pages.drain(range.clone()).collect();
    if !collapsed {
        // Former children of the deleted page move up one level.
        for p in secm.pages.iter_mut().skip(i).take(fam.len().saturating_sub(1)) {
            let pm = Arc::make_mut(p);
            pm.level = pm.level.saturating_sub(1);
        }
    }
    secm.normalize();
    let next = secm.pages.get(i).or_else(|| i.checked_sub(1).and_then(|j| secm.pages.get(j))).map(|p| p.id);
    let nb = s.ws.notebook_mut(nb_id).ok_or_else(no_page)?;
    for page in removed {
        nb.recycle.pages.push(DeletedPage { page, from_section: sid, from_section_name: section_name.clone(), deleted: now });
    }
    s.sel.pages.retain(|x| *x != pid);
    if s.sel.page == Some(pid) {
        match next {
            Some(n) => s.show_page(Some(nb_id), sid, n),
            None => {
                s.sel.page = None;
                s.sel.on_page = PageSel::None;
            }
        }
    }
    Ok(true)
}

fn delete(s: &mut Session, v: &Value) -> CmdResult {
    let n = match p::id(v, "page")? {
        Some(id) => usize::from(delete_one(s, id)?),
        None => delete_pages_to_bin(s)?,
    };
    if n == 0 {
        return Err(CmdError::Failed("no page was deleted".into()));
    }
    Ok(json!({"deleted": n}))
}

fn move_or_copy(s: &mut Session, v: &Value) -> CmdResult {
    let pid = target_page(s, v)?;
    let to = p::req_id(v, "section")?;
    let copy = p::bool(v, "copy").unwrap_or(false);
    if s.ws.section(to).is_none() {
        return Err(CmdError::Params("no such target section".into()));
    }
    let (sid, i) = s.ws.locate_page(pid).ok_or_else(|| CmdError::Params("no such page".into()))?;
    let sec = s.ws.section(sid).ok_or_else(no_page)?.clone();
    let fam = sec.page_family(i);
    let pages: Vec<Arc<Page>> = sec.pages.get(fam.clone()).map(|x| x.to_vec()).unwrap_or_default();
    let base = pages.first().map(|p| p.level).unwrap_or(0);
    let now = s.now();
    let mut moved: Vec<Arc<Page>> = Vec::new();
    for p in &pages {
        let mut np = if copy { ops::clone_page_fresh(s, p, now) } else { (**p).clone() };
        np.level = np.level.saturating_sub(base);
        moved.push(Arc::new(np));
    }
    if !copy {
        if sid == to {
            return Ok(Value::Null);
        }
        let secm = s.ws.section_mut(sid).ok_or_else(no_page)?;
        secm.pages.drain(fam);
        secm.normalize();
    }
    let first = moved.first().map(|p| p.id);
    let dst = s.ws.section_mut(to).ok_or_else(no_page)?;
    dst.pages.extend(moved);
    dst.normalize();
    if let Some(f) = first {
        s.goto_page(f)?;
    }
    Ok(json!({"page": first.map(|f| f.to_string())}))
}

fn relevel(s: &mut Session, v: &Value, delta: i8) -> CmdResult {
    let pid = target_page(s, v)?;
    let (sid, i) = s.ws.locate_page(pid).ok_or_else(|| CmdError::Params("no such page".into()))?;
    let sec = s.ws.section_mut(sid).ok_or_else(no_page)?;
    let fam = sec.page_family(i);
    let cur = sec.pages.get(i).map(|p| p.level).unwrap_or(0);
    if delta > 0 {
        // A page can be at most one level deeper than the page above it.
        let above = i.checked_sub(1).and_then(|j| sec.pages.get(j)).map(|p| p.level);
        let max_in_family = sec.pages.get(fam.clone()).map(|x| x.iter().map(|p| p.level).max().unwrap_or(0)).unwrap_or(0);
        match above {
            Some(a) if cur <= a && max_in_family < notecraft_doc::page::MAX_PAGE_LEVEL => {}
            _ => return Err(CmdError::Disabled("this page can't be made a subpage".into())),
        }
        for p in sec.pages.get_mut(fam).into_iter().flatten() {
            Arc::make_mut(p).level += 1;
        }
    } else {
        if cur == 0 {
            return Err(CmdError::Disabled("this page is not a subpage".into()));
        }
        for p in sec.pages.get_mut(fam).into_iter().flatten() {
            let pm = Arc::make_mut(p);
            pm.level = pm.level.saturating_sub(1);
        }
    }
    sec.normalize();
    Ok(Value::Null)
}

fn set_collapsed(s: &mut Session, v: &Value, on: Option<bool>) -> CmdResult {
    let pid = target_page(s, v)?;
    let (sid, i) = s.ws.locate_page(pid).ok_or_else(|| CmdError::Params("no such page".into()))?;
    let sec = s.ws.section_mut(sid).ok_or_else(no_page)?;
    if sec.page_family(i).len() < 2 {
        return Err(CmdError::Disabled("this page has no subpages".into()));
    }
    if let Some(p) = sec.pages.get_mut(i) {
        let pm = Arc::make_mut(p);
        pm.collapsed = on.unwrap_or(!pm.collapsed);
    }
    Ok(Value::Null)
}

fn move_step(s: &mut Session, up: bool) -> CmdResult {
    let (sid, i, sec) = current(s)?;
    let fam = sec.page_family(i);
    let lvl = sec.pages.get(i).map(|p| p.level).unwrap_or(0);
    let new_index = if up {
        let Some(prev) = (0..i).rev().find(|&j| sec.pages.get(j).is_some_and(|p| p.level <= lvl)) else { return Ok(Value::Null) };
        prev
    } else {
        if fam.end >= sec.pages.len() {
            return Ok(Value::Null);
        }
        let next_end = sec.page_family(fam.end).end;
        next_end - fam.len()
    };
    let secm = s.ws.section_mut(sid).ok_or_else(no_page)?;
    let block: Vec<_> = secm.pages.drain(fam).collect();
    for (k, p) in block.into_iter().enumerate() {
        secm.pages.insert(new_index + k, p);
    }
    secm.normalize();
    Ok(Value::Null)
}

fn move_to(s: &mut Session, v: &Value) -> CmdResult {
    let pid = p::req_id(v, "page")?;
    let index = p::req_usize(v, "index")?;
    let (sid, i) = s.ws.locate_page(pid).ok_or_else(|| CmdError::Params("no such page".into()))?;
    let to = p::id(v, "section")?.unwrap_or(sid);
    if s.ws.section(to).is_none() {
        return Err(CmdError::Params("no such target section".into()));
    }
    let sec = s.ws.section(sid).ok_or_else(no_page)?.clone();
    let fam = sec.page_family(i);
    let base = sec.pages.get(i).map(|p| p.level).unwrap_or(0);
    let want = p::usize(v, "level").map(|l| l.min(notecraft_doc::page::MAX_PAGE_LEVEL as usize) as u8);
    let secm = s.ws.section_mut(sid).ok_or_else(no_page)?;
    let mut block: Vec<Arc<Page>> = secm.pages.drain(fam.clone()).collect();
    secm.normalize();
    if let Some(w) = want {
        for p in &mut block {
            let pm = Arc::make_mut(p);
            pm.level = (w as i16 + pm.level as i16 - base as i16).clamp(0, notecraft_doc::page::MAX_PAGE_LEVEL as i16) as u8;
        }
    }
    let dst = s.ws.section_mut(to).ok_or_else(no_page)?;
    // Within one section the index refers to the list before removal.
    let at = if to == sid && index > fam.start { index.saturating_sub(fam.len()) } else { index }.min(dst.pages.len());
    for (k, p) in block.into_iter().enumerate() {
        dst.pages.insert(at + k, p);
    }
    dst.normalize();
    if to != sid {
        s.goto_page(pid)?;
    }
    Ok(Value::Null)
}

/// The `notecraft:` link to a page.
pub(crate) fn page_link(id: Id) -> String {
    format!("notecraft:page/{id}")
}

fn copy_link(s: &mut Session, v: &Value) -> CmdResult {
    let pid = target_page(s, v)?;
    let title = s.ws.page(pid).map(|p| p.display_title()).ok_or_else(|| CmdError::Params("no such page".into()))?;
    let link = page_link(pid);
    let mut para = ops::new_para(s);
    para.runs.push(notecraft_doc::Run::new(&title, notecraft_doc::CharFormat { link: Some(link.clone()), ..Default::default() }));
    s.clipboard =
        crate::session::Clipboard { paragraphs: vec![para], text: link.clone(), pages: Vec::new(), objects: Vec::new(), html: String::new() };
    Ok(json!({"link": link, "text": link}))
}

fn select(s: &mut Session, _: &Value) -> CmdResult {
    let (_, i, sec) = current(s)?;
    s.sel.focus = FocusArea::PageList;
    // Second press with the page already selected: select its whole page group.
    if s.sel.pages.is_empty() {
        s.sel.pages = vec![];
        if let Some(p) = s.sel.page {
            s.sel.pages.push(p);
        }
    } else {
        s.sel.pages = sec.pages.get(sec.page_family(i)).map(|x| x.iter().map(|p| p.id).collect()).unwrap_or_default();
    }
    Ok(Value::Null)
}

/// Paste pages from the clipboard after the current page (fresh ids: pasting twice makes two
/// copies).
pub(crate) fn paste_pages(s: &mut Session) -> CmdResult {
    let sid = s.sel.section.ok_or_else(|| CmdError::Disabled("open a section first".into()))?;
    let pages = s.clipboard.pages.clone();
    if pages.is_empty() {
        return Err(CmdError::Disabled("no pages on the clipboard".into()));
    }
    let now = s.now();
    let fresh: Vec<Arc<Page>> = pages.iter().map(|p| Arc::new(ops::clone_page_fresh(s, p, now))).collect();
    let after = s.sel.page.and_then(|p| s.ws.section(sid).and_then(|sec| sec.page_index(p).map(|i| sec.page_family(i).end)));
    let first = fresh.first().map(|p| p.id);
    let sec = s.ws.section_mut(sid).ok_or_else(no_page)?;
    let at = after.unwrap_or(sec.pages.len()).min(sec.pages.len());
    for (k, p) in fresh.into_iter().enumerate() {
        sec.pages.insert(at + k, p);
    }
    sec.normalize();
    if let Some(f) = first {
        s.goto_page(f)?;
    }
    Ok(Value::Null)
}

fn set_date_time(s: &mut Session, v: &Value) -> CmdResult {
    let ms = v.get("ms").and_then(Value::as_i64).ok_or_else(|| CmdError::Params("`ms` (Unix milliseconds) is required".into()))?;
    let page = s.page_mut().ok_or_else(no_page)?;
    page.shown_time = Some(notecraft_doc::Timestamp(ms).clamped());
    Ok(Value::Null)
}

/// The page titled `name` in the current section (then notebook), created at the end of the
/// current section if none exists (catalog `BEH.Links.Wiki`).
pub(crate) fn page_by_title_or_create(s: &mut Session, name: &str) -> Result<Id, CmdError> {
    let lower = name.to_lowercase();
    if let Some(sec) = s.section()
        && let Some(p) = sec.pages.iter().find(|p| p.display_title().to_lowercase() == lower)
    {
        return Ok(p.id);
    }
    if let Some(nb) = s.notebook() {
        for sec in nb.sections() {
            if let Some(p) = sec.pages.iter().find(|p| p.display_title().to_lowercase() == lower) {
                return Ok(p.id);
            }
        }
    }
    let sid = s.sel.section.ok_or_else(|| CmdError::Disabled("open a section first".into()))?;
    let page = make_page(s, name);
    let id = page.id;
    let sec = s.ws.section_mut(sid).ok_or_else(no_page)?;
    sec.pages.push(Arc::new(page));
    Ok(id)
}
