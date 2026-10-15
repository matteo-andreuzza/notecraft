//! Shared building blocks for commands: resolving the selection to paragraphs, deleting
//! ranges that span paragraphs, inserting text (creating a note container where needed), and
//! keeping the page tidy.
//!
//! Every function validates positions against the page (they come from the UI, the CLI or an
//! agent and may be stale) and returns `CmdError` instead of panicking.

use notecraft_doc::geom::Point;
use notecraft_doc::{CharFormat, Outline, PageObject, Paragraph, Timestamp};
use notecraft_text as t;

use crate::selection::{Block, PageSel, Pos, TextRange};
use crate::{CmdError, Session};

/// Where a note container is created when text arrives with no caret on the page: the left
/// margin, below the title area and below every existing container (points).
pub const DEFAULT_LEFT: f32 = 36.0;
/// Top of the content area below the title, in points.
pub const CONTENT_TOP: f32 = 86.0;

pub(crate) fn no_page() -> CmdError {
    CmdError::Disabled("no page is open".into())
}

pub(crate) fn need_page(s: &Session) -> Option<&'static str> {
    if s.sel.page.is_some() { None } else { Some("No page is open") }
}

pub(crate) fn need_text(s: &Session) -> Option<&'static str> {
    if s.sel.page.is_none() {
        return Some("No page is open");
    }
    match s.sel.on_page {
        PageSel::Text(_) | PageSel::Objects { .. } | PageSel::Cells { .. } => None,
        PageSel::None => {
            if s.pending_outline.is_some() {
                None
            } else {
                Some("Click in a note first")
            }
        }
    }
}

pub(crate) fn need_selection(s: &Session) -> Option<&'static str> {
    match s.text_range() {
        Some(r) if !r.is_caret() => None,
        _ => match &s.sel.on_page {
            PageSel::Objects { ids } if !ids.is_empty() => None,
            PageSel::Cells { .. } => None,
            _ => Some("Select some text first"),
        },
    }
}

/// The paragraphs a paragraph-level command applies to: `(block, index range)`. A text range
/// covers the paragraphs it touches; selected containers cover all their paragraphs.
pub(crate) fn touched(s: &Session) -> Vec<(Block, std::ops::Range<usize>)> {
    let Some(page) = s.page() else { return Vec::new() };
    match &s.sel.on_page {
        PageSel::Text(_) => match s.text_range() {
            Some(r) => {
                let (a, b) = r.ordered();
                vec![(a.block, a.para..b.para + 1)]
            }
            None => Vec::new(),
        },
        PageSel::Objects { ids } => ids.iter().filter_map(|id| page.outline(*id).map(|o| (Block::Outline(*id), 0..o.paragraphs.len()))).collect(),
        PageSel::Cells { .. } => selected_cells(s).into_iter().map(|(id, n)| (Block::Cell(id), 0..n)).collect(),
        PageSel::None => Vec::new(),
    }
}

/// The cells of a cell selection, in reading order: `(cell id, paragraph count)`.
pub(crate) fn selected_cells(s: &Session) -> Vec<(notecraft_doc::Id, usize)> {
    let PageSel::Cells { table, rows, cols } = s.sel.on_page else { return Vec::new() };
    let Some(t) = s.page().and_then(|p| p.table(table)) else { return Vec::new() };
    t.rows
        .iter()
        .skip(rows.0)
        .take(rows.1.saturating_sub(rows.0).saturating_add(1))
        .flat_map(|r| r.cells.iter().skip(cols.0).take(cols.1.saturating_sub(cols.0).saturating_add(1)))
        .map(|c| (c.id, c.paragraphs.len()))
        .collect()
}

