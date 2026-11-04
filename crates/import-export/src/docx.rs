//! Word documents out (catalog `BS.Export.FormatDocx`, `FMT.Export.Docx`): an [`Export`] as a
//! WordprocessingML package (ECMA-376 Part 1, transitional), written from the public standard.
//!
//! **The package:** `[Content_Types].xml`, `_rels/.rels`, `docProps/core.xml` (the title),
//! `word/document.xml`, `word/styles.xml`, `word/numbering.xml`, `word/_rels/document.xml.rels`
//! and the pictures under `word/media/`.
//!
//! **What carries over:** each page's title (style Title) and date line, then its objects in
//! reading order, a page break between pages; paragraph styles as Word styles built from
//! NoteCraft's own definitions ([`ParaStyle::def`]), with their built-in names for headings,
//! Title and Quote; character formats (font, size, bold, italic, underline, strikethrough,
//! colour, highlight as run shading, super/subscript) and hyperlinks; bulleted and numbered
//! lists (one Word list per run of numbered paragraphs; `restart` starts a new one at its
//! number) with their levels; the indent of other paragraphs, alignment, right-to-left, space
//! before/after and "at least" line spacing; to-do tags as ☐/☒ before the text; tables with
//! their column widths, borders, shading and merged cells; pictures inline at their page size
//! (scaled down to the text width; WebP and BMP converted to PNG). **What does not:** attached
//! files (a line with the file name stands in; counted in [`DocxReport::files_dropped`]),
//! other note tags, ink, the free positions of objects.
//!
//! Element order inside `w:pPr`, `w:rPr`, `w:tblPr` and `w:tcPr` follows the schema's
//! sequences (Word refuses files that don't).

use std::collections::HashMap;
use std::io::{Cursor, Write};

use notecraft_codecs::Format;
use notecraft_doc::color::Rgba;
use notecraft_doc::text::MAX_LEVEL;
use notecraft_doc::{Align, CharFormat, ListFormat, Numbering, ParaStyle, Paragraph, Script, StyleDef};

use crate::export::{Export, Item};
use crate::html::TableBlock;

/// What the Word document could not hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DocxReport {
    /// Attached files written as their name only.
    pub files_dropped: usize,
    /// Pictures left out (unreadable for conversion).
    pub pictures_dropped: usize,
}

/// Text width of a US Letter page with 1-inch margins, points: wider pictures are scaled.
const TEXT_WIDTH_PT: f32 = 468.0;
/// English Metric Units per point.
const EMU_PER_PT: f32 = 12_700.0;

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// `s` escaped for XML text and attributes; characters XML 1.0 forbids are dropped.
fn xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

