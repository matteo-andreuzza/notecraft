//! NoteCraft's egui front end (layer **L6**): the OneNote-style window — title row with the
//! Quick Access Toolbar, the ribbon, the notebook pane, section tabs, the page list and the page
//! canvas — plus the backstage, panes and dialogs.
//!
//! The UI is thin: every action is an engine command run through [`NoteApp::run`] (AGENTS.md ›
//! Everything is a command). It keeps only view state (which tab is open, what is being renamed,
//! layout caches). It sits at L6 because it is the only layer that knows about egui; nothing below
//! it does.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod backstage;
pub mod canvas;
pub mod dialogs;
pub mod fonts;
pub mod icons;
pub mod keymap;
pub mod nav;
pub mod panes;
pub mod ribbon;
pub mod ribbon_layout;
pub mod theme;

use egui::{Color32, Frame, Margin, RichText, Stroke, Ui, vec2};
use notecraft_engine::{Clock, CmdError, Session};
use serde_json::{Value, json};

pub use theme::{ThemeKind, Tokens};

/// What the host platform provides (desktop app, web app, tests).
pub trait Host {
    /// The current time and local UTC offset.
    fn clock(&self) -> Clock;
    /// Ask the user for a folder (notebook location). `None` when cancelled or unavailable.
    fn pick_folder(&mut self, title: &str) -> Option<String> {
        let _ = title;
        None
    }
    /// Ask for a file to save to.
    fn save_file(&mut self, title: &str, default_name: &str) -> Option<String> {
        let _ = (title, default_name);
        None
    }
    /// Where backups go (`None` = backups unavailable on this platform).
    fn backup_folder(&self) -> Option<String> {
        None
    }
}

/// A host for tests and screenshots: fixed clock, no dialogs.
pub struct FixedHost(pub Clock);

impl Host for FixedHost {
    fn clock(&self) -> Clock {
        self.0
    }
}

/// Which backstage page is open (File tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Backstage {
    /// Info: notebook information.
    #[default]
    Info,
    /// New notebook.
    New,
    /// Open notebook.
    Open,
    /// Export.
    Export,
    /// Account (theme, about).
    Account,
    /// Options.
    Options,
}

/// Something being renamed in place.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Rename {
    /// A section tab.
    Section(notecraft_engine::doc::Id, String),
    /// A section group.
    Group(notecraft_engine::doc::Id, String),
    /// A notebook.
    Notebook(notecraft_engine::doc::Id, String),
}

/// View state (serializable so the control channel and screenshots can drive it).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UiState {
    /// The selected ribbon tab.
    pub tab: String,
    /// The backstage page, when File is open.
    pub backstage: Option<Backstage>,
    /// Search box text.
    pub search: String,
    /// Whether the search results drop-down is open.
    pub search_open: bool,
    /// The search results pane is docked.
    pub search_pane: bool,
    /// The find-on-page box text.
    pub find: String,
    /// In-place rename.
    pub rename: Option<Rename>,
    /// The open dialog.
    pub dialog: Option<dialogs::Dialog>,
    /// Theme.
    pub theme: ThemeKind,
    /// Last status message (errors from commands, notices) and when it was shown (seconds).
    pub status: Option<(String, f64)>,
    /// Notebook pane width (points).
    pub notebook_pane_width: f32,
    /// Find Tags pane grouping.
    pub tags_group: String,
    /// Find Tags pane: show only unchecked items.
    pub tags_unchecked: bool,
    /// Find Tags pane scope.
    pub tags_scope: String,
}

impl Default for UiState {
    fn default() -> UiState {
        UiState {
            tab: "Home".into(),
            backstage: None,
            search: String::new(),
            search_open: false,
            search_pane: false,
            find: String::new(),
            rename: None,
            dialog: None,
            theme: ThemeKind::Colorful,
            status: None,
            notebook_pane_width: 210.0,
            tags_group: "tag".into(),
            tags_unchecked: false,
            tags_scope: "all".into(),
        }
    }
}

/// The application.
pub struct NoteApp {
    /// The engine session.
    pub session: Session,
    /// View state.
    pub ui: UiState,
    /// Canvas state (layout cache, drags, caret blink).
    pub canvas: canvas::CanvasState,
    /// Platform services.
    pub host: Box<dyn Host>,
    /// Whether the app asked to quit (File › Exit).
    pub quit_requested: bool,
    fonts_installed: bool,
}

