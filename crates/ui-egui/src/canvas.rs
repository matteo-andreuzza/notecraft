//! The page canvas: draws the current page and turns mouse input into engine commands.
//!
//! **Coordinates.** The page model is in page points (1/72 in, origin top-left; see
//! `notecraft-geom`). On screen, one page point is [`PT`] × zoom egui points (at 100 % zoom an
//! 11 pt font is as tall as in the reference at 96 dpi). [`CanvasState::to_screen`] and
//! [`CanvasState::to_page`] convert.
//!
//! **Layout.** Every frame lays out the title, the date line and each note container's visible
//! paragraphs with egui's text engine (one `LayoutJob` per paragraph, one section per run), and
//! keeps the result for hit testing and for the visual-line caret moves the keymap asks for
//! (Up/Down/Home/End on wrapped lines; catalog `NC.Caret.Up`, `KB.General.LineStart`).
//!
//! **Interaction** (catalog `BEH.Canvas.*`): a click in text places the caret (`caret.set`), a
//! drag selects, a click on empty canvas arms a new container there (`outline.new`), the
//! container's top bar moves it (`outline.move`), its right edge resizes it (`outline.resize`), a
//! checkbox tag toggles (`tag.click`), a paragraph handle selects, drags
//! (`outline.moveParagraph`) and on a double-click collapses (`outline.toggleCollapse`). In a
//! table, a click places the caret in a cell, a drag across cells selects them
//! (`table.selectRange`) and a column border drags to a new width (`table.columnWidth`).

use std::sync::Arc;

use egui::text::{CCursor, LayoutJob, TextFormat};
use egui::{Align, Align2, Color32, CursorIcon, FontId, Galley, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use notecraft_engine::doc::{CharFormat, Id, OutlineWidth, Page, PageObject, Paragraph, RuleLines, Script, Table};
use notecraft_engine::selection::{Block, PageSel};
use serde_json::json;

use crate::theme::c32;
use crate::{NoteApp, Tokens, fonts, icons};

/// egui points per page point at 100 % zoom (96 / 72).
pub const PT: f32 = 4.0 / 3.0;
/// Indent per outline level, page points.
pub const INDENT: f32 = 27.0;
/// Width reserved for a bullet or number, page points.
pub const LIST_W: f32 = 18.0;
/// Width per tag icon, page points.
pub const TAG_W: f32 = 16.0;
/// Margin around the page content, screen points.
const MARGIN: f32 = 0.0;

/// One laid-out paragraph (screen coordinates of the last frame).
#[derive(Clone)]
pub struct ParaLayout {
    /// Its block.
    pub block: Block,
    /// Paragraph index in the block.
    pub para: usize,
    /// The galley.
    pub galley: Arc<Galley>,
    /// Galley top-left on screen.
    pub pos: Pos2,
    /// The paragraph's text (for byte ↔ char conversion).
    pub text: String,
    /// Checkbox tag hit areas: (rect, index among checkbox tags).
    pub checkboxes: Vec<(Rect, usize)>,
    /// The paragraph handle area (left of the text).
    pub handle: Rect,
    /// An equation shown built up (decision D11): drawn instead of the galley, which still
    /// maps clicks to its linear text.
    pub(crate) math: Option<Arc<notecraft_math::MathBox>>,
}

impl ParaLayout {
    /// The paragraph's screen rectangle (galley bounds at its laid-out position).
    pub(crate) fn rect(&self) -> Rect {
        if let Some(m) = &self.math {
            return Rect::from_min_size(self.pos, vec2(m.width.max(4.0), m.height()));
        }
        Rect::from_min_size(self.pos, self.galley.size().max(vec2(4.0, self.galley.size().y)))
    }

    /// The byte offset nearest to a screen point.
    pub fn offset_at(&self, p: Pos2) -> usize {
        let c = self.galley.cursor_from_pos(p - self.pos);
        char_to_byte(&self.text, c.index.0)
    }

    /// The caret rectangle at a byte offset (screen).
    pub fn caret_rect(&self, offset: usize) -> Rect {
        let ci = byte_to_char(&self.text, offset);
        self.galley.pos_from_cursor(CCursor::new(ci)).translate(self.pos.to_vec2())
    }
}

/// One laid-out container.
#[derive(Clone, Copy)]
pub struct OutlineLayout {
    /// Outline id.
    pub id: Id,
    /// The frame around the content (screen).
    pub rect: Rect,
    /// The drag bar on top.
    pub bar: Rect,
    /// The resize handle on the right edge.
    pub resize: Rect,
    /// Left edge of level-0 text (screen x).
    pub left: f32,
    /// A table's frame (moves like a container, has no resize edge).
    pub table: bool,
}

/// One laid-out table cell (screen).
#[derive(Clone, Copy, Debug)]
pub struct CellLayout {
    /// The cell's id (its text block is `Block::Cell(id)`).
    pub id: Id,
    /// Row index.
    pub row: usize,
    /// Column index.
    pub col: usize,
    /// The cell's rectangle.
    pub rect: Rect,
}

/// One laid-out table (screen).
#[derive(Clone, Debug)]
pub struct TableLayout {
    /// The table's id.
    pub id: Id,
    /// The grid's outer rectangle.
    pub rect: Rect,
    /// Every cell, row by row.
    pub cells: Vec<CellLayout>,
    /// The x of each column's right border.
    pub col_edges: Vec<f32>,
}

impl TableLayout {
    /// The cell under a screen point.
    pub fn cell_at(&self, p: Pos2) -> Option<CellLayout> {
        self.cells.iter().find(|c| c.rect.contains(p)).copied()
    }

    /// The column whose right border is within a few points of `p` (for resizing).
    pub fn border_at(&self, p: Pos2) -> Option<(usize, f32)> {
        if p.y < self.rect.min.y || p.y > self.rect.max.y {
            return None;
        }
        let i = self.col_edges.iter().position(|x| (p.x - x).abs() <= 3.0)?;
        let left = if i == 0 { self.rect.min.x } else { self.col_edges.get(i - 1).copied().unwrap_or(self.rect.min.x) };
        Some((i, left))
    }
}

/// Padding inside a table cell, page points.
const CELL_PAD: f32 = 4.0;

/// Where a dragged paragraph would land (catalog `BEH.Outline.DragParagraph`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum ParaDrop {
    /// Into a container: `index` counts paragraphs after the dragged ones are taken out (what
    /// `outline.moveParagraph` expects); `line` is the drop indicator on screen.
    Into { to: Id, index: usize, level: u8, line: [Pos2; 2] },
    /// On empty canvas: a new container at this page point.
    NewAt(notecraft_engine::doc::geom::Point),
    /// Back where it was: nothing to do.
    Nowhere,
}

#[derive(Clone, Copy, PartialEq)]
enum Drag {
    Select {
        anchor: Option<notecraft_engine::selection::Pos>,
    },
    Move {
        id: Id,
        grab_page: notecraft_engine::doc::geom::Vec2,
    },
    Resize {
        id: Id,
        left: f32,
    },
    Para {
        from: Id,
        para: usize,
    },
    Column {
        table: Id,
        col: usize,
        left: f32,
    },
    /// A picture's sizing handle (`pictures::handles` order), from the page rectangle `start`.
    Picture {
        id: Id,
        handle: usize,
        start: (f32, f32, f32, f32),
        grab: notecraft_engine::doc::geom::Point,
    },
}

/// What a right-click context menu is about (`context_menu.rs`): set once when it opens on a
/// secondary click, read every frame while it stays open.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ContextTarget {
    /// A note container's drag bar (`CM.NoteContainer.*`).
    Container(Id),
    /// A paragraph's handle, left of its text (`CM.ParagraphHandle.*`).
    Handle(Block, usize),
    /// Text or a table cell, at a paragraph and byte offset (`CM.Text.*`).
    Text(Block, usize, usize),
    /// A picture (`CM.Picture.*`).
    Picture(Id),
    /// A file attachment (`CM.FileAttachment.*`).
    Attachment(Id),
    /// An ink stroke (`CM.InkDrawing.*`).
    Ink(Id),
    /// A stroke of a multi-object (lasso) selection (`CM.LassoSelection.*`).
    Lasso(Id),
}