/// Character ranges a character-format command applies to: per paragraph, the byte range.
pub(crate) fn segments(s: &Session) -> Vec<(Block, usize, std::ops::Range<usize>)> {
    let Some(page) = s.page() else { return Vec::new() };
    match &s.sel.on_page {
        PageSel::Text(_) => {
            let Some(r) = s.text_range() else { return Vec::new() };
            let (a, b) = r.ordered();
            let Some(paras) = Session::block_paras(page, a.block) else { return Vec::new() };
            let mut out = Vec::new();
            for i in a.para..=b.para {
                let Some(p) = paras.get(i) else { break };
                let start = if i == a.para { a.offset } else { 0 };
                let end = if i == b.para { b.offset } else { p.len() };
                out.push((a.block, i, start..end));
            }
            out
        }
        PageSel::Objects { ids } => ids
            .iter()
            .filter_map(|id| page.outline(*id).map(|o| (*id, o)))
            .flat_map(|(id, o)| o.paragraphs.iter().enumerate().map(move |(i, p)| (Block::Outline(id), i, 0..p.len())))
            .collect(),
        PageSel::Cells { .. } => selected_cells(s)
            .into_iter()
            .filter_map(|(id, _)| page.cell(id).map(|c| (id, c)))
            .flat_map(|(id, c)| c.paragraphs.iter().enumerate().map(move |(i, p)| (Block::Cell(id), i, 0..p.len())))
            .collect(),
        PageSel::None => Vec::new(),
    }
}

/// Stamp `modified`/`author` on a paragraph and its page.
pub(crate) fn touch(s: &mut Session, block: Block, para: usize) {
    let now = s.now();
    let author = s.author();
    if let Some(page) = s.page_mut() {
        page.modified = now;
        if let Some(p) = Session::para_mut(page, block, para) {
            p.modified = now;
            p.author = author;
        }
    }
}

/// Delete the text range `r` (may span paragraphs of one block). Returns the caret position
/// after the deletion. Paragraphs between the ends are removed; the end paragraph's remainder
/// joins the start paragraph.
pub(crate) fn delete_range(s: &mut Session, r: TextRange) -> Result<Pos, CmdError> {
    let (a, b) = r.ordered();
    let page = s.page_mut().ok_or_else(no_page)?;
    if a.para == b.para {
        let p = Session::para_mut(page, a.block, a.para).ok_or_else(|| CmdError::Failed("paragraph not found".into()))?;
        t::delete(p, a.offset..b.offset);
        return Ok(a);
    }
    let paras = Session::block_paras_mut(page, a.block).ok_or_else(|| CmdError::Failed("text block not found".into()))?;
    if b.para >= paras.len() || a.para >= b.para {
        return Err(CmdError::Failed("selection out of range".into()));
    }
    // Trim the tail of the start paragraph and the head of the end paragraph.
    if let Some(p) = paras.get_mut(a.para) {
        let len = p.len();
        t::delete(p, a.offset..len);
    }
    let mut last = paras.remove(b.para);
    t::delete(&mut last, 0..b.offset);
    paras.drain(a.para + 1..b.para);
    if let Some(p) = paras.get_mut(a.para) {
        t::merge(p, last);
    }
    Ok(a)
}

/// Make sure there is a caret on the page: use the text selection's head, or materialise the
/// pending click position into a new container, or (for agents with no caret at all) append a
/// container below the existing content. Returns the caret.
pub(crate) fn ensure_caret(s: &mut Session) -> Result<Pos, CmdError> {
    if let Some(r) = s.text_range() {
        if !r.is_caret() {
            let p = delete_range(s, r)?;
            s.set_caret(p);
            return Ok(p);
        }
        return Ok(r.head);
    }
    let pos = match s.pending_outline.take() {
        Some(p) => p,
        None => {
            let page = s.page().ok_or_else(no_page)?;
            Point::new(DEFAULT_LEFT, next_free_y(page))
        }
    };
    let oid = s.new_id();
    let pid = s.new_id();
    let mut outline = Outline::new(oid, pos, pid);
    if let Some(p) = outline.paragraphs.get_mut(0) {
        p.author = s.author();
        p.modified = s.now();
    }
    let page = s.page_mut().ok_or_else(no_page)?;
    page.objects.push(PageObject::Outline(outline));
    let caret = Pos::new(Block::Outline(oid), 0, 0);
    s.set_caret(caret);
    Ok(caret)
}

/// The y position below every container on the page (a rough estimate: 20 pt per paragraph,
/// since the engine has no layout; good enough to keep new containers from overlapping).
pub(crate) fn next_free_y(page: &notecraft_doc::Page) -> f32 {
    let mut y = CONTENT_TOP;
    for o in &page.objects {
        y = y.max(object_bottom(o) + 24.0);
    }
    y
}

fn para_lines(p: &Paragraph) -> usize {
    1 + p.text().matches('\n').count()
}

