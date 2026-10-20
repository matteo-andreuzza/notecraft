//! The ribbon card and the classic layout of the selected tab's groups (the tab row is in
//! `chrome`, the one-row layout in `ribbon_simplified`).
//!
//! Structure comes from [`crate::ribbon_layout`] (verified against the parity catalog); each
//! control's label, kind and command come from its catalog row. A control whose command is not
//! registered, or whose `enabled` predicate says no, is drawn **disabled** with the reason in its
//! tooltip (AGENTS.md › No fake UI). Clicking runs the command by id — nothing else.

use std::collections::HashMap;
use std::sync::OnceLock;

use egui::{Align2, Color32, Frame, Margin, Popup, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use notecraft_engine::catalog::{self, Row};
use notecraft_engine::doc::color::{FONT_THEME, HIGHLIGHT_COLORS, PAGE_COLORS, STANDARD_COLORS, font_grid};
use notecraft_engine::doc::{ParaStyle, RuleLines};
use serde_json::{Value, json};

use notecraft_engine::RibbonMode;

use crate::ribbon_layout::{Group, Item};
use crate::theme::{TypeRung, c32, radius, size};
use crate::{NoteApp, Tokens, icons};

/// Height of the ribbon body (controls + group label).
pub const BODY_H: f32 = size::RIBBON_CLASSIC;
const LABEL_H: f32 = 16.0;
const CONTENT_H: f32 = BODY_H - LABEL_H - 6.0;

/// Catalog rows by id (parsed once).
pub fn rows() -> &'static HashMap<String, Row> {
    static ROWS: OnceLock<HashMap<String, Row>> = OnceLock::new();
    ROWS.get_or_init(|| catalog::rows().into_iter().map(|r| (r.id.clone(), r)).collect())
}

pub(crate) fn row(id: &str) -> Option<&'static Row> {
    rows().get(id)
}

/// The label shown on a control: the catalog label without its "Tag: "/"Style: " prefixes.
pub(crate) fn label(id: &str) -> String {
    let l = row(id).map(|r| r.label.clone()).unwrap_or_else(|| id.to_string());
    let l = l.trim_start_matches("Tag: ").trim_start_matches("Style: ").trim_start_matches("Recent Edits: ");
    // "Navigation Panes (show/hide)": the parenthetical explains the row, it isn't the label.
    match l.find(" (") {
        Some(i) if l.ends_with(')') => l[..i].to_string(),
        _ => l.to_string(),
    }
}

/// The params a control passes to its command (gallery items carry their value).
pub(crate) fn params_for(id: &str) -> Value {
    let k = id.rsplit('.').next().unwrap_or("");
    match k {
        "FindToday" => json!({"range": "today"}),
        "PasteText" => json!({}),
        _ => json!({}),
    }
}

/// Command id for a control, if any.
pub(crate) fn command(id: &str) -> Option<String> {
    row(id).map(|r| r.command.clone()).filter(|c| !c.is_empty())
}

/// Whether a toggle control is currently on.
/// The table the selection is in (the caret's cell, selected cells or a selected table).
fn current_table(app: &NoteApp) -> Option<&notecraft_engine::doc::Table> {
    use notecraft_engine::selection::{Block, PageSel};
    let page = app.session.page()?;
    match &app.session.sel.on_page {
        PageSel::Text(r) => match r.head.block {
            Block::Cell(c) => page.table_of_cell(c).map(|(t, _, _)| t),
            _ => None,
        },
        PageSel::Cells { table, .. } => page.table(*table),
        PageSel::Objects { ids } => ids.iter().find_map(|id| page.table(*id)),
        PageSel::None => None,
    }
}

