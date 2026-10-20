//! The Draw tab (decision D4, `docs/plan.md` §4.10): the tools (Type, Lasso Select, Panning
//! Hand, the erasers and the pens), the pen gallery (Add Pen, Color & Thickness), and the three
//! gestures the canvas turns into commands: drawing a stroke (`draw.stroke`), erasing along a
//! path (`draw.erase`) and lassoing (`draw.lassoSelect`).
//!
//! The tools and pens are session state ([`DrawState`]), not notebook data: choosing one never
//! adds an undo step. A stroke is one page object and one undo step; an eraser drag coalesces
//! into one step by its `gesture` number. The geometry lives in `notecraft-ink`.

use notecraft_doc::color::Rgba;
use notecraft_doc::geom::{Point, Rect};
use notecraft_doc::ink::{MAX_INK_WIDTH, MIN_INK_WIDTH};
use notecraft_doc::media::MAX_PAGE_OBJECTS;
use notecraft_doc::{Id, Ink, InkTool, OutlineWidth, PageObject};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ops::{need_page, no_page};
use crate::selection::PageSel;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// At most this many pens in the gallery.
pub const MAX_PENS: usize = 32;
/// At most this many path points are read by one `draw.erase` or `draw.lassoSelect` call.
pub const MAX_PATH: usize = 5_000;

/// The active Draw tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DrawTool {
    /// Type and select (the default).
    #[default]
    Type,
    /// Lasso Select.
    Lasso,
    /// Panning Hand: dragging scrolls the page.
    Pan,
    /// The eraser of [`DrawState::eraser`].
    Eraser,
    /// The pen of [`DrawState::pen`].
    Pen,
}

/// Eraser sizes (Draw › Eraser).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EraserKind {
    /// Small partial eraser.
    Small,
    /// Medium partial eraser.
    Medium,
    /// Large partial eraser.
    Large,
    /// Removes whole strokes (the default, unverified).
    #[default]
    Stroke,
}

impl EraserKind {
    /// The eraser's radius, points (unverified; the Stroke Eraser's is its hit slop).
    pub fn radius(self) -> f32 {
        match self {
            EraserKind::Small => 2.5,
            EraserKind::Medium => 6.0,
            EraserKind::Large => 12.0,
            EraserKind::Stroke => 3.0,
        }
    }

    fn parse(s: &str) -> Option<EraserKind> {
        Some(match s {
            "small" => EraserKind::Small,
            "medium" => EraserKind::Medium,
            "large" => EraserKind::Large,
            "stroke" => EraserKind::Stroke,
            _ => return None,
        })
    }
}

/// One pen of the gallery.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PenStyle {
    /// Pen or highlighter.
    pub tool: InkTool,
    /// Colour.
    pub color: Rgba,
    /// Nominal tip width, points.
    pub width: f32,
}

/// 1 mm in points.
const MM: f32 = 72.0 / 25.4;

/// The Draw tab's state: tool, eraser size, the pen gallery and the selected pen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawState {
    /// The active tool.
    pub tool: DrawTool,
    /// The eraser the Eraser button uses (the last one chosen).
    pub eraser: EraserKind,
    /// The pen gallery, in order.
    pub pens: Vec<PenStyle>,
    /// The selected pen (an index into `pens`).
    pub pen: usize,
}

impl Default for DrawState {
    /// Black, red and blue pens of 0.5 mm, yellow and green highlighters of 5 mm (unverified).
    fn default() -> DrawState {
        let pen = |r, g, b| PenStyle { tool: InkTool::Pen, color: Rgba::rgb(r, g, b), width: 0.5 * MM };
        let hl = |r, g, b| PenStyle { tool: InkTool::Highlighter, color: Rgba::rgb(r, g, b), width: 5.0 * MM };
        DrawState {
            tool: DrawTool::Type,
            eraser: EraserKind::Stroke,
            pens: vec![pen(0, 0, 0), pen(0xE8, 0x11, 0x23), pen(0x00, 0x5A, 0xD1), hl(0xFF, 0xF1, 0x00), hl(0x16, 0xC6, 0x0C)],
            pen: 0,
        }
    }
}

