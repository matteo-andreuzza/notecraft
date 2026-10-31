//! View › Paper Size: the Paper Size pane and the page's fixed paper (catalog
//! `R.View.PageSetup.PaperSize`, `UI.Panes.PaperSize`).
//!
//! A page without a paper size ("Auto") grows freely (`BEH.Canvas.PageGrows`); a page with one
//! prints on that paper (`import-export::pdf`) and the canvas marks where its sheets end. All
//! lengths are points.

use notecraft_doc::PaperSize;
use serde_json::{Value, json};

use crate::ops::{need_page, no_page};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

const INCH: f32 = 72.0;
const MM: f32 = 72.0 / 25.4;

/// A named paper in the pane's Size list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaperPreset {
    /// Stable id for agents (`letter`, `a4`, …).
    pub slug: &'static str,
    /// The name the list shows.
    pub name: &'static str,
    /// Portrait width in points.
    pub width: f32,
    /// Portrait height in points.
    pub height: f32,
}

const fn inches(slug: &'static str, name: &'static str, w: f32, h: f32) -> PaperPreset {
    PaperPreset { slug, name, width: w * INCH, height: h * INCH }
}

const fn mm(slug: &'static str, name: &'static str, w: f32, h: f32) -> PaperPreset {
    PaperPreset { slug, name, width: w * MM, height: h * MM }
}

/// The Size list after Auto, in the pane's order (unverified: the reference's list depends on
/// the printer; these are the common ISO, JIS and US sizes).
pub const PAPER_SIZES: &[PaperPreset] = &[
    inches("statement", "Statement", 5.5, 8.5),
    inches("letter", "Letter", 8.5, 11.0),
    inches("tabloid", "Tabloid", 11.0, 17.0),
    inches("legal", "Legal", 8.5, 14.0),
    mm("a3", "A3", 297.0, 420.0),
    mm("a4", "A4", 210.0, 297.0),
    mm("a5", "A5", 148.0, 210.0),
    mm("a6", "A6", 105.0, 148.0),
    mm("b4", "B4 (JIS)", 257.0, 364.0),
    mm("b5", "B5 (JIS)", 182.0, 257.0),
    mm("b6", "B6 (JIS)", 128.0, 182.0),
    mm("postcard", "Japanese Postcard", 100.0, 148.0),
    inches("index", "Index Card", 3.0, 5.0),
    inches("billfold", "Billfold", 3.75, 6.75),
];

/// Print margins a page gets when it first gets a paper size (half an inch, the PDF export's
/// default; unverified).
pub const DEFAULT_MARGINS: [f32; 4] = [36.0; 4];
/// Smallest and largest paper side (the PDF export's limits).
pub const MIN_SIDE: f32 = 72.0;
/// Largest paper side.
pub const MAX_SIDE: f32 = 14_400.0;

/// The preset a paper is (either orientation, half a point of slack), if any.
pub fn preset_of(paper: &PaperSize) -> Option<&'static PaperPreset> {
    let near = |a: f32, b: f32| (a - b).abs() < 0.5;
    PAPER_SIZES
        .iter()
        .find(|s| (near(s.width, paper.width) && near(s.height, paper.height)) || (near(s.width, paper.height) && near(s.height, paper.width)))
}

/// Whether a paper is wider than tall.
pub fn is_landscape(paper: &PaperSize) -> bool {
    paper.width > paper.height
}

/// What the pane and agents see of a page's paper.
pub fn describe(paper: Option<&PaperSize>) -> Value {
    match paper {
        None => json!({"size": "auto"}),
        Some(ps) => json!({
            "size": preset_of(ps).map_or("custom", |s| s.slug),
            "orientation": if is_landscape(ps) { "landscape" } else { "portrait" },
            "width": ps.width,
            "height": ps.height,
            "margins": {"top": ps.margins[0], "right": ps.margins[1], "bottom": ps.margins[2], "left": ps.margins[3]},
        }),
    }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("view.paperSize", "R.View.PageSetup.PaperSize", "Paper Size", "View/Page Setup/Paper Size", |s, v| {
            s.view.paper_pane = p::bool(v, "on").unwrap_or(!s.view.paper_pane);
            Ok(json!({"open": s.view.paper_pane}))
        })
        .params(r#"{"on"?: bool} (the Paper Size pane; default: switch)"#)
        .pure(),
        CommandSpec::new("page.paperSize", "UI.Panes.PaperSize.Size", "Size", "Window/Panes/Paper Size/Size", set_paper)
            .params(
                r#"{"size"?: "auto"|"custom"|"letter"|"a4"|… (paper_cmds::PAPER_SIZES), "orientation"?: "portrait"|"landscape", "width"?: pt, "height"?: pt, "top"?|"right"?|"bottom"?|"left"?: pt (print margins)} — no params: report the page's paper"#,
            )
            .when(need_page),
    ]
}

