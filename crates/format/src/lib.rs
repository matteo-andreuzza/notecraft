//! NoteCraft's native format (layer **L3**: storage for the L1 model).
//!
//! A notebook is a **folder**, as in the reference product (where a notebook is a folder of
//! `.one` section files plus a `.onetoc2` table of contents):
//!
//! ```text
//! Work/                          notebook folder
//!   notebook.ncraft-toc          JSON: notebook id, name, colour, child order
//!   Meetings.ncraft              one section = one zip (section.json [+ media/ from M2])
//!   Projects/                    a section group = a subfolder…
//!     notebook.ncraft-toc        …with its own table of contents
//!     Alpha.ncraft
//!   .ncraft-recycle/
//!     recycle.ncraft-bin         the Notebook Recycle Bin (zip of recycle.json)
//! ```
//!
//! **Crash safety.** Every file is replaced atomically (`craft-atomic`: write a temp file beside
//! the target, fsync, rename, fsync the directory), so a crash or `kill -9` at any moment leaves
//! each file either fully old or fully new — never truncated (catalog `BEH.Save.CrashRecovery`).
//! On open, leftover staging files are swept and sections present on disk but missing from the
//! table of contents are recovered, so a crash between writing a section and writing the TOC
//! loses nothing.
//!
//! **Hostile input.** Files are untrusted: zip entries are size-capped before decompression
//! (zip bombs), JSON nesting is capped by serde_json's recursion limit, the number of entries and
//! sections is capped, file names coming from a TOC are confined to the notebook folder (no
//! `..`, no absolute paths, no separators), and everything loaded is normalised.
//!
//! The pure encode/decode functions build for wasm (the web build keeps notebooks in memory and
//! moves archives through downloads); everything touching the file system is native-only.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::io::{Cursor, Read, Write};
use std::sync::Arc;

use notecraft_doc::{Id, NavNode, Notebook, RecycleBin, Section, SectionColor, SectionGroup};
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
pub mod store;

/// Table-of-contents file name, at the notebook root and in every section-group folder.
pub const TOC_FILE: &str = "notebook.ncraft-toc";
/// Section file extension.
pub const SECTION_EXT: &str = "ncraft";
/// Recycle bin folder (hidden).
pub const RECYCLE_DIR: &str = ".ncraft-recycle";
/// Recycle bin file inside [`RECYCLE_DIR`].
pub const RECYCLE_FILE: &str = "recycle.ncraft-bin";
/// Current schema version written by this build. Readers accept any version up to this one;
/// newer files are refused with an error rather than half-read.
pub const VERSION: u32 = 1;

/// Largest decompressed JSON entry accepted (1 GiB): a 2 GB notebook spreads over many
/// sections, so one section beyond this is corrupt or hostile.
pub const MAX_ENTRY_BYTES: u64 = 1 << 30;
/// Most entries accepted in one section zip.
pub const MAX_ZIP_ENTRIES: usize = 100_000;
/// Most children accepted in one table of contents.
pub const MAX_TOC_ENTRIES: usize = 10_000;

/// Errors.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    /// File-system error.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: String,
        /// The error.
        source: std::io::Error,
    },
    /// The bytes are not a valid NoteCraft file.
    #[error("{0}")]
    Corrupt(String),
    /// Written by a newer NoteCraft.
    #[error("written by a newer NoteCraft (format {found}, this build reads up to {VERSION})")]
    TooNew {
        /// The version in the file.
        found: u32,
    },
}

/// Result alias.
pub type Result<T> = std::result::Result<T, FormatError>;

fn corrupt(msg: impl Into<String>) -> FormatError {
    FormatError::Corrupt(msg.into())
}

/// One child in a table of contents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TocEntry {
    /// A section file in this folder.
    Section {
        /// The section's id (must match the id inside the file).
        id: Id,
        /// File name (no directory part).
        file: String,
    },
    /// A section-group subfolder.
    Group {
        /// The group's id.
        id: Id,
        /// Display name.
        name: String,
        /// Folder name (no directory part).
        dir: String,
    },
}

/// A table-of-contents file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Toc {
    /// Magic: `"notecraft-notebook"`.
    pub format: String,
    /// Schema version.
    pub version: u32,
    /// The notebook's id (at the root) or the group's id (in a group folder).
    pub id: Id,
    /// Display name.
    pub name: String,
    /// Notebook colour (root only).
    #[serde(default)]
    pub color: SectionColor,
    /// Children in tab order.
    pub entries: Vec<TocEntry>,
}

