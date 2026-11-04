//! Equations (decision D11, `docs/plan.md` §4.16): an equation is a paragraph whose text is its
//! UnicodeMath linear format (`Paragraph::math`), shown built up by the canvas (crate `math`).
//! Insert › Equation, the Equation tab's Professional/Linear, its symbols and structures, and
//! the built-in equations of the Equation gallery.

use notecraft_doc::{CharFormat, MathView, ParaStyle};
use serde_json::{Value, json};

use crate::ops::{self, need_page, no_page};
use crate::selection::{Block, Pos};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// The equations of Insert › Equation's gallery: name and linear format (common results; the
/// set is NoteCraft's, unverified against the reference's).
pub const EQUATIONS: &[(&str, &str)] = &[
    ("Area of a circle", "A=πr^2"),
    ("Pythagorean theorem", "a^2+b^2=c^2"),
    ("Quadratic formula", "x=(−b±√(b^2−4ac))/2a"),
    ("Taylor expansion", "e^x=1+x/1!+x^2/2!+x^3/3!+⋯"),
    ("Sum of the first n numbers", "∑_(k=1)^n k=(n(n+1))/2"),
    ("Trigonometric identity", "sin^2 θ+cos^2 θ=1"),
];

/// The Equation tab's Symbols gallery.
pub const SYMBOLS: &str = "±∞=≠∼×÷!∝<≪>≫≤≥∓≅≈≡∀∂√∛∜∪∩∅%°∆∇∃∄∈∋←↑→↓↔∴+−¬αβγδεϵθϑμπρστφωΓΔΘΛΞΠΣΦΨΩ∗∙⋮⋯⋰⋱ℵℏ";

/// The Equation tab's Structures galleries: command, catalog id, label, ribbon path, and their
/// items (label, linear format; `⬚` marks a slot to fill).
pub const STRUCTURES: &[(&str, &str, &str, &str, &[(&str, &str)])] = &[
    (
        "equation.structureFraction",
        "CT.Equation.Structures.EquationFractionGallery",
        "Fraction",
        "Equation/Structures/Fraction",
        &[("Stacked fraction", "⬚/⬚"), ("Differential", "dy/dx"), ("Partial differential", "∂y/∂x"), ("One half", "1/2")],
    ),
    (
        "equation.structureScript",
        "CT.Equation.Structures.EquationScriptGallery",
        "Script",
        "Equation/Structures/Script",
        &[("Superscript", "⬚^⬚"), ("Subscript", "⬚_⬚"), ("Subscript-superscript", "⬚_⬚^⬚"), ("x squared", "x^2"), ("e to the power", "e^(−iωt)")],
    ),
    (
        "equation.structureRadical",
        "CT.Equation.Structures.EquationRadicalGallery",
        "Radical",
        "Equation/Structures/Radical",
        &[("Square root", "√⬚"), ("Radical with degree", "√(⬚&⬚)"), ("Cube root", "∛⬚"), ("Hypotenuse", "√(a^2+b^2)")],
    ),
    (
        "equation.structureIntegral",
        "CT.Equation.Structures.EquationIntegralGallery",
        "Integral",
        "Equation/Structures/Integral",
        &[("Integral", "∫⬚"), ("Integral with limits", "∫_⬚^⬚ ⬚"), ("Double integral", "∬⬚"), ("Contour integral", "∮⬚")],
    ),
    (
        "equation.structureLargeOperator",
        "CT.Equation.Structures.EquationLargeOperatorGallery",
        "Large Operator",
        "Equation/Structures/Large Operator",
        &[("Summation", "∑_⬚^⬚ ⬚"), ("Product", "∏_⬚^⬚ ⬚"), ("Union", "⋃_⬚^⬚ ⬚"), ("Intersection", "⋂_⬚^⬚ ⬚")],
    ),
    (
        "equation.structureDelimiter",
        "CT.Equation.Structures.EquationDelimiterGallery",
        "Bracket",
        "Equation/Structures/Bracket",
        &[("Parentheses", "(⬚)"), ("Square brackets", "[⬚]"), ("Braces", "{⬚}"), ("Absolute value", "|⬚|"), ("Norm", "‖⬚‖")],
    ),
    (
        "equation.structureFunction",
        "CT.Equation.Structures.EquationFunctionGallery",
        "Function",
        "Equation/Structures/Function",
        &[("Sine", "sin⬚"), ("Cosine", "cos⬚"), ("Tangent", "tan⬚"), ("Natural logarithm", "ln⬚"), ("Exponential", "exp⬚")],
    ),
    (
        "equation.structureAccent",
        "CT.Equation.Structures.EquationAccentGallery",
        "Accent",
        "Equation/Structures/Accent",
        &[("Hat", "⬚\u{302}"), ("Tilde", "⬚\u{303}"), ("Bar", "⬚\u{305}"), ("Dot", "⬚\u{307}"), ("Vector", "⬚\u{20D7}")],
    ),
    (
        "equation.structureLimit",
        "CT.Equation.Structures.EquationLimitGallery",
        "Limit and Log",
        "Equation/Structures/Limit and Log",
        &[("Limit", "lim_(⬚→⬚) ⬚"), ("Logarithm with base", "log_⬚ ⬚"), ("Maximum", "max_⬚ ⬚"), ("Minimum", "min_⬚ ⬚")],
    ),
    (
        "equation.structureOperator",
        "CT.Equation.Structures.EquationOperatorGallery",
        "Operator",
        "Equation/Structures/Operator",
        &[("Colon equals", "≔"), ("Defined as", "≜"), ("Identical to", "≡"), ("Right arrow", "→"), ("Implies", "⇒")],
    ),
    (
        "equation.structureMatrix",
        "CT.Equation.Structures.EquationMatrixGallery",
        "Matrix",
        "Equation/Structures/Matrix",
        &[
            ("2 × 2 matrix in parentheses", "(■(⬚&⬚@⬚&⬚))"),
            ("2 × 2 matrix in brackets", "[■(⬚&⬚@⬚&⬚)]"),
            ("Identity matrix", "(■(1&0@0&1))"),
            ("Row of three", "■(⬚&⬚&⬚)"),
        ],
    ),
];

