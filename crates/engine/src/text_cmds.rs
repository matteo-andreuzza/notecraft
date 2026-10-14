//! Typing: inserting text, Enter, Shift+Enter, Backspace, Delete and word deletion.
//!
//! These are the commands the keyboard drives while the caret is in a note. Their behaviour
//! follows the reference (catalog rows cited per command); where the reference's exact rule is
//! undocumented the code says `unverified`.

use notecraft_doc::{ListFormat, PageObject, Paragraph};
use notecraft_text as t;
use serde_json::{Value, json};

use crate::ops::{self, ensure_caret, insert_at, need_page, no_page, typing_fmt};
use crate::selection::{Block, Pos};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // NC.Text.Insert — typing. One undo step per word: consecutive inserts in one paragraph
        // coalesce until a whitespace character ends the word (catalog BEH.Text.Undo).
        CommandSpec::new("text.insert", "NC.Text.Insert", "Type Text", "Keyboard/Typing/Type text", insert)
            .params(r#"{"text": string}"#)
            .when(need_page),
        // NC.Text.Enter — Enter: new paragraph (BEH.Outline.EnterNewParagraph, EmptyBulletEnds).
        CommandSpec::new("text.newline", "NC.Text.Enter", "New Paragraph", "Keyboard/Typing/New paragraph", newline).key("Enter").when(need_page),
        // KB.General.LineBreak — Shift+Enter: soft line break inside the paragraph.
        CommandSpec::new(
            "text.lineBreak",
            "KB.General.LineBreak",
            "Insert a line break without starting a new paragraph",
            "Keyboard/General/Insert a line break without starting a new paragraph",
            line_break,
        )
        .key("Shift+Enter")
        .when(need_page),
        // KB.General.Backspace
        CommandSpec::new(
            "text.backspace",
            "KB.General.Backspace",
            "Delete one character to the left",
            "Keyboard/General/Delete one character to the left",
            backspace,
        )
        .key("Backspace")
        .when(need_page),
        // KB.General.DeleteChar — also deletes a selection or selected containers.
        CommandSpec::new(
            "text.deleteForward",
            "KB.General.DeleteChar",
            "Delete one character to the right",
            "Keyboard/General/Delete one character to the right",
            delete_forward,
        )
        .key("Delete")
        .when(need_page),
        // KB.General.DeleteWordLeft
        CommandSpec::new(
            "text.deleteWordLeft",
            "KB.General.DeleteWordLeft",
            "Delete one word to the left",
            "Keyboard/General/Delete one word to the left",
            delete_word_left,
        )
        .key("Mod+Backspace")
        .when(need_page),
        // KB.General.DeleteWordRight
        CommandSpec::new(
            "text.deleteWordRight",
            "KB.General.DeleteWordRight",
            "Delete one word to the right",
            "Keyboard/General/Delete one word to the right",
            delete_word_right,
        )
        .key("Mod+Delete")
        .when(need_page),
    ]
}

fn coalesce_key(pos: Pos, page: Option<notecraft_doc::Id>) -> String {
    format!("type:{}:{:?}:{}", page.unwrap_or_default(), pos.block, pos.para)
}

/// Type `text` at the caret. Newlines start new paragraphs (as pressing Enter would).
fn insert(s: &mut Session, v: &Value) -> CmdResult {
    let text = p::req_str(v, "text")?;
    if text.len() > 16 << 20 {
        return Err(CmdError::Params("text is larger than 16 MiB".into()));
    }
    let mut caret = ensure_caret(s)?;
    let lines: Vec<&str> = text.split('\n').collect();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            caret = split_at_caret(s, caret)?;
        }
        if line.is_empty() {
            continue;
        }
        let fmt = typing_fmt(s, caret);
        caret = insert_at(s, caret, line, &fmt)?;
        s.set_caret(caret);
        // Keep the pending format for the next keystroke of this burst.
        if let Some(f) = Some(fmt).filter(|f| !f.is_plain()) {
            s.pending_fmt = Some(f);
        }
        caret = after_typing(s, caret, line)?;
    }
    s.set_caret(caret);
    ops::touch(s, caret.block, caret.para);
    let ends_word = text.ends_with(char::is_whitespace);
    let key = coalesce_key(caret, s.sel.page);
    s.coalesce = Some(if ends_word { format!("{key}|end") } else { key });
    if lines.len() > 1 {
        s.coalesce = None;
    }
    Ok(Value::Null)
}