const TOC_MAGIC: &str = "notecraft-notebook";
const SECTION_MAGIC: &str = "notecraft-section";
const RECYCLE_MAGIC: &str = "notecraft-recycle";
const SECTION_ENTRY: &str = "section.json";
const RECYCLE_ENTRY: &str = "recycle.json";

#[derive(Serialize, Deserialize)]
struct SectionFile {
    format: String,
    version: u32,
    section: Section,
}

#[derive(Serialize, Deserialize)]
struct RecycleFile {
    format: String,
    version: u32,
    recycle: RecycleBin,
}

fn check_version(format: &str, magic: &str, version: u32) -> Result<()> {
    if format != magic {
        return Err(corrupt(format!("not a {magic} file (format `{}`)", format.chars().take(40).collect::<String>())));
    }
    if version > VERSION {
        return Err(FormatError::TooNew { found: version });
    }
    Ok(())
}

/// Serialize a table of contents.
pub fn encode_toc(toc: &Toc) -> Result<Vec<u8>> {
    serde_json::to_vec_pretty(toc).map_err(|e| corrupt(format!("toc: {e}")))
}

/// Parse a table of contents (validated: magic, version, entry cap, safe file names).
pub fn decode_toc(bytes: &[u8]) -> Result<Toc> {
    let toc: Toc = serde_json::from_slice(bytes).map_err(|e| corrupt(format!("table of contents: {e}")))?;
    check_version(&toc.format, TOC_MAGIC, toc.version)?;
    if toc.entries.len() > MAX_TOC_ENTRIES {
        return Err(corrupt(format!("table of contents lists {} children (max {MAX_TOC_ENTRIES})", toc.entries.len())));
    }
    for e in &toc.entries {
        let name = match e {
            TocEntry::Section { file, .. } => file,
            TocEntry::Group { dir, .. } => dir,
        };
        if !is_safe_component(name) {
            return Err(corrupt(format!("unsafe file name in table of contents: {name:?}")));
        }
    }
    Ok(toc)
}

/// Whether `name` is a single, harmless path component: not empty, not `.`/`..`, no separators
/// or NULs. A hostile TOC naming `../../.bashrc` is rejected here.
pub fn is_safe_component(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0']) && !name.contains(':') && name.len() <= 255
}

fn zip_one(entry: &str, json: &[u8]) -> Result<Vec<u8>> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(json.len() as u64 > u32::MAX as u64);
    w.start_file(entry, opts).map_err(|e| corrupt(format!("zip: {e}")))?;
    w.write_all(json).map_err(|e| corrupt(format!("zip: {e}")))?;
    let cur = w.finish().map_err(|e| corrupt(format!("zip: {e}")))?;
    Ok(cur.into_inner())
}

fn unzip_one(bytes: &[u8], entry: &str) -> Result<Vec<u8>> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| corrupt(format!("not a zip container: {e}")))?;
    if z.len() > MAX_ZIP_ENTRIES {
        return Err(corrupt(format!("{} zip entries (max {MAX_ZIP_ENTRIES})", z.len())));
    }
    let f = z.by_name(entry).map_err(|e| corrupt(format!("missing {entry}: {e}")))?;
    // The declared size is a hint (hostile files lie); the read is capped regardless.
    if f.size() > MAX_ENTRY_BYTES {
        return Err(corrupt(format!("{entry} declares {} bytes (max {MAX_ENTRY_BYTES})", f.size())));
    }
    let mut out = Vec::with_capacity(f.size().min(64 << 20) as usize);
    f.take(MAX_ENTRY_BYTES + 1).read_to_end(&mut out).map_err(|e| corrupt(format!("{entry}: {e}")))?;
    if out.len() as u64 > MAX_ENTRY_BYTES {
        return Err(corrupt(format!("{entry} is larger than {MAX_ENTRY_BYTES} bytes")));
    }
    Ok(out)
}

/// Serialize a section to `.ncraft` bytes.
pub fn encode_section(section: &Section) -> Result<Vec<u8>> {
    let json = serde_json::to_vec(&SectionFile { format: SECTION_MAGIC.into(), version: VERSION, section: section.clone() })
        .map_err(|e| corrupt(format!("section: {e}")))?;
    zip_one(SECTION_ENTRY, &json)
}

