//! Tests for the automation crate: path roots, the control channel's key rules, the remote
//! bridge, the MCP server and the headless backend. Native only (sockets, files).
#![cfg(not(target_arch = "wasm32"))]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use notecraft_engine::{Clock, Session};
use serde_json::{Value, json};

use crate::control::{self, Request};
use crate::mcp::{self, Server};
use crate::{Backend, Headless, NEEDS_APP, Remote, Roots, keyfile, remote};

const KEY: &str = "0123456789abcdef0123456789abcdef";

fn clock() -> Clock {
    Clock { now_ms: 1_760_000_000_000, utc_offset_min: 0 }
}

fn headless() -> Headless {
    Headless::new(Session::with_sample(clock()), Box::new(clock))
}

/// A fresh, empty folder under the system temp dir (canonical, so root checks compare cleanly).
fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("notecraft-automation-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.canonicalize().unwrap()
}

fn s(p: &Path) -> String {
    p.display().to_string()
}

// ---------------------------------------------------------------------------------------------
// Roots

#[test]
fn roots_allow_paths_inside_and_refuse_outside() {
    let base = tmp("roots-inside");
    let root = base.join("root");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    let r = Roots::new(Some(&root), Some(&root)).unwrap();
    assert_eq!(r.check_read(&s(&root.join("sub"))).unwrap(), root.join("sub"));
    // A path that does not exist yet, below an existing folder.
    assert_eq!(r.check_write(&s(&root.join("sub/new/deeper"))).unwrap(), root.join("sub/new/deeper"));
    // Outside: a sibling of the root, and the parent.
    std::fs::create_dir_all(base.join("other")).unwrap();
    assert!(r.check_read(&s(&base.join("other"))).unwrap_err().contains("outside the read root"));
    assert!(r.check_write(&s(&base)).unwrap_err().contains("outside the write root"));
    // A sibling whose name merely starts with the root's name is still outside.
    std::fs::create_dir_all(base.join("rootx")).unwrap();
    assert!(r.check_write(&s(&base.join("rootx/a"))).is_err());
    assert!(r.check_write("").unwrap_err().contains("empty path"));
}

#[test]
fn roots_refuse_dot_dot_escapes() {
    let base = tmp("roots-dotdot");
    let root = base.join("root");
    std::fs::create_dir_all(&root).unwrap();
    let r = Roots::new(Some(&root), Some(&root)).unwrap();
    // Through an existing folder: canonicalised to the parent's child.
    assert!(r.check_write(&format!("{}/../escaped", s(&root))).is_err());
    // Through a folder that does not exist: refused before anything resolves.
    assert!(r.check_write(&format!("{}/missing/../../escaped", s(&root))).is_err());
    assert!(r.check_read(&format!("{}/..", s(&root))).is_err());
}

#[cfg(unix)]
#[test]
fn roots_follow_links_and_refuse_dangling_ones() {
    let base = tmp("roots-links");
    let root = base.join("root");
    let outside = base.join("outside");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
    std::os::unix::fs::symlink(base.join("nowhere"), root.join("dangling")).unwrap();
    let r = Roots::new(Some(&root), Some(&root)).unwrap();
    assert!(r.check_read(&s(&root.join("link"))).unwrap_err().contains("outside"));
    assert!(r.check_write(&s(&root.join("link/new-notebook"))).unwrap_err().contains("outside"));
    assert!(r.check_write(&s(&root.join("dangling"))).unwrap_err().contains("leads nowhere"));
    // A link that stays inside is fine.
    std::fs::create_dir_all(root.join("real")).unwrap();
    std::os::unix::fs::symlink(root.join("real"), root.join("inner")).unwrap();
    assert_eq!(r.check_write(&s(&root.join("inner/x"))).unwrap(), root.join("real/x"));
}

