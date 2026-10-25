//! The 2024-style icon set (decision D1, `docs/ui-parity.md` §3 row 24), drawn in code: original
//! artwork from WordCraft (same team, MIT/Apache; `ATTRIBUTION.md`), no external icon assets and
//! nothing traced from any other product.
//!
//! Icons are designed on a 20×20 grid in two colours: line work (`c`) and an accent (`a`).
//! `icons::draw` maps catalog controls onto these names; controls with no counterpart here keep
//! NoteCraft's own drawings in `icons.rs`.

use egui::{Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

struct Pen<'a> {
    p: &'a Painter,
    r: Rect,
    c: Color32,
    a: Color32,
    w: f32,
}

impl Pen<'_> {
    fn s(&self) -> f32 {
        self.r.width().min(self.r.height()) / 20.0
    }
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        let s = self.s();
        let o = self.r.center() - vec2(10.0 * s, 10.0 * s);
        pos2(o.x + x * s, o.y + y * s)
    }
    fn stroke(&self, c: Color32) -> Stroke {
        Stroke::new(self.w * self.s(), c)
    }
    fn line(&self, pts: &[(f32, f32)]) {
        self.line_c(pts, self.c);
    }
    fn line_c(&self, pts: &[(f32, f32)], c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::line(v, self.stroke(c)));
    }
    fn closed(&self, pts: &[(f32, f32)], c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::closed_line(v, self.stroke(c)));
    }
    fn fill(&self, pts: &[(f32, f32)], c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::convex_polygon(v, c, Stroke::NONE));
    }
    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32) {
        self.p.rect_stroke(Rect::from_min_max(self.pt(x0, y0), self.pt(x1, y1)), 1.0, self.stroke(c), egui::StrokeKind::Middle);
    }
    fn frect(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32) {
        self.p.rect_filled(Rect::from_min_max(self.pt(x0, y0), self.pt(x1, y1)), 0.5, c);
    }
    fn circle(&self, x: f32, y: f32, r: f32, c: Color32) {
        self.p.circle_stroke(self.pt(x, y), r * self.s(), self.stroke(c));
    }
    fn fcircle(&self, x: f32, y: f32, r: f32, c: Color32) {
        self.p.circle_filled(self.pt(x, y), r * self.s(), c);
    }
    fn text(&self, x: f32, y: f32, size: f32, t: &str, c: Color32, bold: bool) {
        let font = if bold {
            crate::theme::font(self.p.ctx(), size * self.s(), crate::theme::Weight::Semibold)
        } else {
            FontId::proportional(size * self.s())
        };
        self.p.text(self.pt(x, y), egui::Align2::CENTER_CENTER, t, font, c);
    }
    /// Horizontal text lines (a paragraph).
    fn lines(&self, x0: f32, x1: f32, ys: &[f32]) {
        for y in ys {
            self.line(&[(x0, *y), (x1, *y)]);
        }
    }
    /// A page outline with a folded corner.
    fn page(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.closed(&[(x0, y0), (x1 - 4.0, y0), (x1, y0 + 4.0), (x1, y1), (x0, y1)], self.c);
        self.line(&[(x1 - 4.0, y0), (x1 - 4.0, y0 + 4.0), (x1, y0 + 4.0)]);
    }
}