/// Canvas view state.
pub struct CanvasState {
    /// Paragraph layouts of the last frame.
    pub paras: Vec<ParaLayout>,
    /// Misspelled words per paragraph text (`spell_ui`).
    pub(crate) spell: crate::spell_ui::SpellCache,
    /// Container layouts of the last frame (tables included, for their move bar).
    pub outlines: Vec<OutlineLayout>,
    /// Table layouts of the last frame.
    pub tables: Vec<TableLayout>,
    /// Pictures and attachments of the last frame, in z-order.
    pub media: Vec<crate::pictures::MediaLayout>,
    /// Decoded pictures (`pictures::TextureCache`).
    pub(crate) textures: crate::pictures::TextureCache,
    /// Fingerprint of the picture NoteCraft last put on the system clipboard (`clipboard.rs`).
    pub(crate) copied_image: Option<u64>,
    /// Ctrl+V detection when the clipboard holds no text (`clipboard::watch`).
    pub(crate) paste_probe: crate::clipboard::PasteProbe,
    /// Screen position of the page origin in the last frame.
    pub origin: Pos2,
    /// Zoom used in the last frame.
    pub zoom: f32,
    /// Whether the canvas has keyboard focus.
    pub focused: bool,
    /// When the caret last moved (for blinking).
    pub blink_from: f64,
    /// Desired x for consecutive Up/Down moves.
    pub desired_x: Option<f32>,
    /// IME composition in progress.
    pub preedit: String,
    /// Size of the page content in the last frame (screen points; the page grows right and
    /// down with what is on it, catalog `BEH.Canvas.PageGrows`).
    pub extent: egui::Vec2,
    /// The open right-click context menu's target, if any (`context_menu.rs`).
    pub(crate) context_target: Option<ContextTarget>,
    /// Ink strokes of the last frame and the Draw gesture in progress (`ink.rs`).
    pub ink: crate::ink::InkUi,
    drag: Option<Drag>,
    scroll_to: Option<Rect>,
    pub(crate) viewport: Rect,
}

impl Default for CanvasState {
    fn default() -> CanvasState {
        CanvasState {
            paras: Vec::new(),
            spell: Default::default(),
            outlines: Vec::new(),
            tables: Vec::new(),
            media: Vec::new(),
            textures: Default::default(),
            copied_image: None,
            paste_probe: Default::default(),
            origin: Pos2::ZERO,
            zoom: 1.0,
            focused: false,
            blink_from: 0.0,
            desired_x: None,
            preedit: String::new(),
            extent: egui::Vec2::ZERO,
            context_target: None,
            ink: Default::default(),
            drag: None,
            scroll_to: None,
            viewport: Rect::NOTHING,
        }
    }
}

impl CanvasState {
    /// Page point → screen.
    pub fn to_screen(&self, p: notecraft_engine::doc::geom::Point) -> Pos2 {
        let z = self.zoom.max(0.1) * PT;
        pos2(self.origin.x + p.x * z, self.origin.y + p.y * z)
    }

    /// Screen → page point.
    pub fn to_page(&self, p: Pos2) -> notecraft_engine::doc::geom::Point {
        let z = self.zoom.max(0.1) * PT;
        notecraft_engine::doc::geom::Point::new((p.x - self.origin.x) / z, (p.y - self.origin.y) / z)
    }

    /// Restart the caret blink (call after the caret moves).
    pub fn caret_moved(&mut self, ctx: &egui::Context) {
        self.blink_from = ctx.input(|i| i.time);
    }

    /// The layout of a paragraph.
    pub fn para(&self, block: Block, para: usize) -> Option<&ParaLayout> {
        self.paras.iter().find(|p| p.block == block && p.para == para)
    }

    /// Whether a drag (selection, move, resize, paragraph or column) is in progress: UI that
    /// floats near the selection (the mini toolbar) stays hidden while one is, so it doesn't
    /// flicker under the pointer.
    pub(crate) fn is_dragging(&self) -> bool {
        self.drag.is_some() || self.ink.busy()
    }
}

fn byte_to_char(text: &str, byte: usize) -> usize {
    text.get(..byte.min(text.len())).map(|s| s.chars().count()).unwrap_or_else(|| text.chars().count())
}

fn char_to_byte(text: &str, ci: usize) -> usize {
    text.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(text.len())
}

/// The egui text format for a run's effective format.
pub(crate) fn text_format(ctx: &egui::Context, f: &CharFormat, zoom: f32, t: Tokens, page_text: Color32) -> TextFormat {
    let size = f.size.unwrap_or(11.0).clamp(1.0, 1638.0);
    let script = f.script.unwrap_or(Script::Normal);
    let shown = if script == Script::Normal { size } else { size * 0.65 };
    let bold = f.bold == Some(true);
    let font = f.font.clone().unwrap_or_default();
    let mut tf = TextFormat {
        font_id: FontId::new((shown * PT * zoom).max(1.0), fonts::family(ctx, &font, bold)),
        color: f.color.map(c32).filter(|c| *c != Color32::BLACK || page_text == Color32::BLACK).unwrap_or(page_text),
        italics: f.italic == Some(true),
        ..Default::default()
    };
    if let Some(h) = f.highlight {
        tf.background = c32(h);
        // Automatic text stays readable on the highlight: dark on yellow in the dark theme too.
        // An explicit font colour is kept.
        if tf.color == page_text {
            tf.color = crate::theme::text_on(tf.background);
        }
    }
    if f.link.is_some() {
        tf.color = t.link;
        tf.underline = Stroke::new(1.0, t.link);
    }
    if f.underline == Some(true) {
        tf.underline = Stroke::new((zoom * 1.0).max(1.0), tf.color);
    }
    if f.strike == Some(true) {
        tf.strikethrough = Stroke::new((zoom * 1.0).max(1.0), tf.color);
    }
    tf.valign = match script {
        Script::Super => Align::TOP,
        Script::Sub | Script::Normal => Align::BOTTOM,
    };
    tf
}

/// Lay out one paragraph's text.
fn layout_para(
    ctx: &egui::Context,
    p: &Paragraph,
    wrap: f32,
    zoom: f32,
    t: Tokens,
    page_text: Color32,
    preedit: Option<(usize, &str)>,
) -> Arc<Galley> {
    let base = p.style.def().char;
    let mut job = LayoutJob { wrap: egui::text::TextWrapping { max_width: wrap.max(8.0), ..Default::default() }, ..Default::default() };
    let line_h = if p.line_spacing > 0.0 { Some(p.line_spacing * PT * zoom) } else { None };
    let mut push = |text: &str, f: &CharFormat, extra_underline: bool| {
        let mut tf = text_format(ctx, f, zoom, t, page_text);
        tf.line_height = line_h;
        if extra_underline {
            tf.underline = Stroke::new(1.0, tf.color);
        }
        job.append(text, 0.0, tf);
    };
    let mut pos = 0;
    let mut inserted = false;
    for r in &p.runs {
        let eff = r.fmt.over(&base);
        let end = pos + r.text.len();
        if let Some((at, pre)) = preedit
            && !inserted
            && at >= pos
            && at <= end
        {
            let (a, b) = (r.text.get(..at - pos).unwrap_or(""), r.text.get(at - pos..).unwrap_or(""));
            push(a, &eff, false);
            push(pre, &eff, true);
            push(b, &eff, false);
            inserted = true;
        } else {
            push(&r.text, &eff, false);
        }
        pos = end;
    }
    if !inserted && let Some((_, pre)) = preedit {
        push(pre, &base, true);
    }
    if job.text.is_empty() {
        // An empty paragraph still has a line of its style's height.
        let mut tf = text_format(ctx, &base, zoom, t, page_text);
        tf.line_height = line_h;
        job.append("", 0.0, tf);
    }
    ctx.fonts_mut(|f| f.layout_job(job))
}

