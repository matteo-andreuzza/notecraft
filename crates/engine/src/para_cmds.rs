//! Paragraph formatting (Home › Basic Text and Styles): bullets, numbering, indent, alignment,
//! spacing, direction and paragraph styles.

use notecraft_doc::{Align, Bullet, ListFormat, Numbering, ParaStyle};
use serde_json::Value;

use crate::ops::{self, need_text, no_page};
use crate::selection::{Block, PageSel};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn need_list(s: &Session, numbered: bool) -> Option<&'static str> {
    if let Some(why) = need_text(s) {
        return Some(why);
    }
    let Some(page) = s.page() else { return Some("No page is open") };
    let ok = ops::touched(s).iter().any(|(b, r)| {
        Session::block_paras(page, *b).is_some_and(|ps| {
            ps.get(r.clone()).is_some_and(|x| {
                x.iter().any(|p| matches!((p.list, numbered), (Some(ListFormat::Number { .. }), true) | (Some(ListFormat::Bullet(_)), false)))
            })
        })
    });
    if ok { None } else { Some("The paragraph is not in that kind of list") }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    let mut v = vec![
        // R.Home.BasicText.BulletsGallery — Ctrl+. toggles the default bullet; the gallery
        // passes a glyph.
        CommandSpec::new("para.bullets", "R.Home.BasicText.BulletsGallery", "Bullets", "Home/Basic Text/Bullets", bullets)
            .key("Mod+.")
            .params(r#"{"bullet"?: "disc"|"circle"|"square"|"dash"|"arrow"|"diamond"|"check"|"star"}"#)
            .when(need_text),
        CommandSpec::new("para.numbering", "R.Home.BasicText.NumberingGallery", "Numbering", "Home/Basic Text/Numbering", numbering)
            .key("Mod+/")
            .params(r#"{"style"?: "decimal"|"decimalParen"|"lowerAlpha"|"upperAlpha"|"lowerRoman"|"upperRoman"|"lowerAlphaParen"}"#)
            .when(need_text),
        CommandSpec::new("para.bulletRemove", "CM.Bullet.BulletRemove", "Remove Bullet", "Context/Bullet/Remove Bullet", remove_list)
            .when(|s| need_list(s, false)),
        CommandSpec::new("para.numberingRemove", "CM.Number.NumberListRemove", "Remove Numbering", "Context/Number/Remove Numbering", remove_list)
            .when(|s| need_list(s, true)),
        CommandSpec::new("para.restartNumbering", "CM.Number.NumberListRestart", "Restart Numbering", "Context/Number/Restart Numbering", restart)
            .params(r#"{"at"?: int}"#)
            .when(|s| need_list(s, true)),
        CommandSpec::new("para.continueNumbering", "CM.Number.ContinueNumberList", "Continue Numbering", "Context/Number/Continue Numbering", cont)
            .when(|s| need_list(s, true)),
        // R.Home.BasicText.IndentClassic — Alt+Shift+Right always indents; Tab indents at the
        // start of a paragraph (or with several paragraphs selected) and otherwise inserts a tab
        // (catalog BEH.Outline.TabIndent; Tab after text makes a table in M2).
        CommandSpec::new(
            "para.indent",
            "R.Home.BasicText.IndentClassic",
            "Increase Indent Position",
            "Home/Basic Text/Increase Indent Position",
            indent,
        )
        .key("Alt+Shift+Right / Tab")
        .params(r#"{"key"?: "tab"}"#)
        .when(need_text),
        CommandSpec::new(
            "para.outdent",
            "R.Home.BasicText.OutdentClassic",
            "Decrease Indent Position",
            "Home/Basic Text/Decrease Indent Position",
            outdent,
        )
        .key("Alt+Shift+Left / Shift+Tab")
        .when(need_text),
        CommandSpec::new("para.alignLeft", "R.Home.BasicText.AlignLeft", "Align Left", "Home/Basic Text/Paragraph Alignment/Align Left", |s, _| {
            align(s, Align::Left)
        })
        .key("Mod+L")
        .when(need_text),
        CommandSpec::new("para.alignCenter", "R.Home.BasicText.AlignCenter", "Center", "Home/Basic Text/Paragraph Alignment/Center", |s, _| {
            align(s, Align::Center)
        })
        .when(need_text),
        CommandSpec::new(
            "para.alignRight",
            "R.Home.BasicText.AlignRight",
            "Align Right",
            "Home/Basic Text/Paragraph Alignment/Align Right",
            |s, _| align(s, Align::Right),
        )
        .key("Mod+R")
        .when(need_text),
        // R.Home.BasicText.ParagraphSpacingOptions — the dialog's values.
        CommandSpec::new(
            "para.spacingDialog",
            "R.Home.BasicText.ParagraphSpacingOptions",
            "Paragraph Spacing Options",
            "Home/Basic Text/Paragraph Alignment/Paragraph Spacing Options",
            spacing,
        )
        .params(r#"{"before"?: pt, "after"?: pt, "line"?: pt (at least; 0 = single)}"#)
        .when(need_text),
        CommandSpec::new(
            "para.ltr",
            "R.Home.BasicText.TextDirectionLeftToRight",
            "Left-to-Right",
            "Home/Basic Text/Paragraph Alignment/Left-to-Right",
            |s, _| direction(s, false),
        )
        .key("Mod+LeftShift")
        .when(need_text),
        CommandSpec::new(
            "para.rtl",
            "R.Home.BasicText.TextDirectionRightToLeft",
            "Right-to-Left",
            "Home/Basic Text/Paragraph Alignment/Right-to-Left",
            |s, _| direction(s, true),
        )
        .key("Mod+RightShift")
        .when(need_text),
        // R.Home.Styles.QuickStylesGallery
        CommandSpec::new("style.apply", "R.Home.Styles.QuickStylesGallery", "Styles", "Home/Styles", style_apply)
            .params(r#"{"style": "Heading 1".."Heading 6"|"Page Title"|"Citation"|"Quote"|"Code"|"Normal"}"#)
            .when(need_text),
    ];
    // R.Home.Styles.<Style> — one command per gallery item (Ctrl+Alt+1–6 for the headings).
    v.extend([
        CommandSpec::new("style.heading1", "R.Home.Styles.Heading1", "Heading 1", "Home/Styles/Heading 1", |s, _| set_style(s, ParaStyle::Heading1))
            .key("Mod+Alt+1")
            .when(need_text),
        CommandSpec::new("style.heading2", "R.Home.Styles.Heading2", "Heading 2", "Home/Styles/Heading 2", |s, _| set_style(s, ParaStyle::Heading2))
            .key("Mod+Alt+2")
            .when(need_text),
        CommandSpec::new("style.heading3", "R.Home.Styles.Heading3", "Heading 3", "Home/Styles/Heading 3", |s, _| set_style(s, ParaStyle::Heading3))
            .key("Mod+Alt+3")
            .when(need_text),
        CommandSpec::new("style.heading4", "R.Home.Styles.Heading4", "Heading 4", "Home/Styles/Heading 4", |s, _| set_style(s, ParaStyle::Heading4))
            .key("Mod+Alt+4")
            .when(need_text),
        CommandSpec::new("style.heading5", "R.Home.Styles.Heading5", "Heading 5", "Home/Styles/Heading 5", |s, _| set_style(s, ParaStyle::Heading5))
            .key("Mod+Alt+5")
            .when(need_text),
        CommandSpec::new("style.heading6", "R.Home.Styles.Heading6", "Heading 6", "Home/Styles/Heading 6", |s, _| set_style(s, ParaStyle::Heading6))
            .key("Mod+Alt+6")
            .when(need_text),
        CommandSpec::new("style.pageTitle", "R.Home.Styles.PageTitle", "Page Title", "Home/Styles/Page Title", |s, _| {
            set_style(s, ParaStyle::PageTitle)
        })
        .when(need_text),
        CommandSpec::new("style.citation", "R.Home.Styles.Citation", "Citation", "Home/Styles/Citation", |s, _| set_style(s, ParaStyle::Citation))
            .when(need_text),
        CommandSpec::new("style.quote", "R.Home.Styles.Quote", "Quote", "Home/Styles/Quote", |s, _| set_style(s, ParaStyle::Quote)).when(need_text),
        CommandSpec::new("style.code", "R.Home.Styles.Code", "Code", "Home/Styles/Code", |s, _| set_style(s, ParaStyle::Code)).when(need_text),
        CommandSpec::new("style.normal", "R.Home.Styles.Normal", "Normal", "Home/Styles/Normal", |s, _| set_style(s, ParaStyle::Normal))
            .when(need_text),
    ]);
    v
}

/// Apply `f` to every touched paragraph of note containers (the title has no paragraph
/// formatting). Returns how many were changed.
fn each_para(s: &mut Session, f: &mut dyn FnMut(&mut notecraft_doc::Paragraph)) -> Result<usize, CmdError> {
    let touched = ops::touched(s);
    if touched.is_empty() {
        // No caret yet but a pending click: create the container so the format sticks.
        if s.pending_outline.is_some() {
            ops::ensure_caret(s)?;
            return each_para(s, f);
        }
        return Err(CmdError::Disabled("click in a note first".into()));
    }
    let page = s.page_mut().ok_or_else(no_page)?;
    let mut n = 0;
    for (block, range) in touched {
        if block == Block::Title {
            continue;
        }
        for i in range {
            if let Some(p) = Session::para_mut(page, block, i) {
                f(p);
                n += 1;
            }
        }
    }
    Ok(n)
}

fn parse_bullet(s: &str) -> Option<Bullet> {
    Some(match s {
        "disc" => Bullet::Disc,
        "circle" => Bullet::Circle,
        "square" => Bullet::Square,
        "dash" => Bullet::Dash,
        "arrow" => Bullet::Arrow,
        "diamond" => Bullet::Diamond,
        "check" => Bullet::Check,
        "star" => Bullet::Star,
        _ => return None,
    })
}

fn parse_numbering(s: &str) -> Option<Numbering> {
    Some(match s {
        "decimal" => Numbering::Decimal,
        "decimalParen" => Numbering::DecimalParen,
        "lowerAlpha" => Numbering::LowerAlpha,
        "upperAlpha" => Numbering::UpperAlpha,
        "lowerRoman" => Numbering::LowerRoman,
        "upperRoman" => Numbering::UpperRoman,
        "lowerAlphaParen" => Numbering::LowerAlphaParen,
        _ => return None,
    })
}

fn all_touched(s: &Session, pred: &dyn Fn(&notecraft_doc::Paragraph) -> bool) -> bool {
    let Some(page) = s.page() else { return false };
    let t = ops::touched(s);
    !t.is_empty() && t.iter().all(|(b, r)| Session::block_paras(page, *b).is_some_and(|ps| ps.get(r.clone()).is_some_and(|x| x.iter().all(pred))))
}

fn bullets(s: &mut Session, v: &Value) -> CmdResult {
    let chosen = match p::str(v, "bullet") {
        Some(b) => Some(parse_bullet(b).ok_or_else(|| CmdError::Params(format!("unknown bullet `{b}`")))?),
        None => None,
    };
    // Without an explicit glyph, Ctrl+. toggles: all bulleted → remove.
    let remove = chosen.is_none() && all_touched(s, &|p| matches!(p.list, Some(ListFormat::Bullet(_))));
    each_para(s, &mut |p| p.list = if remove { None } else { Some(ListFormat::Bullet(chosen.unwrap_or_else(|| Bullet::for_level(p.level)))) })?;
    Ok(Value::Null)
}

fn numbering(s: &mut Session, v: &Value) -> CmdResult {
    let chosen = match p::str(v, "style") {
        Some(n) => Some(parse_numbering(n).ok_or_else(|| CmdError::Params(format!("unknown numbering `{n}`")))?),
        None => None,
    };
    let remove = chosen.is_none() && all_touched(s, &|p| matches!(p.list, Some(ListFormat::Number { .. })));
    each_para(s, &mut |p| {
        p.list =
            if remove { None } else { Some(ListFormat::Number { style: chosen.unwrap_or_else(|| Numbering::for_level(p.level)), restart: None }) }
    })?;
    Ok(Value::Null)
}

fn remove_list(s: &mut Session, _: &Value) -> CmdResult {
    each_para(s, &mut |p| p.list = None)?;
    Ok(Value::Null)
}

fn restart(s: &mut Session, v: &Value) -> CmdResult {
    let at = p::usize(v, "at").unwrap_or(1).clamp(1, 1_000_000) as u32;
    each_para(s, &mut |p| {
        if let Some(ListFormat::Number { style, .. }) = p.list {
            p.list = Some(ListFormat::Number { style, restart: Some(at) });
        }
    })?;
    Ok(Value::Null)
}

fn cont(s: &mut Session, _: &Value) -> CmdResult {
    each_para(s, &mut |p| {
        if let Some(ListFormat::Number { style, .. }) = p.list {
            p.list = Some(ListFormat::Number { style, restart: None });
        }
    })?;
    Ok(Value::Null)
}

/// Indent or outdent the touched paragraphs together with their children (catalog
/// `BEH.Outline.TabIndent`). The first paragraph of a container may not be deeper than its
/// predecessor + 1.
pub(crate) fn shift_level(s: &mut Session, delta: i8) -> CmdResult {
    let touched = ops::touched(s);
    let page = s.page_mut().ok_or_else(no_page)?;
    for (block, range) in touched {
        let Block::Outline(id) = block else { continue };
        let Some(o) = page.outline_mut(id) else { continue };
        // Extend to the last touched paragraph's descendants.
        let end = o.subtree(range.end.saturating_sub(1)).end.max(range.end);
        let start = range.start;
        if delta > 0 {
            let prev_level = start.checked_sub(1).and_then(|j| o.paragraphs.get(j)).map(|p| p.level);
            let first_level = o.paragraphs.get(start).map(|p| p.level).unwrap_or(0);
            // Can't indent past previous + 1, nor beyond the maximum.
            let can = match prev_level {
                Some(pl) => first_level <= pl,
                None => first_level < notecraft_doc::text::MAX_LEVEL,
            };
            if !can {
                continue;
            }
            let max = o.paragraphs.get(start..end).map(|x| x.iter().map(|p| p.level).max().unwrap_or(0)).unwrap_or(0);
            if max >= notecraft_doc::text::MAX_LEVEL {
                continue;
            }
            for p in o.paragraphs.get_mut(start..end).into_iter().flatten() {
                p.level += 1;
            }
        } else {
            let min = o.paragraphs.get(start..range.end).map(|x| x.iter().map(|p| p.level).min().unwrap_or(0)).unwrap_or(0);
            if min == 0 {
                continue;
            }
            for p in o.paragraphs.get_mut(start..end).into_iter().flatten() {
                p.level = p.level.saturating_sub(1);
            }
        }
        o.normalize();
        // List glyphs follow the new level when they were the level default.
        for p in o.paragraphs.get_mut(start..end).into_iter().flatten() {
            if let Some(ListFormat::Bullet(b)) = p.list
                && Bullet::ALL.iter().take(3).any(|x| *x == b)
            {
                p.list = Some(ListFormat::Bullet(Bullet::for_level(p.level)));
            }
            if let Some(ListFormat::Number { style, restart }) = p.list
                && matches!(style, Numbering::Decimal | Numbering::LowerAlpha | Numbering::LowerRoman)
            {
                p.list = Some(ListFormat::Number { style: Numbering::for_level(p.level), restart });
            }
        }
    }
    Ok(Value::Null)
}

fn indent(s: &mut Session, v: &Value) -> CmdResult {
    // Tab in the middle of a single paragraph inserts a tab character.
    if p::str(v, "key") == Some("tab")
        && let (PageSel::Text(_), Some(r)) = (&s.sel.on_page, s.text_range())
        && r.anchor.para == r.head.para
        && r.ordered().0.offset > 0
    {
        let caret = ops::ensure_caret(s)?;
        if caret.block == Block::Title {
            return Ok(Value::Null);
        }
        let fmt = ops::typing_fmt(s, caret);
        let pos = ops::insert_at(s, caret, "\t", &fmt)?;
        s.set_caret(pos);
        return Ok(Value::Null);
    }
    if ops::touched(s).is_empty() && s.pending_outline.is_some() {
        ops::ensure_caret(s)?;
    }
    shift_level(s, 1)
}

fn outdent(s: &mut Session, _: &Value) -> CmdResult {
    shift_level(s, -1)
}

fn align(s: &mut Session, a: Align) -> CmdResult {
    each_para(s, &mut |p| p.align = a)?;
    Ok(Value::Null)
}

fn spacing(s: &mut Session, v: &Value) -> CmdResult {
    let before = p::f32(v, "before");
    let after = p::f32(v, "after");
    let line = p::f32(v, "line");
    if before.is_none() && after.is_none() && line.is_none() {
        return Err(CmdError::Params("give at least one of `before`, `after`, `line` (points)".into()));
    }
    let ok = |x: f32| (0.0..=1000.0).contains(&x);
    if [before, after, line].into_iter().flatten().any(|x| !ok(x)) {
        return Err(CmdError::Params("spacing must be between 0 and 1000 points".into()));
    }
    each_para(s, &mut |p| {
        if let Some(b) = before {
            p.space_before = b;
        }
        if let Some(a) = after {
            p.space_after = a;
        }
        if let Some(l) = line {
            p.line_spacing = l;
        }
    })?;
    Ok(Value::Null)
}

fn direction(s: &mut Session, rtl: bool) -> CmdResult {
    each_para(s, &mut |p| {
        p.rtl = rtl;
        p.align = if rtl { Align::Right } else { Align::Left };
    })?;
    Ok(Value::Null)
}

/// Apply a paragraph style. Character overrides other than links and highlights are cleared so
/// the paragraph takes the style's look (catalog `BEH.Text.Styles`, unverified).
fn set_style(s: &mut Session, style: ParaStyle) -> CmdResult {
    each_para(s, &mut |p| {
        p.style = style;
        for r in &mut p.runs {
            r.fmt = notecraft_doc::CharFormat { link: r.fmt.link.take(), highlight: r.fmt.highlight, ..Default::default() };
        }
        p.normalize();
    })?;
    s.pending_fmt = None;
    Ok(Value::Null)
}

fn style_apply(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "style")?;
    let style = ParaStyle::from_name(name).ok_or_else(|| CmdError::Params(format!("unknown style `{name}`")))?;
    set_style(s, style)
}
