//! A running NoteCraft app, reached through its loopback control port
//! (`notecraft-cli mcp --connect 127.0.0.1:PORT`).

use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde_json::{Value, json};

use crate::{Backend, keyfile};

/// Looks up the control key for an address (`HOST:PORT`).
pub type KeyFinder = Box<dyn Fn(&str) -> Result<String, String> + Send>;

/// The control channel of a running app.
pub struct Remote {
    addr: String,
    conn: Option<(BufReader<TcpStream>, TcpStream)>,
    next_id: u64,
    /// The connection leads to a loopback address: requests carry the control key.
    send_key: bool,
    /// The key, looked up once per connection (the app picks a new one at every start).
    key: Option<String>,
    find_key: KeyFinder,
}

/// The key for the app at `addr`: `NOTECRAFT_CONTROL_KEY`, else the app's key file.
fn find_key(addr: &str) -> Result<String, String> {
    let port = addr.rsplit_once(':').and_then(|(_, p)| p.parse().ok());
    keyfile::lookup_key(std::env::var("NOTECRAFT_CONTROL_KEY").ok(), keyfile::settings_dir().as_deref(), port)
}

/// Whether `addr` (`HOST:PORT`) names a loopback address: 127.0.0.0/8, ::1 or `localhost`.
/// Other host names are not resolved: they never count.
pub fn is_loopback_addr(addr: &str) -> bool {
    let loopback = |ip: IpAddr| ip.to_canonical().is_loopback();
    if let Ok(sa) = addr.parse::<SocketAddr>() {
        return loopback(sa.ip());
    }
    if let Ok(ip) = addr.parse::<IpAddr>() {
        return loopback(ip);
    }
    let host = addr.rsplit_once(':').map_or(addr, |(h, _)| h);
    let host = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    host.parse::<IpAddr>().map_or_else(|_| host.eq_ignore_ascii_case("localhost"), loopback)
}

impl Remote {
    /// Connect to `addr`, failing fast when nothing is listening.
    pub fn connect(addr: &str) -> std::io::Result<Remote> {
        Remote::connect_with(addr, Box::new(find_key))
    }

    /// [`Remote::connect`] with the key lookup given (tests).
    pub fn connect_with(addr: &str, find_key: KeyFinder) -> std::io::Result<Remote> {
        let mut r = Remote { addr: addr.to_string(), conn: None, next_id: 1, send_key: false, key: None, find_key };
        r.reconnect()?;
        if let Some(w) = r.key_warning() {
            log::warn!("{w}");
        }
        Ok(r)
    }

    /// Why requests go without the key (the address is not loopback), if they do.
    pub fn key_warning(&self) -> Option<String> {
        (!self.send_key).then(|| {
            format!(
                "{} is not a loopback address, so the control key is not sent (it only goes to 127.0.0.0/8, ::1 and localhost) and the app refuses requests; reach the app through 127.0.0.1, for example with an SSH tunnel",
                self.addr
            )
        })
    }

    fn reconnect(&mut self) -> std::io::Result<()> {
        self.conn = None;
        self.key = None;
        let mut last = std::io::Error::new(std::io::ErrorKind::NotFound, format!("cannot resolve {}", self.addr));
        for sa in self.addr.to_socket_addrs()? {
            match TcpStream::connect_timeout(&sa, Duration::from_millis(800)) {
                Ok(s) => {
                    s.set_nodelay(true).ok();
                    // The app answers within 60 s (its own timeout); leave headroom.
                    s.set_read_timeout(Some(Duration::from_secs(90))).ok();
                    // The key goes only to a loopback address, and only when the connection
                    // really leads to one (a `localhost` that resolves elsewhere gets none).
                    self.send_key = is_loopback_addr(&self.addr) && s.peer_addr().is_ok_and(|p| p.ip().to_canonical().is_loopback());
                    let read = s.try_clone()?;
                    self.conn = Some((BufReader::new(read), s));
                    return Ok(());
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn request_line(&mut self, id: u64, method: &str, params: &Value) -> Result<String, String> {
        let mut req = json!({"id": id, "method": method, "params": params});
        if self.send_key {
            let key = match &self.key {
                Some(k) => k.clone(),
                None => (self.find_key)(&self.addr)?,
            };
            req["key"] = json!(key);
            self.key = Some(key);
        }
        Ok(req.to_string())
    }

    fn roundtrip(&mut self, line: &str) -> std::io::Result<String> {
        let Some((reader, writer)) = self.conn.as_mut() else {
            return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "not connected"));
        };
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        let mut reply = String::new();
        if reader.read_line(&mut reply)? == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "control channel closed"));
        }
        Ok(reply)
    }

    /// One request and its reply, with one retry on a new connection (the app may have
    /// restarted; a new connection also reads the key again).
    fn exchange(&mut self, id: u64, method: &str, params: &Value) -> Result<Value, String> {
        let mut first = true;
        let reply = loop {
            let tried = if self.conn.is_some() { Ok(()) } else { self.reconnect() };
            let result = match tried {
                Ok(()) => {
                    let line = self.request_line(id, method, params)?;
                    self.roundtrip(&line)
                }
                Err(e) => Err(e),
            };
            match result {
                Ok(r) => break r,
                Err(e) => {
                    self.conn = None;
                    if !first {
                        return Err(format!("NoteCraft app at {} is not reachable: {e}", self.addr));
                    }
                    first = false;
                }
            }
        };
        serde_json::from_str(reply.trim()).map_err(|e| format!("bad reply from app: {e}"))
    }
}

impl Backend for Remote {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let unauthorized = |v: &Value| v.get("error").and_then(Value::as_str) == Some("unauthorized");
        let mut v = self.exchange(id, method, &params)?;
        if unauthorized(&v) && self.send_key {
            // A stale key (the app restarted on this port): read it again, once.
            self.conn = None;
            v = self.exchange(id, method, &params)?;
        }
        if v.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(v.get("result").cloned().unwrap_or(Value::Null))
        } else if unauthorized(&v) {
            self.conn = None;
            Err(self.key_warning().map_or_else(
                || {
                    format!(
                        "unauthorized: the NoteCraft app at {} refused the control key (from NOTECRAFT_CONTROL_KEY or the app's key file)",
                        self.addr
                    )
                },
                |w| format!("unauthorized: {w}"),
            ))
        } else {
            Err(v.get("error").and_then(Value::as_str).unwrap_or("unknown error").to_string())
        }
    }

    fn has_ui(&self) -> bool {
        true
    }

    fn describe(&self) -> String {
        format!("connected to the NoteCraft app at {}", self.addr)
    }
}
