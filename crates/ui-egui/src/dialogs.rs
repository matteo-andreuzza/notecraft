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
    /// Link (Ctrl+K): address, text to display, and the page/section search filter.
    Link {
        /// Address.
        url: String,
        /// Text to display.
        text: String,
        /// Filters the picker below by name.
        filter: String,
    },
    /// Insert Table: columns, rows.
    InsertTable(usize, usize),
    /// Notebook List (Ctrl+G): pick a notebook to switch to.
    NotebookList,
    /// Symbol: pick a symbol to insert.
    Symbol,
    /// Move or Copy Pages (Ctrl+Alt+M): the page, and the search filter.
    MovePage(Id, String),
    /// Move or Copy a section: the section, and the search filter.
    MoveSection(Id, String),
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
    /// Alt Text of the selected picture or attachment (`CM.Picture.AlternativeText`).
    AltText(String),
    /// Rename the selected file attachment (`CM.FileAttachment.RenameSelected`).
    RenameAttachment(String),
    /// Insert File (`BEH.Paste.Files`): dropped files NoteCraft can print, as the parameters of
    /// `insert.file` / `insert.printout` (path, and where they were dropped).
    InsertFile(Vec<serde_json::Value>),
    /// Save current page as a template (decision D10): its name, and whether it becomes the
    /// current section's default.
    SaveTemplate {
        /// Template name.
        name: String,
        /// Also the section's default template.
        default: bool,
    },
    /// Sort… of the current table (`CT.TableLayout.Data.ShowSortTableRowsDialog`): the column to
    /// sort by and an optional second one, each descending or not, and the header row.
    SortTable {
        /// Sort by, then by (`None`: no second column).
        by: [Option<usize>; 2],
        /// Descending, for each.
        descending: [bool; 2],
        /// The first row is a header, kept on top.
        header: bool,
    },
    /// Color & Thickness of the selected pen (`UI.Panes.ColorThickness`): highlighter or pen,
    /// colour `#rrggbb`, width in points.
    ColorThickness {
        /// A highlighter (else a pen).
        highlighter: bool,
        /// Colour.
        color: String,
        /// Tip width, points.
        width: f32,
    },
}

/// A curated set of common symbols for the Symbol dialog (`UI.Dialogs.Symbol`); plain Unicode
/// characters, not from any Microsoft font or symbol set (clean room).
const SYMBOLS: &[&str] = &[
    "©", "®", "™", "€", "£", "¥", "¢", "§", "¶", "†", "‡", "•", "…", "‰", "′", "″", "°", "±", "×", "÷", "≤", "≥", "≠", "≈", "∞", "√", "∑", "π", "Ω",
    "α", "β", "γ", "Δ", "←", "→", "↑", "↓", "↔", "⇒", "★", "☆", "♥", "♦", "♣", "♠", "✓", "✗", "¼", "½", "¾",
];

/// The file name in a dropped file's `insert.file` parameters.
fn file_name(params: &serde_json::Value) -> String {
    let path = params.get("path").and_then(serde_json::Value::as_str).unwrap_or_default();
    std::path::Path::new(path).file_name().map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned())
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

