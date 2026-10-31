//! Built-up layout: a [`Node`] tree to positioned text and line paths.
//!
//! Coordinates are in the caller's units (the `size` given to [`layout`] is the base font
//! size), x to the right from the box's left edge, y downwards from the baseline (so text above
//! the baseline has negative y). The proportions (script size, gaps, the math axis) are
//! NoteCraft's choices, close to common math typesetting; the reference's are unverified.

use std::f32::consts::PI;

use crate::parse::{BINARY, LIMIT_FUNCS, Node, RELATIONS, is_function};

/// Text measurement, supplied by the caller (the UI's fonts).
pub trait Measure {
    /// Width, ascent (above the baseline) and descent (below) of `text` at `size`.
    fn measure(&self, text: &str, size: f32, italic: bool) -> (f32, f32, f32);
}

/// Something to draw.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// Text with its baseline's left end at `x`, `y`.
    Text {
        /// The text.
        text: String,
        /// Left.
        x: f32,
        /// Baseline.
        y: f32,
        /// Font size.
        size: f32,
        /// Italic (variables).
        italic: bool,
    },
    /// A polyline of `width`.
    Path {
        /// Its points.
        points: Vec<(f32, f32)>,
        /// Line width.
        width: f32,
    },
    /// An empty slot: a dotted box with its top-left at `x`, `y`.
    Slot {
        /// Left.
        x: f32,
        /// Top.
        y: f32,
        /// Width.
        w: f32,
        /// Height.
        h: f32,
    },
}

/// A laid-out piece: its extent around the baseline and what to draw.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MathBox {
    /// Width.
    pub width: f32,
    /// Height above the baseline.
    pub ascent: f32,
    /// Depth below the baseline.
    pub descent: f32,
    /// What to draw.
    pub items: Vec<Item>,
}

impl MathBox {
    /// Move every item by `dx`, `dy`.
    fn shift(&mut self, dx: f32, dy: f32) {
        for it in &mut self.items {
            match it {
                Item::Text { x, y, .. } | Item::Slot { x, y, .. } => {
                    *x += dx;
                    *y += dy;
                }
                Item::Path { points, .. } => {
                    for p in points {
                        p.0 += dx;
                        p.1 += dy;
                    }
                }
            }
        }
    }

    /// Put `b` at `x`, its baseline `dy` below this one's, and grow to hold it.
    fn place(&mut self, mut b: MathBox, x: f32, dy: f32) {
        b.shift(x, dy);
        self.width = self.width.max(x + b.width);
        self.ascent = self.ascent.max(b.ascent - dy);
        self.descent = self.descent.max(b.descent + dy);
        self.items.append(&mut b.items);
    }

    /// The total height.
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }
}

/// Lay out `node` as a displayed equation at font `size`.
pub fn layout(node: &Node, size: f32, m: &dyn Measure) -> MathBox {
    let size = if size.is_finite() { size.clamp(1.0, 2_000.0) } else { 11.0 };
    L { m }.node(node, size, true, 0)
}

struct L<'a> {
    m: &'a dyn Measure,
}

/// Height of the math axis (fraction bars, the middle of large operators) above the baseline.
fn axis(s: f32) -> f32 {
    0.25 * s
}

fn line(s: f32) -> f32 {
    (0.06 * s).max(0.6)
}

