//! NoteCraft's native format (layer **L3**: storage for the L1 model).
//!
//! A notebook is a **folder**, as in the reference product (where a notebook is a folder of
//! `.one` section files plus a `.onetoc2` table of contents):
//!
//! ```text
//! Work/                          notebook folder
//!   notebook.ncraft-toc          JSON: notebook id, name, colour, child order
//!   Meetings.ncraft              one section = one zip: section.json + media/<blob id> entries
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

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::sync::Arc;

use notecraft_doc::media::{MAX_ATTACHMENT_BYTES, collect_blobs, provide_blobs};
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
///
/// History: 1 = M1 (outlines); 2 = M2 tables (`PageObject::Table`); 3 = M2 pictures and
/// attachments (`PageObject::Image`/`Attachment`, bytes in `media/` entries; decision D3); 4 = M2
/// ink (`PageObject::Ink`, one stroke per object; decision D4).
/// Each version reads every older one unchanged; older builds refuse newer files rather than
/// drop objects they don't know.
pub const VERSION: u32 = 4;

/// Largest decompressed JSON entry accepted (1 GiB): a 2 GB notebook spreads over many
/// sections, so one section beyond this is corrupt or hostile.
pub const MAX_ENTRY_BYTES: u64 = 1 << 30;
/// Most entries accepted in one section zip.
pub const MAX_ZIP_ENTRIES: usize = 100_000;
/// Most media bytes (pictures and attached files) read from one file (D3); entries past it
/// load as missing (placeholders), not as an error.
pub const MAX_MEDIA_BYTES: u64 = 1 << 30;
/// Folder of the media entries inside a section, recycle-bin or archive zip.
pub const MEDIA_DIR: &str = "media/";
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

#[cfg(test)]
fn zip_one(entry: &str, json: &[u8]) -> Result<Vec<u8>> {
    zip_with_media(entry, json, &[])
}

/// A zip holding `entry` (deflated JSON) and one `media/<id>` entry per blob. Media is stored
/// uncompressed: pictures are compressed already, and autosave rewrites the whole file.
fn zip_with_media(entry: &str, json: &[u8], media: &[(Id, Arc<[u8]>)]) -> Result<Vec<u8>> {
    let z = |e: zip::result::ZipError| corrupt(format!("zip: {e}"));
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(json.len() as u64 > u32::MAX as u64);
    w.start_file(entry, opts).map_err(z)?;
    w.write_all(json).map_err(|e| corrupt(format!("zip: {e}")))?;
    for (id, bytes) in media {
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(bytes.len() as u64 > u32::MAX as u64);
        w.start_file(format!("{MEDIA_DIR}{id}"), opts).map_err(z)?;
        w.write_all(bytes).map_err(|e| corrupt(format!("zip: {e}")))?;
    }
    let cur = w.finish().map_err(z)?;
    Ok(cur.into_inner())
}

/// Read `entry` (required) and every readable `media/<id>` entry. A media entry that is
/// oversize, unnamed, unreadable or past [`MAX_MEDIA_BYTES`] in total is skipped: the object
/// that wanted it loads with missing bytes and draws as a placeholder (one damaged picture
/// never costs the notes).
fn unzip_with_media(bytes: &[u8], entry: &str) -> Result<(Vec<u8>, HashMap<Id, Arc<[u8]>>)> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| corrupt(format!("not a zip container: {e}")))?;
    if z.len() > MAX_ZIP_ENTRIES {
        return Err(corrupt(format!("{} zip entries (max {MAX_ZIP_ENTRIES})", z.len())));
    }
    let json = {
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
        out
    };
    let mut media = HashMap::new();
    let mut total: u64 = 0;
    let cap = MAX_ATTACHMENT_BYTES as u64;
    for i in 0..z.len() {
        let Ok(f) = z.by_index(i) else { continue };
        let Some(id) = f.name().ok().and_then(|n| n.strip_prefix(MEDIA_DIR).and_then(Id::parse)) else { continue };
        if f.size() > cap || total.saturating_add(f.size()) > MAX_MEDIA_BYTES {
            continue;
        }
        let mut out = Vec::with_capacity(f.size().min(64 << 20) as usize);
        if f.take(cap + 1).read_to_end(&mut out).is_err() || out.len() as u64 > cap {
            continue;
        }
        total = total.saturating_add(out.len() as u64);
        if total > MAX_MEDIA_BYTES {
            break;
        }
        media.insert(id, Arc::from(out));
    }
    Ok((json, media))
}

