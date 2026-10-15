//! The desktop app's preferences: `settings.json` in the settings folder (next to the control
//! key files; `~/.config/notecraft` on Linux).
//!
//! Loading never fails: a missing file means defaults, keys the file lacks take their defaults,
//! and a damaged file is moved aside (`settings.json.broken`) and logged. Saves are atomic
//! (`craft-atomic`), at most once a second while something changed, and on exit.
//! `NOTECRAFT_NO_PREFS` (agents' and tests' runs) keeps them in memory: nothing is read or
//! written.
//!
//! (The suite's `craft-settings` would do this in TOML, but it pulls in an MPL-2.0 crate, and
//! NoteCraft links permissive licences only.)

use std::path::{Path, PathBuf};

use notecraft_engine::doc::Id;
use notecraft_ui_egui::{NoteApp, ThemeKind};
use serde::{Deserialize, Serialize};

/// Settings files larger than this are treated as damaged (a hostile or runaway file).
const MAX_BYTES: u64 = 4 << 20;
/// The least time between two saves while settings keep changing, in milliseconds.
const SAVE_EVERY_MS: i64 = 1000;

/// Everything the desktop app remembers between runs.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopSettings {
    /// Theme (File › Options › General).
    pub appearance: Appearance,
    /// The engine's preferences (author, default font, auto-lists, custom tags, search scope…).
    pub editing: notecraft_engine::Prefs,
    /// The notebooks to reopen.
    pub notebooks: Notebooks,
    /// Pane sizes.
    pub layout: Layout,
}

/// Look.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Colorful, White or Dark.
    pub theme: ThemeKind,
}

/// Open notebooks, as OneNote reopens them.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Notebooks {
    /// Notebook folders, in the notebook pane's order.
    pub open: Vec<String>,
    /// The page that was open (id), reopened when it still exists.
    pub last_page: String,
}

/// Pane sizes in points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    /// Notebook pane width.
    pub notebook_pane_width: f32,
}

impl Default for Layout {
    fn default() -> Layout {
        Layout { notebook_pane_width: notecraft_ui_egui::UiState::default().notebook_pane_width }
    }
}

/// Whether preferences are read and written (`NOTECRAFT_NO_PREFS` unset).
pub fn enabled() -> bool {
    std::env::var_os("NOTECRAFT_NO_PREFS").is_none()
}

/// The settings, and where and when they are written.
pub struct Store {
    path: Option<PathBuf>,
    value: DesktopSettings,
    saved: DesktopSettings,
    last_save_ms: i64,
}

impl Store {
    /// Load from the platform's settings folder, or keep them in memory without preferences.
    pub fn open() -> Store {
        let path = if enabled() { notecraft_automation::keyfile::settings_dir().map(|d| d.join("settings.json")) } else { None };
        match path {
            Some(p) => Store::open_at(p),
            None => {
                log::info!("settings: in memory");
                Store { path: None, value: DesktopSettings::default(), saved: DesktopSettings::default(), last_save_ms: 0 }
            }
        }
    }

    /// Load from `path` (a missing file means defaults; a damaged one is moved aside).
    pub fn open_at(path: PathBuf) -> Store {
        let value = load(&path);
        log::info!("settings: {}", path.display());
        Store { path: Some(path), saved: value.clone(), value, last_save_ms: 0 }
    }

    /// The current settings.
    pub fn get(&self) -> &DesktopSettings {
        &self.value
    }

    /// Replace them (written by [`Store::tick`] or [`Store::flush`]).
    pub fn set(&mut self, value: DesktopSettings) {
        self.value = value;
    }

    /// Save if something changed and the last save is at least a second old (`now_ms`).
    pub fn tick(&mut self, now_ms: i64) {
        if self.value != self.saved && now_ms.saturating_sub(self.last_save_ms) >= SAVE_EVERY_MS {
            self.last_save_ms = now_ms;
            if let Err(e) = self.flush() {
                log::warn!("settings not saved: {e}");
            }
        }
    }

    /// Whether a change is waiting to be saved (the app wakes up for it even when idle).
    pub fn pending(&self) -> bool {
        self.path.is_some() && self.value != self.saved
    }

    /// Save now if something changed.
    pub fn flush(&mut self) -> Result<(), String> {
        let Some(path) = &self.path else { return Ok(()) };
        if self.value == self.saved && path.exists() {
            return Ok(());
        }
        let bytes = serde_json::to_vec_pretty(&self.value).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        craft_atomic::atomic_write(path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        self.saved = self.value.clone();
        Ok(())
    }
}

/// Read `path`: defaults when it is missing; when it can't be read as settings, move it aside
/// and start from defaults.
fn load(path: &Path) -> DesktopSettings {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return DesktopSettings::default(),
    };
    let parsed = if meta.len() > MAX_BYTES {
        Err(format!("larger than {MAX_BYTES} bytes"))
    } else {
        std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<DesktopSettings>(&b).map_err(|e| e.to_string()))
    };
    match parsed {
        Ok(v) => v,
        Err(e) => {
            let aside = path.with_extension("json.broken");
            match std::fs::rename(path, &aside) {
                Ok(()) => log::warn!("settings: {} is damaged ({e}); moved to {} and starting from defaults", path.display(), aside.display()),
                Err(re) => log::warn!("settings: {} is damaged ({e}) and could not be moved aside ({re}); starting from defaults", path.display()),
            }
            DesktopSettings::default()
        }
    }
}

