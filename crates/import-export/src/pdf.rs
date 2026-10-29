//! PDF out (catalog `BS.Export.FormatPdf`, `FMT.Export.Pdf`; decision D13): pages drawn as
//! vector PDF with `krilla`, laid out the way the canvas lays them out.
//!
//! **What is drawn:** the page colour and rule lines, the title with its underline and date
//! line, note containers (paragraphs at the canvas's indents, list bullets and numbers, tag
//! icons), tables (column widths, padding, merged cells, shading, borders), pictures (turns and
//! flips; the page background picture first), attachments (a card with the file name; the file
//! itself is embedded in the PDF), and ink strokes (pen and highlighter). Text is real text:
//! the page's fonts (`notecraft-fonts`: Carlito for Calibri, then open fallbacks) are embedded
//! and subsetted, so it can be selected and searched. Links become link annotations; the
//! title, the author and the time zone's dates go into the metadata.
//!
//! **Paper.** Each page goes on the page setup's paper (else US Letter, half-inch margins),
//! scaled down to fit its width when the content is wider. A page taller than the paper
//! continues on further sheets; a sheet ends between text lines and table rows, never
//! through them, unless a single line or picture is taller than the sheet.
//!
//! **Not drawn:** equations show as their linear text (the built-up layout is the canvas's);
//! italic is slanted and a missing bold face is drawn with an outline (the canvas does the
//! same); ink pressure becomes the stroke's mean width. Nothing here panics: sizes are
//! clamped, pictures that can't be read become grey boxes and are counted.

mod text;

use std::collections::{HashMap, HashSet};

use krilla::color::rgb;
use krilla::geom::{Path, PathBuilder, Point, Size, Transform};
use krilla::num::NormalizedF32;
use krilla::paint::{Fill, FillRule, LineCap, LineJoin, Stroke};
use krilla::surface::Surface;
use krilla::text::{GlyphId, KrillaGlyph};
use notecraft_codecs::Format;
use notecraft_doc::color::Rgba;
use notecraft_doc::page::AUTO_WIDTH_MAX;
use notecraft_doc::{Align, InkTool, ListFormat, OutlineWidth, Page, PageObject, Paragraph, Table, TagSymbol};

use self::text::{Faces, Laid};

/// Indent per outline level, points (the canvas's `INDENT`).
const INDENT: f32 = 27.0;
/// Width reserved for a bullet or number.
const LIST_W: f32 = 18.0;
/// Width per tag icon.
const TAG_W: f32 = 16.0;
/// Padding inside a table cell.
const CELL_PAD: f32 = 4.0;
/// Where the title sits and how wide it wraps.
const TITLE_AT: (f32, f32) = (36.0, 30.0);
const TITLE_WRAP: f32 = 560.0;
/// US Letter, the paper of pages without a page setup paper size.
const LETTER: (f32, f32) = (612.0, 792.0);
/// Margins of the default paper (top, right, bottom, left).
const MARGINS: [f32; 4] = [36.0; 4];
/// Most sheets one export writes.
pub const MAX_SHEETS: usize = 20_000;
/// The attachment card's size (the canvas's `ATTACHMENT_SIZE`).
const CARD: (f32, f32) = notecraft_doc::media::ATTACHMENT_SIZE;

const GREY_META: Rgba = Rgba::rgb(0x61, 0x61, 0x61);
const GREY_LINE: Rgba = Rgba::rgb(0xC8, 0xC6, 0xC4);
const TABLE_LINE: Rgba = Rgba::rgb(0xA3, 0xA3, 0xA3);
const RULE: Rgba = Rgba::rgb(0xB4, 0xCF, 0xE6);
const MARGIN_LINE: Rgba = Rgba::rgb(0xE6, 0x9A, 0x9A);
const HIGHLIGHTER_OPACITY: f32 = 0.45;

/// Who and what the PDF says it is.
#[derive(Debug, Clone, Default)]
pub struct PdfOptions {
    /// The document title.
    pub title: String,
    /// The author.
    pub author: String,
    /// The time zone of the date lines, minutes east of UTC.
    pub utc_offset_min: i32,
    /// One sheet per page, as large as its content (for page pictures): no paper, no scaling.
    pub fit_content: bool,
}

/// What a PDF export did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PdfReport {
    /// Notebook pages written.
    pub pages: usize,
    /// PDF pages (sheets) written.
    pub sheets: usize,
    /// Pictures drawn as grey boxes (unreadable).
    pub pictures_lost: usize,
    /// Files embedded.
    pub files: usize,
    /// Equations written as linear text.
    pub equations: usize,
}

