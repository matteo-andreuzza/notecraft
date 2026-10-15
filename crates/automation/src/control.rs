//! The desktop app's loopback JSON-lines control channel: one request per line, one reply per
//! line (`docs/control-protocol.md`). This is the transport `notecraft-cli mcp --connect` wraps.
//!
//! Requests reach the app as [`Request`]s on a channel; the app answers them on its UI thread
//! (where the session lives) and the connection thread writes the reply. Every request needs
//! the key the app writes to `<settings>/control-key.<instance>` (mode 0600) once the port is
//! bound; the file is removed when the window closes.
//!
//! The module is native-only (`cfg(not(target_arch = "wasm32"))` in `lib.rs`), so its read
//! deadlines may use `Instant::now`.
#![allow(clippy::disallowed_methods)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::keyfile;

/// Longer request lines are refused (and the connection closed).
pub const MAX_LINE: usize = 1 << 20;

/// A connection that sends no authorised request in this time is closed.
pub const FIRST_REQUEST: Duration = Duration::from_secs(30);

/// How long a connection waits for the app to answer one request.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(60);

/// Wakes the app's event loop when a request arrives (`egui::Context::request_repaint`).
pub type Waker = Arc<dyn Fn() + Send + Sync>;

/// One request for the app to answer.
pub struct Request {
    /// Method name (`engine.execute`, `notebook.inspect`, `ui.key`…).
    pub method: String,
    /// Its params.
    pub params: Value,
    reply: Sender<Value>,
}

impl Request {
    /// A request and the receiver its reply arrives on (also for tests and in-process callers).
    pub fn new(method: impl Into<String>, params: Value) -> (Request, Receiver<Value>) {
        let (tx, rx) = channel();
        (Request { method: method.into(), params, reply: tx }, rx)
    }

    /// Answer the request (`Ok` → `{"ok": true, "result": …}`).
    pub fn answer(self, r: Result<Value, String>) {
        let v = match r {
            Ok(v) => json!({"ok": true, "result": v}),
            Err(e) => json!({"ok": false, "error": e}),
        };
        let _ = self.reply.send(v);
    }
}

/// The key file of a running control server; [`KeyFile::remove`] it when the window closes.
pub struct KeyFile(PathBuf);

impl KeyFile {
    /// Where it is.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Delete it.
    pub fn remove(&self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Bind `127.0.0.1:port`, write the key file into `settings` and serve. `None` when the port
/// cannot be bound or there is no random key (the channel is off); the key file is `None` when
/// it could not be written (requests are then refused).
pub fn start(port: u16, settings: Option<&Path>, waker: Waker) -> Option<(Receiver<Request>, Option<KeyFile>)> {
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            log::error!("control channel: cannot bind 127.0.0.1:{port}: {e}");
            return None;
        }
    };
    let key = match random_key() {
        Ok(k) => k,
        Err(e) => {
            log::error!("control channel off: no random key ({e})");
            return None;
        }
    };
    let key_file = match settings {
        Some(dir) => {
            let p = keyfile::key_path(dir, port);
            match write_key_file(&p, &key) {
                Ok(()) => Some(KeyFile(p)),
                Err(e) => {
                    log::error!("control key file not written ({e}): requests will be refused");
                    None
                }
            }
        }
        None => {
            log::error!("no settings folder for the control key file: requests will be refused");
            None
        }
    };
    let key = if key_file.is_some() { key } else { String::new() };
    match &key_file {
        Some(f) => log::info!("control channel on 127.0.0.1:{port}, key in {}", f.path().display()),
        None => log::info!("control channel on 127.0.0.1:{port}"),
    }
    Some((serve_on(listener, key, waker, FIRST_REQUEST), key_file))
}

/// A new key: 32 random bytes from the operating system, as hex.
fn random_key() -> Result<String, String> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| e.to_string())?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// Write the key file: a stale one is replaced; on Unix the new file is created with mode 0600.
fn write_key_file(p: &Path, key: &str) -> Result<(), String> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
    }
    let _ = std::fs::remove_file(p);
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut open, 0o600);
    let mut f = open.open(p).map_err(|e| format!("cannot create {}: {e}", p.display()))?;
    f.write_all(key.as_bytes()).map_err(|e| format!("cannot write {}: {e}", p.display()))
}

