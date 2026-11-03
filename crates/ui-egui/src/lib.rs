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
pub(crate) mod clipboard;
pub mod context_menu;
pub mod dialogs;
pub mod fonts;
pub mod help_ui;
pub mod i18n;
mod i18n_it;
pub mod icons;
pub mod icons_set;
pub mod ink;
pub mod keymap;
mod keytips;
mod math_ui;
pub mod mini_toolbar;
pub mod nav;
pub mod options;
pub mod panes;
pub mod password_ui;
pub mod paste_options;
pub mod pictures;
pub mod preview;
pub mod qat;
pub mod ribbon;
pub mod ribbon_layout;
pub mod ribbon_simplified;
pub mod spell_ui;
pub mod theme;

use egui::{Frame, Margin, Rect, RichText, Ui};
use notecraft_engine::{Clock, CmdError, Session};
use serde_json::{Value, json};

pub use theme::{ThemeKind, Tokens};

/// What the system clipboard holds, as far as the host can read it.
#[derive(Default)]
pub struct RichClip {
    /// Plain text.
    pub text: Option<String>,
    /// HTML (from browsers and office applications).
    pub html: Option<String>,
    /// A picture (RGBA pixels), when one was asked for.
    pub image: Option<notecraft_codecs::Rgba8>,
    /// Files copied in a file manager (local paths, `text/uri-list`).
    pub files: Vec<std::path::PathBuf>,
}

