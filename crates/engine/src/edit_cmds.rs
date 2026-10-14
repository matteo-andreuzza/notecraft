//! Undo/redo, the clipboard, Delete and Format Painter (Home › Clipboard, the Quick Access
//! Toolbar).

use notecraft_doc::{CharFormat, Paragraph, Run};
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
    const PASTE: &str = r#"{"text"?: string}"#;
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
        let mut ids = s.sel.pages.clone();
        if let Some(p) = s.sel.page
            && !ids.contains(&p)
        {
            ids.insert(0, p);
        }
        let pages: Vec<_> = section.pages.iter().filter(|p| ids.contains(&p.id)).cloned().collect();
        let text = pages.iter().map(|p| p.display_title()).collect::<Vec<_>>().join("\n");
        s.clipboard = crate::session::Clipboard { paragraphs: Vec::new(), text: text.clone(), pages };
        return Ok(text);
    }
    let page = s.page().ok_or_else(no_page)?;
    let mut out: Vec<Paragraph> = Vec::new();
    match &s.sel.on_page {
        PageSel::Objects { ids } => {
            for id in ids {
                if let Some(o) = page.outline(*id) {
                    out.extend(o.paragraphs.iter().cloned());
                }
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
    let text = out.iter().map(Paragraph::text).collect::<Vec<_>>().join("\n");
    s.clipboard = crate::session::Clipboard { paragraphs: out, text: text.clone(), pages: Vec::new() };
    Ok(text)
}

fn copy(s: &mut Session, _: &Value) -> CmdResult {
    let text = copy_selection(s)?;
    Ok(json!({"text": text}))
}

fn cut(s: &mut Session, _: &Value) -> CmdResult {
    let text = copy_selection(s)?;
    if s.sel.focus == FocusArea::PageList {
        crate::page_cmds::delete_pages_to_bin(s)?;
    } else {
        crate::text_cmds::delete_selection(s)?;
    }
    Ok(json!({"text": text}))
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
    let paras = adapt(clip_paras(s, v)?, mode);
    // Fresh ids: pasting twice must not create duplicate ids.
    let mut fresh = Vec::with_capacity(paras.len());
    for mut p in paras {
        p.id = s.new_id();
        p.author = s.author();
        p.modified = s.now();
        fresh.push(p);
    }
    let caret = ensure_caret(s)?;
    let pos = paste_paragraphs(s, caret, fresh, mode)?;
    s.set_caret(pos);
    ops::touch(s, pos.block, pos.para);
    Ok(Value::Null)
}

/// Insert pasted paragraphs at `caret`: the first joins the caret paragraph, the rest follow it
/// (levels shifted relative to the caret paragraph), and the caret paragraph's tail joins the
/// last. Returns the caret after the pasted content.
fn paste_paragraphs(s: &mut Session, caret: Pos, paras: Vec<Paragraph>, mode: Mode) -> Result<Pos, CmdError> {
    if paras.is_empty() {
        return Ok(caret);
    }
    let caret_fmt = ops::typing_fmt(s, caret);
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