fn hex(c: Rgba) -> String {
    format!("{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

/// Points to twentieths of a point, clamped to what Word accepts.
fn twips(pt: f32) -> i64 {
    if pt.is_finite() { (pt * 20.0).round().clamp(-31_680.0, 31_680.0) as i64 } else { 0 }
}

/// `w:rPr` children for a character format, in schema order.
fn rpr(f: &CharFormat, out: &mut String) {
    let mut s = String::new();
    if f.link.is_some() {
        s.push_str("<w:rStyle w:val=\"Hyperlink\"/>");
    }
    if let Some(font) = &f.font {
        let font = xml(font);
        s.push_str(&format!("<w:rFonts w:ascii=\"{font}\" w:hAnsi=\"{font}\" w:cs=\"{font}\"/>"));
    }
    let toggle = |s: &mut String, tag: &str, v: Option<bool>| match v {
        Some(true) => s.push_str(&format!("<w:{tag}/>")),
        Some(false) => s.push_str(&format!("<w:{tag} w:val=\"0\"/>")),
        None => {}
    };
    toggle(&mut s, "b", f.bold);
    toggle(&mut s, "i", f.italic);
    toggle(&mut s, "strike", f.strike);
    if let Some(c) = f.color {
        s.push_str(&format!("<w:color w:val=\"{}\"/>", hex(c)));
    }
    if let Some(sz) = f.size.filter(|v| v.is_finite() && *v > 0.0) {
        let half = (sz * 2.0).round().clamp(2.0, 3276.0) as u32;
        s.push_str(&format!("<w:sz w:val=\"{half}\"/><w:szCs w:val=\"{half}\"/>"));
    }
    match f.underline {
        Some(true) => s.push_str("<w:u w:val=\"single\"/>"),
        Some(false) => s.push_str("<w:u w:val=\"none\"/>"),
        None => {}
    }
    if let Some(c) = f.highlight {
        s.push_str(&format!("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>", hex(c)));
    }
    match f.script {
        Some(Script::Super) => s.push_str("<w:vertAlign w:val=\"superscript\"/>"),
        Some(Script::Sub) => s.push_str("<w:vertAlign w:val=\"subscript\"/>"),
        _ => {}
    }
    if !s.is_empty() {
        out.push_str("<w:rPr>");
        out.push_str(&s);
        out.push_str("</w:rPr>");
    }
}

/// A run's text: tabs and soft line breaks become their elements.
fn run_text(text: &str, out: &mut String) {
    let mut buf = String::new();
    let flush = |buf: &mut String, out: &mut String| {
        if !buf.is_empty() {
            out.push_str(&format!("<w:t xml:space=\"preserve\">{}</w:t>", xml(buf)));
            buf.clear();
        }
    };
    for c in text.chars() {
        match c {
            '\n' => {
                flush(&mut buf, out);
                out.push_str("<w:br/>");
            }
            '\t' => {
                flush(&mut buf, out);
                out.push_str("<w:tab/>");
            }
            c => buf.push(c),
        }
    }
    flush(&mut buf, out);
}

/// The Word style id of a paragraph style (`None`: Normal).
fn style_id(s: ParaStyle) -> Option<&'static str> {
    Some(match s {
        ParaStyle::Heading1 => "Heading1",
        ParaStyle::Heading2 => "Heading2",
        ParaStyle::Heading3 => "Heading3",
        ParaStyle::Heading4 => "Heading4",
        ParaStyle::Heading5 => "Heading5",
        ParaStyle::Heading6 => "Heading6",
        ParaStyle::PageTitle => "Title",
        ParaStyle::Citation => "Citation",
        ParaStyle::Quote => "Quote",
        ParaStyle::Code => "Code",
        ParaStyle::Normal => return None,
    })
}

/// Word list formats: (`w:numFmt`, `w:lvlText` with `%N` for the level number).
fn num_format(n: Numbering) -> (&'static str, &'static str) {
    match n {
        Numbering::Decimal => ("decimal", "."),
        Numbering::DecimalParen => ("decimal", ")"),
        Numbering::LowerAlpha => ("lowerLetter", "."),
        Numbering::UpperAlpha => ("upperLetter", "."),
        Numbering::LowerRoman => ("lowerRoman", "."),
        Numbering::UpperRoman => ("upperRoman", "."),
        Numbering::LowerAlphaParen => ("lowerLetter", ")"),
    }
}

const NUMBERINGS: [Numbering; 7] = [
    Numbering::Decimal,
    Numbering::DecimalParen,
    Numbering::LowerAlpha,
    Numbering::UpperAlpha,
    Numbering::LowerRoman,
    Numbering::UpperRoman,
    Numbering::LowerAlphaParen,
];

/// Word list levels used (0–8).
const LEVELS: u8 = 9;

/// The lists of the document: one abstract list per bullet and per number format; one list
/// instance per bullet, and one per run of numbered paragraphs.
#[derive(Default)]
struct Lists {
    /// Numbered instances: (abstract id, start).
    numbered: Vec<(usize, u32)>,
}

impl Lists {
    /// Abstract ids: bullets 0..8, numberings 8..15.
    fn bullet_num_id(b: notecraft_doc::Bullet) -> usize {
        notecraft_doc::Bullet::ALL.iter().position(|x| *x == b).unwrap_or(0) + 1
    }

    fn numbering_abstract(n: Numbering) -> usize {
        notecraft_doc::Bullet::ALL.len() + NUMBERINGS.iter().position(|x| *x == n).unwrap_or(0)
    }

    /// A new numbered list instance; returns its `w:numId`.
    fn new_numbered(&mut self, n: Numbering, start: u32) -> usize {
        self.numbered.push((Lists::numbering_abstract(n), start.max(1)));
        100 + self.numbered.len()
    }

    fn xml(&self) -> String {
        let mut s = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:numbering xmlns:w=\"{NS_W}\">");
        let lvl = |s: &mut String, i: u8, fmt: &str, text: &str, font: Option<&str>| {
            let left = 360 * (u32::from(i) + 1);
            s.push_str(&format!(
                "<w:lvl w:ilvl=\"{i}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{fmt}\"/><w:lvlText w:val=\"{}\"/><w:lvlJc w:val=\"left\"/>\
                 <w:pPr><w:ind w:left=\"{left}\" w:hanging=\"360\"/></w:pPr>",
                xml(text)
            ));
            if let Some(f) = font {
                s.push_str(&format!("<w:rPr><w:rFonts w:ascii=\"{f}\" w:hAnsi=\"{f}\"/></w:rPr>"));
            }
            s.push_str("</w:lvl>");
        };
        for (a, b) in notecraft_doc::Bullet::ALL.iter().enumerate() {
            s.push_str(&format!("<w:abstractNum w:abstractNumId=\"{a}\"><w:multiLevelType w:val=\"hybridMultilevel\"/>"));
            for i in 0..LEVELS {
                lvl(&mut s, i, "bullet", b.glyph(), Some("Segoe UI Symbol"));
            }
            s.push_str("</w:abstractNum>");
        }
        for n in NUMBERINGS {
            let a = Lists::numbering_abstract(n);
            let (fmt, delim) = num_format(n);
            s.push_str(&format!("<w:abstractNum w:abstractNumId=\"{a}\"><w:multiLevelType w:val=\"hybridMultilevel\"/>"));
            for i in 0..LEVELS {
                lvl(&mut s, i, fmt, &format!("%{}{delim}", i + 1), None);
            }
            s.push_str("</w:abstractNum>");
        }
        for (i, _) in notecraft_doc::Bullet::ALL.iter().enumerate() {
            s.push_str(&format!("<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"{i}\"/></w:num>", i + 1));
        }
        for (k, (a, start)) in self.numbered.iter().enumerate() {
            s.push_str(&format!(
                "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"{a}\"/><w:lvlOverride w:ilvl=\"0\"><w:startOverride w:val=\"{start}\"/></w:lvlOverride></w:num>",
                101 + k
            ));
        }
        s.push_str("</w:numbering>");
        s
    }
}

