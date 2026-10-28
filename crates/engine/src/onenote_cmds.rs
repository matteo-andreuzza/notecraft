//! OneNote's own files (decision D12): File › Open › Open Section imports a `.one` section file
//! into the current notebook as a new section, and File › Open › Browse on a OneNote notebook
//! folder (or its `.onetoc2`) imports the whole notebook as an in-memory copy. The original
//! files are only read, never changed; what the reader does not import yet is listed in the
//! notice and the command's result.

use std::sync::Arc;

use notecraft_doc::{NavNode, SectionColor};
use serde_json::{Value, json};

use crate::ops::need_notebook;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// The largest `.one` file read (the reader's own limit is higher; a section this large is
/// already far beyond what the reference writes).
const MAX_ONE_BYTES: usize = 1 << 30;

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // BS.Open.OpenSection — Ctrl+Alt+Shift+O.
        CommandSpec::new("file.openSection", "BS.Open.OpenSection", "Open Section", "File/Open/Open Section", open_section)
            .key("Mod+Alt+Shift+O")
            .params(r#"{"path": .one file | "data": base64 with "name", "name"?: section name, "notebook"?: id} → {section, page?, imported, notImported, damaged}"#)
            .when(need_notebook),
    ]
}

fn open_section(s: &mut Session, v: &Value) -> CmdResult {
    let nb_id = match p::id(v, "notebook")? {
        Some(id) => id,
        None => s.sel.notebook.ok_or_else(|| CmdError::Disabled("open a notebook first".into()))?,
    };
    let nb = s.ws.notebook(nb_id).ok_or_else(|| CmdError::Params("no such notebook".into()))?.clone();
    let (bytes, file_name) = crate::picture_cmds::read_input(v, MAX_ONE_BYTES)?;
    let stem = p::str(v, "name").map(str::to_string).or_else(|| file_name.map(|n| n.strip_suffix(".one").unwrap_or(&n).to_string()));
    let base = stem.map(|n| notecraft_doc::clean_name(&n)).filter(|n| !n.is_empty()).unwrap_or_else(|| "Imported Section".into());
    let name = unique_name(&nb, &base);
    let now = s.now();
    let import = notecraft_onestore::read_section(&bytes, &name, &mut s.ids, now).map_err(|e| CmdError::Failed(e.to_string()))?;
    let mut section = import.section;
    section.color = SectionColor::rotation(nb.sections().len());
    let sid = section.id;
    let first = section.pages.first().map(|pg| pg.id);
    let report = import.report;
    let nbm = s.ws.notebook_mut(nb_id).ok_or_else(|| CmdError::Params("no such notebook".into()))?;
    nbm.children.push(NavNode::Section(Arc::new(section)));
    match first {
        Some(pid) => s.goto_page(pid)?,
        None => s.goto_section(sid)?,
    }
    s.notice(format!("{name}: {}", report.summary()));
    Ok(json!({
        "section": sid.to_string(),
        "page": first.map(|id| id.to_string()),
        "imported": report.kept,
        "notImported": report.lost,
        "damaged": report.warnings.len(),
    }))
}

/// Most section files one notebook import reads.
#[cfg(not(target_arch = "wasm32"))]
const MAX_NOTEBOOK_SECTIONS: usize = 10_000;
/// Most bytes one notebook import reads in all (each file is also capped by [`MAX_ONE_BYTES`]).
#[cfg(not(target_arch = "wasm32"))]
const MAX_NOTEBOOK_BYTES: u64 = 4 << 30;
/// Deepest section group nesting followed (folders can be deep or linked in loops).
#[cfg(not(target_arch = "wasm32"))]
const MAX_GROUP_DEPTH: usize = 16;

/// The lower-case extension of `path` ("one", "onetoc2"…).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn extension(path: &std::path::Path) -> Option<String> {
    path.extension().map(|e| e.to_string_lossy().to_lowercase())
}

/// Whether `dir` is a OneNote notebook folder: no NoteCraft table of contents, and a
/// `.onetoc2` or `.one` file directly inside it.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn is_onenote_folder(dir: &std::path::Path) -> bool {
    if dir.join(notecraft_format::TOC_FILE).exists() {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    entries.flatten().any(|e| matches!(extension(&e.path()).as_deref(), Some("one" | "onetoc2")))
}

