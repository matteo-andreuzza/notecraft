//! Ink on the canvas (decision D4, `docs/plan.md` §4.10): drawing strokes with their pressure,
//! the gestures of the Draw tools (a pen stroke, erasing, the lasso, the panning hand, moving
//! selected ink), the selection box, and the pen gallery of the Draw tab.
//!
//! Every gesture ends in an engine command (`draw.stroke`, `draw.erase`, `draw.lassoSelect`,
//! `object.move`), so the canvas only collects pointer samples. Samples come from every
//! `PointerMoved` event of a frame (not just the last position), so a fast stroke keeps its
//! shape at any frame rate; pressure is egui's touch force where winit reports one, else 0.5.

use egui::{Color32, CursorIcon, Event, Mesh, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, pos2, vec2};
use notecraft_engine::doc::geom::Point;
use notecraft_engine::doc::ink::DEFAULT_PRESSURE;
use notecraft_engine::doc::{Id, Ink, InkTool, Page, PageObject, TagUse};
use notecraft_engine::selection::PageSel;
use notecraft_engine::{DrawTool, PenStyle};
use notecraft_ink::ruler::{self, Ruler};
use serde_json::json;

use crate::canvas::PT;
use crate::theme::{c32, radius};
use crate::{NoteApp, Tokens};

/// The pen gallery's catalog id.
pub const PENS_ID: &str = "R.Draw.Tools.InkStylesGallery";
/// Width of one pen in the gallery, screen points.
pub const PEN_W: f32 = 26.0;
/// Pens shown in the ribbon row before the ▾ (the rest are in the drop-down).
pub const VISIBLE_PENS: usize = 5;
/// How opaque a highlighter is drawn (unverified).
const HIGHLIGHTER_OPACITY: f32 = 0.45;
/// How close to a stroke a click selects it, screen points.
const HIT_SLOP: f32 = 4.0;
/// Pen thicknesses offered by Color & Thickness, millimetres (unverified).
pub const PEN_MM: &[f32] = &[0.25, 0.35, 0.5, 0.7, 1.0, 2.0, 3.5];
/// Highlighter thicknesses offered by Color & Thickness, millimetres (unverified).
pub const HIGHLIGHTER_MM: &[f32] = &[2.0, 3.5, 5.0, 7.0, 10.0];
/// 1 mm in points.
pub const MM: f32 = 72.0 / 25.4;

/// The gallery's width in the ribbon: the visible pens and the ▾.
pub fn gallery_width() -> f32 {
    PEN_W * VISIBLE_PENS as f32 + 14.0
}

/// An ink stroke laid out on screen.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct InkLayout {
    /// The stroke's id.
    pub id: Id,
    /// Its drawn bounds (screen).
    #[serde(skip)]
    pub rect: Rect,
}

/// A gesture in progress.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Gesture {
    /// A pen stroke: page points and pressure so far.
    Stroke(Vec<(Point, f32)>),
    /// An eraser drag: its number (one undo step) and the last page point erased.
    Erase { seq: u64, last: Point },
    /// A lasso loop, page points.
    Lasso(Vec<Point>),
    /// The panning hand.
    Pan,
    /// Insert Space: where the drag began and where the pointer is (page points).
    Space { from: Point, to: Point },
    /// A shape being dragged out (page points).
    Shape { from: Point, to: Point },
    /// Moving the selected objects: the last page point.
    Group { last: Point },
    /// Resizing the selected ink by a handle (`pictures::handles` order): the box when the drag
    /// began and as last sent (page `x, y, w, h`), and where the pointer went down.
    Resize { handle: usize, start: (f32, f32, f32, f32), last: (f32, f32, f32, f32), grab: Point },
    /// Moving the ruler: the last page point.
    Ruler { last: Point },
}

/// The canvas's ink state.
#[derive(Default)]
pub struct InkUi {
    /// Strokes of the last frame, in z-order.
    pub layout: Vec<InkLayout>,
    pub(crate) gesture: Option<Gesture>,
    seq: u64,
    /// The latest pen pressure the platform reported (touch force), if any.
    force: Option<f32>,
}

impl InkUi {
    /// Whether a gesture is in progress.
    pub(crate) fn busy(&self) -> bool {
        self.gesture.is_some()
    }
}

/// Lay out the page's strokes on screen.
pub(crate) fn layout(app: &NoteApp, page: &Page) -> Vec<InkLayout> {
    page.objects
        .iter()
        .filter_map(PageObject::as_ink)
        .map(|k| {
            let b = k.bounds();
            InkLayout { id: k.id, rect: Rect::from_min_max(app.canvas.to_screen(b.min), app.canvas.to_screen(b.max)) }
        })
        .collect()
}

/// The colour a stroke is drawn in: black ink follows the page's text colour (so it stays
/// readable in the dark themes and on dark page colours), highlighters are translucent.
fn ink_color(color: notecraft_engine::doc::color::Rgba, tool: InkTool, page_text: Color32) -> Color32 {
    let c = if color == notecraft_engine::doc::color::Rgba::BLACK { page_text } else { c32(color) };
    match tool {
        InkTool::Pen => c,
        InkTool::Highlighter => c.gamma_multiply(HIGHLIGHTER_OPACITY),
    }
}

