//! OneNote's own files (decision D12): File › Open › Open Section imports a `.one` section file
//! into the current notebook as a new section, and File › Open › Browse on a OneNote notebook
//! folder (or its `.onetoc2`), or on a `.onepkg` package, imports the whole notebook as an
//! in-memory copy. The original
//! files are only read, never changed; what the reader does not import yet is listed in the
//! notice and the command's result.

use std::sync::Arc;

use notecraft_doc::{NavNode, SectionColor};
use serde_json::{Value, json};

use crate::ops::need_notebook;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// The damage messages a result lists (the count is always complete).
const MAX_WARNINGS_SHOWN: usize = 50;

/// The largest `.one` file read (the reader's own limit is higher; a section this large is
/// already far beyond what the reference writes).
const MAX_ONE_BYTES: usize = 1 << 30;

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // BS.Open.OpenSection — Ctrl+Alt+Shift+O.
        CommandSpec::new("file.openSection", "BS.Open.OpenSection", "Open Section", "File/Open/Open Section", open_section)
            .key("Mod+Alt+Shift+O")
            .params(r#"{"path": .one file | "data": base64 with "name", "name"?: section name, "notebook"?: id} → {section, page?, imported, notImported, damaged, warnings}"#)
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
        "warnings": report.warnings.iter().take(MAX_WARNINGS_SHOWN).collect::<Vec<_>>(),
    }))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use notebook_import::{extension, is_onenote_folder};

/// Whole OneNote notebooks: a notebook folder (its `.onetoc2` beside the `.one` files) or a
/// `.onepkg` package ([MS-CAB]), imported as an in-memory copy (decision D12). Desktop only:
/// both start from the file system.
#[cfg(not(target_arch = "wasm32"))]
mod notebook_import {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use notecraft_doc::{NavNode, SectionColor};
    use notecraft_onestore::Report;
    use serde_json::json;

    use super::{MAX_ONE_BYTES, MAX_WARNINGS_SHOWN};
    use crate::{CmdError, CmdResult, Session};

    /// Most section files one notebook import reads.
    const MAX_NOTEBOOK_SECTIONS: usize = 10_000;
    /// Most bytes one notebook import reads in all (each file is also capped by [`MAX_ONE_BYTES`]).
    const MAX_NOTEBOOK_BYTES: u64 = 4 << 30;
    /// The largest `.onepkg` read (its files unpack within [`MAX_NOTEBOOK_BYTES`]).
    const MAX_ONEPKG_BYTES: usize = 1 << 31;
    /// Deepest section group nesting followed (folders can be deep or linked in loops).
    const MAX_GROUP_DEPTH: usize = 16;

    /// The lower-case extension of `path` ("one", "onetoc2"…).
    pub(crate) fn extension(path: &Path) -> Option<String> {
        path.extension().map(|e| e.to_string_lossy().to_lowercase())
    }

    /// Whether `dir` is a OneNote notebook folder: no NoteCraft table of contents, and a
    /// `.onetoc2` or `.one` file directly inside it.
    pub(crate) fn is_onenote_folder(dir: &Path) -> bool {
        if dir.join(notecraft_format::TOC_FILE).exists() {
            return false;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return false };
        entries.flatten().any(|e| matches!(extension(&e.path()).as_deref(), Some("one" | "onetoc2")))
    }

    /// Where a notebook import reads its files; paths are components below the notebook root.
    trait Source {
        /// The entries of the folder `dir`: name, and whether it is a folder.
        fn list(&self, dir: &[String]) -> Result<Vec<(String, bool)>, String>;
        /// The size of the file at `path`, as far as it is known before reading.
        fn size(&self, path: &[String]) -> u64;
        /// The file's bytes (each file is read once).
        fn take(&mut self, path: &[String]) -> Result<Vec<u8>, String>;
        /// `path` for messages.
        fn show(&self, path: &[String]) -> String;
    }

    /// A notebook folder on disk.
    struct Disk(PathBuf);

    impl Disk {
        fn path(&self, path: &[String]) -> PathBuf {
            path.iter().fold(self.0.clone(), |p, c| p.join(c))
        }
    }

    impl Source for Disk {
        fn list(&self, dir: &[String]) -> Result<Vec<(String, bool)>, String> {
            let full = self.path(dir);
            let entries = std::fs::read_dir(&full).map_err(|e| format!("{}: {e}", full.display()))?;
            // Symbolic links are not followed: a link back up would loop.
            Ok(entries
                .flatten()
                .filter_map(|e| {
                    let kind = e.file_type().ok()?;
                    (!kind.is_symlink()).then(|| (e.file_name().to_string_lossy().to_string(), kind.is_dir()))
                })
                .collect())
        }
        fn size(&self, path: &[String]) -> u64 {
            std::fs::metadata(self.path(path)).map(|m| m.len()).unwrap_or(0)
        }
        fn take(&mut self, path: &[String]) -> Result<Vec<u8>, String> {
            let full = self.path(path);
            let file = full.to_str().ok_or_else(|| format!("{}: the file name is not valid Unicode", full.display()))?;
            crate::picture_cmds::read_path(file, MAX_ONE_BYTES).map(|(bytes, _)| bytes).map_err(|e| e.to_string())
        }
        fn show(&self, path: &[String]) -> String {
            self.path(path).display().to_string()
        }
    }

