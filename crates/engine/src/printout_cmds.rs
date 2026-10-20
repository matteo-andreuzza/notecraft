//! File printouts (decision D3's "later", `docs/plan.md` §4.9): Insert › File Printout puts the
//! file on the page as an attachment and, below it, one picture per printed page, each linked
//! back to the attachment (`Image::printout`), as the reference does. PDF files are rendered by
//! `notecraft-codecs` (`hayro`, pure Rust) under its page and pixel caps; a picture file prints
//! as itself. Other formats need a print driver NoteCraft doesn't have: they are refused with a
//! hint to attach the file instead.
//!
//! The attachment's context menu can print an attached file later (Insert Printout, which also
//! replaces earlier pages) or delete its pages; a printed page's menu opens the original file.
//! Every editing command is one undo step.

use std::sync::Arc;

use notecraft_codecs::Limits;
use notecraft_codecs::pdf::{self, PdfLimits};
use notecraft_doc::media::{ATTACHMENT_SIZE, MAX_ATTACHMENT_BYTES, MAX_PAGE_OBJECTS, MAX_PICTURE_SIZE, MIN_PICTURE_SIZE};
use notecraft_doc::{Attachment, Blob, Id, Image, PageObject, geom::Point};
use serde_json::{Value, json};

use crate::ops::{self, need_page, no_page};
use crate::selection::PageSel;
use crate::{CmdError, CmdResult, CommandSpec, Session};

/// Space between the attachment and the first page, and between pages, points.
const GAP: f32 = 12.0;

/// One printed page: PNG (or the original picture) bytes, its pixel size, its drawn size in
/// points.
struct PrintedPage {
    bytes: Vec<u8>,
    pixels: (u32, u32),
    size: (f32, f32),
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("insert.printout", "R.Insert.Files.PrintoutFromFileInsert", "File Printout", "Insert/Files/File Printout", insert_printout)
            .params(r#"{"path": PDF or picture file | "data": base64 with "name", "name"?: shown file name, "x"?: pt, "y"?: pt}"#)
            .when(need_page),
        CommandSpec::new(
            "attachment.insertPrintout",
            "CM.FileAttachment.FilePrintoutInsert",
            "Insert Printout",
            "Context/File Attachment/Insert Printout",
            insert_for_attachment,
        )
        .when(need_printable),
        CommandSpec::new(
            "printout.delete",
            "CM.FileAttachment.JotDeletePrintoutPages",
            "Delete Printout Pages",
            "Context/File Attachment/Delete Printout Pages",
            delete_pages,
        )
        .when(need_printed),
        CommandSpec::new("printout.openOriginal", "CM.Picture.ViewOriginalPrintout", "Open Original", "Context/Picture/Open Original", open_original)
            .params(r#"{"dir"?: folder for the copy} → {"path"} for the host to open"#)
            .pure()
            .when(need_printout_page),
    ]
}

/// The selected attachment, if one is selected.
fn selected_attachment(s: &Session) -> Option<&Attachment> {
    let (Some(page), PageSel::Objects { ids }) = (s.page(), &s.sel.on_page) else { return None };
    ids.iter().find_map(|id| page.attachment(*id))
}

fn printable(bytes: &[u8]) -> bool {
    pdf::is_pdf(bytes) || notecraft_codecs::sniff(bytes).is_some()
}

fn need_printable(s: &Session) -> Option<&'static str> {
    match selected_attachment(s) {
        None => Some("Select a file attachment first"),
        Some(a) if !printable(&a.blob.bytes) => Some("NoteCraft prints PDF files and pictures"),
        Some(_) => None,
    }
}

fn pages_of(s: &Session, attachment: Id) -> Vec<Id> {
    s.page()
        .map(|p| p.objects.iter().filter_map(|o| o.as_image().filter(|im| im.printout == Some(attachment)).map(|im| im.id)).collect())
        .unwrap_or_default()
}

fn need_printed(s: &Session) -> Option<&'static str> {
    match selected_attachment(s) {
        None => Some("Select a file attachment first"),
        Some(a) if pages_of(s, a.id).is_empty() => Some("This file has no printout pages"),
        Some(_) => None,
    }
}

/// The attachment a selected printout page belongs to.
fn original_of_selection(s: &Session) -> Option<Id> {
    let (Some(page), PageSel::Objects { ids }) = (s.page(), &s.sel.on_page) else { return None };
    ids.iter().find_map(|id| page.image(*id).and_then(|im| im.printout)).filter(|a| page.attachment(*a).is_some())
}

fn need_printout_page(s: &Session) -> Option<&'static str> {
    if original_of_selection(s).is_none() { Some("Select a page of a file printout first") } else { None }
}

/// Render `bytes` as printed pages: every page of a PDF (up to the printout caps), or a picture
/// file as itself. Returns the pages and how many pages the file has.
fn print(bytes: &[u8]) -> Result<(Vec<PrintedPage>, usize), CmdError> {
    let fail = |e: notecraft_codecs::CodecError| CmdError::Failed(e.to_string());
    if pdf::is_pdf(bytes) {
        let total = pdf::page_sizes(bytes, &PdfLimits::PRINTOUT).map_err(fail)?.len();
        let mut out = Vec::new();
        for (img, (w, h)) in pdf::render(bytes, &PdfLimits::PRINTOUT).map_err(fail)? {
            let png = notecraft_codecs::encode_png(&img, &Limits::PICTURE).map_err(fail)?;
            out.push(PrintedPage { bytes: png, pixels: (img.width, img.height), size: (w, h) });
        }
        return Ok((out, total));
    }
    if notecraft_codecs::sniff(bytes).is_some() {
        let pixels = crate::picture_cmds::probe(bytes)?;
        return Ok((vec![PrintedPage { bytes: bytes.to_vec(), pixels, size: Image::natural_size(pixels, 0) }], 1));
    }
    Err(CmdError::Failed("NoteCraft can print PDF files and pictures; attach other files with Insert › File Attachment".into()))
}

