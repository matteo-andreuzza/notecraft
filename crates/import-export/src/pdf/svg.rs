//! A page as one SVG picture (FMT.Export.Images): the PDF's display list (the same layout,
//! fonts and pictures as File › Export › PDF and the PNG picture) written as SVG 1.1 instead of
//! PDF. Text is drawn as glyph outlines (`<defs>` once per glyph, `<use>` per character), so the
//! file looks the same without the fonts; pictures are embedded as `data:` URIs; links are
//! `<a>` areas. All lengths are points (`width="…pt"`, one user unit = one point).

use std::collections::HashMap;
use std::fmt::Write as _;

use notecraft_codecs::Format;
use notecraft_doc::Page;
use notecraft_doc::color::Rgba;

use super::text::Faces;
use super::{Display, Images, Op, Painter, PdfOptions, PdfReport, R, TITLE_AT, rule_segments};

/// Margin around the content, as the PNG picture has.
const MARGIN: f32 = 18.0;
/// Most glyph outlines one picture defines (bounds `<defs>` on hostile text).
const MAX_GLYPHS: usize = 20_000;

/// A number for SVG: two decimals at most, never NaN or infinite.
fn n(v: f32) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".to_string() } else { s.to_string() }
}

fn hex(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// `fill-opacity`/`stroke-opacity` for a colour's alpha times `alpha`, empty when opaque.
fn opacity(attr: &str, c: Rgba, alpha: f32) -> String {
    let o = (alpha * f32::from(c.a) / 255.0).clamp(0.0, 1.0);
    if o >= 0.999 { String::new() } else { format!(" {attr}=\"{}\"", n(o)) }
}

/// XML text and attribute escaping.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Characters XML 1.0 forbids.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

/// The `data:` URI of a picture: PNG, JPEG, GIF and WebP as they are, anything else the codecs
/// read re-encoded as PNG; `None` when unreadable.
fn data_uri(bytes: &[u8]) -> Option<String> {
    let mime = match notecraft_codecs::sniff(bytes) {
        Some(Format::Png) => "image/png",
        Some(Format::Jpeg) => "image/jpeg",
        Some(Format::Gif) => "image/gif",
        Some(Format::WebP) => "image/webp",
        _ => {
            let px = notecraft_codecs::decode(bytes, &notecraft_codecs::Limits::PICTURE).ok()?;
            let png = notecraft_codecs::encode_png(&px, &notecraft_codecs::Limits::PICTURE).ok()?;
            return Some(format!("data:image/png;base64,{}", notecraft_codecs::b64::encode(&png)));
        }
    };
    Some(format!("data:{mime};base64,{}", notecraft_codecs::b64::encode(bytes)))
}

/// SVG path data from a glyph outline (font units).
struct PathPen(String);

impl skrifa::outline::OutlinePen for PathPen {
    fn move_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.0, "M{} {}", n(x), n(y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.0, "L{} {}", n(x), n(y));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let _ = write!(self.0, "Q{} {} {} {}", n(cx0), n(cy0), n(x), n(y));
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let _ = write!(self.0, "C{} {} {} {} {} {}", n(cx0), n(cy0), n(cx1), n(cy1), n(x), n(y));
    }
    fn close(&mut self) {
        self.0.push('Z');
    }
}

/// Glyph outlines defined so far: (face, glyph) → the `<defs>` id, or `None` for an empty
/// glyph (a space) or one past the cap.
struct Glyphs<'f> {
    faces: &'f Faces,
    ids: HashMap<(usize, u32), Option<String>>,
    defs: String,
}

impl Glyphs<'_> {
    fn id(&mut self, face: usize, gid: u32) -> Option<String> {
        if let Some(id) = self.ids.get(&(face, gid)) {
            return id.clone();
        }
        let id = if self.ids.len() < MAX_GLYPHS {
            let mut pen = PathPen(String::new());
            let drawn = self.faces.faces.get(face).is_some_and(|f| f.draw_outline(gid, &mut pen));
            Some(pen.0).filter(|d| drawn && !d.is_empty()).map(|d| {
                let id = format!("g{face}-{gid}");
                let _ = writeln!(self.defs, "<path id=\"{id}\" d=\"{d}\"/>");
                id
            })
        } else {
            None
        };
        self.ids.insert((face, gid), id.clone());
        id
    }
}

