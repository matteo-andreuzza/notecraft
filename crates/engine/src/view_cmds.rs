//! View (zoom, page setup, full page, window) and Insert (time stamps, links) commands.

use notecraft_doc::color::Rgba;
use notecraft_doc::{CharFormat, Id, PageObject, RuleLines};
use notecraft_text as t;
use serde_json::{Value, json};

use crate::ops::{self, need_page, need_text, no_page};
use crate::selection::{Block, PageSel, Pos};
use crate::session::{RibbonMode, ScrollRequest};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// Zoom stops for Zoom In/Out (catalog `BEH.View.Zoom`; unverified stops).
pub const ZOOM_STOPS: [f32; 13] = [0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 3.5, 4.0];

fn need_link(s: &Session) -> Option<&'static str> {
    if link_range(s).is_some() || object_link(s).is_some() { None } else { Some("The caret is not on a link") }
}

/// The selected pictures, attachments and ink strokes (the objects that carry a link of their
/// own; a container's or a table's links are in its text).
fn linkable(s: &Session) -> Vec<Id> {
    let (Some(page), PageSel::Objects { ids }) = (s.page(), &s.sel.on_page) else { return Vec::new() };
    ids.iter()
        .copied()
        .filter(|id| page.objects.iter().any(|o| o.id() == *id && matches!(o, PageObject::Image(_) | PageObject::Attachment(_) | PageObject::Ink(_))))
        .collect()
}

/// The link of the first selected object that has one.
pub(crate) fn object_link(s: &Session) -> Option<String> {
    let (Some(page), PageSel::Objects { ids }) = (s.page(), &s.sel.on_page) else { return None };
    ids.iter().find_map(|id| page.objects.iter().find(|o| o.id() == *id).and_then(|o| o.link().map(str::to_string)))
}

/// Set (or clear) the link on the selected objects; false when none is selected.
fn set_object_link(s: &mut Session, url: Option<String>) -> Result<bool, CmdError> {
    let ids = linkable(s);
    if ids.is_empty() {
        return Ok(false);
    }
    let page = s.page_mut().ok_or_else(no_page)?;
    for o in page.objects.iter_mut().filter(|o| ids.contains(&o.id())) {
        if let Some(slot) = o.link_mut() {
            *slot = url.clone();
        }
    }
    Ok(true)
}

