//! Notebooks on disk and the File (backstage) commands: new, open, close, save (autosave),
//! backups, export, the Notebook Recycle Bin.
//!
//! Notebooks live in folders (`notecraft-format`); this module keeps one `NotebookStore` per
//! open notebook and writes only what changed. On the web build there is no file system: new
//! notebooks live in memory and file commands that need a path are disabled.

use std::sync::Arc;

use notecraft_doc::{NavNode, Notebook, Section, SectionColor};
use serde_json::{Value, json};

use crate::ops::no_page;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn need_notebook(s: &Session) -> Option<&'static str> {
    if s.notebook().is_some() { None } else { Some("Open a notebook first") }
}

fn need_bin(s: &Session) -> Option<&'static str> {
    match s.notebook() {
        Some(nb) if !nb.recycle.is_empty() => None,
        Some(_) => Some("The recycle bin is empty"),
        None => Some("Open a notebook first"),
    }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // BS.New.NewNotebook — a folder at `path` (desktop) or an in-memory notebook (web,
        // agents). It starts with one section holding one page (BEH.Notebooks.Default).
        CommandSpec::new("file.newNotebook", "BS.New.NewNotebook", "New Notebook", "File/New/New Notebook", new_notebook)
            .params(r#"{"name": string, "path"?: folder}"#),
        // BS.Open.OpenNotebook — Ctrl+O.
        CommandSpec::new("file.openNotebook", "BS.Open.OpenNotebook", "Open Notebook", "File/Open/Browse", open_notebook)
            .key("Mod+O")
            .params(r#"{"path": folder}"#),
        // BS.Open.RecentNotebooks — a folder picked from the recent list, not Browse; same
        // open (opening an already-open folder just selects it).
        CommandSpec::new("file.openRecent", "BS.Open.RecentNotebooks", "Recent Notebooks", "File/Open/Recent Notebooks", open_notebook)
            .params(r#"{"path": folder}"#),
        // CM.Notebook.NotebookClose — saves first.
        CommandSpec::new(
            "notebook.close",
            "CM.Notebook.NotebookClose",
            "Close This Notebook",
            "Context/Notebook/Close This Notebook",
            close_notebook,
        )
        .params(r#"{"notebook"?: id}"#)
        .when(need_notebook),
        // KB.Pages.Sync — Ctrl+S: notes save automatically; this flushes pending saves now.
        CommandSpec::new("file.save", "KB.Pages.Sync", "Save Now", "Keyboard/Pages/Synchronize the notebook (notes are saved automatically)", save)
            .key("Mod+S")
            .pure(),
        // BS.Export.Notebook — a single-file archive of the notebook.
        CommandSpec::new("file.exportNotebook", "BS.Export.Notebook", "Export Notebook", "File/Export/Notebook", export_notebook)
            .params(r#"{"path": file, "notebook"?: id}"#)
            .pure()
            .when(need_notebook),
        // OPT.SaveBackup.BackupAllNow — back up every open notebook now (BEH.Save.Backups).
        CommandSpec::new(
            "file.backupNow",
            "OPT.SaveBackup.BackupAllNow",
            "Back Up All Notebooks Now",
            "File/Options/Save & Backup/Back Up All Notebooks Now",
            backup_now,
        )
        .params(r#"{"folder": backup folder}"#)
        .pure(),
        // BS.Info.OpenBackups — the backups available for a notebook.
        CommandSpec::new("file.openBackups", "BS.Info.OpenBackups", "Open Backups", "File/Info/Open Backups", list_backups)
            .params(r#"{"folder": backup folder}"#)
            .pure(),
        // BS.Options — preferences round-trip for agents (the dialog lives in the UI).
        CommandSpec::new("file.options", "BS.Options", "Options", "File/Options", options).params(r#"{"set"?: {pref: value}}"#).pure(),
        // R.History.History.RecycleBinMenu — view the recycle bin.
        CommandSpec::new(
            "history.recycleBin",
            "R.History.History.RecycleBinMenu",
            "Notebook Recycle Bin",
            "History/Notebook Recycle Bin",
            recycle_bin,
        )
        .params(r#"{"notebook"?: id}"#)
        .pure()
        .when(need_notebook),
        // R.History.History.EmptyRecycleBin (BEH.Recycle.Empty).
        CommandSpec::new(
            "history.emptyRecycleBin",
            "R.History.History.EmptyRecycleBin",
            "Empty Recycle Bin",
            "History/Notebook Recycle Bin/Empty Recycle Bin",
            empty_bin,
        )
        .when(need_bin),
        // BEH.Recycle.DeletedPages — restore a deleted page or section (Move or Copy from the bin).
        CommandSpec::new("history.restore", "BEH.Recycle.DeletedPages", "Restore from Recycle Bin", "Behaviour/Recycle", restore)
            .params(r#"{"item": id, "section"?: id}"#)
            .when(need_bin),
    ]
}

fn starter_notebook(s: &mut Session, name: &str) -> Notebook {
    let nb_id = s.new_id();
    let mut nb = Notebook::new(nb_id, name, SectionColor::index(0));
    let sid = s.new_id();
    let mut sec = Section::new(sid, "New Section 1", SectionColor::index(0), s.now());
    let page = crate::page_cmds::make_page(s, "");
    sec.pages.push(Arc::new(page));
    nb.children.push(NavNode::Section(Arc::new(sec)));
    nb
}

fn new_notebook(s: &mut Session, v: &Value) -> CmdResult {
    let name = notecraft_doc::clean_name(p::req_str(v, "name")?);
    if name.is_empty() {
        return Err(CmdError::Params("a notebook name can't be empty".into()));
    }
    let nb = starter_notebook(s, &name);
    let id = nb.id;
    if let Some(path) = p::str(v, "path") {
        attach_new(s, &nb, path)?;
    }
    s.add_notebook(nb);
    Ok(json!({"notebook": id.to_string()}))
}

#[cfg(not(target_arch = "wasm32"))]
fn attach_new(s: &mut Session, nb: &Notebook, path: &str) -> Result<(), CmdError> {
    let store = notecraft_format::store::NotebookStore::create(std::path::Path::new(path), nb).map_err(|e| CmdError::Failed(e.to_string()))?;
    s.stores.insert(nb.id, store);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn attach_new(_: &mut Session, _: &Notebook, _: &str) -> Result<(), CmdError> {
    Err(CmdError::Disabled("the web version keeps notebooks in the browser; folders are not available".into()))
}

fn open_notebook(s: &mut Session, v: &Value) -> CmdResult {
    let path = p::req_str(v, "path")?;
    s.open_path(path)
}

impl Session {
    /// Open the notebook folder at `path` (warnings become notices). Opening a notebook that is
    /// already open just selects it.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_path(&mut self, path: &str) -> CmdResult {
        let root = std::path::Path::new(path);
        let canon = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        if let Some((id, _)) = self.stores.iter().find(|(_, st)| std::fs::canonicalize(&st.root).unwrap_or_else(|_| st.root.clone()) == canon) {
            let id = *id;
            if let Some(first) = self.ws.notebook(id).and_then(|n| n.sections().first().map(|x| x.id)) {
                self.goto_section(first)?;
            }
            return Ok(json!({"notebook": id.to_string(), "alreadyOpen": true}));
        }
        let opened = notecraft_format::store::NotebookStore::open(root, &mut self.ids).map_err(|e| CmdError::Failed(e.to_string()))?;
        let mut nb = opened.notebook;
        let purged = nb.recycle.purge(self.now());
        for w in opened.warnings {
            self.notice(w);
        }
        if self.ws.notebook(nb.id).is_some() {
            // Same id open from another folder (a copied notebook): give this one a new id.
            nb.id = self.new_id();
        }
        let id = nb.id;
        self.stores.insert(id, opened.store);
        self.add_notebook(nb);
        if purged > 0 {
            self.dirty.entry(id).or_default().recycle = true;
        }
        Ok(json!({"notebook": id.to_string()}))
    }

    /// Opening folders is not available on the web build.
    #[cfg(target_arch = "wasm32")]
    pub fn open_path(&mut self, _path: &str) -> CmdResult {
        Err(CmdError::Disabled("the web version can't open folders".into()))
    }

    /// Write every dirty notebook (autosave). Returns the errors (also recorded as notices);
    /// failed sections stay dirty and are retried next time.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_dirty(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        let ids: Vec<_> = self.dirty.keys().copied().collect();
        for id in ids {
            let Some(d) = self.dirty.get(&id).cloned() else { continue };
            let (Some(store), Some(nb)) = (self.stores.get_mut(&id), self.ws.notebook(id)) else {
                // In-memory notebook: nothing to write.
                self.dirty.remove(&id);
                continue;
            };
            let fd = notecraft_format::store::Dirty { sections: d.sections.clone(), structure: d.structure, recycle: d.recycle, all: false };
            match store.save(nb, &fd) {
                Ok(report) => {
                    let mut left = crate::session::NbDirty::default();
                    for (sid, e) in report.failed {
                        left.sections.insert(sid);
                        errors.push(format!("could not save a section of {}: {e}", nb.name));
                    }
                    if left.sections.is_empty() {
                        self.dirty.remove(&id);
                    } else {
                        self.dirty.insert(id, left);
                    }
                }
                Err(e) => errors.push(format!("could not save {}: {e}", nb.name)),
            }
        }
        for e in &errors {
            self.notice(e.clone());
        }
        errors
    }

    /// The web build keeps everything in memory.
    #[cfg(target_arch = "wasm32")]
    pub fn save_dirty(&mut self) -> Vec<String> {
        self.dirty.clear();
        Vec::new()
    }

    /// Autosave if changes are older than `debounce_ms` (call this from the UI loop).
    pub fn autosave_due(&self, debounce_ms: i64) -> bool {
        self.is_dirty() && self.clock.now_ms.saturating_sub(self.last_change_ms) >= debounce_ms
    }

    /// The folder a notebook is stored in, if any.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn notebook_path(&self, id: notecraft_doc::Id) -> Option<std::path::PathBuf> {
        self.stores.get(&id).map(|s| s.root.clone())
    }
}

fn close_notebook(s: &mut Session, v: &Value) -> CmdResult {
    let id = match p::id(v, "notebook")? {
        Some(id) => id,
        None => s.sel.notebook.ok_or_else(|| CmdError::Disabled("open a notebook first".into()))?,
    };
    let errors = s.save_dirty();
    if !errors.is_empty() && s.dirty.contains_key(&id) {
        return Err(CmdError::Failed(format!("the notebook could not be saved, so it stays open: {}", errors.join("; "))));
    }
    let i = s.ws.notebook_index(id).ok_or_else(|| CmdError::Params("no such notebook".into()))?;
    s.ws.notebooks.remove(i);
    #[cfg(not(target_arch = "wasm32"))]
    s.stores.remove(&id);
    s.dirty.remove(&id);
    // Closing is not an edit: the history of a closed notebook can't be undone into.
    s.undo.clear();
    s.redo.clear();
    if s.sel.notebook == Some(id) {
        let next = s.ws.notebooks.first().map(|n| n.id);
        match next.and_then(|n| s.ws.notebook(n).and_then(|nb| nb.sections().first().map(|x| x.id))) {
            Some(sid) => s.goto_section(sid)?,
            None => {
                s.sel = crate::Selection::default();
                s.sel.notebook = next;
            }
        }
    }
    Ok(Value::Null)
}

fn save(s: &mut Session, _: &Value) -> CmdResult {
    let errors = s.save_dirty();
    if errors.is_empty() { Ok(json!({"saved": true})) } else { Err(CmdError::Failed(errors.join("; "))) }
}

fn export_notebook(s: &mut Session, v: &Value) -> CmdResult {
    let path = p::req_str(v, "path")?;
    let nb = match p::id(v, "notebook")? {
        Some(id) => s.ws.notebook(id).ok_or_else(|| CmdError::Params("no such notebook".into()))?,
        None => s.notebook().ok_or_else(no_page)?,
    };
    let bytes = notecraft_format::encode_archive(nb).map_err(|e| CmdError::Failed(e.to_string()))?;
    write_file(path, &bytes)?;
    Ok(json!({"bytes": bytes.len()}))
}

#[cfg(not(target_arch = "wasm32"))]
fn write_file(path: &str, bytes: &[u8]) -> Result<(), CmdError> {
    craft_atomic_write(path, bytes)
}

#[cfg(not(target_arch = "wasm32"))]
fn craft_atomic_write(path: &str, bytes: &[u8]) -> Result<(), CmdError> {
    std::fs::write(path, bytes).map_err(|e| CmdError::Failed(format!("{path}: {e}")))
}

#[cfg(target_arch = "wasm32")]
fn write_file(_: &str, _: &[u8]) -> Result<(), CmdError> {
    Err(CmdError::Disabled("the web version downloads exports instead".into()))
}

#[cfg(not(target_arch = "wasm32"))]
fn backup_label(s: &Session) -> String {
    let c = s.now().civil(s.clock.utc_offset_min);
    format!("{:04}-{:02}-{:02}_{:02}{:02}{:02}", c.year, c.month, c.day, c.hour, c.minute, c.second)
}

#[cfg(not(target_arch = "wasm32"))]
fn backup_now(s: &mut Session, v: &Value) -> CmdResult {
    let folder = p::req_str(v, "folder")?;
    let _ = s.save_dirty();
    let label = backup_label(s);
    let mut done = Vec::new();
    for nb in &s.ws.notebooks {
        let Some(store) = s.stores.get(&nb.id) else { continue };
        let dest_root = std::path::Path::new(folder).join(notecraft_format::file_stem(&nb.name));
        std::fs::create_dir_all(&dest_root).map_err(|e| CmdError::Failed(format!("{}: {e}", dest_root.display())))?;
        let dest = notecraft_format::store::backup(&store.root, &dest_root, &label, s.prefs.backup_copies.max(1))
            .map_err(|e| CmdError::Failed(e.to_string()))?;
        done.push(dest.display().to_string());
    }
    Ok(json!({"backups": done}))
}

#[cfg(target_arch = "wasm32")]
fn backup_now(_: &mut Session, _: &Value) -> CmdResult {
    Err(CmdError::Disabled("backups need a file system".into()))
}

#[cfg(not(target_arch = "wasm32"))]
fn list_backups(s: &mut Session, v: &Value) -> CmdResult {
    let folder = p::req_str(v, "folder")?;
    let mut out = Vec::new();
    for nb in &s.ws.notebooks {
        let dir = std::path::Path::new(folder).join(notecraft_format::file_stem(&nb.name));
        let mut labels: Vec<String> = std::fs::read_dir(&dir)
            .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect())
            .unwrap_or_default();
        labels.sort();
        out.push(json!({"notebook": nb.name, "folder": dir.display().to_string(), "backups": labels}));
    }
    Ok(Value::Array(out))
}

#[cfg(target_arch = "wasm32")]
fn list_backups(_: &mut Session, _: &Value) -> CmdResult {
    Err(CmdError::Disabled("backups need a file system".into()))
}

fn options(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(set) = v.get("set") {
        let mut cur = serde_json::to_value(&s.prefs).map_err(|e| CmdError::Failed(e.to_string()))?;
        let (Some(obj), Some(upd)) = (cur.as_object_mut(), set.as_object()) else { return Err(CmdError::Params("`set` must be an object".into())) };
        for (k, val) in upd {
            if !obj.contains_key(k) {
                return Err(CmdError::Params(format!("unknown option `{k}`")));
            }
            obj.insert(k.clone(), val.clone());
        }
        let mut prefs: crate::Prefs = serde_json::from_value(cur).map_err(|e| CmdError::Params(format!("bad option value: {e}")))?;
        prefs.default_size = prefs.default_size.clamp(notecraft_doc::text::MIN_FONT_SIZE, notecraft_doc::text::MAX_FONT_SIZE);
        prefs.backup_copies = prefs.backup_copies.clamp(1, 100);
        prefs.custom_tags.truncate(notecraft_doc::tags::MAX_CUSTOM_TAGS);
        s.prefs = prefs;
    }
    serde_json::to_value(&s.prefs).map_err(|e| CmdError::Failed(e.to_string()))
}

fn recycle_bin(s: &mut Session, v: &Value) -> CmdResult {
    let id = match p::id(v, "notebook")? {
        Some(id) => id,
        None => s.sel.notebook.ok_or_else(no_page)?,
    };
    let nb = s.ws.notebook(id).ok_or_else(|| CmdError::Params("no such notebook".into()))?;
    let off = s.clock.utc_offset_min;
    let pages: Vec<Value> = nb
        .recycle
        .pages
        .iter()
        .map(|d| json!({"id": d.page.id.to_string(), "title": d.page.display_title(), "from": d.from_section_name, "deleted": d.deleted.short_date(off)}))
        .collect();
    let sections: Vec<Value> = nb
        .recycle
        .sections
        .iter()
        .map(
            |d| json!({"id": d.section.id.to_string(), "name": d.section.name, "pages": d.section.pages.len(), "deleted": d.deleted.short_date(off)}),
        )
        .collect();
    s.view.recycle_view = Some(id);
    Ok(json!({"pages": pages, "sections": sections}))
}

fn empty_bin(s: &mut Session, _: &Value) -> CmdResult {
    let id = s.sel.notebook.ok_or_else(no_page)?;
    let nb = s.ws.notebook_mut(id).ok_or_else(no_page)?;
    nb.recycle = notecraft_doc::RecycleBin::default();
    Ok(Value::Null)
}

fn restore(s: &mut Session, v: &Value) -> CmdResult {
    let item = p::req_id(v, "item")?;
    let nb_id = s.sel.notebook.ok_or_else(no_page)?;
    let nb = s.ws.notebook(nb_id).ok_or_else(no_page)?.clone();
    if let Some(i) = nb.recycle.pages.iter().position(|d| d.page.id == item) {
        let target = match p::id(v, "section")? {
            Some(t) => t,
            None => nb
                .recycle
                .pages
                .get(i)
                .map(|d| d.from_section)
                .filter(|sid| s.ws.section(*sid).is_some())
                .or(s.sel.section)
                .ok_or_else(|| CmdError::Params("choose a `section` to restore into".into()))?,
        };
        if s.ws.section(target).is_none() {
            return Err(CmdError::Params("no such target section".into()));
        }
        let nbm = s.ws.notebook_mut(nb_id).ok_or_else(no_page)?;
        let d = nbm.recycle.pages.remove(i);
        let mut page = (*d.page).clone();
        page.level = 0;
        let pid = page.id;
        let sec = s.ws.section_mut(target).ok_or_else(no_page)?;
        sec.pages.push(Arc::new(page));
        s.goto_page(pid)?;
        return Ok(json!({"page": pid.to_string()}));
    }
    if let Some(i) = nb.recycle.sections.iter().position(|d| d.section.id == item) {
        let nbm = s.ws.notebook_mut(nb_id).ok_or_else(no_page)?;
        let d = nbm.recycle.sections.remove(i);
        let sid = d.section.id;
        nbm.children.push(NavNode::Section(d.section));
        s.goto_section(sid)?;
        return Ok(json!({"section": sid.to_string()}));
    }
    Err(CmdError::Params("that item is not in the recycle bin".into()))
}
