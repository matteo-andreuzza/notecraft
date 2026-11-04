//! NoteCraft rich-text editing (layer **L2**: algorithms over the L1 model).
//!
//! Pure functions that edit a [`Paragraph`]'s runs: insert, delete, split, merge, apply or
//! toggle character formats over byte ranges, and the boundary queries caret motion needs
//! (graphemes, words). Every function:
//!
//! - takes **byte offsets** into the paragraph's concatenated text and first snaps them to a
//!   grapheme boundary ([`snap`]), so a hostile or stale offset can never split a UTF-8 sequence
//!   or a user-perceived character (emoji ZWJ sequences, combining marks);
//! - ends with [`Paragraph::normalize`], so run invariants always hold;
//! - never panics: out-of-range offsets are clamped.
//!
//! It sits at L2 because it knows the model (L1) but nothing about sessions, undo or the UI.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use notecraft_doc::text::{CharFormat, ListFormat, MAX_FONT_SIZE, MIN_FONT_SIZE, Numbering, Paragraph, Run, sanitize_text};
use notecraft_doc::{Bullet, Id};
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

/// Snap `offset` to the nearest grapheme boundary at or before it, clamped to the text length.
///
/// Looks only around `offset` (a grapheme cursor), not from the start of the text: the UI
/// validates the selection through this many times a frame.
pub fn snap(text: &str, offset: usize) -> usize {
    let mut o = offset.min(text.len());
    while !text.is_char_boundary(o) {
        o = o.saturating_sub(1);
    }
    let mut cursor = GraphemeCursor::new(o, text.len(), true);
    match cursor.is_boundary(text, 0) {
        Ok(true) => o,
        Ok(false) => match cursor.prev_boundary(text, 0) {
            Ok(b) => b.unwrap_or(0),
            Err(_) => snap_scan(text, o),
        },
        // The whole text is one chunk starting at 0, so the cursor never asks for more context.
        Err(_) => snap_scan(text, o),
    }
}

/// [`snap`] by a scan from the start (the reference the cursor version must agree with).
fn snap_scan(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    if offset == text.len() {
        return offset;
    }
    let mut last = 0;
    for (i, _) in text.grapheme_indices(true) {
        if i > offset {
            break;
        }
        last = i;
    }
    last
}

/// The grapheme boundary after `offset` (or the end).
pub fn next_grapheme(text: &str, offset: usize) -> usize {
    let o = snap(text, offset);
    text.get(o..).and_then(|rest| rest.graphemes(true).next()).map(|g| o + g.len()).unwrap_or(text.len())
}

/// The grapheme boundary before `offset` (or 0).
pub fn prev_grapheme(text: &str, offset: usize) -> usize {
    let o = snap(text, offset);
    text.get(..o).and_then(|head| head.grapheme_indices(true).next_back()).map(|(i, _)| i).unwrap_or(0)
}

fn is_word(s: &str) -> bool {
    s.chars().any(|c| c.is_alphanumeric())
}

/// Ctrl+Left: the start of the word at or before `offset` (skipping spaces and punctuation
/// first), as word processors move (catalog `KB.General.WordLeft`).
pub fn word_left(text: &str, offset: usize) -> usize {
    let o = snap(text, offset);
    let mut target = 0;
    for (i, w) in text.split_word_bound_indices() {
        if i >= o {
            break;
        }
        if is_word(w) {
            target = i;
        }
    }
    target
}

/// Ctrl+Right: the start of the next word after `offset` (catalog `KB.General.WordRight`).
pub fn word_right(text: &str, offset: usize) -> usize {
    let o = snap(text, offset);
    text.split_word_bound_indices().find(|&(i, w)| i > o && is_word(w)).map(|(i, _)| i).unwrap_or(text.len())
}