pub(crate) fn specs() -> Vec<CommandSpec> {
    let mut v = vec![
        CommandSpec::new("insert.equation", "R.Insert.Symbols.EquationInsertNew", "Insert New Equation", "Insert/Symbols/Equation/Insert New Equation", insert_equation)
            .key("Alt+=")
            .params(r#"{"text"?: UnicodeMath linear format (default: the selected text, else an empty equation), "equation"?: index into the built-in equations}"#)
            .when(need_page),
        CommandSpec::new(
            "equation.professional",
            "CT.Equation.Tools.EquationProfessional",
            "Professional",
            "Equation/Tools/Professional",
            |s, _| set_view(s, MathView::Professional),
        )
        .when(need_equation),
        CommandSpec::new("equation.linear", "CT.Equation.Tools.EquationLinearFormat", "Linear", "Equation/Tools/Linear", |s, _| set_view(s, MathView::Linear))
            .when(need_equation),
        CommandSpec::new("equation.symbol", "CT.Equation.Symbols.EquationSymbolsInsertGallery", "Symbols", "Equation/Symbols", |s, v| {
            let sym = p::str(v, "symbol").unwrap_or("");
            if sym.is_empty() || sym.chars().count() > 4 {
                return Err(CmdError::Params("`symbol`: one of the gallery's symbols".into()));
            }
            insert_in_equation(s, sym)
        })
        .params(r#"{"symbol": string}"#)
        .when(need_page),
    ];
    for ((cmd, row, label, path, _), run) in STRUCTURES.iter().zip(STRUCTURE_FNS) {
        v.push(CommandSpec::new(cmd, row, label, path, run).params(r#"{"item"?: index into the gallery (default 0)}"#).when(need_page));
    }
    v
}

/// Insert item `item` of structure gallery `k`.
fn structure(s: &mut Session, v: &Value, k: usize) -> CmdResult {
    let items = STRUCTURES.get(k).map(|x| x.4).unwrap_or(&[]);
    let i = p::usize(v, "item").unwrap_or(0);
    let (_, text) = items.get(i).ok_or_else(|| CmdError::Params(format!("`item`: 0–{}", items.len().saturating_sub(1))))?;
    insert_in_equation(s, text)
}

macro_rules! structure_fns {
    ($($name:ident = $k:expr),*) => {
        $(fn $name(s: &mut Session, v: &Value) -> CmdResult { structure(s, v, $k) })*
        /// One command function per Structures gallery, in [`STRUCTURES`] order.
        const STRUCTURE_FNS: [fn(&mut Session, &Value) -> CmdResult; 11] = [$($name),*];
    };
}
structure_fns!(st0 = 0, st1 = 1, st2 = 2, st3 = 3, st4 = 4, st5 = 5, st6 = 6, st7 = 7, st8 = 8, st9 = 9, st10 = 10);

/// The selected text when the selection lies within one paragraph.
fn selected_text(s: &Session) -> Option<String> {
    let r = s.text_range().filter(|r| !r.is_caret())?;
    let (a, b) = r.ordered();
    if (a.block, a.para) != (b.block, b.para) {
        return None;
    }
    let text = Session::block_paras(s.page()?, a.block)?.get(a.para)?.text();
    text.get(a.offset..b.offset).map(str::to_string)
}

/// Whether the caret is in an equation.
pub(crate) fn caret_equation(s: &Session) -> Option<(Pos, MathView)> {
    let r = s.text_range()?;
    let page = s.page()?;
    let para = Session::block_paras(page, r.head.block)?.get(r.head.para)?;
    para.math.map(|m| (r.head, m))
}

fn need_equation(s: &Session) -> Option<&'static str> {
    if s.sel.page.is_none() {
        return Some("No page is open");
    }
    if caret_equation(s).is_some() { None } else { Some("Click in an equation first") }
}

fn set_view(s: &mut Session, view: MathView) -> CmdResult {
    let (pos, _) = caret_equation(s).ok_or_else(|| CmdError::Disabled("click in an equation first".into()))?;
    let page = s.page_mut().ok_or_else(no_page)?;
    if let Some(para) = Session::para_mut(page, pos.block, pos.para) {
        para.math = Some(view);
    }
    Ok(Value::Null)
}

/// Insert `text` into the equation the caret is in, or make a new equation of it.
fn insert_in_equation(s: &mut Session, text: &str) -> CmdResult {
    if caret_equation(s).is_some() {
        let caret = ops::ensure_caret(s)?;
        let end = ops::insert_at(s, caret, text, &CharFormat::default())?;
        select_slot(s, caret, end);
        return Ok(Value::Null);
    }
    insert_equation(s, &json!({"text": text}))
}

/// After inserting from `from` to `to`, select the first `⬚` slot so typing fills it, else put
/// the caret at the end.
fn select_slot(s: &mut Session, from: Pos, to: Pos) {
    let text = s.page().and_then(|p| Session::block_paras(p, from.block).and_then(|ps| ps.get(from.para)).map(|p| p.text())).unwrap_or_default();
    let slot = text.get(from.offset..to.offset).and_then(|inserted| inserted.find(notecraft_math::PLACEHOLDER));
    match slot {
        Some(i) => {
            let a = Pos { offset: from.offset + i, ..from };
            let b = Pos { offset: a.offset + notecraft_math::PLACEHOLDER.len_utf8(), ..from };
            s.set_range(a, b);
        }
        None => s.set_caret(to),
    }
}

/// Insert › Equation: the caret's empty paragraph becomes an equation, or a new equation
/// paragraph follows the caret's paragraph (an equation is never split; decision D11).
fn insert_equation(s: &mut Session, v: &Value) -> CmdResult {
    let mut text: String = match (p::str(v, "text"), p::usize(v, "equation")) {
        (Some(t), _) => t.to_string(),
        (None, Some(i)) => {
            EQUATIONS.get(i).map(|(_, t)| (*t).to_string()).ok_or_else(|| CmdError::Params(format!("`equation`: 0–{}", EQUATIONS.len() - 1)))?
        }
        (None, None) => selected_text(s).unwrap_or_default(),
    };
    text = notecraft_doc::sanitize_text(&text).replace('\n', " ").chars().take(notecraft_math::MAX_LEN).collect();
    let caret = ops::ensure_caret(s)?;
    if caret.block == Block::Title {
        return Err(CmdError::Disabled("an equation can't go in the page title".into()));
    }
    let page = s.page().ok_or_else(no_page)?;
    let para =
        Session::block_paras(page, caret.block).and_then(|ps| ps.get(caret.para)).ok_or_else(|| CmdError::Failed("paragraph not found".into()))?;
    let (at, inside) = if para.math.is_some() {
        (caret, true)
    } else if para.is_empty() && para.tags.is_empty() && para.list.is_none() {
        (caret, false)
    } else {
        let end = Pos { offset: para.len(), ..caret };
        (crate::text_cmds::split_at_caret(s, end)?, false)
    };
    if !inside {
        let page = s.page_mut().ok_or_else(no_page)?;
        if let Some(p) = Session::para_mut(page, at.block, at.para) {
            p.math = Some(MathView::Professional);
            p.list = None;
            p.tags.clear();
            p.style = ParaStyle::Normal;
        }
    }
    let end = ops::insert_at(s, at, &text, &CharFormat::default())?;
    select_slot(s, at, end);
    ops::touch(s, at.block, at.para);
    let id = match at.block {
        Block::Outline(id) | Block::Cell(id) => id.to_string(),
        Block::Title => String::new(),
    };
    Ok(json!({"block": id, "para": at.para}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{blank_page, run, sample};

    fn equation(s: &Session) -> Option<(String, MathView)> {
        let (pos, view) = caret_equation(s)?;
        let page = s.page()?;
        Some((Session::block_paras(page, pos.block)?.get(pos.para)?.text(), view))
    }

    #[test]
    fn equations_are_paragraphs_of_linear_text() {
        // parity: R.Insert.Symbols.EquationInsertNew, CT.Equation.Tools.EquationLinearFormat, CT.Equation.Tools.EquationProfessional
        let mut s = sample();
        blank_page(&mut s);
        run(&mut s, "outline.new", json!({"x": 40.0, "y": 100.0}));
        run(&mut s, "text.insert", json!({"text": "Before"}));
        let undo = s.undo_labels().len();
        run(&mut s, "insert.equation", json!({"text": "a^2+b^2=c^2"}));
        assert_eq!(s.undo_labels().len(), undo + 1, "one undo step");
        assert_eq!(equation(&s), Some(("a^2+b^2=c^2".to_string(), MathView::Professional)), "a new paragraph after the text");
        run(&mut s, "equation.linear", json!({}));
        assert_eq!(equation(&s).map(|e| e.1), Some(MathView::Linear));
        run(&mut s, "equation.professional", json!({}));
        // A structure inside it: its first slot is selected, so typing fills it.
        run(&mut s, "equation.structureFraction", json!({}));
        run(&mut s, "text.insert", json!({"text": "1"}));
        assert_eq!(equation(&s).map(|e| e.0), Some("a^2+b^2=c^21/⬚".to_string()));
        // Enter leaves the equation whole and starts plain text after it.
        run(&mut s, "caret.left", json!({}));
        run(&mut s, "text.newline", json!({}));
        assert!(equation(&s).is_none(), "the new paragraph is plain text");
        // Structures and symbols outside an equation start one; built-in equations by index.
        run(&mut s, "equation.symbol", json!({"symbol": "π"}));
        assert_eq!(equation(&s).map(|e| e.0), Some("π".to_string()));
        run(&mut s, "text.newline", json!({}));
        run(&mut s, "insert.equation", json!({"equation": 2}));
        assert_eq!(equation(&s).map(|e| e.0), Some(EQUATIONS[2].1.to_string()));
        // Copied and pasted after plain text, it stays an equation.
        let (pos, _) = caret_equation(&s).unwrap();
        let Block::Outline(id) = pos.block else { panic!("{pos:?}") };
        let len = EQUATIONS[2].1.len();
        run(&mut s, "caret.set", json!({"block": id.to_string(), "para": pos.para, "offset": 0}));
        run(&mut s, "caret.set", json!({"block": id.to_string(), "para": pos.para, "offset": len, "extend": true}));
        run(&mut s, "edit.copy", json!({}));
        run(&mut s, "caret.set", json!({"block": id.to_string(), "para": pos.para, "offset": len}));
        run(&mut s, "text.newline", json!({}));
        run(&mut s, "text.insert", json!({"text": "Then"}));
        run(&mut s, "edit.paste", json!({}));
        assert_eq!(equation(&s), Some((EQUATIONS[2].1.to_string(), MathView::Professional)), "CM.Equation.CopyOneNote and paste");
        // Hostile input.
        assert!(s.run("insert.equation", json!({"equation": 99})).is_err());
        assert!(s.run("equation.structureMatrix", json!({"item": 99})).is_err());
        assert!(s.run("equation.symbol", json!({"symbol": "too long a symbol"})).is_err());
        run(&mut s, "text.newline", json!({}));
        run(&mut s, "insert.equation", json!({"text": "x".repeat(10_000)}));
        assert!(equation(&s).is_some_and(|e| e.0.chars().count() <= notecraft_math::MAX_LEN));
        run(&mut s, "nav.pageTitle", json!({}));
        assert!(s.run("insert.equation", json!({})).is_err(), "not in the title");
        assert!(s.run("equation.linear", json!({})).is_err());
    }

    #[test]
    fn every_structure_gallery_inserts_each_of_its_items() {
        let commands = [
            "equation.structureFraction",
            "equation.structureScript",
            "equation.structureRadical",
            "equation.structureIntegral",
            "equation.structureLargeOperator",
            "equation.structureDelimiter",
            "equation.structureFunction",
            "equation.structureAccent",
            "equation.structureLimit",
            "equation.structureOperator",
            "equation.structureMatrix",
        ];
        assert_eq!(commands.len(), STRUCTURES.len());
        for (cmd, (id, _, _, _, items)) in commands.iter().zip(STRUCTURES) {
            assert_eq!(cmd, id);
            let mut s = sample();
            blank_page(&mut s);
            for (i, (_, text)) in items.iter().enumerate() {
                run(&mut s, cmd, json!({"item": i}));
                let (eq, _) = equation(&s).unwrap();
                assert!(eq.contains(text.trim_start_matches('⬚')), "{cmd} item {i}: {eq}");
                // Leave the slot selection, so the next item goes after this one.
                run(&mut s, "caret.lineEnd", json!({}));
            }
        }
    }
}