/// A rectangle in page points.
#[derive(Debug, Clone, Copy, PartialEq)]
struct R {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl R {
    fn bottom(self) -> f32 {
        self.y + self.h
    }
}

/// Something to draw, in page points.
enum Op {
    Fill {
        r: R,
        color: Rgba,
        alpha: f32,
    },
    Line {
        a: (f32, f32),
        b: (f32, f32),
        width: f32,
        color: Rgba,
    },
    Glyphs {
        x: f32,
        baseline: f32,
        face: usize,
        look: text::Look,
        glyphs: Vec<(u32, f32)>,
        text: String,
    },
    Picture {
        r: R,
        image: usize,
        turns: u8,
        flip_h: bool,
        flip_v: bool,
    },
    Ink {
        points: Vec<(f32, f32)>,
        width: f32,
        color: Rgba,
        alpha: f32,
    },
    /// A filled disc, or a ring when `ring` is a line width.
    Dot {
        c: (f32, f32),
        r: f32,
        color: Rgba,
        ring: Option<f32>,
    },
    Frame {
        r: R,
        width: f32,
        color: Rgba,
    },
}

impl Op {
    /// The vertical extent, for picking the sheets an op shows on.
    fn span(&self) -> (f32, f32) {
        match self {
            Op::Fill { r, .. } | Op::Picture { r, .. } | Op::Frame { r, .. } => (r.y, r.bottom()),
            Op::Line { a, b, width, .. } => (a.1.min(b.1) - width, a.1.max(b.1) + width),
            Op::Glyphs { baseline, look, .. } => (baseline - look.size * 1.2, baseline + look.size * 0.5),
            Op::Ink { points, width, .. } => {
                let (lo, hi) = points.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.1), hi.max(p.1)));
                (lo - width, hi + width)
            }
            Op::Dot { c, r, .. } => (c.1 - r - 1.0, c.1 + r + 1.0),
        }
    }
}

/// A page as drawing operations, with what sheets may not cut through.
#[derive(Default)]
struct Display {
    ops: Vec<Op>,
    links: Vec<(R, String)>,
    /// Vertical bands a sheet break should avoid (text lines, table rows, pictures).
    bands: Vec<(f32, f32)>,
    /// The content's extent.
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Display {
    fn grow(&mut self, r: R) {
        if r.w.is_finite() && r.h.is_finite() {
            self.left = self.left.min(r.x);
            self.top = self.top.min(r.y);
            self.right = self.right.max(r.x + r.w);
            self.bottom = self.bottom.max(r.bottom());
        }
    }
}

/// Lays out one page into a [`Display`].
struct Painter<'a> {
    faces: &'a mut Faces,
    images: &'a mut Images,
    d: Display,
    report: &'a mut PdfReport,
}