/// Fill a stroke through `pts` with `half[i]` on each side of point `i`: triangle strips split
/// at sharp corners (an averaged normal there would pinch the stroke), with round caps and
/// round joins at those corners for pens.
fn paint_stroke(painter: &Painter, pts: &[Pos2], half: &[f32], color: Color32, round: bool) {
    let hw = |i: usize| half.get(i).or(half.last()).copied().unwrap_or(0.5).max(0.5);
    let n = pts.len();
    if n == 1 {
        painter.circle_filled(pts[0], hw(0), color);
        return;
    }
    let sharp = |i: usize| {
        i > 0 && i + 1 < n && {
            let (u, v) = ((pts[i] - pts[i - 1]).normalized(), (pts[i + 1] - pts[i]).normalized());
            u.dot(v) < 0.9
        }
    };
    let mut start = 0;
    for i in 1..n {
        if i + 1 < n && !sharp(i) {
            continue;
        }
        // One run of gentle turns, start..=i.
        let geo: Vec<Point> = pts[start..=i].iter().map(|p| Point::new(p.x, p.y)).collect();
        let widths: Vec<f32> = (start..=i).map(hw).collect();
        let edges = notecraft_ink_geom::outline(&geo, &widths);
        let mut mesh = Mesh::default();
        for (l, r) in &edges {
            mesh.colored_vertex(pos2(l.x, l.y), color);
            mesh.colored_vertex(pos2(r.x, r.y), color);
        }
        for k in 0..edges.len().saturating_sub(1) as u32 {
            let k = k * 2;
            mesh.add_triangle(k, k + 1, k + 2);
            mesh.add_triangle(k + 1, k + 3, k + 2);
        }
        painter.add(Shape::mesh(mesh));
        if round {
            painter.circle_filled(pts[i], hw(i), color);
        }
        start = i;
    }
    if round {
        painter.circle_filled(pts[0], hw(0), color);
    }
}

/// Draw one stroke given in page space.
fn paint_ink(app: &NoteApp, painter: &Painter, k: &Ink, page_text: Color32) {
    let z = PT * app.canvas.zoom.max(0.1);
    let pts: Vec<Pos2> = k.page_points().map(|p| app.canvas.to_screen(p)).collect();
    let half: Vec<f32> = k.points.iter().map(|p| (k.width_at(p.pressure) * z / 2.0).max(0.5)).collect();
    paint_stroke(painter, &pts, &half, ink_color(k.color, k.tool, page_text), k.tool == InkTool::Pen);
}

/// The screen box around the selected ink, if any is selected.
pub(crate) fn selection_box(app: &NoteApp) -> Option<Rect> {
    let PageSel::Objects { ids } = &app.session.sel.on_page else { return None };
    app.canvas.ink.layout.iter().filter(|l| ids.contains(&l.id)).map(|l| l.rect).reduce(|a, b| a.union(b)).map(|r| r.expand(4.0))
}