#[test]
fn roots_must_exist_and_be_folders() {
    let base = tmp("roots-missing");
    assert!(Roots::new(Some(&base.join("nope")), None).unwrap_err().contains("--automation-read-root"));
    std::fs::write(base.join("file"), b"x").unwrap();
    assert!(Roots::new(None, Some(&base.join("file"))).unwrap_err().contains("not a folder"));
    let r = Roots::new(Some(&base), None).unwrap();
    assert_eq!(r.read_root(), Some(base.as_path()));
    assert_eq!(r.write_root(), None);
}

#[test]
fn a_missing_root_grants_no_access() {
    let base = tmp("roots-authority");
    let read_only = Roots::new(Some(&base), None).unwrap();
    assert!(read_only.check_read(&s(&base)).is_ok());
    assert!(read_only.check_write(&s(&base.join("x"))).unwrap_err().contains("write authority is absent"));
    let none = Roots::default();
    assert!(none.check_read(&s(&base)).unwrap_err().contains("read authority is absent"));
}

#[test]
fn command_paths_are_checked_by_access() {
    let base = tmp("roots-commands");
    let r_only = Roots::new(Some(&base), None).unwrap();
    let w_only = Roots::new(None, Some(&base)).unwrap();
    let nb = s(&base.join("Work"));
    // Opening needs both (the notebook is saved in place).
    assert!(r_only.check_command("file.openNotebook", &json!({"path": nb})).is_err());
    assert!(w_only.check_command("file.openNotebook", &json!({"path": nb})).is_err());
    // Creating and exporting need write.
    assert!(w_only.check_command("file.newNotebook", &json!({"name": "Work", "path": nb})).is_ok());
    assert!(r_only.check_command("file.newNotebook", &json!({"name": "Work", "path": nb})).is_err());
    assert!(w_only.check_command("file.exportNotebook", &json!({"path": s(&base.join("w.zip"))})).is_ok());
    assert!(w_only.check_command("file.backupNow", &json!({"folder": nb})).is_ok());
    assert!(r_only.check_command("file.openBackups", &json!({"folder": nb})).is_ok());
    // A path key on a command the table doesn't know is refused.
    let e = w_only.check_command("insert.picture", &json!({"file": nb})).unwrap_err();
    assert!(e.contains("don't cover"), "{e}");
    // Commands without paths pass, as do non-object params.
    assert!(Roots::default().check_command("format.bold", &json!({})).is_ok());
    assert!(Roots::default().check_command("file.newNotebook", &json!({"name": "In memory"})).is_ok());
    assert!(Roots::default().check_command("format.bold", &Value::Null).is_ok());
}

// ---------------------------------------------------------------------------------------------
// Key file

#[test]
fn key_files_and_lookup() {
    assert_eq!(keyfile::parse_instance_id("[Instance]\ninstance-id=1234_ab-c\n").as_deref(), Some("1234_ab-c"));
    assert_eq!(keyfile::parse_instance_id("instance-id=../evil"), None);
    assert_eq!(keyfile::parse_instance_id("instance-id="), None);
    assert_eq!(keyfile::parse_instance_id("nothing"), None);
    let dir = tmp("keyfile");
    assert_eq!(keyfile::key_path_for(&dir, None, 7990), dir.join("control-key.local-7990"));
    assert_eq!(keyfile::key_path_for(&dir, Some("42"), 7990), dir.join("control-key.42"));
    // The environment wins; blank values don't count.
    assert_eq!(keyfile::lookup_key(Some(" envkey \n".into()), Some(&dir), Some(7990)).unwrap(), "envkey");
    let e = keyfile::lookup_key(Some("  ".into()), Some(&dir), Some(7990)).unwrap_err();
    assert!(e.contains("--control 7990"), "{e}");
    assert!(keyfile::lookup_key(None, None, Some(7990)).unwrap_err().contains("NOTECRAFT_CONTROL_KEY"));
    std::fs::write(keyfile::key_path(&dir, 7990), "filekey\n").unwrap();
    assert_eq!(keyfile::lookup_key(None, Some(&dir), Some(7990)).unwrap(), "filekey");
}

// ---------------------------------------------------------------------------------------------
// Control channel

