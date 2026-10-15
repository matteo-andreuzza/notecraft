//! A Model Context Protocol server: JSON-RPC 2.0, one message per line over stdio, on a
//! [`Backend`]. See `docs/mcp.md`.

use std::io::{BufRead, Read, Write};

use serde_json::{Value, json};

use crate::Backend;

/// The MCP revision implemented.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED_VERSIONS: &[&str] = &[PROTOCOL_VERSION, "2025-03-26", "2024-11-05"];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;
const RESOURCE_NOT_FOUND: i64 = -32002;

/// Longer input lines are answered with a parse error instead of being parsed.
const MAX_LINE: usize = 16 << 20;

const INSTRUCTIONS: &str = "NoteCraft is a note-taking app (a OneNote-style clone): notebooks hold section groups and \
sections, sections hold pages, pages hold note containers (outlines) of paragraphs with tags. Every action is a \
command: find ids and parameters with list_commands and run them with execute (or several with batch). Typical \
flow: new_notebook {name:\"Work\", path:\"/tmp/Work\"} → new_page {title:\"Meeting\"} → execute caret.down (into \
the body) → type_text \"Agenda\\nBudget\" → execute tag.todo → inspect to verify → get_page_text. Folder notebooks \
are saved automatically.";

/// Resource URIs.
pub const NOTEBOOKS_URI: &str = "notecraft://notebooks";
/// The command registry resource.
pub const COMMANDS_URI: &str = "notecraft://commands";

/// An MCP server bound to one backend.
pub struct Server {
    backend: Box<dyn Backend>,
    initialized: bool,
}

fn response(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message.into()}})
}

impl Server {
    /// A server on `backend`.
    pub fn new(backend: Box<dyn Backend>) -> Server {
        Server { backend, initialized: false }
    }

    /// The backend.
    pub fn backend(&mut self) -> &mut dyn Backend {
        self.backend.as_mut()
    }

    /// Whether the client has sent `notifications/initialized`.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Serve newline-delimited JSON-RPC until `input` closes. Logs go to stderr only (stdout
    /// is the protocol stream).
    pub fn serve(&mut self, mut input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        loop {
            let mut buf = Vec::new();
            let n = Read::take(&mut input, MAX_LINE as u64 + 1).read_until(b'\n', &mut buf)?;
            if n == 0 {
                return Ok(());
            }
            let reply = if buf.len() > MAX_LINE {
                // Skip the rest of the oversize line.
                let mut sink = Vec::new();
                input.read_until(b'\n', &mut sink)?;
                Some(error(Value::Null, PARSE_ERROR, "line too long").to_string())
            } else {
                self.handle_line(&String::from_utf8_lossy(&buf))
            };
            if let Some(reply) = reply {
                output.write_all(reply.as_bytes())?;
                output.write_all(b"\n")?;
                output.flush()?;
            }
        }
    }

    /// Handle one line; the reply line (`None` for notifications and blank lines).
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(batch)) => {
                // Batches were removed in 2025-06-18; still answer older clients sensibly.
                if batch.is_empty() {
                    Some(error(Value::Null, INVALID_REQUEST, "empty batch"))
                } else {
                    let replies: Vec<Value> = batch.into_iter().filter_map(|m| self.handle(m)).collect();
                    (!replies.is_empty()).then_some(Value::Array(replies))
                }
            }
            Ok(msg) => self.handle(msg),
            Err(e) => Some(error(Value::Null, PARSE_ERROR, format!("parse error: {e}"))),
        };
        reply.map(|r| r.to_string())
    }

    /// Handle one JSON-RPC message; `None` for notifications and responses.
    pub fn handle(&mut self, msg: Value) -> Option<Value> {
        let Value::Object(o) = &msg else { return Some(error(Value::Null, INVALID_REQUEST, "message must be an object")) };
        let id = o.get("id").cloned();
        let Some(method) = o.get("method").and_then(Value::as_str) else {
            if o.contains_key("result") || o.contains_key("error") {
                return None;
            }
            return Some(error(id.unwrap_or(Value::Null), INVALID_REQUEST, "missing `method`"));
        };
        let params = o.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = id else {
            if method == "notifications/initialized" {
                self.initialized = true;
            }
            return None;
        };
        if !(id.is_string() || id.is_number()) {
            return Some(error(Value::Null, INVALID_REQUEST, "`id` must be a string or number"));
        }
        Some(match self.request(method, &params) {
            Ok(r) => response(id, r),
            Err((code, m)) => error(id, code, m),
        })
    }

    fn request(&mut self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION);
                let version = if SUPPORTED_VERSIONS.contains(&asked) { asked } else { PROTOCOL_VERSION };
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {}, "resources": {}},
                    "serverInfo": {"name": "notecraft", "title": "NoteCraft", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": format!("{INSTRUCTIONS} Backend: {}.", self.backend.describe()),
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tool_definitions(self.backend.has_ui())})),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).ok_or((INVALID_PARAMS, "missing tool `name`".to_string()))?;
                let args = params.get("arguments").cloned().unwrap_or(Value::Null);
                Ok(call_tool(self.backend.as_mut(), name, &args).to_value())
            }
            "resources/list" => Ok(json!({"resources": [
                {"uri": NOTEBOOKS_URI, "name": "notebooks", "title": "Notebooks", "description": "Open notebooks, sections and pages, the selection and the current page's content (notebook.inspect)", "mimeType": "application/json"},
                {"uri": COMMANDS_URI, "name": "commands", "title": "Commands", "description": "Every command: id, label, ribbon location, shortcut, params, enabled (engine.commands)", "mimeType": "application/json"},
            ]})),
            "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
            "resources/read" => {
                let uri = params.get("uri").and_then(Value::as_str).ok_or((INVALID_PARAMS, "missing `uri`".to_string()))?;
                let v = match uri {
                    NOTEBOOKS_URI => self.backend.call("notebook.inspect", json!({})),
                    COMMANDS_URI => self.backend.call("engine.commands", json!({})),
                    _ => return Err((RESOURCE_NOT_FOUND, format!("resource not found: {uri}"))),
                }
                .map_err(|e| (INTERNAL_ERROR, e))?;
                Ok(json!({"contents": [{"uri": uri, "mimeType": "application/json", "text": serde_json::to_string_pretty(&v).unwrap_or_default()}]}))
            }
            other => Err((METHOD_NOT_FOUND, format!("method not found: {other}"))),
        }
    }
}