/// The word containing `offset` (double-click selection), as a byte range; an offset in
/// whitespace selects that run of whitespace.
pub fn word_at(text: &str, offset: usize) -> std::ops::Range<usize> {
    let o = snap(text, offset);
    for (i, w) in text.split_word_bound_indices() {
        if o >= i && o < i + w.len() {
            return i..i + w.len();
        }
    }
    // At the end: the last word.
    text.split_word_bound_indices().next_back().map(|(i, w)| i..i + w.len()).unwrap_or(0..0)
}

/// Start of the logical line containing `offset` (after the previous soft break).
pub fn line_start(text: &str, offset: usize) -> usize {
    let o = snap(text, offset);
    text.get(..o).and_then(|h| h.rfind('\n')).map(|i| i + 1).unwrap_or(0)
}

/// End of the logical line containing `offset` (before the next soft break).
pub fn line_end(text: &str, offset: usize) -> usize {
    let o = snap(text, offset);
    text.get(o..).and_then(|t| t.find('\n')).map(|i| o + i).unwrap_or(text.len())
}

/// Split the run list so that a run boundary falls at byte `at`; returns the index of the run
/// that starts at `at` (== runs.len() when `at` is the end).
fn cut(runs: &mut Vec<Run>, at: usize) -> usize {
    let mut pos = 0;
    let mut i = 0;
    while i < runs.len() {
        let Some(r) = runs.get(i) else { break };
        let len = r.text.len();
        if at == pos {
            return i;
        }
        if at < pos + len {
            let k = at - pos;
            // `at` was snapped to a grapheme boundary of the whole text, hence a char boundary
            // inside this run.
            let (l, rt) = (r.text.get(..k).unwrap_or("").to_string(), r.text.get(k..).unwrap_or("").to_string());
            let fmt = r.fmt.clone();
            if let Some(cur) = runs.get_mut(i) {
                cur.text = l;
            }
            runs.insert(i + 1, Run { text: rt, fmt });
            return i + 1;
        }
        pos += len;
        i += 1;
    }
    runs.len()
}

/// Insert `text` at `offset` with format `fmt`; returns the offset just after the inserted
/// text. The text is sanitized first (control characters dropped).
pub fn insert(p: &mut Paragraph, offset: usize, text: &str, fmt: &CharFormat) -> usize {
    let full = p.text();
    let o = snap(&full, offset);
    let clean = sanitize_text(text);
    if clean.is_empty() {
        return o;
    }
    let idx = cut(&mut p.runs, o);
    let n = clean.len();
    p.runs.insert(idx, Run { text: clean, fmt: fmt.clone() });
    p.normalize();
    o + n
}

/// Delete the byte range `range` (snapped to graphemes, clamped); returns the deleted text.
pub fn delete(p: &mut Paragraph, range: std::ops::Range<usize>) -> String {
    let full = p.text();
    let (a, b) = (snap(&full, range.start.min(range.end)), snap(&full, range.end.max(range.start)));
    if a >= b {
        return String::new();
    }
    let removed = full.get(a..b).unwrap_or("").to_string();
    let i = cut(&mut p.runs, a);
    let j = cut(&mut p.runs, b);
    if i < j && j <= p.runs.len() {
        p.runs.drain(i..j);
    }
    p.normalize();
    removed
}

/// The runs covering `range` (copies, with their formats), for copy/cut and the clipboard.
pub fn slice(p: &Paragraph, range: std::ops::Range<usize>) -> Vec<Run> {
    let full = p.text();
    let (a, b) = (snap(&full, range.start.min(range.end)), snap(&full, range.end.max(range.start)));
    let mut out = Vec::new();
    let mut pos = 0;
    for r in &p.runs {
        let (s, e) = (pos, pos + r.text.len());
        let (cs, ce) = (a.max(s), b.min(e));
        if cs < ce
            && let Some(t) = r.text.get(cs - s..ce - s)
        {
            out.push(Run { text: t.to_string(), fmt: r.fmt.clone() });
        }
        pos = e;
    }
    out
}

