//! Navigation in the 2024 layout (decision D1, `docs/ui-parity.md` §3 rows 10–13) — catalog
//! `UI.Nav.*`: the row under the ribbon (☰, Search notebooks), the tree of every open notebook with
//! "+ New section" and Quick notes, and the page column between the tree and the page; with their
//! context menus (`CM.Notebook`, `CM.SectionGroup`, `CM.Section`, `CM.Page`, `CM.PageListArea`,
//! `CM.NotebookPaneArea`). There are no section tabs above the page in this layout.

use egui::{Align, Align2, FontId, Frame, Layout, Margin, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use notecraft_engine::doc::{Id, NavNode};
use serde_json::json;

use crate::theme::{TypeRung, Weight, c32, radius, size};
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

/// Paint a tree or page-column row's background: the selection or hover as a rounded fill inset
/// from the pane's edges.
fn row_bg(ui: &Ui, rect: Rect, selected: bool, hovered: bool, t: &Tokens) {
    let r = rect.shrink2(vec2(2.0, 1.0));
    if selected {
        ui.painter().rect_filled(r, radius::CONTROL, t.nav_selected);
    } else if hovered {
        ui.painter().rect_filled(r, radius::CONTROL, t.nav_hover);
    }
}

/// Whether a notebook or section group is collapsed in the tree.
fn is_collapsed(app: &NoteApp, id: Id) -> bool {
    app.ui.nav_collapsed.contains(&id)
}

fn toggle_collapsed(app: &mut NoteApp, id: Id) {
    if let Some(i) = app.ui.nav_collapsed.iter().position(|x| *x == id) {
        app.ui.nav_collapsed.remove(i);
    } else {
        app.ui.nav_collapsed.push(id);
    }
}

/// The row under the ribbon: ☰ on the left, Search notebooks on the right (catalog
/// `UI.Nav.Toggle`, `UI.Nav.SearchBox`).
pub fn nav_row(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    egui::Panel::top("nav_row")
        .show_separator_line(false)
        .exact_size(size::NAV_ROW)
        .frame(Frame::NONE.fill(t.window_back).inner_margin(Margin { left: 6, right: 8, top: 0, bottom: 2 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let shown = !app.session.view.notebook_pane_collapsed;
                let label = if shown { "Hide Navigation" } else { "Show Navigation" };
                let (resp, rect, color) = crate::chrome::icon_button(ui, 26.0, label, true, &t);
                icons::hamburger(ui.painter(), rect.shrink(5.0), color);
                if resp.on_hover_text(label).clicked() {
                    app.run(&ctx, "view.navigationPanes", json!({}));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let w = 240.0_f32.min(ui.available_width() - 8.0).max(120.0);
                    let (r, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
                    search_box(app, ui, r, t);
                });
            });
        });
}