    /// The files of an unpacked `.onepkg`.
    struct Package {
        name: String,
        files: BTreeMap<Vec<String>, Vec<u8>>,
    }

    impl Source for Package {
        fn list(&self, dir: &[String]) -> Result<Vec<(String, bool)>, String> {
            let mut out: Vec<(String, bool)> = Vec::new();
            for path in self.files.keys().filter(|p| p.len() > dir.len() && p.starts_with(dir)) {
                let Some(name) = path.get(dir.len()) else { continue };
                let is_dir = path.len() > dir.len() + 1;
                if !out.iter().any(|(n, d)| n == name && *d == is_dir) {
                    out.push((name.clone(), is_dir));
                }
            }
            Ok(out)
        }
        fn size(&self, path: &[String]) -> u64 {
            self.files.get(path).map_or(0, |d| d.len() as u64)
        }
        fn take(&mut self, path: &[String]) -> Result<Vec<u8>, String> {
            self.files.remove(path).ok_or_else(|| format!("{}: missing from the package", self.show(path)))
        }
        fn show(&self, path: &[String]) -> String {
            format!("{}/{}", self.name, path.join("/"))
        }
    }

    /// The walk over a notebook's folders.
    struct Walk {
        report: Report,
        sections: usize,
        bytes: u64,
    }

    impl Walk {
        /// The sections (`.one` files) and section groups (subfolders holding some) of `dir`,
        /// by name: the order the `.onetoc2` keeps is not in the public [MS-ONE] (its
        /// PropertyIDs are not published), so the folder order stands in for it.
        fn dir(&mut self, s: &mut Session, src: &mut dyn Source, dir: &[String]) -> Vec<NavNode> {
            let mut entries = match src.list(dir) {
                Ok(e) => e,
                Err(e) => {
                    self.report.warnings.push(e);
                    return Vec::new();
                }
            };
            // Hidden entries and OneNote's own recycle bin folder (deleted pages) are skipped.
            entries.retain(|(name, _)| !name.starts_with('.') && !name.eq_ignore_ascii_case("OneNote_RecycleBin"));
            entries.sort_by_key(|(name, _)| name.to_lowercase());
            let mut out = Vec::new();
            for (name, is_dir) in entries {
                let path: Vec<String> = dir.iter().cloned().chain([name.clone()]).collect();
                if is_dir {
                    if path.len() >= MAX_GROUP_DEPTH {
                        self.report.warnings.push(format!("{}: section groups nested too deeply", src.show(&path)));
                        continue;
                    }
                    let children = self.dir(s, src, &path);
                    if !children.is_empty() {
                        let name = notecraft_doc::clean_name(&name);
                        out.push(NavNode::Group(notecraft_doc::SectionGroup { id: s.new_id(), name, children }));
                    }
                    continue;
                }
                match extension(Path::new(&name)).as_deref() {
                    Some("one") => {
                        if let Some(section) = self.section(s, src, &path) {
                            out.push(NavNode::Section(Arc::new(section)));
                        }
                    }
                    Some("onetoc2") => self.report.lose_one("section order and colours (.onetoc2 is not in the public spec)"),
                    _ => {}
                }
            }
            out
        }

