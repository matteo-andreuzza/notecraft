//! NoteCraft desktop app: the egui shell (`notecraft-ui-egui`) in a native window (eframe on
//! wgpu; Wayland and X11 on Linux).
//!
//! ```text
//! notecraft [--sample] [--control PORT] [NOTEBOOK_FOLDER…]
//! ```
//!
//! - Notebook folders on the command line are opened. Without any, the notebooks open when the
//!   app last closed are reopened (with the page that was open); on the very first run a
//!   folder notebook "My Notebook" is created in `~/Documents/NoteCraft Notebooks` (or the
//!   data folder when there is no Documents folder), as OneNote starts with a default notebook.
//!   `--sample` adds the sample notebook (in memory: its changes are not kept). With
//!   `NOTECRAFT_NO_PREFS` nothing is remembered and the sample notebook opens instead.
//! - `--control PORT` (or `NOTECRAFT_CONTROL_PORT`) starts the loopback JSON-lines control
//!   channel, guarded by a key in `<settings>/control-key.<instance>` (mode 0600, removed on
//!   exit). See `docs/control-protocol.md`.
//! - Folder notebooks are saved a second after each change and when the window closes.
//! - Logs go to standard error (`NOTECRAFT_LOG=info|debug|off`); a panic is logged before the
//!   process ends.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod control;
mod desktop_theme;
mod host;
mod screen_clip;
mod settings;

use std::path::PathBuf;
use std::process::ExitCode;

use notecraft_automation::control::KeyFile;
use notecraft_automation::keyfile;
use notecraft_engine::Session;
use notecraft_ui_egui::NoteApp;
use settings::{DesktopSettings, Store};

const USAGE: &str = "\
usage: notecraft [--sample] [--control PORT] [NOTEBOOK_FOLDER...]

  NOTEBOOK_FOLDER   open these notebook folders (default: the ones open last time)
  --sample          also open the sample notebook (in memory)
  --control PORT    serve the JSON-lines control channel on 127.0.0.1:PORT
                    (also NOTECRAFT_CONTROL_PORT; see docs/control-protocol.md)
  --version         print the version
";

/// The parsed command line.
#[derive(Debug, Default, PartialEq)]
struct Args {
    sample: bool,
    control: Option<u16>,
    folders: Vec<String>,
}

/// What `main` should do.
#[derive(Debug, PartialEq)]
enum Cli {
    Run(Args),
    Print(String),
    Fail(String),
}

fn parse_args(args: &[String], env_port: Option<String>) -> Cli {
    let port = |v: &str| v.parse::<u16>().ok().filter(|p| *p != 0);
    let mut a = Args::default();
    if let Some(v) = env_port.filter(|v| !v.trim().is_empty()) {
        match port(v.trim()) {
            Some(p) => a.control = Some(p),
            None => return Cli::Fail(format!("NOTECRAFT_CONTROL_PORT: not a port: {v}")),
        }
    }
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n, Some(v.to_string())),
            _ => (arg.as_str(), None),
        };
        match name {
            "--sample" => a.sample = true,
            "--control" => {
                let Some(v) = inline.or_else(|| it.next().cloned()) else { return Cli::Fail("--control needs a port".into()) };
                match port(&v) {
                    Some(p) => a.control = Some(p),
                    None => return Cli::Fail(format!("--control: not a port: {v}")),
                }
            }
            "--version" | "-V" => return Cli::Print(format!("notecraft {}", env!("CARGO_PKG_VERSION"))),
            "--help" | "-h" => return Cli::Print(USAGE.to_string()),
            "--" => a.folders.extend(it.by_ref().cloned()),
            n if n.starts_with('-') && n.len() > 1 => return Cli::Fail(format!("unknown option {arg}\n\n{USAGE}")),
            _ => a.folders.push(arg.clone()),
        }
    }
    Cli::Run(a)
}

/// Standard-error logger: warnings by default, `NOTECRAFT_LOG` for more or none.
struct StderrLog;

impl log::Log for StderrLog {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::max_level()
    }
    fn log(&self, r: &log::Record) {
        // Our own records, and warnings from the libraries (wgpu and winit are chatty at info).
        if self.enabled(r.metadata()) && (r.target().starts_with("notecraft") || r.level() <= log::Level::Warn) {
            eprintln!("notecraft: {}: {}", r.level().as_str().to_lowercase(), r.args());
        }
    }
    fn flush(&self) {}
}