#[test]
fn keys_match_needs_an_exact_non_empty_key() {
    assert!(control::keys_match(KEY, KEY));
    assert!(!control::keys_match("", ""));
    assert!(!control::keys_match(&KEY[1..], KEY));
    assert!(!control::keys_match(&KEY.replace('0', "1"), KEY));
}

/// A control server on a free port whose requests are answered by `answer` on a thread.
fn server_with(key: &str, first: Duration, mut answer: impl FnMut(&str, Value) -> Result<Value, String> + Send + 'static) -> SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let rx = control::serve_on(l, key.to_string(), Arc::new(|| {}), first);
    std::thread::spawn(move || {
        for req in rx {
            let (m, p) = (req.method.clone(), req.params.clone());
            req.answer(answer(&m, p));
        }
    });
    addr
}

/// A server that echoes each request's method and params.
fn echo_server(key: &str) -> SocketAddr {
    server_with(key, control::FIRST_REQUEST, |m, p| Ok(json!({"method": m, "params": p})))
}

struct Client {
    r: BufReader<TcpStream>,
    w: TcpStream,
}

impl Client {
    fn new(addr: SocketAddr) -> Client {
        let w = TcpStream::connect(addr).unwrap();
        w.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        Client { r: BufReader::new(w.try_clone().unwrap()), w }
    }

    fn send(&mut self, line: &str) -> Value {
        writeln!(self.w, "{line}").unwrap();
        self.read()
    }

    fn read(&mut self) -> Value {
        let mut l = String::new();
        self.r.read_line(&mut l).unwrap();
        serde_json::from_str(&l).unwrap()
    }

    /// True when the server has closed the connection.
    fn closed(&mut self) -> bool {
        let mut b = [0u8; 1];
        matches!(self.r.read(&mut b), Ok(0) | Err(_))
    }
}

