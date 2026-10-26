//! The backstage (File tab): Info, New, Open, Export, Account, Options, Exit (catalog `BS.*`).
//!
//! Pages that need a Microsoft service (Share with People, sign-in) are listed disabled with the
//! reason, never faked.

use egui::{Align, Frame, Layout, Margin, RichText, Sense, Stroke, Ui, vec2};
use serde_json::json;

use crate::{Backstage, NoteApp, ThemeKind, Tokens};

/// Draw the backstage over the whole window.
pub fn show(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    let page = app.ui.backstage.unwrap_or_default();
    egui::Panel::left("backstage_nav").exact_size(190.0).frame(Frame::NONE.fill(t.window_back).inner_margin(Margin::symmetric(0, 10))).show(
        ui,
        |ui| {
            let back = ui
                .horizontal(|ui| {
                    ui.add_space(14.0);
                    let (resp, rect, color) = crate::chrome::icon_button(ui, 32.0, "Back", true, &t);
                    crate::icons::back(ui.painter(), rect.shrink(5.0), color);
                    resp
                })
                .inner
                .on_hover_text("Back (Esc)");
            if back.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                app.ui.backstage = None;
                return;
            }
            ui.add_space(14.0);
            for (p, label) in [
                (Backstage::Info, "Info"),
                (Backstage::New, "New"),
                (Backstage::Open, "Open"),
                (Backstage::Export, "Export"),
                (Backstage::Account, "Account"),
                (Backstage::Options, "Options"),
            ] {
                let sel = page == p;
                let (r, resp) = ui.allocate_exact_size(vec2(190.0, 36.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, sel, label));
                let pill = r.shrink2(vec2(8.0, 2.0));
                if sel {
                    ui.painter().rect_filled(pill, crate::theme::radius::CONTROL, t.nav_selected);
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(pill.min + vec2(0.0, 9.0), vec2(3.0, pill.height() - 18.0)),
                        egui::CornerRadius::same(2),
                        t.accent,
                    );
                } else if resp.hovered() {
                    ui.painter().rect_filled(pill, crate::theme::radius::CONTROL, t.nav_hover);
                }
                ui.painter().text(r.left_center() + vec2(26.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(14.0), t.text);
                if resp.clicked() {
                    app.ui.backstage = Some(p);
                }
            }
            for (label, why) in [
                ("Print", "Printing comes with M3 (PDF export first)"),
                ("Share", "Sharing needs a Microsoft account; NoteCraft syncs folders instead"),
                ("Send", "Email integration is not available yet"),
            ] {
                let (r, resp) = ui.allocate_exact_size(vec2(190.0, 38.0), Sense::hover());
                ui.painter().text(
                    r.left_center() + vec2(26.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(14.0),
                    t.text_disabled,
                );
                resp.on_hover_text(why);
            }
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                ui.add_space(8.0);
                let (r, resp) = ui.allocate_exact_size(vec2(190.0, 34.0), Sense::click());
                ui.painter().text(r.left_center() + vec2(26.0, 0.0), egui::Align2::LEFT_CENTER, "Exit", egui::FontId::proportional(14.0), t.text);
                if resp.clicked() {
                    let _ = app.session.save_dirty();
                    app.quit_requested = true;
                }
            });
        },
    );
    egui::CentralPanel::default().frame(Frame::NONE.fill(t.ribbon_card).inner_margin(Margin::symmetric(40, 30))).show(ui, |ui| {
        ui.visuals_mut().override_text_color = Some(t.text);
        match page {
            Backstage::Info => info(app, ui, &ctx, t),
            Backstage::New => new_notebook(app, ui, &ctx, t),
            Backstage::Open => open(app, ui, &ctx, t),
            Backstage::Export => export(app, ui, &ctx),
            Backstage::Account => account(app, ui, t),
            Backstage::Options => options(app, ui, &ctx, t),
        }
    });
}

fn heading(ui: &mut Ui, s: &str) {
    ui.label(RichText::new(s).size(28.0));
    ui.add_space(16.0);
}