/// Draw the canvas.
pub fn show(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    let Some(page) = app.session.page().cloned() else {
        let r = ui.available_rect_before_wrap();
        ui.painter().rect_filled(r, 0.0, t.page);
        let msg = if app.session.section().is_some() { "This section has no pages. Press Ctrl+N or click Add Page." } else { "" };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, msg, FontId::proportional(14.0), t.page_meta);
        return;
    };
    let zoom = app.session.view.zoom.clamp(0.1, 4.0);
    let page_bg = page.setup.color.map(c32).unwrap_or(t.page);
    let page_text = if page.setup.color.is_some() { crate::theme::text_on(page_bg) } else { t.page_text };
    let avail = ui.available_rect_before_wrap();
    // Scroll requests from the engine (Page Up/Down, Ctrl+Home/End, find).
    let mut scroll_req = app.session.scroll_request.take();
    egui::ScrollArea::both().id_salt(("canvas", page.id)).auto_shrink([false, false]).show_viewport(ui, |ui, viewport| {
        let min = ui.min_rect().min;
        app.canvas.origin = min + vec2(MARGIN, MARGIN);
        app.canvas.zoom = zoom;
        app.canvas.viewport = viewport.translate(min.to_vec2());
        // Lay out everything first (needed for the content size).
        let (paras, outlines, tables, mut extent) = layout_page(app, &ctx, &page, zoom, t, page_text);
        let mut media = crate::pictures::layout(&page, app.canvas.origin, zoom);
        crate::pictures::stack(&page, &mut media, &outlines);
        for m in &media {
            extent = extent.max(m.rect.max - app.canvas.origin + vec2(240.0, 240.0));
        }
        app.canvas.ink.layout = crate::ink::layout(app, &page);
        for k in &app.canvas.ink.layout {
            extent = extent.max(k.rect.max - app.canvas.origin + vec2(240.0, 240.0));
        }
        app.canvas.extent = extent;
        let content = vec2(extent.x.max(avail.width()), extent.y.max(avail.height()));
        let (rect, resp) = ui.allocate_exact_size(content, Sense::click_and_drag());
        let painter = ui.painter_at(rect.union(app.canvas.viewport));
        painter.rect_filled(rect, 0.0, page_bg);
        rule_lines(&painter, &page, rect, app.canvas.origin, zoom, t);
        paper_marks(&painter, &page, rect, app.canvas.origin, zoom, t);
        // Background pictures sit under everything, the title included.
        crate::pictures::draw(app, &painter, &page, &media, crate::pictures::Layer::Background, t);
        title_area(app, &painter, &page, zoom, t, page_text, &paras);
        // Containers: frame for the hovered one and the one with the caret.
        let hover = ctx.pointer_hover_pos();
        let caret_outline = match app.session.sel.caret() {
            Some(c) => match c.block {
                Block::Outline(id) => Some(id),
                Block::Cell(cell) => page.table_of_cell(cell).map(|(tb, _, _)| tb.id),
                Block::Title => None,
            },
            None => None,
        };
        let selected_objects: Vec<Id> = match &app.session.sel.on_page {
            PageSel::Objects { ids } => ids.clone(),
            _ => Vec::new(),
        };
        // Pictures and attachments under the text (text stays readable and clickable where they
        // overlap), unless they are in front of it (`pictures::stack`).
        crate::pictures::draw(app, &painter, &page, &media, crate::pictures::Layer::Under, t);
        // Tables: shading, borders, selected cells.
        for tl in &tables {
            let Some(tb) = page.table(tl.id) else { continue };
            let hot = hover.is_some_and(|h| tl.rect.expand(6.0).contains(h)) || caret_outline == Some(tl.id);
            draw_table(&painter, tb, tl, &app.session.sel.on_page, hot, t);
        }
        for o in &outlines {
            // OPT.Display.ShowNoteContainers: off, a frame shows only on a selected container.
            let near = hover.is_some_and(|h| o.rect.expand(6.0).contains(h)) || caret_outline == Some(o.id);
            let hot = (near && app.session.prefs.show_note_containers) || selected_objects.contains(&o.id);
            if hot {
                painter.rect_stroke(o.rect, 0.0, Stroke::new(1.0, t.container), egui::StrokeKind::Outside);
                painter.rect_filled(o.bar, 0.0, t.container.gamma_multiply(0.6));
                // Grip dots on the bar.
                for i in 0..5 {
                    painter.circle_filled(pos2(o.bar.center().x - 8.0 + i as f32 * 4.0, o.bar.center().y), 0.9, t.page_meta);
                }
                if !o.table {
                    painter.rect_filled(o.resize, 0.0, t.container.gamma_multiply(0.5));
                }
            }
            if selected_objects.contains(&o.id) {
                painter.rect_filled(o.rect, 0.0, t.selection);
            }
        }
        // An equation being edited shows its linear text in a light box (decision D11).
        for pl in paras.iter().filter(|pl| pl.math.is_none() && pl.block != Block::Title) {
            if notecraft_engine::Session::block_paras(&page, pl.block).and_then(|ps| ps.get(pl.para)).is_some_and(|p| p.math.is_some()) {
                painter.rect_filled(pl.rect().expand2(vec2(4.0, 2.0)), crate::theme::radius::CONTROL, t.hover);
            }
        }
        // Selection and find highlights behind the text.
        highlights(app, &painter, &paras, t);
        let defs = app.session.tag_defs();
        let mut labels_cache: std::collections::HashMap<Block, Vec<String>> = std::collections::HashMap::new();
        for o in &page.objects {
            match o {
                PageObject::Outline(ol) => {
                    labels_cache.insert(Block::Outline(ol.id), notecraft_engine::text::list_labels(&ol.paragraphs));
                }
                PageObject::Table(tb) => {
                    for c in tb.rows.iter().flat_map(|r| r.cells.iter()) {
                        labels_cache.insert(Block::Cell(c.id), notecraft_engine::text::list_labels(&c.paragraphs));
                    }
                }
                _ => {}
            }
        }
        for pl in &paras {
            match &pl.math {
                Some(m) => crate::math_ui::draw(&ctx, &painter, pl.pos, m, page_text),
                None => {
                    painter.galley(pl.pos, pl.galley.clone(), page_text);
                    // OPT.Proofing.CheckAsYouType: misspelled words get a wavy underline.
                    for r in crate::spell_ui::ranges(app, &pl.text) {
                        crate::spell_ui::underline(&painter, pl, r);
                    }
                }
            }
            if pl.block != Block::Title
                && let Some(para) = notecraft_engine::Session::block_paras(&page, pl.block).and_then(|ps| ps.get(pl.para))
            {
                let first_row_h = pl.galley.rows.first().map(|r| r.height()).unwrap_or(14.0);
                // Bullet or number.
                let label = labels_cache.get(&pl.block).and_then(|l| l.get(pl.para)).cloned().unwrap_or_default();
                let has_label = !label.is_empty();
                if has_label {
                    let size = (para.effective_fmt_at(0).size.unwrap_or(11.0) * PT * zoom).max(6.0);
                    let lx = pl.pos.x - LIST_W * PT * zoom * 0.95;
                    match para.list {
                        Some(notecraft_engine::doc::ListFormat::Bullet(b)) => {
                            icons::bullet(&painter, pos2(lx + size * 0.3, pl.pos.y + first_row_h / 2.0), size, b, page_text);
                        }
                        _ => {
                            painter.text(
                                pos2(lx, pl.pos.y + first_row_h / 2.0),
                                Align2::LEFT_CENTER,
                                label,
                                FontId::proportional(size * 0.95),
                                page_text,
                            );
                        }
                    }
                }
                // Tag icons (and tag colours: a highlight tag tints the paragraph).
                for (k, tag) in para.tags.iter().enumerate() {
                    let sz = (14.0 * zoom).clamp(8.0, 40.0);
                    let x = pl.pos.x - (if has_label { LIST_W } else { 0.0 } + TAG_W * (para.tags.len() - k) as f32) * PT * zoom;
                    let r = Rect::from_min_size(pos2(x, pl.pos.y + (first_row_h - sz) / 2.0), vec2(sz, sz));
                    let symbol = defs.iter().find(|d| d.key == tag.key).map(|d| d.symbol).unwrap_or(tag.symbol);
                    icons::tag(&painter, r, symbol, tag.checked, page_text);
                }
                // Collapsed paragraphs show a plus in the handle column.
                if para.collapsed {
                    icons::plus_circle(&painter, pl.handle.center(), (5.0 * zoom).max(3.5), t.accent);
                }
            }
        }
        // Paragraph handle on hover (UI.Canvas.ParagraphHandle).
        if let Some(h) = hover
            && let Some(pl) = paras
                .iter()
                .find(|p| matches!(p.block, Block::Outline(_)) && Rect::from_min_max(p.handle.min, pos2(p.rect().max.x, p.rect().max.y)).contains(h))
        {
            icons::move_handle(&painter, pl.handle.center(), (10.0 * zoom).max(7.0), t.page_meta);
        }
        // Pending click-to-type position.
        if let Some(pt) = app.session.pending_outline {
            let p = app.canvas.to_screen(pt);
            blink_caret(app, &painter, &ctx, Rect::from_min_size(p, vec2(1.5, 15.0 * PT * zoom)), page_text);
        }
        // The caret.
        if let Some(r) = app.session.text_range()
            && r.is_caret()
            && let Some(pl) = paras.iter().find(|p| p.block == r.head.block && p.para == r.head.para)
        {
            let mut cr = pl.caret_rect(r.head.offset);
            cr.max.x = cr.min.x + 1.5;
            blink_caret(app, &painter, &ctx, cr, page_text);
            // IME: tell the platform where the caret is (UI.Window.IME).
            if app.canvas.focused {
                ctx.output_mut(|o| {
                    o.ime = Some(egui::output::IMEOutput {
                        purpose: egui::IMEPurpose::Normal,
                        rect: pl.rect(),
                        cursor_rect: cr,
                        should_interrupt_composition: false,
                    });
                });
            }
            if matches!(scroll_req, Some(notecraft_engine::ScrollRequest::Caret)) {
                ui.scroll_to_rect(cr.expand(40.0), None);
                scroll_req = None;
            }
        }
        // Pictures and attachments in front of the text they overlap.
        crate::pictures::draw(app, &painter, &page, &media, crate::pictures::Layer::Over, t);
        // Ink, above the text (decision D4), with the gesture in progress and the selection box.
        crate::ink::draw(app, &painter, &page, t, page_text);
        // Selected pictures and attachments: frame and sizing handles, above the text.
        for m in media.iter().filter(|m| selected_objects.contains(&m.id)) {
            crate::pictures::draw_selection(&painter, m, t);
        }
        if let Some(h) = hover
            && app.canvas.drag.is_none()
            && let Some(m) = media.iter().rev().find(|m| !m.background && m.rect.contains(h))
            && let Some(tip) = crate::pictures::tooltip(&page, m.id)
        {
            resp.clone().on_hover_text_at_pointer(tip);
        }
        // Drag ghost for a container being moved is the container itself (moved live).
        app.canvas.paras = paras;
        app.canvas.outlines = outlines;
        app.canvas.tables = tables;
        app.canvas.media = media;
        // The drop indicator of a paragraph being dragged by its handle.
        if let Some(Drag::Para { from, para }) = app.canvas.drag
            && let Some(h) = ctx.pointer_latest_pos()
        {
            match para_drop(app, &page, from, para, h) {
                ParaDrop::Into { line, .. } => {
                    painter.line_segment(line, Stroke::new(2.0, t.accent));
                    painter.circle_filled(line[0], 3.0, t.accent);
                }
                ParaDrop::NewAt(_) => {
                    let r = Rect::from_min_size(h + vec2(8.0, 4.0), vec2(48.0, 16.0) * zoom.max(0.5));
                    painter.rect_stroke(r, crate::theme::radius::CONTROL, Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
                }
                ParaDrop::Nowhere => {}
            }
            ctx.set_cursor_icon(CursorIcon::Grabbing);
        }
        interact(app, ui, &ctx, &resp);
        crate::pictures::handle_drops(app, &ctx);
        crate::context_menu::show(app, ui, &resp, t);
        if let Some(r) = scroll_req {
            let vp = app.canvas.viewport;
            let target = match r {
                notecraft_engine::ScrollRequest::PageUp => vp.translate(vec2(0.0, -vp.height() * 0.9)),
                notecraft_engine::ScrollRequest::PageDown => vp.translate(vec2(0.0, vp.height() * 0.9)),
                notecraft_engine::ScrollRequest::Top => Rect::from_min_size(rect.min, vp.size()),
                notecraft_engine::ScrollRequest::Bottom => Rect::from_min_size(pos2(rect.min.x, rect.max.y - vp.height()), vp.size()),
                notecraft_engine::ScrollRequest::Caret => vp,
            };
            ui.scroll_to_rect(target, Some(Align::Min));
        }
        if let Some(r) = app.canvas.scroll_to.take() {
            ui.scroll_to_rect(r, None);
        }
    });
}

