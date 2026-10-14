//! Character formatting (Home › Basic Text): bold, italic, underline, strikethrough,
//! sub/superscript, font, size, colour, highlight, clear formatting.
//!
//! Rules (catalog `BEH.Text.FormatToggles`): with a selection, a toggle turns the property on
//! for the whole selection unless every character already has it, in which case it turns it
//! off. With a caret inside a word, the word is formatted (word-processor convention,
//! unverified for the reference); with a caret elsewhere, the format applies to what is typed
//! next.

use notecraft_doc::color::{DEFAULT_HIGHLIGHT, Rgba};
use notecraft_doc::{CharFormat, Script};
use notecraft_text as t;
use serde_json::Value;

use crate::ops::{self, need_text, no_page};
use crate::selection::{Block, PageSel};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("format.bold", "R.Home.BasicText.Bold", "Bold", "Home/Basic Text/Bold", bold).key("Mod+B").when(need_text),
        CommandSpec::new("format.italic", "R.Home.BasicText.Italic", "Italic", "Home/Basic Text/Italic", italic).key("Mod+I").when(need_text),
        CommandSpec::new("format.underline", "R.Home.BasicText.Underline", "Underline", "Home/Basic Text/Underline", underline)
            .key("Mod+U")
            .when(need_text),
        CommandSpec::new("format.strikethrough", "R.Home.BasicText.Strikethrough", "Strikethrough", "Home/Basic Text/Strikethrough", strike)
            .key("Alt+-")
            .when(need_text),
        // R.Home.BasicText.Subscript — Ctrl+= (see the catalog note on the documented key).
        CommandSpec::new(
            "format.subscript",
            "R.Home.BasicText.Subscript",
            "Subscript",
            "Home/Basic Text/Subscript / Superscript/Subscript",
            subscript,
        )
        .key("Mod+=")
        .when(need_text),
        CommandSpec::new(
            "format.superscript",
            "R.Home.BasicText.Superscript",
            "Superscript",
            "Home/Basic Text/Subscript / Superscript/Superscript",
            superscript,
        )
        .key("Mod+Shift+Plus")
        .when(need_text),
        CommandSpec::new("format.font", "R.Home.BasicText.Font", "Font", "Home/Basic Text/Font", font).params(r#"{"font": string}"#).when(need_text),
        CommandSpec::new("format.size", "R.Home.BasicText.FontSize", "Font Size", "Home/Basic Text/Font Size", size)
            .params(r#"{"size": number (1-1638)}"#)
            .when(need_text),
        // QAT.NR.FontSizeIncrease1Point / Decrease — Ctrl+Shift+> / <, stepping the size ladder.
        CommandSpec::new(
            "format.growFont",
            "QAT.NR.FontSizeIncrease1Point",
            "Increase Font Size",
            "Quick Access Toolbar/Commands Not in the Ribbon/Increase Font Size",
            grow,
        )
        .key("Mod+Shift+>")
        .when(need_text),
        CommandSpec::new(
            "format.shrinkFont",
            "QAT.NR.FontSizeDecrease1Point",
            "Decrease Font Size",
            "Quick Access Toolbar/Commands Not in the Ribbon/Decrease Font Size",
            shrink,
        )
        .key("Mod+Shift+<")
        .when(need_text),
        CommandSpec::new("format.color", "R.Home.BasicText.FontColorPicker", "Font Color", "Home/Basic Text/Font Color", color)
            .params(r##"{"color": "#rrggbb" | null (Automatic)}"##)
            .when(need_text),
        // R.Home.BasicText.TextHighlightColorPicker — Ctrl+Alt+H toggles the last colour
        // (yellow by default).
        CommandSpec::new(
            "format.highlight",
            "R.Home.BasicText.TextHighlightColorPicker",
            "Text Highlight Color",
            "Home/Basic Text/Text Highlight Color",
            highlight,
        )
        .key("Mod+Alt+H")
        .params(r##"{"color"?: "#rrggbb"}"##)
        .when(need_text),
        CommandSpec::new(
            "format.highlightNone",
            "R.Home.BasicText.HighlightColorNone",
            "No Color",
            "Home/Basic Text/Text Highlight Color/No Color (highlight)",
            highlight_none,
        )
        .when(need_text),
        // R.Home.BasicText.ClearFormatting — Ctrl+Shift+N: Normal style, no direct formatting
        // (catalog BEH.Text.ClearFormatting).
        CommandSpec::new("format.clear", "R.Home.BasicText.ClearFormatting", "Clear All Formatting", "Home/Basic Text/Clear All Formatting", clear)
            .key("Mod+Shift+N")
            .when(need_text),
    ]
}

/// The ranges to format: the selection's segments, or the word around a caret inside a word.
/// Empty when the caret is not in a word (the format then applies to typed text).
fn targets(s: &Session) -> Vec<(Block, usize, std::ops::Range<usize>)> {
    let segs = ops::segments(s);
    if segs.iter().any(|(_, _, r)| !r.is_empty()) {
        return segs.into_iter().filter(|(_, _, r)| !r.is_empty()).collect();
    }
    if let (PageSel::Text(_), Some(r)) = (&s.sel.on_page, s.text_range())
        && r.is_caret()
        && let Some(page) = s.page()
        && let Some(para) = Session::block_paras(page, r.head.block).and_then(|ps| ps.get(r.head.para))
    {
        let text = para.text();
        let w = t::word_at(&text, r.head.offset);
        let in_word = w.start < r.head.offset && r.head.offset < w.end && text.get(w.clone()).is_some_and(|x| x.chars().any(char::is_alphanumeric));
        if in_word {
            return vec![(r.head.block, r.head.para, w)];
        }
    }
    Vec::new()
}

/// Apply `set` to the targets, or to the pending format at the caret.
fn apply(s: &mut Session, set: &dyn Fn(&mut CharFormat)) -> CmdResult {
    let tg = targets(s);
    if tg.is_empty() {
        let caret = s.text_range().map(|r| r.head);
        let mut f = match caret {
            Some(c) => ops::typing_fmt(s, c),
            None => s.pending_fmt.clone().unwrap_or_default(),
        };
        set(&mut f);
        s.pending_fmt = Some(f.sanitized());
        return Ok(Value::Null);
    }
    let page = s.page_mut().ok_or_else(no_page)?;
    for (block, i, r) in &tg {
        if let Some(p) = Session::para_mut(page, *block, *i) {
            t::apply_format(p, r.clone(), set);
        }
    }
    if let Some((b, i, _)) = tg.first() {
        ops::touch(s, *b, *i);
    }
    Ok(Value::Null)
}

/// Whether every target character's effective format satisfies `pred` (for toggles).
fn all_have(s: &Session, pred: &dyn Fn(&CharFormat) -> bool) -> bool {
    let tg = targets(s);
    let Some(page) = s.page() else { return false };
    if tg.is_empty() {
        let base = s
            .text_range()
            .and_then(|r| Session::block_paras(page, r.head.block).and_then(|ps| ps.get(r.head.para)).map(|p| p.style.def().char))
            .unwrap_or_default();
        let f = match s.text_range() {
            Some(r) => ops::typing_fmt(s, r.head).over(&base),
            None => base,
        };
        return pred(&f);
    }
    tg.iter().all(|(b, i, r)| Session::block_paras(page, *b).and_then(|ps| ps.get(*i)).is_some_and(|p| t::all_effective(p, r.clone(), pred)))
}

fn toggle(s: &mut Session, get: fn(&CharFormat) -> Option<bool>, set: fn(&mut CharFormat, bool)) -> CmdResult {
    let on = !all_have(s, &|f| get(f) == Some(true));
    apply(s, &|f| set(f, on))
}

fn bold(s: &mut Session, _: &Value) -> CmdResult {
    toggle(s, |f| f.bold, |f, v| f.bold = Some(v))
}

fn italic(s: &mut Session, _: &Value) -> CmdResult {
    toggle(s, |f| f.italic, |f, v| f.italic = Some(v))
}

fn underline(s: &mut Session, _: &Value) -> CmdResult {
    toggle(s, |f| f.underline, |f, v| f.underline = Some(v))
}

fn strike(s: &mut Session, _: &Value) -> CmdResult {
    toggle(s, |f| f.strike, |f, v| f.strike = Some(v))
}

fn script(s: &mut Session, want: Script) -> CmdResult {
    let on = !all_have(s, &|f| f.script == Some(want));
    apply(s, &|f| f.script = Some(if on { want } else { Script::Normal }))
}

fn subscript(s: &mut Session, _: &Value) -> CmdResult {
    script(s, Script::Sub)
}

fn superscript(s: &mut Session, _: &Value) -> CmdResult {
    script(s, Script::Super)
}

fn font(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "font")?.trim().to_string();
    if name.is_empty() || name.chars().count() > 128 {
        return Err(CmdError::Params("`font` must be 1–128 characters".into()));
    }
    apply(s, &|f| f.font = Some(name.clone()))
}

fn size(s: &mut Session, v: &Value) -> CmdResult {
    let sz = p::req_f32(v, "size")?;
    if !(notecraft_doc::text::MIN_FONT_SIZE..=notecraft_doc::text::MAX_FONT_SIZE).contains(&sz) {
        return Err(CmdError::Params("`size` must be between 1 and 1638".into()));
    }
    // The reference accepts half-point sizes; round to the nearest half point.
    let sz = (sz * 2.0).round() / 2.0;
    apply(s, &|f| f.size = Some(sz))
}

fn step_size(s: &mut Session, grow: bool) -> CmdResult {
    // Each target run steps from its own effective size.
    let tg = targets(s);
    if tg.is_empty() {
        let cur = s.text_range().and_then(|r| {
            s.page()
                .and_then(|pg| Session::block_paras(pg, r.head.block))
                .and_then(|ps| ps.get(r.head.para))
                .map(|p| ops::typing_fmt(s, r.head).over(&p.style.def().char).size)
        });
        let base = cur.flatten().unwrap_or(s.prefs.default_size);
        let next = if grow { t::grow_size(base) } else { t::shrink_size(base) };
        return apply(s, &|f| f.size = Some(next));
    }
    let page = s.page_mut().ok_or_else(no_page)?;
    for (block, i, r) in tg {
        if let Some(p) = Session::para_mut(page, block, i) {
            let base = p.style.def().char;
            // Split at run boundaries first so each run steps from its own size.
            let runs = t::slice(p, r.clone());
            let mut off = r.start;
            for run in runs {
                let cur = run.fmt.over(&base).size.unwrap_or(notecraft_doc::text::DEFAULT_SIZE);
                let next = if grow { t::grow_size(cur) } else { t::shrink_size(cur) };
                let end = off + run.text.len();
                t::apply_format(p, off..end, &|f| f.size = Some(next));
                off = end;
            }
        }
    }
    Ok(Value::Null)
}

fn grow(s: &mut Session, _: &Value) -> CmdResult {
    step_size(s, true)
}

fn shrink(s: &mut Session, _: &Value) -> CmdResult {
    step_size(s, false)
}

fn parse_color(v: &Value, k: &str) -> Result<Option<Rgba>, CmdError> {
    match v.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(c)) => Rgba::from_hex(c).map(Some).ok_or_else(|| CmdError::Params(format!("`{k}` must be #rrggbb"))),
        Some(_) => Err(CmdError::Params(format!("`{k}` must be a colour string or null"))),
    }
}