/// The navigation pane: the tree of every open notebook, and Quick notes pinned at the bottom
/// (catalog `UI.Nav.NotebookPane`, `UI.Nav.NotebookTree`, `UI.Nav.NewSectionLink`,
/// `UI.Nav.QuickNotes`). Hidden with the page column by ☰.
pub fn notebook_pane(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    if app.session.view.notebook_pane_collapsed {
        return;
    }
    let ctx = ui.ctx().clone();
    // OPT.Display.NavBarLeft: the navigation sits on the left of the page, or on its right.
    let panel = if app.session.prefs.nav_left { egui::Panel::left("notebook_pane") } else { egui::Panel::right("notebook_pane") };
    let shown = panel
        .default_size(app.ui.notebook_pane_width)
        .size_range(160.0..=420.0)
        .resizable(true)
        .show_separator_line(false)
        .frame(Frame::NONE.fill(t.window_back).inner_margin(Margin { left: 4, right: 4, top: 2, bottom: 4 }))
        .show(ui, |ui| {
            let full = ui.available_rect_before_wrap();
            let quick_h = size::TREE_ROW + 9.0;
            let tree = Rect::from_min_max(full.min, pos2(full.max.x, full.max.y - quick_h));
            let quick = Rect::from_min_max(pos2(full.min.x, tree.max.y), full.max);
            // CM.NotebookPaneArea — right-click on the empty pane. Registered before the rows,
            // so a row (registered later, so on top) keeps its own menu.
            let area = ui.interact(tree, ui.id().with("nb_area"), Sense::click());
            ui.scope_builder(egui::UiBuilder::new().max_rect(tree), |ui| {
                egui::ScrollArea::vertical().id_salt("nb_scroll").auto_shrink([false, false]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    let notebooks: Vec<_> = app.session.ws.notebooks.clone();
                    for (i, nb) in notebooks.iter().enumerate() {
                        if i > 0 {
                            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
                            ui.painter()
                                .line_segment([pos2(r.min.x + 8.0, r.center().y), pos2(r.max.x - 8.0, r.center().y)], Stroke::new(1.0, t.border));
                        }
                        notebook_row(app, ui, nb, t);
                        if !is_collapsed(app, nb.id) {
                            nodes(app, ui, &nb.children, 1, t);
                            new_section_row(app, ui, nb.id, t);
                        }
                    }
                });
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
            ui.scope_builder(egui::UiBuilder::new().max_rect(quick), |ui| {
                ui.painter().line_segment(
                    [pos2(quick.min.x + 8.0, quick.min.y + 4.0), pos2(quick.max.x - 8.0, quick.min.y + 4.0)],
                    Stroke::new(1.0, t.border),
                );
                ui.add_space(9.0);
                let (rect, resp) = ui.allocate_exact_size(vec2(quick.width(), size::TREE_ROW), Sense::click());
                let on = app.disabled_reason("nav.quickNotes").is_none();
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, on, "Quick notes"));
                row_bg(ui, rect, false, resp.hovered() && on, &t);
                let color = if on { t.text } else { t.text_disabled };
                icons::quick_note(
                    ui.painter(),
                    Rect::from_center_size(pos2(rect.min.x + 30.0, rect.center().y), vec2(size::ICON, size::ICON)),
                    if on { t.icon } else { color },
                );
                ui.painter().text(
                    pos2(rect.min.x + 44.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    crate::i18n::tr("Quick notes"),
                    TypeRung::Body.font(&ctx, Weight::Regular),
                    color,
                );
                if resp.on_hover_text("Open Quick Notes").clicked() && on {
                    app.run(&ctx, "nav.quickNotes", json!({}));
                }
            });
        });
    // The pane is resizable; remember the width the user dragged it to.
    let w = shown.response.rect.width();
    if w.is_finite() && (w - app.ui.notebook_pane_width).abs() > 0.5 {
        app.ui.notebook_pane_width = w;
    }
}

fn notebook_row(app: &mut NoteApp, ui: &mut Ui, nb: &notecraft_engine::doc::Notebook, t: Tokens) {
    let ctx = ui.ctx().clone();
    let selected = app.session.sel.notebook == Some(nb.id);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), size::TREE_ROW), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &nb.name));
    row_bg(ui, rect, false, resp.hovered(), &t);
    let collapsed = is_collapsed(app, nb.id);
    let chev = Rect::from_center_size(pos2(rect.min.x + 12.0, rect.center().y), vec2(14.0, 14.0));
    icons::chevron(ui.painter(), chev, if collapsed { icons::Dir::Right } else { icons::Dir::Down }, t.icon);
    let cover = c32(nb.color.rgba());
    icons::notebook(
        ui.painter(),
        Rect::from_center_size(pos2(rect.min.x + 30.0, rect.center().y), vec2(18.0, 18.0)),
        cover,
        crate::theme::darken(cover, 0.7),
    );
    let renaming = matches!(&app.ui.rename, Some(Rename::Notebook(id, _)) if *id == nb.id);
    let font = TypeRung::Body.font(&ctx, if selected { Weight::Semibold } else { Weight::Regular });
    if renaming {
        rename_field(app, ui, Rect::from_min_max(rect.min + vec2(42.0, 2.0), rect.max - vec2(4.0, 2.0)));
    } else {
        ui.painter().with_clip_rect(rect).text(pos2(rect.min.x + 44.0, rect.center().y), Align2::LEFT_CENTER, &nb.name, font, t.text);
    }
    if resp.clicked() {
        let on_chevron = resp.interact_pointer_pos().is_some_and(|p| p.x < rect.min.x + 22.0);
        if on_chevron || selected {
            toggle_collapsed(app, nb.id);
        } else {
            app.run(&ctx, "notebook.select", json!({"notebook": nb.id.to_string()}));
            app.ui.nav_collapsed.retain(|x| *x != nb.id);
        }
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

/// "+ New section" after a notebook's sections (catalog `UI.Nav.NewSectionLink`).
fn new_section_row(app: &mut NoteApp, ui: &mut Ui, nb: Id, t: Tokens) {
    let ctx = ui.ctx().clone();
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), size::TREE_ROW), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "New section"));
    row_bg(ui, rect, false, resp.hovered(), &t);
    icons::plus(ui.painter(), Rect::from_center_size(pos2(rect.min.x + 30.0, rect.center().y), vec2(14.0, 14.0)), t.accent_text);
    ui.painter().text(
        pos2(rect.min.x + 44.0, rect.center().y),
        Align2::LEFT_CENTER,
        crate::i18n::tr("New section"),
        TypeRung::Body.font(&ctx, Weight::Regular),
        t.accent_text,
    );
    if resp.on_hover_text("Create a New Section (Ctrl+T)").clicked() {
        app.run(&ctx, "section.new", json!({"notebook": nb.to_string()}));
    }
}

