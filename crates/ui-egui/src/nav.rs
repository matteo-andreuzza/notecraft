//! Navigation: the notebook pane (left), the section tabs (top of the page area), the search
//! box, and the page list (right) — catalog `UI.Nav.*` — with their context menus (`CM.Notebook`,
//! `CM.SectionGroup`, `CM.Section`, `CM.Page`, `CM.PageListArea`, `CM.SectionTabsArea`,
//! `CM.NotebookPaneArea`).

use egui::{Align, Align2, Color32, FontId, Frame, Layout, Margin, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use notecraft_engine::doc::{Id, NavNode};
use serde_json::json;

use crate::theme::c32;
use crate::{NoteApp, Rename, Tokens, icons};

/// A context-menu entry that runs a command (disabled with its reason when unavailable).
pub fn cmd_item(app: &mut NoteApp, ui: &mut Ui, label: &str, cmd: &str, params: serde_json::Value) {
    let reason = app.disabled_reason(cmd);
    let resp = ui.add_enabled(reason.is_none(), egui::Button::new(label).frame(false).min_size(vec2(180.0, 20.0)));
    let resp = match &reason {
        Some(r) => resp.on_disabled_hover_text(r.clone()),
        None => resp,
    };
    if resp.clicked() {
        let ctx = ui.ctx().clone();
        app.run(&ctx, cmd, params);
        ui.close();
    }
}

/// The notebook pane (catalog `UI.Nav.NotebookPane`, `UI.Nav.NotebookPaneCollapse`).
pub fn notebook_pane(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    if app.session.view.notebook_pane_collapsed {
        egui::Panel::left("notebook_pane_collapsed").exact_size(30.0).frame(Frame::NONE.fill(t.pane).stroke(Stroke::new(1.0, t.border))).show(
            ui,
            |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    if ui.add(egui::Button::new("»").frame(false)).on_hover_text("Expand Notebook Pane").clicked() {
                        app.run(&ctx, "view.notebookPane", json!({}));
                    }
                    let name = app.session.notebook().map(|n| n.name.clone()).unwrap_or_default();
                    ui.add_space(8.0);
                    for ch in name.chars().take(18) {
                        ui.label(RichText::new(ch.to_string()).size(12.0).color(t.pane_text));
                    }
                });
            },
        );
        return;
    }
    egui::Panel::left("notebook_pane")
        .default_size(app.ui.notebook_pane_width)
        .size_range(150.0..=420.0)
        .resizable(true)
        .frame(Frame::NONE.fill(t.pane).inner_margin(Margin::symmetric(0, 4)).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("Notebooks").size(12.5).color(t.pane_text.gamma_multiply(0.8)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(4.0);
                    // UI.Nav.NotebookPaneCollapse — the pin/collapse button.
                    if ui.add(egui::Button::new("«").frame(false)).on_hover_text("Collapse Notebook Pane").clicked() {
                        app.run(&ctx, "view.notebookPane", json!({}));
                    }
                });
            });
            ui.add_space(2.0);
            // CM.NotebookPaneArea — right-click on the empty pane. Registered before the rows,
            // so a row (registered later, so on top) keeps its own menu.
            let area = ui.interact(ui.available_rect_before_wrap(), ui.id().with("nb_area"), Sense::click());
            egui::ScrollArea::vertical().id_salt("nb_scroll").show(ui, |ui| {
                let notebooks: Vec<_> = app.session.ws.notebooks.clone();
                for nb in notebooks {
                    notebook_row(app, ui, &nb, t);
                    if app.session.sel.notebook == Some(nb.id) {
                        nodes(app, ui, &nb.children, 1, t);
                    }
                }
                ui.allocate_space(vec2(ui.available_width(), 40.0))
            });
            area.context_menu(|ui| {
                cmd_item(app, ui, "New Section", "section.new", json!({}));
                cmd_item(app, ui, "New Section Group", "sectionGroup.new", json!({}));
                ui.separator();
                if ui.button("New Notebook…").clicked() {
                    app.ui.backstage = Some(crate::Backstage::New);
                    ui.close();
                }
                if ui.button("Open Notebook…").clicked() {
                    app.ui.backstage = Some(crate::Backstage::Open);
                    ui.close();
                }
            });
        });
}

