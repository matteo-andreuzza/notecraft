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
                (Backstage::Print, "Print"),
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
            for (label, why) in
                [("Share", "Sharing needs a Microsoft account; NoteCraft syncs folders instead"), ("Send", "Email integration is not available yet")]
            {
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
            Backstage::Print => print(app, ui, &ctx),
            Backstage::Account => account(app, ui, t),
            // File › Options opens the Options dialog over the backstage (UI.Dialogs.Options).
            Backstage::Options => {
                info(app, ui, &ctx, t);
                crate::options::show(app, &ctx, t);
            }
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
    // BS.Open.OpenNotebook on a file: a OneNote package or a NoteCraft archive, opened as a
    // copy in memory.
    if ui
        .button("Open a Package or Archive…")
        .on_hover_text("Open a OneNote package (.onepkg) or a NoteCraft archive (.ncraft-archive) as a copy in memory")
        .clicked()
        && let Some(path) = app.host.pick_file("Open Notebook", &[("OneNote packages and NoteCraft archives", &["onepkg", "ncraft-archive"])])
        && app.run(ctx, "file.openNotebook", json!({"path": path})).is_some()
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
    ui.add_space(8.0);
    // NC.File.Import: Markdown, text or HTML as a new page of the current section.
    let has_section = app.session.section().is_some();
    let button = ui
        .add_enabled(has_section, egui::Button::new("Import File…"))
        .on_hover_text("Add a Markdown, text or HTML file to this section as a new page, or an Evernote export (.enex) as a new section")
        .on_disabled_hover_text("Open a section first");
    if button.clicked() {
        crate::pictures::host_action(app, ctx, "file.import");
        app.ui.backstage = None;
    }
}

/// File › Print (BS.Print.Print, BS.Print.Preview): printer, copies, what to print; Print
/// sends the print-ready PDF (`file.print`) to the system's spooler, Print Preview opens it in
/// the system's PDF viewer.
fn print(app: &mut NoteApp, ui: &mut Ui, ctx: &egui::Context) {
    heading(ui, "Print");
    let t = app.tokens();
    if app.ui.printers.is_none() {
        let list = app.host.printers();
        if app.ui.print_printer.as_ref().is_none_or(|p| !list.iter().any(|(n, _)| n == p)) {
            app.ui.print_printer = list.iter().find(|(_, d)| *d).or(list.first()).map(|(n, _)| n.clone());
        }
        app.ui.printers = Some(list);
    }
    let printers = app.ui.printers.clone().unwrap_or_default();
    let has_section = app.session.section().is_some();
    if app.ui.print_scope != "section" || !has_section {
        app.ui.print_scope = "page".into();
    }
    egui::Grid::new("print_form").num_columns(2).spacing(vec2(16.0, 10.0)).show(ui, |ui| {
        ui.label("Printer");
        if printers.is_empty() {
            ui.label(RichText::new("No printer found (Print Preview still works)").color(t.page_meta));
        } else {
            let shown = app.ui.print_printer.clone().unwrap_or_else(|| "Default printer".into());
            egui::ComboBox::from_id_salt("print_printer").selected_text(shown).width(260.0).show_ui(ui, |ui| {
                for (name, default) in &printers {
                    let label = if *default { format!("{name} (default)") } else { name.clone() };
                    ui.selectable_value(&mut app.ui.print_printer, Some(name.clone()), label);
                }
            });
        }
        ui.end_row();
        ui.label("Copies");
        ui.add(egui::DragValue::new(&mut app.ui.print_copies).range(1..=99));
        ui.end_row();
        ui.label("Print");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.ui.print_scope, "page".to_string(), "Current page");
            if ui.add_enabled(has_section, egui::Button::selectable(app.ui.print_scope == "section", "Whole section")).clicked() {
                app.ui.print_scope = "section".into();
            }
        });
        ui.end_row();
    });
    ui.add_space(16.0);
    let scope = app.ui.print_scope.clone();
    ui.horizontal(|ui| {
        let can_print = !printers.is_empty();
        let print = ui.add_enabled(can_print, egui::Button::new("Print  (Ctrl+P)")).on_disabled_hover_text("No printer found");
        if print.clicked()
            && let Some(r) = app.run(ctx, "file.print", json!({"scope": scope}))
        {
            let path = r.get("path").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
            let title = r.get("title").and_then(serde_json::Value::as_str).unwrap_or("NoteCraft").to_string();
            let copies = app.ui.print_copies.clamp(1, 99);
            let msg = match app.host.print_pdf(&path, app.ui.print_printer.as_deref(), copies, &title) {
                Ok(m) => format!("Sent to the printer. {m}"),
                Err(e) => e,
            };
            app.ui.status = Some((msg, ctx.input(|i| i.time)));
            app.ui.backstage = None;
        }
        if ui.button("Print Preview").clicked()
            && let Some(r) = app.run(ctx, "file.printPreview", json!({"scope": scope}))
        {
            let path = r.get("path").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
            if let Err(e) = app.host.open_path(&path) {
                app.ui.status = Some((e, ctx.input(|i| i.time)));
            }
        }
    });
    ui.add_space(8.0);
    ui.label(RichText::new("Pages print on their Paper Size (else Letter), scaled down to fit the width.").size(12.0).color(t.page_meta));
}