/// Parse `.ncraft` bytes (validated and normalised).
pub fn decode_section(bytes: &[u8]) -> Result<Section> {
    let json = unzip_one(bytes, SECTION_ENTRY)?;
    let f: SectionFile = serde_json::from_slice(&json).map_err(|e| corrupt(format!("section.json: {e}")))?;
    check_version(&f.format, SECTION_MAGIC, f.version)?;
    let mut s = f.section;
    s.normalize();
    for p in &mut s.pages {
        Arc::make_mut(p).sanitize_positions();
    }
    Ok(s)
}

/// Serialize a recycle bin.
pub fn encode_recycle(bin: &RecycleBin) -> Result<Vec<u8>> {
    let json = serde_json::to_vec(&RecycleFile { format: RECYCLE_MAGIC.into(), version: VERSION, recycle: bin.clone() })
        .map_err(|e| corrupt(format!("recycle: {e}")))?;
    zip_one(RECYCLE_ENTRY, &json)
}

/// Parse a recycle bin.
pub fn decode_recycle(bytes: &[u8]) -> Result<RecycleBin> {
    let json = unzip_one(bytes, RECYCLE_ENTRY)?;
    let f: RecycleFile = serde_json::from_slice(&json).map_err(|e| corrupt(format!("recycle.json: {e}")))?;
    check_version(&f.format, RECYCLE_MAGIC, f.version)?;
    Ok(f.recycle)
}

#[derive(Serialize, Deserialize)]
struct ArchiveFile {
    format: String,
    version: u32,
    notebook: Notebook,
}

const ARCHIVE_MAGIC: &str = "notecraft-archive";

/// A whole notebook in one file (`.ncraft-archive`: a zip with `notebook.json`), for the web
/// build's download/upload and File › Export › Notebook (catalog `FMT.Export.Archive`).
pub fn encode_archive(nb: &Notebook) -> Result<Vec<u8>> {
    let json = serde_json::to_vec(&ArchiveFile { format: ARCHIVE_MAGIC.into(), version: VERSION, notebook: nb.clone() })
        .map_err(|e| corrupt(format!("archive: {e}")))?;
    zip_one("notebook.json", &json)
}

/// Parse a notebook archive (validated and normalised).
pub fn decode_archive(bytes: &[u8]) -> Result<Notebook> {
    let json = unzip_one(bytes, "notebook.json")?;
    let f: ArchiveFile = serde_json::from_slice(&json).map_err(|e| corrupt(format!("notebook.json: {e}")))?;
    check_version(&f.format, ARCHIVE_MAGIC, f.version)?;
    let mut nb = f.notebook;
    nb.normalize();
    Ok(nb)
}

/// A file-system-safe name for a section file or group folder: characters that are reserved
/// on Windows, macOS or Linux become `_`, leading/trailing dots and spaces are trimmed, Windows
/// device names get a suffix, and the result is capped at 120 bytes. Never empty.
pub fn file_stem(name: &str) -> String {
    let mut s: String =
        name.chars().map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect();
    s = s.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string();
    while s.len() > 120 {
        s.pop();
    }
    if s.is_empty() {
        s = "Section".into();
    }
    let upper = s.to_ascii_uppercase();
    let base = upper.split('.').next().unwrap_or("");
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&base)
        || ((base.starts_with("COM") || base.starts_with("LPT")) && base.len() == 4 && base.as_bytes().get(3).is_some_and(u8::is_ascii_digit));
    if reserved {
        s.push('_');
    }
    s
}

/// A child-name allocator for one folder: hands out unique, case-insensitively distinct file
/// names (`Notes.ncraft`, `Notes (2).ncraft`, …).
#[derive(Default)]
pub struct NameAlloc {
    used: std::collections::HashSet<String>,
}

impl NameAlloc {
    /// Reserve and return a unique name built from `stem` and `ext` (`ext` empty for folders).
    pub fn take(&mut self, stem: &str, ext: &str) -> String {
        let mk = |n: usize| {
            let s = if n == 1 { stem.to_string() } else { format!("{stem} ({n})") };
            if ext.is_empty() { s } else { format!("{s}.{ext}") }
        };
        let mut n = 1;
        loop {
            let cand = mk(n);
            if self.used.insert(cand.to_lowercase()) {
                return cand;
            }
            n += 1;
        }
    }

    /// Mark a name used (case-insensitively).
    pub fn reserve(&mut self, name: &str) {
        self.used.insert(name.to_lowercase());
    }
}