pub(crate) fn checked(app: &NoteApp, cmd: &str) -> bool {
    let s = &app.session;
    let fmt = || {
        let r = s.text_range()?;
        let page = s.page()?;
        let paras = notecraft_engine::Session::block_paras(page, r.head.block)?;
        let p = paras.get(r.head.para)?;
        let base = p.style.def().char;
        Some((s.pending_fmt.clone().unwrap_or_else(|| p.fmt_at(r.ordered().0.offset + usize::from(!r.is_caret()))).over(&base), p.align, p.list))
    };
    match cmd {
        "format.bold" => fmt().is_some_and(|(f, _, _)| f.bold == Some(true)),
        "format.italic" => fmt().is_some_and(|(f, _, _)| f.italic == Some(true)),
        "format.underline" => fmt().is_some_and(|(f, _, _)| f.underline == Some(true)),
        "format.strikethrough" => fmt().is_some_and(|(f, _, _)| f.strike == Some(true)),
        "format.subscript" => fmt().is_some_and(|(f, _, _)| f.script == Some(notecraft_engine::doc::Script::Sub)),
        "format.superscript" => fmt().is_some_and(|(f, _, _)| f.script == Some(notecraft_engine::doc::Script::Super)),
        "para.alignLeft" => fmt().is_some_and(|(_, a, _)| a == notecraft_engine::doc::Align::Left),
        "para.alignCenter" => fmt().is_some_and(|(_, a, _)| a == notecraft_engine::doc::Align::Center),
        "para.alignRight" => fmt().is_some_and(|(_, a, _)| a == notecraft_engine::doc::Align::Right),
        "para.bullets" => fmt().is_some_and(|(_, _, l)| matches!(l, Some(notecraft_engine::doc::ListFormat::Bullet(_)))),
        "para.numbering" => fmt().is_some_and(|(_, _, l)| matches!(l, Some(notecraft_engine::doc::ListFormat::Number { .. }))),
        "view.fullPage" => s.view.full_page,
        "view.normal" => !s.view.full_page,
        "view.hidePageTitle" => s.page().is_some_and(|p| p.setup.hide_title),
        "view.newPagesWithRuleLines" => s.prefs.new_pages_rule_lines.is_some(),
        "tag.findTags" => s.view.tags_pane,
        "view.navigationPanes" => !s.view.notebook_pane_collapsed,
        "table.hideBorders" => current_table(app).is_some_and(|t| !t.borders),
        "table.headerRow" => current_table(app).is_some_and(|t| t.header_row),
        "draw.type" => s.draw.tool == notecraft_engine::DrawTool::Type,
        "insert.space" => s.draw.tool == notecraft_engine::DrawTool::Space,
        "draw.lasso" => s.draw.tool == notecraft_engine::DrawTool::Lasso,
        "draw.pan" => s.draw.tool == notecraft_engine::DrawTool::Pan,
        "draw.eraser" => s.draw.tool == notecraft_engine::DrawTool::Eraser,
        "draw.eraserSmall" => s.draw.eraser == notecraft_engine::EraserKind::Small,
        "draw.eraserMedium" => s.draw.eraser == notecraft_engine::EraserKind::Medium,
        "draw.eraserLarge" => s.draw.eraser == notecraft_engine::EraserKind::Large,
        "draw.eraserStroke" => s.draw.eraser == notecraft_engine::EraserKind::Stroke,
        "object.setAsBackground" => match (&s.sel.on_page, s.page()) {
            (notecraft_engine::selection::PageSel::Objects { ids }, Some(p)) => ids.iter().any(|id| p.image(*id).is_some_and(|im| im.background)),
            _ => false,
        },
        _ => false,
    }
}

/// The ribbon under the tab row (`crate::chrome::tab_row`): a floating rounded card (inset from
/// the window edges, with a hairline and a shadow) holding the selected tab in the simplified
/// one-row layout (`ribbon_simplified`, the default) or the classic one with labelled groups, and
/// the ▾ chevron at its bottom right with the layout and display options (catalog
/// `UI.Ribbon.ModeSwitch`, `UI.Ribbon.DisplayOptions`, `UI.Ribbon.Simplified`).
pub fn show(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    if app.session.view.ribbon_collapsed {
        return;
    }
    let Some(tab) = crate::chrome::current_tab(app) else { return };
    let simplified = app.session.view.ribbon_mode == RibbonMode::Simplified;
    let card_h = if simplified { size::RIBBON_SIMPLIFIED } else { BODY_H };
    let m = size::CARD_MARGIN as i8;
    egui::Panel::top("ribbon_body")
        .show_separator_line(false)
        .exact_size(card_h + 2.0 + size::CARD_MARGIN + 2.0)
        .frame(Frame::NONE.fill(t.window_back).inner_margin(Margin { left: m, right: m, top: 2, bottom: m + 2 }))
        .show(ui, |ui| {
            let card = Rect::from_min_size(ui.max_rect().min, vec2(ui.max_rect().width(), card_h));
            ui.painter().add(t.card_shadow().as_shape(card, radius::CARD));
            ui.painter().rect(card, radius::CARD, t.ribbon_card, Stroke::new(1.0, t.card_border), egui::StrokeKind::Inside);
            let chevron = Rect::from_min_size(pos2(card.max.x - 26.0, card.max.y - 24.0), vec2(20.0, 20.0));
            let content = Rect::from_min_max(card.min + vec2(6.0, 3.0), pos2(chevron.min.x - 4.0, card.max.y - 3.0));
            let align = if simplified { egui::Align::Center } else { egui::Align::Min };
            ui.scope_builder(egui::UiBuilder::new().max_rect(content).layout(egui::Layout::left_to_right(align)), |ui| {
                ui.set_clip_rect(content.expand(2.0));
                if simplified {
                    crate::ribbon_simplified::show(app, ui, tab, content, t);
                } else {
                    egui::ScrollArea::horizontal()
                        .id_salt("ribbon_scroll")
                        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                                for g in tab.groups {
                                    group(app, ui, g, t);
                                }
                            });
                        });
                }
            });
            display_options(app, ui, chevron, t);
        });
}