fn info(app: &mut NoteApp, ui: &mut Ui, ctx: &egui::Context, t: Tokens) {
    heading(ui, "Notebook Information");
    let notebooks: Vec<_> = app.session.ws.notebooks.clone();
    if notebooks.is_empty() {
        ui.label("No notebook is open.");
    }
    for nb in notebooks {
        Frame::NONE.stroke(Stroke::new(1.0, t.border)).inner_margin(Margin::same(12)).show(ui, |ui| {
            ui.set_width(520.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(22.0, 28.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, crate::theme::c32(nb.color.rgba()));
                ui.vertical(|ui| {
                    ui.label(RichText::new(&nb.name).size(16.0).strong());
                    #[cfg(not(target_arch = "wasm32"))]
                    let loc = app
                        .session
                        .notebook_path(nb.id)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "In memory (not saved to a folder)".into());
                    #[cfg(target_arch = "wasm32")]
                    let loc = "In this browser".to_string();
                    ui.label(RichText::new(loc).size(12.0).color(t.page_meta));
                    ui.label(
                        RichText::new(format!(
                            "{} sections · {} pages",
                            nb.sections().len(),
                            nb.sections().iter().map(|s| s.pages.len()).sum::<usize>()
                        ))
                        .size(12.0)
                        .color(t.page_meta),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.menu_button("Settings ▾", |ui| {
                        if ui.button("Properties…").clicked() {
                            app.ui.dialog = Some(crate::dialogs::Dialog::NotebookProperties(nb.id, nb.name.clone()));
                            ui.close();
                        }
                        crate::nav::cmd_item(app, ui, "Close", "notebook.close", json!({"notebook": nb.id.to_string()}));
                    });
                });
            });
        });
        ui.add_space(8.0);
    }
    ui.add_space(12.0);
    if let Some(folder) = app.host.backup_folder() {
        if ui.button("Back Up All Notebooks Now").clicked() {
            app.run(ctx, "file.backupNow", json!({"folder": folder}));
        }
        if ui.button("Open Backups").clicked()
            && let Some(r) = app.run(ctx, "file.openBackups", json!({"folder": folder}))
        {
            app.ui.status = Some((format!("Backups: {}", r), ctx.input(|i| i.time)));
        }
    }
}

fn new_notebook(app: &mut NoteApp, ui: &mut Ui, ctx: &egui::Context, t: Tokens) {
    heading(ui, "New Notebook");
    let id = egui::Id::new("new_nb_name");
    let mut name: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_else(|| "My Notebook".to_string()));
    ui.label("Notebook Name");
    ui.add(egui::TextEdit::singleline(&mut name).desired_width(360.0));
    ui.data_mut(|d| d.insert_temp(id, name.clone()));
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui.button("Create in a Folder…").clicked()
            && let Some(dir) = app.host.pick_folder("Choose where to create the notebook")
        {
            let path = std::path::Path::new(&dir).join(&name);
            if app.run(ctx, "file.newNotebook", json!({"name": name, "path": path.to_string_lossy()})).is_some() {
                app.ui.backstage = None;
            }
        }
        if ui.button("Create in Memory").on_hover_text("A notebook that lives only in this session (export it to keep it)").clicked()
            && app.run(ctx, "file.newNotebook", json!({"name": name})).is_some()
        {
            app.ui.backstage = None;
        }
    });
    ui.add_space(20.0);
    ui.label(
        RichText::new("OneDrive and SharePoint locations need a Microsoft account and are not available in NoteCraft.").size(12.0).color(t.page_meta),
    );
}

fn open(app: &mut NoteApp, ui: &mut Ui, ctx: &egui::Context, t: Tokens) {
    heading(ui, "Open Notebook");
    if ui.button("Browse…  (Ctrl+O)").clicked()
        && let Some(dir) = app.host.pick_folder("Open a notebook folder")
        && app.run(ctx, "file.openNotebook", json!({"path": dir})).is_some()
    {
        app.ui.backstage = None;
    }
    ui.add_space(14.0);
    // BS.Open.RecentNotebooks.
    ui.label(RichText::new("Recent Notebooks").strong());
    if app.ui.recent_notebooks.is_empty() {
        ui.label(RichText::new("Notebooks you open appear here.").size(12.0).color(t.page_meta));
    } else {
        for path in app.ui.recent_notebooks.clone() {
            let name = std::path::Path::new(&path).file_name().and_then(|n| n.to_str()).unwrap_or(&path);
            if ui.button(name).on_hover_text(&path).clicked() && app.run(ctx, "file.openRecent", json!({"path": path})).is_some() {
                app.ui.backstage = None;
            }
        }
    }
    ui.add_space(14.0);
    // BS.Open.OpenSection: the section is copied into the current notebook (decision D12).
    let has_notebook = app.session.notebook().is_some();
    let button = ui
        .add_enabled(has_notebook, egui::Button::new("Open Section…  (Ctrl+Alt+Shift+O)"))
        .on_hover_text("Import a OneNote section (.one) into this notebook as a copy; the file itself is not changed")
        .on_disabled_hover_text("Open a notebook first");
    if button.clicked() {
        crate::pictures::host_action(app, ctx, "file.openSection");
        app.ui.backstage = None;
    }
    ui.label(RichText::new("OneNote packages (.onepkg) and notebooks (.onetoc2): coming later in M3.").size(12.0).color(t.page_meta));
}

