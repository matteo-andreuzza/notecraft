//! Markdown out (catalog `FMT.Export.Markdown`): CommonMark with the GitHub extensions most
//! readers know — pipe tables, `~~strikethrough~~` and task list items `- [ ]` for to-do tags.
//!
//! **What carries over:** headings, paragraphs, bold, italic, strikethrough, links, sub- and
//! superscript and underline (as the raw HTML CommonMark allows), soft line breaks, bulleted and
//! numbered lists with their nesting, to-do tags (checked or not), quotes, code (a fenced block
//! per run of Code paragraphs), tables (the first row is the header row Markdown requires;
//! merged cells keep their text in the top-left cell), pictures and attached files (links to
//! the files written beside the document). **What does not:** fonts, sizes, colours,
//! highlight, alignment, other note tags, numbering styles other than decimal, and the indent
//! of paragraphs outside lists (Markdown has no indent; four spaces would make code).

use notecraft_doc::{CharFormat, ListFormat, ParaStyle, Paragraph, Script};

use crate::export::{Export, Item, Links, link};
use crate::html::TableBlock;

/// Characters escaped anywhere in text (CommonMark §2.4: any ASCII punctuation may be).
const ALWAYS: &str = "\\`*_[]<>|~";

/// `s` escaped for Markdown inline text; `line_start` also escapes what would start a block
/// (`#`, `>`, `-`, `+`, `=` and a number followed by `.` or `)`).
fn escape(s: &str, line_start: bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    if line_start {
        let lead = rest.len() - rest.trim_start_matches(' ').len();
        out.push_str(&rest[..lead]);
        rest = &rest[lead..];
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits > 0 && rest[digits..].starts_with(['.', ')']) {
            out.push_str(&rest[..digits]);
            out.push('\\');
            rest = &rest[digits..];
        } else if rest.starts_with(['#', '+', '-', '=', '>']) {
            out.push('\\');
        }
    }
    for c in rest.chars() {
        if ALWAYS.contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The parts of a run's format Markdown shows.
#[derive(Clone, PartialEq, Default)]
struct Look {
    bold: bool,
    italic: bool,
    strike: bool,
    underline: bool,
    script: Option<Script>,
    link: Option<String>,
}

impl Look {
    fn of(f: &CharFormat) -> Look {
        Look {
            bold: f.bold == Some(true),
            italic: f.italic == Some(true),
            strike: f.strike == Some(true),
            underline: f.underline == Some(true),
            script: f.script.filter(|s| *s != Script::Normal),
            link: f.link.clone().filter(|l| !l.trim().is_empty()),
        }
    }
}

/// A link target for `[text](…)`: angle brackets when it has spaces or parentheses.
fn target(url: &str) -> String {
    let u: String = url.chars().filter(|c| !c.is_control() && !"<>".contains(*c)).collect();
    if u.contains([' ', '(', ')']) { format!("<{u}>") } else { u }
}

/// The paragraph's runs as inline Markdown; a soft line break continues with `indent`.
fn inline(p: &Paragraph, indent: &str) -> String {
    // Merge neighbours that look the same, so `**a****b**` never appears.
    let mut segs: Vec<(Look, String)> = Vec::new();
    for r in &p.runs {
        let look = Look::of(&r.fmt);
        match segs.last_mut() {
            Some((l, t)) if *l == look => t.push_str(&r.text),
            _ => segs.push((look, r.text.clone())),
        }
    }
    let mut out = String::new();
    for (look, text) in segs {
        // Emphasis can't start or end on a space: keep spaces outside the markers.
        let core = text.trim_matches(' ');
        if core.is_empty() {
            out.push_str(&text);
            continue;
        }
        let lead = &text[..text.len() - text.trim_start_matches(' ').len()];
        let trail = &text[text.trim_end_matches(' ').len()..];
        let mut s = String::new();
        for (k, line) in core.split('\n').enumerate() {
            if k > 0 {
                s.push_str("\\\n");
                s.push_str(indent);
            }
            s.push_str(&escape(&line.replace('\t', "    "), out.is_empty() && lead.is_empty() && k == 0 || k > 0));
        }
        let mut close = Vec::new();
        for (on, o, c) in [
            (look.bold, "**", "**"),
            (look.italic, "*", "*"),
            (look.strike, "~~", "~~"),
            (look.underline, "<u>", "</u>"),
            (look.script == Some(Script::Super), "<sup>", "</sup>"),
            (look.script == Some(Script::Sub), "<sub>", "</sub>"),
        ] {
            if on {
                s.insert_str(0, o);
                close.push(c);
            }
        }
        for c in close.iter().rev() {
            s.push_str(c);
        }
        if let Some(l) = &look.link {
            s = format!("[{s}]({})", target(l));
        }
        out.push_str(lead);
        out.push_str(&s);
        out.push_str(trail);
    }
    out
}

/// The to-do mark of a paragraph's first checkable tag.
fn task(p: &Paragraph) -> Option<&'static str> {
    p.tags.iter().find(|t| t.symbol.is_checkbox()).map(|t| if t.checked { "[x] " } else { "[ ] " })
}

/// A container's paragraphs as Markdown blocks (blank lines between blocks, none between the
/// items of one list).
fn paragraphs(ps: &[Paragraph], out: &mut String) {
    // Levels of the open list items, outermost first.
    let mut stack: Vec<u8> = Vec::new();
    // The fence of the code block being written.
    let mut fence: Option<&'static str> = None;
    let mut first = true;
    for (i, p) in ps.iter().enumerate() {
        let is_code = p.style == ParaStyle::Code && p.list.is_none();
        if let (Some(f), false) = (fence, is_code) {
            out.push_str(f);
            out.push('\n');
            fence = None;
        }
        if is_code {
            stack.clear();
            if fence.is_none() {
                if !first {
                    out.push('\n');
                }
                // A fence the block's text doesn't contain.
                let run = ps[i..].iter().take_while(|q| q.style == ParaStyle::Code && q.list.is_none());
                let f = if run.into_iter().any(|q| q.text().contains("```")) { "~~~~" } else { "```" };
                out.push_str(f);
                out.push('\n');
                fence = Some(f);
            }
            out.push_str(&p.text());
            out.push('\n');
            first = false;
            continue;
        }
        let todo = task(p);
        if p.list.is_some() || todo.is_some() {
            if stack.is_empty() && !first {
                out.push('\n');
            }
            while stack.last().is_some_and(|&l| l >= p.level) {
                stack.pop();
            }
            let indent = "    ".repeat(stack.len());
            stack.push(p.level);
            let marker = match p.list {
                Some(ListFormat::Number { restart, .. }) => format!("{}. ", restart.unwrap_or(1).min(999_999_999)),
                _ => "- ".to_string(),
            };
            out.push_str(&indent);
            out.push_str(&marker);
            if let Some(t) = todo {
                out.push_str(t);
            }
            let cont = format!("{indent}    ");
            out.push_str(&inline(p, &cont));
            out.push('\n');
        } else {
            let prefix = match p.style {
                ParaStyle::Heading1 => "# ",
                ParaStyle::Heading2 => "## ",
                ParaStyle::Heading3 => "### ",
                ParaStyle::Heading4 => "#### ",
                ParaStyle::Heading5 => "##### ",
                ParaStyle::Heading6 => "###### ",
                ParaStyle::Quote => "> ",
                _ => "",
            };
            let text = inline(p, if p.style == ParaStyle::Quote { "> " } else { "" });
            if text.trim().is_empty() {
                // An empty paragraph is spacing on the page; Markdown collapses it anyway.
                continue;
            }
            stack.clear();
            if !first {
                out.push('\n');
            }
            out.push_str(prefix);
            // A heading holds one line: soft breaks become spaces.
            if prefix.starts_with('#') {
                out.push_str(&text.replace("\\\n", " "))
            } else {
                out.push_str(&text)
            }
            out.push('\n');
        }
        first = false;
    }
    if let Some(f) = fence {
        out.push_str(f);
        out.push('\n');
    }
}

/// A cell's paragraphs on one line of a pipe table.
fn cell_text(ps: &[Paragraph]) -> String {
    ps.iter().map(|p| inline(p, "").replace("\\\n", "<br>")).collect::<Vec<_>>().join("<br>")
}

fn table(t: &TableBlock, out: &mut String) {
    let cols = t.cells.iter().map(Vec::len).max().unwrap_or(0).max(1);
    for (r, row) in t.cells.iter().enumerate() {
        out.push('|');
        for c in 0..cols {
            let text = row.get(c).map(|ps| cell_text(ps)).unwrap_or_default();
            out.push(' ');
            out.push_str(&text);
            out.push_str(" |");
        }
        out.push('\n');
        if r == 0 {
            out.push('|');
            out.push_str(&" --- |".repeat(cols));
            out.push('\n');
        }
    }
}

/// The export as one Markdown document. Files are linked under `files_dir` (relative to the
/// document; the caller writes [`Export::resources`] there).
pub fn write(e: &Export, files_dir: &str) -> String {
    let mut out = String::new();
    for p in &e.pages {
        if !out.is_empty() {
            out.push_str("\n---\n\n");
        }
        out.push_str("# ");
        out.push_str(&escape(&p.title, false));
        out.push_str("\n\n*");
        out.push_str(&escape(&p.date, false));
        out.push_str("*\n");
        for it in &p.items {
            let mut block = String::new();
            match it {
                Item::Paragraphs(ps) => paragraphs(ps, &mut block),
                Item::Table { cells, .. } => table(cells, &mut block),
                Item::Picture { res, alt, .. } => {
                    if let Some(r) = e.resources.get(*res) {
                        block.push_str(&format!("![{}]({})\n", escape(alt, false), target(&link(r, Links::Relative(Some(files_dir))))));
                    }
                }
                Item::File { res, name } => {
                    if let Some(r) = e.resources.get(*res) {
                        block.push_str(&format!("[{}]({})\n", escape(name, false), target(&link(r, Links::Relative(Some(files_dir))))));
                    }
                }
            }
            if !block.is_empty() {
                out.push('\n');
                out.push_str(&block);
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "markdown/tests.rs"]
mod tests;