fn init_log() {
    let level = match std::env::var("NOTECRAFT_LOG").as_deref() {
        Ok("debug") => log::LevelFilter::Debug,
        Ok("info") => log::LevelFilter::Info,
        Ok("off") => log::LevelFilter::Off,
        _ => log::LevelFilter::Warn,
    };
    if log::set_logger(&StderrLog).is_ok() {
        log::set_max_level(level);
    }
    // A panic that escapes the engine's guard: record it in the log before the process ends
    // (folder notebooks were saved a second after their last change).
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("crashed: {info}");
        default(info);
    }));
}

/// Where the first-run notebook goes: `~/Documents/NoteCraft Notebooks/My Notebook`, or the
/// data folder when there is no Documents folder.
fn default_notebook_dir() -> Option<PathBuf> {
    let docs = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Documents")).filter(|d| d.is_dir());
    match docs {
        Some(d) => Some(d.join("NoteCraft Notebooks").join("My Notebook")),
        None => keyfile::data_dir().map(|d| d.join("notebooks").join("My Notebook")),
    }
}

/// Open the session's notebooks: the folders given, else the remembered ones, else the
/// first-run notebook (or the sample without preferences). Problems are logged and shown in
/// the status bar; the app always starts.
fn open_notebooks(app: &mut NoteApp, args: &Args, remembered: &DesktopSettings) {
    for f in &args.folders {
        if let Err(e) = app.session.open_path(f) {
            log::warn!("{f}: {e}");
            app.session.notice(format!("Couldn't open {f}: {e}"));
        }
    }
    if !args.folders.is_empty() || args.sample {
        return;
    }
    if settings::reopen(app, remembered) > 0 {
        return;
    }
    if !settings::enabled() {
        let now = app.session.now();
        let nb = notecraft_engine::sample::sample_notebook(&mut app.session.ids, now);
        app.session.add_notebook(nb);
        return;
    }
    let Some(dir) = default_notebook_dir() else { return };
    let path = dir.display().to_string();
    let r = if dir.is_dir() {
        app.session.open_path(&path)
    } else {
        app.session.run("file.newNotebook", serde_json::json!({"name": "My Notebook", "path": path}))
    };
    match r {
        Ok(_) => {
            // Creating the default notebook is not an edit to undo.
            app.session.clear_history();
            log::info!("default notebook: {path}");
        }
        Err(e) => log::warn!("default notebook {path}: {e}"),
    }
}

/// The app inside eframe: the UI, its remembered settings and the control channel.
struct Desktop {
    app: NoteApp,
    store: Store,
    control: Option<control::Control>,
    key_file: Option<KeyFile>,
    /// The window as last observed (kept in the settings).
    window: settings::Window,
}

impl Desktop {
    /// Remember the current look and notebooks (saved at most once a second, and on exit).
    fn remember(&mut self) {
        let mut snap = settings::capture(&self.app, self.store.get());
        snap.window = self.window;
        if &snap != self.store.get() {
            self.store.set(snap);
        }
    }
}

impl eframe::App for Desktop {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.app.logic(ctx);
        self.window = ctx.input(|i| settings::Window::observe(self.window, i.viewport()));
        if let Some(c) = &mut self.control {
            c.drain(&mut self.app, ctx);
        }
        self.remember();
        self.store.tick(self.app.session.clock.now_ms);
        if self.store.pending() {
            ctx.request_repaint_after(std::time::Duration::from_millis(1100));
        }
        if self.app.quit_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(c) = &mut self.control {
            raw.events.extend(c.take_input());
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
    }

    fn on_exit(&mut self) {
        for e in self.app.session.save_dirty() {
            log::error!("saving on exit: {e}");
        }
        self.remember();
        if let Err(e) = self.store.flush() {
            log::warn!("settings not saved: {e}");
        }
        if let Some(k) = &self.key_file {
            k.remove();
        }
    }
}

/// Window and taskbar icon (drawn by us: `assets/app-icon`).
fn app_icon() -> Option<egui::IconData> {
    let png: &[u8] = include_bytes!("../../../assets/app-icon/hicolor/256x256/apps/ai.storyteller.notecraft.png");
    eframe::icon_data::from_png_bytes(png).map_err(|e| log::warn!("app icon: {e}")).ok()
}

