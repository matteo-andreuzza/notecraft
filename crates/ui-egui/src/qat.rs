//! The Quick Access Toolbar (catalog `UI.Window.QAT`, `UI.Dialogs.CustomizeQAT`,
//! `OPT.QuickAccessToolbar.CustomizeQAT`, the `QAT.*` rows): the buttons in the title bar, the
//! ▾ menu that switches the common ones on and off, and the Quick Access Toolbar page of the
//! Options dialog that adds any QAT command, removes, reorders and resets.
//!
//! The toolbar is a list of catalog ids kept in [`crate::UiState::qat`] (the desktop app
//! remembers it). A button runs its row's command the way the ribbon does (`ribbon::fire`), so
//! commands that need a file or a dialog ask for it.

use egui::{Rect, Sense, Ui, vec2};

use crate::chrome::icon_button;
use crate::theme::size;
use crate::{NoteApp, Tokens, icons};

/// The toolbar of a new installation: Back and Undo, as the reference shows.
pub const DEFAULT: [&str; 2] = ["QAT.ViewGoBack", "QAT.UndoButton"];
/// Most buttons on the toolbar.
pub const MAX_BUTTONS: usize = 40;
/// The commands the ▾ menu offers to switch on and off (the reference's short list,
/// unverified order).
pub const MENU: [&str; 8] = [
    "QAT.ViewGoBack",
    "QAT.ViewGoForward",
    "QAT.UndoButton",
    "QAT.RedoButton",
    "QAT.FilePrint",
    "QAT.FilePrintPreview",
    "QAT.NR.Spelling",
    "QAT.NR.FileExit",
];

/// Every QAT row whose command NoteCraft has, in catalog order: what the Options page offers
/// (parsed once).
pub fn available() -> &'static [String] {
    static IDS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    IDS.get_or_init(|| {
        notecraft_engine::catalog::rows()
            .into_iter()
            .filter(|r| r.id.starts_with("QAT.") && r.status != "out-of-scope" && notecraft_engine::commands::find(&r.command).is_some())
            .map(|r| r.id)
            .collect()
    })
}

/// The row's label.
pub fn label(id: &str) -> String {
    crate::ribbon::row(id).map(|r| r.label.clone()).unwrap_or_else(|| id.to_string())
}

/// The command a QAT row runs.
fn command(id: &str) -> Option<&'static str> {
    crate::ribbon::command(id)
}

/// Keep only known, working, distinct ids (a settings file is hostile input).
pub fn normalize(list: &mut Vec<String>) {
    let ok = available();
    let mut seen = std::collections::HashSet::new();
    list.retain(|id| ok.contains(id) && seen.insert(id.clone()));
    list.truncate(MAX_BUTTONS);
}

/// Run a toolbar button the way the ribbon runs its control (dialogs and file pickers
/// included).
fn press(app: &mut NoteApp, ui: &Ui, id: &str) {
    crate::ribbon::fire(app, ui, id);
}