/// The ▾ chevron at the card's bottom right: Ribbon layout (single-line or classic) and Show
/// ribbon (always, or tabs only).
fn display_options(app: &mut NoteApp, ui: &mut Ui, r: Rect, t: Tokens) {
    let ctx = ui.ctx().clone();
    let resp = ui.interact(r, ui.id().with("ribbon_options"), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Ribbon display options"));
    if resp.hovered() {
        ui.painter().rect_filled(r, radius::CONTROL, t.hover);
    }
    icons::chevron(ui.painter(), r.shrink(3.0), icons::Dir::Down, t.icon);
    let resp = resp.on_hover_text("Ribbon display options: single-line or classic layout");
    Popup::menu(&resp).show(|ui| {
        ui.set_min_width(200.0);
        let simplified = app.session.view.ribbon_mode == RibbonMode::Simplified;
        ui.label(RichText::new("Ribbon layout").size(TypeRung::Caption.size()).color(t.text_dim));
        for (label, mode, on) in [("Single-line ribbon", "simplified", simplified), ("Classic ribbon", "classic", !simplified)] {
            if ui.add(egui::Button::new(label).selected(on).frame(false)).clicked() {
                app.run(&ctx, "ui.ribbonMode", json!({"mode": mode}));
                ui.close();
            }
        }
        ui.separator();
        ui.label(RichText::new("Show ribbon").size(TypeRung::Caption.size()).color(t.text_dim));
        let collapsed = app.session.view.ribbon_collapsed;
        for (label, want_collapsed) in [("Always show", false), ("Show tabs only", true)] {
            if ui.add(egui::Button::new(label).selected(collapsed == want_collapsed).frame(false)).clicked() {
                if collapsed != want_collapsed {
                    app.run(&ctx, "ui.collapseRibbon", json!({}));
                }
                ui.close();
            }
        }
        ui.add_enabled(false, egui::Button::new("Automatically hide").frame(false)).on_disabled_hover_text("Not implemented yet in NoteCraft");
    });
}

fn group(app: &mut NoteApp, ui: &mut Ui, g: &Group, t: Tokens) {
    let start = ui.cursor().min;
    ui.vertical(|ui| {
        ui.set_height(BODY_H - 6.0);
        ui.horizontal(|ui| {
            ui.set_height(CONTENT_H);
            ui.add_space(3.0);
            for it in g.items {
                item(app, ui, it, t);
                ui.add_space(2.0);
            }
            ui.add_space(3.0);
        });
        let w = ui.min_rect().width().max(40.0);
        let (lr, _) = ui.allocate_exact_size(vec2(w, LABEL_H), Sense::hover());
        ui.painter().text(lr.center(), egui::Align2::CENTER_CENTER, g.name, egui::FontId::proportional(TypeRung::Caption.size()), t.text_dim);
    });
    let end = ui.cursor().min;
    let x = end.x + 1.0;
    ui.painter().line_segment([pos2(x, start.y + 4.0), pos2(x, start.y + BODY_H - 10.0)], Stroke::new(1.0, t.border));
    ui.add_space(3.0);
}

fn item(app: &mut NoteApp, ui: &mut Ui, it: &Item, t: Tokens) {
    match *it {
        Item::Large(id) => {
            large(app, ui, id, &[], t);
        }
        Item::LargeMenu(id, kids) => {
            large(app, ui, id, kids, t);
        }
        Item::Stack(ids) => {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
                for id in ids {
                    small(app, ui, id, &[], true, t);
                }
            });
        }
        Item::Row(ids) => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for id in ids {
                    small(app, ui, id, &[], false, t);
                }
            });
        }
        Item::SmallMenu(id, kids) => {
            small(app, ui, id, kids, false, t);
        }
        Item::Combo(id, w) => combo(app, ui, id, w, t),
        Item::Gallery(id, _, extra) if id == crate::ink::PENS_ID => crate::ink::gallery(app, ui, CONTENT_H, extra, t),
        Item::Gallery(id, items, extra) => gallery(app, ui, id, items, extra, t),
        Item::Column(items) => {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
                for x in items {
                    item(app, ui, x, t);
                }
            });
        }
        Item::Small(id) => small(app, ui, id, &[], true, t),
        Item::LabeledMenu(id, kids) => small(app, ui, id, kids, true, t),
        Item::TwoRows(a, b) => {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 4.0);
                ui.add_space(4.0);
                for rowi in [a, b] {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                        for x in rowi {
                            item(app, ui, x, t);
                        }
                    });
                }
            });
        }
    }
}

/// Tooltip: label, shortcut, why it is disabled, catalog id (UI.Ribbon.Tooltips).
pub(crate) fn tooltip(app: &NoteApp, id: &str) -> String {
    let mut s = label(id);
    if let Some(r) = row(id)
        && !r.shortcut.is_empty()
    {
        s.push_str(&format!(" ({})", r.shortcut.replace("Mod", "Ctrl")));
    }
    let reason = match command(id) {
        Some(c) => app.disabled_reason(&c),
        None => {
            if row(id).is_some_and(|r| r.status == "out-of-scope") {
                Some("Needs a Microsoft service; not available in NoteCraft".into())
            } else {
                Some("Not available yet in NoteCraft".into())
            }
        }
    };
    if let Some(r) = reason {
        s.push_str(&format!("\n{r}"));
    }
    s
}

pub(crate) fn enabled(app: &NoteApp, id: &str) -> bool {
    match command(id) {
        Some(c) => app.disabled_reason(&c).is_none(),
        None => false,
    }
}