/// Build a notebook skeleton from its root TOC (sections empty, to be filled by the loader).
pub fn toc_for(id: Id, name: &str, color: SectionColor, entries: Vec<TocEntry>) -> Toc {
    Toc { format: TOC_MAGIC.into(), version: VERSION, id, name: name.to_string(), color, entries }
}

/// Placeholder group for a TOC group entry.
pub fn group_from_entry(id: Id, name: &str) -> SectionGroup {
    SectionGroup { id, name: notecraft_doc::clean_name(name), children: Vec::new() }
}

/// Wrap a section as a navigation node.
pub fn section_node(s: Section) -> NavNode {
    NavNode::Section(Arc::new(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notecraft_doc::{Outline, Page, PageObject, Paragraph, Timestamp, geom::Point};

    fn sample_section() -> Section {
        let mut s = Section::new(Id(10), "Meetings", SectionColor::index(2), Timestamp(5));
        let mut p = Page::new(Id(20), Id(21), Timestamp(5), "me");
        p.title = Paragraph::with_text(Id(21), "Kickoff");
        let mut o = Outline::new(Id(30), Point::new(36.0, 90.0), Id(31));
        o.paragraphs[0] = Paragraph::with_text(Id(31), "Agenda ✓ ünïcode");
        p.objects.push(PageObject::Outline(o));
        s.pages.push(Arc::new(p));
        s
    }

    #[test]
    fn section_round_trip() {
        // parity: FMT.Native.Ncraft
        let s = sample_section();
        let bytes = encode_section(&s).unwrap();
        let back = decode_section(&bytes).unwrap();
        assert_eq!(back.pages.len(), 1);
        assert_eq!(back.pages[0].display_title(), "Kickoff");
        assert_eq!(back.pages[0].plain_text(), s.pages[0].plain_text());
        assert_eq!(back.id, s.id);
    }

    #[test]
    fn corrupt_and_newer_files_are_errors_not_panics() {
        assert!(decode_section(b"").is_err());
        assert!(decode_section(b"PK\x03\x04garbage").is_err());
        let json = br#"{"format":"notecraft-section","version":999,"section":{"id":"0000000000000000000000000000000a","name":"x"}}"#;
        let z = zip_one(SECTION_ENTRY, json).unwrap();
        assert!(matches!(decode_section(&z), Err(FormatError::TooNew { found: 999 })));
        let wrong = zip_one(SECTION_ENTRY, br#"{"format":"evil","version":1,"section":{}}"#).unwrap();
        assert!(decode_section(&wrong).is_err());
        // Deeply nested JSON hits serde_json's recursion limit instead of the stack.
        let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
        assert!(decode_section(&zip_one(SECTION_ENTRY, deep.as_bytes()).unwrap()).is_err());
    }

    #[test]
    fn hostile_toc_rejected() {
        let toc = toc_for(Id(1), "nb", SectionColor::default(), vec![TocEntry::Section { id: Id(2), file: "../../etc/passwd".into() }]);
        let bytes = encode_toc(&toc).unwrap();
        assert!(decode_toc(&bytes).unwrap_err().to_string().contains("unsafe"));
        for bad in ["", ".", "..", "a/b", "a\\b", "c:x", "a\0b"] {
            assert!(!is_safe_component(bad), "{bad:?}");
        }
        assert!(is_safe_component("Notes (2).ncraft"));
    }

    #[test]
    fn file_stems_are_portable() {
        assert_eq!(file_stem("Q1: plans/ideas?"), "Q1_ plans_ideas_");
        assert_eq!(file_stem("  ..hidden.. "), "hidden");
        assert_eq!(file_stem(""), "Section");
        assert_eq!(file_stem("CON"), "CON_");
        assert_eq!(file_stem("com1.txt"), "com1.txt_");
        assert!(file_stem(&"é".repeat(200)).len() <= 120);
        let mut a = NameAlloc::default();
        assert_eq!(a.take("Notes", SECTION_EXT), "Notes.ncraft");
        assert_eq!(a.take("notes", SECTION_EXT), "notes (2).ncraft");
    }

    #[test]
    fn archive_and_recycle_round_trip() {
        let mut nb = Notebook::new(Id(1), "Work", SectionColor::default());
        nb.children.push(section_node(sample_section()));
        let back = decode_archive(&encode_archive(&nb).unwrap()).unwrap();
        assert_eq!(back.sections().len(), 1);
        let bin = RecycleBin::default();
        assert_eq!(decode_recycle(&encode_recycle(&bin).unwrap()).unwrap(), bin);
    }
}
