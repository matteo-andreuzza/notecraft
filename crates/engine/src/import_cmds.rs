//! Import a file (catalog `NC.File.Import`; formats `FMT.Import.Markdown`, `FMT.Import.Text`,
//! `FMT.Import.Html`, `FMT.Import.Enex`). A document becomes a new page of the current section:
//! its first-level heading (else the file name) is the title, its content is placed the way a
//! rich paste places it, task items become To Do tags. An Evernote export becomes a new
//! section of the current notebook with a page per note (title, dates, content, attached
//! files). One undo step.
//!
//! Pictures a Markdown file names by a relative path are read only from the file's own folder
//! and below (never through `..` or a link out of it), each within the picture size limit.

use std::path::{Path, PathBuf};

use notecraft_doc::{Run, TagUse};
use notecraft_import_export::html::Block;
use notecraft_import_export::import::{self, Imported, TODO_MARKS};
use serde_json::{Value, json};

use crate::ops::need_section;
use crate::selection::PageSel;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// Largest document imported (the readers look at the first 16 MiB).
const MAX_FILE_BYTES: u64 = 64 << 20;
/// Largest Evernote export imported (its attachments are inside it).
const MAX_ENEX_BYTES: u64 = 1 << 30;

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // NC.File.Import — Markdown, plain text or HTML into a new page.
        CommandSpec::new("file.import", "NC.File.Import", "Import File", "File/Open/Import File", import)
            .params(r#"{"path": file, "format"?: "md"|"txt"|"html"|"enex"}"#)
            .when(need_section),
    ]
}

/// A file format this module reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Markdown,
    Text,
    Html,
    Enex,
}

fn format(v: &Value, path: &str) -> Result<Format, CmdError> {
    let name = match p::str(v, "format") {
        Some(f) => f.to_ascii_lowercase(),
        None => Path::new(path).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_else(|| "txt".into()),
    };
    match name.as_str() {
        "md" | "markdown" | "mdown" | "mkd" => Ok(Format::Markdown),
        "txt" | "text" => Ok(Format::Text),
        "html" | "htm" | "xhtml" => Ok(Format::Html),
        "enex" => Ok(Format::Enex),
        other => Err(CmdError::Params(format!("unknown import format `{other}` (md, txt, html or enex)"))),
    }
}

/// `%20`-style escapes decoded (a Markdown link's path); invalid escapes are kept.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A picture named by `rel`, read only from below `base` (already canonical).
fn load_below(base: &Path, rel: &str) -> Option<Vec<u8>> {
    let rel = percent_decode(rel.split(['?', '#']).next().unwrap_or(""));
    let rel = rel.strip_prefix("file://").unwrap_or(&rel);
    if rel.is_empty() || Path::new(rel).is_absolute() {
        return None;
    }
    let path = base.join(rel).canonicalize().ok()?;
    if !path.starts_with(base) {
        return None;
    }
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > notecraft_doc::media::MAX_PICTURE_BYTES as u64 {
        return None;
    }
    std::fs::read(&path).ok()
}

/// "☐ " / "☑ " at the start of a paragraph become a To Do tag.
fn todo_tags(s: &Session, blocks: &mut [Block]) {
    let Some(def) = s.tag_defs().into_iter().find(|d| d.key == "todo") else { return };
    for b in blocks {
        let Block::Para(p) = b else { continue };
        let Some(first) = p.runs.first_mut() else { continue };
        let Some(k) = TODO_MARKS.iter().position(|m| first.text.starts_with(m)) else { continue };
        first.text = first.text[TODO_MARKS[k].len()..].to_string();
        p.runs.retain(|r: &Run| !r.text.is_empty());
        let mut tag = TagUse::of(&def);
        tag.checked = k == 1;
        p.tags.push(tag);
    }
}

fn import(s: &mut Session, v: &Value) -> CmdResult {
    let path = p::req_str(v, "path")?;
    let fmt = format(v, path)?;
    let meta = std::fs::metadata(path).map_err(|e| CmdError::Failed(format!("{path}: {e}")))?;
    let cap = if fmt == Format::Enex { MAX_ENEX_BYTES } else { MAX_FILE_BYTES };
    if meta.len() > cap {
        return Err(CmdError::Failed(format!("{path}: larger than {} MiB", cap >> 20)));
    }
    let bytes = std::fs::read(path).map_err(|e| CmdError::Failed(format!("{path}: {e}")))?;
    let text = import::decode_text(&bytes);
    let Imported { title, mut blocks, pictures_lost } = match fmt {
        Format::Markdown => {
            let base: Option<PathBuf> = Path::new(path)
                .parent()
                .and_then(|d| if d.as_os_str().is_empty() { Path::new(".").canonicalize().ok() } else { d.canonicalize().ok() });
            import::markdown(&text, &mut |rel| base.as_deref().and_then(|b| load_below(b, rel)))
        }
        Format::Text => import::plain_text(&text),
        Format::Html => import::html_document(&text),
        Format::Enex => return import_enex(s, path, &bytes),
    };
    let title = title.unwrap_or_else(|| Path::new(path).file_stem().and_then(|n| n.to_str()).unwrap_or("Imported").to_string());
    todo_tags(s, &mut blocks);
    let r = crate::page_cmds::new_page(s, &json!({"title": title}))?;
    s.sel.on_page = PageSel::None;
    s.pending_outline = None;
    crate::edit_cmds::paste_blocks(s, &json!({}), blocks, crate::edit_cmds::Mode::Keep, true)?;
    Ok(json!({"page": r["page"], "title": title, "picturesLost": pictures_lost}))
}

