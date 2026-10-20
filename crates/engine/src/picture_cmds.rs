//! Pictures and attached files (decision D3, `docs/plan.md` §4.9): Insert › Pictures and
//! Insert › File Attachment, and the picture and attachment context menus (`CM.Picture.*`,
//! `CM.FileAttachment.*`): rotate, flip, move, resize, restore the original size, set as the
//! page background, alt text, save as, open and rename.
//!
//! The bytes come from a `path` (native builds) or from base64 `data` (the web build, agents,
//! pasted screenshots), are checked against the D3 limits before anything is created, and are
//! probed by `notecraft-codecs` so a file that isn't a readable picture is refused with the
//! decoder's reason instead of landing on the page as a broken object. The commands act on the
//! pictures or attachments selected as objects (`PageSel::Objects`); every editing command
//! records one undo step, Save As and Open don't change the notebook.

use std::sync::Arc;

use notecraft_codecs::Limits;
use notecraft_doc::media::{
    ATTACHMENT_SIZE, MAX_ALT_TEXT, MAX_ATTACHMENT_BYTES, MAX_PAGE_OBJECTS, MAX_PICTURE_BYTES, MAX_PICTURE_SIZE, MIN_PICTURE_SIZE, PT_PER_PX,
};
use notecraft_doc::{Attachment, Blob, Id, Image, PageObject, geom::Point};
use serde_json::{Value, json};

use crate::ops::{self, need_page, no_page};
use crate::selection::PageSel;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// The selected objects that are pictures (`pictures`) or attachments.
fn selected(s: &Session, pictures: bool) -> Vec<Id> {
    let (Some(page), PageSel::Objects { ids }) = (s.page(), &s.sel.on_page) else { return Vec::new() };
    ids.iter().copied().filter(|id| if pictures { page.image(*id).is_some() } else { page.attachment(*id).is_some() }).collect()
}

pub(crate) fn need_picture(s: &Session) -> Option<&'static str> {
    if s.sel.page.is_none() {
        return Some("No page is open");
    }
    if selected(s, true).is_empty() { Some("Select a picture first") } else { None }
}

/// Rotate and Flip work on pictures and on ink (decision D4).
fn need_turnable(s: &Session) -> Option<&'static str> {
    if s.sel.page.is_none() {
        return Some("No page is open");
    }
    if selected(s, true).is_empty() && crate::draw_cmds::selected_ink(s).is_empty() { Some("Select a picture or ink first") } else { None }
}

fn need_attachment(s: &Session) -> Option<&'static str> {
    if s.sel.page.is_none() {
        return Some("No page is open");
    }
    if selected(s, false).is_empty() { Some("Select a file attachment first") } else { None }
}

fn need_media(s: &Session) -> Option<&'static str> {
    if s.sel.page.is_none() {
        return Some("No page is open");
    }
    if selected(s, true).is_empty() && selected(s, false).is_empty() { Some("Select a picture or file attachment first") } else { None }
}