fn notebook_row(app: &mut NoteApp, ui: &mut Ui, nb: &notecraft_engine::doc::Notebook, t: Tokens) {
    let ctx = ui.ctx().clone();
    let selected = app.session.sel.notebook == Some(nb.id);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &nb.name));
    if selected {
        ui.painter().rect_filled(rect, 0.0, t.pane_selected);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 0.0, t.hover);
    }
    // Notebook icon in the notebook's colour.
    let icon = Rect::from_min_size(rect.min + vec2(10.0, 5.0), vec2(13.0, 16.0));
    ui.painter().rect_filled(icon, 2.0, c32(nb.color.rgba()));
    ui.painter().rect_stroke(icon, 2.0, Stroke::new(1.0, t.pane_text.gamma_multiply(0.5)), egui::StrokeKind::Inside);
    let renaming = matches!(&app.ui.rename, Some(Rename::Notebook(id, _)) if *id == nb.id);
    if renaming {
        rename_field(app, ui, Rect::from_min_max(rect.min + vec2(30.0, 2.0), rect.max - vec2(4.0, 2.0)));
    } else {
        ui.painter().text(pos2(rect.min.x + 30.0, rect.center().y), Align2::LEFT_CENTER, &nb.name, FontId::proportional(13.5), t.pane_text);
    }
    if resp.clicked() {
        app.run(&ctx, "notebook.select", json!({"notebook": nb.id.to_string()}));
    }
    let nb_id = nb.id.to_string();
    resp.context_menu(|ui| {
        cmd_item(app, ui, "Sync This Notebook Now", "notebook.syncNow", json!({}));
        cmd_item(app, ui, "Copy Link to Notebook", "notebook.copyLink", json!({"notebook": nb_id}));
        cmd_item(app, ui, "Mark Notebook as Read", "history.markNotebookRead", json!({}));
        ui.separator();
        cmd_item(app, ui, "Move Up", "notebook.moveUp", json!({"notebook": nb_id}));
        cmd_item(app, ui, "Move Down", "notebook.moveDown", json!({"notebook": nb_id}));
        cmd_item(app, ui, "Notebook Recycle Bin", "history.recycleBin", json!({"notebook": nb_id}));
        ui.separator();
        if ui.button("Properties…").clicked() {
            app.ui.dialog = Some(crate::dialogs::Dialog::NotebookProperties(nb.id, nb.name.clone()));
            ui.close();
        }
        cmd_item(app, ui, "Close This Notebook", "notebook.close", json!({"notebook": nb_id}));
    });
}