/// An Evernote export as a new section of the current notebook, a page per note.
fn import_enex(s: &mut Session, path: &str, bytes: &[u8]) -> CmdResult {
    let nb_id = s.sel.notebook.ok_or_else(|| CmdError::Disabled("open a notebook first".into()))?;
    let notes = notecraft_import_export::enex::read(&String::from_utf8_lossy(bytes)).map_err(CmdError::Failed)?;
    let nb = s.ws.notebook(nb_id).ok_or_else(|| CmdError::Params("no such notebook".into()))?.clone();
    let stem = Path::new(path).file_stem().and_then(|n| n.to_str()).map(notecraft_doc::clean_name).filter(|n| !n.is_empty());
    let name = crate::onenote_cmds::unique_name(&nb, &stem.unwrap_or_else(|| "Evernote".into()));
    let now = s.now();
    let section = notecraft_doc::Section::new(s.new_id(), &name, notecraft_doc::SectionColor::rotation(nb.sections().len()), now);
    let sid = section.id;
    s.ws.notebook_mut(nb_id)
        .ok_or_else(|| CmdError::Params("no such notebook".into()))?
        .children
        .push(notecraft_doc::NavNode::Section(std::sync::Arc::new(section)));
    s.goto_section(sid)?;
    let (mut first, mut files, mut pictures, mut encrypted, mut failed) = (None, 0, 0, 0, 0);
    for note in notes {
        let title = if note.title.is_empty() { notecraft_doc::page::UNTITLED.to_string() } else { note.title };
        let r = crate::page_cmds::new_page(s, &json!({"section": sid.to_string(), "title": title}))?;
        first = first.or_else(|| r["page"].as_str().map(str::to_string));
        if let Some(pg) = s.page_mut() {
            pg.created = note.created.unwrap_or(now);
            pg.modified = note.updated.or(note.created).unwrap_or(now);
        }
        let mut blocks = note.content.blocks;
        pictures += blocks.iter().filter(|b| matches!(b, Block::Image { .. })).count();
        todo_tags(s, &mut blocks);
        s.sel.on_page = PageSel::None;
        s.pending_outline = None;
        if crate::edit_cmds::paste_blocks(s, &json!({}), blocks, crate::edit_cmds::Mode::Keep, true).is_err() {
            failed += 1;
        }
        for (name, data) in note.files {
            if data.len() <= notecraft_doc::media::MAX_PICTURE_BYTES && crate::picture_cmds::push_attachment(s, &json!({}), name, data).is_ok() {
                files += 1;
            }
        }
        encrypted += note.encrypted;
    }
    match first.as_deref().and_then(notecraft_doc::Id::parse) {
        Some(pid) => s.goto_page(pid)?,
        None => s.goto_section(sid)?,
    }
    let n = s.ws.section(sid).map_or(0, |x| x.pages.len());
    s.notice(format!("{name}: {n} notes imported"));
    Ok(json!({"section": sid.to_string(), "notes": n, "pictures": pictures, "files": files, "encrypted": encrypted, "damaged": failed}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::tests::{run, sample};

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("notecraft-import-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn texts(s: &crate::Session) -> Vec<String> {
        let page = s.page().unwrap();
        page.objects.iter().filter_map(|o| o.as_outline()).flat_map(|o| o.paragraphs.iter().map(|p| p.text())).collect()
    }

    #[test]
    fn markdown_becomes_a_page_with_tags_tables_and_local_pictures() {
        // parity: NC.File.Import
        let d = dir("md");
        let png =
            notecraft_codecs::encode_png(&notecraft_codecs::Rgba8::new(2, 2, vec![90; 16]).unwrap(), &notecraft_codecs::Limits::PICTURE).unwrap();
        std::fs::create_dir_all(d.join("img")).unwrap();
        std::fs::write(d.join("img/a b.png"), &png).unwrap();
        let outside = dir("md-outside");
        std::fs::write(outside.join("secret.png"), &png).unwrap();
        let md = "# Groceries\n\n- [ ] milk\n- [x] eggs\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n![pic](img/a%20b.png)\n\n![no](../notecraft-import-md-outside-X/secret.png)\n";
        let md = md.replace("X", &std::process::id().to_string());
        std::fs::write(d.join("list.md"), md).unwrap();
        let mut s = sample();
        let pages = s.section().unwrap().pages.len();
        let undo = s.undo_labels().len();
        let r = run(&mut s, "file.import", json!({"path": d.join("list.md").to_str().unwrap()}));
        assert_eq!(r["title"], "Groceries");
        assert_eq!(r["picturesLost"], 1, "the picture outside the folder is not read");
        assert_eq!(s.section().unwrap().pages.len(), pages + 1);
        assert_eq!(s.undo_labels().len(), undo + 1, "one undo step");
        let page = s.page().unwrap();
        assert_eq!(page.title.text(), "Groceries");
        let todo: Vec<(String, bool)> = page
            .objects
            .iter()
            .filter_map(|o| o.as_outline())
            .flat_map(|o| o.paragraphs.iter())
            .filter_map(|p| p.tags.first().map(|t| (p.text(), t.checked)))
            .collect();
        assert_eq!(todo, [("milk".to_string(), false), ("eggs".to_string(), true)]);
        assert_eq!(page.objects.iter().filter(|o| matches!(o, notecraft_doc::PageObject::Table(_))).count(), 1);
        assert_eq!(page.objects.iter().filter(|o| matches!(o, notecraft_doc::PageObject::Image(_))).count(), 1);
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn text_and_html_files_and_refusals() {
        let d = dir("txt");
        std::fs::write(d.join("Notes.txt"), "one\n\ttwo\n").unwrap();
        std::fs::write(d.join("page.htm"), "<h1>Web</h1><p>Hello <b>there</b></p>").unwrap();
        let mut s = sample();
        let r = run(&mut s, "file.import", json!({"path": d.join("Notes.txt").to_str().unwrap()}));
        assert_eq!(r["title"], "Notes", "FMT.Import.Text: the file name");
        assert_eq!(texts(&s), ["one", "two"]);
        let r = run(&mut s, "file.import", json!({"path": d.join("page.htm").to_str().unwrap()}));
        assert_eq!(r["title"], "Web", "FMT.Import.Html");
        assert_eq!(texts(&s), ["Hello there"]);
        assert!(s.run("file.import", json!({"path": d.join("x.docx").to_str().unwrap()})).is_err());
        // An .enex that isn't one.
        std::fs::write(d.join("bad.enex"), "<html/>").unwrap();
        assert!(s.run("file.import", json!({"path": d.join("bad.enex").to_str().unwrap()})).is_err());
        assert!(s.run("file.import", json!({"path": d.join("missing.md").to_str().unwrap()})).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_evernote_export_becomes_a_section() {
        // parity: FMT.Import.Enex
        let d = dir("enex");
        let xml = "<?xml version=\"1.0\"?><en-export><note><title>Trip</title><created>20240305T143000Z</created>\
                   <content><![CDATA[<en-note><div><en-todo checked=\"true\"/>Pack</div><div>Go</div></en-note>]]></content>\
                   <resource><data encoding=\"base64\">aGVsbG8=</data><mime>text/plain</mime><resource-attributes><file-name>a.txt</file-name></resource-attributes></resource>\
                   </note><note><title>Second</title><content><![CDATA[<en-note>two</en-note>]]></content></note></en-export>";
        std::fs::write(d.join("Travel.enex"), xml).unwrap();
        let mut s = sample();
        let sections = s.notebook().unwrap().sections().len();
        let undo = s.undo_labels().len();
        let r = run(&mut s, "file.import", json!({"path": d.join("Travel.enex").to_str().unwrap()}));
        assert_eq!((r["notes"].as_u64(), r["files"].as_u64()), (Some(2), Some(1)), "{r}");
        assert_eq!(s.notebook().unwrap().sections().len(), sections + 1);
        assert_eq!(s.undo_labels().len(), undo + 1, "one undo step");
        let sec = s.section().unwrap();
        assert_eq!(sec.name, "Travel");
        let titles: Vec<String> = sec.pages.iter().map(|p| p.title.text()).collect();
        assert_eq!(titles, ["Trip", "Second"]);
        let page = s.page().unwrap();
        assert_eq!(page.title.text(), "Trip", "the first note is opened");
        assert_eq!(page.created, notecraft_doc::Timestamp::from_civil(2024, 3, 5, 14, 30, 0));
        let tagged: Vec<(String, bool)> = page
            .objects
            .iter()
            .filter_map(|o| o.as_outline())
            .flat_map(|o| o.paragraphs.iter())
            .filter_map(|p| p.tags.first().map(|t| (p.text(), t.checked)))
            .collect();
        assert_eq!(tagged, [("Pack".to_string(), true)]);
        assert!(page.objects.iter().any(|o| matches!(o, notecraft_doc::PageObject::Attachment(a) if a.name == "a.txt")));
        let _ = std::fs::remove_dir_all(&d);
    }
}