/// What the app looks like now, to be remembered.
pub fn capture(app: &NoteApp, previous: &DesktopSettings) -> DesktopSettings {
    let s = &app.session;
    let open: Vec<String> = s.ws.notebooks.iter().filter_map(|nb| s.notebook_path(nb.id)).map(|p| p.display().to_string()).collect();
    // A page of an in-memory notebook can't be reopened: keep the one remembered before.
    let last_page = match s.page() {
        Some(p) if s.notebook().is_some_and(|nb| s.notebook_path(nb.id).is_some()) => p.id.to_string(),
        _ => previous.notebooks.last_page.clone(),
    };
    DesktopSettings {
        appearance: Appearance { theme: app.ui.theme },
        editing: s.prefs.clone(),
        notebooks: Notebooks { open, last_page },
        layout: Layout { notebook_pane_width: app.ui.notebook_pane_width },
    }
}

/// Apply the remembered look and preferences (not the notebooks: [`reopen`] does that).
pub fn apply(app: &mut NoteApp, s: &DesktopSettings) {
    app.ui.theme = s.appearance.theme;
    app.session.prefs = s.editing.clone();
    // Options › General › User name starts as the account's name, as in OneNote.
    if app.session.prefs.author.trim().is_empty() {
        app.session.prefs.author = system_user_name();
    }
    if s.layout.notebook_pane_width.is_finite() {
        app.ui.notebook_pane_width = s.layout.notebook_pane_width.clamp(120.0, 600.0);
    }
}

/// The account's user name (`USER`, `USERNAME` on Windows), or empty.
fn system_user_name() -> String {
    ["USER", "USERNAME"].iter().find_map(|k| std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())).unwrap_or_default()
}

/// Reopen the remembered notebooks that still exist, then the page that was open. Returns how
/// many notebooks opened.
pub fn reopen(app: &mut NoteApp, s: &DesktopSettings) -> usize {
    let mut opened = 0;
    for path in &s.notebooks.open {
        if !std::path::Path::new(path).is_dir() {
            log::info!("not reopening {path}: it is gone");
            continue;
        }
        match app.session.open_path(path) {
            Ok(_) => opened += 1,
            Err(e) => log::warn!("{path}: {e}"),
        }
    }
    if let Some(id) = Id::parse(&s.notebooks.last_page)
        && app.session.ws.page(id).is_some()
        && let Err(e) = app.session.goto_page(id)
    {
        log::info!("last page: {e}");
    }
    opened
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("notecraft-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn settings_round_trip() {
        let path = tmp("round").join("settings.json");
        let mut want = DesktopSettings::default();
        want.appearance.theme = ThemeKind::Dark;
        want.editing.author = "Ada".into();
        want.editing.new_pages_rule_lines = Some(notecraft_engine::doc::RuleLines::College);
        want.editing.custom_tags = notecraft_engine::doc::builtin_tags().into_iter().take(2).collect();
        want.notebooks.open = vec!["/notes/Work".into()];
        want.notebooks.last_page = "00000000000000000000000000000001".into();
        want.layout.notebook_pane_width = 250.0;
        let mut store = Store::open_at(path.clone());
        assert_eq!(store.get(), &DesktopSettings::default());
        store.set(want.clone());
        // Debounced: the first change saves at once, the next waits a second.
        store.tick(5_000);
        assert_eq!(Store::open_at(path.clone()).get(), &want);
        let mut later = want.clone();
        later.appearance.theme = ThemeKind::White;
        store.set(later.clone());
        store.tick(5_500);
        assert_eq!(Store::open_at(path.clone()).get(), &want);
        store.tick(6_000);
        assert_eq!(Store::open_at(path.clone()).get(), &later);
        // Keys the file lacks take their defaults.
        std::fs::write(&path, br#"{"appearance": {"theme": "Dark"}}"#).unwrap();
        let s = Store::open_at(path.clone());
        assert_eq!(s.get().appearance.theme, ThemeKind::Dark);
        assert_eq!(s.get().editing, notecraft_engine::Prefs::default());
    }

    #[test]
    fn a_damaged_file_is_moved_aside() {
        let dir = tmp("bad");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ this is not json").unwrap();
        let mut s = Store::open_at(path.clone());
        assert_eq!(s.get(), &DesktopSettings::default());
        assert!(dir.join("settings.json.broken").exists());
        // The next save writes a good file.
        s.flush().unwrap();
        assert!(serde_json::from_slice::<DesktopSettings>(&std::fs::read(&path).unwrap()).is_ok());
        // A wrong type is damage too.
        std::fs::write(&path, br#"{"layout": {"notebook_pane_width": "wide"}}"#).unwrap();
        assert_eq!(Store::open_at(path).get(), &DesktopSettings::default());
    }

    #[test]
    fn capture_and_apply() {
        let clock = notecraft_engine::Clock::default();
        let mut app = NoteApp::new(notecraft_engine::Session::with_sample(clock), Box::new(notecraft_ui_egui::FixedHost(clock)));
        app.ui.theme = ThemeKind::White;
        app.session.prefs.author = "Grace".into();
        let s = capture(&app, &DesktopSettings::default());
        // The sample notebook lives in memory: nothing to reopen.
        assert!(s.notebooks.open.is_empty());
        assert!(s.notebooks.last_page.is_empty());
        let mut other = NoteApp::new(notecraft_engine::Session::new(clock), Box::new(notecraft_ui_egui::FixedHost(clock)));
        apply(&mut other, &s);
        assert_eq!(other.ui.theme, ThemeKind::White);
        assert_eq!(other.session.prefs.author, "Grace");
        let mut bad = s.clone();
        bad.layout.notebook_pane_width = f32::NAN;
        apply(&mut other, &bad);
        assert!(other.ui.notebook_pane_width.is_finite());
    }
}