impl Painter<'_> {
    /// Draw a laid-out paragraph with its top-left at (`x`, `y`), aligned within `wrap`.
    fn para(&mut self, laid: &Laid, x: f32, y: f32, wrap: f32, align: Align) {
        let mut top = y;
        for line in &laid.lines {
            let baseline = top + line.ascent;
            let mut lx = match align {
                Align::Left => x,
                Align::Center => x + ((wrap - line.width) / 2.0).max(0.0),
                Align::Right => x + (wrap - line.width).max(0.0),
            };
            self.d.bands.push((top, top + line.height));
            self.d.grow(R { x, y: top, w: line.width, h: line.height });
            let gs = laid.glyphs.get(line.glyphs.clone()).unwrap_or(&[]);
            let mut k = 0;
            while k < gs.len() {
                let (face, look_i) = (gs[k].face, gs[k].look);
                let mut end = k + 1;
                while end < gs.len() && gs[end].face == face && gs[end].look == look_i {
                    end += 1;
                }
                let run = &gs[k..end];
                let width: f32 = run.iter().map(|g| g.adv).sum();
                if let Some(look) = laid.looks.get(look_i) {
                    if let Some(h) = look.highlight {
                        self.d.ops.push(Op::Fill { r: R { x: lx, y: top, w: width, h: line.height }, color: h, alpha: 1.0 });
                    }
                    let bl = baseline - look.rise;
                    if run.iter().any(|g| !g.c.is_whitespace()) {
                        self.d.ops.push(Op::Glyphs {
                            x: lx,
                            baseline: bl,
                            face,
                            look: look.clone(),
                            glyphs: run.iter().map(|g| (g.gid, g.adv)).collect(),
                            text: run.iter().map(|g| g.c).collect(),
                        });
                    }
                    let th = (look.size * 0.06).max(0.5);
                    if look.underline {
                        let uy = bl + look.size * 0.12;
                        self.d.ops.push(Op::Line { a: (lx, uy), b: (lx + width, uy), width: th, color: look.color });
                    }
                    if look.strike {
                        let sy = bl - look.size * 0.28;
                        self.d.ops.push(Op::Line { a: (lx, sy), b: (lx + width, sy), width: th, color: look.color });
                    }
                    if let Some(url) = &look.link {
                        self.d.links.push((R { x: lx, y: top, w: width, h: line.height }, url.clone()));
                    }
                }
                lx += width;
                k = end;
            }
            top += line.height;
        }
    }

    /// A tag icon in the `size`-point square at (`x`, `y`).
    fn tag(&mut self, x: f32, y: f32, size: f32, symbol: TagSymbol, checked: bool) {
        let r = R { x, y, w: size, h: size };
        if symbol.is_checkbox() {
            self.d.ops.push(Op::Frame { r, width: 0.8, color: GREY_META });
            if checked {
                let pts = vec![(x + size * 0.2, y + size * 0.55), (x + size * 0.42, y + size * 0.78), (x + size * 0.82, y + size * 0.25)];
                self.d.ops.push(Op::Ink { points: pts, width: size * 0.14, color: Rgba::rgb(0x10, 0x7C, 0x10), alpha: 1.0 });
            }
        } else {
            // Other tags: a marker (their own icons are the canvas's).
            self.d.ops.push(Op::Dot { c: (x + size / 2.0, y + size / 2.0), r: size * 0.3, color: Rgba::rgb(0xE8, 0x8B, 0x00), ring: None });
        }
    }

    /// The bullet or number before a paragraph whose text starts at `tx`.
    fn label(&mut self, p: &Paragraph, label: &str, tx: f32, y: f32, first_line: f32) {
        let size = p.effective_fmt_at(0).size.filter(|s| s.is_finite()).unwrap_or(11.0).clamp(4.0, 400.0);
        let lx = tx - LIST_W * 0.95;
        let cy = y + first_line / 2.0;
        match p.list {
            Some(ListFormat::Bullet(b)) => {
                use notecraft_doc::Bullet;
                let s = size * 0.22;
                match b {
                    Bullet::Disc => self.d.ops.push(Op::Dot { c: (lx + size * 0.3, cy), r: s, color: text::TEXT, ring: None }),
                    Bullet::Square => self.d.ops.push(Op::Fill {
                        r: R { x: lx + size * 0.3 - s, y: cy - s, w: 2.0 * s, h: 2.0 * s },
                        color: text::TEXT,
                        alpha: 1.0,
                    }),
                    Bullet::Circle => self.d.ops.push(Op::Dot { c: (lx + size * 0.3, cy), r: s, color: text::TEXT, ring: Some(0.7) }),
                    Bullet::Dash => {
                        self.d.ops.push(Op::Line { a: (lx + size * 0.1, cy), b: (lx + size * 0.55, cy), width: size * 0.07, color: text::TEXT })
                    }
                    _ => {
                        let mut q = Paragraph::with_text(notecraft_doc::Id::NIL, b.glyph());
                        q.runs.iter_mut().for_each(|r| r.fmt.size = Some(size));
                        let laid = text::layout(self.faces, &q, 400.0);
                        self.para(&laid, lx, cy - laid.height / 2.0, 400.0, Align::Left);
                    }
                }
            }
            _ => {
                let mut q = Paragraph::with_text(notecraft_doc::Id::NIL, label);
                q.runs.iter_mut().for_each(|r| r.fmt.size = Some(size * 0.95));
                let laid = text::layout(self.faces, &q, 400.0);
                self.para(&laid, lx, cy - laid.height / 2.0, 400.0, Align::Left);
            }
        }
    }

    /// Paragraphs of a container or cell from (`x0`, `y`), `max_w` wide; returns the bottom.
    fn paragraphs(&mut self, ps: &[Paragraph], hidden: &dyn Fn(usize) -> bool, x0: f32, mut y: f32, max_w: f32) -> f32 {
        let labels = notecraft_text::list_labels(ps);
        for (i, p) in ps.iter().enumerate() {
            if hidden(i) {
                continue;
            }
            if p.math.is_some() {
                self.report.equations += 1;
            }
            let label = labels.get(i).map(String::as_str).unwrap_or("");
            let has_list = !label.is_empty();
            let lead = f32::from(p.level) * INDENT + p.tags.len() as f32 * TAG_W + if has_list { LIST_W } else { 0.0 };
            let wrap = (max_w - lead).max(24.0);
            y += finite(p.space_before);
            let laid = text::layout(self.faces, p, wrap);
            let tx = x0 + lead;
            if has_list {
                self.label(p, label, tx, y, laid.first_line());
            }
            for (k, tag) in p.tags.iter().enumerate() {
                let x = tx - (if has_list { LIST_W } else { 0.0 } + TAG_W * (p.tags.len() - k) as f32);
                let sz = 10.5;
                self.tag(x, y + (laid.first_line() - sz) / 2.0, sz, tag.symbol, tag.checked);
            }
            self.para(&laid, tx, y, wrap, p.align);
            y += laid.height + finite(p.space_after);
        }
        y
    }

    fn table(&mut self, t: &Table) {
        let (x0, y0) = (t.pos.x, t.pos.y);
        let mut xs = vec![x0];
        for w in &t.columns {
            let last = xs.last().copied().unwrap_or(x0);
            xs.push(last + finite(*w).max(1.0));
        }
        let n_cols = t.columns.len();
        let grid = t.span_grid();
        // Lay every shown cell out first: rows are as tall as their tallest cell.
        struct Cell<'c> {
            row: usize,
            col: usize,
            span: (usize, usize),
            paras: &'c [Paragraph],
            shading: Option<Rgba>,
            height: f32,
        }
        let mut cells = Vec::new();
        for (r, row) in t.rows.iter().enumerate() {
            for (c, cell) in row.cells.iter().enumerate() {
                let Some((rs, cs)) = grid.get(r).and_then(|g| g.get(c)).copied().flatten() else { continue };
                let cs = cs.min(n_cols.saturating_sub(c)).max(1);
                let width = xs.get(c + cs).copied().unwrap_or(x0) - xs.get(c).copied().unwrap_or(x0);
                let h = self.measure(&cell.paragraphs, width - 2.0 * CELL_PAD) + 2.0 * CELL_PAD;
                cells.push(Cell { row: r, col: c, span: (rs, cs), paras: &cell.paragraphs, shading: cell.shading, height: h });
            }
        }
        let mut row_h = vec![14.0f32; t.rows.len()];
        for c in cells.iter().filter(|c| c.span.0 == 1) {
            if let Some(h) = row_h.get_mut(c.row) {
                *h = h.max(c.height);
            }
        }
        for c in cells.iter().filter(|c| c.span.0 > 1) {
            let last = (c.row + c.span.0).min(row_h.len()).saturating_sub(1);
            let have: f32 = row_h.iter().skip(c.row).take(c.span.0).sum();
            if have < c.height
                && let Some(h) = row_h.get_mut(last)
            {
                *h += c.height - have;
            }
        }
        let mut ys = vec![y0];
        for h in &row_h {
            let last = ys.last().copied().unwrap_or(y0);
            ys.push(last + h);
        }
        for (k, h) in row_h.iter().enumerate() {
            self.d.bands.push((ys[k], ys[k] + h));
        }
        for c in &cells {
            let (cx0, cx1) = (xs.get(c.col).copied().unwrap_or(x0), xs.get(c.col + c.span.1).copied().unwrap_or(x0));
            let (top, bottom) = (ys.get(c.row).copied().unwrap_or(y0), ys.get(c.row + c.span.0).copied().unwrap_or(y0));
            let r = R { x: cx0, y: top, w: cx1 - cx0, h: bottom - top };
            if let Some(s) = c.shading {
                self.d.ops.push(Op::Fill { r, color: s, alpha: 1.0 });
            }
            self.paragraphs(c.paras, &|_| false, cx0 + CELL_PAD, top + CELL_PAD, r.w - 2.0 * CELL_PAD);
            if t.borders {
                self.d.ops.push(Op::Frame { r, width: 0.75, color: TABLE_LINE });
            }
        }
        let (right, bottom) = (xs.last().copied().unwrap_or(x0), ys.last().copied().unwrap_or(y0));
        self.d.grow(R { x: x0, y: y0, w: right - x0, h: bottom - y0 });
    }

    /// The height paragraphs take at `width` (laid out and thrown away).
    fn measure(&mut self, ps: &[Paragraph], width: f32) -> f32 {
        let labels = notecraft_text::list_labels(ps);
        let mut h = 0.0;
        for (i, p) in ps.iter().enumerate() {
            let has_list = labels.get(i).is_some_and(|l| !l.is_empty());
            let lead = f32::from(p.level) * INDENT + p.tags.len() as f32 * TAG_W + if has_list { LIST_W } else { 0.0 };
            h += finite(p.space_before) + text::layout(self.faces, p, (width - lead).max(8.0)).height + finite(p.space_after);
        }
        h
    }

    fn page(&mut self, page: &Page, opts: &PdfOptions) {
        // Title area (BEH.Pages.Title, BEH.Pages.DateTime).
        if !page.setup.hide_title {
            let (tx, ty) = TITLE_AT;
            let laid = text::layout(self.faces, &page.title, TITLE_WRAP);
            self.para(&laid, tx, ty, TITLE_WRAP, Align::Left);
            let uy = ty + laid.height.max(laid.first_line()) + 2.25;
            self.d.ops.push(Op::Line { a: (tx, uy), b: (tx + laid.width.max(225.0), uy), width: 0.75, color: GREY_LINE });
            let t = page.shown_time();
            let date = format!("{}      {}", t.long_date(opts.utc_offset_min), t.short_time(opts.utc_offset_min));
            let mut q = Paragraph::with_text(notecraft_doc::Id::NIL, &date);
            q.runs.iter_mut().for_each(|r| {
                r.fmt.size = Some(9.5 * 0.75);
                r.fmt.color = Some(GREY_META);
            });
            let dl = text::layout(self.faces, &q, TITLE_WRAP);
            self.para(&dl, tx, uy + 3.75, TITLE_WRAP, Align::Left);
            self.d.grow(R { x: tx, y: ty, w: laid.width.max(225.0), h: uy + 3.75 + dl.height - ty });
        }
        // Background pictures first, then text and tables, then pictures and files, ink on top.
        let order = |o: &PageObject| match o {
            PageObject::Image(i) if i.background => 0,
            PageObject::Outline(_) | PageObject::Table(_) => 1,
            PageObject::Image(_) | PageObject::Attachment(_) => 2,
            PageObject::Ink(_) => 3,
        };
        let mut objects: Vec<&PageObject> = page.objects.iter().collect();
        objects.sort_by_key(|o| order(o));
        for o in objects {
            match o {
                PageObject::Outline(ol) => {
                    let max_w = match ol.width {
                        OutlineWidth::Fixed(w) => finite(w).max(24.0),
                        OutlineWidth::Auto => AUTO_WIDTH_MAX,
                    };
                    self.paragraphs(&ol.paragraphs, &|i| ol.is_hidden(i), ol.pos.x, ol.pos.y, max_w);
                }
                PageObject::Table(t) => self.table(t),
                PageObject::Image(im) => {
                    let r = R { x: im.pos.x, y: im.pos.y, w: finite(im.width).max(1.0), h: finite(im.height).max(1.0) };
                    self.d.grow(r);
                    self.d.bands.push((r.y, r.bottom()));
                    match self.images.get(&im.blob) {
                        Some(image) => self.d.ops.push(Op::Picture { r, image, turns: im.quarter_turns % 4, flip_h: im.flip_h, flip_v: im.flip_v }),
                        None => {
                            self.report.pictures_lost += 1;
                            self.d.ops.push(Op::Fill { r, color: Rgba::rgb(0xD0, 0xD0, 0xD0), alpha: 1.0 });
                        }
                    }
                    if let Some(url) = im.link.as_deref().filter(|l| !l.trim().is_empty()) {
                        self.d.links.push((r, url.to_string()));
                    }
                }
                PageObject::Attachment(a) => {
                    let r = R { x: a.pos.x, y: a.pos.y, w: CARD.0, h: CARD.1 };
                    self.d.grow(r);
                    self.d.bands.push((r.y, r.bottom()));
                    self.d.ops.push(Op::Frame { r, width: 0.75, color: GREY_LINE });
                    // A file icon: a page with its corner folded.
                    let (ix, iy, iw, ih) = (r.x + r.w / 2.0 - 10.0, r.y + 8.0, 20.0, 26.0);
                    let pts = vec![(ix, iy), (ix + iw - 7.0, iy), (ix + iw, iy + 7.0), (ix + iw, iy + ih), (ix, iy + ih), (ix, iy)];
                    self.d.ops.push(Op::Ink { points: pts, width: 1.0, color: GREY_META, alpha: 1.0 });
                    let mut q = Paragraph::with_text(notecraft_doc::Id::NIL, &a.name);
                    q.runs.iter_mut().for_each(|x| x.fmt.size = Some(8.0));
                    q.align = Align::Center;
                    let laid = text::layout(self.faces, &q, r.w - 4.0);
                    self.para(&laid, r.x + 2.0, iy + ih + 4.0, r.w - 4.0, Align::Center);
                    if let Some(url) = a.link.as_deref().filter(|l| !l.trim().is_empty()) {
                        self.d.links.push((r, url.to_string()));
                    }
                }
                PageObject::Ink(k) => {
                    let points: Vec<(f32, f32)> = k.page_points().map(|p| (p.x, p.y)).collect();
                    if points.is_empty() {
                        continue;
                    }
                    let n = k.points.len().max(1) as f32;
                    let width = finite(k.points.iter().map(|p| k.width_at(p.pressure)).sum::<f32>() / n).clamp(0.1, 200.0);
                    let alpha = if k.tool == InkTool::Highlighter { HIGHLIGHTER_OPACITY } else { 1.0 };
                    let (lo_x, hi_x, lo_y, hi_y) = points
                        .iter()
                        .fold((f32::MAX, f32::MIN, f32::MAX, f32::MIN), |(a, b, c, d), p| (a.min(p.0), b.max(p.0), c.min(p.1), d.max(p.1)));
                    self.d.grow(R { x: lo_x - width, y: lo_y - width, w: hi_x - lo_x + 2.0 * width, h: hi_y - lo_y + 2.0 * width });
                    self.d.ops.push(Op::Ink { points, width, color: k.color, alpha });
                }
            }
        }
    }
}