/// Insert a list of formatted runs at `offset` (paste); returns the offset after them.
pub fn insert_runs(p: &mut Paragraph, offset: usize, runs: &[Run]) -> usize {
    let mut o = snap(&p.text(), offset);
    for r in runs {
        o = insert(p, o, &r.text, &r.fmt);
    }
    o
}

/// Split `p` at `offset` (Enter). `p` keeps the left part; the returned paragraph (with
/// `new_id`) holds the right part and inherits the list format, level, alignment, style and
/// to-do tags (unchecked), as the reference continues a list or checklist on the next line
/// (catalog `BEH.Outline.EnterNewParagraph`; tag carry-over unverified). A heading style is not
/// continued onto an empty new paragraph split off at the end: it becomes Normal (unverified,
/// word-processor convention).
pub fn split(p: &mut Paragraph, offset: usize, new_id: Id) -> Paragraph {
    let full = p.text();
    let o = snap(&full, offset);
    let idx = cut(&mut p.runs, o);
    let right_runs: Vec<Run> = p.runs.drain(idx.min(p.runs.len())..).collect();
    let mut q = Paragraph::new(new_id);
    q.runs = right_runs;
    q.level = p.level;
    q.align = p.align;
    q.list = match p.list {
        Some(ListFormat::Number { style, .. }) => Some(ListFormat::Number { style, restart: None }),
        other => other,
    };
    q.rtl = p.rtl;
    q.style = p.style;
    if o == full.len() && p.style.is_heading() {
        q.style = notecraft_doc::ParaStyle::Normal;
    }
    q.tags = p
        .tags
        .iter()
        .filter(|t| t.symbol.is_checkbox())
        .map(|t| {
            let mut t = t.clone();
            t.checked = false;
            t
        })
        .collect();
    q.space_before = p.space_before;
    q.space_after = p.space_after;
    q.line_spacing = p.line_spacing;
    p.normalize();
    q.normalize();
    q
}

/// Append `right`'s runs to `left` (Backspace at the start of a paragraph, Delete at the end);
/// returns the join offset (where the caret goes).
pub fn merge(left: &mut Paragraph, right: Paragraph) -> usize {
    let at = left.len();
    left.runs.extend(right.runs);
    left.normalize();
    at
}

/// Apply `f` to the format overrides of every run in `range` (snapped and clamped).
pub fn apply_format(p: &mut Paragraph, range: std::ops::Range<usize>, f: &dyn Fn(&mut CharFormat)) {
    let full = p.text();
    let (a, b) = (snap(&full, range.start.min(range.end)), snap(&full, range.end.max(range.start)));
    if a >= b {
        return;
    }
    let i = cut(&mut p.runs, a);
    let j = cut(&mut p.runs, b);
    for r in p.runs.iter_mut().take(j).skip(i) {
        f(&mut r.fmt);
        r.fmt = std::mem::take(&mut r.fmt).sanitized();
    }
    simplify(p);
}

/// Drop overrides that equal the paragraph style's base format (catalog
/// `BEH.Text.FormatToggles`): toggling Bold on and off again leaves no trace, so the runs merge
/// back and later style changes show through. An override that *differs* from the base (a
/// non-bold word in a bold heading) is kept.
pub fn simplify(p: &mut Paragraph) {
    let base = p.style.def().char;
    for r in &mut p.runs {
        let f = &mut r.fmt;
        if f.bold == base.bold {
            f.bold = None;
        }
        if f.italic == base.italic {
            f.italic = None;
        }
        if f.underline == base.underline {
            f.underline = None;
        }
        if f.strike == base.strike {
            f.strike = None;
        }
        if f.script == base.script {
            f.script = None;
        }
        if f.font.is_some() && f.font == base.font {
            f.font = None;
        }
        if f.size.is_some() && f.size == base.size {
            f.size = None;
        }
        if f.color.is_some() && f.color == base.color {
            f.color = None;
        }
    }
    p.normalize();
}