/// Controls that open a dialog to collect their command's parameters.
pub(crate) fn dialog_for(app: &NoteApp, id: &str) -> Option<crate::dialogs::Dialog> {
    use crate::dialogs::Dialog;
    match id.rsplit('.').next()? {
        "HyperlinkCreateHere" | "HyperlinkCreate" => Some(Dialog::Link { url: String::new(), text: String::new(), filter: String::new() }),
        // Edit Link… starts from the link under the caret or on the selected object.
        "LinkEdit" => Some(Dialog::Link { url: app.session.current_link().unwrap_or_default(), text: String::new(), filter: String::new() }),
        "TableInsert" => Some(Dialog::InsertTable(2, 2)),
        "SymbolInsertGallery" | "SymbolsDialog" => Some(Dialog::Symbol),
        "ParagraphSpacingOptions" => {
            let p = app.session.text_range().and_then(|r| {
                app.session.page().and_then(|pg| notecraft_engine::Session::block_paras(pg, r.head.block).and_then(|ps| ps.get(r.head.para).cloned()))
            });
            Some(p.map(|p| Dialog::Spacing(p.space_before, p.space_after, p.line_spacing)).unwrap_or(Dialog::Spacing(0.0, 0.0, 0.0)))
        }
        "TagsCustomize" => Some(Dialog::CustomizeTags(String::new())),
        "AlternativeText" => {
            let page = app.session.page()?;
            let notecraft_engine::selection::PageSel::Objects { ids } = &app.session.sel.on_page else { return None };
            let text =
                ids.iter().find_map(|id| page.image(*id).map(|im| im.alt_text.clone()).or_else(|| page.attachment(*id).map(|a| a.alt_text.clone())));
            Some(Dialog::AltText(text.unwrap_or_default()))
        }
        "RenameSelected" => {
            let page = app.session.page()?;
            let notecraft_engine::selection::PageSel::Objects { ids } = &app.session.sel.on_page else { return None };
            ids.iter().find_map(|id| page.attachment(*id)).map(|a| Dialog::RenameAttachment(a.name.clone()))
        }
        "EmptyRecycleBin" => Some(Dialog::ConfirmEmptyBin),
        "MoreColorAndThicknessOptions" => {
            let pen = app.session.draw.current_pen();
            Some(Dialog::ColorThickness {
                highlighter: pen.tool == notecraft_engine::doc::InkTool::Highlighter,
                color: pen.color.to_hex(),
                width: pen.width,
            })
        }
        _ => None,
    }
}

pub(crate) fn fire(app: &mut NoteApp, ui: &Ui, id: &str) {
    if let Some(d) = dialog_for(app, id) {
        app.ui.dialog = Some(d);
        return;
    }
    if let Some(c) = command(id) {
        let ctx = ui.ctx().clone();
        if crate::pictures::host_action(app, &ctx, &c) {
            return;
        }
        app.run(&ctx, &c, params_for(id));
    }
}

fn large(app: &mut NoteApp, ui: &mut Ui, id: &str, kids: &[&str], t: Tokens) {
    let lab = label(id);
    let font = egui::FontId::proportional(11.5);
    let tw = ui.fonts_mut(|f| f.layout(lab.clone(), font.clone(), Color32::WHITE, 70.0).size().x);
    let w = (tw + 10.0).max(42.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, CONTENT_H), Sense::click());
    let on = enabled(app, id) || !kids.is_empty() && kids.iter().any(|k| enabled(app, k)) || special_menu(id);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, on, &lab));
    let is_checked = command(id).is_some_and(|c| checked(app, &c));
    let p = ui.painter();
    if is_checked {
        p.rect_filled(rect, crate::theme::radius::CONTROL, t.checked);
    } else if resp.hovered() && on {
        p.rect_filled(rect, crate::theme::radius::CONTROL, t.hover);
    }
    let color = if on { t.text } else { t.text_disabled };
    let icon = Rect::from_center_size(pos2(rect.center().x, rect.min.y + 22.0), vec2(30.0, 30.0));
    let (line, accent) = if on { (t.icon, t.accent) } else { (t.text_disabled, t.text_disabled) };
    icons::draw(p, icon, id, line, accent, t.ribbon_card);
    let has_menu = !kids.is_empty() || special_menu(id);
    let galley = ui.fonts_mut(|f| f.layout(lab.clone(), font.clone(), color, w - 4.0));
    let gsize = galley.size();
    let last_row = galley.rows.last().map(|r| (r.rect().width(), r.rect().center().y)).unwrap_or((gsize.x, gsize.y / 2.0));
    let origin = pos2(rect.center().x - gsize.x / 2.0, rect.min.y + 42.0);
    p.galley(origin, galley, color);
    if has_menu {
        // One-line labels get the arrow below; two-line labels get it after the last line.
        let at = if gsize.y > 18.0 {
            pos2(rect.center().x + last_row.0 / 2.0 + 5.0, origin.y + last_row.1)
        } else {
            pos2(rect.center().x, origin.y + gsize.y + 5.0)
        };
        icons::triangle(p, at, 7.0, icons::Dir::Down, color);
    }
    let resp = resp.on_hover_text(tooltip(app, id));
    if kids.is_empty() && !special_menu(id) {
        if resp.clicked() && on {
            fire(app, ui, id);
        }
        return;
    }
    // Split/menu: a click on the icon half runs the main command when it has one; the drop-down
    // lists the children.
    let main_runs = enabled(app, id) && !matches!(id.rsplit('.').next(), Some("PasteGallery"));
    let icon_half = resp.interact_pointer_pos().is_some_and(|p| p.y < rect.min.y + 38.0);
    if resp.clicked() && main_runs && icon_half && !special_menu(id) {
        fire(app, ui, id);
        return;
    }
    Popup::menu(&resp).show(|ui| {
        ui.set_min_width(180.0);
        special(app, ui, id, t);
        for k in kids {
            menu_item(app, ui, k, t);
        }
    });
}