fn finite(v: f32) -> f32 {
    if v.is_finite() { v.clamp(-1e5, 1e5) } else { 0.0 }
}

fn norm(v: f32) -> NormalizedF32 {
    NormalizedF32::new(if v.is_finite() { v.clamp(0.0, 1.0) } else { 1.0 }).unwrap_or(NormalizedF32::ONE)
}

fn paint(c: Rgba) -> krilla::paint::Paint {
    rgb::Color::new(c.r, c.g, c.b).into()
}

fn fill(c: Rgba, alpha: f32) -> Fill {
    Fill { paint: paint(c), opacity: norm(alpha * f32::from(c.a) / 255.0), rule: FillRule::NonZero }
}

/// A circle as four Bézier arcs.
fn circle(cx: f32, cy: f32, r: f32) -> Option<Path> {
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(cx + r, cy);
    pb.cubic_to(cx + r, cy + k, cx + k, cy + r, cx, cy + r);
    pb.cubic_to(cx - k, cy + r, cx - r, cy + k, cx - r, cy);
    pb.cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r);
    pb.cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy);
    pb.close();
    pb.finish()
}

fn rect_path(r: R) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.push_rect(krilla::geom::Rect::from_xywh(r.x, r.y, r.w.max(0.01), r.h.max(0.01))?);
    pb.finish()
}

