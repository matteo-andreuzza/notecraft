//! Help (F1), Keyboard Shortcuts and About (catalog `R.Help.Help.Help`, `KB.App.Help`,
//! `QAT.NR.Help`, `QAT.NR.HelpKeyboardShortcuts`, `BS.Account.About`, `QAT.NR.About`).
//!
//! NoteCraft's help is local (no network by default): the Help pane searches every command by
//! its name, ribbon path and shortcut, straight from the registry, so it is never out of date.

use serde_json::{Value, json};

use crate::{CommandSpec, Session, commands, p};

/// Most results one help search returns.
pub const MAX_HELP_RESULTS: usize = 400;

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("help.help", "R.Help.Help.Help", "Help", "Help/Help/Help", |s, v| {
            s.view.help_pane = p::bool(v, "on").unwrap_or(!s.view.help_pane);
            s.view.help_shortcuts = false;
            Ok(json!({"open": s.view.help_pane}))
        })
        .key("F1")
        .params(r#"{"on"?: bool} (the Help pane; default: switch)"#)
        .pure(),
        CommandSpec::new(
            "help.keyboardShortcuts",
            "QAT.NR.HelpKeyboardShortcuts",
            "Keyboard Shortcuts",
            "Quick Access Toolbar/Commands Not in the Ribbon/Keyboard Shortcuts",
            |s, _| {
                s.view.help_pane = true;
                s.view.help_shortcuts = true;
                Ok(json!({"open": true}))
            },
        )
        .pure(),
        CommandSpec::new("help.search", "NC.Help.Search", "Search Help", "Window/Panes/Help/Search", search)
            .params(r#"{"query"?: string, "shortcuts"?: bool (only commands with a shortcut), "limit"?: int}"#)
            .pure(),
        CommandSpec::new("help.about", "BS.Account.About", "About NoteCraft", "File/Account/About", |s, _| {
            s.view.about = true;
            Ok(about())
        })
        .pure(),
    ]
}

/// What About says: name, version, licence.
pub fn about() -> Value {
    json!({
        "name": "NoteCraft",
        "version": env!("CARGO_PKG_VERSION"),
        "license": "MIT OR Apache-2.0",
        "commands": commands::all().len(),
    })
}

/// Whether every word of `query` appears in the command's label, menu path, id or shortcut
/// (case-insensitive).
fn matches(spec: &CommandSpec, words: &[String]) -> bool {
    let hay = format!("{} {} {} {}", spec.label, spec.menu, spec.id, spec.shortcut).to_lowercase();
    words.iter().all(|w| hay.contains(w.as_str()))
}

fn search(_: &mut Session, v: &Value) -> crate::CmdResult {
    let query = p::str(v, "query").unwrap_or("");
    // Bounded input: a query is a few words.
    let query: String = query.chars().take(200).collect();
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_string).collect();
    let shortcuts = p::bool(v, "shortcuts").unwrap_or(false);
    let limit = p::usize(v, "limit").unwrap_or(MAX_HELP_RESULTS).clamp(1, MAX_HELP_RESULTS);
    let mut hits: Vec<&CommandSpec> = commands::all()
        .iter()
        // Behaviour rows and keyboard-only internals are not things to look up by menu.
        .filter(|c| !c.menu.starts_with("Behaviour/"))
        .filter(|c| !shortcuts || !c.shortcut.is_empty())
        .filter(|c| matches(c, &words))
        .collect();
    // Label matches first, then by menu path.
    let first = words.first().cloned().unwrap_or_default();
    hits.sort_by_key(|c| (!c.label.to_lowercase().contains(&first), c.menu));
    let total = hits.len();
    let results: Vec<Value> = hits
        .into_iter()
        .take(limit)
        .map(|c| json!({"id": c.id, "label": c.label, "menu": c.menu.replace('/', " › "), "shortcut": c.shortcut.replace("Mod", "Ctrl")}))
        .collect();
    Ok(json!({"total": total, "results": results}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{run, sample};

    #[test]
    fn help_opens_and_finds_commands_by_name_path_and_shortcut() {
        // parity: R.Help.Help.Help, KB.App.Help, QAT.NR.Help, NC.Help.Search
        let mut s = sample();
        assert_eq!(run(&mut s, "help.help", json!({}))["open"], true);
        assert!(s.view.help_pane);
        assert_eq!(crate::commands::by_shortcut("F1").map(|c| c.id), Some("help.help"), "F1");
        let r = run(&mut s, "help.search", json!({"query": "bold"}));
        let first = &r["results"][0];
        assert_eq!((first["label"].as_str(), first["shortcut"].as_str()), (Some("Bold"), Some("Ctrl+B")), "{r}");
        assert!(first["menu"].as_str().unwrap().contains(" › "));
        // Every word must match; a path finds its commands.
        let r = run(&mut s, "help.search", json!({"query": "page versions delete"}));
        assert!(r["total"].as_u64().unwrap() >= 3, "{r}");
        assert_eq!(run(&mut s, "help.search", json!({"query": "zzzz nothing"}))["total"], 0);
        // Hostile queries are cut, never refused.
        let long = "a ".repeat(10_000);
        let _ = run(&mut s, "help.search", json!({"query": long}));
    }

    #[test]
    fn keyboard_shortcuts_lists_only_commands_with_one() {
        // parity: QAT.NR.HelpKeyboardShortcuts
        let mut s = sample();
        run(&mut s, "help.keyboardShortcuts", json!({}));
        assert!(s.view.help_pane && s.view.help_shortcuts);
        let r = run(&mut s, "help.search", json!({"shortcuts": true}));
        let list = r["results"].as_array().unwrap();
        assert!(list.len() > 20 && list.iter().all(|x| !x["shortcut"].as_str().unwrap().is_empty()));
        run(&mut s, "help.help", json!({"on": false}));
        assert!(!s.view.help_pane);
    }

    #[test]
    fn about_says_name_version_and_licence() {
        // parity: BS.Account.About, QAT.NR.About
        let mut s = sample();
        let a = run(&mut s, "help.about", json!({}));
        assert_eq!(a["name"], "NoteCraft");
        assert_eq!(a["version"], env!("CARGO_PKG_VERSION"));
        assert!(s.view.about);
    }
}