/// Whether `pred` holds for the **effective** format of every character in `range`; an empty
/// range tests the format at its position.
pub fn all_effective(p: &Paragraph, range: std::ops::Range<usize>, pred: &dyn Fn(&CharFormat) -> bool) -> bool {
    let full = p.text();
    let (a, b) = (snap(&full, range.start.min(range.end)), snap(&full, range.end.max(range.start)));
    let base = p.style.def().char;
    if a >= b {
        return pred(&p.fmt_at(a).over(&base));
    }
    let mut pos = 0;
    for r in &p.runs {
        let (s, e) = (pos, pos + r.text.len());
        if s < b && e > a && !pred(&r.fmt.over(&base)) {
            return false;
        }
        pos = e;
    }
    true
}

/// The font-size ladder of Increase/Decrease Font Size (catalog `BEH.Text.FontSizeSteps`;
/// unverified for OneNote: the common Office list).
pub const FONT_SIZES: [f32; 16] = [8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 36.0, 48.0, 72.0, 96.0];

/// The next larger font size (Ctrl+Shift+>): the next ladder step, or +10 pt above 72.
pub fn grow_size(size: f32) -> f32 {
    let s = if size.is_finite() { size } else { notecraft_doc::text::DEFAULT_SIZE };
    if s >= 72.0 {
        return (s + 10.0).min(MAX_FONT_SIZE);
    }
    FONT_SIZES.iter().copied().find(|&v| v > s + 1e-3).unwrap_or(MAX_FONT_SIZE).min(MAX_FONT_SIZE)
}

/// The next smaller font size (Ctrl+Shift+<): the previous ladder step, or −10 pt above 72.
pub fn shrink_size(size: f32) -> f32 {
    let s = if size.is_finite() { size } else { notecraft_doc::text::DEFAULT_SIZE };
    if s > 82.0 {
        return (s - 10.0).max(72.0);
    }
    if s > 72.0 {
        return 72.0;
    }
    FONT_SIZES.iter().rev().copied().find(|&v| v < s - 1e-3).unwrap_or(MIN_FONT_SIZE.max(1.0)).max(MIN_FONT_SIZE)
}

/// Auto-list detection (catalog `BEH.Outline.AutoBullet`, `BEH.Outline.AutoNumber`): when the
/// user types a space after one of these prefixes at the very start of a paragraph, the prefix
/// is removed and the paragraph becomes a list item. Returns the list format and the prefix
/// length in bytes (including the space).
///
/// `*`/`-`/`>` → bullets; `1.` `1)` → decimal; `a.` `a)` → letters; `i.` → roman; `A.` `I.` →
/// upper case. Only the first item value 1/a/i/A/I starts a list (unverified: the reference
/// may also start at other values).
pub fn detect_list_prefix(text: &str) -> Option<(ListFormat, usize)> {
    let table: [(&str, ListFormat); 13] = [
        ("* ", ListFormat::Bullet(Bullet::Disc)),
        ("- ", ListFormat::Bullet(Bullet::Dash)),
        ("> ", ListFormat::Bullet(Bullet::Arrow)),
        ("1. ", ListFormat::Number { style: Numbering::Decimal, restart: None }),
        ("1) ", ListFormat::Number { style: Numbering::DecimalParen, restart: None }),
        ("a. ", ListFormat::Number { style: Numbering::LowerAlpha, restart: None }),
        ("a) ", ListFormat::Number { style: Numbering::LowerAlphaParen, restart: None }),
        ("A. ", ListFormat::Number { style: Numbering::UpperAlpha, restart: None }),
        ("i. ", ListFormat::Number { style: Numbering::LowerRoman, restart: None }),
        ("I. ", ListFormat::Number { style: Numbering::UpperRoman, restart: None }),
        ("1 ) ", ListFormat::Number { style: Numbering::DecimalParen, restart: None }),
        ("(1) ", ListFormat::Number { style: Numbering::DecimalParen, restart: None }),
        ("• ", ListFormat::Bullet(Bullet::Disc)),
    ];
    table.into_iter().find(|(pfx, _)| text.starts_with(pfx)).map(|(pfx, f)| (f, pfx.len()))
}