/// A picture placed in the package.
struct Media {
    /// Path in the package (`media/image3.png`, relative to `word/`).
    path: String,
    bytes: Vec<u8>,
}

/// Writes `word/document.xml` and collects what it refers to.
struct Writer<'a> {
    e: &'a Export,
    body: String,
    /// Relationships of `document.xml` after styles and numbering: (id, type, target, external).
    rels: Vec<(String, String, String, bool)>,
    links: HashMap<String, String>,
    media: Vec<Media>,
    media_of: HashMap<usize, Option<(String, String)>>,
    lists: Lists,
    report: DocxReport,
    drawing_id: u32,
}

impl Writer<'_> {
    /// A relationship id for `target` of `kind` (external for hyperlinks).
    fn rel(&mut self, kind: &str, target: String, external: bool) -> String {
        let id = format!("rId{}", self.rels.len() + 3);
        self.rels.push((id.clone(), format!("{REL}/{kind}"), target, external));
        id
    }

    fn link_rel(&mut self, url: &str) -> String {
        if let Some(id) = self.links.get(url) {
            return id.clone();
        }
        let id = self.rel("hyperlink", url.to_string(), true);
        self.links.insert(url.to_string(), id.clone());
        id
    }

    /// The relationship id and package path of picture resource `res`, converting formats
    /// Word may not read; `None` when it can't be read.
    fn picture(&mut self, res: usize) -> Option<(String, String)> {
        if let Some(m) = self.media_of.get(&res) {
            return m.clone();
        }
        let r = self.e.resources.get(res)?;
        let n = self.media.len() + 1;
        let out = match notecraft_codecs::sniff(&r.bytes) {
            Some(f @ (Format::Png | Format::Jpeg | Format::Gif)) => Some((f.extension(), r.bytes.to_vec())),
            _ => notecraft_codecs::decode(&r.bytes, &notecraft_codecs::Limits::PICTURE)
                .and_then(|img| notecraft_codecs::encode_png(&img, &notecraft_codecs::Limits::PICTURE))
                .ok()
                .map(|png| ("png", png)),
        };
        let got = out.map(|(ext, bytes)| {
            let path = format!("media/image{n}.{ext}");
            self.media.push(Media { path: path.clone(), bytes });
            let id = self.rel("image", path.clone(), false);
            (id, path)
        });
        self.media_of.insert(res, got.clone());
        got
    }

    fn para(&mut self, p: &Paragraph, num: Option<(usize, u8)>) {
        let mut ppr = String::new();
        if let Some(id) = style_id(p.style) {
            ppr.push_str(&format!("<w:pStyle w:val=\"{id}\"/>"));
        }
        if let Some((id, lvl)) = num {
            ppr.push_str(&format!("<w:numPr><w:ilvl w:val=\"{lvl}\"/><w:numId w:val=\"{id}\"/></w:numPr>"));
        }
        if p.rtl {
            ppr.push_str("<w:bidi/>");
        }
        let (before, after, line) = (p.space_before, p.space_after, p.line_spacing);
        if before > 0.0 || after > 0.0 || line > 0.0 {
            ppr.push_str("<w:spacing");
            if before > 0.0 {
                ppr.push_str(&format!(" w:before=\"{}\"", twips(before)));
            }
            if after > 0.0 {
                ppr.push_str(&format!(" w:after=\"{}\"", twips(after)));
            }
            if line > 0.0 {
                ppr.push_str(&format!(" w:line=\"{}\" w:lineRule=\"atLeast\"", twips(line)));
            }
            ppr.push_str("/>");
        }
        if num.is_none() && p.level > 0 {
            ppr.push_str(&format!("<w:ind w:left=\"{}\"/>", twips(27.0 * f32::from(p.level))));
        }
        match p.align {
            Align::Center => ppr.push_str("<w:jc w:val=\"center\"/>"),
            Align::Right => ppr.push_str("<w:jc w:val=\"right\"/>"),
            Align::Left => {}
        }
        self.body.push_str("<w:p>");
        if !ppr.is_empty() {
            self.body.push_str(&format!("<w:pPr>{ppr}</w:pPr>"));
        }
        if let Some(t) = p.tags.iter().find(|t| t.symbol.is_checkbox()) {
            self.body.push_str("<w:r>");
            run_text(if t.checked { "☒ " } else { "☐ " }, &mut self.body);
            self.body.push_str("</w:r>");
        }
        for r in &p.runs {
            let link = r.fmt.link.as_deref().filter(|l| !l.trim().is_empty()).map(|l| self.link_rel(l));
            if let Some(id) = &link {
                self.body.push_str(&format!("<w:hyperlink r:id=\"{id}\" w:history=\"1\">"));
            }
            self.body.push_str("<w:r>");
            rpr(&r.fmt, &mut self.body);
            run_text(&r.text, &mut self.body);
            self.body.push_str("</w:r>");
            if link.is_some() {
                self.body.push_str("</w:hyperlink>");
            }
        }
        self.body.push_str("</w:p>");
    }

    /// A container's paragraphs, with its lists.
    fn paragraphs(&mut self, ps: &[Paragraph]) {
        // The numbered list being continued: (format, numId).
        let mut current: Option<(Numbering, usize)> = None;
        for p in ps {
            let lvl = p.level.min(MAX_LEVEL).min(LEVELS - 1);
            let num = match p.list {
                Some(ListFormat::Bullet(b)) => Some((Lists::bullet_num_id(b), lvl)),
                Some(ListFormat::Number { style, restart }) => {
                    let id = match current {
                        Some((s, id)) if s == style && restart.is_none() => id,
                        _ => self.lists.new_numbered(style, restart.unwrap_or(1)),
                    };
                    current = Some((style, id));
                    Some((id, lvl))
                }
                None => {
                    current = None;
                    None
                }
            };
            self.para(p, num);
        }
    }

    fn table(&mut self, t: &TableBlock, widths: &[f32], borders: bool, shading: &[Vec<Option<Rgba>>]) {
        let cols = t.cells.iter().map(Vec::len).max().unwrap_or(0).max(widths.len()).max(1);
        let width = |c: usize| widths.get(c).copied().filter(|w| w.is_finite() && *w > 0.0).unwrap_or(72.0);
        self.body.push_str("<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/>");
        let line = if borders { "w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"A3A3A3\"" } else { "w:val=\"nil\"" };
        self.body.push_str("<w:tblBorders>");
        for side in ["top", "left", "bottom", "right", "insideH", "insideV"] {
            self.body.push_str(&format!("<w:{side} {line}/>"));
        }
        self.body.push_str("</w:tblBorders><w:tblLayout w:type=\"fixed\"/></w:tblPr><w:tblGrid>");
        for c in 0..cols {
            self.body.push_str(&format!("<w:gridCol w:w=\"{}\"/>", twips(width(c))));
        }
        self.body.push_str("</w:tblGrid>");
        let anchors: HashMap<(usize, usize), crate::html::Span> = t.spans.iter().map(|s| ((s.row, s.col), *s)).collect();
        // The span that covers a place below its first row.
        let below = |r: usize, c: usize| t.spans.iter().find(|s| s.col == c && s.row < r && r < s.row + s.rows).copied();
        for (r, row) in t.cells.iter().enumerate() {
            self.body.push_str("<w:tr>");
            let mut c = 0;
            while c < cols {
                let (span, cont) = match (anchors.get(&(r, c)), below(r, c)) {
                    (Some(s), _) => (s.cols.clamp(1, cols - c), (s.rows > 1).then_some("restart")),
                    (None, Some(s)) => (s.cols.clamp(1, cols - c), Some("continue")),
                    (None, None) => (1, None),
                };
                let w: f32 = (c..c + span).map(width).sum();
                self.body.push_str(&format!("<w:tc><w:tcPr><w:tcW w:w=\"{}\" w:type=\"dxa\"/>", twips(w)));
                if span > 1 {
                    self.body.push_str(&format!("<w:gridSpan w:val=\"{span}\"/>"));
                }
                match cont {
                    Some("restart") => self.body.push_str("<w:vMerge w:val=\"restart\"/>"),
                    Some(_) => self.body.push_str("<w:vMerge/>"),
                    None => {}
                }
                if let Some(fill) = shading.get(r).and_then(|row| row.get(c)).copied().flatten() {
                    self.body.push_str(&format!("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>", hex(fill)));
                }
                self.body.push_str("</w:tcPr>");
                let paras = if cont == Some("continue") { None } else { row.get(c).filter(|ps| !ps.is_empty()) };
                match paras {
                    Some(ps) => self.paragraphs(ps),
                    // A cell holds at least one paragraph.
                    None => self.body.push_str("<w:p/>"),
                }
                self.body.push_str("</w:tc>");
                c += span;
            }
            self.body.push_str("</w:tr>");
        }
        self.body.push_str("</w:tbl>");
        // Two tables in a row would merge in Word: a paragraph keeps them apart.
        self.body.push_str("<w:p/>");
    }

    fn picture_para(&mut self, res: usize, alt: &str, width: f32, height: f32) {
        let Some((id, path)) = self.picture(res) else {
            self.report.pictures_dropped += 1;
            return;
        };
        let (mut w, mut h) = (if width.is_finite() { width.max(1.0) } else { 72.0 }, if height.is_finite() { height.max(1.0) } else { 72.0 });
        if w > TEXT_WIDTH_PT {
            h *= TEXT_WIDTH_PT / w;
            w = TEXT_WIDTH_PT;
        }
        let (cx, cy) = ((w * EMU_PER_PT).round() as u64, (h * EMU_PER_PT).round() as u64);
        self.drawing_id += 1;
        let n = self.drawing_id;
        let name = xml(path.trim_start_matches("media/"));
        let alt = xml(alt);
        self.body.push_str(&format!(
            "<w:p><w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/>\
             <wp:docPr id=\"{n}\" name=\"Picture {n}\" descr=\"{alt}\"/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
             <pic:pic><pic:nvPicPr><pic:cNvPr id=\"{n}\" name=\"{name}\" descr=\"{alt}\"/><pic:cNvPicPr/></pic:nvPicPr>\
             <pic:blipFill><a:blip r:embed=\"{id}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>\
             <pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr>\
             </pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"
        ));
    }

    fn document(&mut self) {
        for (i, page) in self.e.pages.iter().enumerate() {
            if i > 0 {
                self.body.push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>");
            }
            self.body.push_str("<w:p><w:pPr><w:pStyle w:val=\"Title\"/></w:pPr><w:r>");
            run_text(&page.title, &mut self.body);
            self.body.push_str("</w:r></w:p><w:p><w:r><w:rPr><w:color w:val=\"767676\"/><w:sz w:val=\"20\"/></w:rPr>");
            run_text(&page.date, &mut self.body);
            self.body.push_str("</w:r></w:p>");
            for it in &page.items {
                match it {
                    Item::Paragraphs(ps) => self.paragraphs(ps),
                    Item::Table { cells, widths, borders, shading } => self.table(cells, widths, *borders, shading),
                    Item::Picture { res, alt, width, height } => self.picture_para(*res, alt, *width, *height),
                    Item::File { name, .. } => {
                        self.report.files_dropped += 1;
                        self.body.push_str("<w:p><w:r><w:rPr><w:i/></w:rPr>");
                        run_text(&format!("Attached file: {name}"), &mut self.body);
                        self.body.push_str("</w:r></w:p>");
                    }
                }
            }
        }
    }
}

