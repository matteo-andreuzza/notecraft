//! The command registry: every command module's `specs()`, assembled once.
//!
//! New commands go in a new or existing `<area>_cmds.rs` module with a `specs()` function; add
//! the module to the list below (one line).

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::{Value, json};

use crate::CommandSpec;

struct Registry {
    specs: Vec<CommandSpec>,
    by_id: HashMap<&'static str, usize>,
}

fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(|| {
        let mut specs = Vec::new();
        specs.extend(crate::edit_cmds::specs());
        specs.extend(crate::text_cmds::specs());
        specs.extend(crate::caret_cmds::specs());
        specs.extend(crate::format_cmds::specs());
        specs.extend(crate::para_cmds::specs());
        specs.extend(crate::tag_cmds::specs());
        specs.extend(crate::outline_cmds::specs());
        specs.extend(crate::table_cmds::specs());
        specs.extend(crate::math_cmds::specs());
        specs.extend(crate::picture_cmds::specs());
        specs.extend(crate::draw_cmds::specs());
        specs.extend(crate::printout_cmds::specs());
        specs.extend(crate::page_cmds::specs());
        specs.extend(crate::paper_cmds::specs());
        specs.extend(crate::template_cmds::specs());
        specs.extend(crate::section_cmds::specs());
        specs.extend(crate::protect_cmds::specs());
        specs.extend(crate::file_cmds::specs());
        specs.extend(crate::export_cmds::specs());
        specs.extend(crate::import_cmds::specs());
        specs.extend(crate::onenote_cmds::specs());
        specs.extend(crate::nav_cmds::specs());
        specs.extend(crate::search_cmds::specs());
        specs.extend(crate::view_cmds::specs());
        let by_id = specs.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
        Registry { specs, by_id }
    })
}

/// Every registered command, in registration order.
pub fn all() -> &'static [CommandSpec] {
    &registry().specs
}

/// The command with `id`.
pub fn find(id: &str) -> Option<&'static CommandSpec> {
    let r = registry();
    r.by_id.get(id).and_then(|&i| r.specs.get(i))
}

/// The command bound to a portable key combination (`Mod+B`), if any. Case-insensitive.
pub fn by_shortcut(key: &str) -> Option<&'static CommandSpec> {
    let k = key.replace(' ', "").to_ascii_lowercase();
    all().iter().find(|s| s.keys().any(|x| x.replace(' ', "").to_ascii_lowercase() == k))
}

/// The registry as JSON (for agents and `cargo xtask parity`): id, catalog id, label, menu path,
/// shortcut, params doc, whether it mutates.
pub fn describe() -> Value {
    Value::Array(
        all()
            .iter()
            .map(|s| json!({"id": s.id, "catalog": s.catalog, "label": s.label, "menu": s.menu, "shortcut": s.shortcut, "params": s.params, "mutates": s.mutates}))
            .collect(),
    )
}