/// Every page, named `Notebook › … › Section › Page` (for the Link dialog's page search;
/// `UI.Dialogs.Link`).
fn pages(app: &NoteApp) -> Vec<(Id, String)> {
    let mut out = Vec::new();
    for nb in &app.session.ws.notebooks {
        fn walk(nodes: &[NavNode], prefix: &str, out: &mut Vec<(Id, String)>, depth: usize) {
            if depth > 16 {
                return;
            }
            for n in nodes {
                match n {
                    NavNode::Section(s) => {
                        for p in &s.pages {
                            out.push((p.id, format!("{prefix}{} › {}", s.name, p.display_title())));
                        }
                    }
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
        Dialog::InsertTable(..) => "Insert Table",
        Dialog::NotebookList => "Notebooks",
        Dialog::Symbol => "Symbol",
        Dialog::MovePage(..) => "Move or Copy Pages",
        Dialog::MoveSection(..) => "Move or Copy Section",
        Dialog::MergeSection(_) => "Merge Section",
        Dialog::NotebookProperties(..) => "Notebook Properties",
        Dialog::Spacing(..) => "Paragraph Spacing",
        Dialog::CustomizeTags(_) => "Customize Tags",
        Dialog::ConfirmEmptyBin => "Empty Recycle Bin",
        Dialog::AltText(_) => "Alt Text",
        Dialog::RenameAttachment(_) => "Rename",
        Dialog::InsertFile(_) => "Insert File",
        Dialog::ColorThickness { .. } => "Color & Thickness",
        Dialog::SortTable { .. } => "Sort",
        Dialog::SaveTemplate { .. } => "Save As Template",
    };
    let modal = egui::Modal::new(egui::Id::new("dialog")).show(ctx, |ui| {
        ui.set_min_width(380.0);
        ui.label(RichText::new(title).size(16.0).strong());
        ui.add_space(8.0);
        match &mut next {
            Dialog::Link { url, text, filter } => {
                ui.label("Text to display:");
                ui.text_edit_singleline(text);
                ui.label("Address:");
                let r = ui.text_edit_singleline(url);
                if !r.has_focus() && url.is_empty() {
                    r.request_focus();
                }
                ui.add_space(4.0);
                ui.label(RichText::new("Or search for a page or section in your notebooks:").size(12.0).color(t.page_meta));
                ui.add(egui::TextEdit::singleline(filter).hint_text("Search pages and sections"));
                let needle = filter.to_lowercase();
                let matches = |name: &str| needle.is_empty() || name.to_lowercase().contains(&needle);
                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    for (id, name) in pages(app).into_iter().filter(|(_, n)| matches(n)) {
                        if ui.selectable_label(false, &name).clicked() {
                            *url = format!("notecraft:page/{id}");
                            if text.is_empty() {
                                *text = name.rsplit(" › ").next().unwrap_or("").to_string();
                            }
                        }
                    }
                    for (id, name) in sections(app).into_iter().filter(|(_, n)| matches(n)) {
                        if ui.selectable_label(false, &name).clicked() {
                            *url = format!("notecraft:section/{id}");
                            if text.is_empty() {
                                *text = name.rsplit(" › ").next().unwrap_or("").to_string();
                            }
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if crate::chrome::primary_button(ui, "OK", &t).clicked() && !url.trim().is_empty() {
                        let mut p = json!({"url": url.trim()});
                        if !text.is_empty() {
                            p["text"] = json!(text);
                        }
                        // On an existing link (text at the caret, or a selected object's) OK
                        // changes it; otherwise it makes a new one.
                        let cmd = if app.disabled_reason("link.edit").is_none() { "link.edit" } else { "insert.link" };
                        app.run(ctx, cmd, p);
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::SaveTemplate { name, default } => {
                ui.label("Template name:");
                let r = ui.text_edit_singleline(name);
                if !r.has_focus() && name.is_empty() {
                    r.request_focus();
                }
                ui.checkbox(default, "Set as default template for new pages in the current section");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if crate::chrome::primary_button(ui, "Save", &t).clicked() && !name.trim().is_empty() {
                        app.run(ctx, "template.save", json!({"name": name.trim(), "default": *default}));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::SortTable { by, descending, header } => {
                // Columns go by their header text when the first row is a header (unverified).
                let names: Vec<String> = crate::ribbon::current_table(app)
                    .map(|tb| {
                        (0..tb.n_cols())
                            .map(|c| {
                                let head = if *header { tb.cell(0, c).map(|x| x.text()).unwrap_or_default() } else { String::new() };
                                if head.trim().is_empty() { format!("Column {}", c + 1) } else { head.trim().to_string() }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if names.is_empty() {
                    close = true;
                }
                let name = |c: Option<usize>| c.and_then(|c| names.get(c).cloned()).unwrap_or_else(|| "(none)".into());
                egui::Grid::new("sort_table").num_columns(3).spacing(vec2(12.0, 8.0)).show(ui, |ui| {
                    for (k, label) in ["Sort by:", "Then by:"].into_iter().enumerate() {
                        ui.label(label);
                        egui::ComboBox::from_id_salt(("sort_by", k)).selected_text(name(by[k])).width(150.0).show_ui(ui, |ui| {
                            if k > 0 {
                                ui.selectable_value(&mut by[k], None, "(none)");
                            }
                            for (c, n) in names.iter().enumerate() {
                                ui.selectable_value(&mut by[k], Some(c), n);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.add_enabled_ui(by[k].is_some(), |ui| {
                                ui.radio_value(&mut descending[k], false, "Ascending");
                                ui.radio_value(&mut descending[k], true, "Descending");
                            });
                        });
                        ui.end_row();
                    }
                });
                ui.add_space(4.0);
                ui.checkbox(header, "Header row (excluded from sort)");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if crate::chrome::primary_button(ui, "OK", &t).clicked() {
                        let keys: Vec<serde_json::Value> =
                            by.iter().zip(descending.iter()).filter_map(|(c, d)| c.map(|c| json!({"col": c, "descending": d}))).collect();
                        if !keys.is_empty() {
                            app.run(ctx, "table.sortDialog", json!({"keys": keys, "headerRow": *header}));
                        }
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::InsertTable(cols, rows) => {
                egui::Grid::new("insert_table").show(ui, |ui| {
                    ui.label("Columns:");
                    ui.add(egui::DragValue::new(cols).range(1..=notecraft_engine::doc::table::MAX_COLS));
                    ui.end_row();
                    ui.label("Rows:");
                    ui.add(egui::DragValue::new(rows).range(1..=notecraft_engine::doc::table::MAX_ROWS));
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if crate::chrome::primary_button(ui, "OK", &t).clicked() {
                        app.run(ctx, "insert.tableDialog", json!({"cols": *cols, "rows": *rows}));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::NotebookList => {
                ui.label("Switch to a notebook:");
                let current = app.session.sel.notebook;
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for nb in app.session.ws.notebooks.clone() {
                        if ui.selectable_label(Some(nb.id) == current, &nb.name).clicked() {
                            app.run(ctx, "notebook.select", json!({"notebook": nb.id.to_string()}));
                            close = true;
                        }
                    }
                });
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            }
            Dialog::Symbol => {
                ui.label("Click a symbol to insert it:");
                ui.add_space(4.0);
                egui::Grid::new("symbol_grid").spacing(vec2(4.0, 4.0)).show(ui, |ui| {
                    for (i, sym) in SYMBOLS.iter().enumerate() {
                        if ui.add(egui::Button::new(RichText::new(*sym).size(16.0)).min_size(vec2(28.0, 28.0))).clicked() {
                            app.run(ctx, "insert.symbolDialog", json!({"char": sym}));
                            close = true;
                        }
                        if (i + 1) % 10 == 0 {
                            ui.end_row();
                        }
                    }
                });
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            }
            Dialog::MovePage(pid, filter) => {
                ui.label("Pick a section:");
                let pid = *pid;
                ui.add(egui::TextEdit::singleline(filter).hint_text("Search sections"));
                let needle = filter.to_lowercase();
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for (id, name) in sections(app).into_iter().filter(|(_, n)| needle.is_empty() || n.to_lowercase().contains(&needle)) {
                        ui.horizontal(|ui| {
                            ui.label(&name);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Copy").clicked() {
                                    app.run(ctx, "page.moveOrCopy", json!({"page": pid.to_string(), "section": id.to_string(), "copy": true}));
                                    close = true;
                                }
                                if crate::chrome::primary_button(ui, "Move", &t).clicked() {
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
            Dialog::MoveSection(sid, filter) => {
                let sid = *sid;
                ui.label("Move or copy into a notebook:");
                ui.add(egui::TextEdit::singleline(filter).hint_text("Search notebooks"));
                let needle = filter.to_lowercase();
                for nb in app
                    .session
                    .ws
                    .notebooks
                    .iter()
                    .filter(|nb| needle.is_empty() || nb.name.to_lowercase().contains(&needle))
                    .cloned()
                    .collect::<Vec<_>>()
                {
                    ui.horizontal(|ui| {
                        ui.label(&nb.name);
                        if ui.button("Copy").clicked() {
                            app.run(ctx, "section.moveOrCopy", json!({"section": sid.to_string(), "notebook": nb.id.to_string(), "copy": true}));
                            close = true;
                        }
                        if crate::chrome::primary_button(ui, "Move", &t).clicked() {
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
                    if crate::chrome::primary_button(ui, "OK", &t).clicked() {
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
                    if crate::chrome::primary_button(ui, "OK", &t).clicked() {
                        app.run(ctx, "para.spacingDialog", json!({"before": b, "after": a, "line": l}));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::ColorThickness { highlighter, color, width } => {
                use notecraft_engine::doc::InkTool;
                ui.horizontal(|ui| {
                    if ui.radio(!*highlighter, "Pen").clicked() && *highlighter {
                        *highlighter = false;
                        *width = 0.5 * crate::ink::MM;
                    }
                    if ui.radio(*highlighter, "Highlighter").clicked() && !*highlighter {
                        *highlighter = true;
                        *width = 5.0 * crate::ink::MM;
                    }
                });
                let tool = if *highlighter { InkTool::Highlighter } else { InkTool::Pen };
                ui.label(RichText::new("Thickness").color(t.text_dim));
                ui.horizontal_wrapped(|ui| {
                    for mm in if *highlighter { crate::ink::HIGHLIGHTER_MM } else { crate::ink::PEN_MM } {
                        let on = (*width - mm * crate::ink::MM).abs() < 0.05;
                        if ui.selectable_label(on, format!("{mm} mm")).clicked() {
                            *width = mm * crate::ink::MM;
                        }
                    }
                });
                ui.label(RichText::new("Color").color(t.text_dim));
                let mut picked = None;
                crate::ribbon::swatch_grid(ui, &crate::ink::colors(tool), 6, |c| picked = Some(c));
                if let Some(c) = picked {
                    *color = c.to_hex();
                }
                let sample = notecraft_engine::doc::color::Rgba::from_hex(color).map(crate::theme::c32).unwrap_or(t.text);
                let (r, _) = ui.allocate_exact_size(vec2(200.0, 18.0), egui::Sense::hover());
                ui.painter().line_segment([r.left_center(), r.right_center()], egui::Stroke::new((*width * 1.33).clamp(1.0, 18.0), sample));
                ui.horizontal(|ui| {
                    if crate::chrome::primary_button(ui, "OK", &t).clicked() {
                        let tool = if *highlighter { "highlighter" } else { "pen" };
                        app.run(ctx, "draw.colorThickness", json!({"tool": tool, "color": color, "width": width}));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::AltText(text) => {
                ui.label("Describe the picture for people who can't see it:");
                ui.add(egui::TextEdit::multiline(text).desired_rows(3).desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    if crate::chrome::primary_button(ui, "OK", &t).clicked() {
                        app.run(ctx, "object.altText", json!({"text": text}));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::RenameAttachment(name) => {
                ui.label("File name:");
                let r = ui.text_edit_singleline(name);
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    if (crate::chrome::primary_button(ui, "OK", &t).clicked() || enter) && !name.trim().is_empty() {
                        app.run(ctx, "attachment.rename", json!({"name": name.trim()}));
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
                    if crate::chrome::primary_button(ui, "Add", &t).clicked() && !name.trim().is_empty() {
                        app.run(ctx, "tag.customize", json!({"action": "add", "name": name.trim()}));
                        name.clear();
                    }
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::InsertFile(files) => {
                let names: Vec<String> = files.iter().map(file_name).collect();
                match names.as_slice() {
                    [one] => ui.label(format!("How do you want to insert \u{201c}{one}\u{201d}?")),
                    many => ui.label(format!("How do you want to insert these {} files?", many.len())),
                };
                if names.len() > 1 {
                    egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                        for n in &names {
                            ui.label(RichText::new(n).color(t.page_meta));
                        }
                    });
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if crate::chrome::primary_button(ui, "Attach File", &t).clicked() {
                        for p in files.iter() {
                            app.run(ctx, "insert.file", p.clone());
                        }
                        close = true;
                    }
                    if ui.button("Insert Printout").clicked() {
                        for p in files.iter() {
                            app.run(ctx, "insert.printout", p.clone());
                        }
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::ConfirmEmptyBin => {
                ui.label("Permanently delete every page and section in this notebook's recycle bin?");
                ui.horizontal(|ui| {
                    if crate::chrome::primary_button(ui, "Delete", &t).clicked() {
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