/// One format of the Export page: label, file extension, and the command that writes it
/// (`None`: not available yet).
type ExportFormat = (&'static str, &'static str, Option<&'static str>);

/// The formats offered for each scope: the reference's list (BS.Export.Format*), then
/// NoteCraft's own (HTML, Markdown, the notebook archive).
fn export_formats(scope: &str) -> &'static [ExportFormat] {
    match scope {
        "section" => &[
            ("OneNote Section (*.one)", "one", None),
            ("PDF (*.pdf)", "pdf", Some("file.exportSection")),
            ("XPS (*.xps)", "xps", None),
            ("Single File Web Page (*.mht)", "mht", Some("file.exportSection")),
            ("Web Page (*.html)", "html", Some("file.exportSection")),
            ("Markdown (*.md)", "md", Some("file.exportSection")),
        ],
        "notebook" => &[
            ("OneNote Package (*.onepkg)", "onepkg", None),
            ("PDF (*.pdf)", "pdf", Some("file.exportNotebook")),
            ("XPS (*.xps)", "xps", None),
            ("NoteCraft Archive (*.ncraft-archive)", "ncraft-archive", Some("file.exportNotebook")),
        ],
        _ => &[
            ("OneNote 2010-2016 Section (*.one)", "one", None),
            ("Word Document (*.docx)", "docx", Some("file.exportPage")),
            ("Word 97-2003 Document (*.doc)", "doc", None),
            ("PDF (*.pdf)", "pdf", Some("file.exportPage")),
            ("XPS (*.xps)", "xps", None),
            ("Single File Web Page (*.mht)", "mht", Some("file.exportPage")),
            ("Web Page (*.html)", "html", Some("file.exportPage")),
            ("PNG Picture (*.png)", "png", Some("file.exportPage")),
            ("SVG Picture (*.svg)", "svg", Some("file.exportPage")),
            ("Markdown (*.md)", "md", Some("file.exportPage")),
        ],
    }
}

/// File › Export: 1. Export Current (Page, Section, Notebook), 2. Select Format, Export.
fn export(app: &mut NoteApp, ui: &mut Ui, ctx: &egui::Context) {
    heading(ui, "Export");
    let t = app.tokens();
    let has = [
        ("page", "Page", app.session.page().is_some()),
        ("section", "Section", app.session.section().is_some()),
        ("notebook", "Notebook", app.session.notebook().is_some()),
    ];
    if !has.iter().any(|(k, _, ok)| *k == app.ui.export_scope && *ok) {
        app.ui.export_scope = has.iter().find(|h| h.2).map(|h| h.0).unwrap_or("page").to_string();
    }
    let formats = export_formats(&app.ui.export_scope);
    if !formats.iter().any(|f| f.1 == app.ui.export_format && f.2.is_some()) {
        app.ui.export_format = formats.iter().find(|f| f.2.is_some()).map(|f| f.1).unwrap_or_default().to_string();
    }
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(200.0);
            ui.label(RichText::new("1. Export Current:").strong());
            ui.add_space(4.0);
            // BS.Export.Page, BS.Export.Section, BS.Export.Notebook.
            for (key, label, ok) in has {
                let r = ui.add_enabled(ok, egui::Button::selectable(app.ui.export_scope == key, label));
                if r.clicked() {
                    app.ui.export_scope = key.to_string();
                }
            }
        });
        ui.add_space(24.0);
        ui.vertical(|ui| {
            ui.label(RichText::new("2. Select Format:").strong());
            ui.add_space(4.0);
            // BS.Export.Format*: the ones not written yet are shown disabled.
            for (label, ext, cmd) in formats {
                let r = ui
                    .add_enabled(cmd.is_some(), egui::Button::selectable(app.ui.export_format == *ext, *label))
                    .on_disabled_hover_text("Not available yet: arrives later in M3");
                if r.clicked() {
                    app.ui.export_format = (*ext).to_string();
                }
            }
            ui.add_space(12.0);
            let chosen = formats.iter().find(|f| f.1 == app.ui.export_format).and_then(|f| f.2.map(|c| (f.1, c)));
            if ui.add_enabled(chosen.is_some(), egui::Button::new("Export…")).clicked()
                && let Some((ext, cmd)) = chosen
            {
                let name = match app.ui.export_scope.as_str() {
                    "section" => app.session.section().map(|s| s.name.clone()),
                    "notebook" => app.session.notebook().map(|n| n.name.clone()),
                    _ => app.session.page().map(|p| p.display_title()),
                }
                .unwrap_or_else(|| "export".into());
                let name = format!("{}.{ext}", name.replace(['/', '\\'], "_"));
                if let Some(path) = app.host.save_file("Export", &name) {
                    let params = if ext == "ncraft-archive" { json!({"path": path}) } else { json!({"path": path, "format": ext}) };
                    if let Some(r) = app.run(ctx, cmd, params) {
                        if r.get("inkLost").and_then(serde_json::Value::as_u64).is_some_and(|n| n > 0) {
                            app.ui.status = Some(("Exported. Ink drawings are not included in this format.".into(), ctx.input(|i| i.time)));
                        }
                        app.ui.backstage = None;
                    }
                }
            }
            if app.ui.export_format == "md" {
                ui.label(RichText::new("Pictures and attached files are saved in a folder next to the Markdown file.").size(12.0).color(t.page_meta));
            }
        });
    });
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
