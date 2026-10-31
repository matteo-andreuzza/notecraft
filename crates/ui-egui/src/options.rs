//! The OneNote Options dialog (File › Options, catalog `UI.Dialogs.Options` and the `OPT.*`
//! rows): the categories down the left, the settings of the chosen one on the right, OK and
//! Cancel at the bottom. Settings are edited on a draft of the engine's [`Prefs`] and applied
//! together with OK (`file.options`), so Cancel leaves everything as it was; the Office Theme
//! is a view setting and changes at once, as in File › Account. Categories NoteCraft can't
//! offer yet say so instead of showing controls that do nothing.

use egui::{RichText, Ui, vec2};
use notecraft_engine::Prefs;
use serde_json::json;

use crate::NoteApp;
use crate::theme::Tokens;

/// The categories of the dialog, in the reference's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum OptionsPage {
    /// User name, initials, theme, Mini Toolbar.
    #[default]
    General,
    /// Rule lines, note containers.
    Display,
    /// Spelling.
    Proofing,
    /// Quick Notes, backups, default notebook location.
    SaveBackup,
    /// Where sent content goes.
    SendToOneNote,
    /// Recording devices.
    AudioVideo,
    /// Display and editing languages.
    Language,
    /// Editing, default font, paste, tables.
    Advanced,
    /// Ribbon customization.
    CustomizeRibbon,
    /// The Quick Access Toolbar.
    QuickAccessToolbar,
    /// Add-ins.
    AddIns,
    /// Privacy.
    TrustCenter,
}

impl OptionsPage {
    /// Every category, in order.
    pub const ALL: [OptionsPage; 12] = [
        OptionsPage::General,
        OptionsPage::Display,
        OptionsPage::Proofing,
        OptionsPage::SaveBackup,
        OptionsPage::SendToOneNote,
        OptionsPage::AudioVideo,
        OptionsPage::Language,
        OptionsPage::Advanced,
        OptionsPage::CustomizeRibbon,
        OptionsPage::QuickAccessToolbar,
        OptionsPage::AddIns,
        OptionsPage::TrustCenter,
    ];

    /// The category's name in the list.
    pub fn label(self) -> &'static str {
        match self {
            OptionsPage::General => "General",
            OptionsPage::Display => "Display",
            OptionsPage::Proofing => "Proofing",
            OptionsPage::SaveBackup => "Save & Backup",
            OptionsPage::SendToOneNote => "Send to NoteCraft",
            OptionsPage::AudioVideo => "Audio & Video",
            OptionsPage::Language => "Language",
            OptionsPage::Advanced => "Advanced",
            OptionsPage::CustomizeRibbon => "Customize Ribbon",
            OptionsPage::QuickAccessToolbar => "Quick Access Toolbar",
            OptionsPage::AddIns => "Add-ins",
            OptionsPage::TrustCenter => "Trust Center",
        }
    }
}

/// Automatic backup intervals offered, in minutes (0: never). The reference's list runs from
/// one minute to six weeks (unverified).
const BACKUP_INTERVALS: [(u32, &str); 15] = [
    (0, "Never"),
    (1, "1 Minute"),
    (5, "5 Minutes"),
    (10, "10 Minutes"),
    (15, "15 Minutes"),
    (30, "30 Minutes"),
    (60, "1 Hour"),
    (120, "2 Hours"),
    (240, "4 Hours"),
    (480, "8 Hours"),
    (1440, "1 Day"),
    (2 * 1440, "2 Days"),
    (7 * 1440, "1 Week"),
    (14 * 1440, "2 Weeks"),
    (6 * 7 * 1440, "6 Weeks"),
];