impl Session {
    /// The link under the selection: the text link at the caret, or the selected object's
    /// (what Edit Link… starts from).
    pub fn current_link(&self) -> Option<String> {
        link_range(self).map(|(_, _, _, url)| url).or_else(|| object_link(self))
    }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("view.zoomIn", "R.View.Zoom.ZoomIn", "Zoom In", "View/Zoom/Zoom In", |s, _| zoom_step(s, true)).key("Mod+Plus").pure(),
        CommandSpec::new("view.zoomOut", "R.View.Zoom.ZoomOut", "Zoom Out", "View/Zoom/Zoom Out", |s, _| zoom_step(s, false)).key("Mod+-").pure(),
        CommandSpec::new("view.zoom", "R.View.Zoom.ZoomClassic", "Zoom", "View/Zoom", zoom).params(r#"{"percent": 10-400}"#).pure(),
        CommandSpec::new("view.zoom100", "R.View.Zoom.ZoomCurrent100", "100%", "View/Zoom/100%", |s, _| set_zoom(s, 1.0)).pure(),
        CommandSpec::new("view.zoom75", "QAT.NR.ZoomCurrent75", "75%", "Quick Access Toolbar/Commands Not in the Ribbon/75%", |s, _| {
            set_zoom(s, 0.75)
        })
        .pure(),
        // R.View.Zoom.ZoomPageWidth — the UI passes the factor that fits its viewport.
        CommandSpec::new("view.pageWidth", "R.View.Zoom.ZoomPageWidth", "Page Width", "View/Zoom/Page Width", page_width)
            .params(r#"{"factor"?: number}"#)
            .pure(),
        // R.View.PageSetup.RuleLinesMenu — Ctrl+Shift+R toggles; the gallery passes a kind.
        CommandSpec::new("view.ruleLines", "R.View.PageSetup.RuleLinesMenu", "Rule Lines", "View/Page Setup/Rule Lines", rule_lines)
            .key("Mod+Shift+R")
            .params(r#"{"kind"?: "none"|"narrow"|"college"|"standard"|"wide"|"gridSmall"|"gridMedium"|"gridLarge"|"gridVeryLarge"}"#)
            .when(need_page),
        CommandSpec::new(
            "view.ruleLineColor",
            "R.View.PageSetup.RuleLineColorGallery",
            "Rule Line Color",
            "View/Page Setup/Rule Lines/Rule Line Color",
            rule_color,
        )
        .params(r##"{"color": "#rrggbb" | null}"##)
        .when(need_page),
        CommandSpec::new(
            "view.newPagesWithRuleLines",
            "R.View.PageSetup.NewPagesWithRuleLines",
            "Always Create Pages with Rule Lines",
            "View/Page Setup/Rule Lines/Always Create Pages with Rule Lines",
            new_pages_rules,
        )
        .pure(),
        CommandSpec::new("view.hidePageTitle", "R.View.PageSetup.PageTitleHide", "Hide Page Title", "View/Page Setup/Hide Page Title", hide_title)
            .when(need_page),
        CommandSpec::new("view.pageColor", "R.View.PageSetup.PageColorGallery", "Page Color", "View/Page Setup/Page Color", page_color)
            .params(r##"{"color": "#rrggbb" | null (No Color)}"##)
            .when(need_page),
        CommandSpec::new("view.fullPage", "R.View.Views.FullPageView", "Full Page View", "View/Views/Full Page View", |s, _| {
            s.view.full_page = !s.view.full_page;
            Ok(json!({"fullPage": s.view.full_page}))
        })
        .key("F11")
        .pure(),
        CommandSpec::new("view.normal", "R.View.Views.NormalViewOneNote", "Normal View", "View/Views/Normal View", |s, _| {
            s.view.full_page = false;
            s.view.docked = false;
            Ok(Value::Null)
        })
        .pure(),
        CommandSpec::new("view.scrollPageUp", "KB.Pages.ScrollUp", "Scroll Up", "Keyboard/Pages/Scroll up in the current page", |s, _| {
            scroll(s, ScrollRequest::PageUp)
        })
        .key("PageUp")
        .pure()
        .when(need_page),
        CommandSpec::new("view.scrollPageDown", "KB.Pages.ScrollDown", "Scroll Down", "Keyboard/Pages/Scroll down in the current page", |s, _| {
            scroll(s, ScrollRequest::PageDown)
        })
        .key("PageDown")
        .pure()
        .when(need_page),
        CommandSpec::new("view.scrollTop", "KB.Pages.Top", "Scroll to Top", "Keyboard/Pages/Scroll to the top of the page", |s, _| {
            scroll(s, ScrollRequest::Top)
        })
        .key("Mod+Home")
        .pure()
        .when(need_page),
        CommandSpec::new("view.scrollBottom", "KB.Pages.Bottom", "Scroll to Bottom", "Keyboard/Pages/Scroll to the bottom of the page", |s, _| {
            scroll(s, ScrollRequest::Bottom)
        })
        .key("Mod+End")
        .pure()
        .when(need_page),
        CommandSpec::new(
            "ui.collapseRibbon",
            "QAT.NR.MinimizeRibbon",
            "Collapse the Ribbon",
            "Quick Access Toolbar/Commands Not in the Ribbon/Collapse the Ribbon",
            |s, _| {
                s.view.ribbon_collapsed = !s.view.ribbon_collapsed;
                Ok(json!({"collapsed": s.view.ribbon_collapsed}))
            },
        )
        .key("Mod+F1")
        .pure(),
        // R.View.Views.NavigationPanes — show or hide the navigation (the notebook tree and the
        // page column), also the ☰ button under the ribbon (UI.Nav.Toggle).
        CommandSpec::new("view.navigationPanes", "R.View.Views.NavigationPanes", "Navigation Panes", "View/Views/Navigation Panes", |s, _| {
            s.view.notebook_pane_collapsed = !s.view.notebook_pane_collapsed;
            Ok(json!({"shown": !s.view.notebook_pane_collapsed}))
        })
        .pure(),
        // UI.Ribbon.ModeSwitch — the ▾ chevron at the ribbon's bottom right: simplified (one row,
        // the 2024 default) or classic. Without `mode` it switches to the other one.
        CommandSpec::new("ui.ribbonMode", "UI.Ribbon.ModeSwitch", "Ribbon Layout", "Window/Ribbon/Mode Switch", |s, p| {
            s.view.ribbon_mode = match p.get("mode").and_then(Value::as_str) {
                Some("simplified") => RibbonMode::Simplified,
                Some("classic") => RibbonMode::Classic,
                Some(other) => return Err(CmdError::Params(format!("unknown ribbon mode `{other}` (simplified or classic)"))),
                None if s.view.ribbon_mode == RibbonMode::Simplified => RibbonMode::Classic,
                None => RibbonMode::Simplified,
            };
            Ok(json!({"mode": if s.view.ribbon_mode == RibbonMode::Simplified { "simplified" } else { "classic" }}))
        })
        .params(r#"{"mode"?: "simplified" | "classic"}"#)
        .pure(),
        CommandSpec::new(
            "ui.pageListWider",
            "KB.Pages.WidenList",
            "Widen Page List",
            "Keyboard/Pages/Increase the width of the page list",
            |s, _| list_width(s, 24.0),
        )
        .key("Mod+Shift+[")
        .pure(),
        CommandSpec::new(
            "ui.pageListNarrower",
            "KB.Pages.NarrowList",
            "Narrow Page List",
            "Keyboard/Pages/Decrease the width of the page list",
            |s, _| list_width(s, -24.0),
        )
        .key("Mod+Shift+]")
        .pure(),
        // NC.View.NotebookPane — the notebook pane's collapse/pin button (UI.Nav.NotebookPaneCollapse).
        CommandSpec::new(
            "view.notebookPane",
            "UI.Nav.NotebookPaneCollapse",
            "Collapse Notebook Pane",
            "Window/Navigation/Notebook Pane Collapse",
            |s, _| {
                s.view.notebook_pane_collapsed = !s.view.notebook_pane_collapsed;
                Ok(json!({"collapsed": s.view.notebook_pane_collapsed}))
            },
        )
        .pure(),
        // R.Insert.TimeStamp.* — Alt+Shift+D / T / F (en-US formats; locale formats come with
        // localization).
        CommandSpec::new("insert.date", "R.Insert.TimeStamp.DateInsert", "Date", "Insert/Time Stamp/Date", |s, _| stamp(s, true, false))
            .key("Alt+Shift+D")
            .when(need_page),
        CommandSpec::new("insert.time", "R.Insert.TimeStamp.TimeInsert", "Time", "Insert/Time Stamp/Time", |s, _| stamp(s, false, true))
            .key("Alt+Shift+T")
            .when(need_page),
        CommandSpec::new("insert.dateTime", "R.Insert.TimeStamp.DateAndTimeInsertOneNote", "Date & Time", "Insert/Time Stamp/Date & Time", |s, _| {
            stamp(s, true, true)
        })
        .key("Alt+Shift+F")
        .when(need_page),
        // R.Insert.Links.HyperlinkCreateHere — Ctrl+K; the Link dialog's result.
        CommandSpec::new("insert.link", "R.Insert.Links.HyperlinkCreateHere", "Link", "Insert/Links/Link", insert_link)
            .key("Mod+K")
            .params(r#"{"url": string, "text"?: string}"#)
            .when(need_page),
        CommandSpec::new("link.edit", "CM.Text.LinkEdit", "Edit Link…", "Context/Text/Edit Link…", edit_link)
            .params(r#"{"url": string, "text"?: string}"#)
            .when(need_link),
        CommandSpec::new("link.remove", "CM.Text.LinkRemove", "Remove Link", "Context/Text/Remove Link", remove_link).when(need_link),
        CommandSpec::new("link.copy", "CM.Text.LinkCopyOneNote", "Copy Link", "Context/Text/Copy Link", copy_link).pure().when(need_link),
        CommandSpec::new("link.select", "CM.Text.LinkSelect", "Select Link", "Context/Text/Select Link", select_link).pure().when(need_link),
        // BEH.Links.CopyLink — Copy Link to Paragraph (paragraph context menu).
        CommandSpec::new("para.copyLink", "BEH.Links.CopyLink", "Copy Link to Paragraph", "Behaviour/Links", copy_para_link).pure().when(need_text),
    ]
}

fn set_zoom(s: &mut Session, z: f32) -> CmdResult {
    s.view.zoom = if z.is_finite() { z.clamp(0.1, 4.0) } else { 1.0 };
    Ok(json!({"zoom": s.view.zoom}))
}

fn zoom_step(s: &mut Session, up: bool) -> CmdResult {
    let z = s.view.zoom;
    let next = if up {
        ZOOM_STOPS.iter().copied().find(|&v| v > z + 1e-3).unwrap_or(4.0)
    } else {
        ZOOM_STOPS.iter().rev().copied().find(|&v| v < z - 1e-3).unwrap_or(0.1)
    };
    set_zoom(s, next)
}

fn zoom(s: &mut Session, v: &Value) -> CmdResult {
    let pct = p::req_f32(v, "percent")?;
    if !(10.0..=400.0).contains(&pct) {
        return Err(CmdError::Params("zoom must be between 10% and 400%".into()));
    }
    set_zoom(s, pct / 100.0)
}

fn page_width(s: &mut Session, v: &Value) -> CmdResult {
    set_zoom(s, p::f32(v, "factor").unwrap_or(1.0))
}

fn parse_rules(k: &str) -> Option<RuleLines> {
    Some(match k {
        "none" => RuleLines::None,
        "narrow" => RuleLines::Narrow,
        "college" => RuleLines::College,
        "standard" => RuleLines::Standard,
        "wide" => RuleLines::Wide,
        "gridSmall" => RuleLines::GridSmall,
        "gridMedium" => RuleLines::GridMedium,
        "gridLarge" => RuleLines::GridLarge,
        "gridVeryLarge" => RuleLines::GridVeryLarge,
        _ => return None,
    })
}

fn rule_lines(s: &mut Session, v: &Value) -> CmdResult {
    let kind = match p::str(v, "kind") {
        Some(k) => parse_rules(k).ok_or_else(|| CmdError::Params(format!("unknown rule lines `{k}`")))?,
        None => {
            // Ctrl+Shift+R: off ↔ the last used kind (standard by default).
            let cur = s.page().map(|p| p.setup.rule_lines).unwrap_or_default();
            if cur == RuleLines::None { s.prefs.new_pages_rule_lines.unwrap_or(RuleLines::Standard) } else { RuleLines::None }
        }
    };
    let page = s.page_mut().ok_or_else(no_page)?;
    page.setup.rule_lines = kind;
    Ok(Value::Null)
}

fn color_param(v: &Value) -> Result<Option<Rgba>, CmdError> {
    match v.get("color") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(c)) => Rgba::from_hex(c).map(Some).ok_or_else(|| CmdError::Params("`color` must be #rrggbb".into())),
        Some(_) => Err(CmdError::Params("`color` must be a colour string or null".into())),
    }
}

fn rule_color(s: &mut Session, v: &Value) -> CmdResult {
    let c = color_param(v)?;
    let page = s.page_mut().ok_or_else(no_page)?;
    page.setup.rule_color = c;
    if page.setup.rule_lines == RuleLines::None {
        page.setup.rule_lines = RuleLines::Standard;
    }
    Ok(Value::Null)
}

fn new_pages_rules(s: &mut Session, _: &Value) -> CmdResult {
    s.prefs.new_pages_rule_lines = match s.prefs.new_pages_rule_lines {
        Some(_) => None,
        None => Some(s.page().map(|p| p.setup.rule_lines).filter(|r| *r != RuleLines::None).unwrap_or(RuleLines::Standard)),
    };
    Ok(json!({"on": s.prefs.new_pages_rule_lines.is_some()}))
}

fn hide_title(s: &mut Session, _: &Value) -> CmdResult {
    let page = s.page_mut().ok_or_else(no_page)?;
    page.setup.hide_title = !page.setup.hide_title;
    Ok(json!({"hidden": page.setup.hide_title}))
}

fn page_color(s: &mut Session, v: &Value) -> CmdResult {
    let c = color_param(v)?;
    let page = s.page_mut().ok_or_else(no_page)?;
    page.setup.color = c;
    Ok(Value::Null)
}

fn scroll(s: &mut Session, r: ScrollRequest) -> CmdResult {
    s.scroll_request = Some(r);
    Ok(Value::Null)
}

fn list_width(s: &mut Session, d: f32) -> CmdResult {
    s.view.page_list_width = (s.view.page_list_width + d).clamp(120.0, 600.0);
    Ok(json!({"width": s.view.page_list_width}))
}

fn stamp(s: &mut Session, date: bool, time: bool) -> CmdResult {
    let now = s.now();
    let off = s.clock.utc_offset_min;
    let text = match (date, time) {
        (true, true) => format!("{} {}", now.short_date(off), now.short_time(off)),
        (true, false) => now.short_date(off),
        _ => now.short_time(off),
    };
    let caret = ops::ensure_caret(s)?;
    let fmt = ops::typing_fmt(s, caret);
    let pos = ops::insert_at(s, caret, &text, &fmt)?;
    s.set_caret(pos);
    Ok(json!({"text": text}))
}

fn validate_url(url: &str) -> Result<String, CmdError> {
    let u = url.trim();
    if u.is_empty() || u.len() > 4096 || u.chars().any(char::is_control) {
        return Err(CmdError::Params("`url` must be 1–4096 characters without control characters".into()));
    }
    Ok(u.to_string())
}

fn insert_link(s: &mut Session, v: &Value) -> CmdResult {
    let url = validate_url(p::req_str(v, "url")?)?;
    // Selected pictures, attachments or ink get the link themselves (their Link… menu items).
    if set_object_link(s, Some(url.clone()))? {
        return Ok(Value::Null);
    }
    let text = p::str(v, "text").map(str::to_string);
    let has_selection = s.text_range().is_some_and(|r| !r.is_caret());
    if has_selection && text.is_none() {
        // Link the selected text.
        let segs = ops::segments(s);
        let page = s.page_mut().ok_or_else(no_page)?;
        for (b, i, r) in segs {
            if let Some(p) = Session::para_mut(page, b, i) {
                t::apply_format(p, r, &|f| f.link = Some(url.clone()));
            }
        }
        return Ok(Value::Null);
    }
    let shown = text.unwrap_or_else(|| url.clone());
    let caret = ops::ensure_caret(s)?;
    let mut fmt = ops::typing_fmt(s, caret);
    fmt.link = Some(url);
    let pos = ops::insert_at(s, caret, &shown, &fmt)?;
    s.set_caret(pos);
    Ok(Value::Null)
}

/// The paragraph and byte range of the link run around the caret (or the selection start).
fn link_range(s: &Session) -> Option<(Block, usize, std::ops::Range<usize>, String)> {
    let r = s.text_range()?;
    let c = r.ordered().0;
    let page = s.page()?;
    let para = Session::block_paras(page, c.block)?.get(c.para)?;
    let mut pos = 0;
    for run in &para.runs {
        let end = pos + run.text.len();
        if let Some(l) = &run.fmt.link
            && c.offset >= pos
            && c.offset <= end
            && (c.offset < end || !r.is_caret() || c.offset > pos)
        {
            // Extend over adjacent runs with the same link (formatting can split a link).
            let mut a = pos;
            let mut b = end;
            let mut p2 = 0;
            let spans: Vec<(usize, usize, Option<&String>)> = para
                .runs
                .iter()
                .map(|x| {
                    let s0 = p2;
                    p2 += x.text.len();
                    (s0, p2, x.fmt.link.as_ref())
                })
                .collect();
            for &(s0, e0, lk) in spans.iter().rev() {
                if e0 == a && lk == Some(l) {
                    a = s0;
                }
            }
            for &(s0, e0, lk) in &spans {
                if s0 == b && lk == Some(l) {
                    b = e0;
                }
            }
            return Some((c.block, c.para, a..b, l.clone()));
        }
        pos = end;
    }
    None
}

fn edit_link(s: &mut Session, v: &Value) -> CmdResult {
    let url = validate_url(p::req_str(v, "url")?)?;
    if link_range(s).is_none() && object_link(s).is_some() {
        set_object_link(s, Some(url))?;
        return Ok(Value::Null);
    }
    let (b, i, r, _) = link_range(s).ok_or_else(|| CmdError::Disabled("the caret is not on a link".into()))?;
    let text = p::str(v, "text").map(str::to_string);
    let page = s.page_mut().ok_or_else(no_page)?;
    let p = Session::para_mut(page, b, i).ok_or_else(no_page)?;
    match text {
        Some(t2) if !t2.is_empty() => {
            let fmt = CharFormat { link: Some(url), ..p.fmt_at(r.start + 1) };
            t::delete(p, r.clone());
            t::insert(p, r.start, &t2, &fmt);
        }
        _ => t::apply_format(p, r, &|f| f.link = Some(url.clone())),
    }
    Ok(Value::Null)
}

fn remove_link(s: &mut Session, _: &Value) -> CmdResult {
    if link_range(s).is_none() && object_link(s).is_some() {
        set_object_link(s, None)?;
        return Ok(Value::Null);
    }
    let (b, i, r, _) = link_range(s).ok_or_else(|| CmdError::Disabled("the caret is not on a link".into()))?;
    let page = s.page_mut().ok_or_else(no_page)?;
    if let Some(p) = Session::para_mut(page, b, i) {
        t::apply_format(p, r, &|f| f.link = None);
    }
    Ok(Value::Null)
}

fn copy_link(s: &mut Session, _: &Value) -> CmdResult {
    let url = s.current_link().ok_or_else(|| CmdError::Disabled("the caret is not on a link".into()))?;
    s.clipboard = crate::session::Clipboard { text: url.clone(), ..Default::default() };
    Ok(json!({"text": url}))
}

fn select_link(s: &mut Session, _: &Value) -> CmdResult {
    let (b, i, r, _) = link_range(s).ok_or_else(|| CmdError::Disabled("the caret is not on a link".into()))?;
    s.set_range(Pos::new(b, i, r.start), Pos::new(b, i, r.end));
    Ok(Value::Null)
}

fn copy_para_link(s: &mut Session, _: &Value) -> CmdResult {
    let c = s.sel.caret().ok_or_else(|| CmdError::Disabled("click in a note first".into()))?;
    let page = s.page().ok_or_else(no_page)?;
    let pid = page.id;
    let para = Session::block_paras(page, c.block).and_then(|ps| ps.get(c.para)).ok_or_else(no_page)?;
    let link = format!("notecraft:para/{pid}/{}", para.id);
    let text = para.text();
    ops::set_link_clipboard(s, if text.is_empty() { &link } else { &text }, &link);
    Ok(json!({"link": link}))
}