/// The toolbar's buttons and the ▾ menu, in the title bar.
pub fn toolbar(app: &mut NoteApp, ui: &mut Ui, t: &Tokens) {
    for id in app.ui.qat.clone() {
        let Some(cmd) = command(&id) else { continue };
        let enabled = app.disabled_reason(cmd).is_none();
        // The label with its shortcut, as the reference's tooltips show ("Undo (Ctrl+Z)").
        let tip = match crate::ribbon::row(&id).map(|r| r.shortcut.trim_matches('`').replace("Mod", "Ctrl")).filter(|k| !k.is_empty()) {
            Some(k) => format!("{} ({k})", label(&id)),
            None => label(&id),
        };
        let (resp, rect, color) = icon_button(ui, size::BUTTON, &tip, enabled, t);
        let glyph = Rect::from_center_size(rect.center(), vec2(size::ICON + 2.0, size::ICON + 2.0));
        match id.as_str() {
            "QAT.ViewGoBack" => icons::back(ui.painter(), glyph, color),
            "QAT.UndoButton" => icons::undo(ui.painter(), glyph, false, color),
            "QAT.RedoButton" => icons::undo(ui.painter(), glyph, true, color),
            _ => icons::draw(ui.painter(), glyph, &id, color, if enabled { t.accent } else { color }, t.page),
        }
        if resp.on_hover_text(format!("{tip}\n{id}")).clicked() && enabled {
            press(app, ui, &id);
        }
    }
    let (rect, resp) = ui.allocate_exact_size(vec2(16.0, size::BUTTON), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Customize Quick Access Toolbar"));
    if resp.hovered() {
        ui.painter().rect_filled(rect, crate::theme::radius::CONTROL, t.hover);
    }
    icons::chevron(ui.painter(), Rect::from_center_size(rect.center(), vec2(12.0, 12.0)), icons::Dir::Down, t.icon);
    let resp = resp.on_hover_text("Customize Quick Access Toolbar");
    egui::Popup::menu(&resp).show(|ui| {
        ui.label(egui::RichText::new("Customize Quick Access Toolbar").strong());
        for id in MENU {
            if !available().iter().any(|a| a == id) {
                continue;
            }
            let mut on = app.ui.qat.iter().any(|x| x == id);
            if ui.checkbox(&mut on, label(id)).changed() {
                if on {
                    app.ui.qat.push(id.to_string());
                } else {
                    app.ui.qat.retain(|x| x != id);
                }
            }
        }
        ui.separator();
        if ui.button("More Commands…").clicked() {
            app.ui.options_page = crate::options::OptionsPage::QuickAccessToolbar;
            app.ui.backstage = Some(crate::Backstage::Options);
            ui.close();
        }
    });
}

/// The Quick Access Toolbar page of the Options dialog: changes apply at once (unverified:
/// the reference applies them with OK).
pub fn options_page(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let sel_id = egui::Id::new("qat_pick");
    let mut pick: (Option<String>, Option<String>) = ui.data_mut(|d| d.get_temp(sel_id)).unwrap_or_default();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(220.0);
            ui.label("Choose commands from:");
            egui::ScrollArea::vertical().id_salt("qat_avail").max_height(330.0).show(ui, |ui| {
                for id in available() {
                    if ui.selectable_label(pick.0.as_deref() == Some(id.as_str()), label(id)).clicked() {
                        pick.0 = Some(id.clone());
                    }
                }
            });
        });
        ui.vertical(|ui| {
            ui.add_space(120.0);
            let can_add = pick.0.as_ref().is_some_and(|p| !app.ui.qat.contains(p)) && app.ui.qat.len() < MAX_BUTTONS;
            if ui.add_enabled(can_add, egui::Button::new("Add >>")).clicked()
                && let Some(p) = pick.0.clone()
            {
                app.ui.qat.push(p.clone());
                pick.1 = Some(p);
            }
            if ui.add_enabled(pick.1.is_some(), egui::Button::new("<< Remove")).clicked()
                && let Some(p) = pick.1.take()
            {
                app.ui.qat.retain(|x| *x != p);
            }
        });
        ui.vertical(|ui| {
            ui.set_width(220.0);
            ui.label("Quick Access Toolbar:");
            egui::ScrollArea::vertical().id_salt("qat_cur").max_height(300.0).show(ui, |ui| {
                for id in app.ui.qat.clone() {
                    if ui.selectable_label(pick.1.as_deref() == Some(id.as_str()), label(&id)).clicked() {
                        pick.1 = Some(id.clone());
                    }
                }
            });
            ui.horizontal(|ui| {
                let at = pick.1.as_ref().and_then(|p| app.ui.qat.iter().position(|x| x == p));
                if ui.add_enabled(at.is_some_and(|i| i > 0), egui::Button::new("Move Up")).clicked()
                    && let Some(i) = at
                {
                    app.ui.qat.swap(i, i - 1);
                }
                if ui.add_enabled(at.is_some_and(|i| i + 1 < app.ui.qat.len()), egui::Button::new("Move Down")).clicked()
                    && let Some(i) = at
                {
                    app.ui.qat.swap(i, i + 1);
                }
                if ui.button("Reset").clicked() {
                    app.ui.qat = DEFAULT.iter().map(|s| s.to_string()).collect();
                    pick.1 = None;
                }
            });
        });
    });
    ui.label(egui::RichText::new("Changes show in the title bar at once.").size(12.0).color(t.page_meta));
    ui.data_mut(|d| d.insert_temp(sel_id, pick));
}