/// A tool call's result.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolResult {
    /// Content blocks.
    pub content: Vec<Value>,
    /// Whether the call failed.
    pub is_error: bool,
}

impl ToolResult {
    /// A text result.
    pub fn text(t: impl Into<String>) -> ToolResult {
        ToolResult { content: vec![json!({"type": "text", "text": t.into()})], is_error: false }
    }

    /// A JSON value, pretty-printed.
    pub fn json(v: &Value) -> ToolResult {
        ToolResult::text(serde_json::to_string_pretty(v).unwrap_or_default())
    }

    /// An error the agent can read and correct.
    pub fn error(t: impl Into<String>) -> ToolResult {
        ToolResult { content: vec![json!({"type": "text", "text": t.into()})], is_error: true }
    }

    /// A PNG image (base64) with a caption.
    pub fn image(png_base64: String, info: &Value) -> ToolResult {
        ToolResult {
            content: vec![json!({"type": "image", "data": png_base64, "mimeType": "image/png"}), json!({"type": "text", "text": info.to_string()})],
            is_error: false,
        }
    }

    /// The `tools/call` result object.
    pub fn to_value(&self) -> Value {
        json!({"content": self.content, "isError": self.is_error})
    }
}

fn s(desc: &str) -> Value {
    json!({"type": "string", "description": desc})
}
fn n(desc: &str) -> Value {
    json!({"type": "number", "description": desc})
}
fn b(desc: &str) -> Value {
    json!({"type": "boolean", "description": desc})
}
fn obj(props: Value, req: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": req})
}
fn tool(name: &str, title: &str, desc: &str, schema: Value, read_only: bool) -> Value {
    json!({"name": name, "title": title, "description": desc, "inputSchema": schema, "annotations": {"title": title, "readOnlyHint": read_only, "openWorldHint": false}})
}