fn color(s: &mut Session, v: &Value) -> CmdResult {
    let c = parse_color(v, "color")?;
    apply(s, &|f| f.color = c)
}

fn highlight(s: &mut Session, v: &Value) -> CmdResult {
    let c = parse_color(v, "color")?;
    match c {
        Some(c) => apply(s, &|f| f.highlight = Some(c)),
        None => {
            // Toggle with the default colour.
            let on = !all_have(s, &|f| f.highlight.is_some());
            apply(s, &|f| f.highlight = if on { Some(DEFAULT_HIGHLIGHT) } else { None })
        }
    }
}

fn highlight_none(s: &mut Session, _: &Value) -> CmdResult {
    apply(s, &|f| f.highlight = None)
}

fn clear(s: &mut Session, _: &Value) -> CmdResult {
    // Paragraph-level: Normal style; character-level: drop every override but links.
    for (block, range) in ops::touched(s) {
        let page = s.page_mut().ok_or_else(no_page)?;
        for i in range {
            if let Some(p) = Session::para_mut(page, block, i)
                && block != Block::Title
            {
                p.style = notecraft_doc::ParaStyle::Normal;
            }
        }
    }
    let tg = ops::segments(s);
    let page = s.page_mut().ok_or_else(no_page)?;
    for (block, i, r) in tg {
        if let Some(p) = Session::para_mut(page, block, i) {
            t::apply_format(p, r, &|f| {
                let link = f.link.take();
                *f = CharFormat { link, ..Default::default() };
            });
        }
    }
    s.pending_fmt = None;
    Ok(Value::Null)
}