fn nodes(app: &mut NoteApp, ui: &mut Ui, children: &[NavNode], depth: usize, t: Tokens) {
    if depth > notecraft_engine::doc::notebook::MAX_GROUP_DEPTH {
        return;
    }
    let ctx = ui.ctx().clone();
    for n in children {
        match n {
            NavNode::Section(s) => {
                let selected = app.session.sel.section == Some(s.id);
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &s.name));
                if selected {
                    ui.painter().rect_filled(rect, 0.0, t.pane_selected);
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, 0.0, t.hover);
                }
                let x = rect.min.x + 14.0 + depth as f32 * 12.0;
                ui.painter().rect_filled(Rect::from_min_size(pos2(x, rect.min.y + 6.0), vec2(5.0, 12.0)), 1.0, c32(s.color.rgba()));
                let renaming = matches!(&app.ui.rename, Some(Rename::Section(id, _)) if *id == s.id);
                if renaming {
                    rename_field(app, ui, Rect::from_min_max(pos2(x + 10.0, rect.min.y + 1.0), rect.max - vec2(4.0, 1.0)));
                } else {
                    ui.painter().text(pos2(x + 11.0, rect.center().y), Align2::LEFT_CENTER, &s.name, FontId::proportional(13.0), t.pane_text);
                }
                if resp.clicked() {
                    app.run(&ctx, "section.select", json!({"section": s.id.to_string()}));
                }
                if resp.double_clicked() {
                    app.ui.rename = Some(Rename::Section(s.id, s.name.clone()));
                }
                section_menu(app, &resp, s.id, &s.name);
            }
            NavNode::Group(g) => {
                let open_id = ui.id().with(("group_open", g.id));
                let open = ui.data_mut(|d| *d.get_temp_mut_or(open_id, true));
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 0.0, t.hover);
                }
                let x = rect.min.x + 10.0 + depth as f32 * 12.0;
                icons::triangle(
                    ui.painter(),
                    pos2(x + 4.0, rect.center().y),
                    8.0,
                    if open { icons::Dir::Down } else { icons::Dir::Right },
                    t.pane_text,
                );
                // A section group: a stack of tabs.
                for k in 0..3 {
                    ui.painter().rect_filled(
                        Rect::from_min_size(pos2(x + 12.0 + k as f32 * 3.0, rect.min.y + 6.0 + k as f32 * 1.0), vec2(4.0, 11.0)),
                        1.0,
                        t.accent.gamma_multiply(0.5 + 0.2 * k as f32),
                    );
                }
                let renaming = matches!(&app.ui.rename, Some(Rename::Group(id, _)) if *id == g.id);
                if renaming {
                    rename_field(app, ui, Rect::from_min_max(pos2(x + 26.0, rect.min.y + 1.0), rect.max - vec2(4.0, 1.0)));
                } else {
                    ui.painter().text(pos2(x + 27.0, rect.center().y), Align2::LEFT_CENTER, &g.name, FontId::proportional(13.0), t.pane_text);
                }
                if resp.clicked() {
                    ui.data_mut(|d| d.insert_temp(open_id, !open));
                }
                let gid = g.id.to_string();
                let gname = g.name.clone();
                let g_id = g.id;
                resp.context_menu(|ui| {
                    if ui.button("Rename").clicked() {
                        app.ui.rename = Some(Rename::Group(g_id, gname.clone()));
                        ui.close();
                    }
                    cmd_item(app, ui, "Copy Link to Section Group", "sectionGroup.copyLink", json!({"group": gid}));
                    cmd_item(app, ui, "Mark as Read", "history.markRead", json!({}));
                    ui.separator();
                    cmd_item(app, ui, "New Section", "section.new", json!({"group": gid}));
                    cmd_item(app, ui, "New Section Group", "sectionGroup.new", json!({"group": gid}));
                    ui.separator();
                    cmd_item(app, ui, "Delete", "sectionGroup.delete", json!({"group": gid}));
                });
                if open {
                    nodes(app, ui, &g.children, depth + 1, t);
                }
            }
        }
    }
}

/// The section context menu (catalog `CM.Section.*`).
fn section_menu(app: &mut NoteApp, resp: &egui::Response, sid: Id, name: &str) {
    let s = sid.to_string();
    let name = name.to_string();
    resp.context_menu(|ui| {
        if ui.button("Rename").clicked() {
            app.ui.rename = Some(Rename::Section(sid, name.clone()));
            ui.close();
        }
        if ui.button("Export…").clicked() {
            app.ui.backstage = Some(crate::Backstage::Export);
            ui.close();
        }
        cmd_item(app, ui, "Delete", "section.delete", json!({"section": s}));
        ui.separator();
        if ui.button("Move or Copy…").clicked() {
            app.ui.dialog = Some(crate::dialogs::Dialog::MoveSection(sid));
            ui.close();
        }
        if ui.button("Merge into Another Section…").clicked() {
            app.ui.dialog = Some(crate::dialogs::Dialog::MergeSection(sid));
            ui.close();
        }
        cmd_item(app, ui, "Copy Link to Section", "section.copyLink", json!({"section": s}));
        ui.separator();
        cmd_item(app, ui, "New Section", "section.new", json!({}));
        cmd_item(app, ui, "New Section Group", "sectionGroup.new", json!({}));
        cmd_item(app, ui, "Mark as Read", "history.markRead", json!({}));
        cmd_item(app, ui, "Password Protect This Section…", "review.password", json!({}));
        ui.menu_button("Section Color", |ui| {
            cmd_item(app, ui, "None", "section.color", json!({"section": s, "color": null}));
            for (i, sw) in notecraft_engine::doc::color::SECTION_COLORS.iter().enumerate() {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                    ui.painter().rect_filled(r, 1.0, c32(sw.color));
                    cmd_item(app, ui, sw.name, "section.color", json!({"section": s, "color": i}));
                });
            }
        });
    });
}