/// Put `pages` below the attachment `att` (replacing any pages it had), linked to it; returns
/// the new pictures' ids.
fn place_pages(s: &mut Session, att: Id, pages: Vec<PrintedPage>, total: usize) -> Result<Vec<Id>, CmdError> {
    let old = pages_of(s, att);
    let page = s.page().ok_or_else(no_page)?;
    let room = MAX_PAGE_OBJECTS.saturating_sub(page.objects.len() - old.len());
    let a = page.attachment(att).ok_or_else(no_page)?;
    let (x, mut y) = (a.pos.x, a.pos.y + ATTACHMENT_SIZE.1 + GAP);
    let mut images = Vec::new();
    for pg in pages.into_iter().take(room) {
        let (id, blob) = (s.new_id(), s.new_id());
        let (w, h) = (pg.size.0.clamp(MIN_PICTURE_SIZE, MAX_PICTURE_SIZE), pg.size.1.clamp(MIN_PICTURE_SIZE, MAX_PICTURE_SIZE));
        let mut im = Image::new(id, Point::new(x, y), Blob::new(blob, Arc::from(pg.bytes)), pg.pixels, w, h);
        im.printout = Some(att);
        y += h + GAP;
        images.push(im);
    }
    let ids: Vec<Id> = images.iter().map(|im| im.id).collect();
    let page = s.page_mut().ok_or_else(no_page)?;
    page.objects.retain(|o| !old.contains(&o.id()));
    page.objects.extend(images.into_iter().map(PageObject::Image));
    if total > ids.len() {
        s.notice(format!("The printout shows the first {} of {total} pages.", ids.len()));
    }
    Ok(ids)
}

fn insert_printout(s: &mut Session, v: &Value) -> CmdResult {
    if s.page().is_some_and(|p| p.objects.len() + 2 > MAX_PAGE_OBJECTS) {
        return Err(CmdError::Failed(format!("a page holds at most {MAX_PAGE_OBJECTS} objects")));
    }
    let (bytes, from_path) = crate::picture_cmds::read_input(v, MAX_ATTACHMENT_BYTES)?;
    // Render first: a file that can't be printed leaves the page untouched.
    let (pages, total) = print(&bytes)?;
    let name = crate::p::str(v, "name").map(str::to_string).or(from_path).unwrap_or_else(|| "Printout.pdf".into());
    let page = s.page().ok_or_else(no_page)?;
    let pos = ops::insert_pos(s, page, v);
    let (id, blob) = (s.new_id(), s.new_id());
    let mut a = Attachment { id, pos, name, blob: Blob::new(blob, Arc::from(bytes)), alt_text: String::new(), link: None };
    a.normalize();
    s.pending_outline = None;
    s.page_mut().ok_or_else(no_page)?.objects.push(PageObject::Attachment(a));
    let ids = place_pages(s, id, pages, total)?;
    s.sel.on_page = PageSel::Objects { ids: vec![id] };
    Ok(json!({"attachment": id.to_string(), "pages": ids.iter().map(Id::to_string).collect::<Vec<_>>(), "total": total}))
}

fn insert_for_attachment(s: &mut Session, _: &Value) -> CmdResult {
    let a = selected_attachment(s).ok_or_else(|| CmdError::Disabled("select a file attachment first".into()))?;
    let (id, bytes) = (a.id, a.blob.bytes.clone());
    if bytes.is_empty() {
        return Err(CmdError::Failed("this file's data is missing (the notebook file was damaged)".into()));
    }
    let (pages, total) = print(&bytes)?;
    let ids = place_pages(s, id, pages, total)?;
    Ok(json!({"pages": ids.iter().map(Id::to_string).collect::<Vec<_>>(), "total": total}))
}

fn delete_pages(s: &mut Session, _: &Value) -> CmdResult {
    let a = selected_attachment(s).map(|a| a.id).ok_or_else(|| CmdError::Disabled("select a file attachment first".into()))?;
    let old = pages_of(s, a);
    if old.is_empty() {
        return Ok(json!({"deleted": 0}));
    }
    s.page_mut().ok_or_else(no_page)?.objects.retain(|o| !old.contains(&o.id()));
    Ok(json!({"deleted": old.len()}))
}

/// Write the printed file to a private folder and return its path (the host opens it), as
/// `attachment.open` does for the attachment itself.
fn open_original(s: &mut Session, v: &Value) -> CmdResult {
    let att = original_of_selection(s).ok_or_else(|| CmdError::Disabled("select a page of a file printout first".into()))?;
    let a = s.page().and_then(|p| p.attachment(att)).ok_or_else(no_page)?;
    if a.blob.bytes.is_empty() {
        return Err(CmdError::Failed("this file's data is missing (the notebook file was damaged)".into()));
    }
    let (blob, name) = (a.blob.clone(), notecraft_doc::media::safe_file_name(&a.name));
    crate::picture_cmds::open_copy(v, blob, &name)
}