/// Most files one drop or paste puts on a page.
pub const MAX_PASTED_FILES: usize = 50;

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
    /// Ask for a file to open; `filters` are `(name, extensions)` pairs (no dots). `None` when
    /// cancelled or unavailable.
    fn pick_file(&mut self, title: &str, filters: &[(&str, &[&str])]) -> Option<String> {
        let _ = (title, filters);
        None
    }
    /// Open a file with the system's handler for its type (an attachment's copy). `Err` with a
    /// reason when that isn't possible on this platform.
    fn open_path(&mut self, path: &str) -> Result<(), String> {
        let _ = path;
        Err("opening files isn't available here".into())
    }
    /// Recognise the text in a picture file's `bytes` (`lang`: a Tesseract language code).
    /// `Err` with a reason when that isn't possible here.
    fn ocr(&mut self, bytes: &[u8], lang: &str) -> Result<String, String> {
        let _ = (bytes, lang);
        Err("Recognising text in pictures isn't available here".into())
    }
    /// Start a new mail in the user's mail program with the file at `attach` attached. `Err`
    /// with a reason when that isn't possible here.
    fn send_email(&mut self, subject: &str, attach: &str) -> Result<(), String> {
        let _ = subject;
        Err(format!("Sending mail isn't available here; the file is at {attach}"))
    }
    /// Read the system clipboard: text and HTML, and the picture too when `image` (decoding a
    /// picture costs time, so only a paste that has no text, or Paste: Picture, asks for it). Empty when the
    /// host has no rich clipboard (the web build, tests).
    fn read_clipboard(&mut self, image: bool) -> RichClip {
        let _ = image;
        RichClip::default()
    }
    /// Put copied notes on the system clipboard: text with its HTML, or a picture. `false` when
    /// the host has no rich clipboard (the UI then copies the plain text through egui).
    fn write_clipboard(&mut self, text: &str, html: &str, image: Option<&notecraft_codecs::Rgba8>) -> bool {
        let _ = (text, html, image);
        false
    }
    /// Start a screen clipping: the desktop lets the user pick an area (the XDG Screenshot portal
    /// on Linux) while NoteCraft keeps running. `Err` with a reason when unavailable here.
    fn start_screen_clip(&mut self) -> Result<(), String> {
        Err("Screen clipping isn't available here".into())
    }
    /// The finished screen clipping, once: the picture file's bytes, or why there is none
    /// (cancelled, refused). Polled every frame.
    fn take_screen_clip(&mut self) -> Option<Result<Vec<u8>, String>> {
        None
    }
    /// The printers the system offers: (name, whether it is the default). Empty when printing
    /// isn't available (the web build, tests). Asked when File › Print opens, not every frame.
    fn printers(&mut self) -> Vec<(String, bool)> {
        Vec::new()
    }
    /// Send the print-ready PDF at `path` to `printer` (`None`: the default), `copies` times.
    /// Returns the spooler's message, or why it failed.
    fn print_pdf(&mut self, path: &str, printer: Option<&str>, copies: u32, title: &str) -> Result<String, String> {
        let _ = (path, printer, copies, title);
        Err("Printing isn't available here: export a PDF instead".into())
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
    /// Print and Print Preview.
    Print,
    /// File › Send (`BS.Send.*`).
    Send,
    /// File › Share (`BS.Share.*`): move the notebook, or keep it in a shared folder.
    Share,
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
    /// The window size before it became a Quick Note window (restored after).
    pub quick_note_restore: Option<[f32; 2]>,
    /// KeyTips: `None` off, `""` the tabs' letters, else the tab whose command letters show.
    #[serde(skip)]
    pub keytips: Option<String>,
    /// Alt is held on its own, and nothing was pressed since (KeyTips start on its release).
    #[serde(skip)]
    pub alt_alone: bool,
    /// Alt (alone) was down last frame.
    #[serde(skip)]
    pub alt_down: bool,
    /// The window's position and size before it was docked (restored after).
    #[serde(default)]
    pub dock_restore: Option<[f32; 4]>,
    /// Whether the window is sized as docked (Dock to Desktop), so it is sized once per change.
    #[serde(default)]
    pub dock_shown: bool,
    /// Whether the window is currently sized as a Quick Note window.
    pub quick_note_shown: bool,
    /// Whether the window was last told to stay on top (View › Always on Top).
    pub on_top_applied: bool,
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
    /// When a paste command last ran (seconds, `ctx` time): the Paste Options button
    /// (`UI.Canvas.PasteOptions`) shows near the caret for a few seconds after.
    pub paste_shown_at: Option<f64>,
    /// The style the pointer rests on in the Styles gallery: the selected paragraphs are drawn
    /// in it until the pointer leaves (`UI.Ribbon.Galleries` live preview). Set every frame.
    #[serde(skip)]
    pub style_preview: Option<notecraft_engine::doc::ParaStyle>,
    /// Notebook folders opened recently, most recent first (`BS.Open.RecentNotebooks`); the
    /// desktop app persists this (`apps/notecraft/src/settings.rs`), empty on the web build.
    pub recent_notebooks: Vec<String>,
    /// File › Export, step 1: what to export (`page`, `section` or `notebook`).
    #[serde(default)]
    pub export_scope: String,
    /// File › Export, step 2: the format, as its file extension (`mht`, `md`…).
    #[serde(default)]
    pub export_format: String,
    /// File › Print: what to print (`page` or `section`).
    #[serde(default)]
    pub print_scope: String,
    /// File › Print: the chosen printer (`None`: the system default).
    #[serde(default)]
    pub print_printer: Option<String>,
    /// File › Print: copies.
    #[serde(default)]
    pub print_copies: u32,
    /// The printers the host listed when File › Print opened (`None`: not asked yet).
    #[serde(skip)]
    pub printers: Option<Vec<(String, bool)>>,
    /// File › Options: the category shown.
    #[serde(default)]
    pub options_page: options::OptionsPage,
    /// File › Options: the settings being edited, applied with OK (`None`: not open).
    #[serde(default)]
    pub options_draft: Option<notecraft_engine::Prefs>,
    /// The display language (`OPT.Language.DisplayLanguage`; the desktop app remembers it).
    #[serde(default)]
    pub lang: i18n::Lang,
    /// The Quick Access Toolbar's buttons, as catalog ids (`qat`; the desktop app remembers it).
    #[serde(default)]
    pub qat: Vec<String>,
    /// Passwords being typed in the Password pane or the unlock prompt (never serialized).
    #[serde(skip)]
    pub password: password_ui::PasswordInput,
    /// When the user last gave any input (host clock, ms): the idle time that locks
    /// password-protected sections (`OPT.Advanced.LockAfterIdle`).
    #[serde(skip)]
    pub last_input_ms: i64,
}

/// Most paths kept in [`UiState::recent_notebooks`].
pub const MAX_RECENT_NOTEBOOKS: usize = 8;