impl NoteApp {
    /// A new app around a session.
    pub fn new(session: Session, host: Box<dyn Host>) -> NoteApp {
        NoteApp { session, ui: UiState::default(), canvas: canvas::CanvasState::default(), host, quit_requested: false, fonts_installed: false }
    }

    /// Theme tokens for the current theme.
    pub fn tokens(&self) -> Tokens {
        Tokens::for_theme(self.ui.theme)
    }

    /// Run a command; errors become the status message. Results that need the platform (open
    /// a URL, copy text) are handled here.
    pub fn run(&mut self, ctx: &egui::Context, id: &str, params: Value) -> Option<Value> {
        self.session.clock = self.host.clock();
        match self.session.run(id, params) {
            Ok(v) => {
                if let Some(url) = v.get("open_url").and_then(Value::as_str) {
                    ctx.open_url(egui::OpenUrl::new_tab(url));
                }
                if matches!(
                    id,
                    "edit.copy"
                        | "edit.cut"
                        | "page.copyLink"
                        | "section.copyLink"
                        | "notebook.copyLink"
                        | "link.copy"
                        | "para.copyLink"
                        | "sectionGroup.copyLink"
                ) && let Some(t) = v.get("text").or_else(|| v.get("link")).and_then(Value::as_str)
                {
                    ctx.copy_text(t.to_string());
                }
                if v.get("rename").and_then(Value::as_bool) == Some(true) {
                    if let Some(sid) = v.get("section").and_then(Value::as_str).and_then(notecraft_engine::doc::Id::parse) {
                        let name = self.session.ws.section(sid).map(|s| s.name.clone()).unwrap_or_default();
                        self.ui.rename = Some(Rename::Section(sid, name));
                    } else if let Some(gid) = v.get("group").and_then(Value::as_str).and_then(notecraft_engine::doc::Id::parse) {
                        let name = self.session.notebook().and_then(|n| n.group(gid)).map(|g| g.name.clone()).unwrap_or_default();
                        self.ui.rename = Some(Rename::Group(gid, name));
                    }
                }
                self.canvas.caret_moved(ctx);
                Some(v)
            }
            Err(e) => {
                let msg = match &e {
                    CmdError::Disabled(m) => m.clone(),
                    other => other.to_string(),
                };
                log::info!("{id}: {msg}");
                self.ui.status = Some((msg, ctx.input(|i| i.time)));
                None
            }
        }
    }

