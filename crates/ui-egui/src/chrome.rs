//! The window chrome of the 2024 look (decision D1, `docs/ui-parity.md` §3 rows 1–5): the title
//! bar — app mark, Quick Access Toolbar, title, the centred Search, account — and the tab row —
//! the tabs with the accent underline, the collapse arrow and Share.
//!
//! Adapted from WordCraft's `chrome.rs` and `ribbon.rs` (same team, MIT/Apache). The native
//! window frame stays (Linux compositors draw it); dragging the title bar moves the window and a
//! double-click maximizes it, as on a client-side title bar.

use egui::{Align, Align2, Frame, Layout, Margin, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

use crate::icons::{self, Dir};
use crate::ribbon_layout::{CONTEXTUAL, TABS, Tab};
use crate::theme::{TypeRung, Weight, radius, size};
use crate::{NoteApp, Tokens};

/// Id of the title-bar Search box (Alt+Q focuses it).
pub const SEARCH_ID: &str = "title_search";

/// A square icon button: hover/pressed background, accessibility label, ScreenTip. Returns the
/// response and the colour to draw the icon in (dimmed when disabled).
pub fn icon_button(ui: &mut Ui, side: f32, label: &str, enabled: bool, t: &Tokens) -> (egui::Response, Rect, egui::Color32) {
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    if enabled && resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, radius::CONTROL, t.pressed);
    } else if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, radius::CONTROL, t.hover);
    }
    let color = if enabled { t.icon } else { t.text_disabled };
    (resp, rect, color)
}

/// The window title: the page, then the notebook.
pub fn window_title(app: &NoteApp) -> String {
    match (app.session.page(), app.session.notebook()) {
        (Some(p), Some(nb)) => format!("{} - {}", p.display_title(), nb.name),
        (None, Some(nb)) => nb.name.clone(),
        _ => "NoteCraft".to_string(),
    }
}

/// The title bar (catalog `UI.Window.TitleBar`, `UI.Window.QAT`, `UI.Window.SearchBox`,
/// `UI.Window.Account`).
pub fn title_bar(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    egui::Panel::top("title_bar")
        .show_separator_line(false)
        .exact_size(size::TITLE_BAR)
        .frame(Frame::NONE.fill(t.window_back).inner_margin(Margin { left: 10, right: 10, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            let full = ui.max_rect();
            // Registered first so every control drawn on top wins the hit test.
            let drag = ui.interact(full, ui.id().with("title_drag"), Sense::click_and_drag());
            if drag.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
            let mut title_end = full.min.x;
            let mut right_start = full.max.x;
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                let (mark, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                icons::app_mark(ui.painter(), mark, t.accent, t.on_accent);
                ui.add_space(8.0);
                quick_access(app, ui, &t);
                ui.add_space(10.0);
                let title = window_title(app);
                let font = TypeRung::Title.font(&ctx, Weight::Regular);
                let galley = ui.painter().layout_no_wrap(title, font, t.text);
                let text = galley.text().to_string();
                let (r, resp) = ui.allocate_exact_size(galley.size(), Sense::hover());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text));
                ui.painter().galley(r.min, galley, t.text);
                title_end = r.max.x;
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    account(app, ui, &t);
                    right_start = ui.min_rect().min.x;
                });
            });
            // The Search box, centred in the window, kept clear of the title and the account.
            let want = (full.width() * 0.32).clamp(260.0, 440.0);
            let lo = title_end + 16.0;
            let hi = right_start - 16.0;
            let mut x0 = full.center().x - want / 2.0;
            let mut x1 = full.center().x + want / 2.0;
            if x0 < lo {
                x0 = lo;
            }
            if x1 > hi {
                x1 = hi;
            }
            if x1 - x0 >= 140.0 {
                let r = Rect::from_center_size(pos2((x0 + x1) / 2.0, full.center().y), vec2(x1 - x0, size::SEARCH_HEIGHT));
                search(app, ui, r, &t);
            }
        });
}

/// The Quick Access Toolbar (`qat`): Back and Undo by default, and the ▾ customize menu.
fn quick_access(app: &mut NoteApp, ui: &mut Ui, t: &Tokens) {
    crate::qat::toolbar(app, ui, t);
}