impl Default for UiState {
    fn default() -> UiState {
        UiState {
            tab: "Home".into(),
            backstage: None,
            search: String::new(),
            search_open: false,
            search_pane: false,
            quick_note_restore: None,
            keytips: None,
            alt_alone: false,
            alt_down: false,
            dock_restore: None,
            dock_shown: false,
            quick_note_shown: false,
            on_top_applied: false,
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
            paste_shown_at: None,
            style_preview: None,
            recent_notebooks: Vec::new(),
            export_scope: "page".into(),
            export_format: "mht".into(),
            print_scope: "page".into(),
            print_printer: None,
            print_copies: 1,
            printers: None,
            options_page: options::OptionsPage::General,
            options_draft: None,
            password: password_ui::PasswordInput::default(),
            lang: i18n::Lang::En,
            qat: qat::DEFAULT.iter().map(|s| s.to_string()).collect(),
            last_input_ms: 0,
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
        let theme = self.resolved_theme();
        let mut t = Tokens::for_theme(theme);
        // View › Switch Background: a light page inside a dark window.
        if self.session.view.light_page && theme.is_dark() {
            let light = Tokens::for_theme(ThemeKind::White);
            t.page = light.page;
            t.page_text = light.page_text;
            t.page_meta = light.page_meta;
            t.container = light.container;
            t.table_border = light.table_border;
            t.selection = light.selection;
            t.link = light.link;
            t.rule = light.rule;
            t.margin_line = light.margin_line;
            t.find = light.find;
        }
        t
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
        // BS.Open.RecentNotebooks: captured before `params` moves into `session.run`, remembered
        // after it succeeds.
        let opened_path = matches!(id, "file.openNotebook" | "file.newNotebook" | "file.openRecent" | "notebook.changeLocation")
            .then(|| params.get("path").and_then(Value::as_str).map(str::to_string))
            .flatten();
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
        if v.get("keytips").and_then(Value::as_bool) == Some(true) {
            self.ui.keytips = if self.ui.keytips.is_some() { None } else { Some(String::new()) };
        }
        if id == "notebook.syncStatus" {
            self.ui.dialog = Some(crate::dialogs::Dialog::SyncStatus);
        }
        if v.get("options").and_then(Value::as_str) == Some("language") {
            self.ui.options_page = crate::options::OptionsPage::Language;
            self.ui.backstage = Some(crate::Backstage::Options);
        }
        if let Some(path) = v.get("email_attach").and_then(Value::as_str) {
            let subject = v.get("subject").and_then(Value::as_str).unwrap_or_default();
            if let Err(e) = self.host.send_email(subject, path) {
                self.ui.status = Some((e, ctx.input(|i| i.time)));
            }
        } else if id == "share.sendToWord"
            && let Some(path) = v.get("path").and_then(Value::as_str)
            && let Err(e) = self.host.open_path(path)
        {
            self.ui.status = Some((e, ctx.input(|i| i.time)));
        }
        if matches!(id, "edit.copy" | "edit.cut") {
            clipboard::after_copy(self, ctx, &v);
        } else if matches!(
            id,
            "page.copyLink"
                | "section.copyLink"
                | "notebook.copyLink"
                | "link.copy"
                | "attachment.copyLink"
                | "object.copyTextFromPicture"
                | "printout.copyPageText"
                | "printout.copyAllText"
                | "para.copyLink"
                | "sectionGroup.copyLink"
        ) && let Some(t) = v.get("text").or_else(|| v.get("link")).and_then(Value::as_str)
        {
            ctx.copy_text(t.to_string());
        }
        // UI.Canvas.PasteOptions: the button shows near the caret for a few seconds after any
        // paste, letting the choice be changed after the fact.
        // OPT.Advanced.ShowPasteOptions turns the button off.
        if self.session.prefs.show_paste_options
            && matches!(id, "edit.paste" | "edit.pasteKeepSource" | "edit.pasteMerge" | "edit.pasteTextOnly" | "edit.pasteOptions")
        {
            self.ui.paste_shown_at = Some(ctx.input(|i| i.time));
        }
        if let Some(path) = opened_path {
            self.ui.recent_notebooks.retain(|p| p != &path);
            self.ui.recent_notebooks.insert(0, path);
            self.ui.recent_notebooks.truncate(MAX_RECENT_NOTEBOOKS);
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
    /// the window size in points, whether the Paste Options button is showing, the contextual
    /// tabs shown, and where the last frame put the tables (cells, column borders) and the
    /// paragraph handles, in screen points, so a driver can click and drag them.
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
            "pasteShown": self.ui.paste_shown_at.is_some(),
            "contextualTabs": chrome::contextual_tabs(self).iter().map(|t| t.name).collect::<Vec<_>>(),
            "tables": self.canvas.tables.iter().take(50).map(|tl| json!({
                "id": tl.id,
                "rect": rect_json(tl.rect),
                "colEdges": tl.col_edges,
                "cells": tl.cells.iter().take(500).map(|c| json!({"id": c.id, "row": c.row, "col": c.col, "rect": rect_json(c.rect)})).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "handles": self.canvas.paras.iter().filter(|p| matches!(p.block, notecraft_engine::selection::Block::Outline(_))).take(500).map(|p| json!({
                "block": match p.block { notecraft_engine::selection::Block::Outline(id) => id.to_string(), _ => String::new() },
                "para": p.para,
                "x": p.handle.center().x,
                "y": p.handle.center().y,
                "text": rect_json(Rect::from_min_size(p.pos, p.galley.size())),
            })).collect::<Vec<_>>(),
            "media": self.canvas.media.iter().take(500).map(|m| json!({
                "id": m.id, "picture": m.picture, "background": m.background, "above": m.above, "rect": rect_json(m.rect),
            })).collect::<Vec<_>>(),
            "textures": self.canvas.textures.stats().0,
            "ink": crate::ink::inspect(self),
        })
    }

    /// Per-frame logic outside drawing: clock, autosave, notices.
    pub fn logic(&mut self, ctx: &egui::Context) {
        self.session.clock = self.host.clock();
        if std::mem::take(&mut self.session.quit_request) {
            self.quit_requested = true;
        }
        // OPT.Advanced.LockAfterIdle / LockOnLeave: idle counts from the last input.
        let now = self.session.clock.now_ms;
        if self.ui.last_input_ms == 0 || ctx.input(|i| !i.events.is_empty() || i.pointer.is_moving()) {
            self.ui.last_input_ms = now;
        }
        if self.session.auto_lock(now.saturating_sub(self.ui.last_input_ms)) {
            self.ui.password = password_ui::PasswordInput::default();
        }
        if self.session.autosave_due(1000) {
            let errors = self.session.save_dirty();
            if let Some(e) = errors.first() {
                self.ui.status = Some((e.clone(), ctx.input(|i| i.time)));
            }
        }
        // OPT.SaveBackup.AutoBackup: the changed notebooks, into the Options' folder or the
        // app's own. A failure is shown once and retried at the next interval. The first
        // interval counts from the first run, so a fresh install doesn't back up at once.
        if self.session.prefs.last_backup_ms == 0 {
            self.session.prefs.last_backup_ms = self.session.clock.now_ms;
        }
        if self.session.backup_due() {
            let folder = Some(self.session.prefs.backup_folder.trim().to_string()).filter(|f| !f.is_empty()).or_else(|| self.host.backup_folder());
            match folder {
                Some(f) => {
                    if let Err(e) = self.session.run("file.backupChanged", json!({"folder": f})) {
                        self.ui.status = Some((format!("Automatic backup failed: {e}"), ctx.input(|i| i.time)));
                        self.session.prefs.last_backup_ms = self.session.clock.now_ms;
                    }
                }
                None => self.session.prefs.last_backup_ms = self.session.clock.now_ms,
            }
        }
        match self.host.take_screen_clip() {
            Some(Ok(bytes)) => {
                self.run(ctx, "insert.screenClipping", json!({"data": notecraft_engine::b64::encode(&bytes)}));
            }
            Some(Err(e)) => self.ui.status = Some((e, ctx.input(|i| i.time))),
            None => {}
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
        i18n::set(self.ui.lang);
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
        keytips::handle(self, ui);
        keymap::handle_input(self, &ctx);
        if self.ui.backstage.is_some() {
            backstage::show(self, ui, t);
            dialogs::show(self, &ctx, t);
            help_ui::about(self, &ctx, t);
            return;
        }
        chrome::title_bar(self, ui, t);
        self.quick_note_window(&ctx);
        if !self.session.view.full_page && !self.session.view.quick_note {
            if !self.session.view.ribbon_hidden {
                chrome::tab_row(self, ui, t);
                ribbon::show(self, ui, t);
                keytips::strip(self, ui, t);
            }
            // Docked, the window shows the page: no notebook pane or page list.
            if !self.session.view.docked {
                nav::nav_row(self, ui, t);
                nav::notebook_pane(self, ui, t);
                nav::page_list(self, ui, t);
            }
            if self.session.view.tags_pane {
                panes::tags_pane(self, ui, t);
            }
            if self.session.view.templates_pane {
                panes::templates_pane(self, ui, t);
            }
            if self.session.view.paper_pane {
                panes::paper_pane(self, ui, t);
            }
            if self.session.view.password_pane {
                password_ui::pane(self, ui, t);
            }
            if self.session.view.spelling_pane {
                spell_ui::pane(self, ui, t);
            }
            if self.session.view.thesaurus.is_some() {
                panes::thesaurus_pane(self, ui, t);
            }
            if self.session.view.accessibility_pane {
                panes::accessibility_pane(self, ui, t);
            }
            if self.session.view.recent_edits.is_some() {
                panes::recent_pane(self, ui, t);
            }
            if self.session.view.help_pane {
                help_ui::pane(self, ui, t);
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
            // A locked section shows its unlock prompt instead of a page (BEH.Password.Lock).
            if self.session.view.preview.is_some() {
                // A version or a deleted page, read-only (decision D14).
                preview::show(self, ui, t);
            } else if !password_ui::locked_view(self, ui, t) {
                canvas::show(self, ui, t);
            }
            ink::replay_bar(self, ui, area, t);
            mini_toolbar::show(self, ui, area, t);
            paste_options::show(self, ui, area, t);
            if self.session.view.find.active {
                panes::find_bar(self, ui, area, t);
            }
            self.expand_button(ui, area, t);
        });
        self.toast(&ctx, t);
        nav::search_dropdown(self, &ctx, t);
        dialogs::show(self, &ctx, t);
        help_ui::about(self, &ctx, t);
    }

    /// The expand / restore button at the page's top right (catalog `UI.Canvas.ExpandButton`,
    /// `UI.Window.FullPage`): Full Page View hides the ribbon and the navigation.
    /// Size the window for a Quick Note and back (View › New Quick Note), once per change.
    fn quick_note_window(&mut self, ctx: &egui::Context) {
        // View › Always on Top, once per change.
        if self.session.view.always_on_top != self.ui.on_top_applied {
            self.ui.on_top_applied = self.session.view.always_on_top;
            let level = if self.ui.on_top_applied { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal };
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
        }
        self.dock_window(ctx);
        let want = self.session.view.quick_note;
        if want == self.ui.quick_note_shown {
            return;
        }
        if want {
            // The window's size, else what egui draws in (a window that never reported its rect).
            let r = ctx.input(|i| i.viewport().inner_rect).unwrap_or_else(|| ctx.content_rect());
            self.ui.quick_note_restore = Some([r.width(), r.height()]);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(460.0, 400.0)));
        } else if let Some(size) = self.ui.quick_note_restore.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(size[0], size[1])));
        }
        self.ui.quick_note_shown = want;
    }

    /// Dock to Desktop and back, once per change: a 380-point-wide window along the screen's
    /// right edge, above the others (Wayland lets the compositor place it), then the old
    /// position, size and level again.
    fn dock_window(&mut self, ctx: &egui::Context) {
        let want = self.session.view.docked;
        if want == self.ui.dock_shown {
            return;
        }
        self.ui.dock_shown = want;
        if want {
            let (outer, inner, monitor) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().inner_rect, i.viewport().monitor_size));
            let r = inner.unwrap_or_else(|| ctx.content_rect());
            let at = outer.map_or(egui::Pos2::ZERO, |o| o.min);
            self.ui.dock_restore = Some([at.x, at.y, r.width(), r.height()]);
            let screen = monitor.unwrap_or(egui::vec2(1280.0, 800.0));
            const W: f32 = 380.0;
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(W, (screen.y - 40.0).max(300.0))));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2((screen.x - W).max(0.0), 0.0)));
        } else {
            let level = if self.session.view.always_on_top { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal };
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
            if let Some([x, y, w, h]) = self.ui.dock_restore.take() {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
            }
        }
    }

    fn expand_button(&mut self, ui: &mut Ui, area: egui::Rect, t: Tokens) {
        let ctx = ui.ctx().clone();
        if self.session.view.quick_note {
            // The Quick Note window opens the full window again.
            let r = egui::Rect::from_min_size(egui::pos2(area.max.x - 40.0, area.min.y + 8.0), egui::vec2(28.0, 28.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| {
                let (resp, rect, color) = chrome::icon_button(ui, 28.0, "Open in the full window", true, &t);
                icons::expand(ui.painter(), rect.shrink(6.0), false, color);
                if resp.on_hover_text("Open in the full window").clicked() {
                    self.run(&ctx, "view.normal", json!({}));
                }
            });
            return;
        }
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
            // KB.Format.OpenLink: the command also takes an explicit `url` (a click on a link,
            // agents), so its `enabled` predicate can't ask for a link; a menu row without
            // one has nothing to open.
            Some(c) if c.id == "link.open" && self.session.current_link().is_none() => Some("Put the caret on a link first".to_string()),
            Some(c) => (c.enabled)(&self.session).map(str::to_string),
        }
    }
}

pub(crate) fn rect_json(r: Rect) -> Value {
    json!({"x": r.min.x, "y": r.min.y, "w": r.width(), "h": r.height()})
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_canvas;
#[cfg(test)]
mod tests_ink;
#[cfg(test)]
mod tests_pictures;