/// Pictures decoded for the PDF, once per blob.
#[derive(Default)]
struct Images {
    list: Vec<krilla::image::Image>,
    by_blob: HashMap<notecraft_doc::Id, Option<usize>>,
}

impl Images {
    fn get(&mut self, blob: &notecraft_doc::Blob) -> Option<usize> {
        if let Some(i) = self.by_blob.get(&blob.id) {
            return *i;
        }
        let bytes = &blob.bytes;
        let data = || krilla::Data::from(bytes.to_vec());
        let img = match notecraft_codecs::sniff(bytes) {
            Some(Format::Png) => krilla::image::Image::from_png(data(), true).ok(),
            Some(Format::Jpeg) => krilla::image::Image::from_jpeg(data(), true).ok(),
            Some(Format::Gif) => krilla::image::Image::from_gif(data(), true).ok(),
            Some(Format::WebP) => krilla::image::Image::from_webp(data(), true).ok(),
            _ => None,
        }
        .or_else(|| {
            // Anything else the codecs read (BMP, odd PNGs) goes in as pixels.
            let px = notecraft_codecs::decode(bytes, &notecraft_codecs::Limits::PICTURE).ok()?;
            Some(krilla::image::Image::from_rgba8(px.pixels, px.width, px.height))
        });
        let idx = img.map(|i| {
            self.list.push(i);
            self.list.len() - 1
        });
        self.by_blob.insert(blob.id, idx);
        idx
    }
}