/// The walk over a OneNote notebook folder.
#[cfg(not(target_arch = "wasm32"))]
struct FolderImport {
    report: notecraft_onestore::Report,
    sections: usize,
    bytes: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl FolderImport {
    /// The sections (`.one` files) and section groups (subfolders holding some) of `dir`, by
    /// name: the order the `.onetoc2` keeps is not in the public [MS-ONE] (its PropertyIDs are
    /// not published), so the folder order stands in for it.
    fn dir(&mut self, s: &mut Session, dir: &std::path::Path, depth: usize) -> Vec<NavNode> {
        let entries = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) => {
                self.report.warnings.push(format!("{}: {e}", dir.display()));
                return Vec::new();
            }
        };
        // Symbolic links are not followed: a link back up would loop.
        let mut entries: Vec<(String, std::path::PathBuf, bool)> = entries
            .flatten()
            .filter_map(|e| {
                let kind = e.file_type().ok()?;
                (!kind.is_symlink()).then(|| (e.file_name().to_string_lossy().to_string(), e.path(), kind.is_dir()))
            })
            // Hidden entries and OneNote's own recycle bin folder (deleted pages) are skipped.
            .filter(|(name, ..)| !name.starts_with('.') && !name.eq_ignore_ascii_case("OneNote_RecycleBin"))
            .collect();
        entries.sort_by_key(|(name, ..)| name.to_lowercase());
        let mut out = Vec::new();
        for (name, path, is_dir) in entries {
            if is_dir {
                if depth + 1 >= MAX_GROUP_DEPTH {
                    self.report.warnings.push(format!("{}: section groups nested too deeply", path.display()));
                    continue;
                }
                let children = self.dir(s, &path, depth + 1);
                if !children.is_empty() {
                    let name = notecraft_doc::clean_name(&name);
                    out.push(NavNode::Group(notecraft_doc::SectionGroup { id: s.new_id(), name, children }));
                }
                continue;
            }
            match extension(&path).as_deref() {
                Some("one") => {
                    if let Some(section) = self.section(s, &path) {
                        out.push(NavNode::Section(Arc::new(section)));
                    }
                }
                Some("onetoc2") => self.report.lose_one("section order and colours (.onetoc2 is not in the public spec)"),
                _ => {}
            }
        }
        out
    }

    /// One `.one` file as a section named after the file (§2.2.94: the file name overrides
    /// the name the notebook stores).
    fn section(&mut self, s: &mut Session, path: &std::path::Path) -> Option<notecraft_doc::Section> {
        if self.sections >= MAX_NOTEBOOK_SECTIONS {
            self.report.lose_one("section files over the notebook limit");
            return None;
        }
        let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if self.bytes.saturating_add(len) > MAX_NOTEBOOK_BYTES {
            self.report.lose_one("section files over the notebook size limit");
            return None;
        }
        let shown = path.display().to_string();
        let Some(file) = path.to_str() else {
            self.report.warnings.push(format!("{shown}: the file name is not valid Unicode"));
            return None;
        };
        let bytes = match crate::picture_cmds::read_path(file, MAX_ONE_BYTES) {
            Ok((bytes, _)) => bytes,
            Err(e) => {
                self.report.warnings.push(e.to_string());
                return None;
            }
        };
        self.bytes = self.bytes.saturating_add(bytes.len() as u64);
        let stem = path.file_stem().map(|n| notecraft_doc::clean_name(&n.to_string_lossy())).filter(|n| !n.is_empty());
        let name = stem.unwrap_or_else(|| "Imported Section".into());
        let now = s.now();
        match notecraft_onestore::read_section(&bytes, &name, &mut s.ids, now) {
            Ok(import) => {
                let mut section = import.section;
                section.color = SectionColor::rotation(self.sections);
                self.sections += 1;
                self.report.absorb(import.report);
                Some(section)
            }
            Err(e) => {
                self.report.warnings.push(format!("{shown}: {e}"));
                None
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Session {
    /// Imports the OneNote notebook folder `root` as a new in-memory notebook (decision D12:
    /// a copy; nothing is ever written into the folder).
    pub(crate) fn import_onenote_folder(&mut self, root: &std::path::Path) -> CmdResult {
        let mut walk = FolderImport { report: notecraft_onestore::Report::default(), sections: 0, bytes: 0 };
        let children = walk.dir(self, root, 0);
        if walk.sections == 0 {
            return Err(CmdError::Failed(format!("{}: no OneNote section (.one) could be read", root.display())));
        }
        let folder = root.file_name().map(|n| notecraft_doc::clean_name(&n.to_string_lossy())).filter(|n| !n.is_empty());
        let name = folder.unwrap_or_else(|| "OneNote Notebook".into());
        let mut nb = notecraft_doc::Notebook::new(self.new_id(), &name, SectionColor::default());
        nb.children = children;
        nb.normalize();
        let id = nb.id;
        self.add_notebook(nb);
        let report = walk.report;
        self.notice(format!("{name}: a copy in memory (export it to keep it); {}", report.summary()));
        Ok(json!({
            "notebook": id.to_string(),
            "sections": walk.sections,
            "imported": report.kept,
            "notImported": report.lost,
            "damaged": report.warnings.len(),
        }))
    }
}

/// `base`, or `base 2`, `base 3`… when the notebook already has a section of that name.
fn unique_name(nb: &notecraft_doc::Notebook, base: &str) -> String {
    let names: Vec<String> = nb.sections().iter().map(|s| s.name.to_lowercase()).collect();
    if !names.contains(&base.to_lowercase()) {
        return base.to_string();
    }
    (2..).map(|n| format!("{base} {n}")).find(|c| !names.contains(&c.to_lowercase())).unwrap_or_else(|| base.to_string())
}

#[cfg(test)]
mod tests {
    use notecraft_onestore::writer::section::{FixtureOutline, FixturePage, write};
    use serde_json::json;

    use crate::tests::{run, sample};

    fn fixture() -> Vec<u8> {
        write(&[
            FixturePage {
                title: "Trip".into(),
                level: 1,
                outlines: vec![FixtureOutline {
                    x: 2.0,
                    y: 4.0,
                    paragraphs: vec![(0, "Tickets".into()), (1, "Train".into())],
                    ..FixtureOutline::default()
                }],
                ..FixturePage::default()
            },
            FixturePage { title: "Hotel".into(), level: 2, ..FixturePage::default() },
        ])
    }

    #[test]
    fn open_section_imports_a_one_file_as_a_new_section() {
        let mut s = sample();
        let row = "BS.Open.OpenSection";
        let before = s.notebook().unwrap().sections().len();
        let r = run(&mut s, "file.openSection", json!({"data": crate::b64::encode(&fixture()), "name": "Travel"}));
        assert_eq!(r["imported"]["pages"], 2, "{row}");
        assert_eq!(r["damaged"], 0);
        let nb = s.notebook().unwrap();
        assert_eq!(nb.sections().len(), before + 1);
        let sec = std::sync::Arc::clone(nb.sections().last().unwrap());
        assert_eq!(sec.name, "Travel");
        assert_eq!(sec.pages.iter().map(|p| (p.title.text(), p.level)).collect::<Vec<_>>(), vec![("Trip".into(), 0), ("Hotel".into(), 1)]);
        // The first imported page is opened, with its outline text.
        let page = s.page().unwrap();
        assert_eq!(page.title.text(), "Trip");
        let text: Vec<_> = page.objects.iter().flat_map(|o| o.paragraphs()).map(|p| (p.level, p.text())).collect();
        assert_eq!(text, vec![(0, "Tickets".to_string()), (1, "Train".to_string())]);
        // One undo step removes the section again.
        run(&mut s, "edit.undo", json!({}));
        assert_eq!(s.notebook().unwrap().sections().len(), before);
    }

    #[test]
    fn a_second_import_gets_a_new_name() {
        let mut s = sample();
        let data = crate::b64::encode(&fixture());
        run(&mut s, "file.openSection", json!({"data": data, "name": "Travel"}));
        run(&mut s, "file.openSection", json!({"data": data, "name": "travel"}));
        let names: Vec<_> = s.notebook().unwrap().sections().iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&"travel 2".to_string()), "{names:?}");
    }

    #[test]
    fn foreign_or_damaged_files_fail_cleanly() {
        let mut s = sample();
        let before = s.notebook().unwrap().sections().len();
        for bytes in [b"%PDF-1.7 not a section".to_vec(), fixture()[..1500].to_vec()] {
            let e = s.run("file.openSection", json!({"data": crate::b64::encode(&bytes)})).unwrap_err();
            assert!(matches!(e, crate::CmdError::Failed(_)), "{e}");
        }
        assert!(s.run("file.openSection", json!({})).is_err());
        assert_eq!(s.notebook().unwrap().sections().len(), before);
    }

    /// A OneNote notebook folder on disk: two sections, a section group, the recycle bin, a
    /// damaged file and the `.onetoc2`.
    #[cfg(not(target_arch = "wasm32"))]
    fn onenote_folder(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("notecraft-onenote-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let one = fixture();
        for dir in ["Work", "OneNote_RecycleBin", "Empty"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("Open Notebook.onetoc2"), b"not read: its PropertyIDs are not public").unwrap();
        std::fs::write(root.join("travel.one"), &one).unwrap();
        std::fs::write(root.join("Ideas.one"), &one).unwrap();
        std::fs::write(root.join("Broken.one"), &one[..1500]).unwrap();
        std::fs::write(root.join("Work").join("Projects.one"), &one).unwrap();
        std::fs::write(root.join("OneNote_RecycleBin").join("Deleted.one"), &one).unwrap();
        root
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn listing(root: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(root).unwrap().flatten() {
            if e.file_type().unwrap().is_dir() {
                out.extend(listing(&e.path()));
            }
            out.push(e.path());
        }
        out.sort();
        out
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn a_onenote_folder_opens_as_an_in_memory_copy() {
        let root = onenote_folder("open");
        let before = listing(&root);
        let mut s = sample();
        let r = run(&mut s, "file.openNotebook", json!({"path": root.to_str().unwrap()}));
        assert_eq!(r["sections"], 3, "FMT.Onetoc2.Read");
        assert_eq!(r["imported"]["pages"], 6);
        assert_eq!(r["damaged"], 1, "Broken.one is reported, not fatal");
        assert_eq!(r["notImported"]["section order and colours (.onetoc2 is not in the public spec)"], 1);
        let nb = s.notebook().unwrap();
        assert_eq!(nb.name, format!("notecraft-onenote-open-{}", std::process::id()));
        // Sections by file name, groups for subfolders; the recycle bin and empty folders are skipped.
        let top: Vec<String> = nb
            .children
            .iter()
            .map(|n| match n {
                notecraft_doc::NavNode::Section(sec) => sec.name.clone(),
                notecraft_doc::NavNode::Group(g) => format!("{}/{}", g.name, g.children.len()),
            })
            .collect();
        assert_eq!(top, vec!["Ideas", "travel", "Work/1"]);
        assert_eq!(nb.sections().len(), 3);
        // The folder is only read: nothing is written into it, now or on save.
        assert!(!s.stores.contains_key(&nb.id));
        assert!(s.save_dirty().is_empty());
        assert_eq!(listing(&root), before);
        // The .onetoc2 itself opens the same notebook.
        let toc = root.join("Open Notebook.onetoc2");
        let r = run(&mut s, "file.openNotebook", json!({"path": toc.to_str().unwrap()}));
        assert_eq!(r["sections"], 3);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn a_onenote_folder_without_readable_sections_fails_cleanly() {
        let root = onenote_folder("none");
        for f in ["travel.one", "Ideas.one", "Work/Projects.one"] {
            std::fs::remove_file(root.join(f)).unwrap();
        }
        let mut s = sample();
        let notebooks = s.ws.notebooks.len();
        let e = s.run("file.openNotebook", json!({"path": root.to_str().unwrap()})).unwrap_err();
        assert!(matches!(e, crate::CmdError::Failed(_)), "{e}");
        assert_eq!(s.ws.notebooks.len(), notebooks);
        let _ = std::fs::remove_dir_all(&root);
    }
}
