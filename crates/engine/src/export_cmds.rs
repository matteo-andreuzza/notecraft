//! File › Export of a page or a section to other applications (catalog `BS.Export.Page`,
//! `BS.Export.Section`, `BS.Export.FormatMht`; formats `FMT.Export.Html`,
//! `FMT.Export.Markdown`). The notebook archive is `file.exportNotebook` in `file_cmds`.
//!
//! The format comes from `format` or, without it, from the file's extension: Single File Web
//! Page (`.mht`, the reference's web format), Word Document (`.docx`), PDF (`.pdf`, decision
//! D13), HTML (`.html`,
//! pictures inline) or Markdown (`.md`, pictures and attachments written to a `<name>_files`
//! folder beside it). The other formats of the reference's Export page (`.one`, `.doc`, `.pdf`,
//! `.xps`) are refused with a clear message until they exist. The result reports what was written and what was left out
//! (ink has no form in these formats).

use notecraft_doc::Page;
use notecraft_import_export::{docx, export, markdown, pdf};
use serde_json::{Value, json};

use crate::ops::{need_page, need_section, no_page};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // BS.Export.Page — the current page (or `page`).
        CommandSpec::new("file.exportPage", "BS.Export.Page", "Export Page", "File/Export/Page", export_page)
            .params(r#"{"path": file, "format"?: "mht"|"docx"|"pdf"|"html"|"md", "page"?: id}"#)
            .pure()
            .when(need_page),
        // BS.Export.Section and CM.Section.FileSaveAsContextualItem — every page of the
        // current section (or `section`), in page-list order.
        CommandSpec::new("file.exportSection", "BS.Export.Section", "Export Section", "File/Export/Section", export_section)
            .params(r#"{"path": file, "format"?: "mht"|"docx"|"pdf"|"html"|"md", "section"?: id}"#)
            .pure()
            .when(need_section),
        // BS.Export.FormatMht — Single File Web Page, of the page (default) or the section.
        CommandSpec::new(
            "file.exportMht",
            "BS.Export.FormatMht",
            "Export as Single File Web Page",
            "File/Export/Single File Web Page (.mht)",
            export_mht,
        )
        .params(r#"{"path": file, "scope"?: "page"|"section"}"#)
        .pure()
        .when(need_page),
    ]
}

/// An export format of this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Mht,
    Docx,
    Pdf,
    Html,
    Markdown,
}

/// The format asked for, or the one the path's extension names (`.mht` when it names none).
fn format(v: &Value, path: &str) -> Result<Format, CmdError> {
    let name = match p::str(v, "format") {
        Some(f) => f.to_ascii_lowercase(),
        None => std::path::Path::new(path).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_else(|| "mht".into()),
    };
    match name.as_str() {
        "mht" | "mhtml" => Ok(Format::Mht),
        "docx" => Ok(Format::Docx),
        "pdf" => Ok(Format::Pdf),
        "html" | "htm" => Ok(Format::Html),
        "md" | "markdown" => Ok(Format::Markdown),
        "one" | "onepkg" | "doc" | "xps" => {
            Err(CmdError::Disabled(format!("export to .{name} is not available yet (M3: web page, Word, PDF and Markdown first)")))
        }
        other => Err(CmdError::Params(format!("unknown export format `{other}` (mht, docx, pdf, html or md)"))),
    }
}

fn export_page(s: &mut Session, v: &Value) -> CmdResult {
    let path = p::req_str(v, "path")?;
    let page = match p::id(v, "page")? {
        Some(id) => s.ws.page(id).ok_or_else(|| CmdError::Params("no such page".into()))?.clone(),
        None => s.page().ok_or_else(no_page)?.clone(),
    };
    let fmt = format(v, path)?;
    let title = page.display_title();
    write(s, path, fmt, &title, &[&page])
}

fn export_section(s: &mut Session, v: &Value) -> CmdResult {
    let path = p::req_str(v, "path")?;
    let section = match p::id(v, "section")? {
        Some(id) => s.ws.section(id).ok_or_else(|| CmdError::Params("no such section".into()))?.clone(),
        None => s.section().ok_or_else(|| CmdError::Disabled("open a section first".into()))?.clone(),
    };
    let fmt = format(v, path)?;
    let pages: Vec<&Page> = section.pages.iter().map(|p| p.as_ref()).collect();
    write(s, path, fmt, &section.name, &pages)
}

