//! Page versions (History › Page Versions; decision D14) and read-only previews of a version or
//! of a page in the Notebook Recycle Bin.
//!
//! A version is an earlier state of a page, kept inside the page (`Page::versions`, saved with
//! its section). [`snapshot`] runs after every editing command: when a page is changed and its
//! previous state was last edited at least [`VERSION_GAP_MS`] ago (a return to the page after
//! a pause), that previous state becomes a version. Continuous typing makes no versions; an
//! empty page's state is never kept. A page keeps at most `MAX_VERSIONS` (the oldest go).
//!
//! A preview (`ViewState::preview`) shows a version or a deleted page instead of the canvas,
//! read-only, drawn as a picture of the page (the PNG export's drawing).

use notecraft_doc::{Id, Page, PageVersion, Workspace};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ops::{need_page, no_page};
use crate::selection::Selection;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// The pause after which the next edit keeps the page's previous state as a version
/// (unverified: the reference's interval is not documented).
pub const VERSION_GAP_MS: i64 = 5 * 60_000;

/// What the page area shows instead of the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Preview {
    /// A version of a page: the page and the version's time (ms).
    Version {
        /// The page.
        page: Id,
        /// `PageVersion::at` of the version.
        at: i64,
    },
    /// A page in the current notebook's Recycle Bin.
    Deleted {
        /// The deleted page.
        page: Id,
    },
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("history.pageVersions", "R.History.History.PageVersions", "Page Versions", "History/Page Versions", |s, v| {
            s.view.page_versions = p::bool(v, "on").unwrap_or(!s.view.page_versions);
            if !s.view.page_versions && matches!(s.view.preview, Some(Preview::Version { .. })) {
                s.view.preview = None;
            }
            Ok(json!({"shown": s.view.page_versions}))
        })
        .params(r#"{"on"?: bool} (versions under the pages in the page list; default: switch)"#)
        .pure(),
        CommandSpec::new("history.versions", "UI.Panes.PageVersions", "Page Versions", "Window/Panes/Page Versions", list_versions)
            .params(r#"{"page"?: id} — the page's versions, newest first"#)
            .pure()
            .when(need_page),
        CommandSpec::new("history.openVersion", "NC.Versions.Open", "Open Version", "Window/Panes/Page Versions/Open a version", open_version)
            .params(r#"{"page"?: id, "at": ms (the version's time)}"#)
            .pure(),
        CommandSpec::new(
            "history.previewDeleted",
            "NC.Recycle.Preview",
            "Open Deleted Page",
            "Window/Panes/Recycle Bin/Open a deleted page",
            preview_deleted,
        )
        .params(r#"{"item": id (a page in the Recycle Bin)}"#)
        .pure(),
        CommandSpec::new("history.closePreview", "NC.Versions.ClosePreview", "Close", "Window/Panes/Page Versions/Close the version", |s, _| {
            s.view.preview = None;
            Ok(Value::Null)
        })
        .pure(),
        CommandSpec::new(
            "history.restoreVersion",
            "CM.PageVersion.PageVersionRestore",
            "Restore Version",
            "Context/Page Version/Restore Version",
            restore_version,
        )
        .params(r#"{"page"?: id, "at"?: ms} (default: the version shown)"#),
        CommandSpec::new(
            "history.deleteVersion",
            "CM.PageVersion.PageVersionDelete",
            "Delete Version",
            "Context/Page Version/Delete Version",
            delete_version,
        )
        .params(r#"{"page"?: id, "at"?: ms} (default: the version shown)"#),
        CommandSpec::new(
            "history.deleteVersionsSection",
            "R.History.History.DeleteAllPageVersionsInSection",
            "Delete All Versions in Section",
            "History/Page Versions/Delete All Versions in Section",
            |s, _| delete_all(s, Scope::Section),
        )
        .when(need_section),
        CommandSpec::new(
            "history.deleteVersionsGroup",
            "R.History.History.DeleteAllPageVersionsInFolder",
            "Delete All Versions in Section Group",
            "History/Page Versions/Delete All Versions in Section Group",
            |s, _| delete_all(s, Scope::Group),
        )
        .when(need_section),
        CommandSpec::new(
            "history.deleteVersionsNotebook",
            "R.History.History.DeleteAllPageVersionsInNotebook",
            "Delete All Versions in Notebook",
            "History/Page Versions/Delete All Versions in Notebook",
            |s, _| delete_all(s, Scope::Notebook),
        )
        .when(need_section),
    ]
}

fn need_section(s: &Session) -> Option<&'static str> {
    if s.sel.section.is_some() { None } else { Some("No section is open") }
}

/// Keep the previous state of the pages an editing command changed, when due (see the module
/// docs). Runs inside the command's undo step.
pub(crate) fn snapshot(s: &mut Session, before: &Workspace, before_sel: &Selection) {
    let now = s.clock.now_ms;
    let author = s.prefs.author.clone();
    let mut ids: Vec<Id> = [before_sel.page, s.sel.page].into_iter().flatten().collect();
    ids.dedup();
    for pid in ids {
        let (Some(old), Some(new)) = (before.page(pid), s.ws.page(pid)) else { continue };
        if std::sync::Arc::ptr_eq(old, new) || old.modified == new.modified {
            continue;
        }
        let quiet = now.saturating_sub(old.modified.0) >= VERSION_GAP_MS;
        let unkept = new.versions.last().is_none_or(|v| v.at < old.modified);
        let empty = old.objects.is_empty() && old.title.is_empty();
        if !(quiet && unkept) || empty {
            continue;
        }
        let v = old.snapshot(&author);
        if let Some(p) = s.ws.page_mut(pid) {
            p.push_version(v);
        }
    }
}

fn page_id(s: &Session, v: &Value) -> Result<Id, CmdError> {
    match p::id(v, "page")? {
        Some(id) => Ok(id),
        None => match s.view.preview {
            Some(Preview::Version { page, .. }) => Ok(page),
            _ => s.sel.page.ok_or_else(no_page),
        },
    }
}

/// The version `at` of page `pid` (`at` from the params, else the one shown).
fn version_at(s: &Session, v: &Value, pid: Id) -> Result<i64, CmdError> {
    if let Some(at) = v.get("at").and_then(Value::as_i64) {
        return Ok(at);
    }
    match s.view.preview {
        Some(Preview::Version { page, at }) if page == pid => Ok(at),
        _ => Err(CmdError::Params("`at` (the version's time) is required".into())),
    }
}

fn list_versions(s: &mut Session, v: &Value) -> CmdResult {
    let pid = page_id(s, v)?;
    let page = s.ws.page(pid).ok_or_else(|| CmdError::Params("no such page".into()))?;
    let off = s.clock.utc_offset_min;
    let list: Vec<Value> = page
        .versions
        .iter()
        .rev()
        .map(|x| json!({"at": x.at.0, "date": x.at.short_date(off), "time": x.at.short_time(off), "author": x.author, "title": x.title.text()}))
        .collect();
    Ok(json!({"page": pid.to_string(), "versions": list}))
}

fn open_version(s: &mut Session, v: &Value) -> CmdResult {
    let pid = page_id(s, v)?;
    let at = version_at(s, v, pid)?;
    let page = s.ws.page(pid).ok_or_else(|| CmdError::Params("no such page".into()))?;
    if !page.versions.iter().any(|x| x.at.0 == at) {
        return Err(CmdError::Params("that page has no version at that time".into()));
    }
    if s.sel.page != Some(pid) {
        s.goto_page(pid)?;
    }
    s.view.preview = Some(Preview::Version { page: pid, at });
    Ok(Value::Null)
}

fn preview_deleted(s: &mut Session, v: &Value) -> CmdResult {
    let item = p::req_id(v, "item")?;
    let nb = s.notebook().ok_or_else(no_page)?;
    if !nb.recycle.pages.iter().any(|d| d.page.id == item) {
        return Err(CmdError::Params("that page is not in the Recycle Bin".into()));
    }
    s.view.preview = Some(Preview::Deleted { page: item });
    Ok(Value::Null)
}

fn restore_version(s: &mut Session, v: &Value) -> CmdResult {
    let pid = page_id(s, v)?;
    let at = version_at(s, v, pid)?;
    let now = s.now();
    let author = s.prefs.author.clone();
    let page = s.ws.page_mut(pid).ok_or_else(|| CmdError::Params("no such page".into()))?;
    let i = page.versions.iter().position(|x| x.at.0 == at).ok_or_else(|| CmdError::Params("no version at that time".into()))?;
    // The current state becomes a version, so restoring can be undone from the list too.
    let current = page.snapshot(&author);
    let old = page.versions.get(i).cloned().ok_or_else(|| CmdError::Params("no version at that time".into()))?;
    page.title = old.title;
    page.objects = old.objects;
    page.setup = old.setup;
    page.push_version(current);
    page.modified = now;
    s.view.preview = None;
    s.goto_page(pid)?;
    Ok(json!({"page": pid.to_string()}))
}

fn delete_version(s: &mut Session, v: &Value) -> CmdResult {
    let pid = page_id(s, v)?;
    let at = version_at(s, v, pid)?;
    let page = s.ws.page_mut(pid).ok_or_else(|| CmdError::Params("no such page".into()))?;
    let before = page.versions.len();
    page.versions.retain(|x| x.at.0 != at);
    if page.versions.len() == before {
        return Err(CmdError::Params("no version at that time".into()));
    }
    if s.view.preview == Some(Preview::Version { page: pid, at }) {
        s.view.preview = None;
    }
    Ok(Value::Null)
}

#[derive(Clone, Copy)]
enum Scope {
    Section,
    Group,
    Notebook,
}

fn delete_all(s: &mut Session, scope: Scope) -> CmdResult {
    let sid = s.sel.section.ok_or_else(no_page)?;
    let kind = match scope {
        Scope::Section => crate::session::ScopeKind::Section,
        Scope::Group => crate::session::ScopeKind::SectionGroup,
        Scope::Notebook => crate::session::ScopeKind::Notebook,
    };
    let nb = s.ws.notebook_of(sid).ok_or_else(no_page)?.clone();
    let group = match s.scope(kind) {
        notecraft_search::Scope::SectionGroup(g) => Some(g),
        _ => None,
    };
    let sections: Vec<Id> = nb
        .sections()
        .iter()
        .filter(|sec| match scope {
            Scope::Section => sec.id == sid,
            Scope::Notebook => true,
            Scope::Group => match group {
                Some(g) => nb.find(sec.id).is_some_and(|path| {
                    (1..path.len()).any(|k| path.get(..k).and_then(|pre| nb.node_at(pre)).map(notecraft_doc::NavNode::id) == Some(g))
                }),
                None => true,
            },
        })
        .map(|sec| sec.id)
        .collect();
    let mut removed = 0usize;
    for sec_id in sections {
        let has = s.ws.section(sec_id).is_some_and(|sec| sec.pages.iter().any(|p| !p.versions.is_empty()));
        if !has {
            continue;
        }
        if let Some(sec) = s.ws.section_mut(sec_id) {
            for p in &mut sec.pages {
                if !p.versions.is_empty() {
                    removed += p.versions.len();
                    std::sync::Arc::make_mut(p).versions.clear();
                }
            }
        }
    }
    if matches!(s.view.preview, Some(Preview::Version { .. })) {
        s.view.preview = None;
    }
    Ok(json!({"deleted": removed}))
}

impl Session {
    /// The page a preview shows: a version made into a page, or the deleted page.
    pub fn preview_page(&self) -> Option<Page> {
        match self.view.preview? {
            Preview::Version { page, at } => {
                let p = self.ws.page(page)?;
                let i = p.versions.iter().position(|x| x.at.0 == at)?;
                p.version_page(i)
            }
            Preview::Deleted { page } => self.notebook()?.recycle.pages.iter().find(|d| d.page.id == page).map(|d| (*d.page).clone()),
        }
    }

    /// The version a preview shows, if it shows one.
    pub fn preview_version(&self) -> Option<&PageVersion> {
        match self.view.preview? {
            Preview::Version { page, at } => self.ws.page(page)?.versions.iter().find(|x| x.at.0 == at),
            Preview::Deleted { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{run, sample};

    /// Type `text` on the current page `mins` minutes after the last edit.
    fn edit_later(s: &mut Session, mins: i64, text: &str) {
        s.clock.now_ms += mins * 60_000;
        run(s, "text.insert", json!({"text": text}));
    }

    fn versions(s: &Session) -> usize {
        s.page().unwrap().versions.len()
    }

    #[test]
    fn a_pause_then_an_edit_keeps_the_previous_state() {
        // parity: BEH.Versions.Snapshots, UI.Panes.PageVersions
        let mut s = sample();
        edit_later(&mut s, 60, "a");
        assert_eq!(versions(&s), 1, "the sample's state, an hour old, is kept");
        edit_later(&mut s, 1, "b");
        edit_later(&mut s, 1, "c");
        assert_eq!(versions(&s), 1, "steady typing keeps none");
        // A non-editing command ends the typing burst, so the next edit is its own undo step.
        run(&mut s, "view.zoom100", json!({}));
        edit_later(&mut s, 10, "d");
        assert_eq!(versions(&s), 2);
        let list = run(&mut s, "history.versions", json!({}));
        let vs = list["versions"].as_array().unwrap();
        assert!(vs[0]["at"].as_i64() > vs[1]["at"].as_i64(), "newest first");
        assert_eq!(vs[0]["author"], "Tester");
        // The version is one undo step with the edit that made it.
        run(&mut s, "edit.undo", json!({}));
        assert_eq!(versions(&s), 1);
        // A new, empty page keeps no empty version.
        run(&mut s, "page.new", json!({}));
        edit_later(&mut s, 60, "first words");
        assert_eq!(versions(&s), 0);
    }

    #[test]
    fn versions_open_restore_and_delete() {
        // parity: CM.PageVersion.PageVersionRestore, CM.PageVersion.PageVersionDelete, R.History.History.PageVersions, NC.Versions.Open
        let mut s = sample();
        let pid = s.page().unwrap().id;
        let original = s.page().unwrap().objects.clone();
        edit_later(&mut s, 60, "CHANGED ");
        let at = s.page().unwrap().versions[0].at.0;
        assert_eq!(run(&mut s, "history.pageVersions", json!({}))["shown"], true);
        run(&mut s, "history.openVersion", json!({"page": pid.to_string(), "at": at}));
        assert_eq!(s.view.preview, Some(Preview::Version { page: pid, at }));
        assert_eq!(s.preview_page().unwrap().objects, original, "the preview is the old state");
        assert_eq!(s.preview_version().unwrap().author, "Tester");
        // Restore: the old content is back, the replaced state is a version, the preview closes.
        run(&mut s, "history.restoreVersion", json!({}));
        assert_eq!(s.page().unwrap().objects, original);
        assert_eq!(versions(&s), 2);
        assert!(s.view.preview.is_none());
        run(&mut s, "edit.undo", json!({}));
        assert_ne!(s.page().unwrap().objects, original, "one undo step");
        run(&mut s, "edit.redo", json!({}));
        // parity: CM.PageVersion.CopyOrMoveToSection
        // A version copied to another section is a new page with the old content.
        let at0 = s.page().unwrap().versions[0].at.0;
        let old0 = s.page().unwrap().version_page(0).unwrap();
        let garden = s.notebook().unwrap().sections().iter().find(|x| x.name == "Garden").unwrap().id;
        let r = run(&mut s, "page.moveOrCopy", json!({"page": pid.to_string(), "version": at0, "section": garden.to_string()}));
        assert_eq!(s.section().unwrap().id, garden);
        assert_ne!(r["page"].as_str(), Some(pid.to_string().as_str()));
        assert_eq!(s.page().unwrap().title.text(), old0.title.text());
        assert!(s.page().unwrap().versions.is_empty(), "the copy starts its own history");
        s.goto_page(pid).unwrap();
        // Delete one version, then all in the notebook.
        let at = s.page().unwrap().versions[0].at.0;
        run(&mut s, "history.deleteVersion", json!({"at": at}));
        assert_eq!(versions(&s), 1);
        assert_eq!(run(&mut s, "history.deleteVersionsNotebook", json!({}))["deleted"], 1);
        assert_eq!(versions(&s), 0);
        assert!(matches!(s.run("history.openVersion", json!({"at": at})), Err(CmdError::Params(_))));
        assert!(matches!(s.run("history.deleteVersion", json!({})), Err(CmdError::Params(_))));
    }

    #[test]
    fn delete_all_versions_by_scope() {
        // parity: R.History.History.DeleteAllPageVersionsInSection, R.History.History.DeleteAllPageVersionsInFolder, R.History.History.DeleteAllPageVersionsInNotebook
        let mut s = sample();
        edit_later(&mut s, 60, "x");
        assert_eq!(run(&mut s, "history.deleteVersionsGroup", json!({}))["deleted"], 1, "a top-level section's group is its notebook");
        edit_later(&mut s, 60, "y");
        assert_eq!(run(&mut s, "history.deleteVersionsSection", json!({}))["deleted"], 1);
        assert_eq!(run(&mut s, "history.deleteVersionsSection", json!({}))["deleted"], 0);
    }

    #[test]
    fn a_deleted_page_previews_from_the_recycle_bin() {
        // parity: NC.Recycle.Preview
        let mut s = sample();
        let page = s.page().unwrap().clone();
        run(&mut s, "page.delete", json!({}));
        run(&mut s, "history.previewDeleted", json!({"item": page.id.to_string()}));
        assert_eq!(s.preview_page().unwrap().title, page.title);
        assert!(s.preview_version().is_none());
        run(&mut s, "history.closePreview", json!({}));
        assert!(s.preview_page().is_none());
        assert!(s.run("history.previewDeleted", json!({"item": page.title.id.to_string()})).is_err());
    }
}