fn rename_field(app: &mut NoteApp, ui: &mut Ui, rect: Rect) {
    let ctx = ui.ctx().clone();
    let Some(r) = app.ui.rename.clone() else { return };
    let mut text = match &r {
        Rename::Section(_, n) | Rename::Group(_, n) | Rename::Notebook(_, n) => n.clone(),
    };
    let id = ui.id().with("rename_field");
    let resp = ui.put(rect, egui::TextEdit::singleline(&mut text).id(id).font(FontId::proportional(13.0)));
    if !resp.has_focus() && !resp.lost_focus() {
        resp.request_focus();
    }
    let commit = resp.lost_focus();
    let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
    app.ui.rename = Some(match r {
        Rename::Section(id, _) => Rename::Section(id, text.clone()),
        Rename::Group(id, _) => Rename::Group(id, text.clone()),
        Rename::Notebook(id, _) => Rename::Notebook(id, text.clone()),
    });
    if cancel {
        app.ui.rename = None;
    } else if commit {
        let r = app.ui.rename.take();
        match r {
            Some(Rename::Section(id, n)) if !n.trim().is_empty() => {
                app.run(&ctx, "section.rename", json!({"section": id.to_string(), "name": n}));
            }
            Some(Rename::Group(id, n)) if !n.trim().is_empty() => {
                app.run(&ctx, "sectionGroup.rename", json!({"group": id.to_string(), "name": n}));
            }
            Some(Rename::Notebook(id, n)) if !n.trim().is_empty() => {
                app.run(&ctx, "notebook.rename", json!({"notebook": id.to_string(), "name": n}));
            }
            _ => {}
        }
    }
}