/// Serialize `value` to JSON, collecting the blobs it holds, and zip both.
fn encode_with_media<T: Serialize>(entry: &str, value: &T, what: &str) -> Result<Vec<u8>> {
    let (json, media) = collect_blobs(|| serde_json::to_vec(value));
    let json = json.map_err(|e| corrupt(format!("{what}: {e}")))?;
    zip_with_media(entry, &json, &media)
}

/// Unzip `entry` and parse it, giving blobs their bytes from the media entries.
fn decode_with_media<T: serde::de::DeserializeOwned>(bytes: &[u8], entry: &str) -> Result<T> {
    let (json, media) = unzip_with_media(bytes, entry)?;
    provide_blobs(media, || serde_json::from_slice(&json)).map_err(|e| corrupt(format!("{entry}: {e}")))
}

/// Serialize a section to `.ncraft` bytes.
pub fn encode_section(section: &Section) -> Result<Vec<u8>> {
    encode_with_media(SECTION_ENTRY, &SectionFile { format: SECTION_MAGIC.into(), version: VERSION, section: section.clone() }, "section")
}

/// Parse `.ncraft` bytes (validated and normalised).
pub fn decode_section(bytes: &[u8]) -> Result<Section> {
    let f: SectionFile = decode_with_media(bytes, SECTION_ENTRY)?;
    check_version(&f.format, SECTION_MAGIC, f.version)?;
    let mut s = f.section;
    s.normalize_pages();
    Ok(s)
}

/// Serialize a recycle bin.
pub fn encode_recycle(bin: &RecycleBin) -> Result<Vec<u8>> {
    encode_with_media(RECYCLE_ENTRY, &RecycleFile { format: RECYCLE_MAGIC.into(), version: VERSION, recycle: bin.clone() }, "recycle")
}

/// Parse a recycle bin.
pub fn decode_recycle(bytes: &[u8]) -> Result<RecycleBin> {
    let f: RecycleFile = decode_with_media(bytes, RECYCLE_ENTRY)?;
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
    encode_with_media("notebook.json", &ArchiveFile { format: ARCHIVE_MAGIC.into(), version: VERSION, notebook: nb.clone() }, "archive")
}

