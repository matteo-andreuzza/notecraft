//! NoteCraft's egui front end (layer **L6**): the OneNote-style window in the 2024 look
//! (decision D1, `docs/ui-parity.md`) — title bar with the Quick Access Toolbar and Search, the
//! tab row and the ribbon card, the navigation (notebook tree and page column) and the page
//! canvas — plus the backstage, panes and dialogs.
//!
//! The UI is thin: every action is an engine command run through [`NoteApp::run`] (AGENTS.md ›
//! Everything is a command). It keeps only view state (which tab is open, what is being renamed,
//! layout caches). It sits at L6 because it is the only layer that knows about egui; nothing below
//! it does.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod backstage;
pub mod canvas;
pub mod chrome;
pub mod dialogs;
pub mod fonts;
pub mod icons;
pub mod icons_set;
pub mod keymap;
pub mod nav;
pub mod panes;
pub mod ribbon;
pub mod ribbon_layout;
pub mod ribbon_simplified;
pub mod theme;

use egui::{Frame, Margin, RichText, Ui};
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
    /// Whether the desktop prefers a dark theme (`None` = it says nothing; egui's own answer is
    /// used then). Hosts that can ask the desktop (the XDG portal on Linux) override this.
    fn prefers_dark(&mut self) -> Option<bool> {
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
    /// Notebooks and section groups collapsed in the navigation tree (the rest are expanded).
    pub nav_collapsed: Vec<notecraft_engine::doc::Id>,
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
            notebook_pane_width: theme::size::NAV_WIDTH,
            nav_collapsed: Vec::new(),
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
    /// The desktop's light/dark preference (`None` = unknown), for "Use system setting": the
    /// host's answer, else the one egui reports.
    pub system_dark: Option<bool>,
    fonts_installed: bool,
    /// The resolved theme last applied to egui's style (re-applied only when it changes).
    applied_theme: Option<ThemeKind>,
}

impl NoteApp {
    /// A new app around a session.
    pub fn new(session: Session, host: Box<dyn Host>) -> NoteApp {
        NoteApp {
            session,
            ui: UiState::default(),
            canvas: canvas::CanvasState::default(),
            host,
            quit_requested: false,
            system_dark: None,
            fonts_installed: false,
            applied_theme: None,
        }
    }

    /// The theme being drawn: the chosen one, with "Use system setting" resolved against the
    /// desktop's preference.
    pub fn resolved_theme(&self) -> ThemeKind {
        self.ui.theme.resolve(self.system_dark)
    }

    /// Theme tokens for the current theme.
    pub fn tokens(&self) -> Tokens {
        Tokens::for_theme(self.resolved_theme())
    }

    /// Run a command; errors become the status message. Results that need the platform (open
    /// a URL, copy text) are handled here.
    pub fn run(&mut self, ctx: &egui::Context, id: &str, params: Value) -> Option<Value> {
        match self.try_run(ctx, id, params) {
            Ok(v) => Some(v),
            Err(msg) => {
                log::info!("{id}: {msg}");
                self.ui.status = Some((msg, ctx.input(|i| i.time)));
                None
            }
        }
    }

    /// Run a command as [`NoteApp::run`] does (platform results handled), but hand the error
    /// back instead of showing it: the control channel answers with it.
    pub fn try_run(&mut self, ctx: &egui::Context, id: &str, params: Value) -> Result<Value, String> {
        self.session.clock = self.host.clock();
        let sel_before = self.session.sel.clone();
        let v = self.session.run(id, params).map_err(|e| match e {
            CmdError::Disabled(m) => m,
            other => other.to_string(),
        })?;
        // A command that puts the caret on the page (New Page, Ctrl+Shift+T, Undo…) gives the
        // page the keyboard, as OneNote does: typing goes on there. A command that leaves the
        // selection alone (a ribbon button) or moves focus to a pane (a page tab) doesn't.
        if self.session.sel != sel_before
            && self.session.sel.focus == notecraft_engine::selection::FocusArea::Canvas
            && self.session.text_range().is_some()
        {
            self.canvas.focused = true;
        }
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
        Ok(v)
    }