/// Auto-formatting that triggers on what was just typed: list prefixes (`* `, `1. `), URLs
/// before a space, and `[[wiki links]]`.
fn after_typing(s: &mut Session, caret: Pos, typed: &str) -> Result<Pos, CmdError> {
    if caret.block == Block::Title {
        return Ok(caret);
    }
    let Some(page) = s.page() else { return Ok(caret) };
    let Some(para) = Session::block_paras(page, caret.block).and_then(|ps| ps.get(caret.para)) else { return Ok(caret) };
    let text = para.text();
    // BEH.Outline.AutoBullet / AutoNumber: the prefix must be the whole text before the caret.
    if typed.ends_with(' ')
        && para.list.is_none()
        && let Some((fmt, n)) = t::detect_list_prefix(&text)
        && n == caret.offset
    {
        let allowed = match fmt {
            ListFormat::Bullet(_) => s.prefs.auto_bullets,
            ListFormat::Number { .. } => s.prefs.auto_numbering,
        };
        if allowed {
            let page = s.page_mut().ok_or_else(no_page)?;
            if let Some(pm) = Session::para_mut(page, caret.block, caret.para) {
                t::delete(pm, 0..n);
                pm.list = Some(fmt);
            }
            return Ok(Pos { offset: 0, ..caret });
        }
    }
    // BEH.Text.HyperlinkAuto: a URL followed by whitespace becomes a link.
    if typed.ends_with(char::is_whitespace) {
        let before = caret.offset.saturating_sub(typed.len());
        if let Some(r) = t::detect_url_before(&text, before)
            && para.fmt_at(r.end).link.is_none()
        {
            let url = t::url_target(text.get(r.clone()).unwrap_or(""));
            let page = s.page_mut().ok_or_else(no_page)?;
            if let Some(pm) = Session::para_mut(page, caret.block, caret.para) {
                t::apply_format(pm, r, &|f| f.link = Some(url.clone()));
            }
        }
    }
    // BEH.Links.Wiki: "[[Page name]]" links to that page, creating it in this section.
    if s.prefs.wiki_links && text.get(..caret.offset).is_some_and(|h| h.ends_with("]]")) {
        let head = text.get(..caret.offset).unwrap_or("");
        if let Some(open) = head.rfind("[[") {
            let name = head.get(open + 2..head.len() - 2).unwrap_or("").trim().to_string();
            if !name.is_empty() && !name.contains("[[") && name.chars().count() <= 255 {
                let target = crate::page_cmds::page_by_title_or_create(s, &name)?;
                let link = format!("notecraft:page/{target}");
                let page = s.page_mut().ok_or_else(no_page)?;
                if let Some(pm) = Session::para_mut(page, caret.block, caret.para) {
                    // Remove the brackets, then link the name.
                    t::delete(pm, caret.offset - 2..caret.offset);
                    t::delete(pm, open..open + 2);
                    let end = caret.offset - 4;
                    t::apply_format(pm, open..end, &|f| f.link = Some(link.clone()));
                    return Ok(Pos { offset: end, ..caret });
                }
            }
        }
    }
    Ok(caret)
}

/// Split the caret paragraph (Enter) and return the caret at the start of the new paragraph.
fn split_at_caret(s: &mut Session, caret: Pos) -> Result<Pos, CmdError> {
    let new_id = s.new_id();
    let author = s.author();
    let now = s.now();
    let page = s.page_mut().ok_or_else(no_page)?;
    let paras = Session::block_paras_mut(page, caret.block).ok_or_else(|| CmdError::Failed("text block not found".into()))?;
    let cur = paras.get_mut(caret.para).ok_or_else(|| CmdError::Failed("paragraph not found".into()))?;
    let mut q = t::split(cur, caret.offset, new_id);
    q.author = author;
    q.modified = now;
    // A collapsed paragraph keeps its hidden children; the new paragraph goes after them.
    let insert_at = if cur.collapsed {
        let lvl = cur.level;
        let mut j = caret.para + 1;
        while paras.get(j).is_some_and(|x| x.level > lvl) {
            j += 1;
        }
        j
    } else {
        caret.para + 1
    };
    paras.insert(insert_at, q);
    Ok(Pos::new(caret.block, insert_at, 0))
}