/// Parse a notebook archive (validated and normalised).
pub fn decode_archive(bytes: &[u8]) -> Result<Notebook> {
    let f: ArchiveFile = decode_with_media(bytes, "notebook.json")?;
    check_version(&f.format, ARCHIVE_MAGIC, f.version)?;
    let mut nb = f.notebook;
    nb.normalize_pages();
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
    use std::io::Write;

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

    /// A section written by format 1 (M1), byte for byte as that build wrote it.
    const V1_SECTION: &str = r#"{"format":"notecraft-section","version":1,"section":{"id":"0000000000000000000000000000000a","name":"Old","color":2,"pages":[{"id":"00000000000000000000000000000014","title":{"id":"00000000000000000000000000000015","runs":[{"text":"From v1"}],"style":"Normal","level":0,"align":"Left","space_before":0.0,"space_after":0.0,"line_spacing":0.0,"rtl":false,"collapsed":false,"modified":0},"level":0,"created":5,"modified":5,"shown_time":null,"author":"me","objects":[{"kind":"Outline","id":"0000000000000000000000000000001e","pos":{"x":36.0,"y":90.0},"width":"Auto","paragraphs":[{"id":"0000000000000000000000000000001f","runs":[{"text":"Written by format 1"}],"style":"Normal","level":0,"align":"Left","space_before":0.0,"space_after":0.0,"line_spacing":0.0,"rtl":false,"collapsed":false,"modified":0}]}],"setup":{"rule_lines":"None","hide_title":false},"collapsed":false}],"created":5}}"#;

    #[test]
    fn format_1_files_still_open() {
        let z = zip_one(SECTION_ENTRY, V1_SECTION.as_bytes()).unwrap();
        let s = decode_section(&z).unwrap();
        assert_eq!(s.name, "Old");
        assert_eq!(s.pages[0].display_title(), "From v1");
        assert_eq!(s.pages[0].objects[0].as_outline().unwrap().paragraphs[0].text(), "Written by format 1");
        // Saved again, it is a current-format file that still reads back the same.
        let again = decode_section(&encode_section(&s).unwrap()).unwrap();
        assert_eq!(again.pages[0].plain_text(), s.pages[0].plain_text());
    }

    #[test]
    fn tables_round_trip_and_hostile_tables_are_repaired() {
        use notecraft_doc::{Table, color::Rgba};
        let mut s = sample_section();
        let mut n = 100u128;
        let mut mint = || {
            n += 1;
            Id(n)
        };
        let mut t = Table::new(Id(50), Point::new(40.0, 200.0), 2, 3, &mut mint);
        t.rows[1].cells[2].paragraphs[0] = Paragraph::with_text(Id(60), "cell");
        t.rows[0].cells[0].shading = Some(Rgba::rgb(0xFF, 0xF2, 0xCC));
        t.columns[1] = 150.0;
        t.borders = false;
        Arc::make_mut(&mut s.pages[0]).objects.push(PageObject::Table(t.clone()));
        let back = decode_section(&encode_section(&s).unwrap()).unwrap();
        assert_eq!(back.pages[0].table(Id(50)), Some(&t), "FMT.Native.Ncraft: tables survive a save");
        // A hostile table: ragged rows, no paragraphs, absurd widths, far too many columns.
        let json =
            String::from_utf8(serde_json::to_vec(&SectionFile { format: SECTION_MAGIC.into(), version: VERSION, section: back }).unwrap()).unwrap();
        let wide = format!("[{}]", vec!["1e30"; 500].join(","));
        let evil = json.replacen(r#""columns":[96.0,150.0,96.0]"#, &format!(r#""columns":{wide}"#), 1).replacen(
            r#""paragraphs":[{"id":"000000000000000000000000000000"#,
            r#""paragraphs":[],"x":[{"id":"000000000000000000000000000000"#,
            1,
        );
        let s2 = decode_section(&zip_one(SECTION_ENTRY, evil.as_bytes()).unwrap()).unwrap();
        let t2 = s2.pages[0].table(Id(50)).unwrap();
        assert_eq!(t2.n_cols(), notecraft_doc::table::MAX_COLS);
        assert!(t2.rows.iter().all(|r| r.cells.len() == t2.n_cols() && r.cells.iter().all(|c| !c.paragraphs.is_empty())));
        assert!(t2.columns.iter().all(|w| *w <= notecraft_doc::table::MAX_COL_WIDTH));
    }

    fn picture(id: u128, blob: &notecraft_doc::Blob) -> PageObject {
        PageObject::Image(notecraft_doc::Image::new(Id(id), Point::new(40.0, 300.0), blob.clone(), (8, 4), 60.0, 30.0))
    }

    #[test]
    fn pictures_and_attachments_round_trip_with_shared_bytes() {
        use notecraft_doc::{Attachment, Blob};
        let mut s = sample_section();
        let shared = Blob::new(Id(70), Arc::from(&b"\x89PNG fake picture bytes"[..]));
        let file = Blob::new(Id(71), Arc::from(&b"%PDF-1.7 attached"[..]));
        {
            let p = Arc::make_mut(&mut s.pages[0]);
            p.objects.push(picture(60, &shared));
            // The same picture pasted twice: one media entry.
            p.objects.push(picture(61, &shared));
            p.objects.push(PageObject::Attachment(Attachment {
                id: Id(62),
                pos: Point::new(40.0, 400.0),
                name: "report.pdf".into(),
                blob: file.clone(),
                alt_text: String::new(),
                // Object links round-trip with the rest (format 4).
                link: Some("https://example.org/report".into()),
            }));
            if let Some(PageObject::Image(im)) = p.objects.get_mut(1) {
                im.alt_text = "A chart".into();
                im.quarter_turns = 1;
                im.flip_h = true;
            }
        }
        let bytes = encode_section(&s).unwrap();
        let z = zip::ZipArchive::new(Cursor::new(&bytes[..])).unwrap();
        let names: Vec<String> = z.file_names().filter_map(|n| n.ok().map(|n| n.to_string())).collect();
        assert_eq!(names.iter().filter(|n| n.starts_with(MEDIA_DIR)).count(), 2, "{names:?}");
        let back = decode_section(&bytes).unwrap();
        assert_eq!(back.pages[0].objects, s.pages[0].objects, "FMT.Native.Ncraft: pictures and attachments survive a save");
        let a = back.pages[0].image(Id(60)).unwrap();
        let b = back.pages[0].image(Id(61)).unwrap();
        assert!(Arc::ptr_eq(&a.blob.bytes, &b.blob.bytes), "one entry loads once and stays shared");
        assert_eq!(back.pages[0].attachment(Id(62)).unwrap().blob.bytes[..], b"%PDF-1.7 attached"[..]);
        assert!(back.pages[0].plain_text().contains("report.pdf"));
        // Deleted pages keep their pictures in the recycle bin; archives carry media too.
        let bin = RecycleBin {
            pages: vec![notecraft_doc::DeletedPage {
                page: back.pages[0].clone(),
                from_section: s.id,
                from_section_name: s.name.clone(),
                deleted: Timestamp(9),
            }],
            sections: Vec::new(),
        };
        let bin2 = decode_recycle(&encode_recycle(&bin).unwrap()).unwrap();
        assert_eq!(bin2.pages[0].page.image(Id(60)).unwrap().blob, shared);
        let mut nb = Notebook::new(Id(1), "Work", SectionColor::default());
        nb.children.push(section_node(back));
        let nb2 = decode_archive(&encode_archive(&nb).unwrap()).unwrap();
        assert_eq!(nb2.sections()[0].pages[0].attachment(Id(62)).unwrap().blob, file);
    }

    #[test]
    fn damaged_media_loads_as_a_placeholder_not_an_error() {
        let mut s = sample_section();
        let blob = notecraft_doc::Blob::new(Id(80), Arc::from(&b"picture"[..]));
        Arc::make_mut(&mut s.pages[0]).objects.push(picture(81, &blob));
        let json =
            collect_blobs(|| serde_json::to_vec(&SectionFile { format: SECTION_MAGIC.into(), version: VERSION, section: s.clone() })).0.unwrap();
        // No media entry at all, a wrong-length entry, and a junk entry name.
        for media in [vec![], vec![(Id(80), Arc::from(&b"pic"[..]))]] {
            let mut z = zip_with_media(SECTION_ENTRY, &json, &media).unwrap();
            if media.is_empty() {
                z = {
                    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
                    let o = zip::write::SimpleFileOptions::default();
                    w.start_file(SECTION_ENTRY, o).unwrap();
                    w.write_all(&json).unwrap();
                    w.start_file("media/../../evil", o).unwrap();
                    w.write_all(b"x").unwrap();
                    w.finish().unwrap().into_inner()
                };
            }
            let back = decode_section(&z).unwrap();
            let im = back.pages[0].image(Id(81)).unwrap();
            assert!(im.blob.is_missing(), "the picture is a placeholder");
            assert_eq!(back.pages[0].display_title(), "Kickoff", "the notes are intact");
        }
    }

    #[test]
    fn format_2_files_still_open() {
        // A version-2 section with a table: the table schema is unchanged in version 3.
        let mut s = sample_section();
        let mut n = 300u128;
        let t = notecraft_doc::Table::new(Id(90), Point::new(40.0, 200.0), 2, 2, &mut || {
            n += 1;
            Id(n)
        });
        Arc::make_mut(&mut s.pages[0]).objects.push(PageObject::Table(t.clone()));
        let json =
            String::from_utf8(serde_json::to_vec(&SectionFile { format: SECTION_MAGIC.into(), version: VERSION, section: s }).unwrap()).unwrap();
        let v2 = json.replacen(&format!(r#""version":{VERSION}"#), r#""version":2"#, 1);
        assert!(v2.contains(r#""version":2"#));
        let back = decode_section(&zip_one(SECTION_ENTRY, v2.as_bytes()).unwrap()).unwrap();
        assert_eq!(back.pages[0].table(Id(90)), Some(&t));
    }

    #[test]
    fn format_3_files_still_open_and_ink_round_trips() {
        use notecraft_doc::{Ink, InkTool, color::Rgba};
        // A version-3 section (pictures) reads unchanged: the picture schema is the same in 4.
        let shared = notecraft_doc::Blob::new(Id(70), Arc::from(&b"\x89PNG fake"[..]));
        let mut s = sample_section();
        Arc::make_mut(&mut s.pages[0]).objects.push(picture(60, &shared));
        let (json, media) = notecraft_doc::media::collect_blobs(|| {
            serde_json::to_vec(&SectionFile { format: SECTION_MAGIC.into(), version: 3, section: s.clone() }).unwrap()
        });
        let v3 = zip_with_media(SECTION_ENTRY, &json, &media).unwrap();
        let back = decode_section(&v3).unwrap();
        assert_eq!(back.pages[0].image(Id(60)).map(|im| im.blob.bytes.len()), Some(shared.bytes.len()));
        // Ink is written as format 4 and comes back point for point.
        let pts = [(Point::new(50.0, 80.0), 0.5), (Point::new(60.0, 85.0), 0.75), (Point::new(72.5, 82.25), 1.0)];
        let ink = Ink::new(Id(80), &pts, Rgba::rgb(200, 0, 0), 1.5, InkTool::Pen);
        let hl = Ink::new(Id(81), &pts, Rgba::rgb(255, 255, 0), 14.0, InkTool::Highlighter);
        Arc::make_mut(&mut s.pages[0]).objects.extend([PageObject::Ink(ink.clone()), PageObject::Ink(hl.clone())]);
        let bytes = encode_section(&s).unwrap();
        let back = decode_section(&bytes).unwrap();
        assert_eq!(back.pages[0].ink(Id(80)), Some(&ink), "FMT.Native.Ncraft: ink survives a save");
        assert_eq!(back.pages[0].ink(Id(81)).map(|k| k.tool), Some(InkTool::Highlighter));
        // Hostile ink: no points is dropped, NaN and huge numbers are clamped.
        let json =
            String::from_utf8(serde_json::to_vec(&SectionFile { format: SECTION_MAGIC.into(), version: VERSION, section: s }).unwrap()).unwrap();
        let evil = json.replacen("[0.0,0.0,0.5]", "[1e30,-1e30,9.0]", 1);
        let back = decode_section(&zip_one(SECTION_ENTRY, evil.as_bytes()).unwrap()).unwrap();
        let k = back.pages[0].ink(Id(80)).unwrap();
        assert!(k.points.iter().all(|p| p.x.is_finite() && p.y.is_finite() && (0.0..=1.0).contains(&p.pressure)));
        let empty = json.replacen(r#""points":[[0.0,0.0,0.5],[10.0,5.0,0.75],[22.5,2.25,1.0]]"#, r#""points":[]"#, 1);
        assert_ne!(empty, json, "the test edits the stored points");
        let back = decode_section(&zip_one(SECTION_ENTRY, empty.as_bytes()).unwrap()).unwrap();
        assert!(back.pages[0].ink(Id(80)).is_none(), "a stroke without points is dropped");
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
