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
    /// Insert Space: dragging on the page pushes what is below (or right of) the start down
    /// (or right), or pulls it back.
    Space,
    /// Draw › Shapes: dragging draws [`DrawState::shape`] with the selected pen.
    Shape,
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

/// The Draw tab's state: tool, eraser size, the pen gallery and the selected pen. Read back
/// from the desktop settings (keys a file lacks take their defaults; [`DrawState::normalize`]
/// repairs the rest).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawState {
    /// The active tool.
    pub tool: DrawTool,
    /// The eraser the Eraser button uses (the last one chosen).
    pub eraser: EraserKind,
    /// The pen gallery, in order.
    pub pens: Vec<PenStyle>,
    /// The selected pen (an index into `pens`).
    pub pen: usize,
    /// The shape the Shape tool draws.
    #[serde(default)]
    pub shape: notecraft_ink::ShapeKind,
    /// Lock Drawing Mode: the Shape tool stays on after a shape (else it returns to Type).
    #[serde(default)]
    pub lock_shapes: bool,
    /// The ruler, when shown (`R.Draw.Tools.Ruler`, decision D7).
    #[serde(default)]
    pub ruler: Option<notecraft_ink::ruler::Ruler>,
    /// Snap to Grid: shapes start and end on the page's grid (decision D8).
    #[serde(default)]
    pub snap_to_grid: bool,
    /// Draw with Touch: a finger draws with the drawing tools; off, a finger moves the page
    /// (decision D8; a missing field means on, like [`DrawState::default`]).
    pub with_touch: bool,
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
            shape: notecraft_ink::ShapeKind::Line,
            lock_shapes: false,
            ruler: None,
            snap_to_grid: false,
            with_touch: true,
        }
    }
}

impl DrawState {
    /// `p` on the page's grid when Snap to Grid is on (decision D8): the grid's spacing on a
    /// page with grid lines, else 1/8 in (the small grid's); lines start at the page's corner.
    pub fn grid_point(&self, page: Option<&notecraft_doc::Page>, p: Point) -> Point {
        if !self.snap_to_grid {
            return p;
        }
        let lines = page.map(|pg| pg.setup.rule_lines).filter(|r| r.is_grid()).unwrap_or(notecraft_doc::RuleLines::GridSmall);
        let step = lines.spacing().max(1.0);
        Point::new((p.x / step).round() * step, (p.y / step).round() * step).sanitized()
    }