fn nodes(app: &mut NoteApp, ui: &mut Ui, children: &[NavNode], depth: usize, t: Tokens) {
    if depth > notecraft_engine::doc::notebook::MAX_GROUP_DEPTH {
        return;
    }
    let ctx = ui.ctx().clone();
    let font = TypeRung::Body.font(&ctx, Weight::Regular);
    let indent = (depth.saturating_sub(1)) as f32 * size::INDENT;
    for n in children {
        match n {
            NavNode::Section(s) => {
                let selected = app.session.sel.section == Some(s.id);
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), size::TREE_ROW), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &s.name));
                row_bg(ui, rect, selected, resp.hovered(), &t);
                // Ctrl+Shift+G (KB.Notebooks.FocusSectionTab) puts the keyboard focus here.
                if selected && app.session.sel.focus == notecraft_engine::selection::FocusArea::SectionTabs {
                    ui.painter().rect_stroke(rect.shrink2(vec2(2.0, 1.0)), radius::CONTROL, Stroke::new(1.5, t.accent), egui::StrokeKind::Inside);
                }
                let x = rect.min.x + 26.0 + indent;
                ui.painter().rect_filled(
                    Rect::from_min_size(pos2(x, rect.center().y - 7.0), vec2(4.0, 14.0)),
                    egui::CornerRadius::same(2),
                    c32(s.color.rgba()),
                );
                let renaming = matches!(&app.ui.rename, Some(Rename::Section(id, _)) if *id == s.id);
                if renaming {
                    rename_field(app, ui, Rect::from_min_max(pos2(x + 14.0, rect.min.y + 2.0), rect.max - vec2(4.0, 2.0)));
                } else {
                    ui.painter().with_clip_rect(rect).text(pos2(x + 18.0, rect.center().y), Align2::LEFT_CENTER, &s.name, font.clone(), t.text);
                }
                // A password-protected section shows a lock, closed while locked (decision D15).
                if s.protection.is_some() {
                    crate::password_ui::draw_lock(ui.painter(), pos2(rect.max.x - 18.0, rect.center().y - 6.0), s.is_locked(), &t);
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
                let open = !is_collapsed(app, g.id);
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), size::TREE_ROW), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &g.name));
                row_bg(ui, rect, false, resp.hovered(), &t);
                let x = rect.min.x + 20.0 + indent;
                icons::chevron(
                    ui.painter(),
                    Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(14.0, 14.0)),
                    if open { icons::Dir::Down } else { icons::Dir::Right },
                    t.icon,
                );
                let renaming = matches!(&app.ui.rename, Some(Rename::Group(id, _)) if *id == g.id);
                if renaming {
                    rename_field(app, ui, Rect::from_min_max(pos2(x + 20.0, rect.min.y + 2.0), rect.max - vec2(4.0, 2.0)));
                } else {
                    ui.painter().with_clip_rect(rect).text(pos2(x + 24.0, rect.center().y), Align2::LEFT_CENTER, &g.name, font.clone(), t.text);
                }
                if resp.clicked() {
                    toggle_collapsed(app, g.id);
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
        // CM.Section.FileSaveAsContextualItem: File › Export with this section chosen.
        if ui.button("Export…").clicked() {
            let ctx = ui.ctx().clone();
            if app.session.sel.section != Some(sid) {
                app.run(&ctx, "nav.goto", json!({"section": s}));
            }
            app.ui.export_scope = "section".into();
            app.ui.backstage = Some(crate::Backstage::Export);
            ui.close();
        }
        cmd_item(app, ui, "Delete", "section.delete", json!({"section": s}));
        ui.separator();
        if ui.button("Move or Copy…").clicked() {
            app.ui.dialog = Some(crate::dialogs::Dialog::MoveSection(sid, String::new()));
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

fn scope_label(k: notecraft_engine::ScopeKind) -> &'static str {
    match k {
        notecraft_engine::ScopeKind::Page => "This Page",
        notecraft_engine::ScopeKind::Section => "This Section",
        notecraft_engine::ScopeKind::SectionGroup => "This Section Group",
        notecraft_engine::ScopeKind::Notebook => "This Notebook",
        notecraft_engine::ScopeKind::AllNotebooks => "All Notebooks",
    }
}

/// Search notebooks: a rounded box with the magnifier, the query, and the scope ▾ (catalog
/// `UI.Nav.SearchBox`, `CM.SearchScope.*`). Its id stays `search_box` (Ctrl+E focuses it).
fn search_box(app: &mut NoteApp, ui: &mut Ui, r: Rect, t: Tokens) {
    let ctx = ui.ctx().clone();
    let id = egui::Id::new("search_box");
    crate::chrome::search_frame(ui, r, &t, id, "Search notebooks");
    icons::search(ui.painter(), Rect::from_center_size(pos2(r.min.x + 14.0, r.center().y), vec2(14.0, 14.0)), t.icon);
    let hint = crate::i18n::tr("Search notebooks");
    let mut q = app.ui.search.clone();
    let edit = Rect::from_min_max(pos2(r.min.x + 26.0, r.min.y + 1.0), pos2(r.max.x - 24.0, r.max.y - 1.0));
    let resp = ui.put(
        edit,
        egui::TextEdit::singleline(&mut q)
            .id(id)
            .frame(Frame::NONE)
            .hint_text(RichText::new(hint).color(t.text_dim))
            .text_color(t.text)
            .font(TypeRung::Body.font(&ctx, Weight::Regular))
            .vertical_align(Align::Center),
    );
    let resp = resp.on_hover_text(format!("Search notebooks (Ctrl+E) · {}", scope_label(app.session.view.scope)));
    if q != app.ui.search {
        app.ui.search = q;
        app.ui.search_open = !app.ui.search.trim().is_empty();
    }
    if resp.gained_focus() && !app.ui.search.is_empty() {
        app.ui.search_open = true;
    }
    // KB.Search.PreviewNext: Down (Up) opens the next (previous) result, the search stays open.
    if resp.has_focus() && !app.ui.search.trim().is_empty() {
        let (down, up) =
            ui.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown), i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)));
        if down || up {
            let q = app.ui.search.clone();
            app.run(&ctx, "search.previewNext", json!({"query": q, "back": up}));
            app.ui.search_open = true;
            resp.request_focus();
        }
    }
    if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.ui.search_open = false;
        app.ui.search.clear();
        app.run(&ctx, "search.dismiss", json!({}));
    }
    // Scope drop-down (catalog CM.SearchScope.*).
    let sb = Rect::from_min_max(pos2(r.max.x - 22.0, r.min.y + 1.0), pos2(r.max.x - 1.0, r.max.y - 1.0));
    let sresp = ui.interact(sb, ui.id().with("scope_btn"), Sense::click()).on_hover_text("Change search scope");
    sresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Change search scope"));
    if sresp.hovered() {
        ui.painter().rect_filled(sb, radius::CONTROL, t.hover);
    }
    icons::chevron(ui.painter(), sb.shrink(4.0), icons::Dir::Down, t.icon);
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
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("search_box_rect"), r));
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
            Frame::popup(ui.style()).fill(t.window_back).show(ui, |ui| {
                ui.set_width(anchor.width().max(300.0));
                ui.label(
                    RichText::new(format!("{total} results · {}", scope_label(app.session.view.scope))).size(11.0).color(t.text.gamma_multiply(0.75)),
                );
                ui.separator();
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    let items = res["results"].as_array().cloned().unwrap_or_default();
                    if items.is_empty() {
                        ui.label("No results.");
                    }
                    let previewed = app.session.view.search_preview.as_ref().filter(|(pq, _)| *pq == q).map(|(_, i)| *i);
                    for (n, it) in items.into_iter().enumerate() {
                        let title = it["title"].as_str().unwrap_or("").to_string();
                        let section = it["section"].as_str().unwrap_or("").to_string();
                        let snippet = it["snippet"].as_str().unwrap_or("").to_string();
                        let page = it["page"].as_str().unwrap_or("").to_string();
                        let resp = ui
                            .vertical(|ui| {
                                ui.label(RichText::new(title).strong().color(t.text));
                                ui.label(RichText::new(format!("{section} · {snippet}")).size(11.0).color(t.text.gamma_multiply(0.7)));
                            })
                            .response
                            .interact(Sense::click());
                        if resp.hovered() || previewed == Some(n) {
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

/// The page column between the notebook tree and the page: Add page and sort on top, then the
/// pages (catalog `UI.Nav.PageList`, `UI.Nav.AddPage`, `UI.Nav.SortPages`, `CM.Page.*`). Hidden
/// with the tree by ☰; its right edge drags its width (also Ctrl+Shift+[ / ]).
pub fn page_list(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    if app.session.view.notebook_pane_collapsed {
        return;
    }
    let ctx = ui.ctx().clone();
    let width = app.session.view.page_list_width;
    let left = app.session.prefs.nav_left;
    let panel = if left { egui::Panel::left("page_list") } else { egui::Panel::right("page_list") };
    let shown = panel
        .exact_size(width)
        .show_separator_line(true)
        .frame(Frame::NONE.fill(t.page_list).inner_margin(Margin { left: 4, right: 4, top: 6, bottom: 4 }))
        .show(ui, |ui| {
            if let Some(nb) = app.session.view.recycle_view {
                recycle_bin(app, ui, nb, t);
                return;
            }
            let Some(section) = app.session.section().cloned() else {
                ui.add_space(10.0);
                ui.label(RichText::new("No section").color(t.text_disabled));
                return;
            };
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            page_header(app, ui, t);
            ui.add_space(4.0);
            // CM.PageListArea — right-click below or between the pages. Registered before the
            // rows, so a row (registered later, so on top) keeps its own menu.
            let area = ui.interact(ui.available_rect_before_wrap(), ui.id().with("pages_area"), Sense::click());
            let row_h = size::PAGE_ROW;
            let font = TypeRung::Body.font(&ctx, Weight::Regular);
            egui::ScrollArea::vertical().id_salt("pages_scroll").auto_shrink([false, false]).show(ui, |ui| {
                let mut shown_i = 0usize;
                for (i, page) in section.pages.iter().enumerate() {
                    if section.page_hidden(i) {
                        continue;
                    }
                    let selected = app.session.sel.page == Some(page.id) || app.session.sel.pages.contains(&page.id);
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::click_and_drag());
                    let page_title = page.display_title();
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &page_title));
                    row_bg(ui, r, selected, resp.hovered(), &t);
                    let indent = size::ROW_PAD + 4.0 + page.level as f32 * size::INDENT;
                    let has_kids = section.page_family(i).len() > 1;
                    if has_kids {
                        let arrow = Rect::from_center_size(pos2(r.max.x - 14.0, r.center().y), vec2(16.0, 16.0));
                        let aresp = ui.interact(arrow, ui.id().with(("collapse", page.id)), Sense::click());
                        icons::chevron(ui.painter(), arrow, if page.collapsed { icons::Dir::Right } else { icons::Dir::Down }, t.icon);
                        if aresp.clicked() {
                            app.run(&ctx, "page.toggleSubpages", json!({"page": page.id.to_string()}));
                        }
                    }
                    let weight_font = if selected { TypeRung::Body.font(&ctx, Weight::Medium) } else { font.clone() };
                    let clip = Rect::from_min_max(r.min, pos2(r.max.x - if has_kids { 26.0 } else { 6.0 }, r.max.y));
                    ui.painter().with_clip_rect(clip).text(
                        pos2(r.min.x + indent, r.center().y),
                        Align2::LEFT_CENTER,
                        page.display_title(),
                        weight_font,
                        t.text,
                    );
                    if resp.clicked() {
                        let (add, range) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                        app.run(&ctx, "nav.goto", json!({"page": page.id.to_string(), "add": add, "range": range}));
                    }
                    page_menu(app, &resp, page.id, has_kids, page.collapsed);
                    // Drag a page onto another position (BEH.Pages.DragReorder); rows are
                    // uniform only while versions are hidden.
                    if resp.drag_stopped()
                        && !app.session.view.page_versions
                        && let Some(pos) = ui.ctx().pointer_interact_pos()
                    {
                        let rows_top = r.min.y - (shown_i as f32) * row_h;
                        let target = (((pos.y - rows_top) / row_h).floor().max(0.0)) as usize;
                        let level = if pos.x > r.min.x + indent + 30.0 { page.level + 1 } else { page.level };
                        if target != i {
                            app.run(&ctx, "page.move", json!({"page": page.id.to_string(), "index": target, "level": level}));
                        }
                    }
                    shown_i += 1;
                    if app.session.view.page_versions {
                        version_rows(app, ui, page, t);
                    }
                    insertion_point(app, ui, r, page.id, i, &section, t);
                }
                ui.allocate_space(vec2(ui.available_width(), 60.0))
            });
            area.context_menu(|ui| {
                cmd_item(app, ui, "New Page", "page.new", json!({}));
                cmd_item(app, ui, "Paste Page", "page.paste", json!({}));
            });
        });
    // Drag the right edge to resize (the canvas is registered later, so the handle sits half
    // inside the column, where it wins).
    let r = shown.response.rect;
    let edge = if left { r.max.x } else { r.min.x };
    let handle = Rect::from_min_max(pos2(edge - 4.0, r.min.y), pos2(edge + 2.0, r.max.y));
    let h = ui.interact(handle, egui::Id::new("page_list_resize"), Sense::drag());
    if h.hovered() || h.dragged() {
        ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if h.dragged() {
        let d = if left { h.drag_delta().x } else { -h.drag_delta().x };
        app.session.view.page_list_width = (width + d).clamp(120.0, 480.0);
    }
}

/// The Notebook Recycle Bin in the page column (History › Notebook Recycle Bin; catalog
/// `UI.Panes.RecycleBin`): the notebook's deleted pages and deleted sections, each with Restore
/// (back where it came from) and Move to (another section) on a right click and a button on
/// hover, and Empty Recycle Bin. Restoring opens the item and leaves the bin; ✕ leaves it too.
fn recycle_bin(app: &mut NoteApp, ui: &mut Ui, nb_id: Id, t: Tokens) {
    let ctx = ui.ctx().clone();
    let Some(nb) = app.session.ws.notebook(nb_id).cloned() else {
        app.session.view.recycle_view = None;
        return;
    };
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(RichText::new("Notebook Recycle Bin").strong().color(t.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(egui::Button::new("✕").frame(false)).on_hover_text("Close the Recycle Bin").clicked() {
                app.session.view.recycle_view = None;
            }
        });
    });
    ui.add_space(4.0);
    let empty = nb.recycle.is_empty();
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        if ui.add_enabled(!empty, egui::Button::new("Empty Recycle Bin")).clicked() {
            app.ui.dialog = Some(crate::dialogs::Dialog::ConfirmEmptyBin);
        }
    });
    ui.add_space(6.0);
    let off = app.session.clock.utc_offset_min;
    let sections: Vec<(Id, String)> = nb.sections().iter().map(|s| (s.id, s.name.clone())).collect();
    egui::ScrollArea::vertical().id_salt("recycle_scroll").auto_shrink([false, false]).show(ui, |ui| {
        if empty {
            ui.add_space(8.0);
            ui.label(
                RichText::new("The Recycle Bin is empty. Deleted pages and sections wait here until you empty it.").size(12.0).color(t.text_dim),
            );
            return;
        }
        let heading = |ui: &mut Ui, text: &str| {
            ui.add_space(6.0);
            ui.label(RichText::new(text).size(12.0).strong().color(t.page_meta));
            ui.add_space(2.0);
        };
        if !nb.recycle.pages.is_empty() {
            heading(ui, "Deleted Pages");
        }
        for d in &nb.recycle.pages {
            let about = format!("From {} · deleted {}", d.from_section_name, d.deleted.short_date(off));
            bin_row(app, ui, &ctx, d.page.id, &d.page.display_title(), &about, &sections, t);
        }
        if !nb.recycle.sections.is_empty() {
            heading(ui, "Deleted Sections");
        }
        for d in &nb.recycle.sections {
            let n = d.section.pages.len();
            let about = format!("{n} page{} · deleted {}", if n == 1 { "" } else { "s" }, d.deleted.short_date(off));
            bin_row(app, ui, &ctx, d.section.id, &d.section.name, &about, &[], t);
        }
    });
}