/// The section tabs across the top of the page area, plus the search box on the right
/// (catalog `UI.Nav.SectionTabs`, `UI.Nav.SearchBox`).
pub fn section_tabs(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    let Some(nb) = app.session.notebook().cloned() else {
        empty_state(app, ui, t);
        return;
    };
    // The tabs show the current level: the sections and groups next to the current section.
    let parent_path: Vec<usize> = app
        .session
        .sel
        .section
        .and_then(|s| nb.find(s))
        .map(|mut p| {
            p.pop();
            p
        })
        .unwrap_or_default();
    let siblings: Vec<NavNode> = nb.children_at(&parent_path).cloned().unwrap_or_default();
    let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
    ui.painter().rect_filled(bar, 0.0, t.ribbon);
    ui.painter().line_segment([bar.left_bottom(), bar.right_bottom()], Stroke::new(1.0, t.border));
    let search_w = 260.0_f32.min(bar.width() * 0.35);
    let tabs_rect = Rect::from_min_max(bar.min, pos2(bar.max.x - search_w - 8.0, bar.max.y));
    let mut x = tabs_rect.min.x + 6.0;
    // A "back up" tab when inside a section group.
    if !parent_path.is_empty() {
        let r = Rect::from_min_size(pos2(x, bar.min.y + 5.0), vec2(26.0, 25.0));
        let resp = ui.interact(r, ui.id().with("tab_up"), Sense::click()).on_hover_text("Back to the parent section group");
        icons::triangle(ui.painter(), r.center(), 10.0, icons::Dir::Left, t.ribbon_text);
        if resp.clicked() {
            let up = &parent_path[..parent_path.len() - 1];
            let first = nb.children_at(up).and_then(|c| c.iter().find_map(|n| if let NavNode::Section(s) = n { Some(s.id) } else { None }));
            if let Some(s) = first {
                app.run(&ctx, "section.select", json!({"section": s.to_string()}));
            }
        }
        x += 30.0;
    }
    for n in &siblings {
        let (name, color, id, is_group) = match n {
            NavNode::Section(s) => (s.name.clone(), c32(s.color.rgba()), s.id, false),
            NavNode::Group(g) => (g.name.clone(), t.pane, g.id, true),
        };
        let font = FontId::proportional(12.5);
        let w = ui.fonts_mut(|f| f.layout_no_wrap(name.clone(), font.clone(), Color32::BLACK).size().x) + 22.0 + if is_group { 14.0 } else { 0.0 };
        // Leave room for the "+" tab after the last one shown.
        if x + w > tabs_rect.max.x - 32.0 {
            break;
        }
        let selected = app.session.sel.section == Some(id);
        let r = Rect::from_min_size(pos2(x, bar.min.y + if selected { 3.0 } else { 6.0 }), vec2(w, if selected { 27.0 } else { 24.0 }));
        let resp = ui.interact(r, ui.id().with(("sectab", id)), Sense::click_and_drag());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &name));
        let fill = if selected { color } else { color.gamma_multiply(0.85) };
        ui.painter().rect_filled(r, egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 }, fill);
        if selected {
            ui.painter().rect_stroke(r, egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 }, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
        }
        let renaming = matches!(&app.ui.rename, Some(Rename::Section(rid, _)) if *rid == id);
        if renaming {
            rename_field(app, ui, r.shrink(2.0));
        } else {
            let tc = crate::theme::text_on(fill);
            if is_group {
                // A stack of tabs marks a section group.
                for k in 0..3 {
                    ui.painter().rect_filled(
                        Rect::from_min_size(pos2(r.min.x + 8.0 + k as f32 * 3.0, r.center().y - 5.0 + k as f32), vec2(3.0, 10.0)),
                        0.5,
                        t.accent.gamma_multiply(0.5 + 0.2 * k as f32),
                    );
                }
                ui.painter().text(r.center() + vec2(7.0, 0.0), Align2::CENTER_CENTER, &name, font, tc);
            } else {
                ui.painter().text(r.center(), Align2::CENTER_CENTER, &name, font, tc);
            }
        }
        if resp.clicked() {
            match n {
                NavNode::Section(_) => {
                    app.run(&ctx, "section.select", json!({"section": id.to_string()}));
                }
                NavNode::Group(g) => {
                    // Opening a group shows its sections as tabs.
                    if let Some(first) = g.children.iter().find_map(|c| if let NavNode::Section(s) = c { Some(s.id) } else { None }) {
                        app.run(&ctx, "section.select", json!({"section": first.to_string()}));
                    }
                }
            }
        }
        if resp.double_clicked() && !is_group {
            app.ui.rename = Some(Rename::Section(id, name.clone()));
        }
        if !is_group {
            section_menu(app, &resp, id, &name);
        }
        x += w + 2.0;
    }
    // The "+" tab: Create a New Section.
    let plus = Rect::from_min_size(pos2(x + 2.0, bar.min.y + 6.0), vec2(26.0, 24.0));
    let presp = ui.interact(plus, ui.id().with("tab_plus"), Sense::click()).on_hover_text("Create a New Section (Ctrl+T)");
    presp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Create a New Section"));
    if presp.hovered() {
        ui.painter().rect_filled(plus, 3.0, t.hover);
    }
    ui.painter().text(plus.center(), Align2::CENTER_CENTER, "+", FontId::proportional(16.0), t.ribbon_text);
    if presp.clicked() {
        app.run(&ctx, "section.new", json!({}));
    }
    // CM.SectionTabsArea.
    let area = ui.interact(Rect::from_min_max(pos2(plus.max.x + 4.0, bar.min.y), tabs_rect.max), ui.id().with("tabs_area"), Sense::click());
    area.context_menu(|ui| {
        cmd_item(app, ui, "New Section", "section.new", json!({}));
        cmd_item(app, ui, "New Section Group", "sectionGroup.new", json!({}));
    });
    // The search box with its scope drop-down.
    let sr = Rect::from_min_max(pos2(bar.max.x - search_w - 4.0, bar.min.y + 4.0), pos2(bar.max.x - 4.0, bar.max.y - 4.0));
    search_box(app, ui, sr, t);
}