/// Draw the page's strokes (above the text, in z-order), the stroke being drawn, the lasso and
/// eraser feedback, and the box around selected ink.
/// Where a stroke's tag icons sit on screen: a row ending just left of its box, level with its
/// top (decision D5; the placement is unverified).
fn tag_rects<'a>(app: &NoteApp, k: &'a Ink, rect: Rect) -> impl Iterator<Item = (Rect, &'a TagUse)> + 'a {
    let zoom = app.canvas.zoom.max(0.1);
    let size = (14.0 * zoom).clamp(8.0, 40.0);
    let step = crate::canvas::TAG_W * PT * zoom;
    let n = k.tags.len();
    k.tags.iter().enumerate().map(move |(i, tag)| (Rect::from_min_size(pos2(rect.min.x - step * (n - i) as f32, rect.min.y), vec2(size, size)), tag))
}

/// The checkbox tag under a screen point: the stroke and the checkbox's index among the
/// stroke's checkboxes (what `tag.click` takes).
pub(crate) fn checkbox_at(app: &NoteApp, p: Pos2) -> Option<(Id, usize)> {
    let page = app.session.page()?;
    page.objects
        .iter()
        .filter_map(PageObject::as_ink)
        .zip(&app.canvas.ink.layout)
        .filter(|(k, _)| !k.tags.is_empty())
        .find_map(|(k, l)| tag_rects(app, k, l.rect).filter(|(_, tag)| tag.symbol.is_checkbox()).position(|(r, _)| r.contains(p)).map(|i| (k.id, i)))
}

/// The strokes on screen with their tag icons, for `ui.inspect`.
pub(crate) fn inspect(app: &NoteApp) -> serde_json::Value {
    let Some(page) = app.session.page() else { return json!([]) };
    page.objects
        .iter()
        .filter_map(PageObject::as_ink)
        .zip(&app.canvas.ink.layout)
        .take(500)
        .map(|(k, l)| {
            let tags: Vec<_> =
                tag_rects(app, k, l.rect).map(|(r, t)| json!({"key": t.key, "checked": t.checked, "rect": crate::rect_json(r)})).collect();
            json!({"id": k.id, "rect": crate::rect_json(l.rect), "tags": tags})
        })
        .collect()
}

pub(crate) fn draw(app: &NoteApp, painter: &Painter, page: &Page, t: Tokens, page_text: Color32) {
    let view = app.canvas.viewport.expand(40.0);
    let mut defs = None;
    for (k, l) in page.objects.iter().filter_map(PageObject::as_ink).zip(&app.canvas.ink.layout) {
        if l.rect.intersects(view) {
            paint_ink(app, painter, k, page_text);
        }
        if !k.tags.is_empty() {
            let defs = defs.get_or_insert_with(|| app.session.tag_defs());
            for (r, tag) in tag_rects(app, k, l.rect) {
                let symbol = defs.iter().find(|d| d.key == tag.key).map(|d| d.symbol).unwrap_or(tag.symbol);
                crate::icons::tag(painter, r, symbol, tag.checked, page_text);
            }
        }
    }
    match &app.canvas.ink.gesture {
        Some(Gesture::Stroke(pts)) => {
            let pen = app.session.draw.current_pen();
            let z = PT * app.canvas.zoom.max(0.1);
            let live = Ink {
                id: Id(0),
                pos: Point::ZERO,
                points: Vec::new(),
                color: pen.color,
                width: pen.width,
                tool: pen.tool,
                link: None,
                tags: Vec::new(),
            };
            let along = app.session.draw.ruler.and_then(|r| r.snap(pts, pen.width / 2.0));
            let pts = along.as_deref().unwrap_or(pts);
            let screen: Vec<Pos2> = pts.iter().map(|(p, _)| app.canvas.to_screen(*p)).collect();
            let half: Vec<f32> = pts.iter().map(|(_, pr)| (live.width_at(*pr) * z / 2.0).max(0.5)).collect();
            paint_stroke(painter, &screen, &half, ink_color(pen.color, pen.tool, page_text), pen.tool == InkTool::Pen);
        }
        Some(Gesture::Shape { from, to }) => {
            let pen = app.session.draw.current_pen();
            let z = PT * app.canvas.zoom.max(0.1);
            // Snap to Grid shows where the shape will land.
            let page = app.session.page().map(|p| p.as_ref());
            let (a, b) = (app.session.draw.grid_point(page, *from), app.session.draw.grid_point(page, *to));
            let screen: Vec<Pos2> = notecraft_ink::shape_points(app.session.draw.shape, a, b).into_iter().map(|p| app.canvas.to_screen(p)).collect();
            let half = vec![(pen.width * z / 2.0).max(0.5); screen.len()];
            paint_stroke(painter, &screen, &half, ink_color(pen.color, pen.tool, page_text), pen.tool == InkTool::Pen);
        }
        Some(Gesture::Space { from, to }) => {
            let (a, b) = (app.canvas.to_screen(*from), app.canvas.to_screen(*to));
            let v = app.canvas.viewport;
            if (b.x - a.x).abs() > (b.y - a.y).abs() {
                let band = Rect::from_x_y_ranges(a.x.min(b.x)..=a.x.max(b.x), v.y_range());
                painter.rect_filled(band, 0.0, t.selection);
                painter.line_segment([pos2(a.x, v.min.y), pos2(a.x, v.max.y)], Stroke::new(1.5, t.accent));
            } else {
                let band = Rect::from_x_y_ranges(v.x_range(), a.y.min(b.y)..=a.y.max(b.y));
                painter.rect_filled(band, 0.0, t.selection);
                painter.line_segment([pos2(v.min.x, a.y), pos2(v.max.x, a.y)], Stroke::new(1.5, t.accent));
            }
        }
        Some(Gesture::Lasso(pts)) => {
            let screen: Vec<Pos2> = pts.iter().map(|p| app.canvas.to_screen(*p)).collect();
            let dashes = Shape::dashed_line(&screen, Stroke::new(1.0, t.accent), 4.0, 3.0);
            painter.extend(dashes);
        }
        _ => {}
    }
    if app.session.draw.tool == DrawTool::Eraser
        && let Some(h) = painter.ctx().pointer_hover_pos()
        && app.canvas.viewport.contains(h)
    {
        let r = app.session.draw.eraser.radius() * PT * app.canvas.zoom.max(0.1);
        painter.circle_stroke(h, r, Stroke::new(1.0, t.page_meta));
    }
    if let Some(b) = selection_box(app) {
        let pts = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom(), b.left_top()];
        painter.extend(Shape::dashed_line(&pts, Stroke::new(1.0, t.accent), 4.0, 3.0));
        for h in crate::pictures::handles(b) {
            painter.rect(h, 1.0, t.page, Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
        }
    }
    if let Some(r) = app.session.draw.ruler {
        draw_ruler(app, painter, r, t);
    }
}

/// The ruler (`R.Draw.Tools.Ruler`): a translucent card turned to its angle, millimetre ticks
/// along its top edge with centimetres numbered, and its angle in the middle (metric scale and
/// look unverified).
fn draw_ruler(app: &NoteApp, painter: &Painter, r: Ruler, t: Tokens) {
    let to = |u: f32, v: f32| app.canvas.to_screen(r.page_point(u, v));
    let corners: Vec<Pos2> = r.corners().iter().map(|p| app.canvas.to_screen(*p)).collect();
    painter.add(Shape::convex_polygon(corners, t.ribbon_card.gamma_multiply(0.88), Stroke::new(1.0, t.card_border)));
    let z = PT * app.canvas.zoom.max(0.1);
    let (half_l, top) = (ruler::LENGTH / 2.0, -ruler::WIDTH / 2.0);
    let mm = 72.0 / 25.4;
    let angle = r.angle.to_radians();
    let font = egui::FontId::proportional((8.0 * z).clamp(7.0, 14.0));
    // Millimetre ticks only when they are at least 3 px apart.
    let step = if mm * z >= 3.0 { 1 } else { 10 };
    let count = (ruler::LENGTH / mm) as i32;
    for i in (0..=count).step_by(step) {
        let u = -half_l + i as f32 * mm;
        let len = if i % 10 == 0 {
            12.0
        } else if i % 5 == 0 {
            8.0
        } else {
            5.0
        };
        painter.line_segment([to(u, top), to(u, top + len)], Stroke::new(1.0, t.text));
        if i % 10 == 0 && i > 0 && i < count {
            let g = painter.layout_no_wrap(format!("{}", i / 10), font.clone(), t.text_dim);
            let at = to(u, top + 14.0) - egui::Vec2::angled(angle) * (g.size().x / 2.0);
            painter.add(egui::epaint::TextShape::new(at, g, t.text_dim).with_angle(angle));
        }
    }
    let g = painter.layout_no_wrap(format!("{}°", r.shown_angle()), egui::FontId::proportional((11.0 * z).clamp(9.0, 18.0)), t.text);
    let at = to(0.0, 6.0) - egui::Vec2::angled(angle) * (g.size().x / 2.0);
    painter.add(egui::epaint::TextShape::new(at, g, t.text).with_angle(angle));
}

/// The sizing handle of the ink selection box under a screen point, with the box in page
/// space (`x, y, w, h`).
fn handle_at(app: &NoteApp, p: Pos2) -> Option<(usize, (f32, f32, f32, f32))> {
    let b = selection_box(app)?;
    let i = crate::pictures::handles(b).iter().position(|h| h.expand(2.0).contains(p))?;
    let (a, z) = (app.canvas.to_page(b.min), PT * app.canvas.zoom.max(0.1));
    Some((i, (a.x, a.y, b.width() / z, b.height() / z)))
}

/// The stroke under a screen point (topmost first).
pub(crate) fn hit(app: &NoteApp, p: Pos2) -> Option<Id> {
    let page = app.session.page()?;
    let slop = HIT_SLOP / (PT * app.canvas.zoom.max(0.1));
    let at = app.canvas.to_page(p);
    app.canvas
        .ink
        .layout
        .iter()
        .rev()
        .filter(|l| l.rect.expand(HIT_SLOP).contains(p))
        .find(|l| page.ink(l.id).is_some_and(|k| notecraft_ink::hit(k, at, slop)))
        .map(|l| l.id)
}

/// Shift while dragging a shape: boxes become squares and circles, lines snap to 45°.
fn constrain(kind: notecraft_ink::ShapeKind, from: Point, to: Point, shift: bool) -> Point {
    if !shift {
        return to;
    }
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    if kind.is_line() {
        let len = (dx * dx + dy * dy).sqrt();
        let a = (dy.atan2(dx) / std::f32::consts::FRAC_PI_4).round() * std::f32::consts::FRAC_PI_4;
        Point::new(from.x + len * a.cos(), from.y + len * a.sin())
    } else {
        let side = dx.abs().max(dy.abs());
        Point::new(from.x + side * dx.signum(), from.y + side * dy.signum())
    }
}

/// The shape gallery under Draw › Shapes: one button per shape, its outline drawn in code.
pub(crate) fn shape_gallery(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    egui::Grid::new("shape_gallery").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
        for (i, kind) in notecraft_ink::ShapeKind::ALL.into_iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, kind.name()));
            let on = app.session.draw.tool == DrawTool::Shape && app.session.draw.shape == kind;
            if on {
                ui.painter().rect_filled(rect, radius::CONTROL, t.checked);
            } else if resp.hovered() {
                ui.painter().rect_filled(rect, radius::CONTROL, t.hover);
            }
            let r = rect.shrink(6.0);
            let (a, b) = if kind.is_line() {
                (Point::new(r.min.x, r.max.y), Point::new(r.max.x, r.min.y))
            } else {
                (Point::new(r.min.x, r.min.y), Point::new(r.max.x, r.max.y))
            };
            let pts: Vec<Pos2> = notecraft_ink::shape_points(kind, a, b).into_iter().map(|p| pos2(p.x, p.y)).collect();
            ui.painter().add(Shape::line(pts, Stroke::new(1.25, t.icon)));
            if resp.on_hover_text(kind.name()).clicked() {
                app.run(&ctx, "draw.shapes", json!({"kind": kind.name()}));
                ui.close();
            }
            if i % 3 == 2 {
                ui.end_row();
            }
        }
    });
    ui.separator();
}