/// One item of the Recycle Bin: its name and a dim line under it; Restore on hover and in the
/// context menu, and for pages Move to › a section.
#[allow(clippy::too_many_arguments)]
fn bin_row(app: &mut NoteApp, ui: &mut Ui, ctx: &egui::Context, id: Id, name: &str, about: &str, sections: &[(Id, String)], t: Tokens) {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    row_bg(ui, r, false, resp.hovered(), &t);
    let font = TypeRung::Body.font(ctx, Weight::Regular);
    let button = Rect::from_min_size(pos2(r.max.x - 66.0, r.center().y - 12.0), vec2(60.0, 24.0));
    let hot = resp.hovered() || ui.rect_contains_pointer(button);
    // The text runs to the edge, leaving room for Restore only while it shows.
    let clip = Rect::from_min_max(r.min, pos2(r.max.x - if hot { 70.0 } else { 6.0 }, r.max.y));
    ui.painter().with_clip_rect(clip).text(pos2(r.min.x + 10.0, r.min.y + 13.0), Align2::LEFT_CENTER, name, font, t.text);
    ui.painter().with_clip_rect(clip).text(pos2(r.min.x + 10.0, r.min.y + 29.0), Align2::LEFT_CENTER, about, FontId::proportional(11.0), t.page_meta);
    let item = id.to_string();
    // Pages (they have sections to move to) open read-only on a click.
    if resp.clicked() && !sections.is_empty() {
        app.run(ctx, "history.previewDeleted", json!({"item": item}));
    }
    if hot {
        let b = ui.put(button, egui::Button::new("Restore").small());
        if b.on_hover_text("Put it back where it was").clicked() {
            app.run(ctx, "history.restore", json!({"item": item}));
        }
    }
    resp.context_menu(|ui| {
        cmd_item(app, ui, "Restore", "history.restore", json!({"item": item}));
        if !sections.is_empty() {
            ui.menu_button("Move to", |ui| {
                for (sid, sname) in sections {
                    cmd_item(app, ui, sname, "history.restore", json!({"item": item, "section": sid.to_string()}));
                }
            });
        }
    });
}