fn scope_label(k: notecraft_engine::ScopeKind) -> &'static str {
    match k {
        notecraft_engine::ScopeKind::Page => "This Page",
        notecraft_engine::ScopeKind::Section => "This Section",
        notecraft_engine::ScopeKind::SectionGroup => "This Section Group",
        notecraft_engine::ScopeKind::Notebook => "This Notebook",
        notecraft_engine::ScopeKind::AllNotebooks => "All Notebooks",
    }
}

fn search_box(app: &mut NoteApp, ui: &mut Ui, r: Rect, t: Tokens) {
    let ctx = ui.ctx().clone();
    let id = egui::Id::new("search_box");
    let hint = format!("Search ({})", scope_label(app.session.view.scope));
    let mut q = app.ui.search.clone();
    let resp = ui.put(
        Rect::from_min_max(r.min, pos2(r.max.x - 24.0, r.max.y)),
        egui::TextEdit::singleline(&mut q).id(id).hint_text(hint).font(FontId::proportional(12.5)),
    );
    if q != app.ui.search {
        app.ui.search = q;
        app.ui.search_open = !app.ui.search.trim().is_empty();
    }
    if resp.gained_focus() && !app.ui.search.is_empty() {
        app.ui.search_open = true;
    }
    if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.ui.search_open = false;
        app.ui.search.clear();
        app.run(&ctx, "search.dismiss", json!({}));
    }
    // Scope drop-down (catalog CM.SearchScope.*).
    let sb = Rect::from_min_max(pos2(r.max.x - 22.0, r.min.y), r.max);
    let sresp = ui.interact(sb, ui.id().with("scope_btn"), Sense::click()).on_hover_text("Change search scope");
    sresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Change search scope"));
    icons::triangle(ui.painter(), sb.center(), 8.0, icons::Dir::Down, t.ribbon_text);
    egui::Popup::menu(&sresp).show(|ui| {
        for (label, cmd) in [
            ("This Page", "search.scopePage"),
            ("This Section", "search.scopeSection"),
            ("This Section Group", "search.scopeSectionGroup"),
            ("This Notebook", "search.scopeNotebook"),
            ("All Notebooks", "search.scopeAll"),
        ] {
            cmd_item(app, ui, label, cmd, json!({}));
        }
        ui.separator();
        cmd_item(app, ui, "Set This Scope as Default", "search.setDefaultScope", json!({}));
    });
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("search_box_rect"), resp.rect));
}

/// The as-you-type results under the search box (catalog `BEH.Search.Instant`).
pub fn search_dropdown(app: &mut NoteApp, ctx: &egui::Context, t: Tokens) {
    if !app.ui.search_open || app.ui.search.trim().is_empty() {
        return;
    }
    let Some(anchor) = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("search_box_rect"))) else { return };
    let q = app.ui.search.clone();
    let Some(res) = app.run(ctx, "search.all", json!({"query": q, "limit": 50})) else { return };
    let total = res["total"].as_u64().unwrap_or(0);
    egui::Area::new(egui::Id::new("search_results")).fixed_pos(anchor.left_bottom() + vec2(0.0, 2.0)).order(egui::Order::Foreground).show(
        ctx,
        |ui| {
            Frame::popup(ui.style()).fill(t.pane).show(ui, |ui| {
                ui.set_width(anchor.width().max(300.0));
                ui.label(
                    RichText::new(format!("{total} results · {}", scope_label(app.session.view.scope)))
                        .size(11.0)
                        .color(t.pane_text.gamma_multiply(0.75)),
                );
                ui.separator();
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    let items = res["results"].as_array().cloned().unwrap_or_default();
                    if items.is_empty() {
                        ui.label("No results.");
                    }
                    for it in items {
                        let title = it["title"].as_str().unwrap_or("").to_string();
                        let section = it["section"].as_str().unwrap_or("").to_string();
                        let snippet = it["snippet"].as_str().unwrap_or("").to_string();
                        let page = it["page"].as_str().unwrap_or("").to_string();
                        let resp = ui
                            .vertical(|ui| {
                                ui.label(RichText::new(title).strong().color(t.pane_text));
                                ui.label(RichText::new(format!("{section} · {snippet}")).size(11.0).color(t.pane_text.gamma_multiply(0.7)));
                            })
                            .response
                            .interact(Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(resp.rect, 2.0, t.hover.gamma_multiply(0.4));
                        }
                        if resp.clicked() {
                            app.run(ctx, "search.openResult", json!({"page": page, "query": q}));
                            app.ui.search_open = false;
                        }
                        ui.add_space(2.0);
                    }
                });
                ui.separator();
                if ui.button("Pin Search Results (Alt+O)").clicked() {
                    app.ui.search_pane = true;
                    app.ui.search_open = false;
                }
            });
        },
    );
}

