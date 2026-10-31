//! Notebook folders on disk (native only).
//!
//! [`NotebookStore`] remembers where each section file and group folder of one open notebook
//! lives, writes only what changed, renames files when sections are renamed or moved, and
//! deletes only files it manages (`*.ncraft` it wrote or loaded, its own TOC files, empty group
//! folders it created): a user's stray files in a notebook folder are never touched.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use notecraft_doc::{IdGen, NavNode, Notebook, RecycleBin, SectionColor, SectionGroup, clean_name};

use crate::{
    FormatError, NameAlloc, RECYCLE_DIR, RECYCLE_FILE, Result, SECTION_EXT, TOC_FILE, TocEntry, decode_recycle, decode_section, decode_toc,
    encode_recycle, encode_section, encode_toc, file_stem, toc_for,
};

/// How deep group folders are followed on load (mirrors `notecraft_doc::notebook::MAX_GROUP_DEPTH`).
const MAX_DEPTH: usize = 16;
/// Suffix of `craft-atomic` staging files (`.{stem}.{pid}-{n}.craft_tmp`).
const STAGING_SUFFIX: &str = ".craft_tmp";

fn io(path: &Path, source: std::io::Error) -> FormatError {
    FormatError::Io { path: path.display().to_string(), source }
}

/// What changed since the last save.
#[derive(Debug, Default, Clone)]
pub struct Dirty {
    /// Sections whose content changed.
    pub sections: HashSet<notecraft_doc::Id>,
    /// The tree (order, names, groups, colours, notebook name) changed.
    pub structure: bool,
    /// The recycle bin changed.
    pub recycle: bool,
    /// Rewrite everything.
    pub all: bool,
}

impl Dirty {
    /// Everything dirty.
    pub fn everything() -> Dirty {
        Dirty { all: true, structure: true, recycle: true, ..Dirty::default() }
    }
}

/// Result of a save: what was written and what failed (and stays dirty).
#[derive(Debug, Default)]
pub struct SaveReport {
    /// Sections written.
    pub written: Vec<notecraft_doc::Id>,
    /// Sections that failed, with the error (the caller keeps them dirty and retries).
    pub failed: Vec<(notecraft_doc::Id, String)>,
}

/// A loaded notebook folder.
#[derive(Debug)]
pub struct NotebookStore {
    /// The notebook folder.
    pub root: PathBuf,
    /// Relative path of each section file / group folder we manage, by id.
    layout: HashMap<notecraft_doc::Id, PathBuf>,
    /// Last bytes written per TOC (relative folder), to skip identical rewrites.
    tocs: HashMap<PathBuf, Vec<u8>>,
}

/// What [`NotebookStore::open`] found.
#[derive(Debug)]
pub struct Opened {
    /// The notebook.
    pub notebook: Notebook,
    /// Its store.
    pub store: NotebookStore,
    /// Problems that were recovered from (shown to the user).
    pub warnings: Vec<String>,
}

fn is_section_file(p: &Path) -> bool {
    p.extension().is_some_and(|e| e == SECTION_EXT)
}

fn sweep_staging(dir: &Path, depth: usize, warnings: &mut Vec<String>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && name.ends_with(STAGING_SUFFIX) && p.is_file() {
            // An interrupted save left this; the real file beside it is intact.
            if std::fs::remove_file(&p).is_ok() {
                warnings.push(format!("removed an interrupted save: {}", p.display()));
            }
        } else if p.is_dir() && name != RECYCLE_DIR && !name.starts_with('.') {
            sweep_staging(&p, depth + 1, warnings);
        }
    }
}