fn side(v: &Value, k: &str) -> Result<Option<f32>, CmdError> {
    match v.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => match p::f32(v, k) {
            Some(x) if x > 0.0 => Ok(Some(x)),
            _ => Err(CmdError::Params(format!("`{k}` must be a positive number of points"))),
        },
    }
}

fn set_paper(s: &mut Session, v: &Value) -> CmdResult {
    let size = p::str(v, "size");
    let orientation = match p::str(v, "orientation") {
        None => None,
        Some("portrait") => Some(false),
        Some("landscape") => Some(true),
        Some(o) => return Err(CmdError::Params(format!("unknown orientation `{o}`"))),
    };
    let preset = match size {
        None | Some("custom" | "auto") => None,
        Some(slug) => Some(PAPER_SIZES.iter().find(|x| x.slug == slug).ok_or_else(|| CmdError::Params(format!("unknown paper size `{slug}`")))?),
    };
    let (width, height) = (side(v, "width")?, side(v, "height")?);
    let mut margins: [Option<f32>; 4] = [None; 4];
    for (i, k) in ["top", "right", "bottom", "left"].into_iter().enumerate() {
        margins[i] = match v.get(k) {
            None | Some(Value::Null) => None,
            Some(_) => Some(p::f32(v, k).filter(|m| *m >= 0.0).ok_or_else(|| CmdError::Params(format!("`{k}` must be a number of points ≥ 0")))?),
        };
    }
    let page = s.page_mut().ok_or_else(no_page)?;
    if size == Some("auto") {
        page.setup.paper = None;
        return Ok(describe(None));
    }
    let asked = size.is_some() || orientation.is_some() || width.is_some() || height.is_some() || margins.iter().any(Option::is_some);
    if !asked {
        return Ok(describe(page.setup.paper.as_ref()));
    }
    // Start from the page's paper, else Letter portrait.
    let mut ps = page.setup.paper.unwrap_or(PaperSize { width: 8.5 * INCH, height: 11.0 * INCH, margins: DEFAULT_MARGINS });
    if let Some(pr) = preset {
        // A new size keeps the page's orientation.
        let land = is_landscape(&ps);
        (ps.width, ps.height) = if land { (pr.height, pr.width) } else { (pr.width, pr.height) };
    }
    if let Some(w) = width {
        ps.width = w;
    }
    if let Some(h) = height {
        ps.height = h;
    }
    ps.width = ps.width.clamp(MIN_SIDE, MAX_SIDE);
    ps.height = ps.height.clamp(MIN_SIDE, MAX_SIDE);
    if let Some(land) = orientation
        && land != is_landscape(&ps)
        && ps.width != ps.height
    {
        std::mem::swap(&mut ps.width, &mut ps.height);
        // The margins turn with the paper (top ← left, right ← top, …).
        let [t, r, b, l] = ps.margins;
        ps.margins = if land { [l, t, r, b] } else { [r, b, l, t] };
    }
    for (i, m) in margins.into_iter().enumerate() {
        if let Some(m) = m
            && let Some(slot) = ps.margins.get_mut(i)
        {
            *slot = m;
        }
    }
    // Margins leave at least half an inch of printable paper each way.
    let (max_v, max_h) = (((ps.height - 36.0) / 2.0).max(0.0), ((ps.width - 36.0) / 2.0).max(0.0));
    for (i, m) in ps.margins.iter_mut().enumerate() {
        let cap = if i % 2 == 0 { max_v } else { max_h };
        *m = if m.is_finite() { m.clamp(0.0, cap.min(720.0)) } else { 0.0 };
    }
    page.setup.paper = Some(ps);
    Ok(describe(Some(&ps)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{run, sample};

    fn paper(s: &Session) -> Option<PaperSize> {
        s.page().and_then(|p| p.setup.paper)
    }

    #[test]
    fn the_paper_size_pane_opens_and_sets_the_page_paper() {
        // parity: R.View.PageSetup.PaperSize, UI.Panes.PaperSize.Size
        let mut s = sample();
        assert_eq!(run(&mut s, "view.paperSize", json!({}))["open"], true);
        assert!(s.view.paper_pane);
        assert_eq!(run(&mut s, "page.paperSize", json!({}))["size"], "auto", "a new page grows freely");
        let r = run(&mut s, "page.paperSize", json!({"size": "a4"}));
        assert_eq!((r["size"].as_str(), r["orientation"].as_str()), (Some("a4"), Some("portrait")));
        let ps = paper(&s).unwrap();
        assert!((ps.width - 595.3).abs() < 0.2 && (ps.height - 841.9).abs() < 0.2, "{ps:?}");
        assert_eq!(ps.margins, DEFAULT_MARGINS);
        // One undo step back to Auto.
        run(&mut s, "edit.undo", json!({}));
        assert_eq!(paper(&s), None);
        run(&mut s, "edit.redo", json!({}));
        assert!(paper(&s).is_some());
        run(&mut s, "page.paperSize", json!({"size": "auto"}));
        assert_eq!(paper(&s), None);
        assert_eq!(run(&mut s, "view.paperSize", json!({"on": false}))["open"], false);
    }

    #[test]
    fn orientation_margins_and_custom_sizes() {
        // parity: UI.Panes.PaperSize.Orientation, UI.Panes.PaperSize.Dimensions, UI.Panes.PaperSize.Margins
        let mut s = sample();
        run(&mut s, "page.paperSize", json!({"size": "letter", "top": 72.0, "left": 18.0}));
        assert_eq!(paper(&s).unwrap().margins, [72.0, 36.0, 36.0, 18.0]);
        let r = run(&mut s, "page.paperSize", json!({"orientation": "landscape"}));
        assert_eq!((r["size"].as_str(), r["width"].as_f64(), r["height"].as_f64()), (Some("letter"), Some(792.0), Some(612.0)));
        assert_eq!(paper(&s).unwrap().margins, [18.0, 72.0, 36.0, 36.0], "the margins turn with the paper");
        // A new size keeps landscape.
        let r = run(&mut s, "page.paperSize", json!({"size": "legal"}));
        assert_eq!((r["width"].as_f64(), r["orientation"].as_str()), (Some(1008.0), Some("landscape")));
        run(&mut s, "page.paperSize", json!({"orientation": "portrait"}));
        assert_eq!(paper(&s).unwrap().margins, [72.0, 36.0, 36.0, 18.0], "and back");
        // A width of no preset is Custom.
        assert_eq!(run(&mut s, "page.paperSize", json!({"width": 500.0}))["size"], "custom");
        // Out-of-range values are clamped, never refused silently into nonsense.
        run(&mut s, "page.paperSize", json!({"width": 1.0, "height": 1e9, "top": 1e6}));
        let ps = paper(&s).unwrap();
        assert_eq!((ps.width, ps.height), (MIN_SIDE, MAX_SIDE));
        assert!(ps.margins[0] <= 720.0 && ps.margins[1] <= 18.0, "{ps:?}");
    }

    #[test]
    fn bad_params_are_errors() {
        let mut s = sample();
        for v in [
            json!({"size": "napkin"}),
            json!({"orientation": "sideways"}),
            json!({"width": -3}),
            json!({"height": "tall"}),
            json!({"top": -1}),
            json!({"left": "x"}),
        ] {
            assert!(matches!(s.run("page.paperSize", v.clone()), Err(CmdError::Params(_))), "{v}");
        }
        assert_eq!(paper(&s), None, "nothing changed");
    }

    #[test]
    fn every_preset_is_found_back() {
        for pr in PAPER_SIZES {
            let ps = PaperSize { width: pr.height, height: pr.width, margins: DEFAULT_MARGINS };
            assert_eq!(preset_of(&ps).map(|x| x.slug), Some(pr.slug));
        }
    }
}
