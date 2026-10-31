//! The Help pane (F1) and About NoteCraft (catalog `R.Help.Help.Help`, `NC.Help.Search`,
//! `QAT.NR.HelpKeyboardShortcuts`, `BS.Account.About`). Help is local: the pane searches the
//! command registry through `help.search`, and a click on a result runs the command.

use egui::{RichText, Sense, Ui, vec2};
use serde_json::json;

use crate::chrome::{card_frame, card_header};
use crate::{NoteApp, Tokens};

/// The Help pane at the right of the page.
pub fn pane(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    egui::Panel::right("help_pane")
        .default_size(300.0)
        .resizable(true)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(t.window_back).inner_margin(egui::Margin { left: 6, right: 8, top: 6, bottom: 8 }))
        .show(ui, |ui| {
            card_frame(&t).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                let title = if app.session.view.help_shortcuts { "Keyboard Shortcuts" } else { "Help" };
                if card_header(ui, title, &t) {
                    app.session.view.help_pane = false;
                    return;
                }
                ui.add_space(4.0);
                let id = egui::Id::new("help_query");
                let mut query: String = ui.data(|d| d.get_temp(id)).unwrap_or_default();
                let l = ui.label("Search commands and shortcuts");
                ui.add(egui::TextEdit::singleline(&mut query).hint_text("bold, page color, export pdf…").desired_width(f32::INFINITY)).labelled_by(l.id);
                ui.data_mut(|d| d.insert_temp(id, query.clone()));
                ui.checkbox(&mut app.session.view.help_shortcuts, "Only commands with a shortcut");
                if query.trim().is_empty() && !app.session.view.help_shortcuts {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(
                            "Type what you want to do (\"bold\", \"page color\", \"export pdf\"): every command is listed with where it is in the ribbon and its shortcut. Click one to run it.",
                        )
                        .size(12.0)
                        .color(t.text_dim),
                    );
                    return;
                }
                let Some(res) = app.run(&ctx, "help.search", json!({"query": query, "shortcuts": app.session.view.help_shortcuts})) else { return };
                ui.separator();
                let total = res["total"].as_u64().unwrap_or(0);
                ui.label(RichText::new(format!("{total} command{}", if total == 1 { "" } else { "s" })).size(11.0).color(t.page_meta));
                egui::ScrollArea::vertical().id_salt("help_scroll").show(ui, |ui| {
                    for it in res["results"].as_array().cloned().unwrap_or_default() {
                        let label = it["label"].as_str().unwrap_or("").to_string();
                        let shortcut = it["shortcut"].as_str().unwrap_or("");
                        let resp = ui
                            .vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.add(egui::Label::new(RichText::new(&label).strong().color(t.text)).selectable(false));
                                    if !shortcut.is_empty() {
                                        ui.add(egui::Label::new(RichText::new(shortcut).size(11.0).color(t.accent_text)).selectable(false));
                                    }
                                });
                                ui.add(egui::Label::new(RichText::new(it["menu"].as_str().unwrap_or("")).size(11.0).color(t.page_meta)).selectable(false));
                                ui.add_space(4.0);
                            })
                            .response
                            .interact(Sense::click());
                        let resp = resp.on_hover_text("Click to run it");
                        if resp.clicked()
                            && let Some(cmd) = it["id"].as_str()
                        {
                            app.run(&ctx, cmd, json!({}));
                        }
                    }
                });
            });
        });
}

/// About NoteCraft: name, version, licence, and that it is not Microsoft's.
pub fn about(app: &mut NoteApp, ctx: &egui::Context, t: Tokens) {
    if !app.session.view.about {
        return;
    }
    let info = notecraft_engine::about();
    let mut open = true;
    egui::Window::new("About NoteCraft")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new("NoteCraft").size(22.0).color(t.text));
            ui.label(format!("Version {}", info["version"].as_str().unwrap_or("")));
            ui.add_space(6.0);
            ui.label("An open-source, local-first notebook. Your notes stay in plain folders on your disk; nothing is sent anywhere.");
            ui.label(format!("Licence: {}. {} commands.", info["license"].as_str().unwrap_or(""), info["commands"]));
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Not affiliated with Microsoft. OneNote is a trademark of Microsoft Corporation, named only to describe compatibility.",
                )
                .size(12.0)
                .color(t.page_meta),
            );
            ui.add_space(8.0);
            if ui.button("OK").clicked() {
                app.session.view.about = false;
            }
        });
    if !open {
        app.session.view.about = false;
    }
}