impl NotebookStore {
    /// Open the notebook folder at `root`. Never fails for damaged content inside the folder:
    /// unreadable sections are skipped with a warning, sections missing from the table of
    /// contents are recovered, a missing table of contents is rebuilt from the files. Fails only
    /// if `root` is not a readable folder. `ids` mints ids for anything that lacks one.
    pub fn open(root: &Path, ids: &mut IdGen) -> Result<Opened> {
        let meta = std::fs::metadata(root).map_err(|e| io(root, e))?;
        if !meta.is_dir() {
            return Err(FormatError::Corrupt(format!("{} is not a notebook folder", root.display())));
        }
        let mut warnings = Vec::new();
        sweep_staging(root, 0, &mut warnings);
        let mut store = NotebookStore { root: root.to_path_buf(), layout: HashMap::new(), tocs: HashMap::new() };
        let folder_name = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Notebook".into());
        let (toc, children) = store.load_dir(Path::new(""), 0, ids, &mut warnings);
        let (id, name, color) = match &toc {
            Some(t) => (t.id, t.name.clone(), t.color),
            None => (ids.next_id(), folder_name, SectionColor::default()),
        };
        let mut nb = Notebook::new(id, &name, color);
        nb.children = children;
        let rpath = root.join(RECYCLE_DIR).join(RECYCLE_FILE);
        if rpath.exists() {
            match std::fs::read(&rpath).map_err(|e| io(&rpath, e)).and_then(|b| decode_recycle(&b)) {
                Ok(bin) => nb.recycle = bin,
                Err(e) => warnings.push(format!("recycle bin unreadable, starting empty: {e}")),
            }
        }
        nb.normalize();
        Ok(Opened { notebook: nb, store, warnings })
    }