/// Where each sheet of a page starts, in page points: breaks fall between `bands` when they
/// can, else at the sheet's height.
fn sheet_breaks(top: f32, bottom: f32, first: f32, rest: f32, bands: &[(f32, f32)]) -> Vec<f32> {
    let mut starts = vec![top];
    let mut start = top;
    let mut avail = first.max(1.0);
    while start + avail < bottom && starts.len() < MAX_SHEETS {
        let limit = start + avail;
        // The lowest place at or above the limit that no band crosses.
        let mut cut = limit;
        let mut moved = true;
        while moved {
            moved = false;
            for &(a, b) in bands {
                if a < cut && b > cut && b - a <= avail && a > start {
                    cut = a;
                    moved = true;
                }
            }
        }
        if cut <= start + avail * 0.25 {
            cut = limit;
        }
        starts.push(cut);
        start = cut;
        avail = rest.max(1.0);
    }
    starts
}

/// The glyph run of a text op on the surface.
fn draw_glyphs(s: &mut Surface, faces: &Faces, x: f32, baseline: f32, face: usize, look: &text::Look, glyphs: &[(u32, f32)], text: &str) {
    let Some(f) = faces.face(face) else { return };
    let size = look.size.max(0.5);
    let mut kg = Vec::with_capacity(glyphs.len());
    let mut at = 0;
    for ((gid, adv), c) in glyphs.iter().zip(text.chars()) {
        let len = c.len_utf8();
        kg.push(KrillaGlyph::new(GlyphId::new(*gid), adv / size, 0.0, 0.0, 0.0, at..at + len, None));
        at += len;
    }
    if look.italic {
        s.push_transform(&Transform::from_row(1.0, 0.0, -0.21, 1.0, 0.21 * baseline, 0.0));
    }
    s.set_fill(Some(fill(look.color, 1.0)));
    s.set_stroke(look.fake_bold.then(|| Stroke { paint: paint(look.color), width: size * 0.03, ..Default::default() }));
    s.draw_glyphs(Point::from_xy(x, baseline), &kg, f.font.clone(), text, size, false);
    s.set_stroke(None);
    if look.italic {
        s.pop();
    }
}

