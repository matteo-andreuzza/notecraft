//! Files in (catalog `FMT.Import.Markdown`, `FMT.Import.Text`, `FMT.Import.Html`): a document
//! becomes a page's title and the blocks the rich paste already places on a page.
//!
//! - **Markdown** (CommonMark with GFM tables, strikethrough and task lists) is read by
//!   `pulldown-cmark` into HTML and then by [`crate::html`]: what a note can hold is kept the
//!   same way a paste keeps it. Pictures the document names by a relative path are read
//!   through the caller's loader (the caller confines them to the document's folder); pictures
//!   from the web are dropped (no network). Task items become "☐ " / "☑ " text, which the
//!   engine turns into To Do tags.
//! - **Plain text**: a paragraph per line; leading tabs become indent levels.
//! - **HTML**: read as a paste is.
//!
//! In all three, a first-level heading that opens the document becomes the page title.
//! Bounded like the HTML reader: at most [`MAX_IMPORT_BYTES`] are read and
//! [`crate::html::MAX_IMAGES`] pictures loaded.

use notecraft_codecs::b64;
use notecraft_doc::text::MAX_LEVEL;
use notecraft_doc::{Id, ParaStyle, Paragraph};

use crate::html::{self, Block, MAX_IMAGES, MAX_PARAGRAPHS, REF};

/// Most bytes of a document read.
pub const MAX_IMPORT_BYTES: usize = 16 << 20;

/// The to-do markers a task item becomes (unchecked, checked).
pub const TODO_MARKS: [&str; 2] = ["☐ ", "☑ "];

/// A document read for one page.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Imported {
    /// The page title, when the document opens with a first-level heading.
    pub title: Option<String>,
    /// The content, in reading order (pictures as `data:` URIs).
    pub blocks: Vec<Block>,
    /// Pictures named but not loaded (missing, outside the folder, from the web, over the cap).
    pub pictures_lost: usize,
}

/// `s` cut to [`MAX_IMPORT_BYTES`] at a character boundary.
fn bounded(s: &str) -> &str {
    let mut end = s.len().min(MAX_IMPORT_BYTES);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s.get(..end).unwrap_or("")
}

/// Bytes as text: UTF-8 (a BOM dropped), else each byte as Latin-1.
pub fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let bytes = bytes.get(..bytes.len().min(MAX_IMPORT_BYTES)).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    }
}

/// A leading first-level heading becomes the title.
fn take_title(mut blocks: Vec<Block>) -> (Option<String>, Vec<Block>) {
    match blocks.first() {
        Some(Block::Para(p)) if p.style == ParaStyle::Heading1 && !p.text().trim().is_empty() => {
            let t = p.text().replace('\n', " ").trim().to_string();
            blocks.remove(0);
            (Some(t), blocks)
        }
        _ => (None, blocks),
    }
}

/// The MIME type of picture bytes, if the codecs know them.
fn mime(bytes: &[u8]) -> Option<&'static str> {
    notecraft_codecs::sniff(bytes).map(|f| f.mime())
}

/// Markdown. `load` reads a picture by the path the document gives (`None`: not loaded).
pub fn markdown(text: &str, load: &mut dyn FnMut(&str) -> Option<Vec<u8>>) -> Imported {
    use pulldown_cmark::{CowStr, Event, Options, Parser, Tag};
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut pictures: Vec<Vec<u8>> = Vec::new();
    let mut lost = 0;
    let events = Parser::new_ext(bounded(text), opts).map(|ev| match ev {
        Event::Start(Tag::Image { link_type, dest_url, title, id }) => {
            let local = !dest_url.contains("://") && !dest_url.starts_with("data:") && !dest_url.starts_with("//");
            let loaded = if local && pictures.len() < MAX_IMAGES { load(&dest_url).filter(|b| mime(b).is_some()) } else { None };
            let dest = match loaded {
                Some(bytes) => {
                    pictures.push(bytes);
                    CowStr::from(format!("{REF}{}", pictures.len() - 1))
                }
                None => {
                    if !dest_url.starts_with("data:") {
                        lost += 1;
                    }
                    dest_url
                }
            };
            Event::Start(Tag::Image { link_type, dest_url: dest, title, id })
        }
        Event::TaskListMarker(done) => Event::Text(CowStr::from(TODO_MARKS[usize::from(done)])),
        e => e,
    });
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, events);
    let blocks = html::parse_refs(&out)
        .into_iter()
        .filter_map(|b| match b {
            Block::Image { src, alt } if src.starts_with(REF) => {
                let bytes = src.strip_prefix(REF).and_then(|n| n.parse::<usize>().ok()).and_then(|n| pictures.get(n))?;
                Some(Block::Image { src: format!("data:{};base64,{}", mime(bytes)?, b64::encode(bytes)), alt })
            }
            b => Some(b),
        })
        .collect();
    let (title, blocks) = take_title(blocks);
    Imported { title, blocks, pictures_lost: lost }
}

/// Plain text: a paragraph per line, leading tabs as indent levels.
pub fn plain_text(text: &str) -> Imported {
    let blocks = bounded(text)
        .split('\n')
        .take(MAX_PARAGRAPHS)
        .map(|line| {
            let line = line.strip_suffix('\r').unwrap_or(line);
            let tabs = line.len() - line.trim_start_matches('\t').len();
            let mut p = Paragraph::with_text(Id::NIL, &line[tabs..]);
            p.level = u8::try_from(tabs).unwrap_or(u8::MAX).min(MAX_LEVEL);
            Block::Para(p)
        })
        .collect::<Vec<_>>();
    // A trailing newline isn't an empty last paragraph.
    let mut blocks = blocks;
    if blocks.len() > 1 && matches!(blocks.last(), Some(Block::Para(p)) if p.is_empty()) {
        blocks.pop();
    }
    Imported { title: None, blocks, pictures_lost: 0 }
}

/// An HTML document, read as a paste is.
pub fn html_document(text: &str) -> Imported {
    let (title, blocks) = take_title(html::parse(bounded(text)));
    Imported { title, blocks, pictures_lost: 0 }
}

#[cfg(test)]
#[path = "import/tests.rs"]
mod tests;
