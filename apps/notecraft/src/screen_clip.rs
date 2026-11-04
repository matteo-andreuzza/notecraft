//! Screen clipping on the desktop (`R.Insert.Images.ScreenCaptureInsert`): on Linux the XDG
//! Screenshot portal asks the user for an area (the compositor draws the selection, so it works
//! the same on Wayland and X11) and hands back a picture file. The portal answers through a
//! D-Bus signal after the user is done, so the call runs on its own thread and the result
//! arrives through a channel the UI polls every frame (`Host::take_screen_clip`).

use std::sync::mpsc::{Receiver, channel};

/// Largest picture accepted from the portal (the D3 picture cap).
const MAX_BYTES: u64 = 64 << 20;

/// A screen clipping in progress.
pub struct Clip(Receiver<Result<Vec<u8>, String>>);

impl Clip {
    /// Start one; `done` runs when the answer is ready (it wakes the UI).
    pub fn start(done: impl Fn() + Send + 'static) -> Result<Clip, String> {
        if !cfg!(target_os = "linux") {
            return Err("Screen clipping isn't available on this system yet".into());
        }
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("screen-clip".into())
            .spawn(move || {
                let _ = tx.send(capture());
                done();
            })
            .map_err(|e| format!("screen clipping: {e}"))?;
        Ok(Clip(rx))
    }

    /// The answer, once it has come.
    pub fn take(&self) -> Option<Result<Vec<u8>, String>> {
        self.0.try_recv().ok()
    }
}

/// `file:///a%20b.png` → `/a b.png` (only local files; the portal gives nothing else).
fn file_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(h) = rest.get(i + 1..i + 3)
            && let Ok(b) = u8::from_str_radix(h, 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).ok()
}

#[cfg(target_os = "linux")]
fn capture() -> Result<Vec<u8>, String> {
    use std::collections::HashMap;
    use std::io::Read;

    use zbus::zvariant::{OwnedValue, Value};

    const PORTAL: &str = "org.freedesktop.portal.Desktop";
    let fail = |e: zbus::Error| format!("screen clipping: {e}");
    let conn = zbus::blocking::Connection::session().map_err(fail)?;
    // The request object's path is known in advance (portal API): subscribe to its Response
    // before calling, so a fast answer can't be missed.
    let sender = conn.unique_name().map(|n| n.as_str().trim_start_matches(':').replace('.', "_")).ok_or("screen clipping: no bus name")?;
    let token = format!("notecraft_clip_{}", std::process::id());
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let request = zbus::blocking::Proxy::new(&conn, PORTAL, path.as_str(), "org.freedesktop.portal.Request").map_err(fail)?;
    let mut answers = request.receive_signal("Response").map_err(fail)?;
    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert("interactive", Value::from(true));
    conn.call_method(Some(PORTAL), "/org/freedesktop/portal/desktop", Some("org.freedesktop.portal.Screenshot"), "Screenshot", &("", options))
        .map_err(|e| format!("screen clipping isn't available from this desktop ({e})"))?;
    let answer = answers.next().ok_or("screen clipping: the desktop didn't answer")?;
    let (code, results): (u32, HashMap<String, OwnedValue>) = answer.body().deserialize().map_err(fail)?;
    if code != 0 {
        return Err("Screen clipping was cancelled.".into());
    }
    let uri = results.get("uri").and_then(|v| String::try_from(v.clone()).ok()).ok_or("screen clipping: no picture came back")?;
    let file = file_path(&uri).ok_or("screen clipping: the picture isn't a local file")?;
    let f = std::fs::File::open(&file).map_err(|e| format!("{file}: {e}"))?;
    let mut bytes = Vec::new();
    f.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| format!("{file}: {e}"))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("the screen clipping is too large".into());
    }
    Ok(bytes)
}

#[cfg(not(target_os = "linux"))]
fn capture() -> Result<Vec<u8>, String> {
    Err("Screen clipping isn't available on this system yet".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_uris_become_paths() {
        assert_eq!(file_path("file:///home/me/Pictures/Screenshot%20from%202026.png").as_deref(), Some("/home/me/Pictures/Screenshot from 2026.png"));
        assert_eq!(file_path("file:///a%2"), Some("/a%2".into()));
        assert_eq!(file_path("https://example.org/x.png"), None);
    }
}