fn draw(s: &mut Surface, faces: &Faces, images: &Images, op: &Op) {
    match op {
        Op::Fill { r, color, alpha } => {
            if let Some(p) = rect_path(*r) {
                s.set_stroke(None);
                s.set_fill(Some(fill(*color, *alpha)));
                s.draw_path(&p);
            }
        }
        Op::Frame { r, width, color } => {
            if let Some(p) = rect_path(*r) {
                s.set_fill(None);
                s.set_stroke(Some(Stroke { paint: paint(*color), width: *width, ..Default::default() }));
                s.draw_path(&p);
                s.set_stroke(None);
            }
        }
        Op::Line { a, b, width, color } => {
            let mut pb = PathBuilder::new();
            pb.move_to(a.0, a.1);
            pb.line_to(b.0, b.1);
            if let Some(p) = pb.finish() {
                s.set_fill(None);
                s.set_stroke(Some(Stroke { paint: paint(*color), width: width.max(0.1), ..Default::default() }));
                s.draw_path(&p);
                s.set_stroke(None);
            }
        }
        Op::Dot { c, r, color, ring } => {
            if let Some(p) = circle(c.0, c.1, r.max(0.1)) {
                match ring {
                    Some(w) => {
                        s.set_fill(None);
                        s.set_stroke(Some(Stroke { paint: paint(*color), width: *w, ..Default::default() }));
                    }
                    None => {
                        s.set_stroke(None);
                        s.set_fill(Some(fill(*color, 1.0)));
                    }
                }
                s.draw_path(&p);
                s.set_stroke(None);
            }
        }
        Op::Ink { points, width, color, alpha } => {
            let mut pb = PathBuilder::new();
            let Some(first) = points.first() else { return };
            pb.move_to(first.0, first.1);
            if points.len() == 1 {
                // A dot: a zero-length line with round caps.
                pb.line_to(first.0 + 0.01, first.1);
            }
            for p in points.iter().skip(1) {
                pb.line_to(p.0, p.1);
            }
            if let Some(p) = pb.finish() {
                s.set_fill(None);
                s.set_stroke(Some(Stroke {
                    paint: paint(*color),
                    width: *width,
                    opacity: norm(*alpha * f32::from(color.a) / 255.0),
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Default::default()
                }));
                s.draw_path(&p);
                s.set_stroke(None);
            }
        }
        Op::Glyphs { x, baseline, face, look, glyphs, text } => draw_glyphs(s, faces, *x, *baseline, *face, look, glyphs, text),
        Op::Picture { r, image, turns, flip_h, flip_v } => {
            let Some(img) = images.list.get(*image) else { return };
            // The stored picture is mirrored, then turned clockwise; `r` is the result.
            let (w0, h0) = if turns % 2 == 1 { (r.h, r.w) } else { (r.w, r.h) };
            let Some(size) = Size::from_wh(w0.max(0.01), h0.max(0.01)) else { return };
            let (c, sn) = match turns % 4 {
                1 => (0.0, 1.0),
                2 => (-1.0, 0.0),
                3 => (0.0, -1.0),
                _ => (1.0, 0.0),
            };
            let (fx, fy) = (if *flip_h { -1.0 } else { 1.0 }, if *flip_v { -1.0 } else { 1.0 });
            // M = translate(centre) · rotate · scale(flip) · translate(−size/2).
            let (a, b, cc, d) = (c * fx, sn * fx, -sn * fy, c * fy);
            let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
            let (tx, ty) = (cx - (a * w0 / 2.0 + cc * h0 / 2.0), cy - (b * w0 / 2.0 + d * h0 / 2.0));
            s.push_transform(&Transform::from_row(a, b, cc, d, tx, ty));
            s.draw_image(img.clone(), size);
            s.pop();
        }
    }
}