/// `w:style` for a NoteCraft paragraph style.
fn style_xml(id: &str, name: &str, d: &StyleDef, outline: Option<u8>, default: bool) -> String {
    let mut s =
        format!("<w:style w:type=\"paragraph\"{} w:styleId=\"{id}\"><w:name w:val=\"{name}\"/>", if default { " w:default=\"1\"" } else { "" });
    if !default {
        s.push_str("<w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/>");
    }
    s.push_str("<w:qFormat/><w:pPr>");
    s.push_str(&format!("<w:spacing w:before=\"{}\" w:after=\"{}\"/>", twips(d.space_before), twips(d.space_after)));
    if let Some(l) = outline {
        s.push_str(&format!("<w:outlineLvl w:val=\"{l}\"/>"));
    }
    s.push_str("</w:pPr>");
    let mut r = String::new();
    // Underline and script "off" are the defaults: leave them out of the style.
    let f = CharFormat { underline: None, script: None, ..d.char.clone() };
    rpr(&f, &mut r);
    s.push_str(&r);
    s.push_str("</w:style>");
    s
}

fn styles_xml() -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:styles xmlns:w=\"{NS_W}\">\
         <w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\" w:cs=\"Calibri\"/><w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/>\
         <w:lang w:val=\"en-US\"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault></w:docDefaults>"
    );
    s.push_str(&style_xml("Normal", "Normal", &ParaStyle::Normal.def(), None, true));
    for (i, st) in [ParaStyle::Heading1, ParaStyle::Heading2, ParaStyle::Heading3, ParaStyle::Heading4, ParaStyle::Heading5, ParaStyle::Heading6]
        .into_iter()
        .enumerate()
    {
        s.push_str(&style_xml(&format!("Heading{}", i + 1), &format!("heading {}", i + 1), &st.def(), Some(i as u8), false));
    }
    s.push_str(&style_xml("Title", "Title", &ParaStyle::PageTitle.def(), None, false));
    s.push_str(&style_xml("Citation", "Citation", &ParaStyle::Citation.def(), None, false));
    s.push_str(&style_xml("Quote", "Quote", &ParaStyle::Quote.def(), None, false));
    s.push_str(&style_xml("Code", "Code", &ParaStyle::Code.def(), None, false));
    s.push_str(
        "<w:style w:type=\"character\" w:styleId=\"Hyperlink\"><w:name w:val=\"Hyperlink\"/><w:rPr><w:color w:val=\"0563C1\"/><w:u w:val=\"single\"/></w:rPr></w:style>",
    );
    s.push_str("</w:styles>");
    s
}

