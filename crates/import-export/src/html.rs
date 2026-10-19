//! HTML for the rich clipboard: [`parse`] turns a fragment from another application into
//! paragraphs and tables, [`write`] turns copied notes into a fragment other applications
//! read.
//!
//! **The reader is small and tolerant, not a browser.** It keeps what a note can hold —
//! paragraphs, headings, soft line breaks, bold/italic/underline/strikethrough, sub- and
//! superscript, links, text colour, bulleted and numbered lists (nesting becomes indent
//! levels) and tables (one level; a table inside a table becomes text) — and drops the rest
//! (scripts, styles, images, forms). Unclosed and stray tags are forgiven. **Bounds against
//! hostile input:** at most [`MAX_HTML_BYTES`] are read, [`MAX_DEPTH`] open elements are
//! tracked (deeper tags are ignored, their text kept), at most [`MAX_PARAGRAPHS`] paragraphs
//! and the table limits of `notecraft_doc::table` are produced; everything else is skipped.
//!
//! Paragraph ids are [`Id::NIL`]: the caller assigns fresh ones.

use notecraft_doc::color::Rgba;
use notecraft_doc::table::{MAX_CELLS, MAX_COLS, MAX_ROWS};
use notecraft_doc::text::MAX_LEVEL;
use notecraft_doc::{Bullet, CharFormat, Id, ListFormat, Numbering, ParaStyle, Paragraph, Run, Script};

/// Most HTML bytes read; the rest of a larger fragment is ignored.
pub const MAX_HTML_BYTES: usize = 16 << 20;
/// Most open elements tracked at once.
pub const MAX_DEPTH: usize = 256;
/// Most paragraphs produced.
pub const MAX_PARAGRAPHS: usize = 50_000;

/// A table as rows of cells, each cell a list of paragraphs.
pub type Cells = Vec<Vec<Vec<Paragraph>>>;

/// What a fragment holds, in reading order.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// A paragraph.
    Para(Paragraph),
    /// A table.
    Table(Cells),
}

#[derive(Clone, Copy, PartialEq)]
enum Fmt {
    Bold,
    Italic,
    Underline,
    Strike,
    Sub,
    Super,
}

/// One open element and what it changed.
struct Open {
    tag: String,
    fmt: Vec<Fmt>,
    link: bool,
    color: bool,
}

#[derive(Default)]
struct TableState {
    rows: Cells,
    row: Vec<Vec<Paragraph>>,
    cell: Vec<Paragraph>,
    cell_open: bool,
}

struct Reader {
    out: Vec<Block>,
    runs: Vec<Run>,
    style: ParaStyle,
    list: Option<(ListFormat, u8)>,
    lists: Vec<bool>,
    stack: Vec<Open>,
    fmt: Vec<Fmt>,
    links: Vec<String>,
    colors: Vec<Rgba>,
    skip: Option<String>,
    pre: usize,
    /// The table being read.
    table: Option<TableState>,
    table_depth: usize,
    paras: usize,
}

fn decode_entity(name: &str) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let v = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(v).filter(|c| *c != '\0');
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "hellip" => '…',
        "mdash" => '—',
        "ndash" => '–',
        "lsquo" => '‘',
        "rsquo" => '’',
        "ldquo" => '“',
        "rdquo" => '”',
        "bull" => '•',
        "middot" => '·',
        "euro" => '€',
        "deg" => '°',
        "times" => '×',
        _ => return None,
    })
}