/// Every pointer position of this frame (the latest last), so a fast stroke keeps its shape.
fn moves(ui: &Ui, resp: &Response) -> Vec<Pos2> {
    let mut v: Vec<Pos2> = ui.input(|i| i.events.iter().filter_map(|e| if let Event::PointerMoved(p) = e { Some(*p) } else { None }).collect());
    if v.is_empty()
        && let Some(p) = resp.interact_pointer_pos()
    {
        v.push(p);
    }
    v
}

/// The mouse wheel over the ruler turns it a degree a notch (up: counterclockwise; unverified)
/// instead of scrolling the page. Returns true when it turned.
fn ruler_wheel(app: &mut NoteApp, ui: &Ui, ctx: &egui::Context, resp: &Response) -> bool {
    let Some(r) = app.session.draw.ruler else { return false };
    let Some(h) = resp.hover_pos() else { return false };
    if !r.contains(app.canvas.to_page(h)) {
        return false;
    }
    if app.canvas.ink.gesture.is_none() && app.session.draw.tool != DrawTool::Pen {
        ctx.set_cursor_icon(CursorIcon::Move);
    }
    let notches: f32 = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| if let Event::MouseWheel { delta, modifiers, .. } = e { (!modifiers.command).then_some(delta.y) } else { None })
            .map(f32::signum)
            .sum()
    });
    if notches == 0.0 {
        return false;
    }
    ctx.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
    app.run(ctx, "draw.rulerPlace", json!({"turn": -notches}));
    true
}

