//! Notebooks on disk and the File (backstage) commands: new, open, close, save (autosave),
//! backups, export, the Notebook Recycle Bin.
//!
//! Notebooks live in folders (`notecraft-format`); this module keeps one `NotebookStore` per
//! open notebook and writes only what changed. On the web build there is no file system: new
//! notebooks live in memory and file commands that need a path are disabled.

use std::sync::Arc;

use notecraft_doc::{NavNode, Notebook, Section, SectionColor};
use serde_json::{Value, json};

use crate::ops::{need_notebook, no_page};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn need_bin(s: &Session) -> Option<&'static str> {
    match s.notebook() {
        Some(nb) if !nb.recycle.is_empty() => None,
        Some(_) => Some("The recycle bin is empty"),
        None => Some("Open a notebook first"),
    }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // BS.Exit (QAT.NR.FileExit): save everything, then ask the shell to close.
        CommandSpec::new("file.exit", "BS.Exit", "Exit", "File/Exit", exit).pure(),
        // BS.New.NewNotebook — a folder at `path` (desktop) or an in-memory notebook (web,
        // agents). It starts with one section holding one page (BEH.Notebooks.Default).
        CommandSpec::new("file.newNotebook", "BS.New.NewNotebook", "New Notebook", "File/New/New Notebook", new_notebook)
            .params(r#"{"name": string, "path"?: folder, "memory"?: bool}"#),
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
            .params(r#"{"path": file, "format"?: "archive"|"pdf", "notebook"?: id}"#)
            .pure()
            .when(need_notebook),
        // UI.Dialogs.NotebookProperties.ChangeLocation — store the notebook in a new, empty
        // folder (an imported copy becomes a NoteCraft notebook on disk).
        CommandSpec::new(
            "notebook.changeLocation",
            "UI.Dialogs.NotebookProperties.ChangeLocation",
            "Change Location",
            "Window/Dialogs/Notebook Properties/Change Location",
            change_location,
        )
        .params(r#"{"path": new empty folder, "notebook"?: id}"#)
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
        .params(r#"{"folder"?: backup folder (default: Options › Save & Backup)}"#)
        .pure(),
        // OPT.SaveBackup.BackupChangedNow — only the notebooks changed since the last backup.
        CommandSpec::new(
            "file.backupChanged",
            "OPT.SaveBackup.BackupChangedNow",
            "Back Up Changed Files Now",
            "File/Options/Save & Backup/Back Up Changed Files Now",
            |s, v| backup(s, v, true),
        )
        .params(r#"{"folder"?: backup folder}"#)
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
    // OPT.SaveBackup.DefaultNotebookLocation: without a path, a new notebook goes into the
    // default location of the Options, if one is set (`memory` keeps it in memory).
    let default_path = (!s.prefs.default_notebook_location.trim().is_empty() && p::bool(v, "memory") != Some(true))
        .then(|| std::path::Path::new(s.prefs.default_notebook_location.trim()).join(notecraft_format::file_stem(&name)).display().to_string());
    if let Some(path) = p::str(v, "path").map(str::to_string).or(default_path) {
        attach_new(s, &nb, &path)?;
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

/// Largest notebook archive opened (pictures and attachments are inside it).
#[cfg(not(target_arch = "wasm32"))]
const MAX_ARCHIVE_BYTES: usize = 2 << 30;

/// New ids for every group, section and page under `nodes` (an archive opened twice).
/// Depth is bounded like the notebook model's own nesting.
#[cfg(not(target_arch = "wasm32"))]
fn fresh_ids(s: &mut Session, nodes: &mut [NavNode], depth: usize) {
    if depth > notecraft_doc::notebook::MAX_GROUP_DEPTH {
        return;
    }
    for n in nodes {
        match n {
            NavNode::Group(g) => {
                g.id = s.new_id();
                fresh_ids(s, &mut g.children, depth + 1);
            }
            NavNode::Section(sec) => {
                let sec = Arc::make_mut(sec);
                sec.id = s.new_id();
                for pg in &mut sec.pages {
                    Arc::make_mut(pg).id = s.new_id();
                }
            }
        }
    }
}

/// UI.Dialogs.NotebookProperties.ChangeLocation: write the whole notebook into `path` (new or
/// empty) and keep saving there. The previous folder, if any, is left as it was (the
/// reference moves the notebook; NoteCraft never deletes a folder of notes — unverified).
#[cfg(not(target_arch = "wasm32"))]
fn change_location(s: &mut Session, v: &Value) -> CmdResult {
    let path = p::req_str(v, "path")?;
    let id = match p::id(v, "notebook")? {
        Some(id) => id,
        None => s.notebook().map(|n| n.id).ok_or_else(no_page)?,
    };
    let nb = s.ws.notebook(id).ok_or_else(|| CmdError::Params("no such notebook".into()))?.clone();
    let root = std::path::Path::new(path);
    if let Some(old) = s.stores.get(&id).map(|st| st.root.clone())
        && std::fs::canonicalize(&old).ok().zip(std::fs::canonicalize(root).ok()).is_some_and(|(a, b)| a == b)
    {
        return Ok(json!({"notebook": id.to_string(), "path": path, "unchanged": true}));
    }
    let store = notecraft_format::store::NotebookStore::create(root, &nb).map_err(|e| CmdError::Failed(e.to_string()))?;
    let previous = s.stores.insert(id, store).map(|st| st.root.display().to_string());
    s.dirty.remove(&id);
    s.notice(format!("{} is now saved in {path}", nb.name));
    Ok(json!({"notebook": id.to_string(), "path": path, "previous": previous}))
}

#[cfg(target_arch = "wasm32")]
fn change_location(_: &mut Session, _: &Value) -> CmdResult {
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
        // A OneNote notebook, by its folder or its .onetoc2, is imported as a copy (D12).
        let folder = match crate::onenote_cmds::extension(root).as_deref() {
            Some("onetoc2") => root.parent().unwrap_or(root),
            Some("onepkg") => return self.import_onepkg(root),
            Some("ncraft-archive") => return self.open_archive(root),
            _ => root,
        };
        if crate::onenote_cmds::is_onenote_folder(folder) {
            return self.import_onenote_folder(folder);
        }
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

    /// Open a notebook archive (`FMT.Export.Archive`) as an in-memory notebook; Change
    /// Location stores it in a folder. An archive of a notebook that is already open gets new
    /// notebook, group, section and page ids, so both can be open side by side.
    #[cfg(not(target_arch = "wasm32"))]
    fn open_archive(&mut self, path: &std::path::Path) -> CmdResult {
        let file = path.to_str().ok_or_else(|| CmdError::Failed(format!("{}: the file name is not valid Unicode", path.display())))?;
        let (bytes, _) = crate::picture_cmds::read_path(file, MAX_ARCHIVE_BYTES)?;
        let mut nb = notecraft_format::decode_archive(&bytes).map_err(|e| CmdError::Failed(format!("{file}: {e}")))?;
        drop(bytes);
        if self.ws.notebook(nb.id).is_some() {
            nb.id = self.new_id();
            fresh_ids(self, &mut nb.children, 0);
        }
        let id = nb.id;
        let name = nb.name.clone();
        self.add_notebook(nb);
        self.notice(format!("{name}: opened from an archive, in memory (File › Info › Properties › Change Location keeps it in a folder)"));
        Ok(json!({"notebook": id.to_string(), "archive": true}))
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

    /// The web build stores no notebook in a folder.
    #[cfg(target_arch = "wasm32")]
    pub fn notebook_path(&self, _id: notecraft_doc::Id) -> Option<std::path::PathBuf> {
        None
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
    // BS.Export.FormatPdf: every page of every section, in navigation order.
    let pdf = match p::str(v, "format") {
        Some(f) => f.eq_ignore_ascii_case("pdf"),
        None => path.to_ascii_lowercase().ends_with(".pdf"),
    };
    if pdf {
        let nb = nb.clone();
        let pages: Vec<&notecraft_doc::Page> = nb.sections().iter().flat_map(|sec| sec.pages.iter().map(|p| p.as_ref())).collect();
        return crate::export_cmds::write_pdf(s, path, &nb.name, &pages);
    }
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

/// Exit: every unsaved change is written first; when a notebook can't be saved the window
/// stays open (closing would lose the change) and the error says why.
fn exit(s: &mut Session, _: &Value) -> CmdResult {
    let errors = s.save_dirty();
    if let Some(e) = errors.first() {
        return Err(CmdError::Failed(format!("not closing: {e}")));
    }
    s.quit_request = true;
    Ok(json!({"quit": true}))
}

fn backup_now(s: &mut Session, v: &Value) -> CmdResult {
    backup(s, v, false)
}

impl Session {
    /// Whether an automatic backup is due (`OPT.SaveBackup.AutoBackup`): the interval of the
    /// Options has passed since the last one and some notebook lives in a folder. The host
    /// asks this from its loop and runs `file.backupChanged`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn backup_due(&self) -> bool {
        let every = i64::from(self.prefs.auto_backup_minutes).saturating_mul(60_000);
        every > 0 && !self.stores.is_empty() && self.clock.now_ms.saturating_sub(self.prefs.last_backup_ms) >= every
    }

    /// The web build has no folders to back up.
    #[cfg(target_arch = "wasm32")]
    pub fn backup_due(&self) -> bool {
        false
    }
}

/// The newest change in a notebook (its pages' modification times), Unix ms.
#[cfg(not(target_arch = "wasm32"))]
fn last_change(nb: &Notebook) -> i64 {
    nb.sections().iter().flat_map(|sec| sec.pages.iter().map(|p| p.modified.0)).max().unwrap_or(0)
}

/// Back up every notebook stored in a folder (or, with `changed`, only those changed since
/// the last backup) into `folder`, else the backup folder of the Options.
#[cfg(not(target_arch = "wasm32"))]
fn backup(s: &mut Session, v: &Value, changed: bool) -> CmdResult {
    let folder = match p::str(v, "folder").filter(|f| !f.trim().is_empty()) {
        Some(f) => f.to_string(),
        None if !s.prefs.backup_folder.trim().is_empty() => s.prefs.backup_folder.trim().to_string(),
        None => return Err(CmdError::Params("give `folder`, or set a backup folder in Options › Save & Backup".into())),
    };
    let folder = folder.as_str();
    let _ = s.save_dirty();
    let label = backup_label(s);
    let since = s.prefs.last_backup_ms;
    let mut done = Vec::new();
    for nb in &s.ws.notebooks {
        let Some(store) = s.stores.get(&nb.id) else { continue };
        if changed && since > 0 && last_change(nb) <= since {
            continue;
        }
        let dest_root = std::path::Path::new(folder).join(notecraft_format::file_stem(&nb.name));
        std::fs::create_dir_all(&dest_root).map_err(|e| CmdError::Failed(format!("{}: {e}", dest_root.display())))?;
        let dest = notecraft_format::store::backup(&store.root, &dest_root, &label, s.prefs.backup_copies.max(1))
            .map_err(|e| CmdError::Failed(e.to_string()))?;
        done.push(dest.display().to_string());
    }
    s.prefs.last_backup_ms = s.clock.now_ms;
    Ok(json!({"backups": done}))
}

#[cfg(target_arch = "wasm32")]
fn backup(_: &mut Session, _: &Value, _: bool) -> CmdResult {
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
        // At most every eight weeks (the reference's longest interval is six weeks; unverified).
        prefs.auto_backup_minutes = prefs.auto_backup_minutes.min(8 * 7 * 24 * 60);
        prefs.initials = prefs.initials.chars().take(9).collect();
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