/// The page content size of the last frame (screen points).
pub fn page_extent(app: &NoteApp) -> egui::Vec2 {
    app.canvas.extent
}

fn blink_caret(app: &NoteApp, painter: &egui::Painter, ctx: &egui::Context, r: Rect, color: Color32) {
    let now = ctx.input(|i| i.time);
    let phase = ((now - app.canvas.blink_from) / 0.53) as i64;
    if !app.canvas.focused || phase % 2 == 0 {
        painter.rect_filled(r, 0.0, color);
    }
    if app.canvas.focused {
        let next = 0.53 - ((now - app.canvas.blink_from) % 0.53);
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(next.max(0.05)));
    }
}

/// Lay out the title and every visible paragraph; returns the layouts and the content extent.
fn layout_page(
    app: &NoteApp,
    ctx: &egui::Context,
    page: &Page,
    zoom: f32,
    t: Tokens,
    page_text: Color32,
) -> (Vec<ParaLayout>, Vec<OutlineLayout>, Vec<TableLayout>, egui::Vec2) {
    let z = PT * zoom;
    let origin = app.canvas.origin;
    let mut paras = Vec::new();
    let mut outlines = Vec::new();
    let mut tables = Vec::new();
    let mut extent = vec2(0.0, 0.0);
    let caret = app.session.sel.caret();
    let preedit_at = |block: Block, para: usize| -> Option<(usize, &str)> {
        let c = caret?;
        (c.block == block && c.para == para && !app.canvas.preedit.is_empty()).then_some((c.offset, app.canvas.preedit.as_str()))
    };
    // Equations (decision D11) show built up, except while the selection is in them: then their
    // linear text is edited in place.
    let range = app.session.text_range();
    let editing = |block: Block, para: usize| range.is_some_and(|r| [r.anchor, r.head].iter().any(|p| p.block == block && p.para == para));
    let math_of = |block: Block, i: usize, para: &Paragraph| -> Option<Arc<notecraft_math::MathBox>> {
        if para.math != Some(notecraft_engine::doc::MathView::Professional) || editing(block, i) {
            return None;
        }
        let size = (para.effective_fmt_at(0).size.unwrap_or(11.0) * z).max(4.0);
        Some(Arc::new(crate::math_ui::layout(ctx, &para.text(), size)))
    };
    // Title (BEH.Pages.Title, UI.Canvas.TitleArea).
    if !page.setup.hide_title {
        let tx = 36.0;
        let ty = 30.0;
        let wrap = 560.0 * z;
        let g = layout_para(ctx, &page.title, wrap, zoom, t, page_text, preedit_at(Block::Title, 0));
        let pos = pos2(origin.x + tx * z, origin.y + ty * z);
        extent = extent.max(vec2(pos.x - origin.x + g.size().x + 200.0, pos.y - origin.y + g.size().y + 120.0));
        paras.push(ParaLayout {
            block: Block::Title,
            para: 0,
            galley: g,
            pos,
            text: page.title.text(),
            checkboxes: Vec::new(),
            handle: Rect::from_min_size(pos - vec2(14.0, 0.0), vec2(12.0, 12.0)),
            math: None,
        });
    }
    for o in &page.objects {
        let ol = match o {
            PageObject::Outline(ol) => ol,
            // Pictures and attachments: `pictures::layout`; ink: `ink::draw`.
            PageObject::Image(_) | PageObject::Attachment(_) | PageObject::Ink(_) => continue,
            PageObject::Table(tb) => {
                let (tl, frame) = layout_table(ctx, tb, origin, zoom, t, page_text, &preedit_at, &math_of, &mut paras);
                extent = extent.max(vec2(frame.rect.max.x - origin.x + 240.0, frame.rect.max.y - origin.y + 240.0));
                tables.push(tl);
                outlines.push(frame);
                continue;
            }
        };
        let x0 = origin.x + ol.pos.x * z;
        let y0 = origin.y + ol.pos.y * z;
        let max_w = match ol.width {
            OutlineWidth::Fixed(w) => w,
            OutlineWidth::Auto => notecraft_engine::doc::page::AUTO_WIDTH_MAX,
        };
        let mut y = y0;
        let mut right: f32 = x0 + 40.0 * z;
        let labels = notecraft_engine::text::list_labels(&ol.paragraphs);
        for (i, para) in ol.paragraphs.iter().enumerate() {
            if ol.is_hidden(i) {
                continue;
            }
            let has_list = labels.get(i).is_some_and(|l| !l.is_empty());
            let lead = para.level as f32 * INDENT + para.tags.len() as f32 * TAG_W + if has_list { LIST_W } else { 0.0 };
            let wrap = ((max_w - lead).max(24.0)) * z;
            y += para.space_before * z;
            let g = layout_para(ctx, para, wrap, zoom, t, page_text, preedit_at(Block::Outline(ol.id), i));
            let tx = x0 + lead * z;
            let pos = pos2(tx, y);
            let math = math_of(Block::Outline(ol.id), i, para);
            let (w, h) = match &math {
                Some(m) => (m.width, m.height()),
                None => (g.size().x, g.size().y),
            };
            let mut checkboxes = Vec::new();
            let mut n_cb = 0;
            for (k, tag) in para.tags.iter().enumerate() {
                if tag.symbol.is_checkbox() {
                    let sz = (14.0 * zoom).clamp(8.0, 40.0);
                    let x = tx - (if has_list { LIST_W } else { 0.0 } + TAG_W * (para.tags.len() - k) as f32) * z;
                    let first_row_h = g.rows.first().map(|r| r.height()).unwrap_or(14.0);
                    checkboxes.push((Rect::from_min_size(pos2(x, y + (first_row_h - sz) / 2.0), vec2(sz, sz)), n_cb));
                    n_cb += 1;
                }
            }
            let handle_x = x0 + para.level as f32 * INDENT * z - 14.0 * zoom;
            let handle = Rect::from_min_size(pos2(handle_x, y), vec2(12.0 * zoom.max(0.6), g.rows.first().map(|r| r.height()).unwrap_or(14.0)));
            right = right.max(tx + w);
            paras.push(ParaLayout { block: Block::Outline(ol.id), para: i, galley: g, pos, text: para.text(), checkboxes, handle, math });
            y += h + para.space_after * z;
        }
        let width_px = match ol.width {
            OutlineWidth::Fixed(w) => w * z,
            OutlineWidth::Auto => (right - x0).max(40.0 * z),
        };
        let pad = 4.0;
        let rect = Rect::from_min_max(pos2(x0 - pad - 16.0 * zoom, y0 - pad), pos2(x0 + width_px + pad, y.max(y0 + 14.0 * z) + pad));
        let bar = Rect::from_min_max(pos2(rect.min.x, rect.min.y - 9.0), pos2(rect.max.x, rect.min.y));
        let resize = Rect::from_min_max(pos2(rect.max.x - 3.0, rect.min.y), pos2(rect.max.x + 4.0, rect.max.y));
        outlines.push(OutlineLayout { id: ol.id, rect, bar, resize, left: x0, table: false });
        extent = extent.max(vec2(rect.max.x - origin.x + 240.0, rect.max.y - origin.y + 240.0));
    }
    (paras, outlines, tables, extent)
}

