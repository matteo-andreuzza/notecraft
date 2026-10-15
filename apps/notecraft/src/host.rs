//! The desktop's platform services for the UI: the system clock, the system file dialogs, the
//! backups folder and the desktop's light/dark preference.

use notecraft_engine::Clock;
use notecraft_ui_egui::Host;

use crate::desktop_theme::DesktopTheme;

/// The desktop host. File dialogs are the system's (`rfd`: the XDG desktop portal on Linux)
/// and block the window while open (asynchronous dialogs are a known gap, `docs/gaps.md`).
pub struct DesktopHost {
    /// Watches the desktop's light/dark preference (a background thread polling the portal).
    theme: DesktopTheme,
    /// The desktop's last answer.
    dark: Option<bool>,
}

impl DesktopHost {
    /// A host whose theme watcher asks `ctx` for a repaint when the desktop's preference changes.
    pub fn new(ctx: &egui::Context) -> DesktopHost {
        let ctx = ctx.clone();
        DesktopHost { theme: DesktopTheme::start(move || ctx.request_repaint()), dark: None }
    }
}

/// The local time and UTC offset (chrono reads the system time zone itself).
pub fn clock() -> Clock {
    let now = chrono::Local::now();
    Clock { now_ms: now.timestamp_millis(), utc_offset_min: now.offset().local_minus_utc() / 60 }
}

impl Host for DesktopHost {
    fn clock(&self) -> Clock {
        clock()
    }

    fn pick_folder(&mut self, title: &str) -> Option<String> {
        rfd::FileDialog::new().set_title(title).pick_folder().map(|p| p.display().to_string())
    }

    fn save_file(&mut self, title: &str, default_name: &str) -> Option<String> {
        rfd::FileDialog::new().set_title(title).set_file_name(default_name).save_file().map(|p| p.display().to_string())
    }

    fn backup_folder(&self) -> Option<String> {
        notecraft_automation::keyfile::data_dir().map(|d| d.join("backups").display().to_string())
    }

    fn prefers_dark(&mut self) -> Option<bool> {
        if let Some(changed) = self.theme.take_change() {
            self.dark = Some(changed);
        }
        self.dark
    }
}
