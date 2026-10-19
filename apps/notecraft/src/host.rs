//! The desktop's platform services for the UI: the system clock, the system file dialogs, opening
//! a file with the desktop's handler, the backups folder and the desktop's light/dark preference.

use notecraft_engine::Clock;
use notecraft_ui_egui::{Host, RichClip};

use crate::desktop_theme::DesktopTheme;

/// The desktop host. File dialogs are the system's (`rfd`: the XDG desktop portal on Linux)
/// and block the window while open (asynchronous dialogs are a known gap, `docs/gaps.md`).
pub struct DesktopHost {
    /// Watches the desktop's light/dark preference (a background thread polling the portal).
    theme: DesktopTheme,
    /// The desktop's last answer.
    dark: Option<bool>,
    /// The system clipboard (`arboard`), opened on first use and kept: on X11 the copied
    /// contents are served by this object, so it must outlive the copy.
    clipboard: Option<arboard::Clipboard>,
}

impl DesktopHost {
    /// A host whose theme watcher asks `ctx` for a repaint when the desktop's preference changes.
    pub fn new(ctx: &egui::Context) -> DesktopHost {
        let ctx = ctx.clone();
        DesktopHost { theme: DesktopTheme::start(move || ctx.request_repaint()), dark: None, clipboard: None }
    }

    fn clipboard(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().map_err(|e| log::info!("clipboard: {e}")).ok();
        }
        self.clipboard.as_mut()
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

    fn pick_file(&mut self, title: &str, filters: &[(&str, &[&str])]) -> Option<String> {
        let mut d = rfd::FileDialog::new().set_title(title);
        for (name, exts) in filters {
            d = d.add_filter(*name, exts);
        }
        d.pick_file().map(|p| p.display().to_string())
    }

    fn open_path(&mut self, path: &str) -> Result<(), String> {
        // The desktop's own opener, reaped on a thread so the window never waits for it.
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(windows) {
            "explorer"
        } else {
            "xdg-open"
        };
        let mut child = std::process::Command::new(opener)
            .arg(path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("couldn't open {path}: {e}"))?;
        std::thread::spawn(move || child.wait());
        Ok(())
    }

    fn read_clipboard(&mut self, image: bool) -> RichClip {
        let Some(c) = self.clipboard() else { return RichClip::default() };
        let text = c.get_text().ok().filter(|t| !t.is_empty());
        let html = c.get().html().ok().filter(|h| !h.trim().is_empty());
        let image = if image {
            c.get_image().ok().and_then(|im| {
                // Another program's picture is hostile input: sized before it's copied.
                let (w, h) = (u32::try_from(im.width).ok()?, u32::try_from(im.height).ok()?);
                let limits = notecraft_codecs::Limits::PICTURE;
                if w == 0 || h == 0 || w > limits.max_side || h > limits.max_side || u64::from(w) * u64::from(h) > limits.max_pixels {
                    return None;
                }
                notecraft_codecs::Rgba8::new(w, h, im.bytes.into_owned()).ok()
            })
        } else {
            None
        };
        RichClip { text, html, image }
    }

    fn write_clipboard(&mut self, text: &str, html: &str, image: Option<&notecraft_codecs::Rgba8>) -> bool {
        let Some(c) = self.clipboard() else { return false };
        let r = match image {
            Some(img) => c.set_image(arboard::ImageData {
                width: img.width as usize,
                height: img.height as usize,
                bytes: std::borrow::Cow::Borrowed(&img.pixels),
            }),
            None if !html.is_empty() => c.set_html(html, Some(text)),
            None => c.set_text(text),
        };
        r.map_err(|e| log::info!("clipboard: {e}")).is_ok()
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