/// Lay out a table: each cell's paragraphs wrap at the column width; a row is as tall as its
/// tallest cell. Returns the table's layout and its frame (for the move bar).
fn layout_table<'a>(
    ctx: &egui::Context,
    tb: &Table,
    origin: Pos2,
    zoom: f32,
    t: Tokens,
    page_text: Color32,
    preedit_at: &dyn Fn(Block, usize) -> Option<(usize, &'a str)>,
    math_of: &dyn Fn(Block, usize, &Paragraph) -> Option<Arc<notecraft_math::MathBox>>,
    paras: &mut Vec<ParaLayout>,
) -> (TableLayout, OutlineLayout) {
    let z = PT * zoom;
    let x0 = origin.x + tb.pos.x * z;
    let y0 = origin.y + tb.pos.y * z;
    let mut xs = Vec::with_capacity(tb.columns.len() + 1);
    let mut x = x0;
    xs.push(x);
    for w in &tb.columns {
        x += w * z;
        xs.push(x);
    }
    let pad = CELL_PAD * z;
    let n_cols = tb.columns.len();
    // Merged cells (decision D9): a cell that shows spans its columns' widths and rows' heights;
    // the places it covers aren't laid out.
    let grid = tb.span_grid();
    struct Laid {
        id: Id,
        row: usize,
        col: usize,
        span: (usize, usize),
        paras: Vec<(usize, std::sync::Arc<egui::Galley>, f32, f32, String, Option<Arc<notecraft_math::MathBox>>)>,
        height: f32,
    }
    let mut laid: Vec<Laid> = Vec::new();
    for (r, row) in tb.rows.iter().enumerate() {
        for (c, cell) in row.cells.iter().enumerate() {
            let Some((rs, cs)) = grid.get(r).and_then(|g| g.get(c)).copied().flatten() else { continue };
            let cs = cs.min(n_cols.saturating_sub(c)).max(1);
            let (cx0, cx1) = (xs.get(c).copied().unwrap_or(x0), xs.get(c + cs).copied().unwrap_or(x0));
            let width = (cx1 - cx0) / z;
            let labels = notecraft_engine::text::list_labels(&cell.paragraphs);
            let mut cy = pad;
            let mut paras_here = Vec::with_capacity(cell.paragraphs.len());
            for (i, para) in cell.paragraphs.iter().enumerate() {
                let has_list = labels.get(i).is_some_and(|l| !l.is_empty());
                let lead = para.level as f32 * INDENT + para.tags.len() as f32 * TAG_W + if has_list { LIST_W } else { 0.0 };
                let wrap = (width - 2.0 * CELL_PAD - lead).max(8.0) * z;
                cy += para.space_before * z;
                let g = layout_para(ctx, para, wrap, zoom, t, page_text, preedit_at(Block::Cell(cell.id), i));
                let math = math_of(Block::Cell(cell.id), i, para);
                let h = math.as_ref().map_or(g.size().y, |m| m.height());
                paras_here.push((i, g, lead * z, cy, para.text(), math));
                cy += h + para.space_after * z;
            }
            laid.push(Laid { id: cell.id, row: r, col: c, span: (rs, cs), paras: paras_here, height: cy + pad });
        }
    }
    // Row heights: one-row cells first, then a merged cell taller than its rows stretches the
    // last of them.
    let mut row_h = vec![14.0 * z; tb.rows.len()];
    for l in laid.iter().filter(|l| l.span.0 == 1) {
        if let Some(h) = row_h.get_mut(l.row) {
            *h = h.max(l.height);
        }
    }
    for l in laid.iter().filter(|l| l.span.0 > 1) {
        let last = (l.row + l.span.0).min(row_h.len()).saturating_sub(1);
        let have: f32 = row_h.iter().skip(l.row).take(l.span.0).sum();
        if have < l.height
            && let Some(h) = row_h.get_mut(last)
        {
            *h += l.height - have;
        }
    }
    let mut ys = Vec::with_capacity(row_h.len() + 1);
    let mut y = y0;
    ys.push(y);
    for h in &row_h {
        y += h;
        ys.push(y);
    }
    let mut cells = Vec::with_capacity(laid.len());
    for l in laid {
        let (cx0, cx1) = (xs.get(l.col).copied().unwrap_or(x0), xs.get(l.col + l.span.1).copied().unwrap_or(x0));
        let (top, bottom) = (ys.get(l.row).copied().unwrap_or(y0), ys.get(l.row + l.span.0).copied().unwrap_or(y));
        for (i, g, lead, oy, text, math) in l.paras {
            paras.push(ParaLayout {
                block: Block::Cell(l.id),
                para: i,
                galley: g,
                pos: pos2(cx0 + pad + lead, top + oy),
                text,
                checkboxes: Vec::new(),
                handle: Rect::NOTHING,
                math,
            });
        }
        cells.push(CellLayout { id: l.id, row: l.row, col: l.col, rect: Rect::from_min_max(pos2(cx0, top), pos2(cx1, bottom)) });
    }
    let right = xs.last().copied().unwrap_or(x0);
    let rect = Rect::from_min_max(pos2(x0, y0), pos2(right, y.max(y0 + 14.0 * z)));
    let frame_rect = rect.expand(4.0);
    let bar = Rect::from_min_max(pos2(frame_rect.min.x, frame_rect.min.y - 9.0), pos2(frame_rect.max.x, frame_rect.min.y));
    let frame = OutlineLayout { id: tb.id, rect: frame_rect, bar, resize: Rect::NOTHING, left: x0, table: true };
    (TableLayout { id: tb.id, rect, cells, col_edges: xs.into_iter().skip(1).collect() }, frame)
}

/// Draw a table's shading, borders and selected cells. Hidden borders show as faint lines
/// while the table is hovered or holds the caret, so its cells stay findable.
fn draw_table(painter: &egui::Painter, tb: &Table, tl: &TableLayout, sel: &PageSel, hot: bool, t: Tokens) {
    for cl in &tl.cells {
        if let Some(c) = tb.cell(cl.row, cl.col).and_then(|c| c.shading) {
            painter.rect_filled(cl.rect, 0.0, c32(c));
        }
    }
    let stroke = if tb.borders {
        Some(Stroke::new(1.0, t.table_border))
    } else if hot {
        Some(Stroke::new(1.0, t.container.gamma_multiply(0.7)))
    } else {
        None
    };
    if let Some(st) = stroke {
        // Cell by cell, so a merged cell has no lines across it.
        painter.rect_stroke(tl.rect, 0.0, st, egui::StrokeKind::Middle);
        for cl in &tl.cells {
            painter.rect_stroke(cl.rect, 0.0, st, egui::StrokeKind::Middle);
        }
    }
    if let PageSel::Cells { table, rows, cols } = sel
        && *table == tl.id
    {
        for cl in tl.cells.iter().filter(|c| c.row >= rows.0 && c.row <= rows.1 && c.col >= cols.0 && c.col <= cols.1) {
            painter.rect_filled(cl.rect.shrink(0.5), 0.0, t.selection);
        }
    }
}