/// The estimated bottom of an object in page points (20 pt per line, no layout in the engine).
pub(crate) fn object_bottom(o: &PageObject) -> f32 {
    match o {
        PageObject::Outline(ol) => ol.pos.y + ol.paragraphs.iter().map(para_lines).sum::<usize>() as f32 * 20.0,
        PageObject::Table(t) => {
            let rows: usize =
                t.rows.iter().map(|r| r.cells.iter().map(|c| c.paragraphs.iter().map(para_lines).sum::<usize>()).max().unwrap_or(1)).sum();
            t.pos.y + rows as f32 * 20.0 + t.rows.len() as f32 * 6.0
        }
    }
}

/// The format typed text gets at `pos`: pending formatting if set, otherwise the run on the
/// left — except a hyperlink, which only continues when typing strictly inside it.
pub(crate) fn typing_fmt(s: &Session, pos: Pos) -> CharFormat {
    if let Some(f) = &s.pending_fmt {
        return f.clone();
    }
    let Some(page) = s.page() else { return CharFormat::default() };
    let Some(p) = Session::block_paras(page, pos.block).and_then(|ps| ps.get(pos.para)) else { return CharFormat::default() };
    let mut f = p.fmt_at(pos.offset);
    if f.link.is_some() {
        let inside_after = p.fmt_at(t::next_grapheme(&p.text(), pos.offset)).link == f.link && pos.offset < p.len();
        let at_start = pos.offset == 0;
        if !inside_after || at_start {
            f.link = None;
        }
    }
    f
}

/// Insert plain `text` at the caret (single paragraph; `'\n'` are soft breaks). Returns the new
/// caret.
pub(crate) fn insert_at(s: &mut Session, pos: Pos, text: &str, fmt: &CharFormat) -> Result<Pos, CmdError> {
    let text = if pos.block == Block::Title { text.replace('\n', " ") } else { text.to_string() };
    let page = s.page_mut().ok_or_else(no_page)?;
    let p = Session::para_mut(page, pos.block, pos.para).ok_or_else(|| CmdError::Failed("paragraph not found".into()))?;
    let end = t::insert(p, pos.offset, &text, fmt);
    Ok(Pos { offset: end, ..pos })
}

/// Remove empty note containers other than the one holding the caret (catalog
/// `BEH.Canvas.EmptyContainerRemoved`). Returns whether anything was removed.
pub(crate) fn prune_empty_outlines(s: &mut Session) -> bool {
    let keep = s.sel.caret().and_then(|c| match c.block {
        Block::Outline(id) => Some(id),
        Block::Title | Block::Cell(_) => None,
    });
    let has_empty = s.page().is_some_and(|p| p.objects.iter().any(|o| o.as_outline().is_some_and(|ol| ol.is_empty() && Some(ol.id) != keep)));
    if !has_empty {
        return false;
    }
    if let Some(page) = s.page_mut() {
        page.objects.retain(|o| !o.as_outline().is_some_and(|ol| ol.is_empty() && Some(ol.id) != keep));
    }
    if let PageSel::Objects { ids } = &mut s.sel.on_page {
        let page = s.ws.page(s.sel.page.unwrap_or_default());
        ids.retain(|id| page.is_some_and(|p| p.object_index(*id).is_some()));
    }
    true
}

/// A fresh paragraph id stamped with author and time.
pub(crate) fn new_para(s: &mut Session) -> Paragraph {
    let id = s.new_id();
    let mut p = Paragraph::new(id);
    p.author = s.author();
    p.modified = s.now();
    p
}

/// Deep-copy a page with fresh ids for the page and everything on it (Copy page, Move or Copy
/// with Copy).
pub(crate) fn clone_page_fresh(s: &mut Session, page: &notecraft_doc::Page, now: Timestamp) -> notecraft_doc::Page {
    let mut p = page.clone();
    p.id = s.new_id();
    p.title.id = s.new_id();
    p.created = now;
    p.modified = now;
    for o in &mut p.objects {
        match o {
            PageObject::Outline(ol) => {
                ol.id = s.new_id();
                for para in &mut ol.paragraphs {
                    para.id = s.new_id();
                }
            }
            PageObject::Table(t) => {
                t.id = s.new_id();
                for row in &mut t.rows {
                    row.id = s.new_id();
                    for cell in &mut row.cells {
                        cell.id = s.new_id();
                        for para in &mut cell.paragraphs {
                            para.id = s.new_id();
                        }
                    }
                }
            }
        }
    }
    p
}