/// Two fingers on the ruler move it and turn it about the point between them (decision D8),
/// instead of scrolling the page. Returns true while they do.
fn ruler_touch(app: &mut NoteApp, ui: &Ui, ctx: &egui::Context) -> bool {
    let Some(r) = app.session.draw.ruler else { return false };
    let Some(m) = ui.input(|i| i.multi_touch()) else { return false };
    let now = app.canvas.to_page(m.center_pos);
    if !r.contains(now) {
        return false;
    }
    ctx.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
    // The first finger is also the pointer: its drag becomes (or stays) a drag of the ruler,
    // resumed from here when one finger lifts; a stroke, shape or lasso it began is dropped.
    if let Some(p) = ui.input(|i| i.pointer.interact_pos()).map(|p| app.canvas.to_page(p)) {
        let down = ui.input(|i| i.pointer.primary_down());
        match app.canvas.ink.gesture.as_mut() {
            Some(Gesture::Ruler { last }) => *last = p,
            Some(Gesture::Stroke(_) | Gesture::Shape { .. } | Gesture::Lasso(_)) => app.canvas.ink.gesture = Some(Gesture::Ruler { last: p }),
            None if down => app.canvas.ink.gesture = Some(Gesture::Ruler { last: p }),
            _ => {}
        }
    }
    let before = app.canvas.to_page(m.center_pos - m.translation_delta);
    if m.rotation_delta == 0.0 && now == before {
        return true;
    }
    let (sin, cos) = m.rotation_delta.sin_cos();
    let (dx, dy) = (r.center.x - before.x, r.center.y - before.y);
    let (x, y) = (now.x + dx * cos - dy * sin, now.y + dx * sin + dy * cos);
    app.run(ctx, "draw.rulerPlace", json!({"x": x, "y": y, "turn": m.rotation_delta.to_degrees()}));
    true
}

/// The pen pressure of this frame: the latest touch force the platform reported (winit gives
/// one for pens on some platforms), else the last one seen in this gesture, else 0.5.
fn pressure(app: &mut NoteApp, ui: &Ui) -> f32 {
    if let Some(f) = ui.input(|i| i.events.iter().rev().find_map(|e| if let Event::Touch { force: Some(f), .. } = e { Some(*f) } else { None })) {
        app.canvas.ink.force = Some(f);
    }
    app.canvas.ink.force.unwrap_or(DEFAULT_PRESSURE)
}

