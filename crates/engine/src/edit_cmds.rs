//! Undo/redo, the clipboard, Delete and Format Painter (Home › Clipboard, the Quick Access
//! Toolbar).

use notecraft_doc::{CharFormat, Paragraph, Run};
use notecraft_import_export::html;
use notecraft_text as t;
use serde_json::{Value, json};

use crate::ops::{self, ensure_caret, need_page, need_selection, need_text, no_page};
use crate::selection::{Block, FocusArea, PageSel, Pos};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn can_undo(s: &Session) -> Option<&'static str> {
    if s.can_undo() { None } else { Some("Nothing to undo") }
}

fn can_redo(s: &Session) -> Option<&'static str> {
    if s.can_redo() { None } else { Some("Nothing to redo") }
}

fn can_copy(s: &Session) -> Option<&'static str> {
    if s.sel.focus == FocusArea::PageList && s.sel.page.is_some() {
        return None;
    }
    need_selection(s)
}

fn can_paste(s: &Session) -> Option<&'static str> {
    if s.sel.focus == FocusArea::PageList && s.sel.section.is_some() {
        return None;
    }
    need_page(s)
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    const PASTE: &str = r#"{"text"?: string, "html"?: HTML from the system clipboard, "image"?: base64 picture file from the system clipboard, "external"?: true when the system clipboard holds another program's content}"#;
    vec![
        // QAT.UndoButton — one user action per step.
        CommandSpec::new("edit.undo", "QAT.UndoButton", "Undo", "Quick Access Toolbar/Undo", undo).key("Mod+Z").when(can_undo),
        // QAT.RedoButton
        CommandSpec::new("edit.redo", "QAT.RedoButton", "Redo", "Quick Access Toolbar/Redo", redo).key("Mod+Y").when(can_redo),
        // QAT.NR.Undo — the Undo drop-down: undo several steps at once.
        CommandSpec::new(
            "edit.undoTo",
            "QAT.NR.Undo",
            "Undo (history list)",
            "Quick Access Toolbar/Commands Not in the Ribbon/Undo (history list)",
            undo_to,
        )
        .params(r#"{"steps": int}"#)
        .when(can_undo),
        // QAT.NR.Redo
        CommandSpec::new(
            "edit.redoTo",
            "QAT.NR.Redo",
            "Redo (history list)",
            "Quick Access Toolbar/Commands Not in the Ribbon/Redo (history list)",
            redo_to,
        )
        .params(r#"{"steps": int}"#)
        .when(can_redo),
        // R.Home.Clipboard.Cut — text, containers, or pages when the page list has focus.
        CommandSpec::new("edit.cut", "R.Home.Clipboard.Cut", "Cut", "Home/Clipboard/Cut", cut).key("Mod+X").when(can_copy),
        // R.Home.Clipboard.CopyOneNote
        CommandSpec::new("edit.copy", "R.Home.Clipboard.CopyOneNote", "Copy", "Home/Clipboard/Copy", copy).key("Mod+C").pure().when(can_copy),
        // R.Home.Clipboard.PasteMenu — default paste keeps source formatting (BEH.Paste.HTML).
        CommandSpec::new("edit.paste", "R.Home.Clipboard.PasteMenu", "Paste", "Home/Clipboard/Paste", paste)
            .key("Mod+V")
            .params(PASTE)
            .when(can_paste),
        // R.Home.Clipboard.PasteKeepSource
        CommandSpec::new(
            "edit.pasteKeepSource",
            "R.Home.Clipboard.PasteKeepSource",
            "Paste: Keep Source Formatting",
            "Home/Clipboard/Paste/Keep Source Formatting",
            paste,
        )
        .params(PASTE)
        .when(need_page),
        // R.Home.Clipboard.PasteMerge — keeps emphasis, takes the destination's font, size and
        // colour.
        CommandSpec::new(
            "edit.pasteMerge",
            "R.Home.Clipboard.PasteMerge",
            "Paste: Merge Formatting",
            "Home/Clipboard/Paste/Merge Formatting",
            paste_merge,
        )
        .params(PASTE)
        .when(need_page),
        // R.Home.Clipboard.PasteText
        CommandSpec::new(
            "edit.pasteTextOnly",
            "R.Home.Clipboard.PasteText",
            "Paste: Keep Text Only",
            "Home/Clipboard/Paste/Keep Text Only",
            paste_text,
        )
        .key("Mod+Shift+V")
        .params(PASTE)
        .when(need_page),
        // R.Home.Clipboard.PastePicture — the clipboard's picture, even when text came with it.
        CommandSpec::new("edit.pastePicture", "R.Home.Clipboard.PastePicture", "Paste: Picture", "Home/Clipboard/Paste/Picture", paste_as_picture)
            .params(PASTE)
            .when(need_page),
        // R.Home.Clipboard.PasteGallery — the Paste Options choice.
        CommandSpec::new("edit.pasteOptions", "R.Home.Clipboard.PasteGallery", "Paste Options", "Home/Clipboard/Paste/Paste Options", paste_options)
            .params(r#"{"mode": "keep" | "merge" | "text", "text"?: string}"#)
            .when(need_page),
        // R.Home.BasicText.Delete — deletes the selection, the selected containers, or the next
        // character.
        CommandSpec::new("edit.delete", "R.Home.BasicText.Delete", "Delete", "Home/Basic Text/Delete", delete).key("Delete").when(need_text),
        // R.Home.Clipboard.FormatPainter — arms the painter with the caret's format; applying
        // happens with `format.pasteFormat` when the next selection is made.
        CommandSpec::new("edit.formatPainter", "R.Home.Clipboard.FormatPainter", "Format Painter", "Home/Clipboard/Format Painter", format_painter)
            .pure()
            .when(need_text),
        // KB.Format.CopyFormat
        CommandSpec::new(
            "format.copyFormat",
            "KB.Format.CopyFormat",
            "Copy Formatting",
            "Keyboard/Format notes/Copy the formatting of the selected text (Format Painter)",
            format_painter,
        )
        .key("Mod+Alt+C")
        .pure()
        .when(need_text),
        // KB.Format.PasteFormat
        CommandSpec::new(
            "format.pasteFormat",
            "KB.Format.PasteFormat",
            "Paste Formatting",
            "Keyboard/Format notes/Paste the formatting to the selected text (Format Painter)",
            paste_format,
        )
        .key("Mod+Alt+V")
        .params(r#"{"once"?: bool}"#)
        .when(|s| if s.format_painter.is_none() { Some("Copy a format first (Format Painter)") } else { need_text(s) }),
    ]
}

fn undo(s: &mut Session, _: &Value) -> CmdResult {
    if !s.undo_step() {
        return Err(CmdError::Disabled("nothing to undo".into()));
    }
    Ok(Value::Null)
}

fn redo(s: &mut Session, _: &Value) -> CmdResult {
    if !s.redo_step() {
        return Err(CmdError::Disabled("nothing to redo".into()));
    }
    Ok(Value::Null)
}

fn undo_to(s: &mut Session, v: &Value) -> CmdResult {
    let n = p::req_usize(v, "steps")?.clamp(1, crate::session::UNDO_MAX);
    let mut done = 0;
    while done < n && s.undo_step() {
        done += 1;
    }
    Ok(json!({"undone": done}))
}

fn redo_to(s: &mut Session, v: &Value) -> CmdResult {
    let n = p::req_usize(v, "steps")?.clamp(1, crate::session::UNDO_MAX);
    let mut done = 0;
    while done < n && s.redo_step() {
        done += 1;
    }
    Ok(json!({"redone": done}))
}

/// Copy the current selection into the engine clipboard; returns the plain text (which the UI
/// mirrors to the system clipboard).
pub(crate) fn copy_selection(s: &mut Session) -> Result<String, CmdError> {
    if s.sel.focus == FocusArea::PageList {
        let section = s.section().ok_or_else(no_page)?;
        let ids = crate::page_cmds::selected_pages(s);
        let pages: Vec<_> = section.pages.iter().filter(|p| ids.contains(&p.id)).cloned().collect();
        let text = pages.iter().map(|p| p.display_title()).collect::<Vec<_>>().join("\n");
        s.clipboard = crate::session::Clipboard { text: text.clone(), pages, ..Default::default() };
        return Ok(text);
    }
    let page = s.page().ok_or_else(no_page)?;
    let mut out: Vec<Paragraph> = Vec::new();
    let mut objects = Vec::new();
    // The same selection as HTML for other applications (tables as tables).
    let mut html: Vec<html::Block> = Vec::new();
    let mut text_extra: Vec<String> = Vec::new();
    match &s.sel.on_page {
        PageSel::Objects { ids } => {
            for id in ids {
                match page.objects.iter().find(|o| o.id() == *id) {
                    Some(notecraft_doc::PageObject::Outline(o)) => {
                        out.extend(o.paragraphs.iter().cloned());
                        html.extend(o.paragraphs.iter().cloned().map(html::Block::Para));
                    }
                    // A table copies as a table object (pasted back as a table) and, for
                    // other applications, as tab-separated text and an HTML table.
                    Some(notecraft_doc::PageObject::Table(t)) => {
                        let (all_r, all_c) = ((0, t.n_rows().saturating_sub(1)), (0, t.n_cols().saturating_sub(1)));
                        let rows = crate::table_cmds::rows_as_paragraphs(t, all_r, all_c, &mut || notecraft_doc::Id::NIL);
                        text_extra.push(rows.iter().map(Paragraph::text).collect::<Vec<_>>().join("\n"));
                        html.push(html::Block::Table(crate::table_cmds::cells_of(t, all_r, all_c)));
                        objects.push(notecraft_doc::PageObject::Table(t.clone()));
                    }
                    // Pictures and attachments copy as objects (bytes shared by `Arc`).
                    Some(
                        o @ (notecraft_doc::PageObject::Image(_) | notecraft_doc::PageObject::Attachment(_) | notecraft_doc::PageObject::Ink(_)),
                    ) => objects.push(o.clone()),
                    None => {}
                }
            }
        }
        PageSel::Cells { table, rows, cols } => {
            if let Some(t) = page.table(*table) {
                out.extend(crate::table_cmds::rows_as_paragraphs(t, *rows, *cols, &mut || notecraft_doc::Id::NIL));
                html.push(html::Block::Table(crate::table_cmds::cells_of(t, *rows, *cols)));
            }
        }
        PageSel::Text(_) => {
            let r = s.text_range().ok_or_else(|| CmdError::Disabled("select some text first".into()))?;
            let (a, b) = r.ordered();
            let paras = Session::block_paras(page, a.block).ok_or_else(|| CmdError::Failed("text block not found".into()))?;
            for i in a.para..=b.para {
                let Some(p) = paras.get(i) else { break };
                let start = if i == a.para { a.offset } else { 0 };
                let end = if i == b.para { b.offset } else { p.len() };
                let mut q = p.clone();
                q.runs = t::slice(p, start..end);
                q.tags.clear();
                if i == a.para && start > 0 {
                    // A partial first paragraph carries text only, not paragraph properties.
                    q.list = None;
                }
                out.push(q);
            }
        }
        PageSel::None => return Err(CmdError::Disabled("select some text first".into())),
    }
    if matches!(s.sel.on_page, PageSel::Text(_)) {
        html.extend(out.iter().cloned().map(html::Block::Para));
    }
    let mut parts: Vec<String> = out.iter().map(Paragraph::text).collect();
    parts.extend(text_extra);
    let text = parts.join("\n");
    let html = if html.is_empty() { String::new() } else { html::write(&html) };
    s.clipboard = crate::session::Clipboard { paragraphs: out, text: text.clone(), pages: Vec::new(), objects, html };
    Ok(text)
}

fn copy(s: &mut Session, _: &Value) -> CmdResult {
    let text = copy_selection(s)?;
    Ok(json!({"text": text, "html": s.clipboard.html}))
}

fn cut(s: &mut Session, _: &Value) -> CmdResult {
    let text = copy_selection(s)?;
    let html = s.clipboard.html.clone();
    if s.sel.focus == FocusArea::PageList {
        crate::page_cmds::delete_pages_to_bin(s)?;
    } else {
        crate::text_cmds::delete_selection(s)?;
    }
    Ok(json!({"text": text, "html": html}))
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Keep,
    Merge,
    Text,
}

/// The paragraphs to paste: the rich clipboard when the system clipboard text (if given)
/// still matches it, otherwise the given plain text split into paragraphs.
fn clip_paras(s: &mut Session, v: &Value) -> Result<Vec<Paragraph>, CmdError> {
    let given = p::str(v, "text");
    if given.is_some_and(|t| t.len() > 64 << 20) {
        return Err(CmdError::Params("pasted text is larger than 64 MiB".into()));
    }
    let rich = !s.clipboard.paragraphs.is_empty() && given.is_none_or(|t| t.replace("\r\n", "\n") == s.clipboard.text);
    if rich {
        return Ok(s.clipboard.paragraphs.clone());
    }
    let text = given.map(str::to_string).unwrap_or_else(|| s.clipboard.text.clone());
    if text.is_empty() {
        return Err(CmdError::Disabled("the clipboard is empty".into()));
    }
    let mut out = Vec::new();
    for line in text.replace("\r\n", "\n").split('\n') {
        let mut para = ops::new_para(s);
        // Leading tabs in plain text become outline indentation.
        let tabs = line.chars().take_while(|c| *c == '\t').count();
        para.level = (tabs as u8).min(notecraft_doc::text::MAX_LEVEL);
        let body = line.get(tabs..).unwrap_or("");
        if !body.is_empty() {
            para.runs.push(Run::new(body, CharFormat::default()));
        }
        out.push(para);
    }
    Ok(out)
}

fn adapt(mut paras: Vec<Paragraph>, mode: Mode) -> Vec<Paragraph> {
    for p in &mut paras {
        for r in &mut p.runs {
            r.fmt = match mode {
                Mode::Keep => std::mem::take(&mut r.fmt),
                Mode::Merge => CharFormat {
                    bold: r.fmt.bold,
                    italic: r.fmt.italic,
                    underline: r.fmt.underline,
                    strike: r.fmt.strike,
                    script: r.fmt.script,
                    link: r.fmt.link.take(),
                    ..Default::default()
                },
                Mode::Text => CharFormat::default(),
            };
        }
        if mode == Mode::Text {
            p.list = None;
        }
        p.normalize();
    }
    paras
}

fn do_paste(s: &mut Session, v: &Value, mode: Mode) -> CmdResult {
    if s.sel.focus == FocusArea::PageList && !s.clipboard.pages.is_empty() {
        return crate::page_cmds::paste_pages(s);
    }
    // Copied pictures and attachments, when the system clipboard (if given) still holds what
    // NoteCraft put there.
    let given = p::str(v, "text").map(|t| t.replace("\r\n", "\n"));
    // The UI says when the system clipboard holds something another program put there since.
    let external = p::bool(v, "external") == Some(true);
    if mode != Mode::Text && !external && !s.clipboard.objects.is_empty() && given.as_ref().is_none_or(|t| *t == s.clipboard.text) {
        let ids = paste_objects(s, v)?;
        if s.clipboard.paragraphs.is_empty() {
            s.sel.on_page = PageSel::Objects { ids: ids.clone() };
            return Ok(json!({"objects": ids.iter().map(|i| i.to_string()).collect::<Vec<_>>()}));
        }
    }
    // From another application: its HTML (formatting, lists, tables) or a picture.
    let internal = !external && !s.clipboard.paragraphs.is_empty() && given.as_ref().is_none_or(|t| *t == s.clipboard.text);
    if mode != Mode::Text && !internal {
        if let Some(h) = p::str(v, "html").filter(|h| !h.trim().is_empty()) {
            let blocks = html::parse(h);
            if !blocks.is_empty() {
                return paste_blocks(s, v, blocks, mode);
            }
        }
        if let Some(img) = p::str(v, "image")
            && given.as_ref().is_none_or(|t| t.trim().is_empty())
        {
            return paste_picture(s, v, img);
        }
    }
    let paras = adapt(clip_paras(s, v)?, mode);
    let fresh = fresh_paras(s, paras);
    let caret = ensure_caret(s)?;
    let pos = paste_paragraphs(s, caret, fresh, mode)?;
    s.set_caret(pos);
    ops::touch(s, pos.block, pos.para);
    Ok(Value::Null)
}

/// Fresh ids, author and time: pasting twice must not create duplicate ids.
fn fresh_paras(s: &mut Session, paras: Vec<Paragraph>) -> Vec<Paragraph> {
    let mut fresh = Vec::with_capacity(paras.len());
    for mut p in paras {
        p.id = s.new_id();
        p.author = s.author();
        p.modified = s.now();
        fresh.push(p);
    }
    fresh
}

/// BEH.Paste.Images: a picture from the system clipboard (base64 file bytes) becomes a
/// picture object where a new object goes.
fn paste_picture(s: &mut Session, v: &Value, b64: &str) -> CmdResult {
    let cap = notecraft_doc::media::MAX_PICTURE_BYTES;
    if b64.len() / 4 * 3 > cap.saturating_add(3) {
        return Err(CmdError::Failed(format!("the pasted picture is larger than {} MiB", cap >> 20)));
    }
    let bytes = crate::b64::decode(b64).ok_or_else(|| CmdError::Params("`image` is not base64".into()))?;
    let pixels = crate::picture_cmds::probe(&bytes)?;
    let id = crate::picture_cmds::add_picture(s, v, bytes, pixels)?;
    Ok(json!({"picture": id.to_string()}))
}

/// Paste: Picture. The system clipboard's picture (`image`) is pasted as a picture even when
/// text or HTML came with it (a browser copies both); with none, pictures NoteCraft copied come
/// back as pictures. Text pasted as a picture of itself needs the renderer, so it is refused
/// for now (unverified: what the reference does with text-only content).
fn paste_as_picture(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(img) = p::str(v, "image") {
        return paste_picture(s, v, img);
    }
    let own = !s.clipboard.objects.is_empty() && s.clipboard.objects.iter().all(|o| matches!(o, notecraft_doc::PageObject::Image(_)));
    if own && p::bool(v, "external") != Some(true) {
        let ids = paste_objects(s, v)?;
        s.sel.on_page = PageSel::Objects { ids: ids.clone() };
        return Ok(json!({"objects": ids.iter().map(|i| i.to_string()).collect::<Vec<_>>()}));
    }
    Err(CmdError::Failed("The clipboard holds no picture".into()))
}

/// Where the next pasted object goes: below `anchor` (the object pasted before it), else
/// where a new object goes.
fn below(s: &Session, anchor: Option<notecraft_doc::Id>, v: &Value) -> Result<notecraft_doc::geom::Point, CmdError> {
    let page = s.page().ok_or_else(no_page)?;
    Ok(match anchor.and_then(|id| page.objects.iter().find(|o| o.id() == id)) {
        Some(o) => notecraft_doc::geom::Point::new(o.pos().x, ops::object_bottom(o) + 12.0),
        None => ops::insert_pos(s, page, v),
    })
}

/// BEH.Paste.HTML from another application: the leading paragraphs join the caret's text as a
/// plain paste would; each table becomes a table object, and paragraphs after a table a new
/// container, stacked below in reading order.
fn paste_blocks(s: &mut Session, v: &Value, blocks: Vec<html::Block>, mode: Mode) -> CmdResult {
    enum Seg {
        Paras(Vec<Paragraph>),
        Table(html::TableBlock),
        Image(String, String),
    }
    let mut segs: Vec<Seg> = Vec::new();
    for b in blocks {
        match (b, segs.last_mut()) {
            (html::Block::Para(p), Some(Seg::Paras(ps))) => ps.push(p),
            (html::Block::Para(p), _) => segs.push(Seg::Paras(vec![p])),
            (html::Block::Table(c), _) => segs.push(Seg::Table(c)),
            (html::Block::Image { src, alt }, _) => segs.push(Seg::Image(src, alt)),
        }
    }
    let room = notecraft_doc::media::MAX_PAGE_OBJECTS.saturating_sub(s.page().map_or(0, |p| p.objects.len()));
    if segs.len() > room {
        return Err(CmdError::Failed(format!("a page holds at most {} objects", notecraft_doc::media::MAX_PAGE_OBJECTS)));
    }
    let mut anchor = None;
    let mut last_table = None;
    for (i, seg) in segs.into_iter().enumerate() {
        match seg {
            Seg::Paras(ps) => {
                let paras = fresh_paras(s, adapt(ps, mode));
                last_table = None;
                if i == 0 {
                    let caret = ensure_caret(s)?;
                    let pos = paste_paragraphs(s, caret, paras, mode)?;
                    s.set_caret(pos);
                    ops::touch(s, pos.block, pos.para);
                    let page = s.page().ok_or_else(no_page)?;
                    anchor = match pos.block {
                        Block::Outline(id) => Some(id),
                        Block::Cell(c) => page.table_of_cell(c).map(|(t, _, _)| t.id),
                        Block::Title => None,
                    };
                    continue;
                }
                let at = below(s, anchor, v)?;
                let oid = s.new_id();
                let mut o = notecraft_doc::Outline::new(oid, at, notecraft_doc::Id::NIL);
                o.paragraphs = paras;
                o.normalize();
                let last = o.paragraphs.len().saturating_sub(1);
                let end = o.paragraphs.last().map(Paragraph::len).unwrap_or(0);
                s.page_mut().ok_or_else(no_page)?.objects.push(notecraft_doc::PageObject::Outline(o));
                s.set_caret(Pos::new(Block::Outline(oid), last, end));
                anchor = Some(oid);
            }
            Seg::Table(mut block) => {
                let at = below(s, anchor, v)?;
                block.cells = block.cells.into_iter().map(|row| row.into_iter().map(|c| adapt(c, mode)).collect()).collect();
                let t = crate::table_cmds::table_from_cells(s, at, block);
                let id = t.id;
                s.page_mut().ok_or_else(no_page)?.objects.push(notecraft_doc::PageObject::Table(t));
                anchor = Some(id);
                last_table = Some(id);
            }
            Seg::Image(src, alt) => {
                // A picture that doesn't decode or breaks a limit is left out, not the paste.
                let Some(bytes) = crate::b64::decode(&src).filter(|b| b.len() <= notecraft_doc::media::MAX_PICTURE_BYTES) else { continue };
                let Ok(pixels) = crate::picture_cmds::probe(&bytes) else { continue };
                let at = below(s, anchor, v)?;
                let id = crate::picture_cmds::add_picture(s, &json!({"x": at.x, "y": at.y}), bytes, pixels)?;
                if let Some(im) = s.page_mut().and_then(|pg| pg.image_mut(id)) {
                    im.alt_text = alt;
                    im.normalize();
                }
                anchor = Some(id);
                last_table = None;
            }
        }
    }
    s.pending_outline = None;
    if let Some(t) = last_table {
        crate::table_cmds::caret_in_first_cell(s, t);
    }
    Ok(Value::Null)
}

/// Paste the clipboard's pictures, attachments and tables with fresh ids, the first where a new
/// object goes (`ops::insert_pos`), the rest at the same offsets from it as when copied.
fn paste_objects(s: &mut Session, v: &Value) -> Result<Vec<notecraft_doc::Id>, CmdError> {
    let objects = s.clipboard.objects.clone();
    let page = s.page().ok_or_else(no_page)?;
    if page.objects.len().saturating_add(objects.len()) > notecraft_doc::media::MAX_PAGE_OBJECTS {
        return Err(CmdError::Failed(format!("a page holds at most {} objects", notecraft_doc::media::MAX_PAGE_OBJECTS)));
    }
    let at = ops::insert_pos(s, page, v);
    let first = objects.first().map(notecraft_doc::PageObject::pos).unwrap_or_default();
    let mut ids = Vec::new();
    let mut fresh = Vec::new();
    for mut o in objects {
        match &mut o {
            notecraft_doc::PageObject::Image(im) => im.id = s.new_id(),
            notecraft_doc::PageObject::Attachment(a) => a.id = s.new_id(),
            notecraft_doc::PageObject::Ink(k) => k.id = s.new_id(),
            notecraft_doc::PageObject::Table(t) => crate::table_cmds::refresh_ids(s, t),
            notecraft_doc::PageObject::Outline(_) => continue,
        }
        let id = o.id();
        let p = o.pos();
        o.set_pos(notecraft_doc::geom::Point::new((at.x + p.x - first.x).max(0.0), (at.y + p.y - first.y).max(0.0)));
        ids.push(id);
        fresh.push(o);
    }
    s.pending_outline = None;
    s.page_mut().ok_or_else(no_page)?.objects.extend(fresh);
    Ok(ids)
}

/// Insert pasted paragraphs at `caret`: the first joins the caret paragraph, the rest follow it
/// (levels shifted relative to the caret paragraph), and the caret paragraph's tail joins the
/// last. Returns the caret after the pasted content.
fn paste_paragraphs(s: &mut Session, caret: Pos, paras: Vec<Paragraph>, mode: Mode) -> Result<Pos, CmdError> {
    if paras.is_empty() {
        return Ok(caret);
    }
    let caret_fmt = ops::typing_fmt(s, caret);
    // A pasted equation stays an equation (decision D11): in the caret's paragraph when that is
    // empty, else in a new paragraph after it.
    let mut caret = caret;
    if let [eq] = paras.as_slice()
        && let Some(view) = eq.math
        && caret.block != Block::Title
    {
        let page = s.page().ok_or_else(no_page)?;
        let target = Session::block_paras(page, caret.block).and_then(|ps| ps.get(caret.para));
        if target.is_some_and(|p| p.math.is_none()) {
            if !target.is_some_and(Paragraph::is_empty) {
                let len = target.map_or(0, Paragraph::len);
                caret = crate::text_cmds::split_at_caret(s, Pos { offset: len, ..caret })?;
            }
            let page = s.page_mut().ok_or_else(no_page)?;
            if let Some(p) = Session::para_mut(page, caret.block, caret.para) {
                p.math = Some(view);
                p.list = None;
                p.tags.clear();
            }
        }
    }
    if caret.block == Block::Title || paras.len() == 1 {
        // One paragraph (or the title, which holds one line): insert its runs.
        let mut runs: Vec<Run> = paras.iter().flat_map(|p| p.runs.clone()).collect();
        if caret.block == Block::Title {
            for r in &mut runs {
                r.text = r.text.replace('\n', " ");
            }
        }
        if mode != Mode::Keep {
            for r in &mut runs {
                r.fmt = r.fmt.over(&caret_fmt);
            }
        }
        let page = s.page_mut().ok_or_else(no_page)?;
        let p = Session::para_mut(page, caret.block, caret.para).ok_or_else(|| CmdError::Failed("paragraph not found".into()))?;
        let end = t::insert_runs(p, caret.offset, &runs);
        return Ok(Pos { offset: end, ..caret });
    }
    let min_level = paras.iter().map(|p| p.level).min().unwrap_or(0);
    let tail_id = s.new_id();
    let page = s.page_mut().ok_or_else(no_page)?;
    let list = Session::block_paras_mut(page, caret.block).ok_or_else(|| CmdError::Failed("text block not found".into()))?;
    let cur = list.get_mut(caret.para).ok_or_else(|| CmdError::Failed("paragraph not found".into()))?;
    let base_level = cur.level;
    let tail = t::split(cur, caret.offset, tail_id);
    let mut iter = paras.into_iter();
    if let Some(p0) = iter.next() {
        let runs =
            if mode != Mode::Keep { p0.runs.iter().map(|r| Run { text: r.text.clone(), fmt: r.fmt.over(&caret_fmt) }).collect() } else { p0.runs };
        t::insert_runs(cur, caret.offset, &runs);
    }
    let mut at = caret.para;
    for mut p in iter {
        p.level = (base_level as u16 + (p.level - min_level) as u16).min(notecraft_doc::text::MAX_LEVEL as u16) as u8;
        at += 1;
        list.insert(at, p);
    }
    let last = list.get_mut(at).ok_or_else(|| CmdError::Failed("paragraph not found".into()))?;
    let end = t::merge(last, tail);
    Ok(Pos::new(caret.block, at, end))
}

fn paste(s: &mut Session, v: &Value) -> CmdResult {
    do_paste(s, v, Mode::Keep)
}

fn paste_merge(s: &mut Session, v: &Value) -> CmdResult {
    do_paste(s, v, Mode::Merge)
}

fn paste_text(s: &mut Session, v: &Value) -> CmdResult {
    do_paste(s, v, Mode::Text)
}

fn paste_options(s: &mut Session, v: &Value) -> CmdResult {
    let mode = match p::str(v, "mode").unwrap_or("keep") {
        "keep" => Mode::Keep,
        "merge" => Mode::Merge,
        "text" => Mode::Text,
        other => return Err(CmdError::Params(format!("unknown paste mode `{other}` (keep, merge, text)"))),
    };
    do_paste(s, v, mode)
}

fn delete(s: &mut Session, _: &Value) -> CmdResult {
    if crate::text_cmds::delete_selection(s)? {
        return Ok(Value::Null);
    }
    s.run_inner_delete_forward()
}

impl Session {
    fn run_inner_delete_forward(&mut self) -> CmdResult {
        let Some(c) = self.text_range().map(|r| r.head) else { return Ok(Value::Null) };
        let page = self.page().ok_or_else(no_page)?;
        let text = Session::block_paras(page, c.block).and_then(|ps| ps.get(c.para)).map(Paragraph::text).unwrap_or_default();
        if c.offset < text.len() {
            let end = t::next_grapheme(&text, c.offset);
            let page = self.page_mut().ok_or_else(no_page)?;
            if let Some(pm) = Session::para_mut(page, c.block, c.para) {
                t::delete(pm, c.offset..end);
            }
        }
        Ok(Value::Null)
    }
}

fn format_painter(s: &mut Session, _: &Value) -> CmdResult {
    let page = s.page().ok_or_else(no_page)?;
    let fmt = match s.text_range() {
        Some(r) => {
            let (a, _) = r.ordered();
            Session::block_paras(page, a.block)
                .and_then(|ps| ps.get(a.para))
                .map(|p| p.fmt_at(if r.is_caret() { a.offset } else { a.offset.saturating_add(1) }))
                .unwrap_or_default()
        }
        None => CharFormat::default(),
    };
    let mut f = fmt;
    f.link = None;
    s.format_painter = Some(f);
    Ok(Value::Null)
}

fn paste_format(s: &mut Session, v: &Value) -> CmdResult {
    let Some(f) = s.format_painter.clone() else { return Err(CmdError::Disabled("copy a format first".into())) };
    let segs = ops::segments(s);
    if segs.iter().all(|(_, _, r)| r.is_empty()) {
        s.pending_fmt = Some(f);
        return Ok(Value::Null);
    }
    let page = s.page_mut().ok_or_else(no_page)?;
    for (block, i, r) in segs {
        if let Some(p) = Session::para_mut(page, block, i) {
            t::apply_format(p, r, &|x| {
                let link = x.link.take();
                *x = f.clone();
                x.link = link;
            });
        }
    }
    if p::bool(v, "once").unwrap_or(false) {
        // The ribbon's Format Painter paints once, then disarms.
        s.format_painter = None;
    }
    Ok(Value::Null)
}