/// The account circle: the author's initials. Signing in needs a Microsoft account, so it is
/// disabled (catalog `UI.Window.Account`, out of scope).
fn account(app: &NoteApp, ui: &mut Ui, t: &Tokens) {
    let side = size::BUTTON;
    let (r, resp) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    let name = app.session.prefs.author.clone();
    let initials: String = name.split_whitespace().filter_map(|w| w.chars().next()).take(2).flat_map(char::to_uppercase).collect();
    let label = format!("Account: {name}");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, false, &label));
    ui.painter().circle_filled(r.center(), side / 2.0 - 1.0, t.nav_selected);
    let font = crate::theme::font(ui.ctx(), 11.0, Weight::Semibold);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, if initials.is_empty() { "?".into() } else { initials }, font, t.text);
    resp.on_hover_text(format!("{name}\nSigning in needs a Microsoft account; set your name in File › Options."));
}

/// The rounded frame of a search box (accent when focused, darker when hovered); the whole box
/// is the click target that focuses the field `id`, and the `label` a screen reader announces.
/// Shared by the title-bar Search and Search notebooks. Returns whether the field has focus.
pub(crate) fn search_frame(ui: &mut Ui, r: Rect, t: &Tokens, id: egui::Id, label: &str) -> bool {
    let ctx = ui.ctx().clone();
    let focused = ctx.memory(|m| m.has_focus(id));
    let hovered = ui.rect_contains_pointer(r);
    let stroke = if focused {
        Stroke::new(1.0, t.accent)
    } else if hovered {
        Stroke::new(1.0, t.input_border)
    } else {
        Stroke::new(1.0, t.card_border)
    };
    ui.painter().rect(r, radius::SEARCH, t.input, stroke, egui::StrokeKind::Inside);
    let area = ui.interact(r, id.with("area"), Sense::click());
    area.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, label));
    if area.clicked() {
        ctx.memory_mut(|m| m.request_focus(id));
    }
    focused
}

/// The title-bar Search: type a command name, pick it to run it; the last row hands the text to
/// Search notebooks (catalog `UI.Window.SearchBox`, `UI.Window.TellMe`).
fn search(app: &mut NoteApp, ui: &mut Ui, r: Rect, t: &Tokens) {
    let ctx = ui.ctx().clone();
    let id = egui::Id::new(SEARCH_ID);
    search_frame(ui, r, t, id, "Search");
    icons::search(ui.painter(), Rect::from_center_size(pos2(r.min.x + 16.0, r.center().y), vec2(size::ICON, size::ICON)), t.icon);
    let mut text: String = ctx.data_mut(|d| d.get_temp::<String>(id).unwrap_or_default());
    let edit = Rect::from_min_max(pos2(r.min.x + 30.0, r.min.y + 1.0), pos2(r.max.x - 8.0, r.max.y - 1.0));
    let resp = ui.put(
        edit,
        egui::TextEdit::singleline(&mut text)
            .id(id)
            .frame(Frame::NONE)
            .hint_text(egui::RichText::new(crate::i18n::tr("Search")).color(t.text_dim))
            .text_color(t.text)
            .font(TypeRung::Body.font(&ctx, Weight::Regular))
            .vertical_align(Align::Center),
    );
    let resp = resp.on_hover_text("Search for commands, or your notes (Alt+Q)");
    if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        text.clear();
        resp.surrender_focus();
    }
    ctx.data_mut(|d| d.insert_temp(id, text.clone()));
    let q = text.trim().to_lowercase();
    if q.is_empty() || !(resp.has_focus() || resp.lost_focus()) {
        return;
    }
    let hits: Vec<&notecraft_engine::CommandSpec> = notecraft_engine::commands::all()
        .iter()
        .filter(|c| c.label.to_lowercase().contains(&q) && !c.menu.starts_with("Behaviour"))
        .take(8)
        .collect();
    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let mut close = false;
    egui::Area::new(id.with("results")).fixed_pos(r.left_bottom() + vec2(0.0, 4.0)).order(egui::Order::Foreground).show(&ctx, |ui| {
        Frame::popup(ui.style()).corner_radius(radius::MENU).show(ui, |ui| {
            ui.set_width(r.width() - 12.0);
            ui.label(egui::RichText::new("Actions").size(TypeRung::Caption.size()).color(t.text_dim));
            if hits.is_empty() {
                ui.label(egui::RichText::new("No matching commands").color(t.text_dim));
            }
            let mut first = true;
            for c in &hits {
                let enabled = (c.enabled)(&app.session).is_none();
                let label = if c.shortcut.is_empty() { c.label.to_string() } else { format!("{}    {}", c.label, c.shortcut.replace("Mod", "Ctrl")) };
                let clicked = ui.add_enabled(enabled, egui::Button::new(label).frame(false)).on_hover_text(c.menu).clicked();
                if clicked || (enter && first && enabled) {
                    app.run(&ctx, c.id, json!({}));
                    close = true;
                }
                first &= !enabled;
            }
            ui.separator();
            let notes = format!("Search notebooks for “{}”", text.trim());
            if ui.add(egui::Button::new(notes).frame(false)).clicked() || (enter && hits.iter().all(|c| (c.enabled)(&app.session).is_some())) {
                app.ui.search = text.trim().to_string();
                app.ui.search_open = true;
                ctx.memory_mut(|m| m.request_focus(egui::Id::new("search_box")));
                close = true;
            }
        });
    });
    if close {
        ctx.data_mut(|d| d.insert_temp(id, String::new()));
        ctx.memory_mut(|m| m.surrender_focus(id));
    }
}