/// Where the printed sheets of a page with a paper size end (View › Paper Size): a dashed line
/// at the printable width and one under each sheet's printable height, measured from the page
/// origin as the PDF export lays content out (unverified: the reference's on-screen marks).
fn paper_marks(painter: &egui::Painter, page: &Page, rect: Rect, origin: Pos2, zoom: f32, t: Tokens) {
    let Some(ps) = page.setup.paper else { return };
    let [mt, mr, mb, ml] = ps.margins;
    let (w, h) = ((ps.width - ml - mr).max(36.0) * PT * zoom, (ps.height - mt - mb).max(36.0) * PT * zoom);
    if !(w.is_finite() && h.is_finite()) || h < 8.0 {
        return;
    }
    let stroke = Stroke::new(1.0, t.page_meta);
    let x = origin.x + w;
    if x < rect.max.x {
        painter.extend(egui::Shape::dashed_line(&[pos2(x, rect.min.y), pos2(x, rect.max.y)], stroke, 6.0, 4.0));
    }
    let mut y = origin.y + h;
    // Bounded: a tall page at a small paper still draws a few hundred lines at most.
    let mut n = 0;
    while y < rect.max.y && n < 2_000 {
        painter.extend(egui::Shape::dashed_line(&[pos2(rect.min.x, y), pos2(x.min(rect.max.x), y)], stroke, 6.0, 4.0));
        y += h;
        n += 1;
    }
}

fn rule_lines(painter: &egui::Painter, page: &Page, rect: Rect, origin: Pos2, zoom: f32, t: Tokens) {
    let kind = page.setup.rule_lines;
    if kind == RuleLines::None {
        return;
    }
    let step = kind.spacing() * PT * zoom;
    if step < 2.0 {
        return;
    }
    let color = page.setup.rule_color.map(c32).unwrap_or(t.rule);
    let stroke = Stroke::new(1.0, color);
    // Ruled lines start below the title area; grids cover the page.
    let top = if kind.is_grid() { rect.min.y } else { origin.y + 86.0 * PT * zoom };
    let mut y = top;
    while y < rect.max.y {
        painter.line_segment([pos2(rect.min.x, y), pos2(rect.max.x, y)], stroke);
        y += step;
    }
    if kind.is_grid() {
        let mut x = rect.min.x;
        while x < rect.max.x {
            painter.line_segment([pos2(x, rect.min.y), pos2(x, rect.max.y)], stroke);
            x += step;
        }
    } else {
        // The margin line of ruled paper.
        let mx = origin.x + 24.0 * PT * zoom;
        painter.line_segment([pos2(mx, rect.min.y), pos2(mx, rect.max.y)], Stroke::new(1.0, t.margin_line));
    }
}

fn title_area(app: &NoteApp, painter: &egui::Painter, page: &Page, zoom: f32, t: Tokens, page_text: Color32, paras: &[ParaLayout]) {
    if page.setup.hide_title {
        return;
    }
    let Some(tl) = paras.iter().find(|p| p.block == Block::Title) else { return };
    let z = PT * zoom;
    // The underline below the title spans at least the default title width.
    let w = tl.galley.size().x.max(300.0 * zoom);
    let y = tl.pos.y + tl.galley.size().y + 3.0 * zoom;
    painter.line_segment([pos2(tl.pos.x, y), pos2(tl.pos.x + w, y)], Stroke::new(1.0, t.container));
    if page.title.is_empty() && app.session.sel.caret().is_none_or(|c| c.block != Block::Title) {
        painter.text(tl.pos, Align2::LEFT_TOP, "", FontId::proportional(20.0 * z), t.page_meta);
    }
    // The date and time under the title (BEH.Pages.DateTime).
    let off = app.session.clock.utc_offset_min;
    let shown = page.shown_time();
    let date = format!("{}      {}", shown.long_date(off), shown.short_time(off));
    painter.text(
        pos2(tl.pos.x, y + 5.0 * zoom),
        Align2::LEFT_TOP,
        date,
        FontId::proportional((9.5 * z).max(6.0)),
        if page.setup.color.is_some() { page_text } else { t.page_meta },
    );
}

fn range_rects(pl: &ParaLayout, a: usize, b: usize) -> Vec<Rect> {
    let (ca, cb) = (byte_to_char(&pl.text, a), byte_to_char(&pl.text, b));
    let mut out = Vec::new();
    let mut start = 0usize;
    for row in &pl.galley.rows {
        let n = row.char_count_including_newline().0;
        let (rs, re) = (start, start + row.char_count_excluding_newline().0);
        let (s, e) = (ca.max(rs), cb.min(re));
        if s < e || (s == e && ca < cb && s == re && row.ends_with_newline && cb > re) {
            let x0 = row.x_offset(egui::text::CharIndex(s - rs));
            let x1 = if e > s { row.x_offset(egui::text::CharIndex(e - rs)) } else { x0 + 4.0 };
            let r = row.rect();
            out.push(Rect::from_min_max(pos2(pl.pos.x + r.min.x + x0, pl.pos.y + r.min.y), pos2(pl.pos.x + r.min.x + x1, pl.pos.y + r.max.y)));
        }
        start += n;
    }
    out
}

fn highlights(app: &NoteApp, painter: &egui::Painter, paras: &[ParaLayout], t: Tokens) {
    for &(block, para, a, b) in &app.session.view.find.matches {
        if let Some(pl) = paras.iter().find(|p| p.block == block && p.para == para) {
            for r in range_rects(pl, a, b) {
                painter.rect_filled(r, 0.0, t.find);
            }
        }
    }
    let Some(r) = app.session.text_range() else { return };
    if r.is_caret() {
        return;
    }
    let (a, b) = r.ordered();
    for pl in paras.iter().filter(|p| p.block == a.block && p.para >= a.para && p.para <= b.para) {
        let s = if pl.para == a.para { a.offset } else { 0 };
        let e = if pl.para == b.para { b.offset } else { pl.text.len() };
        let rects = range_rects(pl, s, e);
        if rects.is_empty() && s == e && pl.para != b.para {
            // An empty paragraph inside the selection: a small block.
            painter.rect_filled(Rect::from_min_size(pl.pos, vec2(6.0, pl.galley.size().y)), 0.0, t.selection);
        }
        for rr in rects {
            painter.rect_filled(rr, 0.0, t.selection);
        }
        // Selected line ends show a little extension like word processors.
        if pl.para != b.para {
            let end = pl.caret_rect(pl.text.len());
            painter.rect_filled(Rect::from_min_size(end.min, vec2(5.0, end.height())), 0.0, t.selection);
        }
    }
}

/// Mouse interaction.
/// Start dragging object `id`, grabbed at screen point `p` (kept at the same spot under the
/// pointer while it moves).
fn start_move(app: &mut NoteApp, id: Id, p: Pos2) {
    let pos = app.session.page().and_then(|pg| pg.objects.iter().find(|x| x.id() == id).map(PageObject::pos)).unwrap_or_default();
    let grab = app.canvas.to_page(p);
    app.canvas.drag = Some(Drag::Move { id, grab_page: notecraft_engine::doc::geom::Vec2::new(grab.x - pos.x, grab.y - pos.y) });
}