/// Pointer handling for ink: the gestures of the Draw tools, and — with the Type tool — a click
/// on a stroke (selects it) or a drag of selected ink (moves it). Returns true when the event
/// was ink's, so the canvas doesn't also treat it as a click on text.
pub(crate) fn interact(app: &mut NoteApp, ui: &Ui, ctx: &egui::Context, resp: &Response) -> bool {
    let tool = app.session.draw.tool;
    if resp.hovered() {
        match tool {
            DrawTool::Pen | DrawTool::Lasso | DrawTool::Shape => ctx.set_cursor_icon(CursorIcon::Crosshair),
            DrawTool::Eraser => ctx.set_cursor_icon(CursorIcon::Cell),
            DrawTool::Space => ctx.set_cursor_icon(CursorIcon::ResizeVertical),
            DrawTool::Pan => ctx.set_cursor_icon(if resp.dragged() { CursorIcon::Grabbing } else { CursorIcon::Grab }),
            DrawTool::Type => {
                if let Some(h) = resp.hover_pos() {
                    if let Some((i, _)) = handle_at(app, h) {
                        ctx.set_cursor_icon(match i {
                            0 | 2 => CursorIcon::ResizeNwSe,
                            1 | 3 => CursorIcon::ResizeNeSw,
                            4 | 6 => CursorIcon::ResizeVertical,
                            _ => CursorIcon::ResizeHorizontal,
                        });
                    } else if selection_box(app).is_some_and(|b| b.contains(h)) || hit(app, h).is_some() {
                        ctx.set_cursor_icon(CursorIcon::Move);
                    }
                }
            }
        }
    }
    if ruler_wheel(app, ui, ctx, resp) || ruler_touch(app, ui, ctx) {
        return true;
    }
    let shift = ui.input(|i| i.modifiers.shift);
    let origin = if resp.drag_started() { ui.input(|i| i.pointer.press_origin()) } else { None };
    if resp.clicked() || resp.drag_started() {
        app.canvas.focused = true;
        app.session.sel.focus = notecraft_engine::selection::FocusArea::Canvas;
    }
    // A new contact forgets the last one's pressure (a mouse after a pen draws at 0.5).
    if ui.input(|i| i.pointer.primary_pressed()) {
        app.canvas.ink.force = None;
    }
    let pr = pressure(app, ui);
    // A checkbox tag on a stroke toggles on click (BEH.Tags.CheckboxClick).
    if resp.clicked()
        && matches!(tool, DrawTool::Type | DrawTool::Lasso)
        && let Some((id, i)) = resp.interact_pointer_pos().and_then(|p| checkbox_at(app, p))
    {
        app.run(ctx, "tag.click", json!({"block": id.to_string(), "tag": i}));
        return true;
    }
    // Start.
    if let Some(o) = origin.or_else(|| resp.clicked().then(|| resp.interact_pointer_pos()).flatten()) {
        let at = app.canvas.to_page(o);
        // On the ruler: a drag moves it and a click does nothing, except that a pen starting at
        // its edge draws (along the edge).
        if let Some(r) = app.session.draw.ruler
            && r.contains(at)
            && !(tool == DrawTool::Pen && r.snap(&[(at, pr)], 0.0).is_some())
        {
            if resp.drag_started() {
                app.canvas.ink.gesture = Some(Gesture::Ruler { last: at });
            }
            return true;
        }
        // Draw with Touch off (decision D8): a finger moves the page instead. A finger is a touch
        // without pressure; pens report pressure (winit gives touch force for them).
        if !app.session.draw.with_touch
            && matches!(tool, DrawTool::Pen | DrawTool::Eraser | DrawTool::Shape | DrawTool::Space | DrawTool::Lasso)
            && app.canvas.ink.force.is_none()
            && ui.input(|i| i.any_touches())
        {
            if resp.drag_started() {
                app.canvas.ink.gesture = Some(Gesture::Pan);
            }
            return true;
        }
        let in_box = selection_box(app).is_some_and(|b| b.contains(o));
        match tool {
            DrawTool::Pen => {
                if resp.drag_started() {
                    app.canvas.ink.gesture = Some(Gesture::Stroke(vec![(at, pr)]));
                } else {
                    // A tap is a dot.
                    app.run(ctx, "draw.stroke", json!({"points": [[at.x, at.y, pr]]}));
                }
            }
            DrawTool::Eraser => {
                app.canvas.ink.seq += 1;
                let seq = app.canvas.ink.seq;
                app.run(ctx, "draw.erase", json!({"points": [[at.x, at.y]], "gesture": seq}));
                if resp.drag_started() {
                    app.canvas.ink.gesture = Some(Gesture::Erase { seq, last: at });
                }
            }
            DrawTool::Pan => {
                if resp.drag_started() {
                    app.canvas.ink.gesture = Some(Gesture::Pan);
                }
            }
            DrawTool::Space => {
                if resp.drag_started() {
                    app.canvas.ink.gesture = Some(Gesture::Space { from: at, to: at });
                }
            }
            DrawTool::Shape => {
                if resp.drag_started() {
                    app.canvas.ink.gesture = Some(Gesture::Shape { from: at, to: at });
                }
            }
            DrawTool::Lasso | DrawTool::Type if resp.drag_started() && handle_at(app, o).is_some() => {
                if let Some((handle, start)) = handle_at(app, o) {
                    app.canvas.ink.gesture = Some(Gesture::Resize { handle, start, last: start, grab: at });
                }
            }
            DrawTool::Lasso if resp.drag_started() => {
                app.canvas.ink.gesture = Some(if in_box { Gesture::Group { last: at } } else { Gesture::Lasso(vec![at]) });
            }
            DrawTool::Lasso | DrawTool::Type => {
                let on = hit(app, o);
                if resp.drag_started() && (in_box || on.is_some()) {
                    if let Some(id) = on
                        && !in_box
                    {
                        app.run(ctx, "object.select", json!({"ids": [id.to_string()], "add": shift}));
                    }
                    app.canvas.ink.gesture = Some(Gesture::Group { last: at });
                } else if let Some(id) = on {
                    app.run(ctx, "object.select", json!({"ids": [id.to_string()], "add": shift}));
                } else {
                    return false;
                }
            }
        }
        return true;
    }
    // Continue.
    if resp.dragged() {
        let pts: Vec<Point> = moves(ui, resp).into_iter().map(|p| app.canvas.to_page(p)).collect();
        let min_step = 0.75 / (PT * app.canvas.zoom.max(0.1));
        match app.canvas.ink.gesture.as_mut() {
            Some(Gesture::Stroke(v)) => {
                for p in pts {
                    if v.last().is_none_or(|(q, _)| q.distance(p) >= min_step) && v.len() < notecraft_ink::MAX_SAMPLES {
                        v.push((p, pr));
                    }
                }
            }
            Some(Gesture::Lasso(v)) => {
                for p in pts {
                    if v.last().is_none_or(|q| q.distance(p) >= min_step * 2.0) && v.len() < notecraft_engine::MAX_DRAW_PATH {
                        v.push(p);
                    }
                }
            }
            Some(Gesture::Erase { seq, last }) => {
                let (seq, from) = (*seq, *last);
                if let Some(to) = pts.last().copied() {
                    *last = to;
                    let path: Vec<[f32; 2]> = std::iter::once(from).chain(pts).map(|p| [p.x, p.y]).collect();
                    app.run(ctx, "draw.erase", json!({"points": path, "gesture": seq}));
                }
            }
            Some(Gesture::Group { last }) => {
                if let Some(to) = pts.last().copied() {
                    let (dx, dy) = (to.x - last.x, to.y - last.y);
                    *last = to;
                    if dx != 0.0 || dy != 0.0 {
                        app.run(ctx, "object.move", json!({"dx": dx, "dy": dy}));
                    }
                }
            }
            Some(Gesture::Resize { handle, start, last, grab }) => {
                if let Some(to) = pts.last().copied() {
                    let next = crate::pictures::resized(*start, *handle, (to.x - grab.x, to.y - grab.y));
                    let from = std::mem::replace(last, next);
                    if from != next {
                        app.run(ctx, "object.resize", json!({"from": [from.0, from.1, from.2, from.3], "to": [next.0, next.1, next.2, next.3]}));
                    }
                }
            }
            Some(Gesture::Space { to, .. }) => {
                if let Some(p) = pts.last() {
                    *to = *p;
                }
            }
            Some(Gesture::Shape { from, to }) => {
                if let Some(p) = pts.last() {
                    *to = constrain(app.session.draw.shape, *from, *p, shift);
                }
            }
            Some(Gesture::Ruler { last }) => {
                if let Some(to) = pts.last().copied() {
                    let (dx, dy) = (to.x - last.x, to.y - last.y);
                    *last = to;
                    if dx != 0.0 || dy != 0.0 {
                        app.run(ctx, "draw.rulerPlace", json!({"dx": dx, "dy": dy}));
                    }
                }
            }
            Some(Gesture::Pan) => ui.scroll_with_delta(resp.drag_delta()),
            None => return false,
        }
        return true;
    }
    // Finish.
    if resp.drag_stopped() {
        match app.canvas.ink.gesture.take() {
            Some(Gesture::Stroke(v)) => {
                let pts: Vec<[f32; 3]> = v.iter().map(|(p, pr)| [p.x, p.y, *pr]).collect();
                app.run(ctx, "draw.stroke", json!({ "points": pts }));
            }
            Some(Gesture::Lasso(v)) => {
                let pts: Vec<[f32; 2]> = v.iter().map(|p| [p.x, p.y]).collect();
                app.run(ctx, "draw.lassoSelect", json!({ "points": pts }));
            }
            Some(Gesture::Shape { from, to }) => {
                app.run(ctx, "draw.shape", json!({"from": [from.x, from.y], "to": [to.x, to.y]}));
                if !app.session.draw.lock_shapes {
                    app.run(ctx, "draw.type", json!({}));
                }
            }
            Some(Gesture::Space { from, to }) => {
                // The longer way the pointer went decides: down/up inserts or removes rows of
                // space, sideways columns. Then back to typing (unverified).
                let (dx, dy) = (to.x - from.x, to.y - from.y);
                let p = if dx.abs() > dy.abs() { json!({"x": from.x, "dx": dx}) } else { json!({"y": from.y, "dy": dy}) };
                app.run(ctx, "canvas.insertSpace", p);
                app.run(ctx, "draw.type", json!({}));
            }
            Some(_) => {}
            None => return false,
        }
        return true;
    }
    false
}

