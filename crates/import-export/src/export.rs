//! Pages out to other applications (catalog `BS.Export.*`, `FMT.Export.*`): [`collect`] turns
//! pages into a format-neutral [`Export`] — every page's title and date, its objects in reading
//! order, and the files they use (pictures, attachments) — which [`html_document`], [`mht`] and
//! [`crate::markdown::write`] then write.
//!
//! **Reading order.** A page is a free-form canvas, a document is a flow: objects are read top
//! to bottom, then left to right (by their top-left corner). Pictures become `<img>` with the
//! size they have on the page, attachments a link to the file. Ink is not exported (no format
//! here draws it) and is counted in [`Report::ink_lost`] so the caller can say so.
//!
//! **Single File Web Page (.mht)** is MIME `multipart/related` (RFC 2557, RFC 2045/2046): the
//! HTML part first, then one base64 part per file, each with a `Content-Location` the HTML
//! names relatively (resolved against the HTML part's location, RFC 2557 §5).

use std::collections::HashMap;
use std::sync::Arc;

use notecraft_codecs::b64;
use notecraft_doc::{Page, PageObject, Paragraph};

use crate::html::{self, Block, TableBlock, escape};

/// Most pages one export holds.
pub const MAX_PAGES: usize = 10_000;

/// A file a page uses, written beside the document (Markdown), inside it (.mht) or as a
/// `data:` URI (single-file HTML).
#[derive(Debug, Clone, PartialEq)]
pub struct Resource {
    /// Its file name: one safe path component, unique in the export.
    pub name: String,
    /// Its MIME type.
    pub mime: String,
    /// The bytes.
    pub bytes: Arc<[u8]>,
}

/// One thing on a page, in reading order.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A note container's paragraphs.
    Paragraphs(Vec<Paragraph>),
    /// A table.
    Table(TableBlock),
    /// A picture: the [`Resource`] index, alt text and size on the page in points.
    Picture {
        /// Index into [`Export::resources`].
        res: usize,
        /// Alternative text.
        alt: String,
        /// Width, points.
        width: f32,
        /// Height, points.
        height: f32,
    },
    /// An attached file: the [`Resource`] index and the name shown.
    File {
        /// Index into [`Export::resources`].
        res: usize,
        /// The file name shown on the page.
        name: String,
    },
}

/// One page of an export.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportPage {
    /// The title (never empty: an untitled page gets the page list's name).
    pub title: String,
    /// The date and time line under the title.
    pub date: String,
    /// Subpage level (0–2).
    pub level: u8,
    /// What is on it, in reading order.
    pub items: Vec<Item>,
}

/// What an export kept and lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Report {
    /// Pages written.
    pub pages: usize,
    /// Pictures written.
    pub pictures: usize,
    /// Attached files written.
    pub files: usize,
    /// Tables written.
    pub tables: usize,
    /// Ink strokes left out.
    pub ink_lost: usize,
    /// Pictures and files left out because their bytes were lost on load.
    pub files_lost: usize,
}

/// Pages ready to write.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Export {
    /// The pages, in order.
    pub pages: Vec<ExportPage>,
    /// The files they use (each blob once).
    pub resources: Vec<Resource>,
    /// What was kept and lost.
    pub report: Report,
}

/// `name` made one safe path component (no separators, no leading dot, not empty).
fn safe_name(name: &str, fallback: &str) -> String {
    let n: String = name.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '_' } else { c }).collect();
    let n = n.trim().trim_start_matches('.').trim();
    if n.is_empty() { fallback.to_string() } else { n.chars().take(120).collect() }
}

/// `name` with ` (2)`, ` (3)`… before its extension until it is not in `taken`.
fn unique(name: String, taken: &mut std::collections::HashSet<String>) -> String {
    if taken.insert(name.to_lowercase()) {
        return name;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name.as_str(), ""),
    };
    for n in 2.. {
        let c = format!("{stem} ({n}){ext}");
        if taken.insert(c.to_lowercase()) {
            return c;
        }
    }
    name
}

struct Collector {
    resources: Vec<Resource>,
    by_blob: HashMap<notecraft_doc::Id, usize>,
    taken: std::collections::HashSet<String>,
}