    /// The view state for the control channel (`ui.inspect`): the ribbon tab and layout, backstage
    /// page, open dialog, theme, panes, whether the navigation shows and the last status message,
    /// plus the window size in points.
    pub fn inspect_ui(&self, ctx: &egui::Context) -> Value {
        let size = ctx.content_rect().size();
        json!({
            "tab": self.ui.tab,
            "backstage": serde_json::to_value(self.ui.backstage).unwrap_or(Value::Null),
            "dialog": serde_json::to_value(&self.ui.dialog).unwrap_or(Value::Null),
            "theme": serde_json::to_value(self.ui.theme).unwrap_or(Value::Null),
            "resolvedTheme": serde_json::to_value(self.resolved_theme()).unwrap_or(Value::Null),
            "rename": serde_json::to_value(&self.ui.rename).unwrap_or(Value::Null),
            "search": self.ui.search,
            "searchPane": self.ui.search_pane,
            "tagsPane": self.session.view.tags_pane,
            "fullPage": self.session.view.full_page,
            "ribbonMode": if self.session.view.ribbon_mode == notecraft_engine::RibbonMode::Simplified { "simplified" } else { "classic" },
            "ribbonCollapsed": self.session.view.ribbon_collapsed,
            "navigation": !self.session.view.notebook_pane_collapsed,
            "status": self.ui.status.as_ref().map(|(m, _)| m.clone()),
            "canvasFocused": self.canvas.focused,
            "window": {"width": size.x, "height": size.y},
            "quitRequested": self.quit_requested,
        })
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
        self.system_dark = self.host.prefers_dark().or_else(|| ctx.system_theme().map(|t| t == egui::Theme::Dark));
        let t = self.tokens();
        if self.applied_theme != Some(self.resolved_theme()) {
            theme::apply(&ctx, &t);
            self.applied_theme = Some(self.resolved_theme());
        }
        keymap::handle_input(self, &ctx);
        if self.ui.backstage.is_some() {
            backstage::show(self, ui, t);
            dialogs::show(self, &ctx, t);
            return;
        }
        chrome::title_bar(self, ui, t);
        if !self.session.view.full_page {
            chrome::tab_row(self, ui, t);
            ribbon::show(self, ui, t);
            nav::nav_row(self, ui, t);
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
            let area = ui.max_rect();
            if self.session.notebook().is_none() {
                nav::empty_state(self, ui, t);
            }
            canvas::show(self, ui, t);
            if self.session.view.find.active {
                panes::find_bar(self, ui, area, t);
            }
            self.expand_button(ui, area, t);
        });
        self.toast(&ctx, t);
        nav::search_dropdown(self, &ctx, t);
        dialogs::show(self, &ctx, t);
    }

    /// The expand / restore button at the page's top right (catalog `UI.Canvas.ExpandButton`,
    /// `UI.Window.FullPage`): Full Page View hides the ribbon and the navigation.
    fn expand_button(&mut self, ui: &mut Ui, area: egui::Rect, t: Tokens) {
        let ctx = ui.ctx().clone();
        let full = self.session.view.full_page;
        let label = if full { "Normal View" } else { "Full Page View" };
        let r = egui::Rect::from_min_size(egui::pos2(area.max.x - 40.0, area.min.y + 8.0), egui::vec2(28.0, 28.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| {
            let (resp, rect, color) = chrome::icon_button(ui, 28.0, label, true, &t);
            icons::expand(ui.painter(), rect.shrink(6.0), full, color);
            if resp.on_hover_text(format!("{label} (F11)")).clicked() {
                self.run(&ctx, "view.fullPage", json!({}));
            }
        });
    }

    /// The last notice or error as a toast at the bottom centre for a few seconds (the 2024 look
    /// has no status bar).
    fn toast(&mut self, ctx: &egui::Context, t: Tokens) {
        let now = ctx.input(|i| i.time);
        let Some((msg, at)) = self.ui.status.clone() else { return };
        if now - at > 6.0 {
            self.ui.status = None;
            return;
        }
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::new("toast"))
            .order(egui::Order::Tooltip)
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -24.0))
            .interactable(false)
            .show(ctx, |ui| {
                Frame::NONE.fill(t.text).corner_radius(theme::radius::CARD).shadow(t.card_shadow()).inner_margin(Margin::symmetric(16, 10)).show(
                    ui,
                    |ui| {
                        ui.set_max_width((screen.width() * 0.6).max(200.0));
                        ui.label(RichText::new(msg).color(t.ribbon_card));
                    },
                );
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
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