fn draw(out: &mut String, glyphs: &mut Glyphs<'_>, images: &Images, uris: &mut HashMap<usize, Option<String>>, op: &Op) {
    match op {
        Op::Fill { r, color, alpha } => {
            let _ = writeln!(
                out,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"{}/>",
                n(r.x),
                n(r.y),
                n(r.w.max(0.01)),
                n(r.h.max(0.01)),
                hex(*color),
                opacity("fill-opacity", *color, *alpha)
            );
        }
        Op::Frame { r, width, color } => {
            let _ = writeln!(
                out,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>",
                n(r.x),
                n(r.y),
                n(r.w.max(0.01)),
                n(r.h.max(0.01)),
                hex(*color),
                n(*width)
            );
        }
        Op::Line { a, b, width, color } => {
            let _ = writeln!(
                out,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>",
                n(a.0),
                n(a.1),
                n(b.0),
                n(b.1),
                hex(*color),
                n(width.max(0.1))
            );
        }
        Op::Dot { c, r, color, ring } => {
            let paint = match ring {
                Some(w) => format!("fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"", hex(*color), n(*w)),
                None => format!("fill=\"{}\"", hex(*color)),
            };
            let _ = writeln!(out, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" {paint}/>", n(c.0), n(c.1), n(r.max(0.1)));
        }
        Op::Ink { points, width, color, alpha } => {
            let Some(first) = points.first() else { return };
            let mut d = format!("M{} {}", n(first.0), n(first.1));
            if points.len() == 1 {
                // A dot: a zero-length line with round caps.
                let _ = write!(d, "h0.01");
            }
            for p in points.iter().skip(1) {
                let _ = write!(d, "L{} {}", n(p.0), n(p.1));
            }
            let _ = writeln!(
                out,
                "<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"{}/>",
                hex(*color),
                n(*width),
                opacity("stroke-opacity", *color, *alpha)
            );
        }
        Op::Glyphs { x, baseline, face, look, glyphs: run, .. } => {
            let Some(upem) = glyphs.faces.faces.get(*face).map(|f| f.upem()) else { return };
            let size = look.size.max(0.5);
            let k = size / upem;
            let skew = if look.italic { format!(" transform=\"matrix(1 0 -0.21 1 {} 0)\"", n(0.21 * baseline)) } else { String::new() };
            let stroke = if look.fake_bold { format!(" stroke=\"{}\" stroke-width=\"{}\"", hex(look.color), n(0.03 * upem)) } else { String::new() };
            let _ = writeln!(out, "<g fill=\"{}\"{stroke}{skew}>", hex(look.color));
            let mut at = *x;
            for (gid, adv) in run {
                if let Some(id) = glyphs.id(*face, *gid) {
                    let _ = writeln!(out, "<use xlink:href=\"#{id}\" transform=\"matrix({} 0 0 {} {} {})\"/>", n4(k), n4(-k), n(at), n(*baseline));
                }
                at += adv;
            }
            let _ = writeln!(out, "</g>");
        }
        Op::Picture { r, image, turns, flip_h, flip_v } => {
            let uri = uris.entry(*image).or_insert_with(|| images.src.get(*image).and_then(|b| data_uri(b))).clone();
            let Some(uri) = uri else { return };
            // The same transform as the PDF: mirrored, then turned clockwise; `r` is the result.
            let (w0, h0) = if turns % 2 == 1 { (r.h, r.w) } else { (r.w, r.h) };
            let (c, sn) = match turns % 4 {
                1 => (0.0, 1.0),
                2 => (-1.0, 0.0),
                3 => (0.0, -1.0),
                _ => (1.0, 0.0),
            };
            let (fx, fy) = (if *flip_h { -1.0 } else { 1.0 }, if *flip_v { -1.0 } else { 1.0 });
            let (a, b, cc, d) = (c * fx, sn * fx, -sn * fy, c * fy);
            let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
            let (tx, ty) = (cx - (a * w0 / 2.0 + cc * h0 / 2.0), cy - (b * w0 / 2.0 + d * h0 / 2.0));
            let _ = writeln!(
                out,
                "<image width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" transform=\"matrix({} {} {} {} {} {})\" xlink:href=\"{uri}\"/>",
                n(w0.max(0.01)),
                n(h0.max(0.01)),
                n(a),
                n(b),
                n(cc),
                n(d),
                n(tx),
                n(ty)
            );
        }
    }
}

/// A scale factor with more precision than [`n`] (glyph scales are small).
fn n4(v: f32) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let s = format!("{v:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `page` as one SVG picture as large as its content, with the page fonts `fonts`.
pub fn write(page: &Page, fonts: &notecraft_fonts::PageFonts, opts: &PdfOptions) -> Result<(String, PdfReport), String> {
    let mut faces = Faces::new(fonts).ok_or("no font for the picture: install Carlito, Noto Sans, Liberation Sans or DejaVu Sans")?;
    let mut images = Images::default();
    let mut report = PdfReport::default();
    let mut p = Painter { faces: &mut faces, images: &mut images, d: Display::default(), report: &mut report };
    p.d.left = f32::MAX;
    p.d.top = f32::MAX;
    p.d.right = f32::MIN;
    p.d.bottom = f32::MIN;
    p.page(page, opts);
    let d = p.d;
    let (left, top, right, bottom) = if d.left <= d.right {
        (d.left.min(TITLE_AT.0), d.top.min(TITLE_AT.1), d.right, d.bottom)
    } else {
        (TITLE_AT.0, TITLE_AT.1, TITLE_AT.0, TITLE_AT.1)
    };
    let (w, h) = ((right - left).clamp(36.0, 14_000.0) + 2.0 * MARGIN, (bottom - top).clamp(36.0, 14_000.0) + 2.0 * MARGIN);
    let mut out = String::new();
    let _ = writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" version=\"1.1\" width=\"{}pt\" height=\"{}pt\" viewBox=\"0 0 {} {}\">",
        n(w),
        n(h),
        n(w),
        n(h)
    );
    if !opts.title.trim().is_empty() {
        let _ = writeln!(out, "<title>{}</title>", esc(&opts.title));
    }
    // The page colour, else white like the PNG picture.
    let _ = writeln!(out, "<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>", hex(page.setup.color.unwrap_or(Rgba::rgb(0xFF, 0xFF, 0xFF))));
    let to_sheet = |x: f32, y: f32| (MARGIN + x - left, MARGIN + y - top);
    if let Some((lines, color, margin)) = rule_segments(page, w, h, &to_sheet, 1.0) {
        let mut dd = String::new();
        for [x0, y0, x1, y1] in lines {
            let _ = write!(dd, "M{} {}L{} {}", n(x0), n(y0), n(x1), n(y1));
        }
        let _ = writeln!(out, "<path d=\"{dd}\" fill=\"none\" stroke=\"{}\" stroke-width=\"0.75\"/>", hex(color));
        if let Some((mx, mc)) = margin {
            let _ = writeln!(out, "<line x1=\"{}\" y1=\"0\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"0.75\"/>", n(mx), n(mx), n(h), hex(mc));
        }
    }
    let mut glyphs = Glyphs { faces: &faces, ids: HashMap::new(), defs: String::new() };
    let mut uris = HashMap::new();
    let mut body = String::new();
    let _ = writeln!(body, "<g transform=\"translate({} {})\">", n(MARGIN - left), n(MARGIN - top));
    for op in &d.ops {
        draw(&mut body, &mut glyphs, &images, &mut uris, op);
    }
    for (r, url) in &d.links {
        let R { x, y, w, h } = *r;
        let _ = writeln!(
            body,
            "<a xlink:href=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#000\" fill-opacity=\"0\"/></a>",
            esc(url),
            n(x),
            n(y),
            n(w.max(0.5)),
            n(h.max(0.5))
        );
    }
    let _ = writeln!(body, "</g>");
    if !glyphs.defs.is_empty() {
        let _ = writeln!(out, "<defs>\n{}</defs>", glyphs.defs);
    }
    out.push_str(&body);
    out.push_str("</svg>\n");
    report.pages = 1;
    report.sheets = 1;
    Ok((out, report))
}