impl Collector {
    /// The resource index for a blob and whether it is new; `name` is asked only the first
    /// time (a blob on several pages is written once).
    fn add(&mut self, blob: &notecraft_doc::Blob, name: impl FnOnce(usize) -> String, mime: String) -> (usize, bool) {
        if let Some(&i) = self.by_blob.get(&blob.id) {
            return (i, false);
        }
        let name = unique(name(self.resources.len() + 1), &mut self.taken);
        self.resources.push(Resource { name, mime, bytes: blob.bytes.clone() });
        let i = self.resources.len() - 1;
        self.by_blob.insert(blob.id, i);
        (i, true)
    }
}

/// Gather `pages` for export; `utc_offset_min` sets the date line's time zone.
pub fn collect(pages: &[&Page], utc_offset_min: i32) -> Export {
    let mut c = Collector { resources: Vec::new(), by_blob: HashMap::new(), taken: Default::default() };
    let mut report = Report::default();
    let mut out = Vec::new();
    for page in pages.iter().take(MAX_PAGES) {
        let mut objects: Vec<&PageObject> = page.objects.iter().collect();
        objects.sort_by(|a, b| {
            let (pa, pb) = (a.pos(), b.pos());
            pa.y.total_cmp(&pb.y).then(pa.x.total_cmp(&pb.x))
        });
        let mut items = Vec::new();
        for o in objects {
            match o {
                PageObject::Outline(ol) => items.push(Item::Paragraphs(ol.paragraphs.clone())),
                PageObject::Table(t) => {
                    report.tables += 1;
                    items.push(Item::Table(TableBlock::of_table(t)));
                }
                PageObject::Image(im) => {
                    if im.blob.bytes.is_empty() {
                        report.files_lost += 1;
                        continue;
                    }
                    let (ext, mime) = match notecraft_codecs::sniff(&im.blob.bytes) {
                        Some(f) => (f.extension(), f.mime().to_string()),
                        None => ("bin", "application/octet-stream".to_string()),
                    };
                    let (res, new) = c.add(&im.blob, |n| format!("image{n:03}.{ext}"), mime);
                    report.pictures += usize::from(new);
                    items.push(Item::Picture { res, alt: im.alt_text.clone(), width: im.width, height: im.height });
                }
                PageObject::Attachment(a) => {
                    if a.blob.bytes.is_empty() {
                        report.files_lost += 1;
                        continue;
                    }
                    let (res, new) = c.add(&a.blob, |_| safe_name(&a.name, "file"), "application/octet-stream".into());
                    report.files += usize::from(new);
                    items.push(Item::File { res, name: a.name.clone() });
                }
                PageObject::Ink(_) => report.ink_lost += 1,
            }
        }
        let t = page.shown_time();
        out.push(ExportPage {
            title: page.display_title(),
            date: format!("{}   {}", t.long_date(utc_offset_min), t.short_time(utc_offset_min)),
            level: page.level,
            items,
        });
    }
    report.pages = out.len();
    Export { pages: out, resources: c.resources, report }
}

/// How the HTML names a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Links<'a> {
    /// Inline `data:` URIs (one self-contained `.html`).
    Inline,
    /// Relative paths: the file's name, under a folder when one is given (`.mht` parts, or
    /// the files written beside the document).
    Relative(Option<&'a str>),
}

/// Percent-encode a path component for an `href`/`src` (RFC 3986 unreserved kept).
pub(crate) fn url_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~()".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The `src`/`href` of resource `r`.
pub(crate) fn link(r: &Resource, links: Links) -> String {
    match links {
        Links::Inline => format!("data:{};base64,{}", r.mime, b64::encode(&r.bytes)),
        Links::Relative(None) => url_component(&r.name),
        Links::Relative(Some(dir)) => format!("{}/{}", url_component(dir), url_component(&r.name)),
    }
}