/// Draw the dialog (File › Options). Closing it, with OK or Cancel, closes the backstage too.
pub(crate) fn show(app: &mut NoteApp, ctx: &egui::Context, t: Tokens) {
    let mut draft = app.ui.options_draft.take().unwrap_or_else(|| app.session.prefs.clone());
    let mut close = None;
    let modal = egui::Modal::new(egui::Id::new("options")).show(ctx, |ui| {
        ui.set_width(760.0);
        ui.label(RichText::new(crate::i18n::tr("NoteCraft Options")).size(16.0).strong());
        ui.add_space(8.0);
        // Fixed height: a separator or layout that takes the available height would make the
        // centred modal grow (and move) every frame.
        let (rect, _) = ui.allocate_exact_size(vec2(760.0, 470.0), egui::Sense::hover());
        let mut cols = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Min)));
        cols.vertical(|ui| {
            ui.set_width(170.0);
            for page in OptionsPage::ALL {
                // Stable ids: the modal's first (sizing) frame lays out differently.
                let picked =
                    ui.push_id(page.label(), |ui| ui.selectable_label(app.ui.options_page == page, crate::i18n::tr(page.label())).clicked()).inner;
                if picked {
                    app.ui.options_page = page;
                }
            }
        });
        cols.add_space(12.0);
        cols.vertical(|ui| {
            ui.set_width(570.0);
            egui::ScrollArea::vertical().max_height(470.0).auto_shrink([false, false]).show(ui, |ui| page(app, ui, ctx, &mut draft, t));
        });
        ui.add_space(8.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Cancel").clicked() {
                    close = Some(false);
                }
                if crate::chrome::primary_button(ui, "OK", &t).clicked() {
                    close = Some(true);
                }
            });
        });
    });
    if modal.should_close() && close.is_none() {
        close = Some(false);
    }
    match close {
        Some(ok) => {
            if ok {
                apply(app, ctx, &draft);
            }
            app.ui.backstage = None;
        }
        None => app.ui.options_draft = Some(draft),
    }
}

/// Send what changed in `draft` to the engine (one `file.options` call; rule lines through
/// their own command).
fn apply(app: &mut NoteApp, ctx: &egui::Context, draft: &Prefs) {
    let (Ok(now), Ok(want)) = (serde_json::to_value(&app.session.prefs), serde_json::to_value(draft)) else { return };
    let (Some(now), Some(want)) = (now.as_object(), want.as_object()) else { return };
    let mut set = serde_json::Map::new();
    for (k, v) in want {
        if now.get(k) != Some(v) && k != "new_pages_rule_lines" {
            set.insert(k.clone(), v.clone());
        }
    }
    if !set.is_empty() {
        app.run(ctx, "file.options", json!({"set": set}));
    }
    if draft.new_pages_rule_lines.is_some() != app.session.prefs.new_pages_rule_lines.is_some() {
        app.run(ctx, "view.newPagesWithRuleLines", json!({}));
    }
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).strong());
    ui.add_space(2.0);
}

fn not_yet(ui: &mut Ui, t: Tokens, what: &str) {
    ui.label(RichText::new(what).size(12.0).color(t.page_meta));
}

/// A folder setting: a text field and a Browse… button (`empty` explains an empty value).
fn folder(app: &mut NoteApp, ui: &mut Ui, label: &str, value: &mut String, empty: &str) {
    ui.label(label);
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(value).desired_width(360.0).hint_text(empty));
        if ui.button("Browse…").clicked()
            && let Some(dir) = app.host.pick_folder(label)
        {
            *value = dir;
        }
    });
}