        /// One `.one` file as a section named after the file ([MS-ONE] §2.2.94: the file name
        /// overrides the name the notebook stores).
        fn section(&mut self, s: &mut Session, src: &mut dyn Source, path: &[String]) -> Option<notecraft_doc::Section> {
            if self.sections >= MAX_NOTEBOOK_SECTIONS {
                self.report.lose_one("section files over the notebook limit");
                return None;
            }
            if self.bytes.saturating_add(src.size(path)) > MAX_NOTEBOOK_BYTES {
                self.report.lose_one("section files over the notebook size limit");
                return None;
            }
            let bytes = match src.take(path) {
                Ok(b) => b,
                Err(e) => {
                    self.report.warnings.push(e);
                    return None;
                }
            };
            self.bytes = self.bytes.saturating_add(bytes.len() as u64);
            let file = path.last().map(String::as_str).unwrap_or_default();
            let stem = Path::new(file).file_stem().map(|n| notecraft_doc::clean_name(&n.to_string_lossy())).filter(|n| !n.is_empty());
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
                    self.report.warnings.push(format!("{}: {e}", src.show(path)));
                    None
                }
            }
        }
    }

    impl Session {
        /// Imports the OneNote notebook folder `root` as a new in-memory notebook (decision
        /// D12: a copy; nothing is ever written into the folder).
        pub(crate) fn import_onenote_folder(&mut self, root: &Path) -> CmdResult {
            let name = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            self.import_notebook(&mut Disk(root.to_path_buf()), &name, Report::default())
        }

        /// Imports the `.onepkg` package at `path` ([MS-CAB]) as a new in-memory notebook named
        /// after the file. A package whose files all sit in one top folder is read from inside
        /// it (the layout the reference writes is not documented — unverified).
        pub(crate) fn import_onepkg(&mut self, path: &Path) -> CmdResult {
            let file = path.to_str().ok_or_else(|| CmdError::Failed(format!("{}: the file name is not valid Unicode", path.display())))?;
            let (bytes, _) = crate::picture_cmds::read_path(file, MAX_ONEPKG_BYTES)?;
            let limits = notecraft_onestore::CabLimits { max_files: MAX_NOTEBOOK_SECTIONS * 2, max_total_bytes: MAX_NOTEBOOK_BYTES };
            let cab = notecraft_onestore::read_cabinet(&bytes, &limits).map_err(|e| CmdError::Failed(format!("{}: {e}", path.display())))?;
            drop(bytes);
            let mut files: BTreeMap<Vec<String>, Vec<u8>> = cab.files.into_iter().map(|f| (f.path, f.data)).collect();
            let tops: std::collections::BTreeSet<&String> = files.keys().filter_map(|p| p.first()).collect();
            if tops.len() == 1 && files.keys().all(|p| p.len() > 1) {
                files = files.into_iter().map(|(p, d)| (p.into_iter().skip(1).collect(), d)).collect();
            }
            let name = path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let report = Report { warnings: cab.warnings, ..Report::default() };
            self.import_notebook(&mut Package { name: name.clone(), files }, &name, report)
        }

        /// A notebook called `name` from the sections in `src`.
        fn import_notebook(&mut self, src: &mut dyn Source, name: &str, report: Report) -> CmdResult {
            let mut walk = Walk { report, sections: 0, bytes: 0 };
            let children = walk.dir(self, src, &[]);
            if walk.sections == 0 {
                let why = walk.report.warnings.first().map(|w| format!(" ({w})")).unwrap_or_default();
                return Err(CmdError::Failed(format!("{name}: no OneNote section (.one) could be read{why}")));
            }
            let name = Some(notecraft_doc::clean_name(name)).filter(|n| !n.is_empty()).unwrap_or_else(|| "OneNote Notebook".into());
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
                "warnings": report.warnings.iter().take(MAX_WARNINGS_SHOWN).collect::<Vec<_>>(),
            }))
        }
    }
}

/// `base`, or `base 2`, `base 3`… when the notebook already has a section of that name.
pub(crate) fn unique_name(nb: &notecraft_doc::Notebook, base: &str) -> String {
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

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn a_onepkg_opens_as_an_in_memory_copy() {
        use notecraft_onestore::writer::cab::{Compression, write};
        let dir = std::env::temp_dir().join(format!("notecraft-onepkg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let one = fixture();
        let pkg = dir.join("Trip Notes.onepkg");
        // The files sit in one top folder, which the import steps into.
        let files: [(&str, &[u8]); 4] = [
            ("Trip\\Open Notebook.onetoc2", b"toc"),
            ("Trip\\Travel.one", &one),
            ("Trip\\Work\\Projects.one", &one),
            ("Trip\\Broken.one", &one[..1500]),
        ];
        std::fs::write(&pkg, write(&files, Compression::MsZip)).unwrap();
        let mut s = sample();
        let r = run(&mut s, "file.openNotebook", json!({"path": pkg.to_str().unwrap()}));
        assert_eq!(r["sections"], 2, "FMT.Onepkg.Read");
        assert_eq!(r["damaged"], 1);
        let nb = s.notebook().unwrap();
        assert_eq!(nb.name, "Trip Notes");
        let top: Vec<String> = nb
            .children
            .iter()
            .map(|n| match n {
                notecraft_doc::NavNode::Section(sec) => sec.name.clone(),
                notecraft_doc::NavNode::Group(g) => format!("{}/{}", g.name, g.children.len()),
            })
            .collect();
        assert_eq!(top, vec!["Travel", "Work/1"]);
        // LZX packages and other files fail cleanly.
        std::fs::write(&pkg, write(&files, Compression::Other(3))).unwrap();
        assert!(matches!(s.run("file.openNotebook", json!({"path": pkg.to_str().unwrap()})), Err(crate::CmdError::Failed(_))));
        std::fs::write(&pkg, b"not a cabinet").unwrap();
        assert!(matches!(s.run("file.openNotebook", json!({"path": pkg.to_str().unwrap()})), Err(crate::CmdError::Failed(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