    /// Per-frame logic outside drawing: clock, autosave, notices.
    pub fn logic(&mut self, ctx: &egui::Context) {
        self.session.clock = self.host.clock();
        if self.session.autosave_due(1000) {
            let errors = self.session.save_dirty();
            if let Some(e) = errors.first() {
                self.ui.status = Some((e.clone(), ctx.input(|i| i.time)));
            }
        }
        if let Some(m) = self.session.messages.pop() {
            self.ui.status = Some((m, ctx.input(|i| i.time)));
        }
        if self.session.is_dirty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(1100));
        }
    }

    /// Draw the whole window.
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if !self.fonts_installed {
            fonts::install(&ctx);
            self.fonts_installed = true;
        }
        let t = self.tokens();
        ctx.set_visuals(t.visuals(self.ui.theme));
        keymap::handle_input(self, &ctx);
        if self.ui.backstage.is_some() {
            backstage::show(self, ui, t);
            dialogs::show(self, &ctx, t);
            return;
        }
        if !self.session.view.full_page {
            self.title_row(ui, t);
            ribbon::show(self, ui, t);
        } else {
            self.full_page_bar(ui, t);
        }
        self.status_bar(ui, t);
        if !self.session.view.full_page {
            nav::notebook_pane(self, ui, t);
            nav::page_list(self, ui, t);
            if self.session.view.tags_pane {
                panes::tags_pane(self, ui, t);
            }
            if self.ui.search_pane {
                panes::search_pane(self, ui, t);
            }
        }
        egui::CentralPanel::no_frame().show(ui, |ui| {
            if !self.session.view.full_page {
                nav::section_tabs(self, ui, t);
            }
            if self.session.view.find.active {
                panes::find_bar(self, ui, t);
            }
            canvas::show(self, ui, t);
        });
        nav::search_dropdown(self, &ctx, t);
        dialogs::show(self, &ctx, t);
    }

    /// The title row: Quick Access Toolbar on the left, the window title in the middle
    /// (catalog `UI.Window.TitleBar`, `UI.Window.QAT`).
    fn title_row(&mut self, ui: &mut Ui, t: Tokens) {
        let ctx = ui.ctx().clone();
        egui::Panel::top("title_row").exact_size(30.0).frame(Frame::NONE.fill(t.chrome).inner_margin(Margin::symmetric(6, 0))).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                // UI.Window.QAT — Back, Undo, Redo by default (the reference shows Back and Undo).
                for (id, catalog, tip) in [
                    ("nav.back", "QAT.ViewGoBack", "Back (Alt+Left)"),
                    ("edit.undo", "QAT.UndoButton", "Undo (Ctrl+Z)"),
                    ("edit.redo", "QAT.RedoButton", "Redo (Ctrl+Y)"),
                ] {
                    let enabled = notecraft_engine::commands::find(id).is_some_and(|c| (c.enabled)(&self.session).is_none());
                    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), egui::Sense::click());
                    if resp.hovered() && enabled {
                        ui.painter().rect_filled(rect, 2.0, t.tab_hover);
                    }
                    let color = if enabled { t.chrome_text } else { t.chrome_text.gamma_multiply(0.45) };
                    let icon_rect = rect.shrink(5.0);
                    match id {
                        "nav.back" => {
                            ui.painter().arrow(icon_rect.right_center(), vec2(-icon_rect.width(), 0.0), Stroke::new(1.6, color));
                        }
                        "edit.undo" | "edit.redo" => {
                            let flip = if id == "edit.redo" { -1.0 } else { 1.0 };
                            let c = icon_rect.center();
                            let pts: Vec<egui::Pos2> = (0..=10)
                                .map(|i| {
                                    let a = std::f32::consts::PI * (1.1 - 1.2 * i as f32 / 10.0);
                                    c + vec2(flip * a.cos(), -a.sin()) * icon_rect.width() * 0.42
                                })
                                .collect();
                            if let Some(first) = pts.first().copied() {
                                ui.painter().add(egui::Shape::line(pts, Stroke::new(1.6, color)));
                                ui.painter().line_segment([first, first + vec2(flip * 4.0, -3.0)], Stroke::new(1.6, color));
                            }
                        }
                        _ => {}
                    }
                    let resp = resp.on_hover_text(format!("{tip}\n{catalog}"));
                    if resp.clicked() && enabled {
                        self.run(&ctx, id, json!({}));
                    }
                }
                let title = match (self.session.page(), self.session.notebook()) {
                    (Some(p), Some(nb)) => format!("{} - {} - NoteCraft", p.display_title(), nb.name),
                    (None, Some(nb)) => format!("{} - NoteCraft", nb.name),
                    _ => "NoteCraft".to_string(),
                };
                ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight), |ui| {
                    ui.label(RichText::new(title).color(t.chrome_text).size(12.5));
                });
            });
        });
    }

    fn full_page_bar(&mut self, ui: &mut Ui, t: Tokens) {
        let ctx = ui.ctx().clone();
        egui::Panel::top("full_page_bar").exact_size(26.0).frame(Frame::NONE.fill(t.chrome).inner_margin(Margin::symmetric(8, 2))).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                // UI.Window.FullPage — the single button back to Normal View.
                if ui
                    .add(egui::Button::new(RichText::new("Normal View").color(t.chrome_text)).fill(Color32::TRANSPARENT))
                    .on_hover_text("Leave Full Page View (F11)")
                    .clicked()
                {
                    self.run(&ctx, "view.fullPage", json!({}));
                }
            });
        });
    }

    fn status_bar(&mut self, ui: &mut Ui, t: Tokens) {
        let now = ui.ctx().input(|i| i.time);
        let Some((msg, at)) = self.ui.status.clone() else { return };
        if now - at > 6.0 {
            self.ui.status = None;
            return;
        }
        egui::Panel::bottom("status")
            .exact_size(22.0)
            .frame(Frame::NONE.fill(t.ribbon).inner_margin(Margin::symmetric(10, 2)).stroke(Stroke::new(1.0, t.border)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new(msg).color(t.ribbon_text).size(12.0));
                });
            });
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    }

    /// Whether a command is enabled right now (`None`) or why not.
    pub fn disabled_reason(&self, id: &str) -> Option<String> {
        match notecraft_engine::commands::find(id) {
            None => Some("Not available yet in NoteCraft".to_string()),
            Some(c) => (c.enabled)(&self.session).map(str::to_string),
        }
    }
}

#[cfg(test)]
mod tests;