/// Enter.
fn newline(s: &mut Session, _: &Value) -> CmdResult {
    // In the title, Enter moves to the page body (the first container, created if needed).
    if s.sel.caret().is_some_and(|c| c.block == Block::Title) {
        let first = s.page().and_then(|p| p.objects.iter().find_map(PageObject::as_outline).map(|o| o.id));
        let pos = match first {
            Some(id) => Pos::new(Block::Outline(id), 0, 0),
            None => {
                s.sel.on_page = crate::selection::PageSel::None;
                ensure_caret(s)?
            }
        };
        s.set_caret(pos);
        return Ok(Value::Null);
    }
    let caret = ensure_caret(s)?;
    let page = s.page().ok_or_else(no_page)?;
    let para =
        Session::block_paras(page, caret.block).and_then(|ps| ps.get(caret.para)).ok_or_else(|| CmdError::Failed("paragraph not found".into()))?;
    // BEH.Outline.EmptyBulletEnds: Enter on an empty list item outdents it, then ends the list;
    // on an empty to-do line it removes the checkbox (unverified exact sequence).
    if para.is_empty() && (para.list.is_some() || para.tags.iter().any(|t| t.symbol.is_checkbox())) {
        let level = para.level;
        let page = s.page_mut().ok_or_else(no_page)?;
        if let Some(pm) = Session::para_mut(page, caret.block, caret.para) {
            if pm.list.is_some() && level > 0 {
                pm.level -= 1;
            } else {
                pm.list = None;
                pm.tags.retain(|t| !t.symbol.is_checkbox());
            }
        }
        return Ok(Value::Null);
    }
    // A URL right before the caret becomes a link on Enter too.
    let text = para.text();
    if let Some(r) = t::detect_url_before(&text, caret.offset)
        && para.fmt_at(r.end).link.is_none()
    {
        let url = t::url_target(text.get(r.clone()).unwrap_or(""));
        let page = s.page_mut().ok_or_else(no_page)?;
        if let Some(pm) = Session::para_mut(page, caret.block, caret.para) {
            t::apply_format(pm, r, &|f| f.link = Some(url.clone()));
        }
    }
    let pos = split_at_caret(s, caret)?;
    s.set_caret(pos);
    ops::touch(s, pos.block, pos.para);
    Ok(json!({"para": pos.para}))
}

fn line_break(s: &mut Session, _: &Value) -> CmdResult {
    let caret = ensure_caret(s)?;
    if caret.block == Block::Title {
        return Ok(Value::Null);
    }
    let fmt = typing_fmt(s, caret);
    let pos = insert_at(s, caret, "\n", &fmt)?;
    s.set_caret(pos);
    Ok(Value::Null)
}