fn export_mht(s: &mut Session, v: &Value) -> CmdResult {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        o.insert("format".into(), json!("mht"));
    }
    match p::str(&v, "scope").unwrap_or("page") {
        "page" => export_page(s, &v),
        "section" => export_section(s, &v),
        other => Err(CmdError::Params(format!("unknown scope `{other}` (page or section)"))),
    }
}

/// The folder Markdown's files go to: `<stem>_files` beside `path`.
fn files_dir(path: &str) -> (std::path::PathBuf, String) {
    let p = std::path::Path::new(path);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("export");
    let name = format!("{stem}_files");
    (p.with_file_name(&name), name)
}

fn write(s: &Session, path: &str, fmt: Format, title: &str, pages: &[&Page]) -> CmdResult {
    let e = export::collect(pages, s.clock.utc_offset_min);
    let mut written = vec![path.to_string()];
    let mut dropped = docx::DocxReport::default();
    let bytes = match fmt {
        Format::Pdf => return write_pdf(s, path, title, pages),
        Format::Mht => export::mht(&e, title),
        Format::Docx => {
            let (bytes, r) = docx::write(&e, title).map_err(CmdError::Failed)?;
            dropped = r;
            bytes
        }
        Format::Html => export::html_document(&e, title, export::Links::Inline).into_bytes(),
        Format::Markdown => {
            let (dir, name) = files_dir(path);
            if !e.resources.is_empty() {
                written.extend(write_files(&dir, &e.resources)?);
            }
            markdown::write(&e, &name).into_bytes()
        }
    };
    write_file(path, &bytes)?;
    let r = e.report;
    Ok(json!({
        "bytes": bytes.len(),
        "pages": r.pages,
        "pictures": r.pictures,
        "files": r.files,
        "tables": r.tables,
        "inkLost": r.ink_lost,
        "filesLost": r.files_lost + dropped.files_dropped + dropped.pictures_dropped,
        "written": written,
    }))
}

/// The pages as PDF (decision D13), with the system's page fonts.
pub(crate) fn write_pdf(s: &Session, path: &str, title: &str, pages: &[&Page]) -> CmdResult {
    let opts = pdf::PdfOptions { title: title.to_string(), author: s.prefs.author.clone(), utc_offset_min: s.clock.utc_offset_min };
    let (bytes, r) = pdf::write(pages, notecraft_fonts::page_fonts(), &opts).map_err(CmdError::Failed)?;
    write_file(path, &bytes)?;
    Ok(json!({
        "bytes": bytes.len(),
        "pages": r.pages,
        "sheets": r.sheets,
        "files": r.files,
        "filesLost": r.pictures_lost,
        // Ink is drawn.
        "inkLost": 0,
        "equationsAsText": r.equations,
        "written": [path],
    }))
}

#[cfg(not(target_arch = "wasm32"))]
fn write_file(path: &str, bytes: &[u8]) -> Result<(), CmdError> {
    std::fs::write(path, bytes).map_err(|e| CmdError::Failed(format!("{path}: {e}")))
}

#[cfg(target_arch = "wasm32")]
fn write_file(_: &str, _: &[u8]) -> Result<(), CmdError> {
    Err(CmdError::Disabled("the web version downloads exports instead".into()))
}

/// Write the resources into `dir` (created; never through a link). Returns the paths.
#[cfg(not(target_arch = "wasm32"))]
fn write_files(dir: &std::path::Path, files: &[export::Resource]) -> Result<Vec<String>, CmdError> {
    let fail = |e: std::io::Error| CmdError::Failed(format!("{}: {e}", dir.display()));
    if dir.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(CmdError::Failed(format!("{}: is a link; not writing through it", dir.display())));
    }
    std::fs::create_dir_all(dir).map_err(fail)?;
    let mut out = Vec::new();
    for r in files {
        let p = dir.join(&r.name);
        std::fs::write(&p, &r.bytes).map_err(fail)?;
        out.push(p.display().to_string());
    }
    Ok(out)
}

#[cfg(target_arch = "wasm32")]
fn write_files(_: &std::path::Path, _: &[export::Resource]) -> Result<Vec<String>, CmdError> {
    Err(CmdError::Disabled("the web version downloads exports instead".into()))
}