/// The insertion point under a page tab (BEH.Pages.NewPageInsertion): pointing at the lower
/// edge of a row shows a line with a ⊕ at its left; a click adds a page right after it. The band
/// is the row's last few pixels, so the rest of the row still selects the page; a page with
/// subpages has none (a page there would split the family).
fn insertion_point(app: &mut NoteApp, ui: &mut Ui, row: Rect, pid: Id, i: usize, section: &notecraft_engine::doc::Section, t: Tokens) {
    // Between a page and its own subpages a new page would split the family: only after it.
    // OPT.Display.FloatingNewPage switches the insertion point off.
    if section.page_family(i).end > i + 1 || !app.session.prefs.floating_new_page {
        return;
    }
    let band = Rect::from_min_max(pos2(row.min.x, row.max.y - 4.0), pos2(row.max.x, row.max.y));
    let resp = ui.interact(band, ui.id().with(("insert_page", pid)), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Add page here"));
    if !resp.hovered() {
        return;
    }
    let y = row.max.y;
    let plus = pos2(row.min.x + 9.0, y);
    ui.painter().line_segment([pos2(row.min.x + 16.0, y), pos2(row.max.x - 4.0, y)], Stroke::new(2.0, t.accent));
    ui.painter().circle(plus, 7.0, t.page_list, Stroke::new(1.5, t.accent));
    ui.painter().line_segment([plus - vec2(3.5, 0.0), plus + vec2(3.5, 0.0)], Stroke::new(1.5, t.accent));
    ui.painter().line_segment([plus - vec2(0.0, 3.5), plus + vec2(0.0, 3.5)], Stroke::new(1.5, t.accent));
    if resp.on_hover_text("Add a page here").clicked() {
        let ctx = ui.ctx().clone();
        app.run(&ctx, "page.new", json!({"after": pid.to_string()}));
    }
}

/// Add page (outlined, with the page-and-pencil icon) and the sort button.
fn page_header(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), size::BUTTON + 4.0), Sense::hover());
    let font = TypeRung::Body.font(&ctx, Weight::Regular);
    let tw = ui.painter().layout_no_wrap(crate::i18n::tr("Add page"), font.clone(), t.accent_text).size().x;
    let add = Rect::from_min_size(pos2(row.min.x + 6.0, row.min.y + 2.0), vec2(tw + 36.0, size::BUTTON));
    let resp = ui.interact(add, ui.id().with("add_page"), Sense::click());
    let on = app.disabled_reason("page.new").is_none();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, on, "Add page"));
    let fill = if resp.hovered() && on { t.hover } else { t.page_list };
    ui.painter().rect(add, radius::CONTROL, fill, Stroke::new(1.0, t.card_border), egui::StrokeKind::Inside);
    let color = if on { t.accent_text } else { t.text_disabled };
    icons::add_page(ui.painter(), Rect::from_center_size(pos2(add.min.x + 15.0, add.center().y), vec2(size::ICON, size::ICON)), color);
    ui.painter().text(pos2(add.min.x + 28.0, add.center().y), Align2::LEFT_CENTER, crate::i18n::tr("Add page"), font, color);
    if resp.on_hover_text("Add a page at the end of the section (Ctrl+N)").clicked() && on {
        app.run(&ctx, "page.new", json!({}));
    }
    let sort = Rect::from_min_size(pos2(row.max.x - size::BUTTON - 4.0, row.min.y + 2.0), vec2(size::BUTTON, size::BUTTON));
    let sresp = ui.interact(sort, ui.id().with("sort_pages"), Sense::click());
    let on = app.disabled_reason("page.sort").is_none();
    sresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, on, "Sort pages"));
    if sresp.hovered() && on {
        ui.painter().rect_filled(sort, radius::CONTROL, t.hover);
    }
    icons::sort(ui.painter(), sort.shrink(6.0), if on { t.icon } else { t.text_disabled });
    let sresp = sresp.on_hover_text("Sort pages");
    egui::Popup::menu(&sresp).show(|ui| {
        cmd_item(app, ui, "Title (A to Z)", "page.sort", json!({"by": "title"}));
        cmd_item(app, ui, "Date created (newest first)", "page.sort", json!({"by": "created"}));
        cmd_item(app, ui, "Date modified (newest first)", "page.sort", json!({"by": "modified"}));
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
            app.ui.dialog = Some(crate::dialogs::Dialog::MovePage(pid, String::new()));
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
        if app.session.view.page_versions {
            cmd_item(app, ui, "Hide Page Versions", "history.pageVersions", json!({"on": false}));
        } else {
            cmd_item(app, ui, "Show Page Versions", "history.pageVersions", json!({"on": true}));
        }
    });
}