/// The contextual tabs for the current selection (catalog `UI.Ribbon.ContextualTabs`): Table
/// Layout while the caret or the selection is in a table. Equation and Audio & Video follow
/// with their objects. A contextual tab named in the UI state without its selection falls back
/// to Home rather than showing tools for nothing.
pub fn contextual_tabs(app: &NoteApp) -> Vec<&'static Tab> {
    let live = |id: &str| notecraft_engine::commands::find(id).is_some_and(|c| (c.enabled)(&app.session).is_none());
    let (in_table, in_equation) = (live("table.selectTable"), live("equation.linear"));
    CONTEXTUAL.iter().filter(|t| t.name == "Table Layout" && in_table || t.name == "Equation" && in_equation).collect()
}

/// The tab whose body the ribbon shows: the selected one if visible, else Home.
pub fn current_tab(app: &NoteApp) -> Option<&'static Tab> {
    let ctx_tabs = contextual_tabs(app);
    TABS.iter().chain(ctx_tabs.iter().copied()).find(|x| x.name == app.ui.tab).or_else(|| TABS.first())
}

/// The tab row (catalog `UI.Ribbon.Tabs`, `UI.Ribbon.ContextualTabs`, `UI.Ribbon.Collapse`,
/// `UI.Window.Share`).
pub fn tab_row(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    let contextual = contextual_tabs(app);
    egui::Panel::top("tab_row")
        .show_separator_line(false)
        .exact_size(size::TAB_ROW)
        .frame(Frame::NONE.fill(t.window_back).inner_margin(Margin { left: 4, right: 10, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                if tab(ui, "File", false, false, &t).clicked() {
                    app.ui.backstage = Some(crate::Backstage::Info);
                }
                let selected = current_tab(app).map(|x| x.name).unwrap_or("Home");
                for (x, is_ctx) in TABS.iter().map(|x| (x, false)).chain(contextual.iter().map(|x| (*x, true))) {
                    let sel = selected == x.name && !app.session.view.ribbon_collapsed;
                    let r = tab(ui, &crate::i18n::tr(x.name), sel, is_ctx, &t);
                    if r.clicked() {
                        app.session.view.ribbon_collapsed = false;
                        app.ui.tab = x.name.to_string();
                    }
                    if r.double_clicked() {
                        app.run(&ctx, "ui.collapseRibbon", json!({}));
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                    share(ui, &t);
                    let collapsed = app.session.view.ribbon_collapsed;
                    let label = if collapsed { "Expand the Ribbon (Ctrl+F1)" } else { "Collapse the Ribbon (Ctrl+F1)" };
                    let (resp, rect, color) = icon_button(ui, 24.0, label, true, &t);
                    icons::chevron(ui.painter(), rect.shrink(4.0), if collapsed { Dir::Down } else { Dir::Up }, color);
                    if resp.on_hover_text(label).clicked() {
                        app.run(&ctx, "ui.collapseRibbon", json!({}));
                    }
                });
            });
        });
}

