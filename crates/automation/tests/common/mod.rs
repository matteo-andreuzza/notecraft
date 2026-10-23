//! What the acceptance workflows share: an agent that calls NoteCraft's MCP tools over a
//! headless session, with a fixed clock.

use notecraft_automation::Headless;
use notecraft_automation::mcp::Server;
use notecraft_engine::{Clock, Session};
use serde_json::{Value, json};

pub fn clock() -> Clock {
    Clock { now_ms: 1_792_007_100_000, utc_offset_min: 60 }
}

pub struct Agent {
    srv: Server,
    next: i64,
}

impl Agent {
    pub fn new() -> Agent {
        Agent { srv: Server::new(Box::new(Headless::new(Session::new(clock()), Box::new(clock)))), next: 1 }
    }

    /// Call a tool; panics with the tool's error text when it fails.
    pub fn tool(&mut self, name: &str, args: Value) -> Value {
        self.next += 1;
        let v =
            self.srv.handle(json!({"jsonrpc": "2.0", "id": self.next, "method": "tools/call", "params": {"name": name, "arguments": args}})).unwrap();
        let r = &v["result"];
        let text = r["content"][0]["text"].as_str().unwrap_or("").to_string();
        assert_eq!(r["isError"], false, "{name} {args}: {text}");
        serde_json::from_str(&text).unwrap_or(Value::String(text))
    }

    pub fn exec(&mut self, command: &str, params: Value) -> Value {
        self.tool("execute", json!({"command": command, "params": params}))
    }

    pub fn page_text(&mut self) -> String {
        self.tool("get_page_text", json!({})).as_str().unwrap_or("").to_string()
    }

    pub fn inspect(&mut self) -> Value {
        self.tool("inspect", json!({}))
    }
}

/// A fresh folder for a workflow's notebook (removed first if a previous run left it).
pub fn notebook_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("notecraft-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A fresh session that opens the notebook at `dir`: the session and the id of its page titled
/// `title`.
pub fn reopen(dir: &std::path::Path, title: &str) -> (Session, String) {
    let mut fresh = Session::new(clock());
    fresh.open_path(&dir.display().to_string()).unwrap();
    let id = fresh.ws.notebooks[0]
        .sections()
        .iter()
        .flat_map(|s| s.pages.iter())
        .find(|p| p.display_title() == title)
        .map(|p| p.id.to_string())
        .unwrap_or_else(|| panic!("no page {title:?} on disk"));
    (fresh, id)
}