#[test]
fn control_refuses_requests_without_the_key_and_closes() {
    let addr = echo_server(KEY);
    let mut c = Client::new(addr);
    let v = c.send(r#"{"id": 1, "method": "engine.commands"}"#);
    assert_eq!(v, json!({"ok": false, "error": "unauthorized", "id": 1}));
    assert!(c.closed());
    // A wrong key too.
    let mut c = Client::new(addr);
    let v = c.send(&json!({"id": 2, "method": "x", "key": "nope"}).to_string());
    assert_eq!(v["error"], "unauthorized");
    assert!(c.closed());
}

#[test]
fn control_accepts_the_key_on_every_request() {
    let addr = echo_server(KEY);
    let mut c = Client::new(addr);
    for i in 0..3 {
        let v = c.send(&json!({"id": i, "method": "page.text", "params": {"n": i}, "key": KEY}).to_string());
        assert_eq!(v, json!({"ok": true, "result": {"method": "page.text", "params": {"n": i}}, "id": i}));
    }
    // A request without the key on the same connection is still refused: only `auth` makes a
    // connection remember its key.
    assert_eq!(c.send(r#"{"id": 9, "method": "x"}"#)["error"], "unauthorized");
}

#[test]
fn control_auth_once_per_connection() {
    let addr = echo_server(KEY);
    let mut c = Client::new(addr);
    let v = c.send(&json!({"id": "a", "method": "auth", "params": {"key": KEY}}).to_string());
    assert_eq!(v, json!({"ok": true, "result": {}, "id": "a"}));
    let v = c.send(r#"{"id": 2, "method": "ui.key", "params": {"key": "Enter"}}"#);
    assert_eq!(v["result"]["params"]["key"], "Enter");
    // A new connection starts unauthorised.
    let mut c2 = Client::new(addr);
    assert_eq!(c2.send(r#"{"id": 3, "method": "x"}"#)["error"], "unauthorized");
}

#[test]
fn control_params_key_is_not_the_control_key() {
    // `ui.key {"key": …}` names a key to press; it never authorises.
    let addr = echo_server(KEY);
    let mut c = Client::new(addr);
    let v = c.send(&json!({"id": 1, "method": "ui.key", "params": {"key": KEY}}).to_string());
    assert_eq!(v["error"], "unauthorized");
}

#[test]
fn control_with_no_key_refuses_everything() {
    // The app could not write its key file: the server runs with an empty key.
    let addr = echo_server("");
    let mut c = Client::new(addr);
    assert_eq!(c.send(r#"{"id": 1, "method": "auth", "params": {"key": ""}, "key": ""}"#)["error"], "unauthorized");
}

#[test]
fn control_closes_on_bad_json() {
    let addr = echo_server(KEY);
    let mut c = Client::new(addr);
    c.send(&json!({"id": 1, "method": "auth", "params": {"key": KEY}}).to_string());
    let v = c.send("GET / HTTP/1.1");
    assert!(v["error"].as_str().unwrap().starts_with("bad JSON"), "{v}");
    assert!(c.closed());
}

#[test]
fn control_closes_on_a_line_too_long() {
    let addr = echo_server(KEY);
    let mut c = Client::new(addr);
    let mut line = vec![b'a'; control::MAX_LINE + 10];
    line.push(b'\n');
    c.w.write_all(&line).unwrap();
    assert_eq!(c.read()["error"], "line too long");
    assert!(c.closed());
}

#[test]
fn control_closes_silent_connections() {
    let addr = server_with(KEY, Duration::from_millis(200), |_, _| Ok(json!({})));
    let mut c = Client::new(addr);
    // Blank lines don't extend the deadline.
    writeln!(c.w).unwrap();
    #[allow(clippy::disallowed_methods)] // native-only test
    let t = std::time::Instant::now();
    assert!(c.closed());
    assert!(t.elapsed() < Duration::from_secs(5));
}

#[test]
fn control_reports_app_errors() {
    let addr = server_with(KEY, control::FIRST_REQUEST, |m, _| Err(format!("{m} failed")));
    let mut c = Client::new(addr);
    let v = c.send(&json!({"id": 1, "method": "notebook.save", "key": KEY}).to_string());
    assert_eq!(v, json!({"ok": false, "error": "notebook.save failed", "id": 1}));
}

#[test]
fn request_answer_shapes() {
    let (req, rx) = Request::new("m", json!({}));
    req.answer(Ok(json!(1)));
    assert_eq!(rx.recv().unwrap(), json!({"ok": true, "result": 1}));
}

// ---------------------------------------------------------------------------------------------
// Remote

#[test]
fn loopback_addresses() {
    for a in ["127.0.0.1:7990", "127.4.5.6:1", "[::1]:7990", "localhost:7990", "LocalHost:1", "[::ffff:127.0.0.1]:1", "::1"] {
        assert!(remote::is_loopback_addr(a), "{a}");
    }
    for a in ["10.0.0.1:7990", "example.com:7990", "0.0.0.0:7990", "[::]:1", "localhost.evil.com:1"] {
        assert!(!remote::is_loopback_addr(a), "{a}");
    }
}

/// A control server answered by a headless session, as the desktop app answers on its UI thread.
fn headless_server(key: &str) -> SocketAddr {
    let mut h = headless();
    server_with(key, control::FIRST_REQUEST, move |m, p| h.call(m, p))
}

#[test]
fn remote_round_trip_through_the_control_channel() {
    let addr = headless_server(KEY);
    let mut r = Remote::connect_with(&addr.to_string(), Box::new(|_| Ok(KEY.to_string()))).unwrap();
    assert!(r.key_warning().is_none());
    assert!(r.has_ui());
    let v = r.call("engine.execute", json!({"command": "page.new", "params": {"title": "Over the wire"}})).unwrap();
    assert!(v.get("page").is_some(), "{v}");
    let t = r.call("page.text", json!({})).unwrap();
    assert!(t["text"].as_str().unwrap().starts_with("# Over the wire"), "{t}");
    assert!(r.call("no.such.command", json!({})).unwrap_err().contains("no.such.command"));
    // And as an MCP server.
    let mut srv = Server::new(Box::new(r));
    let tools = srv.handle(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})).unwrap();
    assert!(tool_names(&tools).contains(&"screenshot".to_string()));
    let v = srv.handle(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "get_page_text", "arguments": {}}})).unwrap();
    assert!(text_of(&v).starts_with("# Over the wire"));
}

#[test]
fn remote_rereads_a_stale_key_once() {
    let addr = headless_server(KEY);
    let lookups = Arc::new(AtomicUsize::new(0));
    let n = lookups.clone();
    let finder = Box::new(move |_: &str| Ok(if n.fetch_add(1, Ordering::SeqCst) == 0 { "stale".to_string() } else { KEY.to_string() }));
    let mut r = Remote::connect_with(&addr.to_string(), finder).unwrap();
    assert!(r.call("engine.parity", json!({})).unwrap()["rows"].as_u64().unwrap() > 0);
    assert_eq!(lookups.load(Ordering::SeqCst), 2);
}

#[test]
fn remote_reports_a_refused_key() {
    let addr = headless_server(KEY);
    let mut r = Remote::connect_with(&addr.to_string(), Box::new(|_| Ok("wrong".to_string()))).unwrap();
    let e = r.call("engine.parity", json!({})).unwrap_err();
    assert!(e.starts_with("unauthorized"), "{e}");
}

#[test]
fn remote_fails_fast_when_nothing_listens() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    drop(l);
    assert!(Remote::connect_with(&addr, Box::new(|_| Ok(KEY.to_string()))).is_err());
}

// ---------------------------------------------------------------------------------------------
// MCP

fn rpc(srv: &mut Server, id: i64, method: &str, params: Value) -> Value {
    srv.handle(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).unwrap()
}

fn call(srv: &mut Server, name: &str, args: Value) -> Value {
    rpc(srv, 1, "tools/call", json!({"name": name, "arguments": args}))["result"].clone()
}

fn text_of(v: &Value) -> String {
    let r = if v.get("result").is_some() { &v["result"] } else { v };
    r["content"][0]["text"].as_str().unwrap_or("").to_string()
}

fn tool_names(v: &Value) -> Vec<String> {
    v["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect()
}

fn mcp_server() -> Server {
    Server::new(Box::new(headless()))
}

#[test]
fn mcp_initialize_and_ping() {
    let mut srv = mcp_server();
    let v = rpc(&mut srv, 1, "initialize", json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}));
    assert_eq!(v["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(v["result"]["serverInfo"]["name"], "notecraft");
    assert!(v["result"]["instructions"].as_str().unwrap().contains("headless"));
    let v = rpc(&mut srv, 2, "initialize", json!({"protocolVersion": "1999-01-01"}));
    assert_eq!(v["result"]["protocolVersion"], mcp::PROTOCOL_VERSION);
    assert!(!srv.is_initialized());
    assert_eq!(srv.handle(json!({"jsonrpc": "2.0", "method": "notifications/initialized"})), None);
    assert!(srv.is_initialized());
    assert_eq!(rpc(&mut srv, 3, "ping", Value::Null), json!({"jsonrpc": "2.0", "id": 3, "result": {}}));
}

#[test]
fn mcp_protocol_errors() {
    let mut srv = mcp_server();
    assert_eq!(rpc(&mut srv, 1, "nope", json!({}))["error"]["code"], -32601);
    let v: Value = serde_json::from_str(&srv.handle_line("{not json").unwrap()).unwrap();
    assert_eq!(v["error"]["code"], -32700);
    assert_eq!(srv.handle(json!({"jsonrpc": "2.0", "id": {"x": 1}, "method": "ping"})).unwrap()["error"]["code"], -32600);
    assert_eq!(srv.handle(json!({"jsonrpc": "2.0", "id": 1})).unwrap()["error"]["code"], -32600);
    assert_eq!(srv.handle(json!([1, 2])).unwrap()["error"]["code"], -32600);
    // A response from the client needs no reply.
    assert_eq!(srv.handle(json!({"jsonrpc": "2.0", "id": 1, "result": {}})), None);
    assert_eq!(srv.handle_line("   "), None);
    assert_eq!(rpc(&mut srv, 1, "tools/call", json!({}))["error"]["code"], -32602);
    // Old-style batches still get one reply per request.
    let v: Value = serde_json::from_str(
        &srv.handle_line(r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/initialized"}]"#).unwrap(),
    )
    .unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    let v: Value = serde_json::from_str(&srv.handle_line("[]").unwrap()).unwrap();
    assert_eq!(v["error"]["code"], -32600);
}

#[test]
fn mcp_tools_list_shows_window_tools_only_with_a_window() {
    struct WithUi;
    impl Backend for WithUi {
        fn call(&mut self, method: &str, _: Value) -> Result<Value, String> {
            Err(method.to_string())
        }
        fn has_ui(&self) -> bool {
            true
        }
        fn describe(&self) -> String {
            "fake".into()
        }
    }
    let headless_tools = tool_names(&rpc(&mut mcp_server(), 1, "tools/list", json!({})));
    let ui_tools = tool_names(&rpc(&mut Server::new(Box::new(WithUi)), 1, "tools/list", json!({})));
    for t in [
        "list_commands",
        "execute",
        "batch",
        "new_notebook",
        "open_notebook",
        "new_page",
        "go_to_page",
        "type_text",
        "inspect",
        "get_page_text",
        "search",
        "find_tags",
        "save",
    ] {
        assert!(headless_tools.contains(&t.to_string()), "{t}");
    }
    for t in ["screenshot", "press_key", "type_keys", "click"] {
        assert!(!headless_tools.contains(&t.to_string()), "{t}");
        assert!(ui_tools.contains(&t.to_string()), "{t}");
    }
    // Every tool has a schema.
    for t in rpc(&mut mcp_server(), 1, "tools/list", json!({}))["result"]["tools"].as_array().unwrap() {
        assert_eq!(t["inputSchema"]["type"], "object", "{t}");
    }
}

#[test]
fn mcp_meeting_notes_workflow() {
    let mut srv = mcp_server();
    let v = call(&mut srv, "new_page", json!({"title": "Meeting"}));
    assert_eq!(v["isError"], false, "{v}");
    let v = call(&mut srv, "type_text", json!({"text": "\nAgenda\nBudget"}));
    assert_eq!(v["isError"], false, "{v}");
    let v = call(&mut srv, "execute", json!({"command": "tag.todo"}));
    assert_eq!(v["isError"], false, "{v}");
    let text = text_of(&call(&mut srv, "get_page_text", json!({})));
    assert!(text.starts_with("# Meeting\n"), "{text}");
    assert!(text.contains("Agenda"), "{text}");
    assert!(text.contains("[To Do] Budget"), "{text}");
    let inspect: Value = serde_json::from_str(&text_of(&call(&mut srv, "inspect", json!({"runs": true})))).unwrap();
    assert_eq!(inspect["page"]["title"], "Meeting");
    let paras = inspect["page"]["objects"][0]["paragraphs"].as_array().unwrap();
    assert_eq!(paras.last().unwrap()["tags"][0]["key"], "todo");
    assert!(paras[0].get("runs").is_some());
    assert_eq!(inspect["canUndo"], true);
    // Find the tag again.
    let v = call(&mut srv, "find_tags", json!({"unchecked": true, "scope": "all"}));
    assert!(text_of(&v).contains("Budget"), "{v}");
}

#[test]
fn mcp_execute_and_list_commands() {
    let mut srv = mcp_server();
    let v = call(&mut srv, "list_commands", json!({"query": "TO DO TAG"}));
    let list: Value = serde_json::from_str(&text_of(&v)).unwrap();
    assert!(list.as_array().unwrap().iter().any(|c| c["id"] == "tag.todo"), "{list}");
    assert!(list.as_array().unwrap().iter().all(|c| c.get("enabled").is_some()));
    let v = call(&mut srv, "execute", json!({"command": "no.such.command"}));
    assert_eq!(v["isError"], true);
    assert!(text_of(&v).contains("no.such.command"));
    assert_eq!(call(&mut srv, "execute", json!({}))["isError"], true);
    assert_eq!(call(&mut srv, "nope", json!({}))["isError"], true);
    // Window tools without a window explain how to get one.
    let v = call(&mut srv, "press_key", json!({"key": "B", "ctrl": true}));
    assert_eq!(v["isError"], true);
    assert!(text_of(&v).contains(NEEDS_APP), "{v}");
}

#[test]
fn mcp_batch_stops_at_the_first_error_unless_keep_going() {
    let mut srv = mcp_server();
    let cmds = json!([{"command": "page.new"}, {"command": "bogus"}, {"command": "page.new", "params": {"title": "Third"}}]);
    let v = call(&mut srv, "batch", json!({"commands": cmds}));
    assert_eq!(v["isError"], true);
    let steps: Value = serde_json::from_str(&text_of(&v)).unwrap();
    assert_eq!(steps.as_array().unwrap().len(), 2);
    assert_eq!(steps[0]["ok"], true);
    assert_eq!(steps[1]["ok"], false);
    let v = call(&mut srv, "batch", json!({"commands": cmds, "keepGoing": true}));
    assert_eq!(v["isError"], false);
    let steps: Value = serde_json::from_str(&text_of(&v)).unwrap();
    assert_eq!(steps.as_array().unwrap().len(), 3);
    assert!(text_of(&call(&mut srv, "get_page_text", json!({}))).starts_with("# Third"));
}

#[test]
fn mcp_go_to_page_by_title_or_id() {
    let mut srv = mcp_server();
    let v = call(&mut srv, "go_to_page", json!({"title": "  retrospective "}));
    assert_eq!(v["isError"], false, "{v}");
    assert!(text_of(&call(&mut srv, "get_page_text", json!({}))).starts_with("# Retrospective"));
    let inspect: Value = serde_json::from_str(&text_of(&call(&mut srv, "inspect", json!({})))).unwrap();
    let id = inspect["page"]["id"].as_str().unwrap().to_string();
    call(&mut srv, "go_to_page", json!({"title": "Ideas"}));
    assert_eq!(call(&mut srv, "go_to_page", json!({"page": id}))["isError"], false);
    assert!(text_of(&call(&mut srv, "get_page_text", json!({}))).starts_with("# Retrospective"));
    assert_eq!(call(&mut srv, "go_to_page", json!({"title": "No such page"}))["isError"], true);
    assert_eq!(call(&mut srv, "go_to_page", json!({}))["isError"], true);
    assert_eq!(call(&mut srv, "get_page_text", json!({"page": "zz"}))["isError"], true);
    assert_eq!(call(&mut srv, "inspect", json!({"page": "zz"}))["isError"], true);
}

#[test]
fn mcp_search_and_resources() {
    let mut srv = mcp_server();
    let v = call(&mut srv, "search", json!({"query": "raised", "scope": "all"}));
    assert_eq!(v["isError"], false, "{v}");
    assert!(text_of(&v).contains("Raised beds"), "{v}");
    let v = rpc(&mut srv, 1, "resources/list", json!({}));
    assert_eq!(v["result"]["resources"].as_array().unwrap().len(), 2);
    let v = rpc(&mut srv, 2, "resources/read", json!({"uri": mcp::NOTEBOOKS_URI}));
    let tree: Value = serde_json::from_str(v["result"]["contents"][0]["text"].as_str().unwrap()).unwrap();
    assert!(!tree["notebooks"].as_array().unwrap().is_empty());
    let v = rpc(&mut srv, 3, "resources/read", json!({"uri": mcp::COMMANDS_URI}));
    assert!(v["result"]["contents"][0]["text"].as_str().unwrap().contains("tag.todo"));
    assert_eq!(rpc(&mut srv, 4, "resources/read", json!({"uri": "notecraft://nope"}))["error"]["code"], -32002);
    assert_eq!(rpc(&mut srv, 5, "resources/templates/list", json!({}))["result"]["resourceTemplates"], json!([]));
}

#[test]
fn mcp_serve_reads_lines_until_eof() {
    let mut srv = mcp_server();
    let mut input = String::new();
    input.push_str(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#);
    input.push('\n');
    input.push_str(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    input.push_str("\n\n");
    input.push_str(&"x".repeat((16 << 20) + 5));
    input.push('\n');
    input.push_str(r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#);
    let mut out = Vec::new();
    srv.serve(std::io::Cursor::new(input), &mut out).unwrap();
    let lines: Vec<Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(lines[0]["id"], 1);
    assert_eq!(lines[1]["error"]["message"], "line too long");
    assert_eq!(lines[2], json!({"jsonrpc": "2.0", "id": 2, "result": {}}));
}

// ---------------------------------------------------------------------------------------------
// Headless

#[test]
fn headless_engine_queries() {
    let mut h = headless();
    let cmds = h.call("engine.commands", json!({})).unwrap();
    let todo = cmds.as_array().unwrap().iter().find(|c| c["id"] == "tag.todo").unwrap();
    assert_eq!(todo["menu"], "Home/Tags/To Do Tag");
    // Enabled or not, with the reason when not.
    assert_eq!(todo["enabled"].as_bool().unwrap(), todo["disabledReason"].is_null(), "{todo}");
    assert!(cmds.as_array().unwrap().iter().any(|c| c["enabled"] == false && c["disabledReason"].is_string()));
    let p = h.call("engine.parity", json!({})).unwrap();
    assert!(p["wired"].as_u64().unwrap() >= p["verified"].as_u64().unwrap());
    assert!(h.call("ui.screenshot", json!({})).unwrap_err().contains(NEEDS_APP));
    assert!(h.call("engine.execute", json!({})).unwrap_err().contains("missing `command`"));
    assert_eq!(h.call("notebook.save", json!({})).unwrap(), json!({"saved": true}));
    // A method that is a command id runs it.
    assert!(h.call("page.new", json!({"title": "Direct"})).unwrap().get("page").is_some());
    assert!(h.call("page.text", json!({})).unwrap()["text"].as_str().unwrap().starts_with("# Direct"));
}

#[test]
fn headless_saves_folder_notebooks_after_each_command() {
    let base = tmp("headless-save");
    let nb = base.join("Work");
    let mut h = headless().with_roots(Some(Roots::new(Some(&base), Some(&base)).unwrap()));
    let v = h.execute("file.newNotebook", json!({"name": "Work", "path": s(&nb)})).unwrap();
    assert!(v.get("notebook").is_some(), "{v}");
    h.execute("page.new", json!({"title": "Saved page"})).unwrap();
    h.execute("text.insert", json!({"text": "\nBody line"})).unwrap();
    assert!(!h.session.is_dirty(), "headless saves after every command");
    // A fresh session reads it back from disk.
    let mut fresh = Session::new(clock());
    fresh.open_path(&s(&nb)).unwrap();
    let titles: Vec<String> =
        fresh.ws.notebooks.iter().flat_map(|n| n.sections()).flat_map(|sec| sec.pages.iter().map(|p| p.display_title())).collect();
    assert!(titles.iter().any(|t| t == "Saved page"), "{titles:?}");
    // Paths outside the roots are refused before the command runs.
    let outside = tmp("headless-outside");
    let e = h.execute("file.newNotebook", json!({"name": "Escape", "path": s(&outside.join("Escape"))})).unwrap_err();
    assert!(e.contains("outside"), "{e}");
    assert!(!outside.join("Escape").exists());
    let e = h.call("file.openNotebook", json!({"path": s(&outside)})).unwrap_err();
    assert!(e.contains("outside the read root"), "{e}");
}