/// Controls whose drop-down has its own content (colour grids, galleries).
pub(crate) fn special_menu(id: &str) -> bool {
    matches!(
        id.rsplit('.').next(),
        Some(
            "PageColorGallery"
                | "RuleLinesMenu"
                | "TextHighlightColorPicker"
                | "FontColorPicker"
                | "BulletsGallery"
                | "NumberingGallery"
                | "ParagraphAlignmentMenu"
                | "ExtendedTextFormattingMenu"
                | "TableInsertGallery"
                | "ShadingColorPicker"
        )
    )
}

/// Columns × rows of the Insert › Table size grid (unverified: the reference's grid size).
const TABLE_GRID: (usize, usize) = (10, 8);

/// The Insert › Table size grid: hover picks columns × rows, a click inserts the table below the
/// caret's container (catalog `R.Insert.Tables.TableInsertGallery`).
fn table_grid(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    let cell = 18.0;
    let (cols, rows) = TABLE_GRID;
    let label_h = 20.0;
    let size = vec2(cols as f32 * cell + 4.0, rows as f32 * cell + label_h + 4.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let grid = Rect::from_min_size(rect.min + vec2(2.0, label_h + 2.0), vec2(cols as f32 * cell, rows as f32 * cell));
    let pick = resp.hover_pos().filter(|p| grid.contains(*p)).map(|p| {
        let c = (((p.x - grid.min.x) / cell) as usize).min(cols - 1) + 1;
        let r = (((p.y - grid.min.y) / cell) as usize).min(rows - 1) + 1;
        (c, r)
    });
    let painter = ui.painter();
    let caption = match pick {
        Some((c, r)) => format!("{c}x{r} Table"),
        None => "Insert Table".to_string(),
    };
    painter.text(
        rect.min + vec2(4.0, 2.0),
        Align2::LEFT_TOP,
        caption,
        crate::theme::TypeRung::Body.font(&ctx, crate::theme::Weight::Regular),
        t.text,
    );
    for r in 0..rows {
        for c in 0..cols {
            let sq = Rect::from_min_size(grid.min + vec2(c as f32 * cell, r as f32 * cell), vec2(cell - 3.0, cell - 3.0));
            let on = pick.is_some_and(|(pc, pr)| c < pc && r < pr);
            painter.rect_filled(sq, 1.0, if on { t.accent.gamma_multiply(0.25) } else { t.page });
            painter.rect_stroke(sq, 1.0, Stroke::new(1.0, if on { t.accent } else { t.border }), egui::StrokeKind::Inside);
        }
    }
    let resp = resp.on_hover_text("Insert a table of this size");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Table size grid"));
    if resp.clicked()
        && let Some((c, r)) = pick
    {
        let mut params = json!({"rows": r, "cols": c});
        if let Some(pt) = table_spot(app) {
            params["x"] = json!(pt.x);
            params["y"] = json!(pt.y);
        }
        app.run(&ctx, "insert.table", params);
        app.canvas.focused = true;
        ui.close();
    }
}

/// Where a new table goes when the caret is in a container: below the container as last laid
/// out (the engine has no layout and only estimates).
pub(crate) fn table_spot(app: &NoteApp) -> Option<notecraft_engine::doc::geom::Point> {
    let c = app.session.sel.caret()?;
    let notecraft_engine::selection::Block::Outline(id) = c.block else { return None };
    let o = app.canvas.outlines.iter().find(|o| o.id == id)?;
    let pos = app.session.page()?.outline(id)?.pos;
    let bottom = app.canvas.to_page(o.rect.max).y;
    Some(notecraft_engine::doc::geom::Point::new(pos.x, bottom + 8.0))
}

/// Draw the special drop-down content; returns true when drawn.
pub(crate) fn special(app: &mut NoteApp, ui: &mut Ui, id: &str, _t: Tokens) -> bool {
    let ctx = ui.ctx().clone();
    match id.rsplit('.').next() {
        Some("TableInsertGallery") => {
            table_grid(app, ui, _t);
            ui.separator();
            false
        }
        Some("ShadingColorPicker") => {
            if ui.button("No Color").clicked() {
                app.run(&ctx, "table.shading", json!({"color": null}));
                ui.close();
            }
            swatch_grid(ui, &PAGE_COLORS.iter().map(|s| (s.name, s.color)).collect::<Vec<_>>(), 4, |c| {
                app.run(&ctx, "table.shading", json!({"color": c.to_hex()}));
            });
            true
        }
        Some("PageColorGallery") => {
            if ui.button("No Color").clicked() {
                app.run(&ctx, "view.pageColor", json!({"color": null}));
                ui.close();
            }
            swatch_grid(ui, &PAGE_COLORS.iter().map(|s| (s.name, s.color)).collect::<Vec<_>>(), 4, |c| {
                app.run(&ctx, "view.pageColor", json!({"color": c.to_hex()}));
            });
            true
        }
        Some("RuleLinesMenu") => {
            for r in RuleLines::ALL {
                let name = format!("{r:?}").replace("Grid", "Grid ");
                let kind = match r {
                    RuleLines::None => "none",
                    RuleLines::Narrow => "narrow",
                    RuleLines::College => "college",
                    RuleLines::Standard => "standard",
                    RuleLines::Wide => "wide",
                    RuleLines::GridSmall => "gridSmall",
                    RuleLines::GridMedium => "gridMedium",
                    RuleLines::GridLarge => "gridLarge",
                    RuleLines::GridVeryLarge => "gridVeryLarge",
                };
                let label = if r == RuleLines::None {
                    "None".to_string()
                } else if r.is_grid() {
                    name
                } else {
                    format!("{name} Ruled")
                };
                if ui.button(label).clicked() {
                    app.run(&ctx, "view.ruleLines", json!({"kind": kind}));
                    ui.close();
                }
            }
            ui.separator();
            false
        }
        Some("TextHighlightColorPicker") => {
            swatch_grid(ui, &HIGHLIGHT_COLORS.iter().map(|s| (s.name, s.color)).collect::<Vec<_>>(), 5, |c| {
                app.run(&ctx, "format.highlight", json!({"color": c.to_hex()}));
            });
            false
        }
        Some("FontColorPicker") => {
            if ui.button("Automatic").clicked() {
                app.run(&ctx, "format.color", json!({"color": null}));
                ui.close();
            }
            ui.label(RichText::new("Theme Colors").size(11.0));
            swatch_grid(ui, &FONT_THEME.iter().map(|s| (s.name, s.color)).collect::<Vec<_>>(), 10, |c| {
                app.run(&ctx, "format.color", json!({"color": c.to_hex()}));
            });
            let grid = font_grid();
            for shade in 0..5 {
                let row: Vec<(&str, notecraft_engine::doc::color::Rgba)> =
                    grid.iter().map(|g| ("", g.get(shade).copied().unwrap_or_default())).collect();
                swatch_grid(ui, &row, 10, |c| {
                    app.run(&ctx, "format.color", json!({"color": c.to_hex()}));
                });
            }
            ui.label(RichText::new("Standard Colors").size(11.0));
            swatch_grid(ui, &STANDARD_COLORS.iter().map(|s| (s.name, s.color)).collect::<Vec<_>>(), 10, |c| {
                app.run(&ctx, "format.color", json!({"color": c.to_hex()}));
            });
            true
        }
        Some("BulletsGallery") => {
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(200.0);
                for b in notecraft_engine::doc::Bullet::ALL {
                    if ui.add(egui::Button::new(RichText::new(b.glyph()).size(18.0)).min_size(vec2(36.0, 30.0))).clicked() {
                        app.run(&ctx, "para.bullets", json!({"bullet": format!("{b:?}").to_lowercase()}));
                        ui.close();
                    }
                }
            });
            true
        }
        Some("NumberingGallery") => {
            for (n, key) in [
                (notecraft_engine::doc::Numbering::Decimal, "decimal"),
                (notecraft_engine::doc::Numbering::DecimalParen, "decimalParen"),
                (notecraft_engine::doc::Numbering::LowerAlpha, "lowerAlpha"),
                (notecraft_engine::doc::Numbering::UpperAlpha, "upperAlpha"),
                (notecraft_engine::doc::Numbering::LowerRoman, "lowerRoman"),
                (notecraft_engine::doc::Numbering::UpperRoman, "upperRoman"),
                (notecraft_engine::doc::Numbering::LowerAlphaParen, "lowerAlphaParen"),
            ] {
                if ui.button(format!("{}  {}  {}", n.label(1), n.label(2), n.label(3))).clicked() {
                    app.run(&ctx, "para.numbering", json!({"style": key}));
                    ui.close();
                }
            }
            false
        }
        _ => false,
    }
}

