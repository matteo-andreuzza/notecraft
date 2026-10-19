//! NoteCraft engine (layer **L5**): the editing session and its commands.
//!
//! **Everything is a command.** Each user-visible action is a [`CommandSpec`] with a stable id
//! (`format.bold`, `page.new`), the **parity-catalog id** of the reference control it implements
//! (`R.Home.BasicText.Bold`), a label, the **menu path** mirroring the reference's ribbon/menu
//! path (`Home/Basic Text/Bold`), the reference's default shortcut (`Mod+B`), a params doc, an
//! `enabled` predicate and a `run` function. The ribbon, context menus, keyboard, CLI, JSON
//! control channel and MCP server all call [`Session::run`], so an action behaves the same
//! however it is invoked, and every mutating command records exactly one undo step.
//!
//! The engine sits at L5: it owns the session state (workspace, selection, undo, search index,
//! autosave) and is the seam every front end (L6) uses; it never draws anything.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod b64;
pub mod catalog;
pub mod commands;
pub mod sample;
pub mod selection;
pub mod session;

mod caret_cmds;
mod edit_cmds;
mod file_cmds;
mod format_cmds;
mod nav_cmds;
mod ops;
mod outline_cmds;
mod page_cmds;
mod para_cmds;
mod picture_cmds;
mod search_cmds;
mod section_cmds;
mod table_cmds;
mod tag_cmds;
mod text_cmds;
mod view_cmds;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_cmds;
#[cfg(test)]
mod tests_picture;
#[cfg(test)]
mod tests_table;

use serde_json::Value;

pub use notecraft_doc as doc;
pub use notecraft_search as search;
pub use notecraft_text as text;
pub use selection::{Block, Pos, Selection, TextRange};
pub use session::{Clock, FindState, NbDirty, Prefs, RibbonMode, ScopeKind, ScrollRequest, Session, ViewState};

/// Errors a command can return. Commands never panic; bad input is `Params`, an unavailable
/// command is `Disabled`, everything else `Failed`.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum CmdError {
    /// No command with this id.
    #[error("unknown command `{0}`")]
    Unknown(String),
    /// The parameters are missing or wrong.
    #[error("bad parameters: {0}")]
    Params(String),
    /// The command can't run in the current state (the reason the UI shows in its tooltip).
    #[error("not available: {0}")]
    Disabled(String),
    /// The command ran and failed.
    #[error("{0}")]
    Failed(String),
}

/// A command's result: JSON for agents (often `null`), or an error.
pub type CmdResult = Result<Value, CmdError>;

/// "Why is this disabled?" — `None` = enabled.
pub type Enabled = fn(&Session) -> Option<&'static str>;

/// A registered command.
#[derive(Clone)]
pub struct CommandSpec {
    /// Stable id (`area.camelCase`).
    pub id: &'static str,
    /// The parity-catalog row this command implements (its primary placement).
    pub catalog: &'static str,
    /// Functional label.
    pub label: &'static str,
    /// Menu path mirroring the reference (`Home/Tags/To Do Tag`); must equal the catalog row's
    /// path (`cargo xtask parity` checks).
    pub menu: &'static str,
    /// Default shortcut in portable form (`Mod+B`; alternatives separated by ` / `). `Mod` is
    /// Ctrl, or ⌘ on macOS.
    pub shortcut: &'static str,
    /// Parameter description for agents (`{"size": number}`).
    pub params: &'static str,
    /// Whether it changes the document (records undo, marks sections dirty).
    pub mutates: bool,
    /// Enabled predicate.
    pub enabled: Enabled,
    /// The implementation.
    pub run: fn(&mut Session, &Value) -> CmdResult,
}

fn always(_: &Session) -> Option<&'static str> {
    None
}

impl CommandSpec {
    /// A mutating command, enabled always, no shortcut, no params.
    pub const fn new(
        id: &'static str,
        catalog: &'static str,
        label: &'static str,
        menu: &'static str,
        run: fn(&mut Session, &Value) -> CmdResult,
    ) -> CommandSpec {
        CommandSpec { id, catalog, label, menu, shortcut: "", params: "{}", mutates: true, enabled: always, run }
    }
    /// Set the shortcut.
    pub const fn key(mut self, s: &'static str) -> CommandSpec {
        self.shortcut = s;
        self
    }
    /// Set the params doc.
    pub const fn params(mut self, p: &'static str) -> CommandSpec {
        self.params = p;
        self
    }
    /// Mark as not changing the document (navigation, view, queries).
    pub const fn pure(mut self) -> CommandSpec {
        self.mutates = false;
        self
    }
    /// Set the enabled predicate.
    pub const fn when(mut self, f: Enabled) -> CommandSpec {
        self.enabled = f;
        self
    }
    /// The shortcut's alternatives.
    pub fn keys(&self) -> impl Iterator<Item = &'static str> {
        self.shortcut.split(" / ").map(str::trim).filter(|k| !k.is_empty())
    }
}

/// Parameter helpers: every accessor validates type and range and never panics.
pub mod p {
    use serde_json::Value;

    use crate::CmdError;

    /// An optional string.
    pub fn str<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
        v.get(k).and_then(Value::as_str)
    }
    /// A required string.
    pub fn req_str<'a>(v: &'a Value, k: &str) -> Result<&'a str, CmdError> {
        str(v, k).ok_or_else(|| CmdError::Params(format!("`{k}` (string) is required")))
    }
    /// An optional finite number.
    pub fn f32(v: &Value, k: &str) -> Option<f32> {
        v.get(k).and_then(Value::as_f64).map(|x| x as f32).filter(|x| x.is_finite())
    }
    /// A required finite number.
    pub fn req_f32(v: &Value, k: &str) -> Result<f32, CmdError> {
        f32(v, k).ok_or_else(|| CmdError::Params(format!("`{k}` (number) is required")))
    }
    /// An optional non-negative integer (floats are truncated; negatives and NaN rejected).
    pub fn usize(v: &Value, k: &str) -> Option<usize> {
        v.get(k)
            .and_then(|x| x.as_u64().or_else(|| x.as_f64().filter(|f| *f >= 0.0 && f.is_finite()).map(|f| f as u64)))
            .map(|u| u.min(usize::MAX as u64) as usize)
    }
    /// A required non-negative integer.
    pub fn req_usize(v: &Value, k: &str) -> Result<usize, CmdError> {
        usize(v, k).ok_or_else(|| CmdError::Params(format!("`{k}` (non-negative integer) is required")))
    }
    /// An optional bool.
    pub fn bool(v: &Value, k: &str) -> Option<bool> {
        v.get(k).and_then(Value::as_bool)
    }
    /// An optional id (32 hex digits).
    pub fn id(v: &Value, k: &str) -> Result<Option<notecraft_doc::Id>, CmdError> {
        match v.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => notecraft_doc::Id::parse(s).map(Some).ok_or_else(|| CmdError::Params(format!("`{k}` is not an id"))),
            Some(_) => Err(CmdError::Params(format!("`{k}` must be an id string"))),
        }
    }
    /// A required id.
    pub fn req_id(v: &Value, k: &str) -> Result<notecraft_doc::Id, CmdError> {
        id(v, k)?.ok_or_else(|| CmdError::Params(format!("`{k}` (id) is required")))
    }
}