/// The page list (catalog `UI.Nav.PageList`, `UI.Nav.AddPage`, `CM.Page.*`).
pub fn page_list(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    let width = app.session.view.page_list_width;
    egui::Panel::right("page_list").exact_size(width).frame(Frame::NONE.fill(t.pane).stroke(Stroke::new(1.0, t.border))).show(ui, |ui| {
        let Some(section) = app.session.section().cloned() else {
            ui.add_space(10.0);
            ui.label(RichText::new("No section").color(t.disabled));
            return;
        };
        // "+ Add Page".
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Add Page"));
        if resp.hovered() {
            ui.painter().rect_filled(r, 0.0, t.hover);
        }
        icons::plus_circle(ui.painter(), pos2(r.min.x + 18.0, r.center().y), 7.0, t.accent);
        ui.painter().text(pos2(r.min.x + 32.0, r.center().y), Align2::LEFT_CENTER, "Add Page", FontId::proportional(13.0), t.accent);
        if resp.on_hover_text("Add a page at the end of the section (Ctrl+N)").clicked() {
            app.run(&ctx, "page.new", json!({}));
        }
        ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, t.border));
        // CM.PageListArea — right-click below or between the page tabs. Registered before the
        // tabs, so a tab (registered later, so on top) keeps its own menu.
        let area = ui.interact(ui.available_rect_before_wrap(), ui.id().with("pages_area"), Sense::click());
        egui::ScrollArea::vertical().id_salt("pages_scroll").show(ui, |ui| {
            for (i, page) in section.pages.iter().enumerate() {
                if section.page_hidden(i) {
                    continue;
                }
                let selected = app.session.sel.page == Some(page.id) || app.session.sel.pages.contains(&page.id);
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click_and_drag());
                let page_title = page.display_title();
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &page_title));
                if selected {
                    ui.painter().rect_filled(r, 0.0, t.pane_selected);
                    ui.painter().rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), 0.0, t.accent);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, t.hover);
                }
                let indent = 12.0 + page.level as f32 * 16.0;
                let has_kids = section.page_family(i).len() > 1;
                if has_kids {
                    let arrow = Rect::from_min_size(pos2(r.max.x - 22.0, r.min.y + 4.0), vec2(18.0, 18.0));
                    let aresp = ui.interact(arrow, ui.id().with(("collapse", page.id)), Sense::click());
                    icons::triangle(
                        ui.painter(),
                        arrow.center(),
                        8.0,
                        if page.collapsed { icons::Dir::Right } else { icons::Dir::Down },
                        t.pane_text,
                    );
                    if aresp.clicked() {
                        app.run(&ctx, "page.toggleSubpages", json!({"page": page.id.to_string()}));
                    }
                }
                let font = FontId::proportional(13.0);
                let title = page.display_title();
                let max_w = r.width() - indent - 28.0;
                let galley = ui.fonts_mut(|f| f.layout(title, font, t.pane_text, max_w.max(20.0)));
                let row_rect = galley.rows.first().map(|row| row.rect()).unwrap_or(Rect::NOTHING);
                let _ = row_rect;
                ui.painter().with_clip_rect(r).galley(pos2(r.min.x + indent, r.center().y - 8.0), galley, t.pane_text);
                if resp.clicked() {
                    let (add, range) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                    app.run(&ctx, "nav.goto", json!({"page": page.id.to_string(), "add": add, "range": range}));
                }
                page_menu(app, &resp, page.id, has_kids, page.collapsed);
                // Drag a page onto another position (BEH.Pages.DragReorder).
                if resp.drag_stopped()
                    && let Some(pos) = ui.ctx().pointer_interact_pos()
                {
                    let rows_top = r.min.y - (i as f32) * 26.0;
                    let target = (((pos.y - rows_top) / 26.0).floor().max(0.0)) as usize;
                    let level = if pos.x > r.min.x + indent + 30.0 { page.level + 1 } else { page.level };
                    if target != i {
                        app.run(&ctx, "page.move", json!({"page": page.id.to_string(), "index": target, "level": level}));
                    }
                }
            }
            ui.allocate_space(vec2(ui.available_width(), 60.0))
        });
        area.context_menu(|ui| {
            cmd_item(app, ui, "New Page", "page.new", json!({}));
            cmd_item(app, ui, "Paste Page", "page.paste", json!({}));
        });
    });
}