fn need_object(s: &Session) -> Option<&'static str> {
    match &s.sel.on_page {
        _ if s.sel.page.is_none() => Some("No page is open"),
        PageSel::Objects { ids } if !ids.is_empty() => None,
        _ => Some("Select a picture or note container first"),
    }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // R.Insert.Images.PictureFromFileInsert
        CommandSpec::new("insert.picture", "R.Insert.Images.PictureFromFileInsert", "Pictures", "Insert/Images/Pictures", insert_picture)
            .params(r#"{"path": picture file | "data": base64, "x"?: pt, "y"?: pt}"#)
            .when(need_page),
        // R.Insert.Files.FileAttachmentInsert
        CommandSpec::new("insert.file", "R.Insert.Files.FileAttachmentInsert", "File Attachment", "Insert/Files/File Attachment", insert_file)
            .params(r#"{"path": file | "data": base64 with "name", "name"?: shown file name, "x"?: pt, "y"?: pt}"#)
            .when(need_page),
        // CM.Picture.Rotate submenu.
        CommandSpec::new(
            "object.rotateRight90",
            "CM.Picture.ObjectRotateRight90",
            "Rotate Right 90°",
            "Context/Picture/Rotate/Rotate Right 90°",
            |s, _| turn(s, 1),
        )
        .when(need_turnable),
        CommandSpec::new("object.rotateLeft90", "CM.Picture.ObjectRotateLeft90", "Rotate Left 90°", "Context/Picture/Rotate/Rotate Left 90°", |s, _| {
            turn(s, 3)
        })
        .when(need_turnable),
        CommandSpec::new(
            "object.flipHorizontal",
            "CM.Picture.ObjectFlipHorizontal",
            "Flip Horizontal",
            "Context/Picture/Rotate/Flip Horizontal",
            |s, _| flip(s, true),
        )
        .when(need_turnable),
        CommandSpec::new("object.flipVertical", "CM.Picture.ObjectFlipVertical", "Flip Vertical", "Context/Picture/Rotate/Flip Vertical", |s, _| {
            flip(s, false)
        })
        .when(need_turnable),
        // CM.Picture.ObjectMove / ObjectResize — from the menu (no params) they explain the
        // mouse and keyboard gestures; the canvas and the arrow keys pass the amounts.
        CommandSpec::new("object.move", "CM.Picture.ObjectMove", "Move", "Context/Picture/Move", move_objects)
            .params(r#"{"dx": pt, "dy": pt} (selected objects)"#)
            .when(need_object),
        CommandSpec::new("object.resize", "CM.Picture.ObjectResize", "Resize", "Context/Picture/Resize", resize_picture)
            .params(r#"{"id"?: picture id (default: the selected one), "width"?: pt, "height"?: pt, "dw"?: pt, "dh"?: pt, "x"?: pt, "y"?: pt (new top-left, for handles on the left or top)}"#)
            .when(need_page),
        CommandSpec::new(
            "object.restoreSize",
            "CM.Picture.RestoreImageSize",
            "Restore to Original Size",
            "Context/Picture/Restore to Original Size",
            restore_size,
        )
        .when(need_picture),
        CommandSpec::new(
            "object.setAsBackground",
            "CM.Picture.PictureAsBackground",
            "Set Picture as Background",
            "Context/Picture/Set Picture as Background",
            set_background,
        )
        .when(need_picture),
        CommandSpec::new("object.altText", "CM.Picture.AlternativeText", "Alt Text", "Context/Picture/Alt Text", alt_text)
            .params(r#"{"text": alt text (pictures and attachments selected)}"#)
            .when(need_media),
        CommandSpec::new("object.saveAs", "CM.Picture.ImageSaveAs", "Save As (picture)", "Context/Picture/Save As (picture)", save_picture)
            .params(r#"{"path": file to write}"#)
            .pure()
            .when(need_picture),
        // CM.FileAttachment.*
        CommandSpec::new("attachment.open", "CM.FileAttachment.FileAttachmentOpen", "Open", "Context/File Attachment/Open", open_attachment)
            .params(r#"{"dir"?: folder for the copy (default: a private temporary folder)} → {"path"} for the host to open"#)
            .pure()
            .when(need_attachment),
        CommandSpec::new(
            "attachment.saveAs",
            "CM.FileAttachment.FileAttachmentSaveAs",
            "Save As…",
            "Context/File Attachment/Save As…",
            save_attachment,
        )
        .params(r#"{"path": file to write}"#)
        .pure()
        .when(need_attachment),
        CommandSpec::new("attachment.rename", "CM.FileAttachment.RenameSelected", "Rename", "Context/File Attachment/Rename", rename)
            .params(r#"{"name": new file name}"#)
            .when(need_attachment),
    ]
}

/// The bytes a command was given: base64 `data`, or the file at `path` (native only), at most
/// `cap` bytes. Returns the bytes and the file name, if a path gave one.
pub(crate) fn read_input(v: &Value, cap: usize) -> Result<(Vec<u8>, Option<String>), CmdError> {
    if let Some(d) = p::str(v, "data") {
        // Base64 is 4/3 the size; refuse before decoding anything absurd.
        if d.len() / 4 * 3 > cap.saturating_add(3) {
            return Err(CmdError::Params(format!("the file is larger than {} MiB", cap >> 20)));
        }
        let bytes = crate::b64::decode(d).ok_or_else(|| CmdError::Params("`data` is not base64".into()))?;
        if bytes.is_empty() {
            return Err(CmdError::Params("`data` is empty".into()));
        }
        return Ok((bytes, None));
    }
    let path = p::str(v, "path").filter(|s| !s.is_empty()).ok_or_else(|| CmdError::Params("give `path` or base64 `data`".into()))?;
    read_path(path, cap)
}

#[cfg(not(target_arch = "wasm32"))]
fn read_path(path: &str, cap: usize) -> Result<(Vec<u8>, Option<String>), CmdError> {
    use std::io::Read;
    let fail = |e: std::io::Error| CmdError::Failed(format!("{path}: {e}"));
    let f = std::fs::File::open(path).map_err(fail)?;
    let meta = f.metadata().map_err(fail)?;
    if !meta.is_file() {
        return Err(CmdError::Failed(format!("{path}: not a file")));
    }
    if meta.len() > cap as u64 {
        return Err(CmdError::Failed(format!("{path}: larger than {} MiB", cap >> 20)));
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    // The size can change between the check and the read: the read is capped too.
    f.take(cap as u64 + 1).read_to_end(&mut bytes).map_err(fail)?;
    if bytes.len() > cap {
        return Err(CmdError::Failed(format!("{path}: larger than {} MiB", cap >> 20)));
    }
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned());
    Ok((bytes, name))
}

#[cfg(target_arch = "wasm32")]
fn read_path(_: &str, _: usize) -> Result<(Vec<u8>, Option<String>), CmdError> {
    Err(CmdError::Disabled("the web version takes files as `data`, not paths".into()))
}

#[cfg(not(target_arch = "wasm32"))]
fn write_path(path: &str, bytes: &[u8]) -> Result<(), CmdError> {
    if path.is_empty() {
        return Err(CmdError::Params("`path` is empty".into()));
    }
    std::fs::write(path, bytes).map_err(|e| CmdError::Failed(format!("{path}: {e}")))
}

#[cfg(target_arch = "wasm32")]
fn write_path(_: &str, _: &[u8]) -> Result<(), CmdError> {
    Err(CmdError::Disabled("the web version can't write files yet".into()))
}

fn room_for_one(s: &Session) -> Result<(), CmdError> {
    if s.page().is_some_and(|p| p.objects.len() >= MAX_PAGE_OBJECTS) {
        return Err(CmdError::Failed(format!("a page holds at most {MAX_PAGE_OBJECTS} objects")));
    }
    Ok(())
}

/// Put a picture file on the page where `v`'s `x`/`y` or the caret say (see
/// [`ops::insert_pos`]) and select it. `bytes` must already be a probed picture of `pixels`.
pub(crate) fn add_picture(s: &mut Session, v: &Value, bytes: Vec<u8>, pixels: (u32, u32)) -> Result<Id, CmdError> {
    room_for_one(s)?;
    let page = s.page().ok_or_else(no_page)?;
    let pos = ops::insert_pos(s, page, v);
    let (id, blob_id) = (s.new_id(), s.new_id());
    let (w, h) = Image::natural_size(pixels, 0);
    let image = Image::new(id, pos, Blob::new(blob_id, Arc::from(bytes)), pixels, w, h);
    s.pending_outline = None;
    s.page_mut().ok_or_else(no_page)?.objects.push(PageObject::Image(image));
    s.sel.on_page = PageSel::Objects { ids: vec![id] };
    Ok(id)
}

/// Probe picture bytes against the D3 limits (an error the user can read, never a panic).
pub(crate) fn probe(bytes: &[u8]) -> Result<(u32, u32), CmdError> {
    let info = notecraft_codecs::probe(bytes, &Limits::PICTURE).map_err(|e| CmdError::Failed(e.to_string()))?;
    Ok((info.width, info.height))
}

fn insert_picture(s: &mut Session, v: &Value) -> CmdResult {
    room_for_one(s)?;
    let (bytes, _) = read_input(v, MAX_PICTURE_BYTES)?;
    let pixels = probe(&bytes)?;
    let id = add_picture(s, v, bytes, pixels)?;
    Ok(json!({"picture": id.to_string(), "pixels": [pixels.0, pixels.1]}))
}

fn insert_file(s: &mut Session, v: &Value) -> CmdResult {
    room_for_one(s)?;
    let (bytes, from_path) = read_input(v, MAX_ATTACHMENT_BYTES)?;
    let name = p::str(v, "name").map(str::to_string).or(from_path).unwrap_or_default();
    let page = s.page().ok_or_else(no_page)?;
    let pos = ops::insert_pos(s, page, v);
    let (id, blob_id) = (s.new_id(), s.new_id());
    let mut a = Attachment { id, pos, name, blob: Blob::new(blob_id, Arc::from(bytes)), alt_text: String::new() };
    a.normalize();
    s.pending_outline = None;
    s.page_mut().ok_or_else(no_page)?.objects.push(PageObject::Attachment(a));
    s.sel.on_page = PageSel::Objects { ids: vec![id] };
    Ok(json!({"attachment": id.to_string()}))
}

/// Apply `f` to every selected picture.
fn each_picture(s: &mut Session, f: impl Fn(&mut Image)) -> CmdResult {
    let ids = selected(s, true);
    let page = s.page_mut().ok_or_else(no_page)?;
    for id in ids {
        if let Some(im) = page.image_mut(id) {
            f(im);
            im.normalize();
        }
    }
    Ok(Value::Null)
}

/// Turn by `quarters` clockwise quarter turns about the picture's centre (90° turns swap the
/// drawn width and height; the top-left corner stays on the page).
fn turn(s: &mut Session, quarters: u8) -> CmdResult {
    crate::draw_cmds::transform_ink(s, |k, c| notecraft_ink::rotate(k, c, f32::from(quarters) * 90.0))?;
    if selected(s, true).is_empty() {
        return Ok(Value::Null);
    }
    each_picture(s, |im| {
        im.quarter_turns = (im.quarter_turns + quarters) % 4;
        if quarters % 2 == 1 {
            let (cx, cy) = (im.pos.x + im.width / 2.0, im.pos.y + im.height / 2.0);
            std::mem::swap(&mut im.width, &mut im.height);
            im.pos = Point::new((cx - im.width / 2.0).max(0.0), (cy - im.height / 2.0).max(0.0));
        }
    })
}

/// Mirror as seen on the page: flips are stored in the picture's own axes (applied before the
/// turns), so on a picture turned 90° a horizontal flip on screen is a vertical one stored.
fn flip(s: &mut Session, horizontal: bool) -> CmdResult {
    crate::draw_cmds::transform_ink(s, |k, c| notecraft_ink::flip(k, c, horizontal))?;
    if selected(s, true).is_empty() {
        return Ok(Value::Null);
    }
    each_picture(s, |im| {
        if horizontal == (im.quarter_turns % 2 == 0) {
            im.flip_h = !im.flip_h;
        } else {
            im.flip_v = !im.flip_v;
        }
    })
}

fn move_objects(s: &mut Session, v: &Value) -> CmdResult {
    let (Some(dx), Some(dy)) = (p::f32(v, "dx"), p::f32(v, "dy")) else {
        s.notice("Drag the selection, or use the arrow keys, to move it.");
        return Ok(Value::Null);
    };
    let PageSel::Objects { ids } = s.sel.on_page.clone() else { return Err(CmdError::Disabled("select an object first".into())) };
    let page = s.page_mut().ok_or_else(no_page)?;
    for o in page.objects.iter_mut().filter(|o| ids.contains(&o.id())) {
        let p = o.pos();
        o.set_pos(Point::new((p.x + dx).max(0.0), (p.y + dy).max(0.0)));
    }
    s.coalesce = Some(format!("nudge:{ids:?}"));
    Ok(Value::Null)
}

fn resize_picture(s: &mut Session, v: &Value) -> CmdResult {
    let id = match p::id(v, "id")? {
        Some(id) => id,
        None => *selected(s, true).first().ok_or_else(|| CmdError::Disabled("select a picture first".into()))?,
    };
    let im = s.page().and_then(|p| p.image(id)).ok_or_else(|| CmdError::Params("no such picture on this page".into()))?;
    let (w0, h0) = (im.width, im.height);
    let ratio = if w0 > 0.0 { h0 / w0 } else { 1.0 };
    let (w, h) = match (p::f32(v, "width"), p::f32(v, "height"), p::f32(v, "dw"), p::f32(v, "dh")) {
        (Some(w), Some(h), _, _) => (w, h),
        // One side given: the other follows, keeping the proportions (a corner handle).
        (Some(w), None, _, _) => (w, w * ratio),
        (None, Some(h), _, _) => (h / ratio.max(1e-6), h),
        (None, None, Some(dw), dh) => {
            let w = w0 + dw;
            (w, dh.map_or(w * ratio, |dh| h0 + dh))
        }
        (None, None, None, Some(dh)) => {
            let h = h0 + dh;
            (h / ratio.max(1e-6), h)
        }
        (None, None, None, None) => {
            s.notice("Drag a corner handle, or use Shift with the arrow keys, to resize the picture.");
            return Ok(Value::Null);
        }
    };
    if !(w.is_finite() && h.is_finite()) {
        return Err(CmdError::Params("sizes must be numbers".into()));
    }
    let at = match (p::f32(v, "x"), p::f32(v, "y")) {
        (Some(x), Some(y)) => Some(Point::new(x.max(0.0), y.max(0.0)).sanitized()),
        _ => None,
    };
    let im = s.page_mut().and_then(|p| p.image_mut(id)).ok_or_else(no_page)?;
    im.width = w.clamp(MIN_PICTURE_SIZE, MAX_PICTURE_SIZE);
    im.height = h.clamp(MIN_PICTURE_SIZE, MAX_PICTURE_SIZE);
    if let Some(at) = at {
        im.pos = at;
    }
    s.coalesce = Some(format!("resize:{id}"));
    Ok(json!({"width": w.clamp(MIN_PICTURE_SIZE, MAX_PICTURE_SIZE), "height": h.clamp(MIN_PICTURE_SIZE, MAX_PICTURE_SIZE)}))
}

/// Back to the picture's pixel size at 96 dpi (not capped to the insert width), turned.
fn restore_size(s: &mut Session, _: &Value) -> CmdResult {
    each_picture(s, |im| {
        if im.pixels.0 == 0 || im.pixels.1 == 0 {
            return;
        }
        let (w, h) = (im.pixels.0 as f32 * PT_PER_PX, im.pixels.1 as f32 * PT_PER_PX);
        (im.width, im.height) = if im.quarter_turns % 2 == 1 { (h, w) } else { (w, h) };
    })
}

/// Toggle: a background picture goes behind every other object and is left alone by left
/// clicks (the canvas only reaches it through its context menu).
fn set_background(s: &mut Session, _: &Value) -> CmdResult {
    let ids = selected(s, true);
    let page = s.page_mut().ok_or_else(no_page)?;
    let on = !ids.iter().all(|id| page.image(*id).is_some_and(|im| im.background));
    for id in &ids {
        if let Some(im) = page.image_mut(*id) {
            im.background = on;
        }
        if on && let Some(i) = page.object_index(*id) {
            let o = page.objects.remove(i);
            page.objects.insert(0, o);
        }
    }
    if on {
        s.sel.on_page = PageSel::None;
    }
    Ok(json!({"background": on}))
}

fn alt_text(s: &mut Session, v: &Value) -> CmdResult {
    let text = p::req_str(v, "text")?;
    if text.chars().count() > MAX_ALT_TEXT {
        return Err(CmdError::Params(format!("alt text is at most {MAX_ALT_TEXT} characters")));
    }
    let ids: Vec<Id> = selected(s, true).into_iter().chain(selected(s, false)).collect();
    let page = s.page_mut().ok_or_else(no_page)?;
    for id in ids {
        if let Some(im) = page.image_mut(id) {
            im.alt_text = text.to_string();
            im.normalize();
        } else if let Some(a) = page.attachment_mut(id) {
            a.alt_text = text.to_string();
            a.normalize();
        }
    }
    Ok(Value::Null)
}

fn first_blob(s: &Session, pictures: bool) -> Result<Blob, CmdError> {
    let id = *selected(s, pictures).first().ok_or_else(|| CmdError::Disabled("nothing selected".into()))?;
    let page = s.page().ok_or_else(no_page)?;
    let blob = page.objects.iter().find(|o| o.id() == id).and_then(PageObject::blob).cloned().ok_or_else(no_page)?;
    if blob.bytes.is_empty() {
        return Err(CmdError::Failed("this file's data is missing (the notebook file was damaged)".into()));
    }
    Ok(blob)
}

fn save_picture(s: &mut Session, v: &Value) -> CmdResult {
    let blob = first_blob(s, true)?;
    let path = p::req_str(v, "path")?;
    write_path(path, &blob.bytes)?;
    Ok(json!({"bytes": blob.bytes.len()}))
}

fn save_attachment(s: &mut Session, v: &Value) -> CmdResult {
    let blob = first_blob(s, false)?;
    let path = p::req_str(v, "path")?;
    write_path(path, &blob.bytes)?;
    Ok(json!({"bytes": blob.bytes.len()}))
}

/// Write the attachment to a private folder (`dir`, or the system's temporary folder) under its
/// own name and return the path; the host opens it with the system's handler.
fn open_attachment(s: &mut Session, v: &Value) -> CmdResult {
    let blob = first_blob(s, false)?;
    let id = *selected(s, false).first().ok_or_else(no_page)?;
    let name = s.page().and_then(|p| p.attachment(id)).map(|a| notecraft_doc::media::safe_file_name(&a.name)).ok_or_else(no_page)?;
    open_copy(v, blob, &name)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn open_copy(v: &Value, blob: Blob, name: &str) -> CmdResult {
    let base = match p::str(v, "dir").filter(|d| !d.is_empty()) {
        Some(d) => std::path::PathBuf::from(d),
        None => std::env::temp_dir().join("notecraft-attachments"),
    };
    // One folder per blob, so two attachments with the same name don't overwrite each other.
    let dir = base.join(blob.id.to_string());
    std::fs::create_dir_all(&dir).map_err(|e| CmdError::Failed(format!("{}: {e}", dir.display())))?;
    let path = dir.join(name);
    std::fs::write(&path, &blob.bytes).map_err(|e| CmdError::Failed(format!("{}: {e}", path.display())))?;
    Ok(json!({"path": path.display().to_string()}))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn open_copy(_: &Value, _: Blob, _: &str) -> CmdResult {
    Err(CmdError::Disabled("the web version can't open attached files yet".into()))
}

fn rename(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "name")?;
    if name.trim().is_empty() {
        return Err(CmdError::Params("the name is empty".into()));
    }
    let id = *selected(s, false).first().ok_or_else(|| CmdError::Disabled("select a file attachment first".into()))?;
    let a = s.page_mut().and_then(|p| p.attachment_mut(id)).ok_or_else(no_page)?;
    a.name = name.to_string();
    a.normalize();
    Ok(json!({"name": a.name}))
}

/// The drawn size of an object for layout estimates in the engine (the UI measures the real
/// one): pictures have theirs, attachments a fixed card.
pub(crate) fn media_size(o: &PageObject) -> Option<(f32, f32)> {
    match o {
        PageObject::Image(im) => Some((im.width, im.height)),
        PageObject::Attachment(_) => Some(ATTACHMENT_SIZE),
        _ => None,
    }
}