impl L<'_> {
    fn text(&self, text: &str, s: f32, italic: bool) -> MathBox {
        let (w, a, d) = self.m.measure(text, s, italic);
        let (w, a, d) = (fin(w), fin(a), fin(d));
        MathBox { width: w, ascent: a, descent: d, items: vec![Item::Text { text: text.to_string(), x: 0.0, y: 0.0, size: s, italic }] }
    }

    fn node(&self, n: &Node, s: f32, display: bool, depth: usize) -> MathBox {
        if depth > crate::MAX_DEPTH + 4 {
            return MathBox::default();
        }
        let d = depth + 1;
        match n {
            Node::Row(items) => self.row(items, s, display, d),
            Node::Ident(t) => self.text(t, s, true),
            Node::Number(t) | Node::Text(t) | Node::Op(t) => self.text(t, s, false),
            Node::Placeholder => {
                let (w, h) = (0.55 * s, 0.7 * s);
                MathBox {
                    width: w + 0.1 * s,
                    ascent: h - 0.1 * s,
                    descent: 0.1 * s,
                    items: vec![Item::Slot { x: 0.05 * s, y: -(h - 0.1 * s), w, h }],
                }
            }
            Node::Frac(a, b) => {
                let cs = if display { s } else { (s * 0.8).max(s.min(4.0)) };
                let (num, den) = (self.node(a, cs, false, d), self.node(b, cs, false, d));
                let (t, gap, pad) = (line(s), 0.12 * s, 0.12 * s);
                let w = num.width.max(den.width) + 2.0 * pad;
                let ax = axis(s);
                let mut out = MathBox { width: w, ascent: 0.0, descent: 0.0, items: Vec::new() };
                let (nx, dx) = ((w - num.width) / 2.0, (w - den.width) / 2.0);
                let num_base = -(ax + t / 2.0 + gap + num.descent);
                let den_base = -ax + t / 2.0 + gap + den.ascent;
                out.place(num, nx, num_base);
                out.place(den, dx, den_base);
                out.items.push(Item::Path { points: vec![(pad * 0.5, -ax), (w - pad * 0.5, -ax)], width: t });
                out
            }
            Node::Script { base, sub, sup } => {
                let b = self.node(base, s, display, d);
                let limits = display && matches!(&**base, Node::Text(t) if LIMIT_FUNCS.contains(&t.as_str()));
                let ss = (s * 0.7).max(s.min(4.0));
                let sub = sub.as_ref().map(|x| self.node(x, ss, false, d));
                let sup = sup.as_ref().map(|x| self.node(x, ss, false, d));
                if limits { self.limits(b, sub, sup, s) } else { self.scripts(b, sub, sup, s) }
            }
            Node::Radical { degree, body } => self.radical(degree.as_deref(), body, s, d),
            Node::NAry { op, sub, sup, body } => {
                let ss = (s * 0.7).max(s.min(4.0));
                let sub = sub.as_ref().map(|x| self.node(x, ss, false, d));
                let sup = sup.as_ref().map(|x| self.node(x, ss, false, d));
                let sign = self.nary_sign(*op, s, display);
                let integral = matches!(op, '∫' | '∬' | '∭' | '∮');
                let mut out = if integral || !display { self.scripts(sign, sub, sup, s) } else { self.limits(sign, sub, sup, s) };
                let body = self.node(body, s, display, d);
                let x = out.width + 0.15 * s;
                out.place(body, x, 0.0);
                out
            }
            Node::Delim { open, close, body } => {
                let b = self.node(body, s, display, d);
                let top = -(b.ascent.max(0.72 * s) + 0.08 * s);
                let bottom = b.descent.max(0.24 * s) + 0.08 * s;
                let mut out = MathBox::default();
                let ow = self.delim(&mut out, *open, 0.0, top, bottom, s);
                out.place(b, ow + 0.05 * s, 0.0);
                let x = out.width + 0.05 * s;
                let cw = if *close == '\0' { 0.0 } else { self.delim(&mut out, *close, x, top, bottom, s) };
                out.width = x + cw;
                out.ascent = out.ascent.max(-top);
                out.descent = out.descent.max(bottom);
                out
            }
            Node::Matrix(rows) => self.matrix(rows, s, d),
            Node::Accent { accent, base } => {
                let b = self.node(base, s, display, d);
                let (w, top) = (b.width, -b.ascent - 0.08 * s);
                let mut out = MathBox { width: w, ascent: b.ascent + 0.3 * s, descent: b.descent, items: Vec::new() };
                let (l, r, mid) = (w * 0.15, w * 0.85, w * 0.5);
                let pts = match accent {
                    '\u{302}' => vec![(l, top), (mid, top - 0.18 * s), (r, top)],
                    '\u{303}' => (0..=8)
                        .map(|i| {
                            let t = i as f32 / 8.0;
                            (l + (r - l) * t, top - 0.08 * s - 0.06 * s * (2.0 * PI * t).sin())
                        })
                        .collect(),
                    '\u{307}' => vec![(mid - 0.04 * s, top - 0.08 * s), (mid + 0.04 * s, top - 0.08 * s)],
                    '\u{308}' => vec![(mid - 0.16 * s, top - 0.08 * s), (mid - 0.08 * s, top - 0.08 * s)],
                    '\u{20D7}' => vec![(l, top - 0.1 * s), (r, top - 0.1 * s), (r - 0.12 * s, top - 0.18 * s)],
                    _ => vec![(l, top - 0.08 * s), (r, top - 0.08 * s)],
                };
                out.items.push(Item::Path { points: pts, width: if *accent == '\u{307}' || *accent == '\u{308}' { 0.1 * s } else { line(s) } });
                if *accent == '\u{308}' {
                    out.items.push(Item::Path { points: vec![(mid + 0.08 * s, top - 0.08 * s), (mid + 0.16 * s, top - 0.08 * s)], width: 0.1 * s });
                }
                if *accent == '\u{20D7}' {
                    out.items.push(Item::Path { points: vec![(r, top - 0.1 * s), (r - 0.12 * s, top - 0.02 * s)], width: line(s) });
                }
                out.place(b, 0.0, 0.0);
                out
            }
        }
    }

    /// Side by side, with the spacing operators and relations get.
    fn row(&self, items: &[Node], s: f32, display: bool, d: usize) -> MathBox {
        let mut out = MathBox { width: 0.0, ascent: 0.7 * s, descent: 0.2 * s, items: Vec::new() };
        let mut prev_operand = false;
        for (i, n) in items.iter().enumerate() {
            let b = self.node(n, s, display, d);
            let op = match n {
                Node::Op(t) => t.chars().next(),
                _ => None,
            };
            let func = match n {
                Node::Text(t) => is_function(t),
                Node::Script { base, .. } => matches!(&**base, Node::Text(t) if is_function(t)),
                _ => false,
            };
            let (before, after) = match op {
                _ if func && i + 1 < items.len() => (0.0, 0.17 * s),
                Some(c) if RELATIONS.contains(c) => (0.28 * s, 0.28 * s),
                Some(c) if BINARY.contains(c) && prev_operand && i + 1 < items.len() => (0.22 * s, 0.22 * s),
                Some(',' | ';') => (0.0, 0.17 * s),
                _ => (0.0, 0.0),
            };
            let x = out.width + before;
            out.place(b, x, 0.0);
            out.width += after;
            prev_operand = op.is_none() || matches!(op, Some(')' | ']' | '}' | '!' | '\''));
        }
        out
    }

    /// Scripts to the right of `b`.
    fn scripts(&self, b: MathBox, sub: Option<MathBox>, sup: Option<MathBox>, s: f32) -> MathBox {
        let x = b.width + 0.04 * s;
        let mut out = MathBox::default();
        let (base_a, base_d) = (b.ascent, b.descent);
        out.place(b, 0.0, 0.0);
        let both = sub.is_some() && sup.is_some();
        if let Some(sp) = sup {
            let up = (base_a - 0.3 * s).max(0.38 * s) + if both { 0.05 * s } else { 0.0 };
            out.place(sp, x, -up);
        }
        if let Some(sb) = sub {
            let down = (base_d + 0.05 * s).max(0.2 * s) + if both { 0.1 * s } else { 0.0 };
            out.place(sb, x, down);
        }
        out.width += 0.04 * s;
        out
    }

    /// Limits under and over `b`, centred.
    fn limits(&self, b: MathBox, sub: Option<MathBox>, sup: Option<MathBox>, s: f32) -> MathBox {
        let w = b.width.max(sub.as_ref().map_or(0.0, |x| x.width)).max(sup.as_ref().map_or(0.0, |x| x.width));
        let gap = 0.12 * s;
        let (ba, bd) = (b.ascent, b.descent);
        let mut out = MathBox::default();
        let bw = b.width;
        out.place(b, (w - bw) / 2.0, 0.0);
        if let Some(sp) = sup {
            let dy = -(ba + gap + sp.descent);
            let x = (w - sp.width) / 2.0;
            out.place(sp, x, dy);
        }
        if let Some(sb) = sub {
            let dy = bd + gap + sb.ascent;
            let x = (w - sb.width) / 2.0;
            out.place(sb, x, dy);
        }
        out.width = w;
        out
    }

    fn radical(&self, degree: Option<&Node>, body: &Node, s: f32, d: usize) -> MathBox {
        let b = self.node(body, s, false, d);
        let gap = 0.12 * s;
        let top = -(b.ascent.max(0.7 * s) + gap);
        let bottom = b.descent.max(0.2 * s) + 0.04 * s;
        let h = bottom - top;
        let sw = (0.5 * s).max(h * 0.25);
        let deg = degree.map(|x| self.node(x, (s * 0.5).max(s.min(4.0)), false, d));
        let lead = deg.as_ref().map_or(0.0, |g| (g.width - sw * 0.45).max(0.0));
        let mut out = MathBox::default();
        let t = line(s);
        let x0 = lead;
        let points = vec![
            (x0, bottom - h * 0.42),
            (x0 + sw * 0.22, bottom - h * 0.5),
            (x0 + sw * 0.55, bottom),
            (x0 + sw, top),
            (x0 + sw + b.width + 0.1 * s, top),
        ];
        out.items.push(Item::Path { points, width: t });
        out.ascent = -top + t;
        out.descent = bottom;
        let bx = x0 + sw + 0.05 * s;
        out.place(b, bx, 0.0);
        out.width = out.width.max(bx) + 0.1 * s;
        if let Some(g) = deg {
            let dy = bottom - h * 0.55 - g.descent;
            let x = (x0 + sw * 0.45 - g.width).max(0.0);
            out.place(g, x, dy);
        }
        out
    }

    /// A large operator drawn as lines.
    fn nary_sign(&self, op: char, s: f32, display: bool) -> MathBox {
        let h = if display { 1.5 * s } else { 1.15 * s };
        let ax = axis(s);
        let (top, bottom) = (-ax - h / 2.0, -ax + h / 2.0);
        let t = line(s) * 1.3;
        let mut items = Vec::new();
        let integral = |x: f32, items: &mut Vec<Item>| {
            let w = 0.5 * s;
            let pts: Vec<(f32, f32)> = (0..=16)
                .map(|i| {
                    let u = i as f32 / 16.0;
                    // A slanted stem with hooks at both ends.
                    let y = bottom + (top - bottom) * u;
                    let hook = (u * PI).cos();
                    (x + w * (0.15 + 0.7 * u) - 0.12 * w * hook * hook.abs().powf(6.0), y)
                })
                .collect();
            items.push(Item::Path { points: pts, width: t });
            items.push(Item::Path { points: vec![(x, bottom - 0.04 * h), (x + 0.12 * w, bottom), (x + 0.2 * w, bottom - 0.02 * h)], width: t });
            items.push(Item::Path { points: vec![(x + 0.8 * w, top + 0.02 * h), (x + 0.88 * w, top), (x + w, top + 0.04 * h)], width: t });
        };
        let w = match op {
            '∑' => {
                let w = 0.85 * s;
                items.push(Item::Path {
                    points: vec![
                        (w, top + 0.14 * h),
                        (w * 0.96, top),
                        (0.0, top),
                        (w * 0.5, -ax),
                        (0.0, bottom),
                        (w * 0.96, bottom),
                        (w, bottom - 0.14 * h),
                    ],
                    width: t,
                });
                w
            }
            '∏' | '∐' => {
                let w = 0.85 * s;
                let (a, b) = if op == '∏' { (top, bottom) } else { (bottom, top) };
                items.push(Item::Path { points: vec![(0.0, a), (w, a)], width: t });
                items.push(Item::Path { points: vec![(0.15 * w, a), (0.15 * w, b)], width: t });
                items.push(Item::Path { points: vec![(0.85 * w, a), (0.85 * w, b)], width: t });
                w
            }
            '⋃' | '⋂' | '⋁' | '⋀' => {
                let w = 0.75 * s;
                let up = op == '⋂' || op == '⋀';
                let pts: Vec<(f32, f32)> = if matches!(op, '⋃' | '⋂') {
                    (0..=16)
                        .map(|i| {
                            let u = i as f32 / 16.0;
                            let (x, y) = if u < 0.3 {
                                (0.0, top + (bottom - top) * 0.7 * (u / 0.3))
                            } else if u > 0.7 {
                                (w, top + (bottom - top) * 0.7 * ((1.0 - u) / 0.3))
                            } else {
                                let a = (u - 0.3) / 0.4 * PI;
                                (w / 2.0 - w / 2.0 * a.cos(), top + (bottom - top) * (0.7 + 0.3 * a.sin()))
                            };
                            (x, if up { top + bottom - y } else { y })
                        })
                        .collect()
                } else if up {
                    vec![(0.0, bottom), (w / 2.0, top), (w, bottom)]
                } else {
                    vec![(0.0, top), (w / 2.0, bottom), (w, top)]
                };
                items.push(Item::Path { points: pts, width: t });
                w
            }
            _ => {
                let n = match op {
                    '∬' => 2,
                    '∭' => 3,
                    _ => 1,
                };
                for k in 0..n {
                    integral(k as f32 * 0.3 * s, &mut items);
                }
                if op == '∮' {
                    let (cx, cy, r) = (0.25 * s, -ax, 0.16 * s);
                    let ring = (0..=16).map(|i| (cx + r * (i as f32 / 16.0 * 2.0 * PI).cos(), cy + r * (i as f32 / 16.0 * 2.0 * PI).sin())).collect();
                    items.push(Item::Path { points: ring, width: line(s) });
                }
                0.5 * s + (n - 1) as f32 * 0.3 * s
            }
        };
        MathBox { width: w, ascent: -top + t, descent: bottom + t, items }
    }

    /// One stretchy bracket at `x` from `top` to `bottom`; returns its width.
    fn delim(&self, out: &mut MathBox, c: char, x: f32, top: f32, bottom: f32, s: f32) -> f32 {
        let h = bottom - top;
        let t = line(s);
        let mid = (top + bottom) / 2.0;
        let (w, pts): (f32, Vec<Vec<(f32, f32)>>) = match c {
            '(' | ')' => {
                let w = 0.3 * s + 0.04 * h;
                let pts = (0..=16)
                    .map(|i| {
                        let u = i as f32 / 16.0;
                        let bulge = (u * PI).sin();
                        let px = if c == '(' { w * (0.9 - 0.75 * bulge) } else { w * (0.1 + 0.75 * bulge) };
                        (x + px, top + h * u)
                    })
                    .collect();
                (w, vec![pts])
            }
            '[' | ']' | '⌊' | '⌋' | '⌈' | '⌉' => {
                let w = 0.28 * s;
                let (outer, inner) = if matches!(c, '[' | '⌊' | '⌈') { (w * 0.85, w * 0.3) } else { (w * 0.15, w * 0.7) };
                let top_bar = !matches!(c, '⌊' | '⌋');
                let bottom_bar = !matches!(c, '⌈' | '⌉');
                let mut p = Vec::new();
                if top_bar {
                    p.push((x + outer, top));
                }
                p.push((x + inner, top));
                p.push((x + inner, bottom));
                if bottom_bar {
                    p.push((x + outer, bottom));
                }
                (w, vec![p])
            }
            '{' | '}' => {
                let w = 0.38 * s;
                let f = |a: f32| if c == '{' { x + w * a } else { x + w * (1.0 - a) };
                (
                    w,
                    vec![vec![
                        (f(0.85), top),
                        (f(0.55), top + 0.06 * h),
                        (f(0.5), mid - 0.08 * h),
                        (f(0.12), mid),
                        (f(0.5), mid + 0.08 * h),
                        (f(0.55), bottom - 0.06 * h),
                        (f(0.85), bottom),
                    ]],
                )
            }
            '|' => (0.22 * s, vec![vec![(x + 0.11 * s, top), (x + 0.11 * s, bottom)]]),
            '‖' => (0.32 * s, vec![vec![(x + 0.1 * s, top), (x + 0.1 * s, bottom)], vec![(x + 0.22 * s, top), (x + 0.22 * s, bottom)]]),
            '⟨' | '⟩' => {
                let w = 0.3 * s;
                let (a, b) = if c == '⟨' { (w * 0.85, w * 0.15) } else { (w * 0.15, w * 0.85) };
                (w, vec![vec![(x + a, top), (x + b, mid), (x + a, bottom)]])
            }
            other => {
                // Anything else is drawn as text at the normal size.
                let b = self.text(&other.to_string(), s, false);
                let w = b.width;
                out.place(b, x, 0.0);
                return w;
            }
        };
        for p in pts {
            out.items.push(Item::Path { points: p, width: t });
        }
        out.width = out.width.max(x + w);
        w
    }

    fn matrix(&self, rows: &[Vec<Node>], s: f32, d: usize) -> MathBox {
        let cells: Vec<Vec<MathBox>> = rows.iter().map(|r| r.iter().map(|c| self.node(c, s, false, d)).collect()).collect();
        let n_cols = cells.iter().map(Vec::len).max().unwrap_or(0);
        let col_w: Vec<f32> = (0..n_cols).map(|c| cells.iter().filter_map(|r| r.get(c)).map(|b| b.width).fold(0.0, f32::max)).collect();
        let (col_gap, row_gap) = (0.8 * s, 0.25 * s);
        let row_h: Vec<(f32, f32)> =
            cells.iter().map(|r| (r.iter().map(|b| b.ascent).fold(0.7 * s, f32::max), r.iter().map(|b| b.descent).fold(0.2 * s, f32::max))).collect();
        let total: f32 = row_h.iter().map(|(a, d)| a + d).sum::<f32>() + row_gap * row_h.len().saturating_sub(1) as f32;
        let mut y = -axis(s) - total / 2.0;
        let mut out = MathBox { width: 0.0, ascent: axis(s) + total / 2.0, descent: (total / 2.0 - axis(s)).max(0.0), items: Vec::new() };
        for (r, row) in cells.into_iter().enumerate() {
            let (a, dsc) = row_h.get(r).copied().unwrap_or((0.0, 0.0));
            let base = y + a;
            let mut x = 0.0;
            for (c, b) in row.into_iter().enumerate() {
                let cw = col_w.get(c).copied().unwrap_or(0.0);
                let bw = b.width;
                out.place(b, x + (cw - bw) / 2.0, base);
                x += cw + col_gap;
            }
            y += a + dsc + row_gap;
        }
        out.width = col_w.iter().sum::<f32>() + col_gap * n_cols.saturating_sub(1) as f32;
        out
    }
}

fn fin(v: f32) -> f32 {
    if v.is_finite() { v.max(0.0) } else { 0.0 }
}
