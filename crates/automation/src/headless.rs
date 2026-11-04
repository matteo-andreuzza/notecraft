//! An in-process session without a window (`notecraft-cli mcp`).

use notecraft_engine::{Clock, CmdError, Session, commands};
use serde_json::{Value, json};

use crate::{Backend, NEEDS_APP, Roots, inspect};

/// The control-channel methods on an in-process [`Session`]. Folder notebooks are saved after
/// every command that changed them, as the desktop app's autosave would.
pub struct Headless {
    /// The session.
    pub session: Session,
    roots: Option<Roots>,
    clock: Box<dyn FnMut() -> Clock + Send>,
}

impl Headless {
    /// A headless backend on `session`; `clock` gives the time before every call.
    pub fn new(session: Session, clock: Box<dyn FnMut() -> Clock + Send>) -> Headless {
        Headless { session, roots: None, clock }
    }

    /// Confine the paths commands read and write (`--automation-read-root`,
    /// `--automation-write-root`).
    pub fn with_roots(mut self, roots: Option<Roots>) -> Headless {
        self.roots = roots;
        self
    }

    /// Run one command: roots checked first, folder notebooks saved after.
    pub fn execute(&mut self, id: &str, params: Value) -> Result<Value, String> {
        if let Some(r) = &self.roots {
            r.check_command(id, &params)?;
        }
        self.session.clock = (self.clock)();
        let mut v = self.session.run(id, params).map_err(|e| match e {
            CmdError::Disabled(m) => format!("{id}: disabled: {m}"),
            other => format!("{id}: {other}"),
        })?;
        let errors = self.session.save_dirty();
        let notices = std::mem::take(&mut self.session.messages);
        if let Some(o) = v.as_object_mut() {
            if !errors.is_empty() {
                o.insert("saveErrors".into(), json!(errors));
            }
            if !notices.is_empty() {
                o.insert("notices".into(), json!(notices));
            }
        } else if !errors.is_empty() {
            log::warn!("{id}: saving failed: {}", errors.join("; "));
        }
        Ok(v)
    }
}

/// Every command with whether it is enabled in `s` (and why not).
pub fn commands_json(s: &Session) -> Value {
    Value::Array(
        commands::all()
            .iter()
            .map(|c| {
                let disabled = (c.enabled)(s);
                json!({
                    "id": c.id, "catalog": c.catalog, "label": c.label, "menu": c.menu, "shortcut": c.shortcut,
                    "params": c.params, "mutates": c.mutates, "enabled": disabled.is_none(), "disabledReason": disabled,
                })
            })
            .collect(),
    )
}

/// The engine methods every backend answers the same way: `engine.commands`,
/// `notebook.inspect`, `page.text`. `None` for other methods.
pub fn engine_query(s: &Session, method: &str, params: &Value) -> Option<Result<Value, String>> {
    Some(match method {
        "engine.commands" => Ok(commands_json(s)),
        "notebook.inspect" => inspect::inspect(s, params),
        "page.text" => {
            let page = match params.get("page").and_then(Value::as_str) {
                Some(id) => notecraft_engine::doc::Id::parse(id).and_then(|id| s.ws.page(id)),
                None => s.page(),
            };
            match page {
                Some(p) => Ok(json!({"page": p.id, "text": inspect::page_text(p)})),
                None => Err("no page".to_string()),
            }
        }
        "engine.parity" => {
            let rows = notecraft_engine::catalog::rows();
            let wired = rows.iter().filter(|r| r.status == "wired" || r.status == "verified").count();
            let verified = rows.iter().filter(|r| r.status == "verified").count();
            Ok(json!({"rows": rows.len(), "wired": wired, "verified": verified, "commands": commands::all().len()}))
        }
        _ => return None,
    })
}

/// The `command` and `params` of an `engine.execute` request (`params` defaults to `{}`).
pub fn execute_request(params: &Value) -> Result<(&str, Value), String> {
    let id = params.get("command").and_then(Value::as_str).ok_or("missing `command`")?;
    Ok((id, params.get("params").cloned().filter(|v| !v.is_null()).unwrap_or_else(|| json!({}))))
}

/// `notebook.save`: write every dirty notebook now.
pub fn save_now(s: &mut Session) -> Result<Value, String> {
    let errors = s.save_dirty();
    if errors.is_empty() { Ok(json!({"saved": true})) } else { Err(errors.join("; ")) }
}

impl Backend for Headless {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        if let Some(r) = engine_query(&self.session, method, &params) {
            return r;
        }
        match method {
            "engine.execute" | "command" => {
                let (id, p) = execute_request(&params)?;
                self.execute(id, p)
            }
            "notebook.save" => save_now(&mut self.session),
            m if m.starts_with("ui.") => Err(format!("`{m}` {NEEDS_APP}")),
            other => self.execute(other, params),
        }
    }

    fn has_ui(&self) -> bool {
        false
    }

    fn describe(&self) -> String {
        "headless session (no window)".to_string()
    }
}
