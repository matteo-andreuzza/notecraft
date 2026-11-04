//! NoteCraft automation (L6): how agents and scripts drive NoteCraft.
//!
//! - [`control`]: the desktop app's loopback JSON-lines control channel (`notecraft --control
//!   PORT`), guarded by a per-start key in a mode-0600 file ([`keyfile`]). See
//!   `docs/control-protocol.md`.
//! - [`mcp`]: a Model Context Protocol server (JSON-RPC 2.0 over stdio) on a [`Backend`]: the
//!   in-process [`Headless`] session (`notecraft-cli mcp`) or a running app through its control
//!   channel ([`Remote`], `notecraft-cli mcp --connect 127.0.0.1:PORT`). See `docs/mcp.md`.
//! - [`inspect`]: the notebook tree, selection and page content as JSON, shared by both.
//! - [`roots`]: `--automation-read-root` / `--automation-write-root` confinement of the paths
//!   commands read and write.
//!
//! Everything that changes notes goes through [`notecraft_engine::Session::run`], so an agent
//! does exactly what the ribbon or the keyboard would, with the same undo steps.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

/// Base64 for screenshots in control replies: the engine's codec, re-exported for the hosts.
pub use notecraft_engine::b64 as base64;
#[cfg(not(target_arch = "wasm32"))]
pub mod control;
pub mod headless;
pub mod inspect;
pub mod keyfile;
pub mod mcp;
#[cfg(not(target_arch = "wasm32"))]
pub mod remote;
pub mod roots;

pub use headless::Headless;
#[cfg(not(target_arch = "wasm32"))]
pub use remote::Remote;
pub use roots::Roots;

use serde_json::Value;

/// Something that answers control-channel methods (`engine.execute`, `engine.commands`,
/// `notebook.inspect`, `ui.*`…).
pub trait Backend {
    /// Call one method. `Ok` carries the result, `Err` the error message.
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String>;
    /// True when a real window is attached (`ui.screenshot`, `ui.key`… work).
    fn has_ui(&self) -> bool;
    /// Short description for the MCP `initialize` reply ("headless session", "connected to …").
    fn describe(&self) -> String;
}

/// The error for `ui.*` methods without a window.
pub const NEEDS_APP: &str = "needs the desktop app: start `notecraft --control 7990` and run `notecraft-cli mcp --connect 127.0.0.1:7990`";

#[cfg(test)]
mod tests;
