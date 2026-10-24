//! Task panes: the Tags Summary (Find Tags), the Templates pane, the pinned search results, and
//! the find-on-page bar (catalog `UI.Panes.FindTags`, `UI.Panes.Templates`,
//! `UI.Nav.SearchResultsPane`, `UI.Nav.FindOnPage`). In the 2024
//! look (`docs/ui-parity.md` §3 rows 15–16) the panes are cards at the right of the page with a
//! title and ✕, and the find bar is a card floating over the page's top right corner.

use egui::{Frame, Margin, RichText, Sense, Ui, vec2};
use serde_json::json;

use crate::chrome::{card_frame, card_header, icon_button};
use crate::{NoteApp, Tokens, icons};

/// The right-hand strip that holds a pane card.
fn pane_panel(id: &'static str, width: f32, t: &Tokens) -> egui::Panel {
    egui::Panel::right(id)
        .default_size(width)
        .resizable(true)
        .show_separator_line(false)
        .frame(Frame::NONE.fill(t.window_back).inner_margin(Margin { left: 6, right: 8, top: 6, bottom: 8 }))
}

/// The Tags Summary pane: grouped tagged notes in a scope, click to go there, Create Summary
/// Page (catalog `BEH.Tags.Summary`, `BEH.Tags.SummaryPage`).
pub fn tags_pane(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    pane_panel("tags_pane", 280.0, &t).show(ui, |ui| {
        card_frame(&t).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            if card_header(ui, "Tags Summary", &t) {
                app.session.view.tags_pane = false;
            }
            ui.add_space(4.0);
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
            egui::ScrollArea::vertical().id_salt("tags_scroll").max_height((ui.available_height() - 80.0).max(60.0)).show(ui, |ui| {
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
                                // Not selectable: a selectable label would take the click from the row.
                                ui.add(
                                    egui::Label::new(RichText::new(if text.is_empty() { "(empty)".to_string() } else { text.clone() }).color(t.text))
                                        .selectable(false),
                                );
                            })
                            .response
                            .interact(Sense::click());
                        if resp.clicked() {
                            let page = it["page"].as_str().unwrap_or("").to_string();
                            let block = it["block"].as_str().unwrap_or("").to_string();
                            let para = it["para"].as_u64().unwrap_or(0);
                            app.run(&ctx, "nav.goto", json!({"page": page}));
                            if it["ink"].as_bool().unwrap_or(false) {
                                app.run(&ctx, "object.select", json!({"ids": [block]}));
                            } else {
                                app.run(&ctx, "caret.set", json!({"block": block, "para": para, "offset": 0}));
                            }
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
    });
}

/// The name a template id shows (`builtin:<slug>` or `my:<name>`).
pub(crate) fn template_name(id: &str) -> String {
    match id.split_once(':') {
        Some(("builtin", slug)) => {
            notecraft_engine::BUILT_IN_TEMPLATES.iter().find(|b| b.slug == slug).map_or_else(|| slug.to_string(), |b| b.name.to_string())
        }
        Some((_, name)) => name.to_string(),
        None => id.to_string(),
    }
}

/// The Templates pane (Insert › Page Templates; decision D10): templates by category, a click
/// adds a page from one; the section's default template; Save current page as a template.
pub fn templates_pane(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    pane_panel("templates_pane", 280.0, &t).show(ui, |ui| {
        card_frame(&t).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            if card_header(ui, "Templates", &t) {
                app.session.view.templates_pane = false;
            }
            ui.add_space(4.0);
            ui.label(RichText::new("Add a page").strong().color(t.text));
            ui.label(RichText::new("Click a template to add a page based on it.").size(12.0).color(t.text_dim));
            ui.add_space(4.0);
            let mine: Vec<String> = app.session.prefs.templates.iter().map(|x| x.name.clone()).collect();
            let mut categories: Vec<&str> = Vec::new();
            for b in notecraft_engine::BUILT_IN_TEMPLATES {
                if !categories.contains(&b.category) {
                    categories.push(b.category);
                }
            }
            egui::ScrollArea::vertical().id_salt("templates_scroll").max_height((ui.available_height() - 120.0).max(80.0)).show(ui, |ui| {
                let recent = app.session.prefs.recent_templates.clone();
                if !recent.is_empty() {
                    egui::CollapsingHeader::new("Recently Used Templates").default_open(true).show(ui, |ui| {
                        for id in &recent {
                            template_row(app, ui, &ctx, id, &template_name(id), "", false, &t);
                        }
                    });
                }
                for cat in categories {
                    egui::CollapsingHeader::new(cat).default_open(cat == "Blank").show(ui, |ui| {
                        for b in notecraft_engine::BUILT_IN_TEMPLATES.iter().filter(|b| b.category == cat) {
                            template_row(app, ui, &ctx, &format!("builtin:{}", b.slug), b.name, b.description, false, &t);
                        }
                    });
                }
                if !mine.is_empty() {
                    egui::CollapsingHeader::new(notecraft_engine::MY_TEMPLATES).default_open(true).show(ui, |ui| {
                        for name in &mine {
                            template_row(app, ui, &ctx, &format!("my:{name}"), name, "", true, &t);
                        }
                    });
                }
            });
            ui.separator();
            ui.label(RichText::new("Always use a specific template").strong().color(t.text));
            ui.label(RichText::new("Pick a template for new pages in the current section.").size(12.0).color(t.text_dim));
            let current = app.session.sel.section.and_then(|id| app.session.ws.section(id)).and_then(|s| s.default_template.clone());
            let mut chosen = current.clone();
            egui::ComboBox::from_id_salt("default_template")
                .width(220.0)
                .selected_text(chosen.as_deref().map_or_else(|| "No Default Template".to_string(), template_name))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut chosen, None, "No Default Template");
                    for b in notecraft_engine::BUILT_IN_TEMPLATES {
                        ui.selectable_value(&mut chosen, Some(format!("builtin:{}", b.slug)), b.name);
                    }
                    for name in &mine {
                        ui.selectable_value(&mut chosen, Some(format!("my:{name}")), name);
                    }
                });
            if chosen != current {
                app.run(&ctx, "template.setDefault", json!({"template": chosen}));
            }
            ui.add_space(6.0);
            ui.label(RichText::new("Create new template").strong().color(t.text));
            if ui.link("Save current page as a template").clicked() && app.session.page().is_some() {
                let name = app.session.page().map(|p| p.display_title()).unwrap_or_default();
                app.ui.dialog = Some(crate::dialogs::Dialog::SaveTemplate { name, default: false });
            }
        });
    });
}