fn page(app: &mut NoteApp, ui: &mut Ui, ctx: &egui::Context, d: &mut Prefs, t: Tokens) {
    match app.ui.options_page {
        OptionsPage::General => {
            section(ui, "Personalize your copy of NoteCraft");
            egui::Grid::new("opt-general").num_columns(2).show(ui, |ui| {
                // OPT.General.UserName, OPT.General.Initials.
                ui.label("User name:");
                ui.text_edit_singleline(&mut d.author);
                ui.end_row();
                ui.label("Initials:");
                ui.add(egui::TextEdit::singleline(&mut d.initials).char_limit(9).desired_width(60.0));
                ui.end_row();
            });
            // OPT.General.OfficeTheme (applies at once, like File › Account).
            ui.label("Office Theme");
            ui.horizontal(|ui| {
                for k in crate::ThemeKind::ALL {
                    if ui.selectable_label(app.ui.theme == k, k.label()).clicked() {
                        app.ui.theme = k;
                    }
                }
            });
            section(ui, "User Interface options");
            // OPT.General.ShowMiniToolbar.
            ui.checkbox(&mut d.show_mini_toolbar, "Show Mini Toolbar on selection");
        }
        OptionsPage::Display => {
            section(ui, "Display");
            // OPT.Display.NewPagesRuleLines.
            let mut rules = d.new_pages_rule_lines.is_some();
            if ui.checkbox(&mut rules, "Create all new pages with rule lines").changed() {
                d.new_pages_rule_lines = rules.then_some(notecraft_engine::doc::RuleLines::College);
            }
            // OPT.Display.ShowNoteContainers.
            ui.checkbox(&mut d.show_note_containers, "Show note containers on pages");
            // OPT.Display.NavBarLeft.
            ui.checkbox(&mut d.nav_left, "Navigation bar appears on the left");
            // OPT.Display.FloatingNewPage.
            ui.checkbox(&mut d.floating_new_page, "Show the floating New Page button near page tabs");
        }
        OptionsPage::SaveBackup => {
            section(ui, "Save");
            // OPT.SaveBackup.QuickNotesSection: the notebook holding Quick Notes.
            let names: Vec<String> = app.session.ws.notebooks.iter().map(|n| n.name.clone()).collect();
            ui.label("Quick Notes section (in the notebook):");
            let shown = if d.quick_notes_notebook.is_empty() { "The first open notebook".to_string() } else { d.quick_notes_notebook.clone() };
            egui::ComboBox::from_id_salt("opt-quick-notes").width(300.0).selected_text(shown).show_ui(ui, |ui| {
                ui.selectable_value(&mut d.quick_notes_notebook, String::new(), "The first open notebook");
                for n in names {
                    ui.selectable_value(&mut d.quick_notes_notebook, n.clone(), n);
                }
            });
            // OPT.SaveBackup.BackupFolder, OPT.SaveBackup.DefaultNotebookLocation.
            folder(app, ui, "Backup folder:", &mut d.backup_folder, "NoteCraft's own backups folder");
            folder(app, ui, "Default notebook location:", &mut d.default_notebook_location, "Ask each time");
            section(ui, "Backup");
            // OPT.SaveBackup.AutoBackup.
            ui.horizontal(|ui| {
                ui.label("Automatically back up my notebook at the following time interval:");
                let shown = BACKUP_INTERVALS.iter().find(|(m, _)| *m == d.auto_backup_minutes).map_or("Custom", |(_, l)| l);
                egui::ComboBox::from_id_salt("opt-backup-every").selected_text(shown).show_ui(ui, |ui| {
                    for (m, l) in BACKUP_INTERVALS {
                        ui.selectable_value(&mut d.auto_backup_minutes, m, l);
                    }
                });
            });
            // OPT.SaveBackup.BackupCopies.
            ui.horizontal(|ui| {
                ui.label("Number of backup copies of each file:");
                let mut copies = d.backup_copies as u32;
                if ui.add(egui::DragValue::new(&mut copies).range(1..=100)).changed() {
                    d.backup_copies = copies as usize;
                }
            });
            // OPT.SaveBackup.BackupChangedNow, OPT.SaveBackup.BackupAllNow: with the folder
            // as it stands in the dialog.
            let folder = Some(d.backup_folder.trim().to_string()).filter(|f| !f.is_empty()).or_else(|| app.host.backup_folder());
            ui.add_enabled_ui(folder.is_some(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Back Up Changed Files Now").clicked()
                        && let Some(f) = &folder
                    {
                        app.run(ctx, "file.backupChanged", json!({"folder": f}));
                    }
                    if ui.button("Back Up All Notebooks Now").clicked()
                        && let Some(f) = &folder
                    {
                        app.run(ctx, "file.backupNow", json!({"folder": f}));
                    }
                });
            });
            ui.label(RichText::new("Notes are saved automatically about a second after each change.").size(12.0).color(t.page_meta));
        }
        OptionsPage::Advanced => {
            section(ui, "Editing");
            // OPT.Advanced.AutoBulleting, AutoNumbering, CalculateMath, CreateTableOnTab,
            // WikiLinks, ShowPasteOptions.
            ui.checkbox(&mut d.auto_bullets, "Apply auto-bulleting to lists");
            ui.checkbox(&mut d.auto_numbering, "Apply auto-numbering to lists");
            ui.checkbox(&mut d.calculate_math, "Calculate mathematical expressions automatically");
            ui.checkbox(&mut d.create_table_on_tab, "Create a table when Tab is pressed after text");
            ui.checkbox(&mut d.wiki_links, "Enable wiki linking ([[Page]])");
            ui.checkbox(&mut d.show_paste_options, "Show Paste Options button when content is pasted");
            section(ui, "Passwords");
            // OPT.Advanced.LockAfterIdle, OPT.Advanced.LockOnLeave.
            ui.horizontal(|ui| {
                let mut idle = d.lock_after_minutes > 0;
                if ui.checkbox(&mut idle, "Lock password-protected sections after I have not worked in them for").changed() {
                    d.lock_after_minutes = if idle { 10 } else { 0 };
                }
                ui.add_enabled(idle, egui::DragValue::new(&mut d.lock_after_minutes).range(1..=1440).suffix(" min"));
            });
            ui.checkbox(&mut d.lock_on_leave, "Lock password-protected sections as soon as I navigate away from them");
            section(ui, "Default font");
            // OPT.Advanced.DefaultFont.
            egui::Grid::new("opt-font").num_columns(2).show(ui, |ui| {
                ui.label("Font:");
                egui::ComboBox::from_id_salt("opt-font-name").width(180.0).selected_text(d.default_font.clone()).show_ui(ui, |ui| {
                    for f in crate::ribbon::FONT_CHOICES {
                        ui.selectable_value(&mut d.default_font, f.to_string(), f);
                    }
                });
                ui.end_row();
                ui.label("Size:");
                ui.add(
                    egui::DragValue::new(&mut d.default_size)
                        .range(notecraft_engine::doc::text::MIN_FONT_SIZE..=notecraft_engine::doc::text::MAX_FONT_SIZE)
                        .speed(0.5),
                );
                ui.end_row();
                ui.label("Font color:");
                ui.horizontal(|ui| {
                    let mut auto = d.default_color.is_none();
                    if ui.checkbox(&mut auto, "Automatic").changed() {
                        d.default_color = if auto { None } else { Some(notecraft_engine::doc::color::Rgba::rgb(0, 0, 0)) };
                    }
                    if let Some(c) = &mut d.default_color {
                        let mut rgb = [c.r, c.g, c.b];
                        if ui.color_edit_button_srgb(&mut rgb).changed() {
                            *c = notecraft_engine::doc::color::Rgba::rgb(rgb[0], rgb[1], rgb[2]);
                        }
                    }
                });
                ui.end_row();
            });
        }
        OptionsPage::Proofing => {
            section(ui, "When correcting spelling");
            not_yet(ui, t, "Spelling checks arrive with the Review tab; NoteCraft doesn't check spelling yet.");
        }
        OptionsPage::SendToOneNote => {
            section(ui, "Send to NoteCraft");
            not_yet(ui, t, "Screen clippings go onto the current page. Email, web, print and meeting destinations need Microsoft Office.");
        }
        OptionsPage::AudioVideo => {
            section(ui, "Audio & Video");
            not_yet(ui, t, "Audio and video recording isn't available in NoteCraft yet.");
        }
        OptionsPage::Language => {
            // OPT.Language.DisplayLanguage: applies at once (unverified: the reference asks
            // for a restart).
            section(ui, "Display language");
            for l in crate::i18n::Lang::ALL {
                if ui.radio(app.ui.lang == l, l.name()).clicked() {
                    app.ui.lang = l;
                    crate::i18n::set(l);
                }
            }
            not_yet(ui, t, "The ribbon, menus and panes are translated; some dialog text is still in English.");
        }
        OptionsPage::CustomizeRibbon => {
            section(ui, "Customize the Ribbon");
            not_yet(ui, t, "The ribbon can't be customized yet. Switch between the simplified and classic ribbon with the ribbon's own button.");
        }
        OptionsPage::QuickAccessToolbar => {
            // OPT.QuickAccessToolbar.CustomizeQAT, UI.Dialogs.CustomizeQAT.
            section(ui, "Customize the Quick Access Toolbar");
            crate::qat::options_page(app, ui, t);
        }
        OptionsPage::AddIns => {
            section(ui, "Add-ins");
            not_yet(ui, t, "NoteCraft has no add-ins (COM add-ins are Windows-only).");
        }
        OptionsPage::TrustCenter => {
            section(ui, "Privacy");
            not_yet(ui, t, "NoteCraft keeps your notes on this computer and sends nothing anywhere: no telemetry, no network access.");
        }
    }
    ui.add_space(4.0);
}