/// The tools; the window tools only when a window is attached.
pub fn tool_definitions(has_ui: bool) -> Value {
    let mut tools = vec![
        tool(
            "list_commands",
            "List commands",
            "Every NoteCraft command: id, label, ribbon location, shortcut, params and whether it is enabled now. Filter with `query`.",
            obj(json!({"query": s("Case-insensitive filter on id/label/location")}), &[]),
            true,
        ),
        tool(
            "execute",
            "Run a command",
            "Run one command by id, e.g. {command:\"format.bold\"} or {command:\"tag.toggle\", params:{key:\"todo\"}}. One command is one undo step.",
            obj(json!({"command": s("Command id"), "params": {"type": "object", "description": "Command parameters"}}), &["command"]),
            false,
        ),
        tool(
            "batch",
            "Run several commands",
            "Run commands in order; stops at the first error unless keepGoing.",
            obj(
                json!({"commands": {"type": "array", "items": {"type": "object", "properties": {"command": {"type": "string"}, "params": {"type": "object"}}, "required": ["command"]}}, "keepGoing": b("Run the rest after an error")}),
                &["commands"],
            ),
            false,
        ),
        tool(
            "new_notebook",
            "New notebook",
            "Create a notebook: in memory, or as a folder at `path` (saved automatically).",
            obj(json!({"name": s("Notebook name"), "path": s("Folder to create (optional)")}), &["name"]),
            false,
        ),
        tool("open_notebook", "Open notebook", "Open a .ncraft notebook folder.", obj(json!({"path": s("Notebook folder")}), &["path"]), false),
        tool(
            "new_page",
            "New page",
            "Add a page at the end of the current section, optionally with a title; the caret ends in the title.",
            obj(json!({"title": s("Page title")}), &[]),
            false,
        ),
        tool(
            "go_to_page",
            "Go to page",
            "Open a page by id or by title (first case-insensitive match).",
            obj(json!({"page": s("Page id"), "title": s("Page title")}), &[]),
            false,
        ),
        tool(
            "type_text",
            "Type text",
            "Type at the caret, replacing the selection. A newline starts a new paragraph (in the title it moves into the page body).",
            obj(json!({"text": s("Text")}), &["text"]),
            false,
        ),
        tool(
            "inspect",
            "Inspect",
            "Open notebooks with their section groups, sections and pages; the selection; and a page's note containers with paragraphs, levels, styles, list labels and tags.",
            obj(json!({"page": s("Page id (default: current)"), "runs": b("Include formatted runs")}), &[]),
            true,
        ),
        tool(
            "get_page_text",
            "Page text",
            "A page as plain text: title, then paragraphs indented by level with tags and list labels.",
            obj(json!({"page": s("Page id (default: current)")}), &[]),
            true,
        ),
        tool(
            "search",
            "Search",
            "Search notes (prefix words, \"quoted phrases\") in a scope.",
            obj(json!({"query": s("Query"), "scope": s("page|section|sectionGroup|notebook|all")}), &["query"]),
            true,
        ),
        tool(
            "find_tags",
            "Find tags",
            "Tagged notes, grouped (tag|section|title|date|text), optionally only unchecked To Do items.",
            obj(json!({"group": s("Grouping"), "unchecked": b("Only unchecked"), "scope": s("page|section|sectionGroup|notebook|all")}), &[]),
            true,
        ),
        tool("save", "Save", "Save changed folder notebooks now (they are also saved automatically).", obj(json!({}), &[]), false),
    ];
    if has_ui {
        tools.extend([
            tool(
                "screenshot",
                "Screenshot",
                "A PNG of the app window.",
                obj(json!({"path": s("Write the PNG here instead of returning it")}), &[]),
                true,
            ),
            tool(
                "press_key",
                "Press key",
                "Press a key through the window (shortcuts apply), e.g. {key:\"B\", ctrl:true}.",
                obj(
                    json!({"key": s("Key name: A–Z, 0–9, Enter, Tab, Escape, Left, F3…"), "shift": b("Shift"), "ctrl": b("Ctrl"), "alt": b("Alt")}),
                    &["key"],
                ),
                false,
            ),
            tool("type_keys", "Type keys", "Type text through the window, as the keyboard would.", obj(json!({"text": s("Text")}), &["text"]), false),
            tool(
                "click",
                "Click",
                "Click at window coordinates (points).",
                obj(json!({"x": n("x"), "y": n("y"), "button": s("primary|secondary"), "count": n("1 or 2")}), &["x", "y"]),
                false,
            ),
        ]);
    }
    Value::Array(tools)
}

fn exec(b: &mut dyn Backend, command: &str, params: Value) -> Result<Value, String> {
    b.call("engine.execute", json!({"command": command, "params": params}))
}

fn wrap(r: Result<Value, String>) -> ToolResult {
    match r {
        Ok(v) => ToolResult::json(&v),
        Err(e) => ToolResult::error(e),
    }
}

/// The page whose title matches `title` (case-insensitive), from an inspect tree.
fn find_page_by_title(tree: &Value, title: &str) -> Option<String> {
    fn walk(nodes: &Value, title: &str) -> Option<String> {
        for n in nodes.as_array()? {
            if let Some(pages) = n.get("pages").and_then(Value::as_array) {
                for p in pages {
                    if p.get("title").and_then(Value::as_str).is_some_and(|t| t.trim().eq_ignore_ascii_case(title.trim())) {
                        return p.get("id").and_then(Value::as_str).map(str::to_string);
                    }
                }
            }
            if let Some(found) = n.get("children").and_then(|c| walk(c, title)) {
                return Some(found);
            }
        }
        None
    }
    tree.get("notebooks")?.as_array()?.iter().find_map(|nb| walk(nb.get("children")?, title))
}