fn export(app: &mut NoteApp, ui: &mut Ui, ctx: &egui::Context) {
    heading(ui, "Export");
    ui.label("Export the current notebook as a single NoteCraft archive (.ncraft-archive).");
    if ui.button("Export Notebook…").clicked() {
        let name = app.session.notebook().map(|n| format!("{}.ncraft-archive", n.name)).unwrap_or_else(|| "notebook.ncraft-archive".into());
        if let Some(path) = app.host.save_file("Export notebook", &name) {
            app.run(ctx, "file.exportNotebook", json!({"path": path}));
        }
    }
    ui.add_space(16.0);
    for (fmt, when) in [
        ("OneNote Section (.one)", "M3"),
        ("OneNote Package (.onepkg)", "M3"),
        ("Word Document (.docx)", "M3"),
        ("PDF (.pdf)", "M3"),
        ("Single File Web Page (.mht)", "M3"),
    ] {
        ui.add_enabled(false, egui::Button::new(fmt)).on_disabled_hover_text(format!("Arrives with {when}"));
    }
}

/// The Office Theme choices, one selectable label each (File › Account and File › Options).
fn theme_picker(app: &mut NoteApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        for k in ThemeKind::ALL {
            if ui.selectable_label(app.ui.theme == k, k.label()).clicked() {
                app.ui.theme = k;
            }
        }
    });
}

fn account(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    heading(ui, "Account");
    ui.label(RichText::new("Office Theme").strong());
    // OPT.General.OfficeTheme / UI.Window.DarkMode.
    theme_picker(app, ui);
    ui.add_space(20.0);
    ui.label(RichText::new("About NoteCraft").strong());
    ui.label(format!("NoteCraft {} — an open-source, local-first notebook. MIT OR Apache-2.0.", env!("CARGO_PKG_VERSION")));
    ui.label(
        RichText::new("Not affiliated with Microsoft. OneNote is a trademark of Microsoft Corporation, named only to describe compatibility.")
            .size(12.0)
            .color(t.page_meta),
    );
}

fn options(app: &mut NoteApp, ui: &mut Ui, ctx: &egui::Context, t: Tokens) {
    heading(ui, "Options");
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label(RichText::new("General").strong());
        let mut author = app.session.prefs.author.clone();
        ui.horizontal(|ui| {
            ui.label("User name");
            if ui.text_edit_singleline(&mut author).changed() {
                app.run(ctx, "file.options", json!({"set": {"author": author}}));
            }
        });
        ui.add_space(6.0);
        // OPT.General.OfficeTheme (also in File › Account, UI.Window.DarkMode).
        ui.label("Office Theme");
        theme_picker(app, ui);
        ui.add_space(10.0);
        ui.label(RichText::new("Advanced › Editing").strong());
        let mut bullets = app.session.prefs.auto_bullets;
        if ui.checkbox(&mut bullets, "Apply auto-bulleting to lists").changed() {
            app.run(ctx, "file.options", json!({"set": {"auto_bullets": bullets}}));
        }
        let mut numbers = app.session.prefs.auto_numbering;
        if ui.checkbox(&mut numbers, "Apply auto-numbering to lists").changed() {
            app.run(ctx, "file.options", json!({"set": {"auto_numbering": numbers}}));
        }
        let mut wiki = app.session.prefs.wiki_links;
        if ui.checkbox(&mut wiki, "Enable wiki linking ([[Page]])").changed() {
            app.run(ctx, "file.options", json!({"set": {"wiki_links": wiki}}));
        }
        ui.add_space(10.0);
        ui.label(RichText::new("Advanced › Default font").strong());
        let mut font = app.session.prefs.default_font.clone();
        ui.horizontal(|ui| {
            ui.label("Font");
            if ui.text_edit_singleline(&mut font).lost_focus() {
                app.run(ctx, "file.options", json!({"set": {"default_font": font}}));
            }
        });
        let ui = &mut cols[1];
        ui.label(RichText::new("Save & Backup").strong());
        let mut copies = app.session.prefs.backup_copies as u32;
        ui.horizontal(|ui| {
            ui.label("Number of backup copies of each file");
            if ui.add(egui::DragValue::new(&mut copies).range(1..=100)).changed() {
                app.run(ctx, "file.options", json!({"set": {"backup_copies": copies}}));
            }
        });
        ui.label(RichText::new("Notes are saved automatically about a second after each change.").size(12.0).color(t.page_meta));
        ui.add_space(10.0);
        ui.label(RichText::new("Display").strong());
        let mut rules = app.session.prefs.new_pages_rule_lines.is_some();
        if ui.checkbox(&mut rules, "Create all new pages with rule lines").changed() {
            app.run(ctx, "view.newPagesWithRuleLines", json!({}));
        }
    });
    ui.add_space(16.0);
    ui.label(
        RichText::new(
            "More option pages (Proofing, Send to OneNote, Audio & Video, Language, Customize Ribbon, Quick Access Toolbar) arrive with M4.",
        )
        .size(12.0)
        .color(t.page_meta),
    );
}