fn interact(app: &mut NoteApp, ui: &Ui, ctx: &egui::Context, resp: &egui::Response) {
    // The Draw tools, and clicks or drags on ink (`ink.rs`).
    if app.canvas.drag.is_none() && crate::ink::interact(app, ui, ctx, resp) {
        return;
    }
    let hover = resp.hover_pos();
    // Cursor shape.
    let selected: Vec<Id> = match &app.session.sel.on_page {
        PageSel::Objects { ids } => ids.clone(),
        _ => Vec::new(),
    };
    // A selected picture's sizing handle under a point: `(picture, handle index)`.
    let picture_handle = |app: &NoteApp, p: Pos2| {
        app.canvas
            .media
            .iter()
            .filter(|m| m.picture && selected.contains(&m.id))
            .find_map(|m| crate::pictures::handles(m.rect).iter().position(|h| h.expand(2.0).contains(p)).map(|i| (*m, i)))
    };
    if let Some(h) = hover {
        if let Some((_, i)) = picture_handle(app, h) {
            ctx.set_cursor_icon(crate::pictures::handle_cursor(i));
        } else if app.canvas.outlines.iter().any(|o| o.resize.contains(h)) || app.canvas.tables.iter().any(|tl| tl.border_at(h).is_some()) {
            ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
        } else if app.canvas.outlines.iter().any(|o| o.bar.contains(h)) {
            ctx.set_cursor_icon(CursorIcon::Move);
        } else if app.canvas.paras.iter().any(|p| p.block != Block::Title && p.handle.contains(h)) {
            ctx.set_cursor_icon(CursorIcon::Grab);
        } else if app.canvas.paras.iter().any(|p| p.rect().expand2(vec2(2.0, 0.0)).contains(h)) {
            ctx.set_cursor_icon(CursorIcon::Text);
        }
    }
    let shift = ui.input(|i| i.modifiers.shift);
    if resp.clicked() || resp.drag_started() {
        app.canvas.focused = true;
        app.session.sel.focus = notecraft_engine::selection::FocusArea::Canvas;
        app.canvas.desired_x = None;
        // A drag is recognised only after the pointer has moved a little: hit-test where the
        // button went down, not where the pointer is now.
        let origin = if resp.drag_started() { ui.input(|i| i.pointer.press_origin()) } else { None };
        let Some(p) = origin.or_else(|| resp.interact_pointer_pos()) else { return };
        // A selected picture's sizing handle (decision D3).
        if let Some((m, handle)) = picture_handle(app, p) {
            if resp.drag_started()
                && let Some(im) = app.session.page().and_then(|pg| pg.image(m.id))
            {
                let start = (im.pos.x, im.pos.y, im.width, im.height);
                app.canvas.drag = Some(Drag::Picture { id: m.id, handle, start, grab: app.canvas.to_page(p) });
            }
            return;
        }
        // Resize handle.
        if let Some(o) = app.canvas.outlines.iter().find(|o| o.resize.contains(p)).copied() {
            if resp.drag_started() {
                app.canvas.drag = Some(Drag::Resize { id: o.id, left: o.rect.min.x });
            }
            return;
        }
        // A table's column border: dragging sets the column's width.
        if let Some((tid, (col, left))) = app.canvas.tables.iter().find_map(|tl| tl.border_at(p).map(|b| (tl.id, b))) {
            if resp.drag_started() {
                app.canvas.drag = Some(Drag::Column { table: tid, col, left });
            }
            return;
        }
        // Move bar: selects the container (or table); dragging moves it.
        if let Some(o) = app.canvas.outlines.iter().find(|o| o.bar.contains(p)).copied() {
            app.run(ctx, "object.select", json!({"ids": [o.id.to_string()]}));
            if resp.drag_started() {
                start_move(app, o.id, p);
            }
            return;
        }
        // Checkbox tags.
        if resp.clicked()
            && let Some((pl, idx)) =
                app.canvas.paras.iter().find_map(|pl| pl.checkboxes.iter().find(|(r, _)| r.contains(p)).map(|(_, i)| (pl.clone(), *i)))
            && let Block::Outline(oid) = pl.block
        {
            app.run(ctx, "tag.click", json!({"block": oid.to_string(), "para": pl.para, "tag": idx}));
            return;
        }
        // Paragraph handle: click selects the paragraph with its children; double-click collapses.
        if let Some(pl) = app.canvas.paras.iter().find(|pl| pl.block != Block::Title && pl.handle.contains(p)).cloned()
            && let Block::Outline(oid) = pl.block
        {
            if resp.double_clicked() {
                app.run(ctx, "outline.toggleCollapse", json!({"block": oid.to_string(), "para": pl.para}));
            } else {
                app.run(ctx, "caret.set", json!({"block": oid.to_string(), "para": pl.para, "offset": 0}));
                app.run(ctx, "outline.selectBranch", json!({}));
            }
            // Dragging the handle moves the paragraph with its children (BEH.Outline.DragParagraph).
            if resp.drag_started() {
                app.canvas.drag = Some(Drag::Para { from: oid, para: pl.para });
            }
            return;
        }
        // A picture or attachment (not a background picture, and not where a container's text
        // is, unless it is drawn over it): click selects it (Shift adds), drag moves it,
        // double-click opens an attachment.
        if let Some(m) = crate::pictures::hit(app, p, false)
            && (m.above || !app.canvas.outlines.iter().any(|o| o.rect.contains(p)))
        {
            if !(selected.contains(&m.id) && resp.drag_started()) {
                app.run(ctx, "object.select", json!({"ids": [m.id.to_string()], "add": shift}));
            }
            if resp.double_clicked() && !m.picture {
                crate::pictures::host_action(app, ctx, "attachment.open");
            }
            if resp.drag_started() {
                start_move(app, m.id, p);
            }
            return;
        }
        // Text.
        if let Some(pl) = hit_para(app, p) {
            let off = pl.offset_at(p);
            let block = block_param(pl.block);
            app.run(ctx, "caret.set", json!({"block": block, "para": pl.para, "offset": off, "extend": shift}));
            if resp.double_clicked() {
                let w = notecraft_engine::text::word_at(&pl.text, off);
                app.run(ctx, "caret.set", json!({"block": block, "para": pl.para, "offset": w.start}));
                app.run(ctx, "caret.set", json!({"block": block, "para": pl.para, "offset": w.end, "extend": true}));
            } else if resp.triple_clicked() {
                app.run(ctx, "caret.set", json!({"block": block, "para": pl.para, "offset": 0}));
                app.run(ctx, "caret.set", json!({"block": block, "para": pl.para, "offset": pl.text.len(), "extend": true}));
            }
            if resp.drag_started() {
                app.canvas.drag = Some(Drag::Select { anchor: app.session.text_range().map(|r| r.anchor) });
            }
            return;
        }
        // Empty canvas: the next keystroke creates a container here (BEH.Canvas.ClickToType).
        if resp.clicked() {
            let pt = app.canvas.to_page(p);
            app.run(ctx, "outline.new", json!({"x": pt.x.max(0.0), "y": pt.y.max(0.0)}));
        }
    }
    if resp.dragged()
        && let Some(p) = resp.interact_pointer_pos()
    {
        match app.canvas.drag {
            Some(Drag::Select { anchor }) => {
                // From one cell into another of the same table: the cells are selected.
                if let Some(a) = anchor
                    && let Block::Cell(ac) = a.block
                    && let Some((tl, from, to)) = app.canvas.tables.iter().find_map(|tl| {
                        let from = tl.cells.iter().find(|c| c.id == ac)?;
                        let to = tl.cell_at(p)?;
                        Some((tl.id, *from, to))
                    })
                    && from.id != to.id
                {
                    app.run(ctx, "table.selectRange", json!({"table": tl.to_string(), "rows": [from.row, to.row], "cols": [from.col, to.col]}));
                } else if let Some(a) = anchor
                    && let Some(pl) =
                        app.canvas.paras.iter().filter(|pl| pl.block == a.block).min_by(|x, y| dist(x, p).total_cmp(&dist(y, p))).cloned()
                {
                    // Back inside the anchor's block: a text selection again.
                    if app.session.text_range().is_none_or(|r| r.anchor != a) {
                        app.run(ctx, "caret.set", json!({"block": block_param(a.block), "para": a.para, "offset": a.offset}));
                    }
                    let off = pl.offset_at(p);
                    app.run(ctx, "caret.set", json!({"block": block_param(pl.block), "para": pl.para, "offset": off, "extend": true}));
                }
            }
            Some(Drag::Column { table, col, left }) => {
                let w = (p.x - left) / (PT * app.canvas.zoom.max(0.1));
                app.run(ctx, "table.columnWidth", json!({"table": table.to_string(), "col": col, "width": w}));
            }
            Some(Drag::Move { id, grab_page }) => {
                let pt = app.canvas.to_page(p);
                app.run(ctx, "outline.move", json!({"id": id.to_string(), "x": pt.x - grab_page.x, "y": pt.y - grab_page.y}));
            }
            Some(Drag::Resize { id, left }) => {
                let w = (p.x - left) / (PT * app.canvas.zoom.max(0.1)) - 16.0;
                app.run(ctx, "outline.resize", json!({"id": id.to_string(), "width": w.max(notecraft_engine::doc::page::MIN_OUTLINE_WIDTH)}));
            }
            Some(Drag::Picture { id, handle, start, grab }) => {
                let pt = app.canvas.to_page(p);
                let (x, y, w, h) = crate::pictures::resized(start, handle, (pt.x - grab.x, pt.y - grab.y));
                app.run(ctx, "object.resize", json!({"id": id.to_string(), "x": x, "y": y, "width": w, "height": h}));
            }
            Some(Drag::Para { .. }) | None => {}
        }
    }
    if resp.drag_stopped() {
        if let Some(Drag::Para { from, para }) = app.canvas.drag
            && let Some(p) = resp.interact_pointer_pos().or_else(|| ctx.pointer_latest_pos())
            && let Some(page) = app.session.page().cloned()
        {
            match para_drop(app, &page, from, para, p) {
                ParaDrop::Into { to, index, level, .. } => {
                    app.run(
                        ctx,
                        "outline.moveParagraph",
                        json!({"from": from.to_string(), "para": para, "to": to.to_string(), "index": index, "level": level}),
                    );
                }
                ParaDrop::NewAt(pt) => {
                    app.run(ctx, "outline.moveParagraph", json!({"from": from.to_string(), "para": para, "x": pt.x, "y": pt.y}));
                }
                ParaDrop::Nowhere => {}
            }
        }
        app.canvas.drag = None;
    }
    // Clicking elsewhere (ribbon, panes) takes focus away from the page.
    if ui.input(|i| i.pointer.any_pressed()) && !resp.hovered() && !resp.contains_pointer() && hover.is_none() {
        let over_page = ctx.pointer_interact_pos().is_some_and(|p| resp.rect.contains(p) && app.canvas.viewport.contains(p));
        if !over_page {
            app.canvas.focused = false;
        }
    }
}