pub(crate) fn swatch_grid(
    ui: &mut Ui,
    colors: &[(&str, notecraft_engine::doc::color::Rgba)],
    per_row: usize,
    mut pick: impl FnMut(notecraft_engine::doc::color::Rgba),
) {
    egui::Grid::new(ui.next_auto_id()).spacing(vec2(3.0, 3.0)).show(ui, |ui| {
        for (i, (name, c)) in colors.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
            ui.painter().rect_filled(r, 1.0, c32(*c));
            ui.painter().rect_stroke(r, 1.0, Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color), egui::StrokeKind::Inside);
            let resp = if name.is_empty() { resp } else { resp.on_hover_text(*name) };
            if resp.clicked() {
                pick(*c);
                ui.close();
            }
            if (i + 1) % per_row == 0 {
                ui.end_row();
            }
        }
    });
}

pub(crate) fn menu_item(app: &mut NoteApp, ui: &mut Ui, id: &str, t: Tokens) {
    let on = enabled(app, id);
    let mut text = RichText::new(label(id));
    if !on {
        text = text.color(t.text_disabled);
    }
    let is_checked = command(id).is_some_and(|c| checked(app, &c));
    let btn = egui::Button::new(text).selected(is_checked).frame(false).min_size(vec2(170.0, 22.0));
    let resp = ui.add_enabled(on, btn).on_hover_text(tooltip(app, id)).on_disabled_hover_text(tooltip(app, id));
    if resp.clicked() {
        fire(app, ui, id);
        ui.close();
    }
}