/// One tab: semibold with a 3 pt accent underline when selected; contextual tabs in accent text.
fn tab(ui: &mut Ui, name: &str, selected: bool, contextual: bool, t: &Tokens) -> egui::Response {
    let ctx = ui.ctx().clone();
    // Width from the semibold face so selecting a tab doesn't move its neighbours.
    let bold = TypeRung::Tab.font(&ctx, Weight::Semibold);
    let font = if selected { bold.clone() } else { TypeRung::Tab.font(&ctx, Weight::Regular) };
    let color = if contextual { t.accent_text } else { t.text };
    let w = ui.painter().layout_no_wrap(name.to_string(), bold, color).size().x;
    let (rect, resp) = ui.allocate_exact_size(vec2(w + 20.0, size::TAB_ROW), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, name));
    let p = ui.painter();
    if resp.hovered() && !selected {
        p.rect_filled(rect.shrink2(vec2(2.0, 4.0)), radius::CONTROL, t.hover);
    }
    p.text(rect.center() - vec2(0.0, 1.0), Align2::CENTER_CENTER, name, font, color);
    if selected {
        let line = Rect::from_center_size(pos2(rect.center().x, rect.max.y - 4.5), vec2(w, size::TAB_UNDERLINE));
        p.rect_filled(line, egui::CornerRadius::same(2), t.accent);
    }
    resp
}

/// Share: accent-filled, disabled (sharing needs OneDrive; catalog `UI.Window.Share`).
fn share(ui: &mut Ui, t: &Tokens) {
    let font = crate::theme::font(ui.ctx(), TypeRung::Body.size(), Weight::Medium);
    let w = ui.painter().layout_no_wrap(crate::i18n::tr("Share"), font.clone(), t.on_accent).size().x + 36.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, false, "Share"));
    let fill = t.accent.gamma_multiply(0.55);
    ui.painter().rect_filled(r, radius::CONTROL, fill);
    icons::share(ui.painter(), Rect::from_center_size(pos2(r.min.x + 14.0, r.center().y), vec2(14.0, 14.0)), t.on_accent);
    ui.painter().text(pos2(r.min.x + 25.0, r.center().y), Align2::LEFT_CENTER, crate::i18n::tr("Share"), font, t.on_accent);
    resp.on_hover_text("Sharing needs OneDrive, a Microsoft service; not available in NoteCraft");
}

/// A primary (accent-filled) dialog button.
pub fn primary_button(ui: &mut Ui, label: &str, t: &Tokens) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(t.on_accent))
            .fill(t.accent)
            .stroke(Stroke::NONE)
            .corner_radius(radius::CONTROL)
            .min_size(vec2(76.0, 28.0)),
    )
}

/// A card on the window background — the panes and the find bar of the 2024 look: the card
/// colour, rounded corners, a hairline and the card shadow.
pub fn card_frame(t: &Tokens) -> Frame {
    Frame::NONE
        .fill(t.ribbon_card)
        .corner_radius(radius::CARD)
        .stroke(Stroke::new(1.0, t.card_border))
        .shadow(t.card_shadow())
        .inner_margin(Margin::same(12))
}

/// A card's title row: the title in the heading rung and a ✕ that returns true when clicked.
pub fn card_header(ui: &mut Ui, title: &str, t: &Tokens) -> bool {
    let ctx = ui.ctx().clone();
    let mut closed = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(crate::i18n::tr(title)).font(TypeRung::Heading.font(&ctx, Weight::Semibold)).color(t.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (resp, rect, color) = icon_button(ui, 24.0, "Close", true, t);
            icons::cross(ui.painter(), rect.shrink(3.0), color);
            closed = resp.on_hover_text("Close").clicked();
        });
    });
    closed
}