/// A page's versions under it in the page list (History › Page Versions; decision D14),
/// newest first: date, time and author, indented and dimmer than pages. A click shows the
/// version read-only; the context menu restores or deletes it.
fn version_rows(app: &mut NoteApp, ui: &mut Ui, page: &notecraft_engine::doc::Page, t: Tokens) {
    let ctx = ui.ctx().clone();
    let off = app.session.clock.utc_offset_min;
    let indent = size::ROW_PAD + 22.0 + page.level as f32 * size::INDENT;
    for v in page.versions.iter().rev() {
        let at = v.at.0;
        let label = format!(
            "{} {}{}",
            v.at.short_date(off),
            v.at.short_time(off),
            if v.author.is_empty() { String::new() } else { format!("  {}", v.author) }
        );
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
        let shown = app.session.view.preview == Some(notecraft_engine::Preview::Version { page: page.id, at });
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, shown, &label));
        row_bg(ui, r, shown, resp.hovered(), &t);
        ui.painter().with_clip_rect(r).text(
            pos2(r.min.x + indent, r.center().y),
            Align2::LEFT_CENTER,
            &label,
            FontId::new(12.0, egui::FontFamily::Proportional),
            t.page_meta,
        );
        let p = page.id.to_string();
        if resp.clicked() {
            app.run(&ctx, "history.openVersion", json!({"page": p, "at": at}));
        }
        resp.context_menu(|ui| {
            cmd_item(app, ui, "Restore Version", "history.restoreVersion", json!({"page": p, "at": at}));
            cmd_item(app, ui, "Delete Version", "history.deleteVersion", json!({"page": p, "at": at}));
            // CM.PageVersion.CopyOrMoveToSection: the version as a new page in a section.
            let sections: Vec<(String, String)> =
                app.session.notebook().map(|nb| nb.sections().iter().map(|s| (s.id.to_string(), s.name.clone())).collect()).unwrap_or_default();
            ui.menu_button("Move or Copy to Section", |ui| {
                for (sid, name) in &sections {
                    cmd_item(app, ui, name, "page.moveOrCopy", json!({"page": p, "version": at, "section": sid, "copy": true}));
                }
            });
            ui.separator();
            cmd_item(app, ui, "Delete All Versions in Section", "history.deleteVersionsSection", json!({}));
            cmd_item(app, ui, "Delete All Versions in Section Group", "history.deleteVersionsGroup", json!({}));
            cmd_item(app, ui, "Delete All Versions in Notebook", "history.deleteVersionsNotebook", json!({}));
            ui.separator();
            cmd_item(app, ui, "Hide Page Versions", "history.pageVersions", json!({"on": false}));
        });
    }
}

/// The page area with no notebook open.
pub fn empty_state(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
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
}