/// One template in the pane: a click adds a page from it; the user's own have Delete on a right
/// click.
fn template_row(app: &mut NoteApp, ui: &mut Ui, ctx: &egui::Context, id: &str, name: &str, about: &str, mine: bool, t: &Tokens) {
    let resp = ui.add(egui::Button::new(RichText::new(name).color(t.text)).frame(false).min_size(vec2(200.0, 22.0)));
    let resp = if about.is_empty() { resp } else { resp.on_hover_text(about) };
    if resp.clicked() {
        app.run(ctx, "page.newFromTemplate", json!({"template": id}));
        app.canvas.focused = true;
    }
    if mine {
        resp.context_menu(|ui| {
            if ui.button("Delete").clicked() {
                app.run(ctx, "template.delete", json!({"template": id}));
                ui.close();
            }
        });
    }
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
    pane_panel("search_pane", 270.0, &t).show(ui, |ui| {
        card_frame(&t).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            if card_header(ui, "Search Results", &t) {
                app.ui.search_pane = false;
            }
            ui.add_space(4.0);
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
    });
}

/// The find-on-page bar (Ctrl+F): a card floating over the page's top right corner with the
/// query, the match count, previous/next and close. `area` is the page area.
pub fn find_bar(app: &mut NoteApp, ui: &mut Ui, area: egui::Rect, t: Tokens) {
    let ctx = ui.ctx().clone();
    let pos = egui::pos2(area.max.x - 440.0, area.min.y + 8.0).max(area.min + vec2(8.0, 8.0));
    egui::Area::new(egui::Id::new("find_bar")).fixed_pos(pos).order(egui::Order::Foreground).show(&ctx, |ui| {
        card_frame(&t).inner_margin(Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut app.ui.find)
                        .id(egui::Id::new("find_box"))
                        .hint_text(RichText::new("Find on page").color(t.text_dim))
                        .desired_width(200.0),
                );
                if resp.changed() {
                    let q = app.ui.find.clone();
                    app.run(&ctx, "search.page", json!({"query": q}));
                }
                if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    app.run(&ctx, "search.next", json!({}));
                    resp.request_focus();
                }
                let n = app.session.view.find.matches.len();
                let count = if n == 0 { "No matches".to_string() } else { format!("{} of {n}", app.session.view.find.current + 1) };
                ui.label(RichText::new(count).color(t.text_dim));
                for (label, tip, cmd, dir) in
                    [("Previous", "Previous (Shift+F3)", "search.prev", icons::Dir::Up), ("Next", "Next (F3)", "search.next", icons::Dir::Down)]
                {
                    let (b, rect, color) = icon_button(ui, 26.0, label, n > 0, &t);
                    icons::chevron(ui.painter(), rect.shrink(5.0), dir, color);
                    if b.on_hover_text(tip).clicked() && n > 0 {
                        app.run(&ctx, cmd, json!({}));
                    }
                }
                let (b, rect, color) = icon_button(ui, 26.0, "Close", true, &t);
                icons::cross(ui.painter(), rect.shrink(4.0), color);
                if b.on_hover_text("Close (Esc)").clicked() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape))) {
                    app.run(&ctx, "search.dismiss", json!({}));
                    app.canvas.focused = true;
                }
            });
        });
    });
}
