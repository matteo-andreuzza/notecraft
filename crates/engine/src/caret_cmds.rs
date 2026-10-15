//! Caret movement and text selection.
//!
//! Visual-line moves (Up/Down, Home/End on a wrapped line) depend on text layout, which lives
//! in the UI: the UI computes the target and passes it as `{"para", "offset"}`. Without a
//! target (CLI, agents) the commands use logical lines — soft breaks inside a paragraph and
//! paragraph boundaries — which is exact for unwrapped text.

use notecraft_text as t;
use serde_json::{Value, json};

use crate::ops::{need_page, no_page};
use crate::selection::{Block, PageSel, Pos};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn need_caret(s: &Session) -> Option<&'static str> {
    if s.sel.page.is_none() {
        return Some("No page is open");
    }
    if s.text_range().is_none() { Some("Click in a note first") } else { None }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    const EXT: &str = r#"{"extend"?: bool, "para"?: int, "offset"?: int}"#;
    vec![
        // NC.Caret.Set — a click in text (the UI resolves the click to a position).
        CommandSpec::new("caret.set", "NC.Caret.Set", "Place Caret", "Mouse/Click in text", set)
            .params(r#"{"block": "title" | outline id | cell id, "para": int, "offset": int, "extend"?: bool}"#)
            .pure()
            .when(need_page),
        // NC.Caret.Left / Right / Up / Down — arrow keys (Shift extends).
        CommandSpec::new("caret.left", "NC.Caret.Left", "Move Left", "Keyboard/Navigation/Left", left)
            .key("Left")
            .params(EXT)
            .pure()
            .when(need_caret),
        CommandSpec::new("caret.right", "NC.Caret.Right", "Move Right", "Keyboard/Navigation/Right", right)
            .key("Right")
            .params(EXT)
            .pure()
            .when(need_caret),
        CommandSpec::new("caret.up", "NC.Caret.Up", "Move Up", "Keyboard/Navigation/Up", up).key("Up").params(EXT).pure().when(need_caret),
        CommandSpec::new("caret.down", "NC.Caret.Down", "Move Down", "Keyboard/Navigation/Down", down)
            .key("Down")
            .params(EXT)
            .pure()
            .when(need_caret),
        // KB.General.LineStart / LineEnd
        CommandSpec::new(
            "caret.lineStart",
            "KB.General.LineStart",
            "Move to the beginning of the line",
            "Keyboard/General/Move to the beginning of the line",
            line_start,
        )
        .key("Home")
        .params(EXT)
        .pure()
        .when(need_caret),
        CommandSpec::new(
            "caret.lineEnd",
            "KB.General.LineEnd",
            "Move to the end of the line",
            "Keyboard/General/Move to the end of the line",
            line_end,
        )
        .key("End")
        .params(EXT)
        .pure()
        .when(need_caret),
        // KB.General.WordLeft / WordRight
        CommandSpec::new(
            "caret.wordLeft",
            "KB.General.WordLeft",
            "Move one word to the left",
            "Keyboard/General/Move one word to the left",
            word_left,
        )
        .key("Mod+Left")
        .params(EXT)
        .pure()
        .when(need_caret),
        CommandSpec::new(
            "caret.wordRight",
            "KB.General.WordRight",
            "Move one word to the right",
            "Keyboard/General/Move one word to the right",
            word_right,
        )
        .key("Mod+Right")
        .params(EXT)
        .pure()
        .when(need_caret),
        // KB.Select.ToLineEnd — Shift+End.
        CommandSpec::new(
            "caret.selectLineEnd",
            "KB.Select.ToLineEnd",
            "Select to the end of the line",
            "Keyboard/Select/Select to the end of the line",
            select_line_end,
        )
        .key("Shift+End")
        .params(EXT)
        .pure()
        .when(need_caret),
        // KB.Select.Line — Shift+Down.
        CommandSpec::new(
            "caret.selectDown",
            "KB.Select.Line",
            "Select the whole line (Shift+Down at the line start)",
            "Keyboard/Select/Select the whole line (Shift+Down at the line start)",
            select_down,
        )
        .key("Shift+Down")
        .params(EXT)
        .pure()
        .when(need_caret),
        // KB.Pages.NextParagraph / PrevParagraph — Ctrl+Down / Ctrl+Up.
        CommandSpec::new(
            "caret.nextParagraph",
            "KB.Pages.NextParagraph",
            "Go to the next paragraph",
            "Keyboard/Pages/Go to the next paragraph",
            next_paragraph,
        )
        .key("Mod+Down")
        .params(EXT)
        .pure()
        .when(need_caret),
        CommandSpec::new(
            "caret.prevParagraph",
            "KB.Pages.PrevParagraph",
            "Go to the previous paragraph",
            "Keyboard/Pages/Go to the previous paragraph",
            prev_paragraph,
        )
        .key("Mod+Up")
        .params(EXT)
        .pure()
        .when(need_caret),
        // KB.General.SelectAll — Ctrl+A, repeated to expand: paragraph → container → every
        // container on the page (unverified exact steps).
        CommandSpec::new(
            "edit.selectAll",
            "KB.General.SelectAll",
            "Select All",
            "Keyboard/General/Select all items on the current page (repeat to expand)",
            select_all,
        )
        .key("Mod+A")
        .pure()
        .when(need_page),
        // KB.Select.Cancel — Esc.
        CommandSpec::new(
            "edit.cancelSelection",
            "KB.Select.Cancel",
            "Cancel Selection",
            "Keyboard/Select/Cancel selecting the outline or page",
            cancel,
        )
        .key("Escape")
        .pure()
        .when(need_page),
        // BEH.Canvas.ClickToType — a click on empty canvas: a container appears there when the
        // user types.
        CommandSpec::new("outline.new", "BEH.Canvas.ClickToType", "Click to Type", "Behaviour/Canvas", click_to_type)
            .params(r#"{"x": number, "y": number}"#)
            .pure()
            .when(need_page),
    ]
}

fn parse_block(s: &Session, v: &Value) -> Result<Block, CmdError> {
    match v.get("block") {
        Some(Value::String(b)) if b == "title" => Ok(Block::Title),
        Some(Value::String(b)) => {
            let id = notecraft_doc::Id::parse(b).ok_or_else(|| CmdError::Params("`block` must be \"title\", an outline id or a cell id".into()))?;
            let page = s.page().ok_or_else(no_page)?;
            if page.outline(id).is_some() {
                Ok(Block::Outline(id))
            } else if page.cell(id).is_some() {
                Ok(Block::Cell(id))
            } else {
                Err(CmdError::Params("no such note container or table cell on this page".into()))
            }
        }
        Some(Value::Object(_)) => {
            serde_json::from_value(v.get("block").cloned().unwrap_or_default()).map_err(|e| CmdError::Params(format!("block: {e}")))
        }
        _ => Err(CmdError::Params("`block` is required".into())),
    }
}

fn set(s: &mut Session, v: &Value) -> CmdResult {
    let block = parse_block(s, v)?;
    let para = p::usize(v, "para").unwrap_or(0);
    let offset = p::usize(v, "offset").unwrap_or(0);
    let pos = s.clamp_pos(Pos::new(block, para, offset)).ok_or_else(|| CmdError::Params("position not on this page".into()))?;
    s.pending_outline = None;
    move_to(s, pos, p::bool(v, "extend").unwrap_or(false));
    Ok(json!({"para": pos.para, "offset": pos.offset}))
}

/// Move the caret to `pos`, extending the selection from its anchor when `extend`.
pub(crate) fn move_to(s: &mut Session, pos: Pos, extend: bool) {
    match (extend, s.text_range()) {
        (true, Some(r)) if r.anchor.block == pos.block => s.set_range(r.anchor, pos),
        _ => s.set_caret(pos),
    }
}

fn caret(s: &Session) -> Result<Pos, CmdError> {
    s.text_range().map(|r| r.head).ok_or_else(|| CmdError::Disabled("click in a note first".into()))
}

fn para_text(s: &Session, block: Block, para: usize) -> Option<String> {
    s.page().and_then(|p| Session::block_paras(p, block)).and_then(|ps| ps.get(para)).map(|p| p.text())
}

fn para_count(s: &Session, block: Block) -> usize {
    s.page().and_then(|p| Session::block_paras(p, block)).map(|ps| ps.len()).unwrap_or(0)
}

fn hidden(s: &Session, block: Block, para: usize) -> bool {
    match block {
        Block::Title => false,
        Block::Outline(id) => s.page().and_then(|p| p.outline(id)).is_some_and(|o| o.is_hidden(para)),
        Block::Cell(_) => false,
    }
}

/// Across a table cell's edge: the neighbouring cell's start (`forward`) or end, or `None`
/// outside tables and at the table's edge.
fn across_cell(s: &Session, block: Block, dr: i8, dc: i8, forward: bool) -> Option<Pos> {
    let Block::Cell(cell) = block else { return None };
    let next = crate::table_cmds::neighbour_cell(s, cell, dr, dc)?;
    let paras = s.page().and_then(|p| p.cell(next))?.paragraphs.clone();
    if forward {
        Some(Pos::new(Block::Cell(next), 0, 0))
    } else {
        let last = paras.len().checked_sub(1)?;
        Some(Pos::new(Block::Cell(next), last, paras.get(last).map(|p| p.len()).unwrap_or(0)))
    }
}

/// The nearest visible paragraph before/after `para` (collapsed children are skipped).
fn step_para(s: &Session, block: Block, para: usize, forward: bool) -> Option<usize> {
    let n = para_count(s, block);
    let mut i = para;
    loop {
        i = if forward { i.checked_add(1).filter(|&j| j < n)? } else { i.checked_sub(1)? };
        if !hidden(s, block, i) {
            return Some(i);
        }
    }
}

fn ui_target(s: &Session, v: &Value, block: Block) -> Option<Pos> {
    let para = p::usize(v, "para")?;
    let offset = p::usize(v, "offset")?;
    s.clamp_pos(Pos::new(block, para, offset))
}

fn left(s: &mut Session, v: &Value) -> CmdResult {
    let extend = p::bool(v, "extend").unwrap_or(false);
    let r = s.text_range().ok_or_else(|| CmdError::Disabled("click in a note first".into()))?;
    if !extend && !r.is_caret() {
        s.set_caret(r.ordered().0);
        return Ok(Value::Null);
    }
    let c = r.head;
    let text = para_text(s, c.block, c.para).unwrap_or_default();
    let pos = if c.offset > 0 {
        Pos { offset: t::prev_grapheme(&text, c.offset), ..c }
    } else if let Some(prev) = step_para(s, c.block, c.para, false) {
        Pos::new(c.block, prev, para_text(s, c.block, prev).map(|t| t.len()).unwrap_or(0))
    } else if !extend && let Some(p) = across_cell(s, c.block, 0, -1, false) {
        p
    } else {
        c
    };
    move_to(s, pos, extend);
    Ok(Value::Null)
}

fn right(s: &mut Session, v: &Value) -> CmdResult {
    let extend = p::bool(v, "extend").unwrap_or(false);
    let r = s.text_range().ok_or_else(|| CmdError::Disabled("click in a note first".into()))?;
    if !extend && !r.is_caret() {
        s.set_caret(r.ordered().1);
        return Ok(Value::Null);
    }
    let c = r.head;
    let text = para_text(s, c.block, c.para).unwrap_or_default();
    let pos = if c.offset < text.len() {
        Pos { offset: t::next_grapheme(&text, c.offset), ..c }
    } else if let Some(next) = step_para(s, c.block, c.para, true) {
        Pos::new(c.block, next, 0)
    } else if !extend && let Some(p) = across_cell(s, c.block, 0, 1, true) {
        p
    } else {
        c
    };
    move_to(s, pos, extend);
    Ok(Value::Null)
}

fn vertical(s: &mut Session, v: &Value, forward: bool, extend: bool) -> CmdResult {
    let c = caret(s)?;
    if let Some(target) = ui_target(s, v, c.block) {
        move_to(s, target, extend);
        return Ok(Value::Null);
    }
    let text = para_text(s, c.block, c.para).unwrap_or_default();
    // Logical lines inside the paragraph first (soft breaks), then neighbouring paragraphs.
    let col = c.offset - t::line_start(&text, c.offset);
    let pos = if forward {
        let le = t::line_end(&text, c.offset);
        if le < text.len() {
            let ns = le + 1;
            let ne = t::line_end(&text, ns);
            Pos { offset: t::snap(&text, (ns + col).min(ne)), ..c }
        } else if let Some(next) = step_para(s, c.block, c.para, true) {
            let nt = para_text(s, c.block, next).unwrap_or_default();
            Pos::new(c.block, next, t::snap(&nt, col.min(t::line_end(&nt, 0))))
        } else if !extend && let Some(p) = across_cell(s, c.block, 1, 0, true) {
            p
        } else {
            Pos { offset: text.len(), ..c }
        }
    } else {
        let ls = t::line_start(&text, c.offset);
        if ls > 0 {
            let ps = t::line_start(&text, ls - 1);
            Pos { offset: t::snap(&text, (ps + col).min(ls - 1)), ..c }
        } else if let Some(prev) = step_para(s, c.block, c.para, false) {
            let pt = para_text(s, c.block, prev).unwrap_or_default();
            let ls = t::line_start(&pt, pt.len());
            Pos::new(c.block, prev, t::snap(&pt, (ls + col).min(pt.len())))
        } else if !extend && let Some(p) = across_cell(s, c.block, -1, 0, false) {
            p
        } else {
            Pos { offset: 0, ..c }
        }
    };
    move_to(s, pos, extend);
    Ok(Value::Null)
}

fn up(s: &mut Session, v: &Value) -> CmdResult {
    vertical(s, v, false, p::bool(v, "extend").unwrap_or(false))
}

fn down(s: &mut Session, v: &Value) -> CmdResult {
    vertical(s, v, true, p::bool(v, "extend").unwrap_or(false))
}

fn select_down(s: &mut Session, v: &Value) -> CmdResult {
    vertical(s, v, true, true)
}

fn home_end(s: &mut Session, v: &Value, end: bool, extend: bool) -> CmdResult {
    let c = caret(s)?;
    if let Some(target) = ui_target(s, v, c.block) {
        move_to(s, target, extend);
        return Ok(Value::Null);
    }
    let text = para_text(s, c.block, c.para).unwrap_or_default();
    let off = if end { t::line_end(&text, c.offset) } else { t::line_start(&text, c.offset) };
    move_to(s, Pos { offset: off, ..c }, extend);
    Ok(Value::Null)
}

fn line_start(s: &mut Session, v: &Value) -> CmdResult {
    home_end(s, v, false, p::bool(v, "extend").unwrap_or(false))
}

fn line_end(s: &mut Session, v: &Value) -> CmdResult {
    home_end(s, v, true, p::bool(v, "extend").unwrap_or(false))
}

fn select_line_end(s: &mut Session, v: &Value) -> CmdResult {
    home_end(s, v, true, true)
}

fn word_move(s: &mut Session, v: &Value, forward: bool) -> CmdResult {
    let extend = p::bool(v, "extend").unwrap_or(false);
    let c = caret(s)?;
    let text = para_text(s, c.block, c.para).unwrap_or_default();
    let pos = if forward {
        if c.offset >= text.len() {
            match step_para(s, c.block, c.para, true) {
                Some(n) => Pos::new(c.block, n, 0),
                None => c,
            }
        } else {
            Pos { offset: t::word_right(&text, c.offset), ..c }
        }
    } else if c.offset == 0 {
        match step_para(s, c.block, c.para, false) {
            Some(prev) => Pos::new(c.block, prev, para_text(s, c.block, prev).map(|t| t.len()).unwrap_or(0)),
            None => c,
        }
    } else {
        Pos { offset: t::word_left(&text, c.offset), ..c }
    };
    move_to(s, pos, extend);
    Ok(Value::Null)
}

fn word_left(s: &mut Session, v: &Value) -> CmdResult {
    word_move(s, v, false)
}

fn word_right(s: &mut Session, v: &Value) -> CmdResult {
    word_move(s, v, true)
}

fn para_move(s: &mut Session, v: &Value, forward: bool) -> CmdResult {
    let extend = p::bool(v, "extend").unwrap_or(false);
    let c = caret(s)?;
    let pos = if forward {
        match step_para(s, c.block, c.para, true) {
            Some(n) => Pos::new(c.block, n, 0),
            None => Pos { offset: para_text(s, c.block, c.para).map(|t| t.len()).unwrap_or(0), ..c },
        }
    } else if c.offset > 0 {
        Pos { offset: 0, ..c }
    } else {
        match step_para(s, c.block, c.para, false) {
            Some(prev) => Pos::new(c.block, prev, 0),
            None => c,
        }
    };
    move_to(s, pos, extend);
    Ok(Value::Null)
}

fn next_paragraph(s: &mut Session, v: &Value) -> CmdResult {
    para_move(s, v, true)
}

fn prev_paragraph(s: &mut Session, v: &Value) -> CmdResult {
    para_move(s, v, false)
}

fn select_all(s: &mut Session, _: &Value) -> CmdResult {
    let page = s.page().ok_or_else(no_page)?;
    let all_objects: Vec<_> = page.objects.iter().map(|o| o.id()).collect();
    let step = match (&s.sel.on_page, s.text_range()) {
        (PageSel::Text(_), Some(r)) => {
            let n = para_count(s, r.head.block);
            let (a, b) = r.ordered();
            let para_len = |i: usize| para_text(s, r.head.block, i).map(|t| t.len()).unwrap_or(0);
            let whole_para = a.para == b.para && a.offset == 0 && b.offset == para_len(b.para);
            let whole_block = a.para == 0 && a.offset == 0 && b.para + 1 == n && b.offset == para_len(n.saturating_sub(1));
            if whole_block || (r.head.block == Block::Title && whole_para) {
                // Third press: every container on the page as objects.
                PageSel::Objects { ids: all_objects }
            } else if whole_para {
                // Second press: the whole container.
                let last = n.saturating_sub(1);
                PageSel::Text(crate::TextRange { anchor: Pos::new(r.head.block, 0, 0), head: Pos::new(r.head.block, last, para_len(last)) })
            } else {
                // First press: the current paragraph.
                let i = r.head.para;
                PageSel::Text(crate::TextRange { anchor: Pos::new(r.head.block, i, 0), head: Pos::new(r.head.block, i, para_len(i)) })
            }
        }
        _ => PageSel::Objects { ids: all_objects },
    };
    s.sel.on_page = step;
    s.pending_fmt = None;
    Ok(Value::Null)
}

fn cancel(s: &mut Session, _: &Value) -> CmdResult {
    s.pending_outline = None;
    s.sel.on_page = match (&s.sel.on_page, s.text_range()) {
        (PageSel::Text(_), Some(r)) if !r.is_caret() => PageSel::Text(crate::TextRange::caret(r.head)),
        _ => PageSel::None,
    };
    Ok(Value::Null)
}

fn click_to_type(s: &mut Session, v: &Value) -> CmdResult {
    let x = p::req_f32(v, "x")?;
    let y = p::req_f32(v, "y")?;
    s.pending_outline = Some(notecraft_doc::geom::Point::new(x, y).sanitized());
    s.sel.on_page = PageSel::None;
    s.pending_fmt = None;
    Ok(Value::Null)
}