pub(crate) fn small(app: &mut NoteApp, ui: &mut Ui, id: &str, kids: &[&str], with_label: bool, t: Tokens) {
    let lab = label(id);
    let font = egui::FontId::proportional(11.5);
    let tw = if with_label { ui.fonts_mut(|f| f.layout_no_wrap(lab.clone(), font.clone(), Color32::WHITE).size().x) + 6.0 } else { 0.0 };
    let has_menu = !kids.is_empty() || special_menu(id);
    let w = 22.0 + tw + if has_menu { 10.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
    let on = enabled(app, id) || kids.iter().any(|k| enabled(app, k)) || special_menu(id) && app.session.page().is_some();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, on, &lab));
    let is_checked = command(id).is_some_and(|c| checked(app, &c));
    let p = ui.painter();
    if is_checked {
        p.rect_filled(rect, crate::theme::radius::CONTROL, t.checked);
    } else if resp.hovered() && on {
        p.rect_filled(rect, crate::theme::radius::CONTROL, t.hover);
    }
    let color = if on { t.text } else { t.text_disabled };
    let (line, accent) = if on { (t.icon, t.accent) } else { (t.text_disabled, t.text_disabled) };
    icons::draw(p, Rect::from_min_size(rect.min + vec2(3.0, 3.0), vec2(16.0, 16.0)), id, line, accent, t.ribbon_card);
    if with_label {
        p.text(pos2(rect.min.x + 23.0, rect.center().y), egui::Align2::LEFT_CENTER, &lab, font, color);
    }
    if has_menu {
        icons::triangle(p, pos2(rect.max.x - 5.0, rect.center().y), 6.0, icons::Dir::Down, color);
    }
    let resp = resp.on_hover_text(tooltip(app, id));
    if !has_menu {
        if resp.clicked() && on {
            fire(app, ui, id);
        }
        return;
    }
    let arrow = resp.interact_pointer_pos().is_some_and(|p| p.x > rect.max.x - 11.0);
    if resp.clicked() && !arrow && enabled(app, id) {
        fire(app, ui, id);
        return;
    }
    Popup::menu(&resp).show(|ui| {
        ui.set_min_width(170.0);
        special(app, ui, id, t);
        for k in kids {
            menu_item(app, ui, k, t);
        }
    });
}

pub(crate) fn combo(app: &mut NoteApp, ui: &mut Ui, id: &str, w: f32, t: Tokens) {
    let ctx = ui.ctx().clone();
    let on = enabled(app, id);
    let key = id.rsplit('.').next().unwrap_or("");
    // The value shown: the format at the caret, or the zoom.
    let cur = current_format(app);
    let value = match key {
        "Font" => cur.as_ref().and_then(|f| f.font.clone()).unwrap_or_else(|| app.session.prefs.default_font.clone()),
        "FontSize" => cur.as_ref().and_then(|f| f.size).map(fmt_size).unwrap_or_else(|| fmt_size(app.session.prefs.default_size)),
        "ZoomClassic" => format!("{:.0}%", app.session.view.zoom * 100.0),
        _ => String::new(),
    };
    let resp = ui.add_enabled_ui(on, |ui| {
        egui::ComboBox::from_id_salt(id).width(w).selected_text(RichText::new(value).size(12.0)).show_ui(ui, |ui| match key {
            "Font" => {
                for f in
                    ["Calibri", "Calibri Light", "Arial", "Cambria", "Consolas", "Courier New", "Georgia", "Segoe UI", "Times New Roman", "Verdana"]
                {
                    if ui.selectable_label(false, f).clicked() {
                        app.run(&ctx, "format.font", json!({"font": f}));
                    }
                }
            }
            "FontSize" => {
                for s in notecraft_engine::text::FONT_SIZES {
                    if ui.selectable_label(false, fmt_size(s)).clicked() {
                        app.run(&ctx, "format.size", json!({"size": s}));
                    }
                }
            }
            "ZoomClassic" => {
                for z in [50, 75, 100, 125, 150, 200, 300, 400] {
                    if ui.selectable_label(false, format!("{z}%")).clicked() {
                        app.run(&ctx, "view.zoom", json!({"percent": z}));
                    }
                }
            }
            _ => {}
        })
    });
    let _ = (resp, t);
}

fn fmt_size(s: f32) -> String {
    if (s - s.round()).abs() < 1e-3 { format!("{}", s.round() as i32) } else { format!("{s:.1}") }
}

fn current_format(app: &NoteApp) -> Option<notecraft_engine::doc::CharFormat> {
    let s = &app.session;
    let r = s.text_range()?;
    let page = s.page()?;
    let p = notecraft_engine::Session::block_paras(page, r.head.block)?.get(r.head.para)?;
    Some(s.pending_fmt.clone().unwrap_or_else(|| p.fmt_at(r.ordered().0.offset)).over(&p.style.def().char))
}