/// The export as a `.docx` package titled `title`.
pub fn write(e: &Export, title: &str) -> Result<(Vec<u8>, DocxReport), String> {
    let mut w = Writer {
        e,
        body: String::new(),
        rels: Vec::new(),
        links: HashMap::new(),
        media: Vec::new(),
        media_of: HashMap::new(),
        lists: Lists::default(),
        report: DocxReport::default(),
        drawing_id: 0,
    };
    w.document();
    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\" \
         xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
         xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><w:body>{}\
         <w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\" \
         w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>",
        w.body
    );
    let mut doc_rels = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    doc_rels.push_str(&format!("<Relationship Id=\"rId1\" Type=\"{REL}/styles\" Target=\"styles.xml\"/>"));
    doc_rels.push_str(&format!("<Relationship Id=\"rId2\" Type=\"{REL}/numbering\" Target=\"numbering.xml\"/>"));
    for (id, kind, target, external) in &w.rels {
        doc_rels.push_str(&format!(
            "<Relationship Id=\"{id}\" Type=\"{kind}\" Target=\"{}\"{}/>",
            xml(target),
            if *external { " TargetMode=\"External\"" } else { "" }
        ));
    }
    doc_rels.push_str("</Relationships>");
    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
        <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
        <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
        <Default Extension=\"png\" ContentType=\"image/png\"/><Default Extension=\"jpg\" ContentType=\"image/jpeg\"/>\
        <Default Extension=\"gif\" ContentType=\"image/gif\"/>\
        <Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
        <Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\
        <Override PartName=\"/word/numbering.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml\"/>\
        <Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/></Types>";
    let root_rels = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rId1\" Type=\"{REL}/officeDocument\" Target=\"word/document.xml\"/>\
         <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>\
         </Relationships>"
    );
    let core = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<cp:coreProperties \
         xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
         xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><dc:title>{}</dc:title></cp:coreProperties>",
        xml(title)
    );
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let err = |e: &dyn std::fmt::Display| format!("writing the Word document: {e}");
    let (styles, numbering) = (styles_xml(), w.lists.xml());
    let parts: [(&str, &[u8]); 7] = [
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", root_rels.as_bytes()),
        ("docProps/core.xml", core.as_bytes()),
        ("word/document.xml", document.as_bytes()),
        ("word/styles.xml", styles.as_bytes()),
        ("word/numbering.xml", numbering.as_bytes()),
        ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
    ];
    for (name, bytes) in parts {
        z.start_file(name, deflate).map_err(|e| err(&e))?;
        z.write_all(bytes).map_err(|e| err(&e))?;
    }
    for m in &w.media {
        z.start_file(format!("word/{}", m.path), stored).map_err(|e| err(&e))?;
        z.write_all(&m.bytes).map_err(|e| err(&e))?;
    }
    let out = z.finish().map_err(|e| err(&e))?.into_inner();
    Ok((out, w.report))
}

#[cfg(test)]
#[path = "docx/tests.rs"]
mod tests;