    fn load_dir(&mut self, rel: &Path, depth: usize, ids: &mut IdGen, warnings: &mut Vec<String>) -> (Option<crate::Toc>, Vec<NavNode>) {
        let dir = self.root.join(rel);
        let toc_path = dir.join(TOC_FILE);
        let toc = if toc_path.exists() {
            match std::fs::read(&toc_path).map_err(|e| io(&toc_path, e)).and_then(|b| decode_toc(&b)) {
                Ok(t) => {
                    self.tocs.insert(rel.to_path_buf(), Vec::new());
                    Some(t)
                }
                Err(e) => {
                    warnings.push(format!("{}: {e}; rebuilt from the files in the folder", toc_path.display()));
                    None
                }
            }
        } else {
            None
        };
        let mut children = Vec::new();
        let mut seen_files: HashSet<String> = HashSet::new();
        let mut seen_ids = HashSet::new();
        if let Some(t) = &toc {
            for e in &t.entries {
                match e {
                    TocEntry::Section { id, file } => {
                        seen_files.insert(file.to_lowercase());
                        self.load_section(rel, file, Some(*id), ids, &mut seen_ids, &mut children, warnings);
                    }
                    TocEntry::Group { id, name, dir: d } => {
                        seen_files.insert(d.to_lowercase());
                        if depth >= MAX_DEPTH {
                            warnings.push(format!("section group {name:?} nested too deeply; skipped"));
                            continue;
                        }
                        let sub = rel.join(d);
                        if !self.root.join(&sub).is_dir() {
                            warnings.push(format!("section group folder {} is missing", self.root.join(&sub).display()));
                            continue;
                        }
                        let (_, kids) = self.load_dir(&sub, depth + 1, ids, warnings);
                        self.layout.insert(*id, sub);
                        children.push(NavNode::Group(SectionGroup { id: *id, name: clean_name(name), children: kids }));
                    }
                }
            }
        }
        // Recover what the TOC doesn't list (a crash between writing a section and the TOC, or
        // files copied in by the user), in name order for determinism.
        let mut extra: Vec<(String, bool)> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        let n = e.file_name().to_string_lossy().to_string();
                        let p = e.path();
                        if n.starts_with('.') || seen_files.contains(&n.to_lowercase()) {
                            return None;
                        }
                        if p.is_file() && is_section_file(&p) {
                            Some((n, false))
                        } else if p.is_dir() && p.join(TOC_FILE).exists() && depth < MAX_DEPTH {
                            Some((n, true))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        extra.sort();
        for (n, is_dir) in extra {
            if is_dir {
                let sub = rel.join(&n);
                let (sub_toc, kids) = self.load_dir(&sub, depth + 1, ids, warnings);
                let (gid, gname) = match sub_toc {
                    Some(t) => (t.id, t.name),
                    None => (ids.next_id(), n.clone()),
                };
                self.layout.insert(gid, sub);
                children.push(NavNode::Group(SectionGroup { id: gid, name: clean_name(&gname), children: kids }));
                if toc.is_some() {
                    warnings.push(format!("recovered section group {n:?} missing from the table of contents"));
                }
            } else {
                self.load_section(rel, &n, None, ids, &mut seen_ids, &mut children, warnings);
                if toc.is_some() {
                    warnings.push(format!("recovered section file {n:?} missing from the table of contents"));
                }
            }
        }
        (toc, children)
    }

    fn load_section(
        &mut self,
        rel: &Path,
        file: &str,
        expect: Option<notecraft_doc::Id>,
        ids: &mut IdGen,
        seen_ids: &mut HashSet<notecraft_doc::Id>,
        children: &mut Vec<NavNode>,
        warnings: &mut Vec<String>,
    ) {
        let rp = rel.join(file);
        let p = self.root.join(&rp);
        match std::fs::read(&p).map_err(|e| io(&p, e)).and_then(|b| decode_section(&b)) {
            Ok(mut s) => {
                if let Some(id) = expect
                    && id != s.id
                {
                    warnings.push(format!("{}: id differs from the table of contents; using the file's", p.display()));
                }
                if s.id.is_nil() || !seen_ids.insert(s.id) {
                    s.id = ids.next_id();
                    warnings.push(format!("{}: duplicate section id; assigned a new one", p.display()));
                }
                self.layout.insert(s.id, rp);
                children.push(NavNode::Section(Arc::new(s)));
            }
            Err(e) => warnings.push(format!("section skipped: {e}")),
        }
    }

    /// Create a new notebook folder at `root` (which must not exist, or be an empty folder) and
    /// write `nb` into it.
    pub fn create(root: &Path, nb: &Notebook) -> Result<NotebookStore> {
        if root.exists() {
            let empty = std::fs::read_dir(root).map_err(|e| io(root, e))?.next().is_none();
            if !empty {
                return Err(FormatError::Corrupt(format!("{} already exists and is not empty", root.display())));
            }
        }
        std::fs::create_dir_all(root).map_err(|e| io(root, e))?;
        let mut store = NotebookStore { root: root.to_path_buf(), layout: HashMap::new(), tocs: HashMap::new() };
        let report = store.save(nb, &Dirty::everything())?;
        if let Some((_, e)) = report.failed.first() {
            return Err(FormatError::Corrupt(e.clone()));
        }
        Ok(store)
    }

    /// Write what changed. Section files are written atomically one by one; a failure is
    /// reported in [`SaveReport::failed`] and the rest continues. TOC failures are errors.
    pub fn save(&mut self, nb: &Notebook, dirty: &Dirty) -> Result<SaveReport> {
        let mut report = SaveReport::default();
        let mut new_layout: HashMap<notecraft_doc::Id, PathBuf> = HashMap::new();
        let mut tocs: Vec<(PathBuf, Vec<u8>)> = Vec::new();
        self.plan(Path::new(""), &nb.children, 0, &mut new_layout, &mut tocs, Some((nb.id, nb.name.as_str(), nb.color)))?;

        // Group folders first, so section files have somewhere to go.
        for (id, rp) in &new_layout {
            if !self.is_section(nb, *id) {
                let p = self.root.join(rp);
                std::fs::create_dir_all(&p).map_err(|e| io(&p, e))?;
            }
        }
        // Sections: write when dirty, new, or moved/renamed.
        for s in nb.sections() {
            let Some(target) = new_layout.get(&s.id) else { continue };
            let moved = self.layout.get(&s.id) != Some(target);
            if dirty.all || moved || dirty.sections.contains(&s.id) {
                let p = self.root.join(target);
                match encode_section(s).and_then(|b| craft_atomic::atomic_write(&p, &b).map_err(|e| io(&p, e))) {
                    Ok(()) => report.written.push(s.id),
                    Err(e) => {
                        report.failed.push((s.id, e.to_string()));
                        // Keep the old file mapping so it isn't deleted below.
                        if let Some(old) = self.layout.get(&s.id) {
                            new_layout.insert(s.id, old.clone());
                        }
                    }
                }
            }
        }
        // Tables of contents (skip identical rewrites).
        for (rel, bytes) in &tocs {
            if dirty.all || dirty.structure || self.tocs.get(rel) != Some(bytes) {
                let p = self.root.join(rel).join(TOC_FILE);
                craft_atomic::atomic_write(&p, bytes).map_err(|e| io(&p, e))?;
                self.tocs.insert(rel.clone(), bytes.clone());
            }
        }
        if dirty.all || dirty.recycle {
            self.save_recycle(&nb.recycle)?;
        }
        // Remove files we managed that are no longer part of the notebook (deleted sections
        // live on in the recycle bin file), and group folders we emptied.
        let keep: HashSet<&PathBuf> = new_layout.values().collect();
        let mut stale_dirs = Vec::new();
        for (id, old) in &self.layout {
            if keep.contains(old) {
                continue;
            }
            let p = self.root.join(old);
            if is_section_file(&p) {
                let _ = std::fs::remove_file(&p);
            } else if !new_layout.contains_key(id) || new_layout.get(id) != Some(old) {
                stale_dirs.push(p);
            }
        }
        // Deepest first.
        stale_dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
        for d in stale_dirs {
            let _ = std::fs::remove_file(d.join(TOC_FILE));
            let _ = std::fs::remove_dir(&d); // only succeeds when empty: user files are safe
        }
        self.tocs.retain(|rel, _| tocs.iter().any(|(r, _)| r == rel));
        self.layout = new_layout;
        Ok(report)
    }

    fn is_section(&self, nb: &Notebook, id: notecraft_doc::Id) -> bool {
        nb.section(id).is_some()
    }

    fn plan(
        &self,
        rel: &Path,
        nodes: &[NavNode],
        depth: usize,
        layout: &mut HashMap<notecraft_doc::Id, PathBuf>,
        tocs: &mut Vec<(PathBuf, Vec<u8>)>,
        head: Option<(notecraft_doc::Id, &str, SectionColor)>,
    ) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(FormatError::Corrupt("section groups nested too deeply to save".into()));
        }
        let mut alloc = NameAlloc::default();
        alloc.reserve(TOC_FILE);
        alloc.reserve(RECYCLE_DIR);
        // Keep existing names where they are still right, so renaming one section never
        // renames its siblings' files.
        let mut entries = Vec::new();
        let mut planned: Vec<Option<String>> = vec![None; nodes.len()];
        for (i, n) in nodes.iter().enumerate() {
            if let Some(old) = self.layout.get(&n.id())
                && old.parent().unwrap_or(Path::new("")) == rel
                && let Some(fname) = old.file_name().map(|f| f.to_string_lossy().to_string())
            {
                let stem_ok = match n {
                    NavNode::Section(s) => fname
                        .strip_suffix(&format!(".{SECTION_EXT}"))
                        .is_some_and(|st| st == file_stem(&s.name) || st.starts_with(&format!("{} (", file_stem(&s.name)))),
                    NavNode::Group(g) => fname == file_stem(&g.name) || fname.starts_with(&format!("{} (", file_stem(&g.name))),
                };
                if stem_ok {
                    alloc.reserve(&fname);
                    if let Some(slot) = planned.get_mut(i) {
                        *slot = Some(fname);
                    }
                }
            }
        }
        for (i, n) in nodes.iter().enumerate() {
            let fname = match planned.get(i).cloned().flatten() {
                Some(f) => f,
                None => match n {
                    NavNode::Section(s) => alloc.take(&file_stem(&s.name), SECTION_EXT),
                    NavNode::Group(g) => alloc.take(&file_stem(&g.name), ""),
                },
            };
            let rp = rel.join(&fname);
            match n {
                NavNode::Section(s) => {
                    layout.insert(s.id, rp);
                    entries.push(TocEntry::Section { id: s.id, file: fname });
                }
                NavNode::Group(g) => {
                    layout.insert(g.id, rp.clone());
                    entries.push(TocEntry::Group { id: g.id, name: g.name.clone(), dir: fname });
                    self.plan(&rp, &g.children, depth + 1, layout, tocs, Some((g.id, g.name.as_str(), SectionColor::default())))?;
                }
            }
        }
        if let Some((id, name, color)) = head {
            tocs.push((rel.to_path_buf(), encode_toc(&toc_for(id, name, color, entries))?));
        }
        Ok(())
    }

    fn save_recycle(&self, bin: &RecycleBin) -> Result<()> {
        let dir = self.root.join(RECYCLE_DIR);
        let p = dir.join(RECYCLE_FILE);
        if bin.is_empty() && !p.exists() {
            return Ok(());
        }
        std::fs::create_dir_all(&dir).map_err(|e| io(&dir, e))?;
        let bytes = encode_recycle(bin)?;
        craft_atomic::atomic_write(&p, &bytes).map_err(|e| io(&p, e))
    }
}