fn main() -> ExitCode {
    init_log();
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&argv, std::env::var("NOTECRAFT_CONTROL_PORT").ok()) {
        Cli::Run(a) => a,
        Cli::Print(s) => {
            print!("{s}");
            if !s.ends_with('\n') {
                println!();
            }
            return ExitCode::SUCCESS;
        }
        Cli::Fail(e) => {
            eprintln!("notecraft: {e}");
            return ExitCode::from(2);
        }
    };
    // The settings are read before the window opens: it opens at the size it had last time.
    let store = Store::open();
    let window = store.get().window.sane();
    let mut viewport = window.viewport(egui::ViewportBuilder::default().with_title("NoteCraft").with_app_id("ai.storyteller.notecraft"));
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    let result = eframe::run_native(
        "NoteCraft",
        options,
        Box::new(move |cc| {
            let remembered = store.get().clone();
            let session = if args.sample { Session::with_sample(host::clock()) } else { Session::new(host::clock()) };
            let mut app = NoteApp::new(session, Box::new(host::DesktopHost::new(&cc.egui_ctx)));
            settings::apply(&mut app, &remembered);
            open_notebooks(&mut app, &args, &remembered);
            let (mut control, mut key_file) = (None, None);
            if let Some(port) = args.control {
                let ctx = cc.egui_ctx.clone();
                let waker: notecraft_automation::control::Waker = std::sync::Arc::new(move || ctx.request_repaint());
                match notecraft_automation::control::start(port, keyfile::settings_dir().as_deref(), waker) {
                    Some((rx, file)) => {
                        control = Some(control::Control::new(rx));
                        key_file = file;
                    }
                    None => app.session.notice(format!("The control channel could not start on port {port} (see the log).")),
                }
            }
            Ok(Box::new(Desktop { app, store, control, key_file, window }))
        }),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("the window could not open: {e}");
            eprintln!("notecraft: the window could not open: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn command_line() {
        assert_eq!(parse_args(&args(&[]), None), Cli::Run(Args::default()));
        assert_eq!(
            parse_args(&args(&["--sample", "--control", "7990", "/n/Work", "--", "--odd name"]), None),
            Cli::Run(Args { sample: true, control: Some(7990), folders: vec!["/n/Work".into(), "--odd name".into()] })
        );
        assert_eq!(parse_args(&args(&["--control=7991"]), None), Cli::Run(Args { control: Some(7991), ..Args::default() }));
        assert_eq!(parse_args(&args(&[]), Some("7992".into())), Cli::Run(Args { control: Some(7992), ..Args::default() }));
        assert_eq!(parse_args(&args(&["--control", "7993"]), Some("7992".into())), Cli::Run(Args { control: Some(7993), ..Args::default() }));
        assert!(matches!(parse_args(&args(&["--control"]), None), Cli::Fail(_)));
        assert!(matches!(parse_args(&args(&["--control", "0"]), None), Cli::Fail(_)));
        assert!(matches!(parse_args(&args(&["--control", "99999"]), None), Cli::Fail(_)));
        assert!(matches!(parse_args(&args(&[]), Some("abc".into())), Cli::Fail(_)));
        assert!(matches!(parse_args(&args(&["--bogus"]), None), Cli::Fail(_)));
        assert!(matches!(parse_args(&args(&["--version"]), None), Cli::Print(v) if v.starts_with("notecraft ")));
        assert!(matches!(parse_args(&args(&["-h"]), None), Cli::Print(v) if v.contains("--control")));
    }

    #[test]
    fn folders_given_are_opened_and_failures_reported() {
        let clock = notecraft_engine::Clock::default();
        let dir = std::env::temp_dir().join(format!("notecraft-desktop-open-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = Session::new(clock);
        s.run("file.newNotebook", serde_json::json!({"name": "Work", "path": dir.display().to_string()})).unwrap();
        drop(s);
        let mut app = NoteApp::new(Session::new(clock), Box::new(notecraft_ui_egui::FixedHost(clock)));
        let a = Args { folders: vec![dir.display().to_string(), "/definitely/not/a/notebook".into()], ..Args::default() };
        open_notebooks(&mut app, &a, &DesktopSettings::default());
        assert_eq!(app.session.ws.notebooks.len(), 1);
        assert!(app.session.messages.iter().any(|m| m.contains("/definitely/not/a/notebook")));
    }
}