/// A pen glyph for the gallery: a slanted pen (or a broad highlighter) whose tip and band carry
/// the pen's colour, drawn in code (no Microsoft artwork).
fn pen_glyph(p: &Painter, r: Rect, pen: PenStyle, t: Tokens) {
    let c = c32(pen.color);
    let body = if pen.tool == InkTool::Highlighter { 7.0 } else { 5.0 };
    let (top, tip) = (pos2(r.center().x + 3.0, r.min.y + 4.0), pos2(r.center().x - 3.0, r.max.y - 10.0));
    p.line_segment([top, tip], Stroke::new(body, t.icon));
    p.line_segment([top, top + (tip - top) * 0.3], Stroke::new(body, c));
    let n = (tip - top).normalized();
    let nib = tip + n * if pen.tool == InkTool::Highlighter { 3.0 } else { 5.0 };
    let side = vec2(-n.y, n.x) * (body / 2.0);
    p.add(Shape::convex_polygon(vec![tip + side, tip - side, nib], c, Stroke::NONE));
    // A sample of the line it draws, as thick as it draws (capped).
    let w = (pen.width * 0.6).clamp(1.0, 6.0);
    let y = r.max.y - 4.0;
    let line = if pen.tool == InkTool::Highlighter { c.gamma_multiply(HIGHLIGHTER_OPACITY) } else { c };
    p.line_segment([pos2(r.min.x + 4.0, y), pos2(r.max.x - 4.0, y)], Stroke::new(w, line));
}

