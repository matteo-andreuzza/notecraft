//! `notecraft-cli`: NoteCraft without a window.
//!
//! Every subcommand goes through the same command registry as the desktop app, so a script
//! here does exactly what the ribbon or keyboard would do.
//!
//! ```text
//! notecraft-cli commands [--json]                    list every command (id, catalog id, menu path, shortcut, params)
//! notecraft-cli run [--sample | --notebook DIR] [--cmd 'id={json}']... [--print]
//!                                                    run commands in order, then save (folder notebooks) and print results
//! notecraft-cli batch FILE.jsonl [--sample | --notebook DIR]
//!                                                    one {"command": id, "params": {...}} per line
//! notecraft-cli --version
//! ```
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;

use notecraft_engine::{Clock, Session, commands};
use serde_json::{Value, json};

const USAGE: &str = "\
usage: notecraft-cli <command>

  commands [--json]         list every command
  run [--sample | --notebook DIR] [--cmd 'id={json}']... [--print]
                            run commands in order; folder notebooks are saved at the end;
                            --print shows the current page as text afterwards
  batch FILE [--sample | --notebook DIR]
                            run a JSON-lines script: {\"command\": id, \"params\": {...}} per line
  --version
";

/// The host clock: real time and the local UTC offset.
fn clock() -> Clock {
    let now = chrono::Local::now();
    Clock { now_ms: now.timestamp_millis(), utc_offset_min: now.offset().local_minus_utc() / 60 }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("commands") => list(args.iter().any(|a| a == "--json")),
        Some("run") => run(&args[1..]),
        Some("batch") => batch(&args[1..]),
        Some("--version" | "-V") => {
            println!("notecraft-cli {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("-h" | "--help" | "help") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn list(json: bool) -> Result<(), String> {
    if json {
        println!("{}", commands::describe());
        return Ok(());
    }
    for c in commands::all() {
        println!("{:<28} {:<16} {:<44} {}", c.id, c.shortcut, c.menu, c.params);
    }
    println!("\n{} commands", commands::all().len());
    Ok(())
}

/// Build the session from `--sample` / `--notebook DIR` (default: an empty session).
fn session(args: &[String]) -> Result<Session, String> {
    let mut s = Session::new(clock());
    if args.iter().any(|a| a == "--sample") {
        s = Session::with_sample(clock());
    }
    if let Some(i) = args.iter().position(|a| a == "--notebook") {
        let dir = args.get(i + 1).ok_or("--notebook needs a folder")?;
        if std::path::Path::new(dir).exists() {
            s.open_path(dir).map_err(|e| e.to_string())?;
        } else {
            let name = std::path::Path::new(dir).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Notebook".into());
            s.run("file.newNotebook", json!({"name": name, "path": dir})).map_err(|e| e.to_string())?;
        }
    }
    for m in s.messages.drain(..) {
        eprintln!("note: {m}");
    }
    Ok(s)
}

fn exec(s: &mut Session, id: &str, params: Value) -> Result<Value, String> {
    s.clock = clock();
    s.run(id, params).map_err(|e| format!("{id}: {e}"))
}

fn finish(s: &mut Session, print: bool) -> Result<(), String> {
    let errors = s.save_dirty();
    if print && let Some(page) = s.page() {
        println!("# {}", page.display_title());
        for o in &page.objects {
            if let Some(ol) = o.as_outline() {
                let labels = notecraft_engine::text::list_labels(&ol.paragraphs);
                for (p, label) in ol.paragraphs.iter().zip(labels) {
                    let tags: Vec<String> = p.tags.iter().map(|t| format!("[{}{}]", t.name, if t.checked { " ✓" } else { "" })).collect();
                    let tags = if tags.is_empty() { String::new() } else { format!("{} ", tags.join("")) };
                    let label = if label.is_empty() { label } else { format!("{label} ") };
                    println!("{}{tags}{label}{}", "  ".repeat(p.level as usize), p.text());
                }
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

fn run(args: &[String]) -> Result<(), String> {
    let mut s = session(args)?;
    let mut i = 0;
    while i < args.len() {
        if args.get(i).map(String::as_str) == Some("--cmd") {
            let spec = args.get(i + 1).ok_or("--cmd needs `id` or `id={json}`")?;
            let (id, params) = match spec.split_once('=') {
                Some((id, js)) => (id.to_string(), serde_json::from_str::<Value>(js).map_err(|e| format!("{id}: bad JSON params: {e}"))?),
                None => (spec.clone(), json!({})),
            };
            let out = exec(&mut s, &id, params)?;
            if !out.is_null() {
                println!("{out}");
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    finish(&mut s, args.iter().any(|a| a == "--print"))
}

fn batch(args: &[String]) -> Result<(), String> {
    let file = args.first().ok_or("batch needs a file")?;
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let mut s = session(args)?;
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let v: Value = serde_json::from_str(line).map_err(|e| format!("{file}:{}: {e}", n + 1))?;
        let id = v.get("command").and_then(Value::as_str).ok_or_else(|| format!("{file}:{}: missing \"command\"", n + 1))?;
        let params = v.get("params").cloned().unwrap_or(json!({}));
        let out = exec(&mut s, id, params).map_err(|e| format!("{file}:{}: {e}", n + 1))?;
        println!("{}", json!({"line": n + 1, "command": id, "result": out}));
    }
    finish(&mut s, args.iter().any(|a| a == "--print"))
}