/// Decode character references (unknown ones stay as written).
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(rest.get(..i).unwrap_or(""));
        let tail = rest.get(i + 1..).unwrap_or("");
        match tail.find(';').filter(|j| *j <= 10).and_then(|j| decode_entity(tail.get(..j)?).map(|c| (c, j))) {
            Some((c, j)) => {
                out.push(c);
                rest = tail.get(j + 1..).unwrap_or("");
            }
            None => {
                out.push('&');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The value of attribute `name` in a tag's attribute text.
fn attr(attrs: &str, name: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower.get(from..)?.find(name) {
        let at = from + i;
        from = at + name.len();
        let before_ok = at == 0 || lower.as_bytes().get(at - 1).is_some_and(|b| b.is_ascii_whitespace());
        let rest = attrs.get(from..)?.trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let v = rest.get(1..)?.trim_start();
        let (q, body) = match v.chars().next()? {
            q @ ('"' | '\'') => (Some(q), v.get(1..)?),
            _ => (None, v),
        };
        let end = match q {
            Some(q) => body.find(q).unwrap_or(body.len()),
            None => body.find(|c: char| c.is_ascii_whitespace() || c == '>').unwrap_or(body.len()),
        };
        return Some(unescape(body.get(..end)?));
    }
    None
}

/// What a `style` attribute turns on: formats and a colour.
fn style_fmt(style: &str) -> (Vec<Fmt>, Option<Rgba>) {
    let mut fmt = Vec::new();
    let mut color = None;
    for decl in style.split(';') {
        let Some((k, v)) = decl.split_once(':') else { continue };
        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_ascii_lowercase());
        match k.as_str() {
            "font-weight" if v == "bold" || v == "bolder" || v.parse::<u32>().is_ok_and(|w| w >= 600) => fmt.push(Fmt::Bold),
            "font-style" if v.starts_with("italic") || v.starts_with("oblique") => fmt.push(Fmt::Italic),
            "text-decoration" | "text-decoration-line" => {
                if v.contains("underline") {
                    fmt.push(Fmt::Underline);
                }
                if v.contains("line-through") {
                    fmt.push(Fmt::Strike);
                }
            }
            "vertical-align" if v == "super" => fmt.push(Fmt::Super),
            "vertical-align" if v == "sub" => fmt.push(Fmt::Sub),
            "color" => color = parse_color(&v),
            _ => {}
        }
    }
    (fmt, color)
}

fn parse_color(v: &str) -> Option<Rgba> {
    if v.starts_with('#') {
        return Rgba::from_hex(v);
    }
    let inner = v.strip_prefix("rgb(").or_else(|| v.strip_prefix("rgba("))?.trim_end_matches(')');
    let mut it = inner.split(',').map(|x| x.trim().parse::<f32>().ok().map(|f| f.clamp(0.0, 255.0) as u8));
    Some(Rgba::rgb(it.next()??, it.next()??, it.next()??))
}

fn heading(tag: &str) -> Option<ParaStyle> {
    Some(match tag {
        "h1" => ParaStyle::Heading1,
        "h2" => ParaStyle::Heading2,
        "h3" => ParaStyle::Heading3,
        "h4" => ParaStyle::Heading4,
        "h5" => ParaStyle::Heading5,
        "h6" => ParaStyle::Heading6,
        _ => return None,
    })
}

const BLOCKS: &[&str] = &[
    "p",
    "div",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "li",
    "blockquote",
    "pre",
    "section",
    "article",
    "header",
    "footer",
    "dd",
    "dt",
    "figure",
    "address",
];
const SKIP: &[&str] = &["script", "style", "head", "title", "template", "noscript", "svg", "math", "object", "iframe", "select"];

impl Reader {
    fn new() -> Reader {
        Reader {
            out: Vec::new(),
            runs: Vec::new(),
            style: ParaStyle::Normal,
            list: None,
            lists: Vec::new(),
            stack: Vec::new(),
            fmt: Vec::new(),
            links: Vec::new(),
            colors: Vec::new(),
            skip: None,
            pre: 0,
            table: None,
            table_depth: 0,
            paras: 0,
        }
    }

    fn char_fmt(&self) -> CharFormat {
        let has = |f: Fmt| self.fmt.contains(&f).then_some(true);
        CharFormat {
            bold: has(Fmt::Bold),
            italic: has(Fmt::Italic),
            underline: has(Fmt::Underline),
            strike: has(Fmt::Strike),
            script: if self.fmt.contains(&Fmt::Super) {
                Some(Script::Super)
            } else if self.fmt.contains(&Fmt::Sub) {
                Some(Script::Sub)
            } else {
                None
            },
            link: self.links.last().cloned(),
            color: self.colors.last().copied(),
            ..CharFormat::default()
        }
    }

    fn text(&mut self, raw: &str) {
        if self.skip.is_some() || self.paras >= MAX_PARAGRAPHS {
            return;
        }
        let t = unescape(raw);
        let t = if self.pre > 0 {
            t
        } else {
            // HTML whitespace collapsing; a paragraph never starts with a space.
            let mut s = String::with_capacity(t.len());
            let mut space = self.runs.last().is_none_or(|r| r.text.ends_with([' ', '\n']));
            for c in t.chars() {
                if c.is_whitespace() {
                    if !space {
                        s.push(' ');
                        space = true;
                    }
                } else {
                    s.push(c);
                    space = false;
                }
            }
            s
        };
        if t.is_empty() {
            return;
        }
        let fmt = self.char_fmt();
        // In <pre>, a line break is a soft break inside the paragraph.
        self.runs.push(Run::new(&t, fmt));
    }

    /// End the current paragraph (if it has text, or is a list item) into the table cell or the
    /// output.
    fn flush(&mut self) {
        let has_text = self.runs.iter().any(|r| !r.text.trim().is_empty());
        if !has_text {
            self.runs.clear();
            return;
        }
        if self.paras >= MAX_PARAGRAPHS {
            self.runs.clear();
            return;
        }
        let mut p = Paragraph::new(Id::NIL);
        if let Some(last) = self.runs.last_mut() {
            let trimmed = last.text.trim_end_matches(' ').to_string();
            last.text = trimmed;
        }
        p.runs = std::mem::take(&mut self.runs);
        p.style = self.style;
        if let Some((list, level)) = self.list {
            p.list = Some(list);
            p.level = level;
        }
        p.normalize();
        self.paras += 1;
        match &mut self.table {
            Some(t) => t.cell.push(p),
            None => self.out.push(Block::Para(p)),
        }
    }

    fn open(&mut self, tag: &str, attrs: &str) {
        if self.skip.is_some() {
            return;
        }
        if SKIP.contains(&tag) {
            self.skip = Some(tag.to_string());
            return;
        }
        match tag {
            "br" => {
                let fmt = self.char_fmt();
                self.runs.push(Run::new("\n", fmt));
                return;
            }
            "hr" => {
                self.flush();
                return;
            }
            "img" | "input" | "meta" | "link" | "wbr" | "col" | "area" | "source" => return,
            _ => {}
        }
        if self.stack.len() >= MAX_DEPTH {
            return;
        }
        if BLOCKS.contains(&tag) {
            // A <p> inside an <li> continues the item (as office apps write lists).
            let in_li_start = tag == "p" && self.stack.last().is_some_and(|o| o.tag == "li") && self.runs.is_empty();
            if !in_li_start {
                self.flush();
            }
            if let Some(h) = heading(tag) {
                self.style = h;
            }
            if tag == "pre" {
                self.pre += 1;
                self.style = ParaStyle::Code;
            }
            if tag == "blockquote" {
                self.style = ParaStyle::Quote;
            }
            if tag == "li" {
                let level = self.lists.len().saturating_sub(1).min(MAX_LEVEL as usize) as u8;
                let list = match self.lists.last() {
                    Some(true) => ListFormat::Number { style: Numbering::Decimal, restart: None },
                    _ => ListFormat::Bullet(Bullet::default()),
                };
                self.list = Some((list, level));
            }
        }
        match tag {
            "ul" | "ol" => {
                self.flush();
                self.lists.push(tag == "ol");
            }
            "table" => {
                self.flush();
                self.table_depth += 1;
                if self.table_depth == 1 {
                    self.table = Some(TableState::default());
                }
            }
            "tr" if self.table_depth == 1 => self.end_row(),
            // A table inside a table is text: its cells separated by spaces.
            "tr" | "td" | "th" if self.table_depth > 1 => self.text(" "),
            "td" | "th" if self.table_depth == 1 => {
                if self.table.as_ref().is_some_and(|t| t.cell_open) {
                    self.end_cell();
                }
                if let Some(t) = &mut self.table {
                    t.cell_open = true;
                }
                if tag == "th" {
                    self.fmt.push(Fmt::Bold);
                }
            }
            _ => {}
        }
        let mut fmt = match tag {
            "b" | "strong" => vec![Fmt::Bold],
            "i" | "em" | "cite" | "dfn" => vec![Fmt::Italic],
            "u" | "ins" => vec![Fmt::Underline],
            "s" | "strike" | "del" => vec![Fmt::Strike],
            "sub" => vec![Fmt::Sub],
            "sup" => vec![Fmt::Super],
            _ => Vec::new(),
        };
        let mut color = None;
        if let Some(st) = attr(attrs, "style") {
            let (f, c) = style_fmt(&st);
            fmt.extend(f);
            color = c;
        }
        let link = if tag == "a" { attr(attrs, "href").filter(|h| !h.trim().is_empty() && !h.trim_start().starts_with("javascript:")) } else { None };
        self.fmt.extend(fmt.iter().copied());
        if let Some(c) = color {
            self.colors.push(c);
        }
        if let Some(l) = &link {
            self.links.push(l.trim().to_string());
        }
        self.stack.push(Open { tag: tag.to_string(), fmt, link: link.is_some(), color: color.is_some() });
    }

    fn close(&mut self, tag: &str) {
        if let Some(sk) = &self.skip {
            if sk == tag {
                self.skip = None;
            }
            return;
        }
        // Close the most recent matching element (and everything opened inside it).
        let Some(i) = self.stack.iter().rposition(|o| o.tag == tag) else { return };
        while self.stack.len() > i {
            let Some(o) = self.stack.pop() else { break };
            for f in &o.fmt {
                if let Some(k) = self.fmt.iter().rposition(|x| x == f) {
                    self.fmt.remove(k);
                }
            }
            if o.link {
                self.links.pop();
            }
            if o.color {
                self.colors.pop();
            }
            self.closed(&o.tag);
        }
    }

    /// Block-level effects of an element ending.
    fn closed(&mut self, tag: &str) {
        if BLOCKS.contains(&tag) {
            self.flush();
            if heading(tag).is_some() || tag == "blockquote" {
                self.style = ParaStyle::Normal;
            }
            if tag == "pre" {
                self.pre = self.pre.saturating_sub(1);
                self.style = ParaStyle::Normal;
            }
            if tag == "li" {
                self.list = None;
            }
        }
        match tag {
            "ul" | "ol" => {
                self.flush();
                self.lists.pop();
                self.list = None;
            }
            "td" | "th" if self.table_depth == 1 => {
                if tag == "th"
                    && let Some(k) = self.fmt.iter().rposition(|x| *x == Fmt::Bold)
                {
                    self.fmt.remove(k);
                }
                self.end_cell();
            }
            "tr" if self.table_depth == 1 => self.end_row(),
            "table" => {
                self.table_depth = self.table_depth.saturating_sub(1);
                if self.table_depth == 0 {
                    self.end_table();
                }
            }
            _ => {}
        }
    }

    /// The current cell ends: its paragraphs join the row (within the table limits).
    fn end_cell(&mut self) {
        self.flush();
        if let Some(t) = &mut self.table {
            let paras = std::mem::take(&mut t.cell);
            let so_far: usize = t.rows.iter().map(Vec::len).sum::<usize>() + t.row.len();
            if t.cell_open && t.row.len() < MAX_COLS && so_far < MAX_CELLS {
                t.row.push(paras);
            }
            t.cell_open = false;
        }
    }

    /// The current row ends (closing an open cell first).
    fn end_row(&mut self) {
        if self.table.as_ref().is_some_and(|t| t.cell_open) {
            self.end_cell();
        }
        if let Some(t) = &mut self.table {
            let r = std::mem::take(&mut t.row);
            if !r.is_empty() && t.rows.len() < MAX_ROWS {
                t.rows.push(r);
            }
        }
    }

    /// The table ends: it joins the output if it has any cell.
    fn end_table(&mut self) {
        self.end_row();
        if let Some(t) = self.table.take()
            && !t.rows.is_empty()
        {
            self.out.push(Block::Table(t.rows));
        }
    }
}

/// Read an HTML fragment (see the module docs). Never fails: what can't be read is skipped.
pub fn parse(html: &str) -> Vec<Block> {
    let mut end = html.len().min(MAX_HTML_BYTES);
    while !html.is_char_boundary(end) {
        end -= 1;
    }
    let src = html.get(..end).unwrap_or("");
    let mut r = Reader::new();
    let mut rest = src;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            r.text(rest);
            break;
        };
        if lt > 0 {
            r.text(rest.get(..lt).unwrap_or(""));
        }
        let tail = rest.get(lt..).unwrap_or("");
        if let Some(c) = tail.strip_prefix("<!--") {
            rest = c.find("-->").and_then(|e| c.get(e + 3..)).unwrap_or("");
            continue;
        }
        let Some(gt) = tail.find('>') else {
            // A stray '<' with no tag after it is text.
            r.text(tail);
            break;
        };
        let inner = tail.get(1..gt).unwrap_or("");
        rest = tail.get(gt + 1..).unwrap_or("");
        if inner.starts_with(['!', '?']) {
            continue;
        }
        let closing = inner.starts_with('/');
        let body = inner.trim_start_matches('/');
        let name_end = body.find(|c: char| c.is_ascii_whitespace() || c == '/').unwrap_or(body.len());
        let name = body.get(..name_end).unwrap_or("").to_ascii_lowercase();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ':') {
            if r.skip.is_none() {
                r.text(tail.get(..gt + 1).unwrap_or(""));
            }
            continue;
        }
        if closing {
            r.close(&name);
        } else {
            r.open(&name, body.get(name_end..).unwrap_or(""));
            if body.ends_with('/') {
                r.close(&name);
            }
        }
    }
    // Whatever is still open ends here.
    while let Some(o) = r.stack.last() {
        let tag = o.tag.clone();
        r.close(&tag);
    }
    if r.table.is_some() {
        r.end_table();
    }
    r.flush();
    r.out
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("<br>"),
            c => out.push(c),
        }
    }
    out
}