/// If the text just before `offset` ends with a URL (http, https, ftp, mailto or `www.`), its
/// byte range — used to turn a typed URL into a link when the user types a space or presses
/// Enter (catalog `BEH.Text.HyperlinkAuto`).
pub fn detect_url_before(text: &str, offset: usize) -> Option<std::ops::Range<usize>> {
    let o = snap(text, offset);
    let head = text.get(..o)?;
    let start = head.rfind(|c: char| c.is_whitespace()).map(|i| i + 1).unwrap_or(0);
    let word = head.get(start..)?;
    let word_trim = word.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', '"', '\'']);
    let lower = word_trim.to_ascii_lowercase();
    let is_url = ["http://", "https://", "ftp://", "mailto:"].iter().any(|p| lower.starts_with(p) && lower.len() > p.len())
        || (lower.starts_with("www.") && lower.len() > 5 && lower[4..].contains('.'));
    is_url.then(|| start..start + word_trim.len())
}

/// The link target for a detected URL (adds `http://` to bare `www.` addresses).
pub fn url_target(url: &str) -> String {
    if url.to_ascii_lowercase().starts_with("www.") { format!("http://{url}") } else { url.to_string() }
}

/// List labels for an outline: for each paragraph, the bullet glyph or number label it shows
/// (empty when not in a list). Numbering counts consecutive numbered siblings at the same level,
/// resets when a shallower paragraph interrupts, and honours `restart`.
pub fn list_labels(paragraphs: &[Paragraph]) -> Vec<String> {
    // counters[level] = the last number used at that level in the current run.
    let mut counters: Vec<Option<u32>> = vec![None; notecraft_doc::text::MAX_LEVEL as usize + 1];
    let mut out = Vec::with_capacity(paragraphs.len());
    for p in paragraphs {
        let lvl = (p.level as usize).min(counters.len() - 1);
        // Deeper counters reset whenever a shallower-or-equal paragraph appears.
        for c in counters.iter_mut().skip(lvl + 1) {
            *c = None;
        }
        match p.list {
            Some(ListFormat::Bullet(b)) => {
                if let Some(c) = counters.get_mut(lvl) {
                    *c = None;
                }
                out.push(b.glyph().to_string());
            }
            Some(ListFormat::Number { style, restart }) => {
                let n = match (restart, counters.get(lvl).copied().flatten()) {
                    (Some(r), _) => r.max(1),
                    (None, Some(prev)) => prev.saturating_add(1),
                    (None, None) => 1,
                };
                if let Some(c) = counters.get_mut(lvl) {
                    *c = Some(n);
                }
                out.push(style.label(n));
            }
            None => {
                if let Some(c) = counters.get_mut(lvl) {
                    *c = None;
                }
                out.push(String::new());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use notecraft_doc::{ParaStyle, TagUse, builtin_tags};
    use proptest::prelude::*;

    fn para(s: &str) -> Paragraph {
        Paragraph::with_text(Id(1), s)
    }

    #[test]
    fn insert_and_delete_keep_runs_normal() {
        let mut p = para("hello world");
        let bold = CharFormat { bold: Some(true), ..Default::default() };
        let end = insert(&mut p, 5, ",", &bold);
        assert_eq!(end, 6);
        assert_eq!(p.text(), "hello, world");
        assert_eq!(p.runs.len(), 3);
        let removed = delete(&mut p, 5..6);
        assert_eq!(removed, ",");
        assert_eq!(p.runs.len(), 1, "equal neighbours merge again");
        // Hostile offsets are clamped.
        assert_eq!(insert(&mut p, 9999, "!", &CharFormat::default()), p.len());
        assert_eq!(delete(&mut p, 50..60), "", "a range past the end deletes nothing");
    }

    #[test]
    fn offsets_snap_to_graphemes() {
        // "e" + combining acute, then a family emoji (ZWJ sequence).
        let s = "e\u{301}👨‍👩‍👧x";
        let mut p = para(s);
        // Offset 1 is inside "é" (between e and the combining mark): snapped back to 0.
        assert_eq!(snap(s, 1), 0);
        assert_eq!(next_grapheme(s, 0), 3);
        let emoji_end = s.len() - 1;
        assert_eq!(next_grapheme(s, 3), emoji_end);
        assert_eq!(prev_grapheme(s, emoji_end), 3);
        // Deleting from inside the emoji removes nothing partial.
        delete(&mut p, 4..emoji_end);
        assert!(p.text().is_char_boundary(0));
        assert!(std::str::from_utf8(p.text().as_bytes()).is_ok());
    }

    #[test]
    fn word_motion() {
        // parity: KB.General.WordLeft, KB.General.WordRight
        let s = "one two, three";
        assert_eq!(word_right(s, 0), 4);
        assert_eq!(word_right(s, 4), 9);
        assert_eq!(word_right(s, 9), s.len());
        assert_eq!(word_left(s, s.len()), 9);
        assert_eq!(word_left(s, 9), 4);
        assert_eq!(word_left(s, 2), 0);
        assert_eq!(word_at(s, 5), 4..7);
        assert_eq!(line_start("ab\ncd", 4), 3);
        assert_eq!(line_end("ab\ncd", 1), 2);
    }

    #[test]
    fn split_carries_list_level_and_todo() {
        // parity: BEH.Outline.EnterNewParagraph
        let mut p = para("buy milk");
        p.level = 2;
        p.list = Some(ListFormat::Number { style: Numbering::Decimal, restart: Some(5) });
        let todo = builtin_tags().into_iter().next().unwrap();
        let mut t = TagUse::of(&todo);
        t.checked = true;
        p.tags.push(t);
        let star = builtin_tags().into_iter().nth(1).unwrap();
        p.tags.push(TagUse::of(&star));
        let q = split(&mut p, 3, Id(2));
        assert_eq!(p.text(), "buy");
        assert_eq!(q.text(), " milk");
        assert_eq!(q.level, 2);
        assert_eq!(q.list, Some(ListFormat::Number { style: Numbering::Decimal, restart: None }));
        assert_eq!(q.tags.len(), 1, "only the to-do tag carries over");
        assert!(!q.tags[0].checked);
        // A heading split at its end gives a Normal paragraph.
        let mut h = para("Title");
        h.style = ParaStyle::Heading1;
        assert_eq!(split(&mut h, 5, Id(3)).style, ParaStyle::Normal);
    }

    #[test]
    fn merge_returns_join_point() {
        let mut a = para("ab");
        let b = para("cd");
        assert_eq!(merge(&mut a, b), 2);
        assert_eq!(a.text(), "abcd");
    }

    #[test]
    fn toggling_formats_over_ranges() {
        // parity: BEH.Text.FormatToggles
        let mut p = para("abcdef");
        apply_format(&mut p, 2..4, &|f| f.bold = Some(true));
        assert!(all_effective(&p, 2..4, &|f| f.bold == Some(true)));
        assert!(!all_effective(&p, 0..4, &|f| f.bold == Some(true)));
        assert_eq!(slice(&p, 1..5).len(), 3);
        apply_format(&mut p, 0..6, &|f| f.bold = None);
        assert_eq!(p.runs.len(), 1);
        // Empty range tests the caret position.
        assert!(!all_effective(&p, 3..3, &|f| f.bold == Some(true)));
    }

    #[test]
    fn font_size_ladder() {
        // parity: BEH.Text.FontSizeSteps
        assert_eq!(grow_size(11.0), 12.0);
        assert_eq!(grow_size(10.0), 10.5);
        assert_eq!(grow_size(72.0), 82.0);
        assert_eq!(shrink_size(11.0), 10.5);
        assert_eq!(shrink_size(8.0), MIN_FONT_SIZE);
        assert_eq!(shrink_size(80.0), 72.0);
        assert_eq!(shrink_size(100.0), 90.0);
        assert_eq!(grow_size(f32::NAN), 12.0);
        assert!(grow_size(MAX_FONT_SIZE) <= MAX_FONT_SIZE);
    }

    #[test]
    fn list_prefixes_and_urls() {
        // parity: BEH.Outline.AutoBullet, BEH.Outline.AutoNumber, BEH.Text.HyperlinkAuto
        assert_eq!(detect_list_prefix("* x").map(|(_, n)| n), Some(2));
        assert!(matches!(detect_list_prefix("1. "), Some((ListFormat::Number { style: Numbering::Decimal, .. }, 3))));
        assert!(matches!(detect_list_prefix("a) "), Some((ListFormat::Number { style: Numbering::LowerAlphaParen, .. }, 3))));
        assert_eq!(detect_list_prefix("2. "), None);
        assert_eq!(detect_list_prefix("*x"), None);
        let s = "see https://example.org/a.";
        assert_eq!(detect_url_before(s, s.len()), Some(4..s.len() - 1));
        assert_eq!(detect_url_before("go www.example.com", 18), Some(3..18));
        assert_eq!(detect_url_before("http:// x", 7), None);
        assert_eq!(url_target("www.a.com"), "http://www.a.com");
    }

    #[test]
    fn numbering_labels() {
        let mk = |lvl: u8, list: Option<ListFormat>| {
            let mut p = para("x");
            p.level = lvl;
            p.list = list;
            p
        };
        let num = Some(ListFormat::Number { style: Numbering::Decimal, restart: None });
        let sub = Some(ListFormat::Number { style: Numbering::LowerAlpha, restart: None });
        let ps = vec![mk(0, num), mk(1, sub), mk(1, sub), mk(0, num), mk(1, sub), mk(0, None), mk(0, num)];
        assert_eq!(list_labels(&ps), vec!["1.", "a.", "b.", "2.", "a.", "", "1."]);
    }

    proptest! {
        // Property: the cursor `snap` agrees with the scan from the start at every offset,
        // across combining marks, ZWJ emoji sequences, flags (regional indicators) and CRLF.
        #[test]
        fn snap_matches_the_scan(text in "[a\u{301}\u{200d}👍👩🇮🇹\r\né ]{0,12}") {
            for o in 0..=text.len() + 2 {
                prop_assert_eq!(snap(&text, o), snap_scan(&text, o), "offset {} of {:?}", o, text);
            }
        }

        // Property: any sequence of inserts/deletes at arbitrary offsets keeps valid UTF-8, no empty runs, and text consistent with a plain-string model.
        #[test]
        fn edits_never_break_invariants(ops in proptest::collection::vec((any::<bool>(), 0usize..40, "[a-zé👍\u{301} ]{0,4}", 0usize..8), 0..30)) {
            let mut p = para("start");
            for (is_insert, at, s, len) in ops {
                if is_insert {
                    insert(&mut p, at, &s, &CharFormat { italic: Some(at % 2 == 0), ..Default::default() });
                } else {
                    delete(&mut p, at..at + len);
                }
                prop_assert!(p.runs.iter().all(|r| !r.text.is_empty()));
                for w in p.runs.windows(2) {
                    prop_assert!(w[0].fmt != w[1].fmt);
                }
            }
        }
        #[test]
        fn word_motion_stays_in_bounds(s in "\\PC{0,30}", at in 0usize..60) {
            let l = word_left(&s, at);
            let r = word_right(&s, at);
            prop_assert!(l <= s.len() && r <= s.len());
            prop_assert!(s.is_char_boundary(l) && s.is_char_boundary(r));
        }
    }
}
