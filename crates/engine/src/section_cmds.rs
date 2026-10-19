//! Sections, section groups and notebook properties (the section tabs, the notebook pane and
//! their context menus).

use std::sync::Arc;

use notecraft_doc::{DeletedSection, Id, NavNode, Section, SectionColor, SectionGroup};
use serde_json::{Value, json};

use crate::ops::no_page;
use crate::selection::{Block, Pos};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn need_notebook(s: &Session) -> Option<&'static str> {
    if s.notebook().is_some() { None } else { Some("Open a notebook first") }
}

fn need_section(s: &Session) -> Option<&'static str> {
    if s.section().is_some() { None } else { Some("Select a section first") }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // UI.Nav.QuickNotes — "Quick notes" pinned at the bottom of the navigation (2024 UI):
        // opens the Quick Notes section of the default notebook (the first one open), creating it
        // at the end of that notebook when it is missing (unverified: the reference lets the user
        // pick the Quick Notes location in Options).
        CommandSpec::new("nav.quickNotes", "UI.Nav.QuickNotes", "Quick Notes", "Window/Navigation/Quick Notes", quick_notes)
            .when(|s| if s.ws.notebooks.is_empty() { Some("Open a notebook first") } else { None }),
        // KB.Notebooks.NewSection — Ctrl+T: "New Section N" with the next colour and one
        // Untitled page; the UI then renames it in place (BEH.Sections.NewSectionName).
        CommandSpec::new(
            "section.new",
            "KB.Notebooks.NewSection",
            "New Section",
            "Keyboard/Notebooks and sections/Create a new section",
            new_section,
        )
        .key("Mod+T")
        .params(r#"{"name"?: string, "group"?: id}"#)
        .when(need_notebook),
        CommandSpec::new("section.rename", "CM.Section.SectionRenameOneNote", "Rename", "Context/Section/Rename", rename_section)
            .params(r#"{"name": string, "section"?: id}"#)
            .when(need_section),
        // CM.Section.NotebookDelete — the section goes to the recycle bin.
        CommandSpec::new("section.delete", "CM.Section.NotebookDelete", "Delete", "Context/Section/Delete", delete_section)
            .params(r#"{"section"?: id}"#)
            .when(need_section),
        // CM.Section.SectionMove — Move or Copy (to a notebook or group, at an index).
        CommandSpec::new("section.moveOrCopy", "CM.Section.SectionMove", "Move or Copy…", "Context/Section/Move or Copy…", move_or_copy)
            .params(r#"{"section"?: id, "notebook"?: id, "group"?: id, "index"?: int, "copy"?: bool}"#)
            .when(need_section),
        // CM.Section.SectionsMerge — pages move to the target; the emptied section goes to the
        // recycle bin.
        CommandSpec::new(
            "section.merge",
            "CM.Section.SectionsMerge",
            "Merge into Another Section…",
            "Context/Section/Merge into Another Section…",
            merge,
        )
        .params(r#"{"into": id, "section"?: id}"#)
        .when(need_section),
        // CM.Section.SectionColorMenu — Color 1–16 or None.
        CommandSpec::new("section.color", "CM.Section.SectionColorMenu", "Section Color", "Context/Section/Section Color", color)
            .params(r#"{"color": 0-15 | null, "section"?: id}"#)
            .when(need_section),
        CommandSpec::new(
            "section.copyLink",
            "CM.Section.SectionLinkCreate",
            "Copy Link to Section",
            "Context/Section/Copy Link to Section",
            copy_link,
        )
        .params(r#"{"section"?: id}"#)
        .pure()
        .when(need_section),
        // NC.Section.Select — click a section tab.
        CommandSpec::new("section.select", "NC.Section.Select", "Select Section", "Mouse/Section tab", select).params(r#"{"section": id}"#).pure(),
        CommandSpec::new(
            "sectionGroup.new",
            "CM.SectionGroup.SectionGroupInsert",
            "New Section Group",
            "Context/Section Group/New Section Group",
            new_group,
        )
        .params(r#"{"name"?: string, "group"?: id}"#)
        .when(need_notebook),
        CommandSpec::new("sectionGroup.rename", "CM.SectionGroup.FolderRename", "Rename", "Context/Section Group/Rename", rename_group)
            .params(r#"{"group": id, "name": string}"#)
            .when(need_notebook),
        CommandSpec::new("sectionGroup.delete", "CM.SectionGroup.FolderDelete", "Delete", "Context/Section Group/Delete", delete_group)
            .params(r#"{"group": id}"#)
            .when(need_notebook),
        CommandSpec::new("sectionGroup.move", "CM.SectionGroup.FolderMove", "Move…", "Context/Section Group/Move…", move_group)
            .params(r#"{"group": id, "notebook"?: id, "into"?: id, "index"?: int}"#)
            .when(need_notebook),
        CommandSpec::new(
            "sectionGroup.copyLink",
            "CM.SectionGroup.SectionGroupLinkCreate",
            "Copy Link to Section Group",
            "Context/Section Group/Copy Link to Section Group",
            copy_group_link,
        )
        .params(r#"{"group": id}"#)
        .pure()
        .when(need_notebook),
        // QAT.NR.NotebookRename / BS.Info.NotebookInformation (Notebook Properties).
        CommandSpec::new(
            "notebook.rename",
            "QAT.NR.NotebookRename",
            "Rename Notebook",
            "Quick Access Toolbar/Commands Not in the Ribbon/Rename Notebook",
            rename_notebook,
        )
        .params(r#"{"name": string, "notebook"?: id}"#)
        .when(need_notebook),
        CommandSpec::new("notebook.properties", "BS.Info.NotebookInformation", "Notebook Properties", "File/Info/Notebook Information", properties)
            .params(r#"{"notebook"?: id, "name"?: string, "color"?: 0-15 | null}"#)
            .when(need_notebook),
        CommandSpec::new("notebook.moveUp", "CM.Notebook.NotebookMoveUp", "Move Up", "Context/Notebook/Move Up", |s, v| move_notebook(s, v, true))
            .params(r#"{"notebook"?: id}"#)
            .when(need_notebook),
        CommandSpec::new("notebook.moveDown", "CM.Notebook.NotebookMoveDown", "Move Down", "Context/Notebook/Move Down", |s, v| {
            move_notebook(s, v, false)
        })
        .params(r#"{"notebook"?: id}"#)
        .when(need_notebook),
        CommandSpec::new(
            "notebook.copyLink",
            "CM.Notebook.NotebookLinkCreate",
            "Copy Link to Notebook",
            "Context/Notebook/Copy Link to Notebook",
            copy_nb_link,
        )
        .params(r#"{"notebook"?: id}"#)
        .pure()
        .when(need_notebook),
        // NC.Notebook.Select — click a notebook in the notebook pane.
        CommandSpec::new("notebook.select", "NC.Notebook.Select", "Select Notebook", "Mouse/Notebook pane", select_notebook)
            .params(r#"{"notebook": id}"#)
            .pure(),
    ]
}

fn target_section(s: &Session, v: &Value) -> Result<Id, CmdError> {
    match p::id(v, "section")? {
        Some(id) => Ok(id),
        None => s.sel.section.ok_or_else(|| CmdError::Disabled("select a section first".into())),
    }
}

fn target_notebook(s: &Session, v: &Value) -> Result<Id, CmdError> {
    match p::id(v, "notebook")? {
        Some(id) => Ok(id),
        None => s.sel.notebook.ok_or_else(|| CmdError::Disabled("open a notebook first".into())),
    }
}

/// The next unused "New Section N" name in a notebook.
fn next_section_name(nb: &notecraft_doc::Notebook, base: &str) -> String {
    let names: Vec<String> = nb.sections().iter().map(|s| s.name.to_lowercase()).collect();
    (1..).map(|n| format!("{base} {n}")).find(|c| !names.contains(&c.to_lowercase())).unwrap_or_else(|| base.to_string())
}

/// Count sections for the colour rotation.
fn section_count(nb: &notecraft_doc::Notebook) -> usize {
    nb.sections().len()
}

/// Where new children go: the group with `group` (if given), the group containing the current
/// section, or the notebook's top level. Returns the group path.
fn parent_path(s: &Session, nb: &notecraft_doc::Notebook, group: Option<Id>) -> Vec<usize> {
    if let Some(g) = group
        && let Some(path) = nb.find(g)
        && matches!(nb.node_at(&path), Some(NavNode::Group(_)))
    {
        return path;
    }
    s.sel
        .section
        .and_then(|sid| nb.find(sid))
        .map(|mut path| {
            path.pop();
            path
        })
        .unwrap_or_default()
}

/// The name of the Quick Notes section.
pub(crate) const QUICK_NOTES: &str = "Quick Notes";

fn quick_notes(s: &mut Session, _v: &Value) -> CmdResult {
    let nb = s.ws.notebooks.first().cloned().ok_or_else(|| CmdError::Disabled("open a notebook first".into()))?;
    let existing = nb.children.iter().find_map(|n| match n {
        NavNode::Section(sec) if sec.name.eq_ignore_ascii_case(QUICK_NOTES) => Some(sec.id),
        _ => None,
    });
    if let Some(sid) = existing {
        s.goto_section(sid)?;
        return Ok(json!({"section": sid.to_string(), "created": false}));
    }
    let sid = s.new_id();
    let mut sec = Section::new(sid, QUICK_NOTES, SectionColor::rotation(section_count(&nb)), s.now());
    let page = crate::page_cmds::make_page(s, "");
    let pid = page.id;
    sec.pages.push(Arc::new(page));
    let nbm = s.ws.notebook_mut(nb.id).ok_or_else(no_page)?;
    nbm.children.push(NavNode::Section(Arc::new(sec)));
    s.goto_page(pid)?;
    s.set_caret(Pos::new(Block::Title, 0, 0));
    Ok(json!({"section": sid.to_string(), "page": pid.to_string(), "created": true}))
}

pub(crate) fn new_section(s: &mut Session, v: &Value) -> CmdResult {
    let nb_id = target_notebook(s, v)?;
    let nb = s.ws.notebook(nb_id).ok_or_else(|| CmdError::Params("no such notebook".into()))?.clone();
    let parent = parent_path(s, &nb, p::id(v, "group")?);
    let name = match p::str(v, "name") {
        Some(n) if !notecraft_doc::clean_name(n).is_empty() => notecraft_doc::clean_name(n),
        _ => next_section_name(&nb, "New Section"),
    };
    let sid = s.new_id();
    let mut sec = Section::new(sid, &name, SectionColor::rotation(section_count(&nb)), s.now());
    let page = crate::page_cmds::make_page(s, "");
    let pid = page.id;
    sec.pages.push(Arc::new(page));
    let nbm = s.ws.notebook_mut(nb_id).ok_or_else(no_page)?;
    let children = nbm.children_at_mut(&parent).ok_or_else(|| CmdError::Failed("section group not found".into()))?;
    children.push(NavNode::Section(Arc::new(sec)));
    s.goto_page(pid)?;
    s.set_caret(Pos::new(Block::Title, 0, 0));
    Ok(json!({"section": sid.to_string(), "page": pid.to_string(), "rename": true}))
}

fn rename_section(s: &mut Session, v: &Value) -> CmdResult {
    let sid = target_section(s, v)?;
    let name = notecraft_doc::clean_name(p::req_str(v, "name")?);
    if name.is_empty() {
        return Err(CmdError::Params("a section name can't be empty".into()));
    }
    let sec = s.ws.section_mut(sid).ok_or_else(|| CmdError::Params("no such section".into()))?;
    sec.name = name;
    Ok(Value::Null)
}

/// Remove the node at `path` from notebook `nb` and return it.
fn take_node(s: &mut Session, nb: Id, id: Id) -> Result<NavNode, CmdError> {
    let nbm = s.ws.notebook_mut(nb).ok_or_else(no_page)?;
    let path = nbm.find(id).ok_or_else(|| CmdError::Params("not found in that notebook".into()))?;
    let (last, parent) = path.split_last().ok_or_else(|| CmdError::Failed("bad path".into()))?;
    let children = nbm.children_at_mut(parent).ok_or_else(|| CmdError::Failed("bad path".into()))?;
    if *last >= children.len() {
        return Err(CmdError::Failed("bad path".into()));
    }
    Ok(children.remove(*last))
}

/// After removing the current section, select a neighbour.
fn reselect(s: &mut Session, nb: Id) {
    let first = s.ws.notebook(nb).and_then(|n| n.sections().first().map(|x| x.id));
    match first {
        Some(sid) => {
            let _ = s.goto_section(sid);
        }
        None => {
            s.sel.section = None;
            s.sel.page = None;
            s.sel.on_page = crate::selection::PageSel::None;
        }
    }
}

fn delete_section(s: &mut Session, v: &Value) -> CmdResult {
    let sid = target_section(s, v)?;
    let nb = s.ws.notebook_of(sid).map(|n| n.id).ok_or_else(|| CmdError::Params("no such section".into()))?;
    let NavNode::Section(sec) = take_node(s, nb, sid)? else { return Err(CmdError::Params("that is a section group".into())) };
    let now = s.now();
    let nbm = s.ws.notebook_mut(nb).ok_or_else(no_page)?;
    nbm.recycle.sections.push(DeletedSection { section: sec, deleted: now });
    if s.sel.section == Some(sid) {
        reselect(s, nb);
    }
    Ok(Value::Null)
}

fn fresh_section(s: &mut Session, sec: &Section) -> Section {
    let now = s.now();
    let mut copy = sec.clone();
    copy.id = s.new_id();
    copy.pages = sec.pages.iter().map(|p| Arc::new(crate::ops::clone_page_fresh(s, p, now))).collect();
    copy
}

fn move_or_copy(s: &mut Session, v: &Value) -> CmdResult {
    let sid = target_section(s, v)?;
    let from_nb = s.ws.notebook_of(sid).map(|n| n.id).ok_or_else(|| CmdError::Params("no such section".into()))?;
    let to_nb = p::id(v, "notebook")?.unwrap_or(from_nb);
    if s.ws.notebook(to_nb).is_none() {
        return Err(CmdError::Params("no such target notebook".into()));
    }
    let group = p::id(v, "group")?;
    let copy = p::bool(v, "copy").unwrap_or(false);
    let node = if copy {
        let sec = s.ws.section(sid).ok_or_else(no_page)?.clone();
        NavNode::Section(Arc::new(fresh_section(s, &sec)))
    } else {
        take_node(s, from_nb, sid)?
    };
    let new_id = node.id();
    let nbm = s.ws.notebook_mut(to_nb).ok_or_else(no_page)?;
    let parent = match group {
        Some(g) => match nbm.find(g) {
            Some(path) if matches!(nbm.node_at(&path), Some(NavNode::Group(_))) => path,
            _ => return Err(CmdError::Params("no such target section group".into())),
        },
        None => Vec::new(),
    };
    let children = nbm.children_at_mut(&parent).ok_or_else(|| CmdError::Failed("section group not found".into()))?;
    let at = p::usize(v, "index").unwrap_or(children.len()).min(children.len());
    children.insert(at, node);
    let _ = s.goto_section(new_id);
    Ok(json!({"section": new_id.to_string()}))
}

fn merge(s: &mut Session, v: &Value) -> CmdResult {
    let sid = target_section(s, v)?;
    let into = p::req_id(v, "into")?;
    if into == sid {
        return Err(CmdError::Params("a section can't be merged into itself".into()));
    }
    let src = s.ws.section(sid).ok_or_else(|| CmdError::Params("no such section".into()))?.clone();
    if s.ws.section(into).is_none() {
        return Err(CmdError::Params("no such target section".into()));
    }
    let dst = s.ws.section_mut(into).ok_or_else(no_page)?;
    dst.pages.extend(src.pages.iter().cloned());
    dst.normalize();
    // The emptied original goes to the recycle bin (it still holds its pages there, so the
    // merge can be undone by restoring it).
    let nb = s.ws.notebook_of(sid).map(|n| n.id).ok_or_else(no_page)?;
    let _ = take_node(s, nb, sid)?;
    let now = s.now();
    if let Some(nbm) = s.ws.notebook_mut(nb) {
        nbm.recycle.sections.push(DeletedSection { section: src, deleted: now });
    }
    s.goto_section(into)?;
    Ok(Value::Null)
}

fn color(s: &mut Session, v: &Value) -> CmdResult {
    let sid = target_section(s, v)?;
    let c = match v.get("color") {
        Some(Value::Null) => SectionColor(None),
        Some(x) => {
            let i = x.as_u64().filter(|i| *i < 16).ok_or_else(|| CmdError::Params("`color` must be 0–15 or null".into()))?;
            SectionColor::index(i as usize)
        }
        None => return Err(CmdError::Params("`color` is required (0–15 or null)".into())),
    };
    let sec = s.ws.section_mut(sid).ok_or_else(|| CmdError::Params("no such section".into()))?;
    sec.color = c;
    Ok(Value::Null)
}

fn set_link_clipboard(s: &mut Session, title: &str, link: &str) {
    let mut para = crate::ops::new_para(s);
    para.runs.push(notecraft_doc::Run::new(title, notecraft_doc::CharFormat { link: Some(link.to_string()), ..Default::default() }));
    s.clipboard =
        crate::session::Clipboard { paragraphs: vec![para], text: link.to_string(), pages: Vec::new(), objects: Vec::new(), html: String::new() };
}

fn copy_link(s: &mut Session, v: &Value) -> CmdResult {
    let sid = target_section(s, v)?;
    let name = s.ws.section(sid).map(|x| x.name.clone()).ok_or_else(|| CmdError::Params("no such section".into()))?;
    let link = format!("notecraft:section/{sid}");
    set_link_clipboard(s, &name, &link);
    Ok(json!({"link": link}))
}

fn select(s: &mut Session, v: &Value) -> CmdResult {
    let sid = p::req_id(v, "section")?;
    s.goto_section(sid)?;
    Ok(Value::Null)
}

fn new_group(s: &mut Session, v: &Value) -> CmdResult {
    let nb_id = target_notebook(s, v)?;
    let nb = s.ws.notebook(nb_id).ok_or_else(no_page)?.clone();
    let parent = match p::id(v, "group")? {
        Some(g) => parent_path(s, &nb, Some(g)),
        None => Vec::new(),
    };
    let name = match p::str(v, "name") {
        Some(n) if !notecraft_doc::clean_name(n).is_empty() => notecraft_doc::clean_name(n),
        _ => "New Section Group".to_string(),
    };
    let gid = s.new_id();
    let nbm = s.ws.notebook_mut(nb_id).ok_or_else(no_page)?;
    let children = nbm.children_at_mut(&parent).ok_or_else(|| CmdError::Failed("section group not found".into()))?;
    children.push(NavNode::Group(SectionGroup { id: gid, name, children: Vec::new() }));
    Ok(json!({"group": gid.to_string(), "rename": true}))
}

fn with_group(s: &mut Session, gid: Id, f: &mut dyn FnMut(&mut SectionGroup)) -> Result<(), CmdError> {
    let nb = s.ws.notebook_of(gid).map(|n| n.id).ok_or_else(|| CmdError::Params("no such section group".into()))?;
    let nbm = s.ws.notebook_mut(nb).ok_or_else(no_page)?;
    let path = nbm.find(gid).ok_or_else(|| CmdError::Params("no such section group".into()))?;
    match nbm.node_at_mut(&path) {
        Some(NavNode::Group(g)) => {
            f(g);
            Ok(())
        }
        _ => Err(CmdError::Params("that is not a section group".into())),
    }
}

fn rename_group(s: &mut Session, v: &Value) -> CmdResult {
    let gid = p::req_id(v, "group")?;
    let name = notecraft_doc::clean_name(p::req_str(v, "name")?);
    if name.is_empty() {
        return Err(CmdError::Params("a section group name can't be empty".into()));
    }
    with_group(s, gid, &mut |g| g.name = name.clone())?;
    Ok(Value::Null)
}

fn delete_group(s: &mut Session, v: &Value) -> CmdResult {
    let gid = p::req_id(v, "group")?;
    let nb = s.ws.notebook_of(gid).map(|n| n.id).ok_or_else(|| CmdError::Params("no such section group".into()))?;
    let NavNode::Group(g) = take_node(s, nb, gid)? else { return Err(CmdError::Params("that is not a section group".into())) };
    // Every section inside goes to the recycle bin.
    let mut sections = Vec::new();
    fn collect(nodes: &[NavNode], out: &mut Vec<Arc<Section>>, depth: usize) {
        if depth > notecraft_doc::notebook::MAX_GROUP_DEPTH {
            return;
        }
        for n in nodes {
            match n {
                NavNode::Section(sec) => out.push(sec.clone()),
                NavNode::Group(g) => collect(&g.children, out, depth + 1),
            }
        }
    }
    collect(&g.children, &mut sections, 0);
    let now = s.now();
    let current_inside = s.sel.section.is_some_and(|sid| sections.iter().any(|x| x.id == sid));
    if let Some(nbm) = s.ws.notebook_mut(nb) {
        for sec in sections {
            nbm.recycle.sections.push(DeletedSection { section: sec, deleted: now });
        }
    }
    if current_inside {
        reselect(s, nb);
    }
    Ok(Value::Null)
}

fn move_group(s: &mut Session, v: &Value) -> CmdResult {
    let gid = p::req_id(v, "group")?;
    let from_nb = s.ws.notebook_of(gid).map(|n| n.id).ok_or_else(|| CmdError::Params("no such section group".into()))?;
    let to_nb = p::id(v, "notebook")?.unwrap_or(from_nb);
    let into = p::id(v, "into")?;
    if into == Some(gid) {
        return Err(CmdError::Params("a group can't move into itself".into()));
    }
    // Refuse moving a group into its own descendant (would orphan it).
    if let Some(target) = into
        && let Some(g) = s.ws.notebook(from_nb).and_then(|n| n.group(gid))
        && g.children.iter().any(|c| contains(c, target, 0))
    {
        return Err(CmdError::Params("a group can't move into one of its own groups".into()));
    }
    let node = take_node(s, from_nb, gid)?;
    let nbm = s.ws.notebook_mut(to_nb).ok_or_else(|| CmdError::Params("no such target notebook".into()))?;
    let parent = match into {
        Some(t) => nbm.find(t).ok_or_else(|| CmdError::Params("no such target group".into()))?,
        None => Vec::new(),
    };
    let children = nbm.children_at_mut(&parent).ok_or_else(|| CmdError::Params("the target is not a section group".into()))?;
    let at = p::usize(v, "index").unwrap_or(children.len()).min(children.len());
    children.insert(at, node);
    Ok(Value::Null)
}

fn contains(n: &NavNode, id: Id, depth: usize) -> bool {
    if depth > notecraft_doc::notebook::MAX_GROUP_DEPTH {
        return false;
    }
    match n {
        NavNode::Section(s) => s.id == id,
        NavNode::Group(g) => g.id == id || g.children.iter().any(|c| contains(c, id, depth + 1)),
    }
}

fn copy_group_link(s: &mut Session, v: &Value) -> CmdResult {
    let gid = p::req_id(v, "group")?;
    let name =
        s.ws.notebook_of(gid).and_then(|n| n.group(gid)).map(|g| g.name.clone()).ok_or_else(|| CmdError::Params("no such section group".into()))?;
    let link = format!("notecraft:group/{gid}");
    set_link_clipboard(s, &name, &link);
    Ok(json!({"link": link}))
}

fn rename_notebook(s: &mut Session, v: &Value) -> CmdResult {
    let nb = target_notebook(s, v)?;
    let name = notecraft_doc::clean_name(p::req_str(v, "name")?);
    if name.is_empty() {
        return Err(CmdError::Params("a notebook name can't be empty".into()));
    }
    s.ws.notebook_mut(nb).ok_or_else(|| CmdError::Params("no such notebook".into()))?.name = name;
    Ok(Value::Null)
}

fn properties(s: &mut Session, v: &Value) -> CmdResult {
    let nb = target_notebook(s, v)?;
    let color = match v.get("color") {
        None => None,
        Some(Value::Null) => Some(SectionColor(None)),
        Some(x) => {
            Some(SectionColor::index(x.as_u64().filter(|i| *i < 16).ok_or_else(|| CmdError::Params("`color` must be 0–15 or null".into()))? as usize))
        }
    };
    let name = p::str(v, "name").map(notecraft_doc::clean_name);
    if name.as_ref().is_some_and(String::is_empty) {
        return Err(CmdError::Params("a notebook name can't be empty".into()));
    }
    let n = s.ws.notebook_mut(nb).ok_or_else(|| CmdError::Params("no such notebook".into()))?;
    if let Some(c) = color {
        n.color = c;
    }
    if let Some(nm) = name {
        n.name = nm;
    }
    let out = json!({"name": n.name, "color": n.color.0, "sections": n.sections().len()});
    Ok(out)
}

fn move_notebook(s: &mut Session, v: &Value, up: bool) -> CmdResult {
    let nb = target_notebook(s, v)?;
    let i = s.ws.notebook_index(nb).ok_or_else(|| CmdError::Params("no such notebook".into()))?;
    let j = if up { i.checked_sub(1) } else { Some(i + 1).filter(|j| *j < s.ws.notebooks.len()) };
    if let Some(j) = j {
        s.ws.notebooks.swap(i, j);
    }
    Ok(Value::Null)
}

fn copy_nb_link(s: &mut Session, v: &Value) -> CmdResult {
    let nb = target_notebook(s, v)?;
    let name = s.ws.notebook(nb).map(|n| n.name.clone()).ok_or_else(|| CmdError::Params("no such notebook".into()))?;
    let link = format!("notecraft:notebook/{nb}");
    set_link_clipboard(s, &name, &link);
    Ok(json!({"link": link}))
}

fn select_notebook(s: &mut Session, v: &Value) -> CmdResult {
    let nb = p::req_id(v, "notebook")?;
    let first = s.ws.notebook(nb).ok_or_else(|| CmdError::Params("no such notebook".into()))?.sections().first().map(|x| x.id);
    match first {
        Some(sid) => s.goto_section(sid)?,
        None => {
            s.sel.notebook = Some(nb);
            s.sel.section = None;
            s.sel.page = None;
            s.sel.on_page = crate::selection::PageSel::None;
        }
    }
    s.sel.notebook = Some(nb);
    Ok(Value::Null)
}