fn write_runs(p: &Paragraph, out: &mut String) {
    for r in &p.runs {
        let f = &r.fmt;
        let mut open = String::new();
        let mut close = Vec::new();
        let mut wrap = |o: String, c: &'static str| {
            open.push_str(&o);
            close.push(c);
        };
        if let Some(l) = &f.link {
            wrap(format!("<a href=\"{}\">", escape(l)), "</a>");
        }
        let mut css = Vec::new();
        if let Some(c) = f.color {
            css.push(format!("color:{}", c.to_hex()));
        }
        if let Some(c) = f.highlight {
            css.push(format!("background-color:{}", c.to_hex()));
        }
        if let Some(font) = &f.font {
            css.push(format!("font-family:'{}'", escape(font).replace('\'', "")));
        }
        if let Some(sz) = f.size.filter(|s| s.is_finite()) {
            css.push(format!("font-size:{sz}pt"));
        }
        if !css.is_empty() {
            wrap(format!("<span style=\"{}\">", css.join(";")), "</span>");
        }
        for (on, o, c) in [
            (f.bold == Some(true), "<b>", "</b>"),
            (f.italic == Some(true), "<i>", "</i>"),
            (f.underline == Some(true), "<u>", "</u>"),
            (f.strike == Some(true), "<s>", "</s>"),
            (f.script == Some(Script::Super), "<sup>", "</sup>"),
            (f.script == Some(Script::Sub), "<sub>", "</sub>"),
        ] {
            if on {
                wrap(o.to_string(), c);
            }
        }
        out.push_str(&open);
        out.push_str(&escape(&r.text));
        for c in close.iter().rev() {
            out.push_str(c);
        }
    }
}

