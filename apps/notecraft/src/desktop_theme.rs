//! The desktop's own light/dark choice, for the "Use system setting" Office Theme
//! (`OPT.General.OfficeTheme`). Ported from WordCraft (MIT/Apache, same team).
//!
//! winit reports no system theme on Linux — `ActiveEventLoop::system_theme` is `None` on every
//! desktop there — so "Use system setting" has to ask the desktop itself. The XDG Desktop Portal's
//! Settings interface answers on GNOME, KDE and XFCE alike, over X11 and Wayland alike.

use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

/// How often [`DesktopTheme::start`] re-reads the desktop's answer. The portal also publishes a
/// change signal, but subscribing needs a connection kept alive for the life of every window; one
/// round trip every few seconds is cheap and reacts soon enough after a theme switch.
const POLL: Duration = Duration::from_secs(4);

/// Watches the desktop and reports its choice: once at start-up, then whenever the user switches
/// theme while the app runs.
#[derive(Default)]
pub struct DesktopTheme {
    /// `None` when no watcher thread could be started; the one-shot read still works.
    changes: Option<Receiver<Option<bool>>>,
}

impl DesktopTheme {
    /// Start watching. Nothing runs before this is called, and calling it once is enough. The
    /// first answer is sent straight away, so the caller does not have to ask for it separately.
    /// `changed` runs on the watcher's thread after each answer is sent (the app asks for a
    /// repaint there, so an idle window picks the change up). The app binary starts this; tests
    /// and headless screenshots don't, so they never depend on the machine's desktop theme.
    pub fn start(changed: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = channel();
        let started = std::thread::Builder::new().name("notecraft-desktop-theme".into()).spawn(move || {
            // One session-bus connection for the life of the watcher, opened again only after
            // it fails.
            let mut bus = Bus::new();
            let mut last = read(&mut bus);
            if tx.send(last).is_err() {
                return;
            }
            changed();
            loop {
                std::thread::sleep(POLL);
                let now = read(&mut bus);
                if now != last {
                    last = now;
                    if tx.send(now).is_err() {
                        return;
                    }
                    changed();
                }
            }
        });
        Self { changes: started.ok().map(|_| rx) }
    }

    /// The desktop's new answer if it changed since the last call, to be polled once a frame.
    /// Several changes in one frame collapse to the newest, and a desktop that stops answering
    /// leaves the last answer in place.
    pub fn take_change(&mut self) -> Option<bool> {
        let Some(changes) = &self.changes else { return None };
        let mut newest = None;
        loop {
            match changes.try_recv() {
                Ok(changed) => newest = changed,
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return newest,
            }
        }
    }
}

/// The watcher's session-bus connection, kept between polls.
#[cfg(target_os = "linux")]
struct Bus(Option<zbus::blocking::Connection>);

#[cfg(target_os = "linux")]
impl Bus {
    /// No connection yet: the first read opens one.
    fn new() -> Self {
        Bus(None)
    }
}

/// Elsewhere there is no session bus to keep.
#[cfg(not(target_os = "linux"))]
struct Bus;

#[cfg(not(target_os = "linux"))]
impl Bus {
    fn new() -> Self {
        Bus
    }
}

/// Ask the portal once, over `bus`'s connection (opened when there is none). D-Bus calls
/// block, so this belongs off the UI thread. A connection whose calls all fail is dropped, so
/// the next poll opens a fresh one (the bus or portal may have restarted).
#[cfg(target_os = "linux")]
fn read(bus: &mut Bus) -> Option<bool> {
    const PORTAL: &str = "org.freedesktop.portal.Desktop";
    const SETTINGS: &str = "org.freedesktop.portal.Settings";
    const SCHEMAS: [(&str, &str); 2] = [("org.gnome.desktop.interface", "color-scheme"), ("org.freedesktop.interface.color-scheme", "color-scheme")];
    if bus.0.is_none() {
        bus.0 = zbus::blocking::Connection::session().ok();
    }
    let connection = bus.0.as_ref()?;
    let mut answered = false;
    for (schema, key) in SCHEMAS {
        let Ok(reply) = connection.call_method(Some(PORTAL), "/org/freedesktop/portal/desktop", Some(SETTINGS), "ReadOne", &(schema, key)) else {
            continue;
        };
        answered = true;
        let Ok((answer,)) = reply.body().deserialize::<(zbus::zvariant::OwnedValue,)>() else { continue };
        if let Ok(scheme) = String::try_from(&*answer) {
            return parse(&scheme);
        }
    }
    if !answered {
        bus.0 = None;
    }
    None
}

/// The portal's `color-scheme` answer. `default` means the desktop states no preference, so
/// neither does the application.
#[cfg(target_os = "linux")]
fn parse(scheme: &str) -> Option<bool> {
    match scheme {
        "prefer-dark" => Some(true),
        "prefer-light" => Some(false),
        _ => None,
    }
}

/// macOS and Windows answer through winit, which the caller already has.
#[cfg(not(target_os = "linux"))]
fn read(_bus: &mut Bus) -> Option<bool> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The portal answers with one of three strings; only the two preferences are a choice.
    #[test]
    #[cfg(target_os = "linux")]
    fn the_portal_answer_reads_as_a_choice_or_as_none() {
        assert_eq!(parse("prefer-dark"), Some(true));
        assert_eq!(parse("prefer-light"), Some(false));
        assert_eq!(parse("default"), None);
        assert_eq!(parse(""), None);
    }

    /// A watcher that was never started is inert rather than a panic.
    #[test]
    fn a_watcher_without_a_thread_is_inert() {
        assert_eq!(DesktopTheme::default().take_change(), None);
    }
}