/// The colours offered for a pen (black and the standard colours) or a highlighter (the text
/// highlight colours).
pub(crate) fn colors(tool: InkTool) -> Vec<(&'static str, notecraft_engine::doc::color::Rgba)> {
    use notecraft_engine::doc::color::{HIGHLIGHT_COLORS, Rgba, STANDARD_COLORS};
    match tool {
        InkTool::Pen => std::iter::once(("Black", Rgba::BLACK)).chain(STANDARD_COLORS.iter().map(|s| (s.name, s.color))).collect(),
        InkTool::Highlighter => HIGHLIGHT_COLORS.iter().map(|s| (s.name, s.color)).collect(),
    }
}

/// The quick colour and thickness picks shown under a pen's drop-down.
fn pen_options(app: &mut NoteApp, ui: &mut Ui, index: usize, t: Tokens) {
    let ctx = ui.ctx().clone();
    let Some(pen) = app.session.draw.pens.get(index).copied() else { return };
    ui.label(egui::RichText::new("Thickness").color(t.text_dim));
    let mms = if pen.tool == InkTool::Highlighter { HIGHLIGHTER_MM } else { PEN_MM };
    ui.horizontal_wrapped(|ui| {
        for mm in mms {
            let on = (pen.width - mm * MM).abs() < 0.05;
            if ui.selectable_label(on, format!("{mm} mm")).clicked() {
                app.run(&ctx, "draw.colorThickness", json!({"index": index, "width": mm * MM}));
            }
        }
    });
    ui.label(egui::RichText::new("Color").color(t.text_dim));
    crate::ribbon::swatch_grid(ui, &colors(pen.tool), 6, |c| {
        app.run(&ctx, "draw.colorThickness", json!({"index": index, "color": c.to_hex()}));
    });
}

/// The pen gallery (`R.Draw.Tools.InkStylesGallery`): the pens in a row, the selected one
/// marked; a click picks a pen, a click on the selected one opens its colour and thickness, and
/// the ▾ lists every pen with the Ink Mode controls (`extra`).
pub(crate) fn gallery(app: &mut NoteApp, ui: &mut Ui, h: f32, extra: &[&str], t: Tokens) {
    let ctx = ui.ctx().clone();
    let pens = app.session.draw.pens.clone();
    let current = app.session.draw.pen;
    let on_pen = app.session.draw.tool == DrawTool::Pen;
    let on = app.session.page().is_some();
    // Keep the selected pen in view.
    let first = current.saturating_sub(VISIBLE_PENS - 1).min(pens.len().saturating_sub(VISIBLE_PENS));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        for (i, pen) in pens.iter().enumerate().skip(first).take(VISIBLE_PENS) {
            let (rect, resp) = ui.allocate_exact_size(vec2(PEN_W, h), Sense::click());
            let name = format!("{} {}", if pen.tool == InkTool::Highlighter { "Highlighter" } else { "Pen" }, i + 1);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, on, &name));
            let p = ui.painter();
            if on_pen && i == current {
                p.rect_filled(rect, radius::CONTROL, t.checked);
            } else if resp.hovered() && on {
                p.rect_filled(rect, radius::CONTROL, t.hover);
            }
            pen_glyph(p, rect.shrink2(vec2(2.0, 4.0)), *pen, t);
            let selected_again = on_pen && i == current;
            let resp = resp.on_hover_text(format!("{name}: {:.2} mm", pen.width / MM));
            if resp.clicked() && on && !selected_again {
                app.run(&ctx, "draw.pen", json!({"index": i}));
            }
            if selected_again {
                egui::Popup::menu(&resp).show(|ui| pen_options(app, ui, i, t));
            }
        }
        let (rect, resp) = ui.allocate_exact_size(vec2(14.0, h), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, on, "More pens"));
        if resp.hovered() && on {
            ui.painter().rect_filled(rect, radius::CONTROL, t.hover);
        }
        crate::icons::chevron(ui.painter(), Rect::from_center_size(rect.center(), vec2(10.0, 10.0)), crate::icons::Dir::Down, t.icon);
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(200.0);
            for (i, pen) in pens.iter().enumerate() {
                let what = if pen.tool == InkTool::Highlighter { "Highlighter" } else { "Pen" };
                let label = format!("{what} {} — {:.2} mm {}", i + 1, pen.width / MM, pen.color.to_hex());
                if ui.selectable_label(on_pen && i == current, label).clicked() {
                    app.run(&ctx, "draw.pen", json!({"index": i}));
                }
            }
            ui.separator();
            for k in extra {
                crate::ribbon::menu_item(app, ui, k, t);
            }
        });
    });
}