/// Run one tool.
pub fn call_tool(b: &mut dyn Backend, name: &str, a: &Value) -> ToolResult {
    let st = |k: &str| a.get(k).and_then(Value::as_str);
    match name {
        "list_commands" => {
            let q = st("query").unwrap_or("").to_lowercase();
            match b.call("engine.commands", json!({})) {
                Ok(Value::Array(v)) => {
                    let f: Vec<Value> = v
                        .into_iter()
                        .filter(|c| {
                            q.is_empty()
                                || ["id", "label", "menu"]
                                    .iter()
                                    .any(|k| c.get(*k).and_then(Value::as_str).is_some_and(|x| x.to_lowercase().contains(&q)))
                        })
                        .collect();
                    ToolResult::json(&Value::Array(f))
                }
                Ok(v) => ToolResult::json(&v),
                Err(e) => ToolResult::error(e),
            }
        }
        "execute" => match st("command") {
            Some(c) => wrap(exec(b, c, a.get("params").cloned().filter(|v| !v.is_null()).unwrap_or(json!({})))),
            None => ToolResult::error("missing `command`"),
        },
        "batch" => {
            let keep = a.get("keepGoing").and_then(Value::as_bool).unwrap_or(false);
            let mut out = Vec::new();
            for c in a.get("commands").and_then(Value::as_array).cloned().unwrap_or_default() {
                let id = c.get("command").and_then(Value::as_str).unwrap_or("");
                let r = exec(b, id, c.get("params").cloned().filter(|v| !v.is_null()).unwrap_or(json!({})));
                let failed = r.is_err();
                out.push(match r {
                    Ok(v) => json!({"command": id, "ok": true, "result": v}),
                    Err(e) => json!({"command": id, "ok": false, "error": e}),
                });
                if failed && !keep {
                    let mut r = ToolResult::json(&Value::Array(out));
                    r.is_error = true;
                    return r;
                }
            }
            ToolResult::json(&Value::Array(out))
        }
        "new_notebook" => {
            let mut p = json!({"name": st("name").unwrap_or("My Notebook")});
            if let Some(path) = st("path") {
                p["path"] = json!(path);
            }
            wrap(exec(b, "file.newNotebook", p))
        }
        "open_notebook" => wrap(exec(b, "file.openNotebook", json!({"path": st("path").unwrap_or("")}))),
        "new_page" => {
            let mut p = json!({});
            if let Some(t) = st("title") {
                p["title"] = json!(t);
            }
            wrap(exec(b, "page.new", p))
        }
        "go_to_page" => {
            let page = match (st("page"), st("title")) {
                (Some(p), _) => Some(p.to_string()),
                (None, Some(t)) => match b.call("notebook.inspect", json!({})) {
                    Ok(tree) => find_page_by_title(&tree, t),
                    Err(e) => return ToolResult::error(e),
                },
                (None, None) => return ToolResult::error("give `page` or `title`"),
            };
            match page {
                Some(p) => wrap(exec(b, "nav.goto", json!({"page": p}))),
                None => ToolResult::error(format!("no page titled {:?}", st("title").unwrap_or(""))),
            }
        }
        "type_text" => wrap(exec(b, "text.insert", json!({"text": st("text").unwrap_or("")}))),
        "inspect" => {
            let mut p = json!({"runs": a.get("runs").and_then(Value::as_bool).unwrap_or(false)});
            if let Some(page) = st("page") {
                p["page"] = json!(page);
            }
            wrap(b.call("notebook.inspect", p))
        }
        "get_page_text" => {
            let p = match st("page") {
                Some(page) => json!({"page": page}),
                None => json!({}),
            };
            match b.call("page.text", p) {
                Ok(v) => ToolResult::text(v.get("text").and_then(Value::as_str).unwrap_or("")),
                Err(e) => ToolResult::error(e),
            }
        }
        "search" => {
            let mut p = json!({"query": st("query").unwrap_or("")});
            if let Some(scope) = st("scope") {
                p["scope"] = json!(scope);
            }
            wrap(exec(b, "search.all", p))
        }
        "find_tags" => {
            let mut p = json!({});
            for k in ["group", "scope"] {
                if let Some(v) = st(k) {
                    p[k] = json!(v);
                }
            }
            if let Some(u) = a.get("unchecked").and_then(Value::as_bool) {
                p["unchecked"] = json!(u);
            }
            wrap(exec(b, "tag.findTags", p))
        }
        "save" => wrap(b.call("notebook.save", json!({}))),
        "screenshot" => match b.call("ui.screenshot", json!({"path": st("path")})) {
            Ok(v) => match v.get("png").and_then(Value::as_str) {
                Some(png) if !png.is_empty() => ToolResult::image(png.to_string(), &json!({"width": v.get("width"), "height": v.get("height")})),
                _ => ToolResult::json(&v),
            },
            Err(e) => ToolResult::error(e),
        },
        "press_key" => wrap(b.call("ui.key", a.clone())),
        "type_keys" => wrap(b.call("ui.text", json!({"text": st("text").unwrap_or("")}))),
        "click" => wrap(b.call("ui.click", a.clone())),
        other => ToolResult::error(format!("unknown tool `{other}`")),
    }
}