/// Compare keys in time that does not depend on where they differ. An empty key never matches.
pub fn keys_match(given: &str, key: &str) -> bool {
    let (a, b) = (given.as_bytes(), key.as_bytes());
    !b.is_empty() && a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Serve an already bound listener with `key` (tests bind port 0).
pub fn serve_on(listener: TcpListener, key: String, waker: Waker, first: Duration) -> Receiver<Request> {
    let (tx, rx) = channel::<Request>();
    let key: Arc<str> = key.into();
    let spawned = std::thread::Builder::new().name("control-accept".into()).spawn(move || {
        for stream in listener.incoming().flatten() {
            let (tx, key, waker) = (tx.clone(), key.clone(), waker.clone());
            if let Err(e) = std::thread::Builder::new().name("control-conn".into()).spawn(move || serve(stream, tx, &key, &waker, first)) {
                log::warn!("control channel: no thread for a connection: {e}");
            }
        }
    });
    if let Err(e) = spawned {
        log::error!("control channel: no accept thread: {e}");
    }
    rx
}

/// The read side of a connection with an optional overall deadline: each read waits only for
/// the time left, so blank lines, bad JSON or a slow drip of bytes cannot extend it.
struct Timed {
    s: TcpStream,
    deadline: Option<Instant>,
}

impl Read for Timed {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(d) = self.deadline {
            let left = d.checked_duration_since(Instant::now()).filter(|l| !l.is_zero()).ok_or(std::io::ErrorKind::TimedOut)?;
            self.s.set_read_timeout(Some(left))?;
        }
        self.s.read(buf)
    }
}

impl Timed {
    fn set_deadline(&mut self, d: Option<Instant>) {
        self.deadline = d;
        if d.is_none() {
            let _ = self.s.set_read_timeout(None);
        }
    }
}

/// The next line (trimmed), `None` at the end of the stream or after a read error.
fn read_line(r: &mut BufReader<Timed>) -> Option<Result<String, &'static str>> {
    let mut buf = Vec::new();
    let n = r.by_ref().take(MAX_LINE as u64 + 1).read_until(b'\n', &mut buf).ok()?;
    if n == 0 {
        return None;
    }
    if buf.len() > MAX_LINE {
        return Some(Err("line too long"));
    }
    Some(String::from_utf8(buf).map(|s| s.trim().to_string()).map_err(|_| "not UTF-8"))
}

/// Read and drop the rest of an oversize line, so closing the socket does not reset the
/// connection before the client has read the error. At most 64 MiB, within 2 s.
fn discard_rest_of_line(r: &mut BufReader<Timed>) {
    r.get_mut().set_deadline(Some(Instant::now() + Duration::from_secs(2)));
    let mut left: usize = 64 << 20;
    while left > 0 {
        let (used, done) = match r.fill_buf() {
            Ok([]) | Err(_) => return,
            Ok(b) => match b.iter().position(|&c| c == b'\n') {
                Some(i) => (i + 1, true),
                None => (b.len(), false),
            },
        };
        r.consume(used);
        if done {
            return;
        }
        left = left.saturating_sub(used);
    }
}

fn serve(stream: TcpStream, tx: Sender<Request>, key: &str, waker: &Waker, first: Duration) {
    let Ok(read) = stream.try_clone() else { return };
    let mut out = stream;
    // One overall deadline from accept to the first authorised request; after it, none.
    let mut reader = BufReader::new(Timed { s: read, deadline: Some(Instant::now() + first) });
    let mut authorised = false;
    let mut conn_key: Option<String> = None;
    while let Some(line) = read_line(&mut reader) {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                let _ = writeln!(out, "{}", json!({"ok": false, "error": e}));
                if e == "line too long" {
                    discard_rest_of_line(&mut reader);
                }
                break;
            }
        };
        if line.is_empty() {
            continue;
        }
        let msg = match serde_json::from_str::<Value>(&line) {
            Ok(m) => m,
            // A line that isn't JSON closes the connection: an HTTP request (a web page's
            // `fetch` to this port) starts with a request line that never parses, so its body
            // can't carry a command.
            Err(e) => {
                let _ = writeln!(out, "{}", json!({"ok": false, "error": format!("bad JSON: {e}; closing the connection")}));
                break;
            }
        };
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("").to_string();
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        // The top-level `key`; only `auth` reads it from its params (`ui.key {"key": "Enter"}`
        // presses Enter).
        let given =
            msg.get("key").and_then(Value::as_str).or_else(|| if method == "auth" { params.get("key").and_then(Value::as_str) } else { None });
        let mut reply = |mut v: Value| -> bool {
            if let Some(o) = v.as_object_mut() {
                o.insert("id".into(), id.clone());
            }
            writeln!(out, "{v}").is_ok()
        };
        let k = given.or(conn_key.as_deref()).unwrap_or("");
        if !keys_match(k, key) {
            let _ = reply(json!({"ok": false, "error": "unauthorized"}));
            break;
        }
        if !authorised {
            authorised = true;
            reader.get_mut().set_deadline(None);
        }
        if method == "auth" {
            conn_key = Some(k.to_string());
            if !reply(json!({"ok": true, "result": {}})) {
                break;
            }
            continue;
        }
        let (req, rrx) = Request::new(method, params);
        if tx.send(req).is_err() {
            break;
        }
        waker();
        let r = rrx.recv_timeout(ANSWER_TIMEOUT).unwrap_or_else(|_| json!({"ok": false, "error": "timeout"}));
        if !reply(r) {
            break;
        }
    }
}
