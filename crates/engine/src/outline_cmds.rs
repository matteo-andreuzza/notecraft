//! Outlines: show levels, expand/collapse, move paragraphs with their children, move and resize
//! note containers.

use notecraft_doc::OutlineWidth;
use notecraft_doc::geom::Point;
use serde_json::Value;

use crate::ops::{need_page, need_text, no_page};
use crate::selection::{Block, PageSel, Pos, TextRange};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn need_outline_caret(s: &Session) -> Option<&'static str> {
    if let Some(w) = need_text(s) {
        return Some(w);
    }
    match s.sel.caret() {
        Some(Pos { block: Block::Outline(_), .. }) => None,
        _ => Some("Click in a note container first"),
    }
}

macro_rules! level_cmd {
    ($id:literal, $cat:literal, $label:literal, $path:literal, $key:literal, $n:expr) => {
        CommandSpec::new($id, $cat, $label, $path, |s, _| show_level(s, $n)).key($key).when(need_page)
    };
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // KB.Outline.Level1..9 — Alt+Shift+1…9 show the outline through that level.
        level_cmd!("outline.level1", "KB.Outline.Level1", "Show Level 1", "Keyboard/Outlines/Collapse the outline to level 1", "Alt+Shift+1", 1),
        level_cmd!("outline.level2", "KB.Outline.Level2", "Show Level 2", "Keyboard/Outlines/Expand the outline to level 2", "Alt+Shift+2", 2),
        level_cmd!("outline.level3", "KB.Outline.Level3", "Show Level 3", "Keyboard/Outlines/Expand the outline to level 3", "Alt+Shift+3", 3),
        level_cmd!("outline.level4", "KB.Outline.Level4", "Show Level 4", "Keyboard/Outlines/Expand the outline to level 4", "Alt+Shift+4", 4),
        level_cmd!("outline.level5", "KB.Outline.Level5", "Show Level 5", "Keyboard/Outlines/Expand the outline to level 5", "Alt+Shift+5", 5),
        level_cmd!("outline.level6", "KB.Outline.Level6", "Show Level 6", "Keyboard/Outlines/Expand the outline to level 6", "Alt+Shift+6", 6),
        level_cmd!("outline.level7", "KB.Outline.Level7", "Show Level 7", "Keyboard/Outlines/Expand the outline to level 7", "Alt+Shift+7", 7),
        level_cmd!("outline.level8", "KB.Outline.Level8", "Show Level 8", "Keyboard/Outlines/Expand the outline to level 8", "Alt+Shift+8", 8),
        level_cmd!("outline.level9", "KB.Outline.Level9", "Show Level 9", "Keyboard/Outlines/Expand the outline to level 9", "Alt+Shift+9", 9),
        CommandSpec::new("outline.expandAll", "KB.Outline.ExpandAll", "Expand All", "Keyboard/Outlines/Expand all levels", |s, _| {
            show_level(s, u8::MAX)
        })
        .key("Alt+Shift+0")
        .when(need_page),
        // KB.Outline.ExpandHeading / CollapseHeading (BEH.Outline.CollapseExpand).
        CommandSpec::new("outline.expand", "KB.Outline.ExpandHeading", "Expand", "Keyboard/Outlines/Expand the selected heading", |s, _| {
            set_collapsed(s, false)
        })
        .key("Alt+Shift+= / Alt+Shift+Plus")
        .when(need_outline_caret),
        CommandSpec::new("outline.collapse", "KB.Outline.CollapseHeading", "Collapse", "Keyboard/Outlines/Collapse the selected heading", |s, _| {
            set_collapsed(s, true)
        })
        .key("Alt+Shift+-")
        .when(need_outline_caret),
        // NC.Outline.ToggleCollapse — double-click on a paragraph handle.
        CommandSpec::new("outline.toggleCollapse", "BEH.Outline.CollapseExpand", "Toggle Collapse", "Behaviour/Outline", toggle_collapse)
            .params(r#"{"block"?: outline id, "para"?: int}"#)
            .when(need_page),
        // KB.Select.MoveUp / MoveDown — move paragraphs with their children (BEH.Outline.MoveParagraph).
        CommandSpec::new("outline.moveUp", "KB.Select.MoveUp", "Move Up", "Keyboard/Select/Move the selected paragraphs up", |s, _| {
            move_paras(s, true)
        })
        .key("Alt+Shift+Up")
        .when(need_outline_caret),
        CommandSpec::new("outline.moveDown", "KB.Select.MoveDown", "Move Down", "Keyboard/Select/Move the selected paragraphs down", |s, _| {
            move_paras(s, false)
        })
        .key("Alt+Shift+Down")
        .when(need_outline_caret),
        // KB.Select.Subordinates — the paragraph and its children.
        CommandSpec::new(
            "outline.selectBranch",
            "KB.Select.Subordinates",
            "Select Paragraph and Subordinates",
            "Keyboard/Select/Select the current paragraph and its subordinate paragraphs",
            select_branch,
        )
        .key("Mod+Shift+-")
        .pure()
        .when(need_outline_caret),
        // BEH.Canvas.ContainerMove / ContainerResize.
        CommandSpec::new("outline.move", "BEH.Canvas.ContainerMove", "Move Note Container", "Behaviour/Canvas", move_outline)
            .params(r#"{"id": outline id, "x": pt, "y": pt}"#)
            .when(need_page),
        CommandSpec::new("outline.resize", "BEH.Canvas.ContainerResize", "Resize Note Container", "Behaviour/Canvas", resize_outline)
            .params(r#"{"id": outline id, "width": pt | null (auto)}"#)
            .when(need_page),
        // BEH.Outline.DragParagraph — drop a paragraph (with children) elsewhere.
        CommandSpec::new("outline.moveParagraph", "BEH.Outline.DragParagraph", "Move Paragraph", "Behaviour/Outline", move_paragraph)
            .params(r#"{"from": outline id, "para": int, "to": outline id, "index": int, "level"?: int} | {"from", "para", "x": pt, "y": pt} (a new container there)"#)
            .when(need_page),
        // NC.Object.Select — click a container's top bar.
        CommandSpec::new("object.select", "NC.Object.Select", "Select Note Container", "Mouse/Select container", select_object)
            .params(r#"{"ids": [outline id], "add"?: bool}"#)
            .pure()
            .when(need_page),
    ]
}

/// The outlines a whole-outline command applies to: the caret's, the selected containers, or
/// every container on the page.
fn outlines_in_scope(s: &Session) -> Vec<notecraft_doc::Id> {
    match (&s.sel.on_page, s.sel.caret()) {
        (PageSel::Text(_), Some(Pos { block: Block::Outline(id), .. })) => vec![id],
        (PageSel::Objects { ids }, _) if !ids.is_empty() => ids.clone(),
        _ => s.page().map(|p| p.objects.iter().filter_map(|o| o.as_outline().map(|ol| ol.id)).collect()).unwrap_or_default(),
    }
}

fn show_level(s: &mut Session, n: u8) -> CmdResult {
    let ids = outlines_in_scope(s);
    let page = s.page_mut().ok_or_else(no_page)?;
    for id in ids {
        let Some(o) = page.outline_mut(id) else { continue };
        let len = o.paragraphs.len();
        for i in 0..len {
            let has_children = o.subtree(i).len() > 1;
            if let Some(p) = o.paragraphs.get_mut(i) {
                // Paragraphs at level n-1 hide their children; everything above opens.
                p.collapsed = n != u8::MAX && has_children && p.level + 1 >= n;
            }
        }
    }
    // A caret inside a now-hidden paragraph moves up to its visible ancestor.
    if let Some(c) = s.sel.caret()
        && let Block::Outline(id) = c.block
        && let Some(o) = s.page().and_then(|p| p.outline(id))
        && o.is_hidden(c.para)
    {
        let mut j = c.para;
        while j > 0 && o.is_hidden(j) {
            j -= 1;
        }
        s.set_caret(Pos::new(c.block, j, 0));
    }
    Ok(Value::Null)
}

fn set_collapsed(s: &mut Session, on: bool) -> CmdResult {
    let c = s.sel.caret().ok_or_else(|| CmdError::Disabled("click in a note first".into()))?;
    let Block::Outline(id) = c.block else { return Ok(Value::Null) };
    let page = s.page_mut().ok_or_else(no_page)?;
    if let Some(o) = page.outline_mut(id) {
        let has_children = o.subtree(c.para).len() > 1;
        if let Some(p) = o.paragraphs.get_mut(c.para) {
            p.collapsed = on && has_children;
        }
    }
    Ok(Value::Null)
}

fn toggle_collapse(s: &mut Session, v: &Value) -> CmdResult {
    let (id, para) = match (p::id(v, "block")?, p::usize(v, "para")) {
        (Some(id), Some(i)) => (id, i),
        _ => match s.sel.caret() {
            Some(Pos { block: Block::Outline(id), para, .. }) => (id, para),
            _ => return Err(CmdError::Params("`block` and `para` are required".into())),
        },
    };
    let page = s.page_mut().ok_or_else(no_page)?;
    let o = page.outline_mut(id).ok_or_else(|| CmdError::Params("no such note container".into()))?;
    if o.subtree(para).len() > 1
        && let Some(p) = o.paragraphs.get_mut(para)
    {
        p.collapsed = !p.collapsed;
    }
    Ok(Value::Null)
}

/// Move the selected paragraphs (with their children) past the previous/next sibling subtree.
fn move_paras(s: &mut Session, up: bool) -> CmdResult {
    let r = s.text_range().ok_or_else(|| CmdError::Disabled("click in a note first".into()))?;
    let Block::Outline(id) = r.head.block else { return Ok(Value::Null) };
    let (a, b) = r.ordered();
    let page = s.page_mut().ok_or_else(no_page)?;
    let o = page.outline_mut(id).ok_or_else(|| CmdError::Failed("note container not found".into()))?;
    let start = a.para;
    let end = o.subtree(b.para).end.max(b.para + 1);
    let lvl = o.paragraphs.get(start).map(|p| p.level).unwrap_or(0);
    let moved: isize = if up {
        // The previous sibling: the nearest earlier paragraph at level <= lvl. When the block is
        // the first child of its parent, that is the parent: the block moves above it and
        // `normalize` pulls its level up to fit (unverified: the reference's exact rule for
        // crossing a parent).
        let Some(prev) = (0..start).rev().find(|&j| o.paragraphs.get(j).is_some_and(|p| p.level <= lvl)) else { return Ok(Value::Null) };
        let block: Vec<_> = o.paragraphs.drain(start..end).collect();
        for (k, p) in block.into_iter().enumerate() {
            o.paragraphs.insert(prev + k, p);
        }
        prev as isize - start as isize
    } else {
        // The next sibling subtree after the block.
        if end >= o.paragraphs.len() {
            return Ok(Value::Null);
        }
        let next_end = o.subtree(end).end;
        let block: Vec<_> = o.paragraphs.drain(start..end).collect();
        let n = block.len();
        let insert_at = next_end - n;
        for (k, p) in block.into_iter().enumerate() {
            o.paragraphs.insert(insert_at + k, p);
        }
        insert_at as isize - start as isize
    };
    o.normalize();
    let shift = |x: usize| (x as isize + moved).max(0) as usize;
    s.sel.on_page =
        PageSel::Text(TextRange { anchor: Pos { para: shift(r.anchor.para), ..r.anchor }, head: Pos { para: shift(r.head.para), ..r.head } });
    Ok(Value::Null)
}

fn select_branch(s: &mut Session, _: &Value) -> CmdResult {
    let c = s.sel.caret().ok_or_else(|| CmdError::Disabled("click in a note first".into()))?;
    let Block::Outline(id) = c.block else { return Ok(Value::Null) };
    let o = s.page().and_then(|p| p.outline(id)).ok_or_else(no_page)?;
    let r = o.subtree(c.para);
    let last = r.end.saturating_sub(1);
    let len = o.paragraphs.get(last).map(|p| p.len()).unwrap_or(0);
    s.set_range(Pos::new(c.block, c.para, 0), Pos::new(c.block, last, len));
    Ok(Value::Null)
}

fn move_outline(s: &mut Session, v: &Value) -> CmdResult {
    let id = p::req_id(v, "id")?;
    let x = p::req_f32(v, "x")?;
    let y = p::req_f32(v, "y")?;
    let page = s.page_mut().ok_or_else(no_page)?;
    let o = page.outline_mut(id).ok_or_else(|| CmdError::Params("no such note container".into()))?;
    // Containers can't be dragged above or left of the page origin.
    o.pos = Point::new(x.max(0.0), y.max(0.0)).sanitized();
    s.coalesce = Some(format!("move:{id}"));
    Ok(Value::Null)
}

fn resize_outline(s: &mut Session, v: &Value) -> CmdResult {
    let id = p::req_id(v, "id")?;
    let width = match v.get("width") {
        None | Some(Value::Null) => OutlineWidth::Auto,
        Some(w) => OutlineWidth::Fixed(
            w.as_f64().filter(|f| f.is_finite()).ok_or_else(|| CmdError::Params("`width` must be a number or null".into()))? as f32,
        ),
    };
    let page = s.page_mut().ok_or_else(no_page)?;
    let o = page.outline_mut(id).ok_or_else(|| CmdError::Params("no such note container".into()))?;
    o.width = width;
    o.normalize();
    s.coalesce = Some(format!("resize:{id}"));
    Ok(Value::Null)
}

fn move_paragraph(s: &mut Session, v: &Value) -> CmdResult {
    let from = p::req_id(v, "from")?;
    let para = p::req_usize(v, "para")?;
    let to = p::id(v, "to")?;
    // Without a target container the paragraph is dropped on empty canvas: a new container
    // appears there (unverified: the reference's exact drop rule).
    let at_point = match to {
        Some(_) => None,
        None => Some(Point::new(p::req_f32(v, "x")?.max(0.0), p::req_f32(v, "y")?.max(0.0)).sanitized()),
    };
    let index = if to.is_some() { p::req_usize(v, "index")? } else { 0 };
    let level = p::usize(v, "level");
    let new_id = at_point.map(|_| s.new_id());
    let page = s.page_mut().ok_or_else(no_page)?;
    let src = page.outline_mut(from).ok_or_else(|| CmdError::Params("no such source container".into()))?;
    if para >= src.paragraphs.len() {
        return Err(CmdError::Params("no such paragraph".into()));
    }
    let r = src.subtree(para);
    let mut block: Vec<_> = src.paragraphs.drain(r.clone()).collect();
    let base = block.first().map(|p| p.level).unwrap_or(0);
    let target_level = match (level, at_point) {
        (Some(l), _) => Some(l.min(notecraft_doc::text::MAX_LEVEL as usize) as u8),
        (None, Some(_)) => Some(0),
        (None, None) => None,
    };
    if let Some(l) = target_level {
        for p in &mut block {
            p.level = (l as i16 + p.level as i16 - base as i16).clamp(0, notecraft_doc::text::MAX_LEVEL as i16) as u8;
        }
    }
    if src.paragraphs.is_empty() {
        page.objects.retain(|o| o.id() != from);
    }
    let (to, at) = match (to, at_point, new_id) {
        (Some(to), _, _) => {
            let dst = page.outline_mut(to).ok_or_else(|| CmdError::Params("no such target container".into()))?;
            // Moving within the same container: the index refers to the list after removal.
            let at = index.min(dst.paragraphs.len());
            for (k, p) in block.into_iter().enumerate() {
                dst.paragraphs.insert(at + k, p);
            }
            dst.normalize();
            (to, at)
        }
        (None, Some(pt), Some(id)) => {
            let mut o = notecraft_doc::Outline::new(id, pt, notecraft_doc::Id::NIL);
            o.paragraphs = block;
            o.normalize();
            page.objects.push(notecraft_doc::PageObject::Outline(o));
            (id, 0)
        }
        _ => return Err(CmdError::Params("`to` or `x` and `y` are required".into())),
    };
    s.set_caret(Pos::new(Block::Outline(to), at, 0));
    Ok(Value::Null)
}

fn select_object(s: &mut Session, v: &Value) -> CmdResult {
    let ids: Vec<notecraft_doc::Id> = v
        .get("ids")
        .and_then(Value::as_array)
        .ok_or_else(|| CmdError::Params("`ids` (array of outline ids) is required".into()))?
        .iter()
        .take(10_000)
        .filter_map(|x| x.as_str().and_then(notecraft_doc::Id::parse))
        .collect();
    let page = s.page().ok_or_else(no_page)?;
    let valid: Vec<_> = ids.into_iter().filter(|id| page.object_index(*id).is_some()).collect();
    let add = p::bool(v, "add").unwrap_or(false);
    let mut cur = match (&s.sel.on_page, add) {
        (PageSel::Objects { ids }, true) => ids.clone(),
        _ => Vec::new(),
    };
    for id in valid {
        if !cur.contains(&id) {
            cur.push(id);
        }
    }
    s.sel.on_page = PageSel::Objects { ids: cur };
    Ok(Value::Null)
}