fn block_tag(p: &Paragraph) -> &'static str {
    match p.style {
        ParaStyle::Heading1 => "h1",
        ParaStyle::Heading2 => "h2",
        ParaStyle::Heading3 => "h3",
        ParaStyle::Heading4 => "h4",
        ParaStyle::Heading5 => "h5",
        ParaStyle::Heading6 => "h6",
        ParaStyle::Quote => "blockquote",
        ParaStyle::Code => "pre",
        _ => "p",
    }
}

/// Write paragraphs and tables as an HTML fragment. List items nest by indent level; other
/// indented paragraphs get a left margin (27 pt per level, as on the page).
pub fn write(blocks: &[Block]) -> String {
    let mut out = String::from("<meta charset=\"utf-8\">");
    // Open lists: (level, numbered).
    let mut lists: Vec<(u8, bool)> = Vec::new();
    let close_to = |out: &mut String, lists: &mut Vec<(u8, bool)>, keep: usize| {
        while lists.len() > keep {
            if let Some((_, numbered)) = lists.pop() {
                out.push_str(if numbered { "</ol>" } else { "</ul>" });
            }
        }
    };
    for b in blocks {
        match b {
            Block::Para(p) => match p.list {
                Some(list) => {
                    let numbered = matches!(list, ListFormat::Number { .. });
                    let keep = lists.iter().take_while(|(l, n)| *l < p.level || (*l == p.level && *n == numbered)).count();
                    close_to(&mut out, &mut lists, keep);
                    if lists.last().is_none_or(|(l, _)| *l < p.level) {
                        out.push_str(if numbered { "<ol>" } else { "<ul>" });
                        lists.push((p.level, numbered));
                    }
                    out.push_str("<li>");
                    write_runs(p, &mut out);
                    out.push_str("</li>");
                }
                None => {
                    close_to(&mut out, &mut lists, 0);
                    let tag = block_tag(p);
                    if p.level > 0 {
                        out.push_str(&format!("<{tag} style=\"margin-left:{}pt\">", u32::from(p.level) * 27));
                    } else {
                        out.push_str(&format!("<{tag}>"));
                    }
                    write_runs(p, &mut out);
                    out.push_str(&format!("</{tag}>"));
                }
            },
            Block::Table(rows) => {
                close_to(&mut out, &mut lists, 0);
                out.push_str("<table border=\"1\" style=\"border-collapse:collapse\">");
                for row in rows {
                    out.push_str("<tr>");
                    for cell in row {
                        out.push_str("<td>");
                        let inner: Vec<Block> = cell.iter().cloned().map(Block::Para).collect();
                        let html = write(&inner);
                        out.push_str(html.trim_start_matches("<meta charset=\"utf-8\">"));
                        out.push_str("</td>");
                    }
                    out.push_str("</tr>");
                }
                out.push_str("</table>");
            }
        }
    }
    close_to(&mut out, &mut lists, 0);
    out
}

#[cfg(test)]
mod tests;