/// Copy every managed file of the notebook folder `root` into
/// `backup_root/<label>/` (keeping the folder structure), then delete the oldest backups so at
/// most `keep` remain (catalog `BEH.Save.Backups`, `OPT.SaveBackup.BackupCopies`). `label`
/// must sort chronologically (e.g. `2026-10-14_1945`). Returns the new backup folder.
pub fn backup(root: &Path, backup_root: &Path, label: &str, keep: usize) -> Result<PathBuf> {
    if !crate::is_safe_component(label) {
        return Err(FormatError::Corrupt(format!("bad backup label {label:?}")));
    }
    let dest = backup_root.join(label);
    fn copy_tree(src: &Path, dst: &Path, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH {
            return Ok(());
        }
        std::fs::create_dir_all(dst).map_err(|e| io(dst, e))?;
        let rd = std::fs::read_dir(src).map_err(|e| io(src, e))?;
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name();
            let n = name.to_string_lossy();
            if n.ends_with(STAGING_SUFFIX) {
                continue;
            }
            if p.is_dir() {
                if n == RECYCLE_DIR || p.join(TOC_FILE).exists() {
                    copy_tree(&p, &dst.join(&name), depth + 1)?;
                }
            } else if is_section_file(&p) || n == TOC_FILE || n == RECYCLE_FILE {
                let bytes = std::fs::read(&p).map_err(|e| io(&p, e))?;
                let out = dst.join(&name);
                craft_atomic::atomic_write(&out, &bytes).map_err(|e| io(&out, e))?;
            }
        }
        Ok(())
    }
    copy_tree(root, &dest, 0)?;
    // Prune: backup folders sort by label.
    let mut all: Vec<PathBuf> =
        std::fs::read_dir(backup_root).map_err(|e| io(backup_root, e))?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    all.sort();
    let excess = all.len().saturating_sub(keep.max(1));
    for old in all.into_iter().take(excess) {
        let _ = std::fs::remove_dir_all(&old);
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use notecraft_doc::{Id, Page, Paragraph, Section, Timestamp};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("notecraft-format-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn sec(id: u128, name: &str, page_title: &str) -> NavNode {
        let mut s = Section::new(Id(id), name, SectionColor::index(0), Timestamp(0));
        let mut p = Page::new(Id(id + 1000), Id(id + 2000), Timestamp(0), "");
        p.title = Paragraph::with_text(Id(id + 2000), page_title);
        s.pages.push(Arc::new(p));
        NavNode::Section(Arc::new(s))
    }

    fn notebook() -> Notebook {
        let mut nb = Notebook::new(Id(1), "Work", SectionColor::default());
        nb.children = vec![
            sec(10, "Meetings", "Kickoff"),
            NavNode::Group(SectionGroup {
                id: Id(20),
                name: "Projects".into(),
                children: vec![sec(21, "Alpha", "Plan"), sec(22, "Beta: v2", "Spec")],
            }),
        ];
        nb
    }

    #[test]
    fn create_open_round_trip() {
        let root = tmp("rt");
        let nb = notebook();
        NotebookStore::create(&root, &nb).unwrap();
        assert!(root.join(TOC_FILE).exists());
        assert!(root.join("Meetings.ncraft").exists());
        assert!(root.join("Projects").join("Beta_ v2.ncraft").exists());
        let mut ids = IdGen::new(9);
        let o = NotebookStore::open(&root, &mut ids).unwrap();
        assert!(o.warnings.is_empty(), "{:?}", o.warnings);
        assert_eq!(o.notebook.name, "Work");
        let names: Vec<_> = o.notebook.sections().iter().map(|s| s.name.clone()).collect();
        assert_eq!(names, vec!["Meetings", "Alpha", "Beta: v2"]);
        assert_eq!(o.notebook.group(Id(20)).map(|g| g.name.as_str()), Some("Projects"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rename_move_delete_keep_disk_in_step() {
        let root = tmp("mv");
        let mut nb = notebook();
        let mut store = NotebookStore::create(&root, &nb).unwrap();
        // Rename a section, move Alpha out of the group, delete the group's other section.
        nb.section_mut(Id(10)).unwrap().name = "Standups".into();
        let alpha = match nb.children_at_mut(&[1]).unwrap().remove(0) {
            n @ NavNode::Section(_) => n,
            _ => panic!("expected a section"),
        };
        nb.children.push(alpha);
        nb.children_at_mut(&[1]).unwrap().clear();
        let report = store.save(&nb, &Dirty { structure: true, ..Dirty::default() }).unwrap();
        assert!(report.failed.is_empty());
        assert!(!root.join("Meetings.ncraft").exists());
        assert!(root.join("Standups.ncraft").exists());
        assert!(root.join("Alpha.ncraft").exists());
        assert!(!root.join("Projects").join("Beta_ v2.ncraft").exists());
        let o = NotebookStore::open(&root, &mut IdGen::new(3)).unwrap();
        assert_eq!(o.notebook.sections().len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn recovery_from_missing_toc_entries_staging_files_and_corrupt_sections() {
        let root = tmp("rec");
        let nb = notebook();
        NotebookStore::create(&root, &nb).unwrap();
        // A section written but never added to the TOC (crash between the two writes).
        let extra = match sec(30, "Orphan", "Lost?") {
            NavNode::Section(s) => s,
            _ => panic!("expected a section"),
        };
        std::fs::write(root.join("Orphan.ncraft"), crate::encode_section(&extra).unwrap()).unwrap();
        // An interrupted atomic write.
        std::fs::write(root.join(".Meetings.ncraft.123-1.craft_tmp"), b"partial").unwrap();
        // A corrupt section listed in the TOC.
        std::fs::write(root.join("Projects").join("Alpha.ncraft"), b"garbage").unwrap();
        // A user's unrelated file.
        std::fs::write(root.join("readme.txt"), b"mine").unwrap();
        let o = NotebookStore::open(&root, &mut IdGen::new(4)).unwrap();
        let names: Vec<_> = o.notebook.sections().iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&"Orphan".to_string()), "{names:?}");
        assert!(!names.contains(&"Alpha".to_string()));
        assert!(!root.join(".Meetings.ncraft.123-1.craft_tmp").exists());
        assert!(o.warnings.iter().any(|w| w.contains("recovered")));
        assert!(o.warnings.iter().any(|w| w.contains("skipped")));
        // Saving never deletes the user's file.
        let mut store = o.store;
        store.save(&o.notebook, &Dirty::everything()).unwrap();
        assert!(root.join("readme.txt").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_toc_rebuilds_from_files() {
        let root = tmp("notoc");
        NotebookStore::create(&root, &notebook()).unwrap();
        std::fs::remove_file(root.join(TOC_FILE)).unwrap();
        let o = NotebookStore::open(&root, &mut IdGen::new(5)).unwrap();
        assert_eq!(o.notebook.sections().len(), 3);
        assert!(o.notebook.name.starts_with("notecraft-format-notoc"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn backups_rotate() {
        // parity: BEH.Save.Backups
        let root = tmp("bk");
        let broot = tmp("bk-dest");
        NotebookStore::create(&root, &notebook()).unwrap();
        for label in ["2026-01-01_0000", "2026-01-02_0000", "2026-01-03_0000"] {
            backup(&root, &broot, label, 2).unwrap();
        }
        let mut left: Vec<String> = std::fs::read_dir(&broot).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        left.sort();
        assert_eq!(left, vec!["2026-01-02_0000", "2026-01-03_0000"]);
        assert!(broot.join("2026-01-03_0000").join("Projects").join("Alpha.ncraft").exists());
        assert!(backup(&root, &broot, "../x", 2).is_err());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&broot);
    }

    #[test]
    fn open_rejects_non_folders() {
        assert!(NotebookStore::open(Path::new("/definitely/not/here"), &mut IdGen::new(1)).is_err());
    }
}