/// In-ribbon gallery: Styles (text samples) and Tags (icon + name) — catalog `UI.Ribbon.Galleries`.
fn gallery(app: &mut NoteApp, ui: &mut Ui, id: &str, items: &[&str], extra: &[&str], t: Tokens) {
    let ctx = ui.ctx().clone();
    let is_tags = id.ends_with("TagsGallery");
    let is_styles = id.ends_with("QuickStylesGallery");
    let width = if is_tags {
        210.0
    } else if is_styles {
        168.0
    } else {
        120.0
    };
    let frame_rect = ui.allocate_exact_size(vec2(width, CONTENT_H), Sense::hover()).0;
    ui.painter().rect_stroke(frame_rect, 2.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    let on = app.disabled_reason(command(id).as_deref().unwrap_or("")).is_none();
    let inner = frame_rect.shrink(2.0);
    let visible = if is_tags {
        4
    } else if is_styles {
        3
    } else {
        0
    };
    let row_h = (inner.height() / visible.max(1) as f32).floor();
    let defs = app.session.tag_defs();
    for (i, item) in items.iter().take(visible).enumerate() {
        let r = Rect::from_min_size(pos2(inner.min.x, inner.min.y + i as f32 * row_h), vec2(inner.width() - 16.0, row_h));
        let resp = ui.interact(r, ui.id().with((id, i)), Sense::click());
        let enabled_item = command(item).map(|c| app.disabled_reason(&c).is_none()).unwrap_or(on);
        if resp.hovered() && enabled_item {
            ui.painter().rect_filled(r, 1.0, t.hover);
        }
        let color = if enabled_item { t.text } else { t.text_disabled };
        if is_styles {
            let style = ParaStyle::from_name(&label(item)).unwrap_or_default();
            let d = style.def();
            let size = d.char.size.unwrap_or(11.0).clamp(9.0, 15.0);
            let c = d.char.color.map(c32).unwrap_or(color);
            ui.painter().text(
                r.left_center() + vec2(4.0, 0.0),
                egui::Align2::LEFT_CENTER,
                d.name,
                egui::FontId::proportional(size),
                if enabled_item { c } else { t.text_disabled },
            );
        } else {
            let lab = label(item);
            if let Some(def) = defs.iter().find(|d| d.name == lab) {
                icons::tag(ui.painter(), Rect::from_min_size(r.min + vec2(3.0, (row_h - 13.0) / 2.0), vec2(13.0, 13.0)), def.symbol, false, color);
            }
            ui.painter().text(r.left_center() + vec2(21.0, 0.0), egui::Align2::LEFT_CENTER, &lab, egui::FontId::proportional(11.5), color);
        }
        let resp = resp.on_hover_text(tooltip(app, item));
        if resp.clicked() && enabled_item {
            gallery_pick(app, &ctx, id, item);
        }
    }
    // The "more" button opens the full gallery with its extra commands.
    let more = Rect::from_min_max(pos2(inner.max.x - 14.0, inner.min.y), inner.max);
    let mresp = ui.interact(more, ui.id().with((id, "more")), Sense::click());
    if mresp.hovered() {
        ui.painter().rect_filled(more, 1.0, t.hover);
    }
    icons::triangle(ui.painter(), more.center(), 7.0, icons::Dir::Down, t.text);
    Popup::menu(&mresp).show(|ui| gallery_menu(app, ui, id, items, extra, t));
}

/// The full gallery in a drop-down: every item (tags with their symbol and shortcut), then the
/// gallery's extra commands. Used by the classic gallery's ▾ and the simplified ribbon.
pub(crate) fn gallery_menu(app: &mut NoteApp, ui: &mut Ui, id: &str, items: &[&str], extra: &[&str], t: Tokens) {
    let ctx = ui.ctx().clone();
    let is_tags = id.ends_with("TagsGallery");
    let on = app.disabled_reason(command(id).as_deref().unwrap_or("")).is_none();
    let defs = app.session.tag_defs();
    ui.set_min_width(220.0);
    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
        for item in items {
            let lab = label(item);
            let enabled_item = command(item).map(|c| app.disabled_reason(&c).is_none()).unwrap_or(on);
            let resp = ui.horizontal(|ui| {
                if is_tags && let Some(def) = defs.iter().find(|d| d.name == lab) {
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    icons::tag(ui.painter(), r, def.symbol, false, t.text);
                }
                let shortcut = row(item).map(|r| r.shortcut.replace("Mod", "Ctrl")).unwrap_or_default();
                ui.add_enabled(
                    enabled_item,
                    egui::Button::new(format!("{lab}{}", if shortcut.is_empty() { String::new() } else { format!("    {shortcut}") })).frame(false),
                )
            });
            if resp.inner.clicked() {
                gallery_pick(app, &ctx, id, item);
                ui.close();
            }
        }
        if !extra.is_empty() {
            ui.separator();
            for e in extra {
                menu_item(app, ui, e, t);
            }
        }
    });
}

pub(crate) fn gallery_pick(app: &mut NoteApp, ctx: &egui::Context, gallery_id: &str, item: &str) {
    if let Some(c) = command(item) {
        app.run(ctx, &c, json!({}));
        return;
    }
    // Gallery items without a dedicated command go through the gallery's command with a value.
    if gallery_id.ends_with("TagsGallery") {
        let lab = label(item);
        if let Some(def) = app.session.tag_defs().into_iter().find(|d| d.name == lab) {
            app.run(ctx, "tag.toggle", json!({"key": def.key}));
        }
    }
}