/// The page tab context menu (catalog `CM.Page.*`).
fn page_menu(app: &mut NoteApp, resp: &egui::Response, pid: Id, has_kids: bool, collapsed: bool) {
    let p = pid.to_string();
    resp.context_menu(|ui| {
        let ctx = ui.ctx().clone();
        if ui.button("Rename").clicked() {
            app.run(&ctx, "nav.goto", json!({"page": p}));
            app.run(&ctx, "nav.pageTitle", json!({}));
            ui.close();
        }
        cmd_item(app, ui, "Delete", "page.delete", json!({"page": p}));
        ui.separator();
        if ui.button("Cut").clicked() {
            app.run(&ctx, "nav.goto", json!({"page": p}));
            app.session.sel.focus = notecraft_engine::selection::FocusArea::PageList;
            app.run(&ctx, "edit.cut", json!({}));
            ui.close();
        }
        if ui.button("Copy").clicked() {
            app.run(&ctx, "nav.goto", json!({"page": p}));
            app.session.sel.focus = notecraft_engine::selection::FocusArea::PageList;
            app.run(&ctx, "edit.copy", json!({}));
            ui.close();
        }
        cmd_item(app, ui, "Paste", "page.paste", json!({}));
        cmd_item(app, ui, "Copy Link to Page", "page.copyLink", json!({"page": p}));
        if ui.button("Move or Copy…").clicked() {
            app.ui.dialog = Some(crate::dialogs::Dialog::MovePage(pid));
            ui.close();
        }
        ui.separator();
        cmd_item(app, ui, "New Page", "page.new", json!({"after": p}));
        cmd_item(app, ui, "Make Subpage", "page.demote", json!({"page": p}));
        cmd_item(app, ui, "Promote Subpage", "page.promote", json!({"page": p}));
        if has_kids {
            if collapsed {
                cmd_item(app, ui, "Expand Subpages", "page.expandSubpages", json!({"page": p}));
            } else {
                cmd_item(app, ui, "Collapse Subpages", "page.collapseSubpages", json!({"page": p}));
            }
        }
        ui.separator();
        cmd_item(app, ui, "Mark as Unread", "history.markUnread", json!({}));
    });
}

fn empty_state(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("No notebook is open").size(18.0).color(t.page_meta));
        ui.add_space(10.0);
        if ui.button("New Notebook…").clicked() {
            app.ui.backstage = Some(crate::Backstage::New);
        }
        if ui.button("Open Notebook…").clicked() {
            app.ui.backstage = Some(crate::Backstage::Open);
        }
    });
    let _ = (Margin::ZERO, Color32::TRANSPARENT);
}
