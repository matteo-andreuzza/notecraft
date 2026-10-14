//! Modal dialogs. Each collects the parameters of one engine command and runs it on OK
//! (catalog `UI.Dialogs.*`): Link, Move or Copy Pages, Move or Copy Section, Merge Section,
//! Notebook Properties, Paragraph Spacing, Customize Tags, and confirmations.

use egui::{RichText, vec2};
use notecraft_engine::doc::{Id, NavNode};
use serde_json::json;

use crate::{NoteApp, Tokens};

/// An open dialog and its in-progress values.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Dialog {
    /// Link (Ctrl+K): address and text to display.
    Link {
        /// Address.
        url: String,
        /// Text to display.
        text: String,
    },
    /// Move or Copy Pages (Ctrl+Alt+M).
    MovePage(Id),
    /// Move or Copy a section.
    MoveSection(Id),
    /// Merge a section into another.
    MergeSection(Id),
    /// Notebook Properties: display name.
    NotebookProperties(Id, String),
    /// Paragraph Spacing: before, after, line (points).
    Spacing(f32, f32, f32),
    /// Customize Tags: the new tag's name.
    CustomizeTags(String),
    /// Confirm emptying the recycle bin.
    ConfirmEmptyBin,
}

fn sections(app: &NoteApp) -> Vec<(Id, String)> {
    let mut out = Vec::new();
    for nb in &app.session.ws.notebooks {
        fn walk(nodes: &[NavNode], prefix: &str, out: &mut Vec<(Id, String)>, depth: usize) {
            if depth > 16 {
                return;
            }
            for n in nodes {
                match n {
                    NavNode::Section(s) => out.push((s.id, format!("{prefix}{}", s.name))),
                    NavNode::Group(g) => walk(&g.children, &format!("{prefix}{} › ", g.name), out, depth + 1),
                }
            }
        }
        walk(&nb.children, &format!("{} › ", nb.name), &mut out, 0);
    }
    out
}

