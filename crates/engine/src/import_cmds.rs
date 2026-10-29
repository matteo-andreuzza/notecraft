//! Import a file as a new page of the current section (catalog `NC.File.Import`; formats
//! `FMT.Import.Markdown`, `FMT.Import.Text`, `FMT.Import.Html`): the document's first-level
//! heading (else the file name) is the title, its content is placed the way a rich paste places
//! it, task items become To Do tags. One undo step.
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

/// Largest file imported (the readers look at the first 16 MiB).
const MAX_FILE_BYTES: u64 = 64 << 20;

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // NC.File.Import — Markdown, plain text or HTML into a new page.
        CommandSpec::new("file.import", "NC.File.Import", "Import File", "File/Open/Import File", import)
            .params(r#"{"path": file, "format"?: "md"|"txt"|"html"}"#)
            .when(need_section),
    ]
}

/// A file format this module reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Markdown,
    Text,
    Html,
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
        other => Err(CmdError::Params(format!("unknown import format `{other}` (md, txt or html)"))),
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
    if meta.len() > MAX_FILE_BYTES {
        return Err(CmdError::Failed(format!("{path}: larger than {} MiB", MAX_FILE_BYTES >> 20)));
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
    };
    let title = title.unwrap_or_else(|| Path::new(path).file_stem().and_then(|n| n.to_str()).unwrap_or("Imported").to_string());
    todo_tags(s, &mut blocks);
    let r = crate::page_cmds::new_page(s, &json!({"title": title}))?;
    s.sel.on_page = PageSel::None;
    s.pending_outline = None;
    crate::edit_cmds::paste_blocks(s, &json!({}), blocks, crate::edit_cmds::Mode::Keep, true)?;
    Ok(json!({"page": r["page"], "title": title, "picturesLost": pictures_lost}))
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
        assert!(s.run("file.import", json!({"path": d.join("missing.md").to_str().unwrap()})).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