/// Where paragraph `para` of container `from` (with its children) lands when dropped at the
/// screen point `p`: before the visible paragraph whose middle is below the pointer, at the
/// level the pointer's x suggests (at most one deeper than the paragraph above), or in a new
/// container when the pointer is away from every container.
pub(crate) fn para_drop(app: &NoteApp, page: &Page, from: Id, para: usize, p: Pos2) -> ParaDrop {
    let Some(src) = page.outline(from) else { return ParaDrop::Nowhere };
    let moved = src.subtree(para);
    if moved.is_empty() {
        return ParaDrop::Nowhere;
    }
    let base_level = src.paragraphs.get(para).map(|q| q.level).unwrap_or(0);
    if app.canvas.tables.iter().any(|tl| tl.rect.expand(8.0).contains(p)) {
        return ParaDrop::Nowhere;
    }
    let Some(o) = app.canvas.outlines.iter().find(|o| !o.table && o.rect.expand(16.0).contains(p)) else {
        let pt = app.canvas.to_page(p);
        return ParaDrop::NewAt(notecraft_engine::doc::geom::Point::new(pt.x.max(0.0), pt.y.max(0.0)));
    };
    let Some(dst) = page.outline(o.id) else { return ParaDrop::Nowhere };
    let same = o.id == from;
    let rows: Vec<&ParaLayout> =
        app.canvas.paras.iter().filter(|pl| pl.block == Block::Outline(o.id) && !(same && moved.contains(&pl.para))).collect();
    // Index (before removal) and the y of the indicator.
    let (index, y) = match rows.iter().find(|pl| p.y < pl.rect().center().y) {
        Some(pl) => (pl.para, pl.pos.y - 1.0),
        None => (dst.paragraphs.len(), rows.last().map(|pl| pl.rect().max.y + 1.0).unwrap_or(o.rect.max.y - 4.0)),
    };
    let index = if same && index > moved.start && index < moved.end { moved.start } else { index };
    // The paragraph the dropped one would follow decides how deep it may go.
    let prev = (0..index).rev().find(|j| !(same && moved.contains(j))).and_then(|j| dst.paragraphs.get(j));
    let max_level = prev.map(|q| q.level.saturating_add(1)).unwrap_or(0).min(notecraft_engine::doc::text::MAX_LEVEL);
    let z = PT * app.canvas.zoom.max(0.1);
    let wanted = ((p.x - o.left) / (INDENT * z)).round().max(0.0) as u8;
    let level = wanted.min(max_level);
    let after_removal = if same && index >= moved.end { index - moved.len() } else { index };
    if same && (index == moved.start || index == moved.end) && level == base_level {
        return ParaDrop::Nowhere;
    }
    let x = o.left + level as f32 * INDENT * z;
    ParaDrop::Into { to: o.id, index: after_removal, level, line: [pos2(x, y), pos2(o.rect.max.x - 4.0, y)] }
}

fn dist(pl: &ParaLayout, p: Pos2) -> f32 {
    let r = pl.rect();
    let dx = if p.x < r.min.x {
        r.min.x - p.x
    } else if p.x > r.max.x {
        p.x - r.max.x
    } else {
        0.0
    };
    let dy = if p.y < r.min.y {
        r.min.y - p.y
    } else if p.y > r.max.y {
        p.y - r.max.y
    } else {
        0.0
    };
    dx * dx + dy * dy
}

/// The `block` parameter of `caret.set` for a block.
fn block_param(b: Block) -> String {
    match b {
        Block::Title => "title".to_string(),
        Block::Outline(id) | Block::Cell(id) => id.to_string(),
    }
}

/// The paragraph under a point: inside a paragraph's rect, anywhere in a table cell (its
/// nearest paragraph), or to the right of a line of a container (clicking past the end of a
/// line puts the caret at its end).
pub(crate) fn hit_para(app: &NoteApp, p: Pos2) -> Option<ParaLayout> {
    if let Some(pl) = app.canvas.paras.iter().find(|pl| pl.rect().expand2(vec2(3.0, 0.0)).contains(p)) {
        return Some(pl.clone());
    }
    if let Some(cell) = app.canvas.tables.iter().find_map(|tl| tl.cell_at(p)) {
        return app.canvas.paras.iter().filter(|pl| pl.block == Block::Cell(cell.id)).min_by(|a, b| dist(a, p).total_cmp(&dist(b, p))).cloned();
    }
    // Within a container's frame: the nearest paragraph of that container.
    let o = app.canvas.outlines.iter().find(|o| !o.table && o.rect.contains(p))?;
    app.canvas.paras.iter().filter(|pl| pl.block == Block::Outline(o.id)).min_by(|a, b| dist(a, p).total_cmp(&dist(b, p))).cloned()
}

/// Visual-line targets for the keymap: the position one row up/down from the caret, or the
/// start/end of the caret's visual row. Returns `(para, offset)`.
pub fn visual_target(app: &mut NoteApp, kind: &str) -> Option<(usize, usize)> {
    let c = app.session.text_range()?.head;
    let pl = app.canvas.para(c.block, c.para)?.clone();
    let ci = CCursor::new(byte_to_char(&pl.text, c.offset));
    match kind {
        "up" | "down" => {
            let x = app.canvas.desired_x.unwrap_or_else(|| pl.caret_rect(c.offset).min.x - pl.pos.x);
            let (nc, _) = if kind == "up" { pl.galley.cursor_up_one_row(&ci, Some(x)) } else { pl.galley.cursor_down_one_row(&ci, Some(x)) };
            let row_of = |cc: CCursor| pl.galley.pos_from_cursor(cc).min.y;
            let same_row = (row_of(nc) - row_of(ci)).abs() < 0.5;
            let at_edge = if kind == "up" {
                pl.galley.pos_from_cursor(ci).min.y < 1.0
            } else {
                (pl.galley.pos_from_cursor(ci).max.y - pl.galley.size().y).abs() < 1.0
            };
            app.canvas.desired_x = Some(x);
            if same_row || at_edge {
                // Move to the neighbouring paragraph, keeping the x position.
                let order: Vec<&ParaLayout> = app.canvas.paras.iter().filter(|p| p.block == c.block).collect();
                let idx = order.iter().position(|p| p.para == c.para)?;
                let next = if kind == "up" { idx.checked_sub(1).and_then(|i| order.get(i)) } else { order.get(idx + 1) }?;
                let target_y = if kind == "up" { next.galley.size().y - 1.0 } else { 1.0 };
                let cc = next.galley.cursor_from_pos(vec2(x + pl.pos.x - next.pos.x, target_y));
                return Some((next.para, char_to_byte(&next.text, cc.index.0)));
            }
            Some((c.para, char_to_byte(&pl.text, nc.index.0)))
        }
        "home" => Some((c.para, char_to_byte(&pl.text, pl.galley.cursor_begin_of_row(&ci).index.0))),
        "end" => Some((c.para, char_to_byte(&pl.text, pl.galley.cursor_end_of_row(&ci).index.0))),
        _ => None,
    }
}