/// Draw the open dialog, if any.
pub fn show(app: &mut NoteApp, ctx: &egui::Context, t: Tokens) {
    let Some(dialog) = app.ui.dialog.clone() else { return };
    let mut close = false;
    let mut next = dialog.clone();
    let title = match &dialog {
        Dialog::Link { .. } => "Link",
        Dialog::MovePage(_) => "Move or Copy Pages",
        Dialog::MoveSection(_) => "Move or Copy Section",
        Dialog::MergeSection(_) => "Merge Section",
        Dialog::NotebookProperties(..) => "Notebook Properties",
        Dialog::Spacing(..) => "Paragraph Spacing",
        Dialog::CustomizeTags(_) => "Customize Tags",
        Dialog::ConfirmEmptyBin => "Empty Recycle Bin",
    };
    let modal = egui::Modal::new(egui::Id::new("dialog")).show(ctx, |ui| {
        ui.set_min_width(380.0);
        ui.label(RichText::new(title).size(16.0).strong());
        ui.add_space(8.0);
        match &mut next {
            Dialog::Link { url, text } => {
                ui.label("Text to display:");
                ui.text_edit_singleline(text);
                ui.label("Address:");
                let r = ui.text_edit_singleline(url);
                if !r.has_focus() && url.is_empty() {
                    r.request_focus();
                }
                ui.add_space(4.0);
                ui.label(RichText::new("Or pick a location in your notebooks:").size(12.0).color(t.page_meta));
                egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                    for (id, name) in sections(app) {
                        if ui.selectable_label(false, &name).clicked() {
                            *url = format!("notecraft:section/{id}");
                            if text.is_empty() {
                                *text = name.rsplit(" › ").next().unwrap_or("").to_string();
                            }
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() && !url.trim().is_empty() {
                        let mut p = json!({"url": url.trim()});
                        if !text.is_empty() {
                            p["text"] = json!(text);
                        }
                        app.run(ctx, "insert.link", p);
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::MovePage(pid) => {
                ui.label("Pick a section:");
                let pid = *pid;
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for (id, name) in sections(app) {
                        ui.horizontal(|ui| {
                            ui.label(&name);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Copy").clicked() {
                                    app.run(ctx, "page.moveOrCopy", json!({"page": pid.to_string(), "section": id.to_string(), "copy": true}));
                                    close = true;
                                }
                                if ui.button("Move").clicked() {
                                    app.run(ctx, "page.moveOrCopy", json!({"page": pid.to_string(), "section": id.to_string()}));
                                    close = true;
                                }
                            });
                        });
                    }
                });
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            }
            Dialog::MoveSection(sid) => {
                let sid = *sid;
                ui.label("Move or copy into a notebook:");
                for nb in app.session.ws.notebooks.clone() {
                    ui.horizontal(|ui| {
                        ui.label(&nb.name);
                        if ui.button("Copy").clicked() {
                            app.run(ctx, "section.moveOrCopy", json!({"section": sid.to_string(), "notebook": nb.id.to_string(), "copy": true}));
                            close = true;
                        }
                        if ui.button("Move").clicked() {
                            app.run(ctx, "section.moveOrCopy", json!({"section": sid.to_string(), "notebook": nb.id.to_string()}));
                            close = true;
                        }
                    });
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            }
            Dialog::MergeSection(sid) => {
                let sid = *sid;
                ui.label("Merge this section's pages into:");
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for (id, name) in sections(app) {
                        if id != sid && ui.button(&name).clicked() {
                            app.run(ctx, "section.merge", json!({"section": sid.to_string(), "into": id.to_string()}));
                            close = true;
                        }
                    }
                });
                ui.label(RichText::new("The emptied section goes to the Notebook Recycle Bin.").size(12.0).color(t.page_meta));
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            }
            Dialog::NotebookProperties(id, name) => {
                ui.label("Display name:");
                ui.text_edit_singleline(name);
                ui.label("Color:");
                let nid = *id;
                ui.horizontal_wrapped(|ui| {
                    for (i, sw) in notecraft_engine::doc::color::SECTION_COLORS.iter().enumerate() {
                        let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::click());
                        ui.painter().rect_filled(r, 2.0, crate::theme::c32(sw.color));
                        if resp.on_hover_text(sw.name).clicked() {
                            app.run(ctx, "notebook.properties", json!({"notebook": nid.to_string(), "color": i}));
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        app.run(ctx, "notebook.properties", json!({"notebook": nid.to_string(), "name": name}));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::Spacing(b, a, l) => {
                egui::Grid::new("spacing").show(ui, |ui| {
                    ui.label("Before:");
                    ui.add(egui::DragValue::new(b).range(0.0..=1000.0).suffix(" pt"));
                    ui.end_row();
                    ui.label("After:");
                    ui.add(egui::DragValue::new(a).range(0.0..=1000.0).suffix(" pt"));
                    ui.end_row();
                    ui.label("Line Spacing (at least):");
                    ui.add(egui::DragValue::new(l).range(0.0..=1000.0).suffix(" pt"));
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        app.run(ctx, "para.spacingDialog", json!({"before": b, "after": a, "line": l}));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::CustomizeTags(name) => {
                ui.label("Custom tags:");
                for d in app.session.prefs.custom_tags.clone() {
                    ui.horizontal(|ui| {
                        ui.label(&d.name);
                        if ui.small_button("Remove").clicked() {
                            app.run(ctx, "tag.customize", json!({"action": "remove", "key": d.key}));
                        }
                    });
                }
                ui.separator();
                ui.label("New Tag › Display name:");
                ui.text_edit_singleline(name);
                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() && !name.trim().is_empty() {
                        app.run(ctx, "tag.customize", json!({"action": "add", "name": name.trim()}));
                        name.clear();
                    }
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::ConfirmEmptyBin => {
                ui.label("Permanently delete every page and section in this notebook's recycle bin?");
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        app.run(ctx, "history.emptyRecycleBin", json!({}));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
        }
    });
    if modal.should_close() {
        close = true;
    }
    app.ui.dialog = if close { None } else { Some(next) };
}
