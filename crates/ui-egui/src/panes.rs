//! Task panes: the Tags Summary (Find Tags), the pinned search results, and the find-on-page
//! bar (catalog `UI.Panes.FindTags`, `UI.Nav.SearchResultsPane`, `UI.Nav.FindOnPage`).

use egui::{Frame, Margin, RichText, Sense, Stroke, Ui, vec2};
use serde_json::json;

use crate::{NoteApp, Tokens, icons};

/// The Tags Summary pane: grouped tagged notes in a scope, click to go there, Create Summary
/// Page (catalog `BEH.Tags.Summary`, `BEH.Tags.SummaryPage`).
pub fn tags_pane(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    egui::Panel::right("tags_pane")
        .default_size(270.0)
        .resizable(true)
        .frame(Frame::NONE.fill(t.window_back).inner_margin(Margin::same(8)).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Tags Summary").size(15.0).strong().color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        app.session.view.tags_pane = false;
                    }
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Group tags by:");
                egui::ComboBox::from_id_salt("tags_group").selected_text(group_label(&app.ui.tags_group)).show_ui(ui, |ui| {
                    for g in ["tag", "section", "title", "date", "text"] {
                        ui.selectable_value(&mut app.ui.tags_group, g.to_string(), group_label(g));
                    }
                });
            });
            ui.checkbox(&mut app.ui.tags_unchecked, "Show only unchecked items");
            let q = json!({"group": app.ui.tags_group, "unchecked": app.ui.tags_unchecked, "scope": app.ui.tags_scope});
            let Some(res) = app.run(&ctx, "tag.findTags", q.clone()) else { return };
            ui.separator();
            egui::ScrollArea::vertical().id_salt("tags_scroll").max_height(ui.available_height() - 70.0).show(ui, |ui| {
                let groups = res["groups"].as_array().cloned().unwrap_or_default();
                if groups.is_empty() {
                    ui.label(RichText::new("No tagged notes in this scope.").color(t.text_disabled));
                }
                let defs = app.session.tag_defs();
                for g in groups {
                    ui.label(RichText::new(g["group"].as_str().unwrap_or("")).strong().color(t.text));
                    for it in g["items"].as_array().cloned().unwrap_or_default() {
                        let text = it["text"].as_str().unwrap_or("").to_string();
                        let key = it["key"].as_str().unwrap_or("").to_string();
                        let checked = it["checked"].as_bool().unwrap_or(false);
                        let resp = ui
                            .horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                if let Some(d) = defs.iter().find(|d| d.key == key) {
                                    icons::tag(ui.painter(), r, d.symbol, checked, t.text);
                                }
                                ui.label(RichText::new(if text.is_empty() { "(empty)".to_string() } else { text.clone() }).color(t.text));
                            })
                            .response
                            .interact(Sense::click());
                        if resp.clicked() {
                            let page = it["page"].as_str().unwrap_or("").to_string();
                            let block = it["block"].as_str().unwrap_or("").to_string();
                            let para = it["para"].as_u64().unwrap_or(0);
                            app.run(&ctx, "nav.goto", json!({"page": page}));
                            app.run(&ctx, "caret.set", json!({"block": block, "para": para, "offset": 0}));
                            app.canvas.focused = true;
                        }
                        resp.on_hover_text(it["pageTitle"].as_str().unwrap_or(""));
                    }
                    ui.add_space(6.0);
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Search:");
                egui::ComboBox::from_id_salt("tags_scope").selected_text(scope_label(&app.ui.tags_scope)).show_ui(ui, |ui| {
                    for s in ["page", "section", "sectionGroup", "notebook", "all"] {
                        ui.selectable_value(&mut app.ui.tags_scope, s.to_string(), scope_label(s));
                    }
                });
            });
            if ui.button("Create Summary Page").clicked() {
                app.run(&ctx, "tag.summaryPage", q);
            }
        });
}

fn group_label(g: &str) -> &'static str {
    match g {
        "section" => "Section",
        "title" => "Title",
        "date" => "Date",
        "text" => "Note Text",
        _ => "Tag name",
    }
}

fn scope_label(s: &str) -> &'static str {
    match s {
        "page" => "This page",
        "section" => "This section",
        "sectionGroup" => "This section group",
        "notebook" => "This notebook",
        _ => "All notebooks",
    }
}

/// The pinned search results (Alt+O).
pub fn search_pane(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    egui::Panel::right("search_pane")
        .default_size(260.0)
        .resizable(true)
        .frame(Frame::NONE.fill(t.window_back).inner_margin(Margin::same(8)).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Search Results").size(15.0).strong().color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        app.ui.search_pane = false;
                    }
                });
            });
            ui.text_edit_singleline(&mut app.ui.search);
            ui.separator();
            if app.ui.search.trim().is_empty() {
                return;
            }
            let q = app.ui.search.clone();
            let Some(res) = app.run(&ctx, "search.all", json!({"query": q, "limit": 500})) else { return };
            ui.label(RichText::new(format!("{} results", res["total"].as_u64().unwrap_or(0))).size(11.0));
            egui::ScrollArea::vertical().id_salt("search_pane_scroll").show(ui, |ui| {
                for it in res["results"].as_array().cloned().unwrap_or_default() {
                    let resp = ui
                        .vertical(|ui| {
                            ui.label(RichText::new(it["title"].as_str().unwrap_or("")).strong().color(t.text));
                            ui.label(RichText::new(it["section"].as_str().unwrap_or("")).size(11.0).color(t.page_meta));
                        })
                        .response
                        .interact(Sense::click());
                    if resp.clicked() {
                        app.run(&ctx, "search.openResult", json!({"page": it["page"], "query": q}));
                    }
                }
            });
        });
}

/// The find-on-page bar (Ctrl+F): query, match count, previous/next, close.
pub fn find_bar(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    Frame::NONE.fill(t.find.gamma_multiply(0.35)).inner_margin(Margin::symmetric(8, 3)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Find on page:");
            let resp = ui.add(egui::TextEdit::singleline(&mut app.ui.find).id(egui::Id::new("find_box")).desired_width(220.0));
            if resp.changed() {
                let q = app.ui.find.clone();
                app.run(&ctx, "search.page", json!({"query": q}));
            }
            if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                app.run(&ctx, "search.next", json!({}));
                resp.request_focus();
            }
            let n = app.session.view.find.matches.len();
            ui.label(if n == 0 { "No matches".to_string() } else { format!("{} of {n}", app.session.view.find.current + 1) });
            if ui.add_enabled(n > 0, egui::Button::new("Previous")).on_hover_text("Previous (Shift+F3)").clicked() {
                app.run(&ctx, "search.prev", json!({}));
            }
            if ui.add_enabled(n > 0, egui::Button::new("Next")).on_hover_text("Next (F3)").clicked() {
                app.run(&ctx, "search.next", json!({}));
            }
            if ui.button("Close").on_hover_text("Close (Esc)").clicked() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape))) {
                app.run(&ctx, "search.dismiss", json!({}));
                app.canvas.focused = true;
            }
        });
    });
}