    /// Repair a state read from outside (a settings file): at most [`MAX_PENS`] pens, tip
    /// widths in range, the default pens when none are left, a valid selected pen, the Type
    /// tool and no ruler (a run starts typing).
    pub fn normalize(&mut self) {
        self.pens.truncate(MAX_PENS);
        for p in &mut self.pens {
            p.width = if p.width.is_nan() { 0.5 * MM } else { p.width.clamp(MIN_INK_WIDTH, MAX_INK_WIDTH) };
        }
        if self.pens.is_empty() {
            self.pens = DrawState::default().pens;
        }
        if self.pen >= self.pens.len() {
            self.pen = 0;
        }
        self.tool = DrawTool::Type;
        self.ruler = None;
    }

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
        CommandSpec::new("draw.shapes", "R.Draw.Shapes.InkShapeGallery", "Shapes", "Draw/Shapes", choose_shape)
            .params(r#"{"kind"?: "line" | "arrow" | "double arrow" | "rectangle" | "oval" | "triangle" | "right triangle" | "diamond" | "parallelogram"}"#)
            .pure(),
        CommandSpec::new("draw.lockDrawingMode", "R.Draw.Shapes.ShapesDuplicate", "Lock Drawing Mode", "Draw/Shapes/Lock Drawing Mode", |s, _| {
            s.draw.lock_shapes = !s.draw.lock_shapes;
            Ok(json!({"locked": s.draw.lock_shapes}))
        })
        .pure(),
        CommandSpec::new("draw.snapToGrid", "R.Draw.Shapes.GridSnapTo", "Snap to Grid", "Draw/Shapes/Snap to Grid", |s, v| {
            s.draw.snap_to_grid = p::bool(v, "on").unwrap_or(!s.draw.snap_to_grid);
            Ok(json!({"snap": s.draw.snap_to_grid}))
        })
        .params(r#"{"on"?: bool (default: switch)}"#)
        .pure(),
        CommandSpec::new("draw.withTouch", "R.Draw.Tools.FingerPaintingMode", "Draw with Touch", "Draw/Tools/Draw with Touch", |s, v| {
            s.draw.with_touch = p::bool(v, "on").unwrap_or(!s.draw.with_touch);
            Ok(json!({"touch": s.draw.with_touch}))
        })
        .params(r#"{"on"?: bool (default: switch)}"#)
        .pure(),
        CommandSpec::new("draw.ruler", "R.Draw.Tools.Ruler", "Ruler", "Draw/Tools/Ruler", toggle_ruler)
            .params(r#"{"x"?: pt, "y"?: pt centre when shown (default: the top left of the page's content area), "angle"?: degrees clockwise}"#)
            .pure()
            .when(need_page),
        CommandSpec::new("draw.rulerPlace", "NC.Draw.RulerPlace", "Move or Turn the Ruler", "Mouse/Canvas/Ruler", place_ruler)
            .params(r#"{"x"?: pt, "y"?: pt, "angle"?: degrees} | {"dx"?: pt, "dy"?: pt, "turn"?: degrees}"#)
            .pure()
            .when(need_ruler),
        CommandSpec::new("draw.shape", "NC.Draw.Shape", "Shape", "Mouse/Canvas/Shape", draw_shape)
            .params(r##"{"from": [x, y], "to": [x, y] page points, "kind"?: shape (default: the chosen one), "pen"?: index, "color"?: "#rrggbb", "width"?: pt}"##)
            .when(need_page),
        CommandSpec::new("draw.erase", "NC.Draw.Erase", "Erase", "Mouse/Canvas/Erase", erase)
            .params(r#"{"points": [[x, y], …] eraser path, "kind"?: "small" | "medium" | "large" | "stroke" (default: the eraser), "gesture"?: number (calls with the same number undo together)}"#)
            .when(need_page),
        CommandSpec::new("insert.space", "R.Insert.Insert.PenInsertSpace", "Insert Space", "Insert/Insert Space", |s, _| {
            let t = if s.draw.tool == DrawTool::Space { DrawTool::Type } else { DrawTool::Space };
            set_tool(s, t)
        })
        .pure()
        .when(need_page),
        CommandSpec::new("canvas.insertSpace", "BEH.Canvas.InsertSpace", "Insert Space", "Behaviour/Canvas", insert_space)
            .params(r#"{"y": pt, "dy": pt} (push down what starts below y) | {"x": pt, "dx": pt} (push right); negative removes space"#)
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
    let mut samples = path(v, notecraft_ink::MAX_SAMPLES)?;
    // R.Draw.Tools.Ruler: a stroke started at the ruler's edge runs along it.
    if let Some(r) = s.draw.ruler
        && p::bool(v, "free") != Some(true)
        && let Some(along) = r.snap(&samples, stroke_pen(s, v)?.width / 2.0)
    {
        samples = along;
    }
    push_ink(s, v, &samples)
}

fn choose_shape(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(k) = p::str(v, "kind") {
        s.draw.shape = notecraft_ink::ShapeKind::parse(k).ok_or_else(|| CmdError::Params(format!("unknown shape {k:?}")))?;
    }
    s.draw.tool = DrawTool::Shape;
    Ok(json!({"shape": s.draw.shape.name()}))
}

fn point_param(v: &Value, k: &str) -> Result<Point, CmdError> {
    let a = v.get(k).and_then(Value::as_array);
    let n = |i: usize| a.and_then(|a| a.get(i)).and_then(Value::as_f64).map(|x| x as f32);
    match (n(0), n(1)) {
        (Some(x), Some(y)) => Ok(Point::new(x, y).sanitized()),
        _ => Err(CmdError::Params(format!("`{k}` is [x, y]"))),
    }
}

/// A shape from `from` to `to`, drawn with the selected pen as one ink stroke.
fn draw_shape(s: &mut Session, v: &Value) -> CmdResult {
    let kind = match p::str(v, "kind") {
        Some(k) => notecraft_ink::ShapeKind::parse(k).ok_or_else(|| CmdError::Params(format!("unknown shape {k:?}")))?,
        None => s.draw.shape,
    };
    let (a, b) = (point_param(v, "from")?, point_param(v, "to")?);
    let (a, b) = (s.draw.grid_point(s.page().map(|p| p.as_ref()), a), s.draw.grid_point(s.page().map(|p| p.as_ref()), b));
    let samples: Vec<(Point, f32)> = notecraft_ink::shape_points(kind, a, b).into_iter().map(|p| (p, notecraft_doc::ink::DEFAULT_PRESSURE)).collect();
    push_ink(s, v, &samples)
}

/// Clean `samples` and add them as a stroke drawn with the pen the params or the gallery say.
/// The pen a stroke is drawn with: `pen` (an index) or the selected one, with `tool`, `color`
/// and `width` overrides.
fn stroke_pen(s: &Session, v: &Value) -> Result<PenStyle, CmdError> {
    let base = match p::usize(v, "pen") {
        Some(i) => *s.draw.pens.get(i).ok_or_else(|| CmdError::Params(format!("there are {} pens", s.draw.pens.len())))?,
        None => s.draw.current_pen(),
    };
    pen_from(v, base)
}

/// Show the ruler (at `x, y`, else near the top left of the page's content) or hide it.
fn toggle_ruler(s: &mut Session, v: &Value) -> CmdResult {
    if s.draw.ruler.take().is_some() {
        return Ok(json!({"ruler": false}));
    }
    let default = Point::new(crate::ops::DEFAULT_LEFT + notecraft_ink::ruler::LENGTH / 2.0, 200.0);
    let center = match (p::f32(v, "x"), p::f32(v, "y")) {
        (Some(x), Some(y)) => Point::new(x, y),
        _ => default,
    };
    let r = notecraft_ink::ruler::Ruler::new(center, p::f32(v, "angle").unwrap_or(0.0));
    s.draw.ruler = Some(r);
    Ok(json!({"ruler": true, "x": r.center.x, "y": r.center.y, "angle": r.angle}))
}

/// Move or turn the ruler: to `x`, `y`, `angle`, or by `dx`, `dy`, `turn`.
fn place_ruler(s: &mut Session, v: &Value) -> CmdResult {
    let mut r = s.draw.ruler.ok_or_else(|| CmdError::Disabled("show the ruler first".into()))?;
    r.center.x = p::f32(v, "x").unwrap_or(r.center.x) + p::f32(v, "dx").unwrap_or(0.0);
    r.center.y = p::f32(v, "y").unwrap_or(r.center.y) + p::f32(v, "dy").unwrap_or(0.0);
    r.angle = p::f32(v, "angle").unwrap_or(r.angle) + p::f32(v, "turn").unwrap_or(0.0);
    let r = notecraft_ink::ruler::Ruler::new(r.center, r.angle);
    s.draw.ruler = Some(r);
    Ok(json!({"x": r.center.x, "y": r.center.y, "angle": r.angle, "shown": r.shown_angle()}))
}

fn need_ruler(s: &Session) -> Option<&'static str> {
    if s.draw.ruler.is_none() { Some("Show the ruler first (Draw › Ruler)") } else { None }
}

fn push_ink(s: &mut Session, v: &Value, samples: &[(Point, f32)]) -> CmdResult {
    if s.page().is_some_and(|p| p.objects.len() >= MAX_PAGE_OBJECTS) {
        return Err(CmdError::Failed(format!("a page holds at most {MAX_PAGE_OBJECTS} objects")));
    }
    let samples = notecraft_ink::clean_samples(samples);
    if samples.is_empty() {
        return Err(CmdError::Params("the stroke has no points".into()));
    }
    let pen = stroke_pen(s, v)?;
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

/// Insert Space: every object that starts at or beyond the line moves by the amount; removing
/// space never pulls an object back past the line (unverified). A drag that moves nothing
/// leaves the page untouched (no empty undo step).
fn insert_space(s: &mut Session, v: &Value) -> CmdResult {
    let (vertical, at, by) = match (p::f32(v, "y"), p::f32(v, "dy"), p::f32(v, "x"), p::f32(v, "dx")) {
        (Some(y), Some(dy), _, _) => (true, y, dy),
        (_, _, Some(x), Some(dx)) => (false, x, dx),
        _ => return Err(CmdError::Params("give `y` and `dy`, or `x` and `dx`".into())),
    };
    let start = |o: &PageObject| if vertical { o.pos().y } else { o.pos().x };
    let page = s.page().ok_or_else(no_page)?;
    let moving: Vec<Id> = page.objects.iter().filter(|o| start(o) >= at).map(PageObject::id).collect();
    let nearest = page.objects.iter().map(start).filter(|p| *p >= at).fold(f32::INFINITY, f32::min);
    let by = if by < 0.0 { by.max(at - nearest) } else { by };
    if moving.is_empty() || by == 0.0 || !by.is_finite() {
        return Ok(json!({"moved": 0}));
    }
    let page = s.page_mut().ok_or_else(no_page)?;
    for o in page.objects.iter_mut().filter(|o| moving.contains(&o.id())) {
        let p = o.pos();
        o.set_pos(if vertical { Point::new(p.x, (p.y + by).max(0.0)) } else { Point::new((p.x + by).max(0.0), p.y) });
    }
    Ok(json!({"moved": moving.len(), "by": by}))
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