/// Delete the selection (text range or selected containers); `Ok(true)` if there was one.
pub(crate) fn delete_selection(s: &mut Session) -> Result<bool, CmdError> {
    if let crate::selection::PageSel::Objects { ids } = s.sel.on_page.clone() {
        if ids.is_empty() {
            return Ok(false);
        }
        let page = s.page_mut().ok_or_else(no_page)?;
        page.objects.retain(|o| !ids.contains(&o.id()));
        s.sel.on_page = crate::selection::PageSel::None;
        return Ok(true);
    }
    match s.text_range() {
        Some(r) if !r.is_caret() => {
            let pos = ops::delete_range(s, r)?;
            s.set_caret(pos);
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn backspace(s: &mut Session, _: &Value) -> CmdResult {
    if delete_selection(s)? {
        return Ok(Value::Null);
    }
    let Some(caret) = s.text_range().map(|r| r.head) else { return Ok(Value::Null) };
    let page = s.page().ok_or_else(no_page)?;
    let paras = Session::block_paras(page, caret.block).ok_or_else(|| CmdError::Failed("text block not found".into()))?;
    let para = paras.get(caret.para).ok_or_else(|| CmdError::Failed("paragraph not found".into()))?;
    if caret.offset > 0 {
        let start = t::prev_grapheme(&para.text(), caret.offset);
        let page = s.page_mut().ok_or_else(no_page)?;
        if let Some(pm) = Session::para_mut(page, caret.block, caret.para) {
            t::delete(pm, start..caret.offset);
        }
        let pos = Pos { offset: start, ..caret };
        s.set_caret(pos);
        s.coalesce = Some(format!("bksp:{:?}:{}", caret.block, caret.para));
        return Ok(Value::Null);
    }
    // At the start of a paragraph: remove its bullet first, then its indent, then join it to
    // the previous paragraph (word-processor convention; unverified for the reference).
    let (has_list, level) = (para.list.is_some(), para.level);
    let page = s.page_mut().ok_or_else(no_page)?;
    if has_list || level > 0 {
        if let Some(pm) = Session::para_mut(page, caret.block, caret.para) {
            if has_list {
                pm.list = None;
            } else {
                pm.level -= 1;
            }
        }
        return Ok(Value::Null);
    }
    if caret.para == 0 {
        return Ok(Value::Null);
    }
    let paras = Session::block_paras_mut(page, caret.block).ok_or_else(|| CmdError::Failed("text block not found".into()))?;
    let cur = paras.remove(caret.para);
    let prev = paras.get_mut(caret.para - 1).ok_or_else(|| CmdError::Failed("paragraph not found".into()))?;
    let join = t::merge(prev, cur);
    s.set_caret(Pos::new(caret.block, caret.para - 1, join));
    Ok(Value::Null)
}

fn delete_forward(s: &mut Session, _: &Value) -> CmdResult {
    if delete_selection(s)? {
        return Ok(Value::Null);
    }
    let Some(caret) = s.text_range().map(|r| r.head) else { return Ok(Value::Null) };
    let page = s.page().ok_or_else(no_page)?;
    let paras = Session::block_paras(page, caret.block).ok_or_else(|| CmdError::Failed("text block not found".into()))?;
    let para = paras.get(caret.para).ok_or_else(|| CmdError::Failed("paragraph not found".into()))?;
    let len = para.len();
    let n_paras = paras.len();
    if caret.offset < len {
        let end = t::next_grapheme(&para.text(), caret.offset);
        let page = s.page_mut().ok_or_else(no_page)?;
        if let Some(pm) = Session::para_mut(page, caret.block, caret.para) {
            t::delete(pm, caret.offset..end);
        }
        s.coalesce = Some(format!("del:{:?}:{}", caret.block, caret.para));
        return Ok(Value::Null);
    }
    if caret.para + 1 >= n_paras {
        return Ok(Value::Null);
    }
    let page = s.page_mut().ok_or_else(no_page)?;
    let paras = Session::block_paras_mut(page, caret.block).ok_or_else(|| CmdError::Failed("text block not found".into()))?;
    let next: Paragraph = paras.remove(caret.para + 1);
    if let Some(cur) = paras.get_mut(caret.para) {
        t::merge(cur, next);
    }
    Ok(Value::Null)
}

fn delete_word(s: &mut Session, left: bool) -> CmdResult {
    if delete_selection(s)? {
        return Ok(Value::Null);
    }
    let Some(caret) = s.text_range().map(|r| r.head) else { return Ok(Value::Null) };
    let page = s.page().ok_or_else(no_page)?;
    let text = Session::block_paras(page, caret.block)
        .and_then(|ps| ps.get(caret.para))
        .map(Paragraph::text)
        .ok_or_else(|| CmdError::Failed("paragraph not found".into()))?;
    let (a, b) = if left { (t::word_left(&text, caret.offset), caret.offset) } else { (caret.offset, t::word_right(&text, caret.offset)) };
    if a == b {
        // At a paragraph edge: behave like Backspace/Delete (join paragraphs).
        return if left { backspace(s, &Value::Null) } else { delete_forward(s, &Value::Null) };
    }
    let page = s.page_mut().ok_or_else(no_page)?;
    if let Some(pm) = Session::para_mut(page, caret.block, caret.para) {
        t::delete(pm, a..b);
    }
    s.set_caret(Pos { offset: a, ..caret });
    Ok(Value::Null)
}

fn delete_word_left(s: &mut Session, _: &Value) -> CmdResult {
    delete_word(s, true)
}

fn delete_word_right(s: &mut Session, _: &Value) -> CmdResult {
    delete_word(s, false)
}