impl DrawState {
    /// The selected pen (the first one if the index is stale).
    pub fn current_pen(&self) -> PenStyle {
        self.pens.get(self.pen).or(self.pens.first()).copied().unwrap_or(PenStyle { tool: InkTool::Pen, color: Rgba::BLACK, width: 0.5 * MM })
    }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("draw.type", "R.Draw.Tools.SelectAndTypeMode", "Type", "Draw/Tools/Type", |s, _| set_tool(s, DrawTool::Type)).pure(),
        CommandSpec::new("draw.lasso", "R.Draw.Tools.LassoSelect", "Lasso Select", "Draw/Tools/Lasso Select", |s, _| set_tool(s, DrawTool::Lasso)).pure(),
        CommandSpec::new("draw.pan", "R.Draw.Tools.PanningHand", "Panning Hand", "Draw/Tools/Panning Hand", |s, _| set_tool(s, DrawTool::Pan)).pure(),
        CommandSpec::new("draw.eraser", "R.Draw.Tools.EraserMenu", "Eraser", "Draw/Tools/Eraser", |s, _| set_tool(s, DrawTool::Eraser)).pure(),
        CommandSpec::new("draw.eraserSmall", "R.Draw.Tools.SmallEraser", "Small Eraser", "Draw/Tools/Eraser/Small Eraser", |s, _| {
            set_eraser(s, EraserKind::Small)
        })
        .pure(),
        CommandSpec::new("draw.eraserMedium", "R.Draw.Tools.MediumEraser", "Medium Eraser", "Draw/Tools/Eraser/Medium Eraser", |s, _| {
            set_eraser(s, EraserKind::Medium)
        })
        .pure(),
        CommandSpec::new("draw.eraserLarge", "R.Draw.Tools.LargeEraser", "Large Eraser", "Draw/Tools/Eraser/Large Eraser", |s, _| {
            set_eraser(s, EraserKind::Large)
        })
        .pure(),
        CommandSpec::new("draw.eraserStroke", "R.Draw.Tools.StrokeEraserOneNote", "Stroke Eraser", "Draw/Tools/Eraser/Stroke Eraser", |s, _| {
            set_eraser(s, EraserKind::Stroke)
        })
        .pure(),
        CommandSpec::new("draw.pen", "R.Draw.Tools.InkStylesGallery", "Pens", "Draw/Tools/Pens", choose_pen)
            .params(r#"{"index"?: pen in the gallery (default: the selected one)}"#)
            .pure(),
        CommandSpec::new("draw.addPen", "R.Draw.Tools.AddPen", "Add Pen", "Draw/Tools/Add Pen", add_pen)
            .params(r##"{"tool"?: "pen" | "highlighter", "color"?: "#rrggbb", "width"?: pt}"##)
            .pure(),
        CommandSpec::new(
            "draw.colorThickness",
            "R.Draw.Tools.MoreColorAndThicknessOptions",
            "Color & Thickness",
            "Draw/Tools/Color & Thickness",
            color_thickness,
        )
        .params(r##"{"index"?: pen (default: the selected one), "tool"?: "pen" | "highlighter", "color"?: "#rrggbb", "width"?: pt}"##)
        .pure(),
        CommandSpec::new("draw.stroke", "NC.Draw.Stroke", "Ink", "Mouse/Canvas/Draw", stroke)
            .params(r##"{"points": [[x, y, pressure?], …] page points, "pen"?: index, "tool"?: "pen" | "highlighter", "color"?: "#rrggbb", "width"?: pt}"##)
            .when(need_page),
        CommandSpec::new("draw.erase", "NC.Draw.Erase", "Erase", "Mouse/Canvas/Erase", erase)
            .params(r#"{"points": [[x, y], …] eraser path, "kind"?: "small" | "medium" | "large" | "stroke" (default: the eraser), "gesture"?: number (calls with the same number undo together)}"#)
            .when(need_page),
        CommandSpec::new("draw.lassoSelect", "BEH.Ink.Lasso", "Lasso Select", "Behaviour/Ink", lasso_select)
            .params(r#"{"points": [[x, y], …] the loop, page points}"#)
            .pure()
            .when(need_page),
        CommandSpec::new(
            "object.rotateRight45",
            "R.Draw.Edit.RotateRight45OneNote",
            "Rotate Right 45°",
            "Draw/Edit/Rotate/Rotate Right 45°",
            |s, _| rotate_ink(s, 45.0),
        )
        .when(need_ink),
        CommandSpec::new("object.rotateLeft45", "R.Draw.Edit.RotateLeft45OneNote", "Rotate Left 45°", "Draw/Edit/Rotate/Rotate Left 45°", |s, _| {
            rotate_ink(s, -45.0)
        })
        .when(need_ink),
    ]
}

fn set_tool(s: &mut Session, tool: DrawTool) -> CmdResult {
    s.draw.tool = tool;
    Ok(Value::Null)
}

fn set_eraser(s: &mut Session, kind: EraserKind) -> CmdResult {
    s.draw.eraser = kind;
    s.draw.tool = DrawTool::Eraser;
    Ok(Value::Null)
}

fn choose_pen(s: &mut Session, v: &Value) -> CmdResult {
    let i = p::usize(v, "index").unwrap_or(s.draw.pen);
    if i >= s.draw.pens.len() {
        return Err(CmdError::Params(format!("there are {} pens", s.draw.pens.len())));
    }
    s.draw.pen = i;
    s.draw.tool = DrawTool::Pen;
    Ok(json!({"pen": i}))
}

/// `tool`, `color` and `width` from params over `base`.
fn pen_from(v: &Value, base: PenStyle) -> Result<PenStyle, CmdError> {
    let tool = match p::str(v, "tool") {
        None => base.tool,
        Some("pen") => InkTool::Pen,
        Some("highlighter") => InkTool::Highlighter,
        Some(o) => return Err(CmdError::Params(format!("`tool` is \"pen\" or \"highlighter\", not {o:?}"))),
    };
    let color = match p::str(v, "color") {
        None => base.color,
        Some(c) => Rgba::from_hex(c).ok_or_else(|| CmdError::Params(format!("`color` {c:?} is not #rrggbb")))?,
    };
    let width = p::f32(v, "width").unwrap_or(base.width).clamp(MIN_INK_WIDTH, MAX_INK_WIDTH);
    Ok(PenStyle { tool, color, width })
}

fn add_pen(s: &mut Session, v: &Value) -> CmdResult {
    if s.draw.pens.len() >= MAX_PENS {
        return Err(CmdError::Failed(format!("the gallery holds at most {MAX_PENS} pens")));
    }
    let pen = pen_from(v, s.draw.current_pen())?;
    s.draw.pens.push(pen);
    s.draw.pen = s.draw.pens.len() - 1;
    s.draw.tool = DrawTool::Pen;
    Ok(json!({"pen": s.draw.pen}))
}

fn color_thickness(s: &mut Session, v: &Value) -> CmdResult {
    let i = p::usize(v, "index").unwrap_or(s.draw.pen);
    let base = *s.draw.pens.get(i).ok_or_else(|| CmdError::Params(format!("there are {} pens", s.draw.pens.len())))?;
    let pen = pen_from(v, base)?;
    if let Some(slot) = s.draw.pens.get_mut(i) {
        *slot = pen;
    }
    s.draw.pen = i;
    s.draw.tool = DrawTool::Pen;
    Ok(json!({"pen": i, "width": pen.width, "color": pen.color.to_hex()}))
}

/// `[[x, y, p?], …]` → (page point, pressure), at most `cap` entries; anything that isn't a
/// pair or triple of numbers is refused.
fn path(v: &Value, cap: usize) -> Result<Vec<(Point, f32)>, CmdError> {
    let arr = v.get("points").and_then(Value::as_array).ok_or_else(|| CmdError::Params("`points` (array of [x, y]) is required".into()))?;
    arr.iter()
        .take(cap)
        .map(|pt| {
            let n = |i: usize| pt.get(i).and_then(Value::as_f64).map(|x| x as f32);
            match (n(0), n(1)) {
                (Some(x), Some(y)) => Ok((Point::new(x, y).sanitized(), n(2).unwrap_or(notecraft_doc::ink::DEFAULT_PRESSURE))),
                _ => Err(CmdError::Params("every point is [x, y] or [x, y, pressure]".into())),
            }
        })
        .collect()
}

fn stroke(s: &mut Session, v: &Value) -> CmdResult {
    if s.page().is_some_and(|p| p.objects.len() >= MAX_PAGE_OBJECTS) {
        return Err(CmdError::Failed(format!("a page holds at most {MAX_PAGE_OBJECTS} objects")));
    }
    let samples = path(v, notecraft_ink::MAX_SAMPLES)?;
    let samples = notecraft_ink::clean_samples(&samples);
    if samples.is_empty() {
        return Err(CmdError::Params("the stroke has no points".into()));
    }
    let base = match p::usize(v, "pen") {
        Some(i) => *s.draw.pens.get(i).ok_or_else(|| CmdError::Params(format!("there are {} pens", s.draw.pens.len())))?,
        None => s.draw.current_pen(),
    };
    let pen = pen_from(v, base)?;
    let id = s.new_id();
    let ink = Ink::new(id, &samples, pen.color, pen.width, pen.tool);
    s.pending_outline = None;
    s.page_mut().ok_or_else(no_page)?.objects.push(PageObject::Ink(ink));
    Ok(json!({"ink": id.to_string()}))
}

fn erase(s: &mut Session, v: &Value) -> CmdResult {
    let kind = match p::str(v, "kind") {
        None => s.draw.eraser,
        Some(k) => EraserKind::parse(k).ok_or_else(|| CmdError::Params(format!("unknown eraser {k:?}")))?,
    };
    let pts: Vec<Point> = path(v, MAX_PATH)?.into_iter().map(|(p, _)| p).collect();
    let r = kind.radius();
    // Work out every change read-only first: a drag that touches nothing must not touch the
    // page (`page_mut` would record an empty undo step).
    let page = s.page().ok_or_else(no_page)?;
    let mut changes: Vec<(Id, Option<Vec<Vec<(Point, f32)>>>)> = Vec::new();
    for o in &page.objects {
        let PageObject::Ink(k) = o else { continue };
        if kind == EraserKind::Stroke {
            if notecraft_ink::swept(k, &pts, r) {
                changes.push((k.id, None));
            }
        } else if let Some(pieces) = notecraft_ink::erase(k, &pts, r) {
            changes.push((k.id, Some(pieces)));
        }
    }
    if changes.is_empty() {
        return Ok(json!({"erased": 0, "pieces": 0}));
    }
    let room = MAX_PAGE_OBJECTS.saturating_sub(page.objects.len());
    let (mut erased, mut added) = (0usize, 0usize);
    let mut fresh: Vec<Vec<Id>> = Vec::new();
    for (_, pieces) in &changes {
        let n = pieces.as_ref().map_or(0, Vec::len).saturating_sub(1);
        fresh.push((0..n).map(|_| s.new_id()).collect());
    }
    let gesture = p::usize(v, "gesture");
    let page = s.page_mut().ok_or_else(no_page)?;
    for ((id, pieces), ids) in changes.into_iter().zip(fresh) {
        let Some(i) = page.object_index(id) else { continue };
        let Some(PageObject::Ink(old)) = page.objects.get(i).cloned() else { continue };
        page.objects.remove(i);
        erased += 1;
        let Some(pieces) = pieces else { continue };
        // The pieces take the stroke's place in the z-order, the first one keeping its id.
        for (at, (k, piece)) in (i..).zip(pieces.iter().enumerate()) {
            if k > 0 && added >= room {
                break;
            }
            let pid = if k == 0 { id } else { ids.get(k - 1).copied().unwrap_or(id) };
            page.objects.insert(at, PageObject::Ink(Ink::new(pid, piece, old.color, old.width, old.tool)));
            if k > 0 {
                added += 1;
            }
        }
    }
    if let Some(g) = gesture {
        s.coalesce = Some(format!("erase:{g}"));
    }
    if let PageSel::Objects { ids } = &mut s.sel.on_page {
        let page = s.ws.page(s.sel.page.unwrap_or_default());
        ids.retain(|id| page.is_some_and(|p| p.object_index(*id).is_some()));
    }
    Ok(json!({"erased": erased, "pieces": added}))
}

/// The estimated box of an object in page points (the engine has no text layout: 20 pt per
/// line, auto-width containers at their maximum width).
pub(crate) fn object_box(o: &PageObject) -> Rect {
    let pos = o.pos();
    let w = match o {
        PageObject::Outline(ol) => match ol.width {
            OutlineWidth::Fixed(w) => w,
            OutlineWidth::Auto => notecraft_doc::page::AUTO_WIDTH_MAX,
        },
        PageObject::Table(t) => t.columns.iter().sum(),
        PageObject::Ink(k) => return k.bounds(),
        _ => crate::picture_cmds::media_size(o).map_or(0.0, |(w, _)| w),
    };
    Rect::from_points(pos, Point::new(pos.x + w, crate::ops::object_bottom(o)))
}

fn lasso_select(s: &mut Session, v: &Value) -> CmdResult {
    let poly: Vec<Point> = path(v, MAX_PATH)?.into_iter().map(|(p, _)| p).collect();
    let page = s.page().ok_or_else(no_page)?;
    let ids: Vec<Id> = page
        .objects
        .iter()
        .filter(|o| match o {
            PageObject::Ink(k) => notecraft_ink::lasso_picks(k, &poly),
            PageObject::Image(im) if im.background => false,
            _ => notecraft_ink_geom::in_polygon(&poly, object_box(o).center()),
        })
        .map(PageObject::id)
        .collect();
    let out: Vec<String> = ids.iter().map(Id::to_string).collect();
    s.pending_outline = None;
    s.sel.on_page = if ids.is_empty() { PageSel::None } else { PageSel::Objects { ids } };
    Ok(json!({"ids": out}))
}

/// The ink strokes among the selected objects.
pub(crate) fn selected_ink(s: &Session) -> Vec<Id> {
    let (Some(page), PageSel::Objects { ids }) = (s.page(), &s.sel.on_page) else { return Vec::new() };
    ids.iter().copied().filter(|id| page.ink(*id).is_some()).collect()
}

fn need_ink(s: &Session) -> Option<&'static str> {
    if s.sel.page.is_none() {
        return Some("No page is open");
    }
    if selected_ink(s).is_empty() { Some("Select some ink first") } else { None }
}

/// Apply `f` to the selected strokes as one group (about the centre of their joint box), then
/// shift the group back onto the page if it went past the top or left edge. Returns whether
/// any ink was selected (and so changed).
pub(crate) fn transform_ink(s: &mut Session, f: impl Fn(&mut Ink, Point)) -> Result<bool, CmdError> {
    let ids = selected_ink(s);
    let page = s.page().ok_or_else(no_page)?;
    let Some(group) = notecraft_ink::union_bounds(ids.iter().filter_map(|id| page.ink(*id))) else { return Ok(false) };
    let c = group.center();
    let page = s.page_mut().ok_or_else(no_page)?;
    for id in &ids {
        if let Some(k) = page.ink_mut(*id) {
            f(k, c);
        }
    }
    let after = notecraft_ink::union_bounds(ids.iter().filter_map(|id| page.ink(*id)));
    if let Some(b) = after {
        let (dx, dy) = ((-b.min.x).max(0.0), (-b.min.y).max(0.0));
        if dx > 0.0 || dy > 0.0 {
            for id in &ids {
                if let Some(k) = page.ink_mut(*id) {
                    k.pos = Point::new(k.pos.x + dx, k.pos.y + dy);
                }
            }
        }
    }
    Ok(true)
}

/// `[x, y, w, h]` from params, finite with a positive size.
fn rect_param(v: &Value, k: &str) -> Option<Rect> {
    let a = v.get(k)?.as_array()?;
    let n = |i: usize| a.get(i).and_then(Value::as_f64).map(|x| x as f32).filter(|x| x.is_finite());
    let (x, y, w, h) = (n(0)?, n(1)?, n(2)?, n(3)?);
    (w > 0.0 && h > 0.0).then(|| Rect::from_points(Point::new(x, y), Point::new(x + w, y + h)))
}

/// Resize the selected ink: the rectangle `from` (the selection box as it was drawn) maps onto
/// `to`, so the sizing handles can send one small step per frame (they coalesce into one undo
/// step). Without rectangles, `dw`/`dh` grow the group's box from its top-left corner.
pub(crate) fn resize_ink(s: &mut Session, v: &Value) -> CmdResult {
    let ids = selected_ink(s);
    let page = s.page().ok_or_else(no_page)?;
    let group =
        notecraft_ink::union_bounds(ids.iter().filter_map(|id| page.ink(*id))).ok_or_else(|| CmdError::Disabled("select some ink first".into()))?;
    let (from, to) = match (rect_param(v, "from"), rect_param(v, "to")) {
        (Some(a), Some(b)) => (a, b),
        _ => match (p::f32(v, "dw"), p::f32(v, "dh")) {
            (None, None) => {
                s.notice("Drag a sizing handle of the selection to resize the ink.");
                return Ok(Value::Null);
            }
            (dw, dh) => {
                let (w, h) = (group.width() + dw.unwrap_or(0.0), group.height() + dh.unwrap_or(0.0));
                (group, Rect::from_points(group.min, Point::new(group.min.x + w.max(1.0), group.min.y + h.max(1.0))))
            }
        },
    };
    // At most 100× per step either way: a hostile rectangle can't fling points to infinity
    // (they are clamped by `geom::finite` anyway).
    let kx = (to.width() / from.width().max(0.01)).clamp(0.01, 100.0);
    let ky = (to.height() / from.height().max(0.01)).clamp(0.01, 100.0);
    let (dx, dy) = (to.min.x - from.min.x, to.min.y - from.min.y);
    let page = s.page_mut().ok_or_else(no_page)?;
    for id in &ids {
        if let Some(k) = page.ink_mut(*id) {
            notecraft_ink::scale(k, from.min, kx, ky);
            k.pos = Point::new(k.pos.x + dx, k.pos.y + dy).sanitized();
        }
    }
    s.coalesce = Some(format!("resize:{ids:?}"));
    Ok(Value::Null)
}

fn rotate_ink(s: &mut Session, degrees: f32) -> CmdResult {
    transform_ink(s, |k, c| notecraft_ink::rotate(k, c, degrees))?;
    Ok(Value::Null)
}