/// Paint icon `name` into `r`. `c` is the line colour; the accent comes from the theme.
pub fn paint(p: &Painter, r: Rect, name: &str, c: Color32, accent: Color32) {
    let pen = Pen { p, r, c, a: accent, w: 1.25 };
    let a = accent;
    let red = Color32::from_rgb(0xD1, 0x3B, 0x3B);
    let yellow = Color32::from_rgb(0xF2, 0xC8, 0x11);
    let green = Color32::from_rgb(0x2E, 0x8B, 0x57);
    match name {
        // Clipboard
        "paste" => {
            pen.rect(4.0, 3.5, 14.0, 17.0, c);
            pen.frect(7.0, 2.0, 11.0, 5.0, a);
            pen.frect(9.0, 8.0, 17.5, 18.5, pen.a.linear_multiply(0.18));
            pen.rect(9.0, 8.0, 17.5, 18.5, a);
            pen.lines(10.8, 15.8, &[11.0, 13.5, 16.0]);
        }
        "cut" => {
            pen.circle(6.0, 15.0, 2.5, c);
            pen.circle(14.0, 15.0, 2.5, c);
            pen.line(&[(7.5, 13.0), (14.0, 3.0)]);
            pen.line(&[(12.5, 13.0), (6.0, 3.0)]);
        }
        "copy" => {
            pen.rect(3.5, 3.0, 12.5, 14.0, c);
            pen.frect(7.5, 6.5, 16.5, 17.5, Color32::TRANSPARENT);
            pen.rect(7.5, 6.5, 16.5, 17.5, a);
        }
        "painter" => {
            pen.frect(4.0, 3.0, 16.0, 8.0, yellow);
            pen.rect(4.0, 3.0, 16.0, 8.0, c);
            pen.line(&[(16.0, 5.5), (17.5, 5.5), (17.5, 10.0), (10.0, 10.0), (10.0, 12.0)]);
            pen.frect(8.8, 12.0, 11.2, 18.0, c);
        }
        // Font
        "bold" => pen.text(10.0, 10.5, 15.0, "B", c, true),
        "italic" => pen.text(10.0, 10.5, 15.0, "I", c, false),
        "underline" => {
            pen.text(10.0, 9.0, 14.0, "U", c, false);
            pen.line(&[(5.0, 17.0), (15.0, 17.0)]);
        }
        "strike" => {
            pen.text(10.0, 10.0, 12.0, "ab", c, false);
            pen.line_c(&[(3.0, 10.5), (17.0, 10.5)], c);
        }
        "subscript" => {
            pen.text(8.0, 9.0, 13.0, "x", c, false);
            pen.text(15.0, 14.5, 8.0, "2", a, true);
        }
        "superscript" => {
            pen.text(8.0, 11.0, 13.0, "x", c, false);
            pen.text(15.0, 5.0, 8.0, "2", a, true);
        }
        "clear" => {
            pen.text(8.0, 9.0, 13.0, "A", c, false);
            pen.fill(&[(10.0, 15.0), (14.0, 11.0), (18.0, 15.0), (14.0, 19.0)], Color32::from_rgb(0xE8, 0x6F, 0xA0));
        }
        "highlight" => {
            pen.fill(&[(5.0, 12.0), (11.0, 4.0), (14.0, 7.0), (8.0, 15.0)], c);
            pen.line(&[(5.0, 12.0), (3.5, 15.0), (8.0, 15.0)]);
            pen.frect(2.0, 16.5, 18.0, 19.0, yellow);
        }
        "fontcolor" => {
            pen.text(10.0, 9.0, 14.0, "A", c, false);
            pen.frect(2.0, 16.5, 18.0, 19.0, red);
        }
        // Paragraph
        "bullets" => {
            for (i, y) in [5.0, 10.0, 15.0].iter().enumerate() {
                pen.fcircle(4.0, *y, 1.4, if i == 0 { a } else { c });
                pen.line(&[(7.5, *y), (17.0, *y)]);
            }
        }
        "numbering" => {
            for (i, y) in [5.0, 10.0, 15.0].iter().enumerate() {
                pen.text(4.0, *y, 6.0, &(i + 1).to_string(), if i == 0 { a } else { c }, true);
                pen.line(&[(7.5, *y), (17.0, *y)]);
            }
        }
        "outdent" | "indent" => {
            pen.lines(9.0, 17.0, &[7.0, 10.0, 13.0]);
            pen.lines(3.0, 17.0, &[3.5, 16.5]);
            if name == "indent" {
                pen.fill(&[(3.0, 7.0), (3.0, 13.0), (6.5, 10.0)], a);
            } else {
                pen.fill(&[(6.5, 7.0), (6.5, 13.0), (3.0, 10.0)], a);
            }
        }
        "sort" => {
            pen.text(6.0, 5.5, 7.0, "A", c, true);
            pen.text(6.0, 14.5, 7.0, "Z", c, true);
            pen.line_c(&[(14.0, 3.0), (14.0, 17.0)], a);
            pen.line_c(&[(11.5, 14.5), (14.0, 17.0), (16.5, 14.5)], a);
        }
        "shading" => {
            pen.fill(&[(4.0, 9.0), (10.0, 3.0), (16.0, 9.0), (10.0, 15.0)], Color32::TRANSPARENT);
            pen.closed(&[(4.0, 9.0), (10.0, 3.0), (16.0, 9.0), (10.0, 15.0)], c);
            pen.fcircle(16.5, 13.5, 1.6, a);
            pen.frect(2.0, 16.5, 18.0, 19.0, yellow);
        }
        // Editing
        "select" => {
            pen.fill(&[(5.0, 3.0), (5.0, 16.0), (8.0, 13.0), (10.5, 18.0), (12.5, 17.0), (10.0, 12.0), (14.0, 12.0)], c);
        }
        "dictate" => {
            pen.rect(7.5, 2.5, 12.5, 12.0, c);
            pen.line(&[(5.0, 9.5), (5.0, 11.0), (10.0, 15.0), (15.0, 11.0), (15.0, 9.5)]);
            pen.line(&[(10.0, 15.0), (10.0, 18.0)]);
        }
        "spelling" => {
            pen.text(8.0, 7.0, 9.0, "abc", c, true);
            pen.line_c(&[(5.0, 13.0), (8.5, 16.5), (16.0, 9.0)], green);
        }
        "styles" => {
            pen.text(10.0, 10.0, 14.0, "A", a, false);
            pen.line(&[(13.0, 15.0), (17.0, 11.0)]);
        }
        // Insert
        "table" => {
            pen.frect(3.0, 3.0, 17.0, 7.0, a);
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            pen.line(&[(3.0, 7.0), (17.0, 7.0)]);
            pen.line(&[(3.0, 12.0), (17.0, 12.0)]);
            pen.line(&[(8.0, 3.0), (8.0, 17.0)]);
            pen.line(&[(12.5, 3.0), (12.5, 17.0)]);
        }
        "picture" => {
            pen.rect(2.5, 4.0, 17.5, 16.0, c);
            pen.fill(&[(4.0, 14.5), (8.5, 8.0), (12.0, 14.5)], a);
            pen.fill(&[(9.5, 14.5), (13.0, 10.0), (16.0, 14.5)], green);
            pen.fcircle(13.5, 7.0, 1.5, yellow);
        }
        "shapes" => {
            pen.rect(3.0, 3.0, 11.0, 11.0, c);
            pen.fcircle(13.0, 13.0, 4.5, a);
        }
        "screenshot" => {
            pen.line(&[(3.0, 7.0), (3.0, 3.0), (7.0, 3.0)]);
            pen.line(&[(13.0, 3.0), (17.0, 3.0), (17.0, 7.0)]);
            pen.line(&[(17.0, 13.0), (17.0, 17.0), (13.0, 17.0)]);
            pen.line(&[(7.0, 17.0), (3.0, 17.0), (3.0, 13.0)]);
            pen.fcircle(10.0, 10.0, 3.0, a);
        }
        "video" => {
            pen.rect(2.5, 5.0, 14.0, 15.0, c);
            pen.fill(&[(14.0, 10.0), (18.0, 6.5), (18.0, 13.5)], a);
        }
        "link" => {
            pen.closed(
                &[(3.0, 10.0), (7.0, 6.0), (9.5, 6.0), (9.5, 8.0), (7.5, 8.0), (5.0, 10.5), (7.5, 13.0), (9.5, 13.0), (9.5, 15.0), (7.0, 15.0)],
                c,
            );
            pen.line_c(&[(7.5, 10.5), (12.5, 10.5)], a);
            pen.closed(
                &[
                    (17.0, 10.5),
                    (13.0, 14.5),
                    (10.5, 14.5),
                    (10.5, 12.5),
                    (12.5, 12.5),
                    (15.0, 10.0),
                    (12.5, 7.5),
                    (10.5, 7.5),
                    (10.5, 5.5),
                    (13.0, 5.5),
                ],
                c,
            );
        }
        "comment" => {
            pen.closed(&[(2.5, 3.5), (17.5, 3.5), (17.5, 13.5), (9.0, 13.5), (5.0, 17.0), (5.0, 13.5), (2.5, 13.5)], c);
            if name == "newComment" {
                pen.line_c(&[(10.0, 6.0), (10.0, 11.0)], a);
                pen.line_c(&[(7.5, 8.5), (12.5, 8.5)], a);
            } else {
                pen.lines(5.5, 14.5, &[7.0, 10.0]);
            }
        }
        "equation" => pen.text(10.0, 10.0, 17.0, "π", a, false),
        "symbol" => pen.text(10.0, 10.0, 17.0, "Ω", c, false),
        // Draw
        "pen" => {
            pen.line(&[(4.0, 16.0), (14.0, 6.0), (16.0, 8.0), (6.0, 18.0), (3.5, 18.5), (4.0, 16.0)]);
            pen.fill(&[(14.0, 6.0), (15.5, 4.5), (17.5, 6.5), (16.0, 8.0)], a);
        }
        "eraser" => {
            pen.closed(&[(3.0, 13.0), (11.0, 5.0), (17.0, 11.0), (11.0, 17.0), (7.0, 17.0)], c);
            pen.fill(&[(3.0, 13.0), (7.0, 9.0), (13.0, 15.0), (11.0, 17.0), (7.0, 17.0)], Color32::from_rgb(0xE8, 0x6F, 0xA0));
        }
        // Ink thickness: three strokes, thin to thick.
        "thickness" => {
            pen.frect(3.0, 4.5, 17.0, 5.5, c);
            pen.frect(3.0, 9.0, 17.0, 11.0, c);
            pen.frect(3.0, 14.0, 17.0, 17.0, a);
        }
        "lasso" => {
            pen.closed(&[(4.0, 7.0), (9.0, 3.0), (16.0, 5.0), (16.0, 11.0), (10.0, 13.0), (4.0, 11.0)], c);
            pen.line_c(&[(6.0, 12.5), (5.0, 17.5)], a);
        }
        "replay" => {
            pen.circle(10.0, 10.0, 6.5, c);
            pen.fill(&[(8.5, 7.0), (8.5, 13.0), (13.0, 10.0)], a);
        }
        // Design
        "colors" => {
            pen.fcircle(7.0, 7.5, 3.5, red);
            pen.fcircle(13.0, 7.5, 3.5, a);
            pen.fcircle(10.0, 13.0, 3.5, green);
        }
        // Layout
        "size" => {
            pen.page(5.0, 2.0, 15.0, 18.0);
            pen.line_c(&[(2.5, 2.5), (2.5, 17.5)], a);
            pen.line_c(&[(5.0, 19.0), (15.0, 19.0)], a);
        }
        "position" => {
            pen.lines(3.0, 17.0, &[3.0, 17.0]);
            pen.lines(3.0, 6.0, &[7.0, 10.0, 13.0]);
            pen.frect(7.5, 6.0, 12.5, 14.0, a);
            pen.lines(14.0, 17.0, &[7.0, 10.0, 13.0]);
        }
        "rotate" => {
            pen.circle(10.0, 10.0, 6.0, c);
            pen.fill(&[(14.5, 3.0), (17.5, 6.0), (13.5, 7.0)], a);
        }
        // References
        "researcher" => {
            pen.circle(9.0, 9.0, 5.0, c);
            pen.line_c(&[(12.5, 12.5), (17.0, 17.0)], a);
            pen.lines(6.5, 11.5, &[8.0, 10.0]);
        }
        // Mailings
        "envelope" => {
            pen.rect(2.5, 5.0, 17.5, 15.5, c);
            pen.line_c(&[(2.5, 5.0), (10.0, 11.0), (17.5, 5.0)], a);
        }
        // Review
        "thesaurus" => {
            pen.rect(3.5, 2.5, 16.5, 17.5, c);
            pen.text(10.0, 9.5, 8.0, "Aa", a, true);
        }
        "accessibility" => {
            pen.fcircle(10.0, 4.0, 1.6, a);
            pen.line(&[(4.0, 7.0), (16.0, 7.0)]);
            pen.line(&[(10.0, 7.0), (10.0, 12.0), (7.0, 17.5)]);
            pen.line(&[(10.0, 12.0), (13.0, 17.5)]);
        }
        "translate" | "language" => {
            pen.text(6.5, 7.0, 9.0, "A", c, true);
            pen.text(13.5, 13.5, 9.0, "あ", a, false);
        }
        "compare" => {
            pen.page(2.0, 3.0, 10.0, 15.0);
            pen.page(10.0, 5.0, 18.0, 17.0);
            pen.line_c(&[(6.0, 18.5), (14.0, 18.5)], a);
        }
        "protect" => {
            pen.rect(5.0, 9.0, 15.0, 17.5, c);
            pen.line_c(&[(7.0, 9.0), (7.0, 6.0), (10.0, 3.0), (13.0, 6.0), (13.0, 9.0)], a);
        }
        "immersive" => {
            pen.closed(&[(2.0, 4.0), (9.5, 5.0), (9.5, 17.0), (2.0, 16.0)], c);
            pen.closed(&[(18.0, 4.0), (10.5, 5.0), (10.5, 17.0), (18.0, 16.0)], c);
            pen.fcircle(14.0, 10.0, 2.0, a);
        }
        // View
        "ruler" => {
            pen.rect(2.0, 7.0, 18.0, 13.0, c);
            for x in [4.5, 7.0, 9.5, 12.0, 14.5] {
                pen.line_c(&[(x, 7.0), (x, 10.0)], a);
            }
        }
        "gridlines" => {
            pen.rect(3.0, 3.0, 17.0, 17.0, c);
            for k in [6.5, 10.0, 13.5] {
                pen.line_c(&[(k, 3.0), (k, 17.0)], a.linear_multiply(0.6));
                pen.line_c(&[(3.0, k), (17.0, k)], a.linear_multiply(0.6));
            }
        }
        "zoom" => {
            pen.circle(8.5, 8.5, 5.5, c);
            pen.line_c(&[(12.5, 12.5), (17.5, 17.5)], a);
            pen.line(&[(6.0, 8.5), (11.0, 8.5)]);
            pen.line(&[(8.5, 6.0), (8.5, 11.0)]);
        }
        "zoom100" => pen.text(10.0, 10.0, 7.5, "100", a, true),
        // Chrome
        "info" => {
            pen.circle(10.0, 10.0, 7.0, c);
            pen.line_c(&[(10.0, 9.0), (10.0, 14.5)], a);
            pen.fcircle(10.0, 6.3, 1.0, a);
        }
        "pin" => {
            pen.line(&[(10.0, 12.0), (10.0, 18.0)]);
            pen.rect(7.0, 2.0, 13.0, 9.0, c);
            pen.line(&[(5.0, 12.0), (15.0, 12.0)]);
        }
        "addins" => {
            pen.rect(3.0, 3.0, 9.0, 9.0, c);
            pen.rect(11.0, 3.0, 17.0, 9.0, c);
            pen.rect(3.0, 11.0, 9.0, 17.0, c);
            pen.line_c(&[(14.0, 11.0), (14.0, 17.0)], a);
            pen.line_c(&[(11.0, 14.0), (17.0, 14.0)], a);
        }
        "file" => {
            pen.page(4.5, 2.0, 15.5, 18.0);
            pen.lines(7.0, 13.0, &[9.0, 12.0, 15.0]);
        }
        "help" => {
            pen.circle(10.0, 10.0, 7.0, c);
            pen.text(10.0, 10.0, 10.0, "?", a, true);
        }
        _ => {
            // Fallback: a rounded tile with the first letter.
            p.rect_stroke(r.shrink(r.width() * 0.12), 3.0, Stroke::new(1.0, c), egui::StrokeKind::Middle);
            let ch = name.chars().next().map(|x| x.to_ascii_uppercase().to_string()).unwrap_or_default();
            p.text(r.center(), egui::Align2::CENTER_CENTER, ch, FontId::proportional(r.height() * 0.45), a);
        }
    }
}