/// A complete HTML document (UTF-8) of the export, titled `title`.
pub fn html_document(e: &Export, title: &str, links: Links) -> String {
    let mut out = String::from("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str(&format!("<title>{}</title>\n", escape(title)));
    out.push_str("<meta name=\"generator\" content=\"NoteCraft\">\n");
    out.push_str(
        "<style>body{font-family:Calibri,Carlito,sans-serif;font-size:11pt}.date{color:#767676;font-size:10pt}\
         table{border-collapse:collapse}td{border:1px solid #a3a3a3;padding:2pt 4pt;vertical-align:top}\
         section+section{margin-top:24pt;border-top:1px solid #d1d1d1}</style>\n",
    );
    out.push_str("</head>\n<body>\n");
    for p in &e.pages {
        out.push_str("<section>\n");
        out.push_str(&format!("<h1 class=\"title\">{}</h1>\n<p class=\"date\">{}</p>\n", escape(&p.title), escape(&p.date)));
        for it in &p.items {
            match it {
                Item::Paragraphs(ps) => {
                    let blocks: Vec<Block> = ps.iter().cloned().map(Block::Para).collect();
                    out.push_str(html::write(&blocks).trim_start_matches("<meta charset=\"utf-8\">"));
                }
                Item::Table(t) => out.push_str(html::write(&[Block::Table(t.clone())]).trim_start_matches("<meta charset=\"utf-8\">")),
                Item::Picture { res, alt, width, height } => {
                    let Some(r) = e.resources.get(*res) else { continue };
                    out.push_str(&format!(
                        "<p><img src=\"{}\" alt=\"{}\" style=\"width:{}pt;height:{}pt\"></p>",
                        escape(&link(r, links)),
                        escape(alt),
                        pt(*width),
                        pt(*height)
                    ));
                }
                Item::File { res, name } => {
                    let Some(r) = e.resources.get(*res) else { continue };
                    out.push_str(&format!("<p><a href=\"{}\">{}</a></p>", escape(&link(r, links)), escape(name)));
                }
            }
            out.push('\n');
        }
        out.push_str("</section>\n");
    }
    out.push_str("</body>\n</html>\n");
    out
}

/// A length in points for CSS (finite, at most two decimals).
fn pt(v: f32) -> String {
    let v = if v.is_finite() { v.max(0.0) } else { 0.0 };
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `s` as a MIME header value: ASCII as is, otherwise an RFC 2047 encoded word.
fn header_text(s: &str) -> String {
    let s: String = s.chars().filter(|c| !c.is_control()).collect();
    if s.is_ascii() { s } else { format!("=?utf-8?B?{}?=", b64::encode(s.as_bytes())) }
}

/// Base64 in lines of 76 characters (RFC 2045 §6.8).
fn b64_lines(bytes: &[u8], out: &mut Vec<u8>) {
    let s = b64::encode(bytes);
    for chunk in s.as_bytes().chunks(76) {
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\r\n");
    }
}

/// Where the HTML part says it lives; the files are its siblings.
const MHT_BASE: &str = "file:///notecraft-export/";

/// The export as a Single File Web Page (`.mht`, RFC 2557), titled `title`.
pub fn mht(e: &Export, title: &str) -> Vec<u8> {
    let html = html_document(e, title, Links::Relative(None));
    // A boundary can't occur in base64 lines (they never start with "--").
    let boundary = "----=_NextPart_NoteCraft_0000";
    let mut out = Vec::new();
    let head = format!(
        "From: <Saved by NoteCraft>\r\nSubject: {}\r\nMIME-Version: 1.0\r\nContent-Type: multipart/related;\r\n\
         \tboundary=\"{boundary}\";\r\n\ttype=\"text/html\"\r\n\r\nThis is a multi-part message in MIME format.\r\n\r\n",
        header_text(title)
    );
    out.extend_from_slice(head.as_bytes());
    let part = |out: &mut Vec<u8>, mime: &str, location: &str, bytes: &[u8]| {
        out.extend_from_slice(
            format!("--{boundary}\r\nContent-Type: {mime}\r\nContent-Transfer-Encoding: base64\r\nContent-Location: {location}\r\n\r\n").as_bytes(),
        );
        b64_lines(bytes, out);
        out.extend_from_slice(b"\r\n");
    };
    part(&mut out, "text/html; charset=\"utf-8\"", &format!("{MHT_BASE}index.html"), html.as_bytes());
    for r in &e.resources {
        part(&mut out, &r.mime, &format!("{MHT_BASE}{}", url_component(&r.name)), &r.bytes);
    }
    out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    out
}

#[cfg(test)]
#[path = "export/tests.rs"]
mod tests;