/// `pages` as a PDF with the page fonts `fonts`.
pub fn write(pages: &[&Page], fonts: &notecraft_fonts::PageFonts, opts: &PdfOptions) -> Result<(Vec<u8>, PdfReport), String> {
    let mut faces = Faces::new(fonts).ok_or("no font for the PDF: install Carlito, Noto Sans, Liberation Sans or DejaVu Sans")?;
    let mut images = Images::default();
    let mut report = PdfReport::default();
    let mut pdf = krilla::Document::new();
    let mut meta = krilla::metadata::Metadata::new().creator("NoteCraft".into()).producer("NoteCraft".into());
    if !opts.title.trim().is_empty() {
        meta = meta.title(opts.title.clone());
    }
    if !opts.author.trim().is_empty() {
        meta = meta.authors(vec![opts.author.clone()]);
    }
    pdf.set_metadata(meta);
    let mut embedded: HashSet<String> = HashSet::new();
    for page in pages {
        let mut p = Painter { faces: &mut faces, images: &mut images, d: Display::default(), report: &mut report };
        p.d.left = f32::MAX;
        p.d.top = f32::MAX;
        p.d.right = f32::MIN;
        p.d.bottom = f32::MIN;
        p.page(page, opts);
        let d = p.d;
        // An empty page still has its paper.
        let (left, top, right, bottom) = if d.left <= d.right {
            (d.left.min(TITLE_AT.0), d.top.min(TITLE_AT.1), d.right, d.bottom)
        } else {
            (TITLE_AT.0, TITLE_AT.1, TITLE_AT.0, TITLE_AT.1)
        };
        let paper = page.setup.paper;
        let ((pw, ph), [mt, mr, mb, ml]) = if opts.fit_content {
            let m = 18.0;
            (((right - left).clamp(36.0, 14_000.0) + 2.0 * m, (bottom - top).clamp(36.0, 14_000.0) + 2.0 * m), [m; 4])
        } else {
            (
                paper.map_or(LETTER, |s| (finite(s.width).clamp(72.0, 14_400.0), finite(s.height).clamp(72.0, 14_400.0))),
                paper.map_or(MARGINS, |s| s.margins.map(|m| finite(m).clamp(0.0, 720.0))),
            )
        };
        let (pw_in, ph_in) = ((pw - ml - mr).max(36.0), (ph - mt - mb).max(36.0));
        let k = (pw_in / (right - left).max(1.0)).min(1.0);
        let starts = if opts.fit_content { vec![top] } else { sheet_breaks(top, bottom, ph_in / k, ph_in / k, &d.bands) };
        report.pages += 1;
        for (n, &start) in starts.iter().enumerate() {
            if report.sheets >= MAX_SHEETS {
                break;
            }
            let end = starts.get(n + 1).copied().unwrap_or(f32::MAX);
            let Some(size) = Size::from_wh(pw, ph) else { continue };
            let mut sheet = pdf.start_page_with(krilla::page::PageSettings::new(size));
            let mut s = sheet.surface();
            // Page colour and rule lines cover the whole sheet.
            if let Some(c) = page.setup.color
                && let Some(path) = rect_path(R { x: 0.0, y: 0.0, w: pw, h: ph })
            {
                s.set_fill(Some(fill(c, 1.0)));
                s.draw_path(&path);
            }
            let to_sheet = |x: f32, y: f32| (ml + (x - left) * k, mt + (y - start) * k);
            rule_lines(&mut s, page, pw, ph, &to_sheet, k);
            // The window of the page this sheet shows.
            if let Some(clip) = rect_path(R { x: 0.0, y: mt, w: pw, h: ((end.min(bottom + 1.0) - start) * k).min(ph_in) + 0.5 }) {
                s.push_clip_path(&clip, &FillRule::NonZero);
            }
            s.push_transform(&Transform::from_row(k, 0.0, 0.0, k, ml - left * k, mt - start * k));
            for op in &d.ops {
                let (a, b) = op.span();
                if b >= start && a < end {
                    draw(&mut s, &faces, &images, op);
                }
            }
            s.pop();
            s.pop();
            s.finish();
            for (r, url) in &d.links {
                if r.bottom() < start || r.y >= end {
                    continue;
                }
                let (x0, y0) = to_sheet(r.x, r.y.max(start));
                let (x1, y1) = to_sheet(r.x + r.w, r.bottom().min(end));
                if let Some(rect) = krilla::geom::Rect::from_ltrb(x0, y0, x1.max(x0 + 0.5), y1.max(y0 + 0.5)) {
                    let target = krilla::annotation::Target::Action(krilla::action::Action::Link(krilla::action::LinkAction::new(url.clone())));
                    sheet.add_annotation(krilla::annotation::Annotation::new_link(
                        krilla::annotation::LinkAnnotation::new(rect, target),
                        Some(url.clone()),
                    ));
                }
            }
            sheet.finish();
            report.sheets += 1;
        }
        // Attached files ride along inside the PDF.
        for o in &page.objects {
            if let PageObject::Attachment(a) = o
                && !a.blob.bytes.is_empty()
            {
                let mut name = a.name.clone();
                let mut n = 2;
                while embedded.contains(&name) {
                    name = format!("{n} {}", a.name);
                    n += 1;
                }
                let file = krilla::embed::EmbeddedFile {
                    path: name.clone(),
                    mime_type: None,
                    description: None,
                    association_kind: krilla::embed::AssociationKind::Unspecified,
                    data: krilla::Data::from(a.blob.bytes.to_vec()),
                    modification_date: None,
                    compress: Some(true),
                    location: None,
                };
                if pdf.embed_file(file).is_some() {
                    embedded.insert(name);
                    report.files += 1;
                }
            }
        }
    }
    if report.sheets == 0 {
        return Err("nothing to write: no pages".into());
    }
    let bytes = pdf.finish().map_err(|e| format!("writing the PDF: {e:?}"))?;
    Ok((bytes, report))
}

/// Rule lines or grid across the sheet, aligned with the page's.
fn rule_lines(s: &mut Surface, page: &Page, pw: f32, ph: f32, to_sheet: &dyn Fn(f32, f32) -> (f32, f32), k: f32) {
    let kind = page.setup.rule_lines;
    let step = kind.spacing() * k;
    if step < 2.0 {
        return;
    }
    let color = page.setup.rule_color.unwrap_or(RULE);
    let stroke = Stroke { paint: paint(color), width: 0.75, ..Default::default() };
    let mut pb = PathBuilder::new();
    // Ruled lines start below the title area (86 pt); grids cover the page.
    let mut y = if kind.is_grid() { to_sheet(0.0, 0.0).1 } else { to_sheet(0.0, 86.0).1 };
    if kind.is_grid() && y > 0.0 {
        // A grid also covers the sheet above the page's top.
        y -= (y / step).ceil() * step;
    }
    while y < ph {
        if y >= 0.0 {
            pb.move_to(0.0, y);
            pb.line_to(pw, y);
        }
        y += step;
    }
    if kind.is_grid() {
        let x0 = to_sheet(0.0, 0.0).0;
        let mut x = x0 - (x0 / step).floor() * step;
        while x < pw {
            pb.move_to(x, 0.0);
            pb.line_to(x, ph);
            x += step;
        }
    }
    if let Some(p) = pb.finish() {
        s.set_fill(None);
        s.set_stroke(Some(stroke));
        s.draw_path(&p);
    }
    if !kind.is_grid() {
        let mx = to_sheet(24.0, 0.0).0;
        let mut pb = PathBuilder::new();
        pb.move_to(mx, 0.0);
        pb.line_to(mx, ph);
        if let Some(p) = pb.finish() {
            s.set_stroke(Some(Stroke { paint: paint(MARGIN_LINE), width: 0.75, ..Default::default() }));
            s.draw_path(&p);
        }
    }
    s.set_stroke(None);
}

#[cfg(test)]
#[path = "pdf/tests.rs"]
mod tests;
