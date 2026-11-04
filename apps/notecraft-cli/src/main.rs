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
//! notecraft-cli mcp [--sample | --notebook DIR] [--automation-read-root DIR] [--automation-write-root DIR]
//! notecraft-cli mcp --connect 127.0.0.1:PORT        MCP server over stdio (headless, or bridged to a running app)
//! notecraft-cli --version
//! ```
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::Path;
use std::process::ExitCode;

use notecraft_automation::{Backend, Headless, Remote, Roots, inspect, mcp};
use notecraft_engine::{Clock, Session, commands};
use serde_json::{Value, json};

const USAGE: &str = "\
usage: notecraft-cli <command>

  commands [--json]         list every command
  run [--sample | --notebook DIR] [--cmd 'id={json}']... [--print] [--author NAME]
                            run commands in order; folder notebooks are saved at the end;
                            --print shows the current page as text afterwards
  batch FILE [--sample | --notebook DIR] [--print] [--author NAME]
                            run a JSON-lines script: {\"command\": id, \"params\": {...}} per line
  mcp [--sample | --notebook DIR] [--author NAME]
      [--automation-read-root DIR] [--automation-write-root DIR]
                            MCP server over stdio on a headless session; folder notebooks are
                            saved after every command; the roots confine the paths commands use
  mcp --connect 127.0.0.1:PORT
                            MCP server bridged to a running app (`notecraft --control PORT`),
                            with screenshots, keys and clicks
  onenote PATH [--json]     import a OneNote .one file, notebook folder, .onetoc2 or .onepkg
                            into a throwaway session and print what was kept and what was not
                            (the files are only read)
  --version

--notebook DIR opens a notebook folder, or creates one there if it doesn't exist.
";

/// Options that take a value, and switches, per subcommand.
const RUN_VALUES: &[&str] = &["--notebook", "--cmd", "--author"];
const RUN_SWITCHES: &[&str] = &["--sample", "--print"];
const MCP_VALUES: &[&str] = &["--notebook", "--author", "--connect", "--automation-read-root", "--automation-write-root"];
const MCP_SWITCHES: &[&str] = &["--sample"];

/// The host clock: real time and the local UTC offset.
fn clock() -> Clock {
    let now = chrono::Local::now();
    Clock { now_ms: now.timestamp_millis(), utc_offset_min: now.offset().local_minus_utc() / 60 }
}

/// Warnings and errors from the libraries go to stderr (stdout is the output, or for `mcp` the
/// protocol stream). `NOTECRAFT_LOG=info|debug` shows more.
struct StderrLog;

impl log::Log for StderrLog {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::max_level()
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            eprintln!("notecraft-cli: {}: {}", r.level().as_str().to_lowercase(), r.args());
        }
    }
    fn flush(&self) {}
}

fn init_log() {
    let level = match std::env::var("NOTECRAFT_LOG").as_deref() {
        Ok("debug") => log::LevelFilter::Debug,
        Ok("info") => log::LevelFilter::Info,
        Ok("off") => log::LevelFilter::Off,
        _ => log::LevelFilter::Warn,
    };
    if log::set_logger(&StderrLog).is_ok() {
        log::set_max_level(level);
    }
}

fn main() -> ExitCode {
    init_log();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(args: &[String]) -> Result<(), String> {
    let rest = args.get(1..).unwrap_or_default();
    match args.first().map(String::as_str) {
        Some("commands") => list(&Opts::parse("commands", rest, &[], &["--json"])?),
        Some("run") => run(&Opts::parse("run", rest, RUN_VALUES, RUN_SWITCHES)?),
        Some("batch") => batch(&Opts::parse("batch", rest, &["--notebook", "--author"], RUN_SWITCHES)?),
        Some("onenote") => onenote(&Opts::parse("onenote", rest, &[], &["--json"])?),
        Some("mcp") => serve_mcp(&Opts::parse("mcp", rest, MCP_VALUES, MCP_SWITCHES)?),
        Some("--version" | "-V") => {
            println!("notecraft-cli {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("-h" | "--help" | "help") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    }
}

/// A subcommand's arguments: options (`--name value`, `--name=value` or a switch) and
/// positionals, in order. Unknown options are refused before anything runs.
#[derive(Debug, Default, PartialEq)]
struct Opts {
    options: Vec<(String, Option<String>)>,
    positional: Vec<String>,
}

impl Opts {
    fn parse(cmd: &str, args: &[String], values: &[&str], switches: &[&str]) -> Result<Opts, String> {
        let mut o = Opts::default();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            if !a.starts_with("--") {
                o.positional.push(a.clone());
                continue;
            }
            let (name, inline) = match a.split_once('=') {
                Some((n, v)) => (n, Some(v.to_string())),
                None => (a.as_str(), None),
            };
            if values.contains(&name) {
                let v = match inline {
                    Some(v) => v,
                    None => it.next().cloned().ok_or_else(|| format!("{cmd}: {name} needs a value"))?,
                };
                o.options.push((name.to_string(), Some(v)));
            } else if switches.contains(&name) && inline.is_none() {
                o.options.push((name.to_string(), None));
            } else {
                return Err(format!("{cmd}: unknown option {a}\n\n{USAGE}"));
            }
        }
        Ok(o)
    }

    fn has(&self, name: &str) -> bool {
        self.options.iter().any(|(n, _)| n == name)
    }

    /// Every value of a repeatable option (`--cmd`), in order.
    fn values<'a, 'n>(&'a self, name: &'n str) -> impl Iterator<Item = &'a str> + use<'a, 'n> {
        self.options.iter().filter(move |(n, _)| n == name).filter_map(|(_, v)| v.as_deref())
    }

    /// The value of an option given at most once.
    fn once<'a>(&'a self, name: &str) -> Result<Option<&'a str>, String> {
        let mut v = self.values(name);
        let first = v.next();
        if v.next().is_some() {
            return Err(format!("{name} is given more than once"));
        }
        Ok(first)
    }
}

fn list(o: &Opts) -> Result<(), String> {
    if o.has("--json") {
        println!("{}", commands::describe());
        return Ok(());
    }
    for c in commands::all() {
        println!("{:<28} {:<16} {:<44} {}", c.id, c.shortcut, c.menu, c.params);
    }
    println!("\n{} commands", commands::all().len());
    Ok(())
}

/// Build the session from `--sample` / `--notebook DIR` / `--author NAME` (default: an empty
/// session).
fn session(o: &Opts) -> Result<Session, String> {
    let mut s = if o.has("--sample") { Session::with_sample(clock()) } else { Session::new(clock()) };
    if let Some(name) = o.once("--author")? {
        let name = name.trim();
        if name.is_empty() {
            return Err("--author: the name can't be empty".into());
        }
        s.prefs.author = name.to_string();
    }
    if let Some(dir) = o.once("--notebook")? {
        if Path::new(dir).exists() {
            s.open_path(dir).map_err(|e| e.to_string())?;
        } else {
            let name = Path::new(dir).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Notebook".into());
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
        print!("{}", inspect::page_text(page));
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

/// `id` or `id={json}`.
fn command_spec(spec: &str) -> Result<(String, Value), String> {
    match spec.split_once('=') {
        Some((id, js)) => Ok((id.to_string(), serde_json::from_str::<Value>(js).map_err(|e| format!("{id}: bad JSON params: {e}"))?)),
        None => Ok((spec.to_string(), json!({}))),
    }
}

fn run(o: &Opts) -> Result<(), String> {
    if let Some(p) = o.positional.first() {
        return Err(format!("run: unexpected argument `{p}` (commands go in --cmd)"));
    }
    let mut s = session(o)?;
    for spec in o.values("--cmd") {
        let (id, params) = command_spec(spec)?;
        let out = exec(&mut s, &id, params)?;
        if !out.is_null() {
            println!("{out}");
        }
    }
    finish(&mut s, o.has("--print"))
}

fn batch(o: &Opts) -> Result<(), String> {
    let file = match o.positional.as_slice() {
        [file] => file,
        [] => return Err("batch needs a file".into()),
        [_, extra, ..] => return Err(format!("batch: unexpected argument `{extra}`")),
    };
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let mut s = session(o)?;
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
    finish(&mut s, o.has("--print"))
}

/// `onenote PATH`: the fidelity report of a OneNote import (decision D12), for measuring real
/// files. A `.one` file goes into the sample notebook; a folder, `.onetoc2` or `.onepkg` becomes its own
/// in-memory notebook. Nothing is saved.
fn onenote(o: &Opts) -> Result<(), String> {
    let path = match o.positional.as_slice() {
        [path] => path,
        [] => return Err("onenote needs a .one file, a notebook folder, a .onetoc2 or a .onepkg".into()),
        [_, extra, ..] => return Err(format!("onenote: unexpected argument `{extra}`")),
    };
    let mut s = Session::with_sample(clock());
    let is_section = Path::new(path).extension().is_some_and(|e| e.eq_ignore_ascii_case("one"));
    let id = if is_section { "file.openSection" } else { "file.openNotebook" };
    let out = exec(&mut s, id, json!({"path": path}))?;
    if o.has("--json") {
        println!("{out}");
    } else {
        print!("{}", onenote_report(&out));
    }
    Ok(())
}

/// The import result as text: one construct per line.
fn onenote_report(out: &Value) -> String {
    let mut text = String::new();
    let counts = |key: &str| {
        out.get(key)
            .and_then(Value::as_object)
            .map(|m| m.iter().map(|(k, v)| format!("  {:>6}  {k}\n", v.as_u64().unwrap_or(0))).collect::<String>())
            .unwrap_or_default()
    };
    if let Some(n) = out.get("sections").and_then(Value::as_u64) {
        text.push_str(&format!("sections read: {n}\n"));
    }
    text.push_str("imported:\n");
    text.push_str(&counts("imported"));
    let lost = counts("notImported");
    if !lost.is_empty() {
        text.push_str("not imported:\n");
        text.push_str(&lost);
    }
    let damaged = out.get("damaged").and_then(Value::as_u64).unwrap_or(0);
    text.push_str(&format!("damaged objects skipped: {damaged}\n"));
    for w in out.get("warnings").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        text.push_str(&format!("  {w}\n"));
    }
    text
}

/// The MCP backend the options ask for: a headless session (optionally confined to the roots)
/// or a running app.
fn mcp_backend(o: &Opts) -> Result<Box<dyn Backend>, String> {
    if let Some(p) = o.positional.first() {
        return Err(format!("mcp: unexpected argument `{p}`"));
    }
    let (read, write) = (o.once("--automation-read-root")?, o.once("--automation-write-root")?);
    if let Some(addr) = o.once("--connect")? {
        if read.is_some() || write.is_some() {
            return Err(
                "mcp: --automation-read-root and --automation-write-root confine the headless server; a running app is not confined by them".into()
            );
        }
        if o.has("--sample") || o.has("--notebook") || o.has("--author") {
            return Err(
                "mcp: --sample, --notebook and --author apply to the headless server; with --connect the app's own notebooks and settings are used"
                    .into(),
            );
        }
        let remote = Remote::connect(addr).map_err(|e| format!("can't reach the NoteCraft app at {addr}: {e}"))?;
        return Ok(Box::new(remote));
    }
    let roots = if read.is_some() || write.is_some() { Some(Roots::new(read.map(Path::new), write.map(Path::new))?) } else { None };
    if let Some(r) = &roots {
        let show = |root: Option<&Path>| root.map_or_else(|| "none".to_string(), |r| r.display().to_string());
        eprintln!("notecraft-cli: files confined: read root {}, write root {}", show(r.read_root()), show(r.write_root()));
    }
    Ok(Box::new(Headless::new(session(o)?, Box::new(clock)).with_roots(roots)))
}

fn serve_mcp(o: &Opts) -> Result<(), String> {
    let mut server = mcp::Server::new(mcp_backend(o)?);
    let stdin = std::io::stdin();
    match server.serve(stdin.lock(), std::io::stdout()) {
        // A client that closes our stdout has hung up: end cleanly.
        Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => Err(format!("mcp: {e}")),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn options_parse_both_forms_and_refuse_unknown_ones() {
        let o = Opts::parse("mcp", &args(&["--sample", "--automation-read-root=/a", "--author", "Claude"]), MCP_VALUES, MCP_SWITCHES).unwrap();
        assert!(o.has("--sample"));
        assert_eq!(o.once("--automation-read-root").unwrap(), Some("/a"));
        assert_eq!(o.once("--author").unwrap(), Some("Claude"));
        assert!(Opts::parse("mcp", &args(&["--conect", "x"]), MCP_VALUES, MCP_SWITCHES).unwrap_err().contains("unknown option --conect"));
        assert!(Opts::parse("mcp", &args(&["--connect"]), MCP_VALUES, MCP_SWITCHES).unwrap_err().contains("needs a value"));
        assert!(Opts::parse("mcp", &args(&["--sample=1"]), MCP_VALUES, MCP_SWITCHES).is_err());
        let o = Opts::parse("mcp", &args(&["--connect", "a", "--connect", "b"]), MCP_VALUES, MCP_SWITCHES).unwrap();
        assert!(o.once("--connect").is_err());
        let o = Opts::parse("run", &args(&["--cmd", "page.new", "--cmd=text.insert={\"text\":\"x\"}"]), RUN_VALUES, RUN_SWITCHES).unwrap();
        assert_eq!(o.values("--cmd").collect::<Vec<_>>(), ["page.new", "text.insert={\"text\":\"x\"}"]);
    }

    #[test]
    fn connect_refuses_headless_options() {
        let o = |a: &[&str]| Opts::parse("mcp", &args(a), MCP_VALUES, MCP_SWITCHES).unwrap();
        let e = mcp_backend(&o(&["--connect", "127.0.0.1:9", "--automation-write-root", "/tmp"])).err().unwrap();
        assert!(e.contains("confine the headless server"), "{e}");
        let e = mcp_backend(&o(&["--connect", "127.0.0.1:9", "--sample"])).err().unwrap();
        assert!(e.contains("headless server"), "{e}");
        assert!(mcp_backend(&o(&["--automation-read-root", "/definitely/not/here"])).is_err());
        assert!(mcp_backend(&o(&["--sample"])).is_ok());
    }

    #[test]
    fn author_names_the_session() {
        let o = Opts::parse("run", &args(&["--author", " Claude (notes) "]), RUN_VALUES, RUN_SWITCHES).unwrap();
        assert_eq!(session(&o).unwrap().author(), "Claude (notes)");
        let o = Opts::parse("run", &args(&["--author", "  "]), RUN_VALUES, RUN_SWITCHES).unwrap();
        assert!(session(&o).is_err());
    }

    #[test]
    fn run_and_batch_check_their_arguments() {
        assert!(dispatch(&args(&["run", "stray"])).unwrap_err().contains("unexpected argument"));
        assert!(dispatch(&args(&["batch"])).unwrap_err().contains("needs a file"));
        assert!(dispatch(&args(&["onenote"])).unwrap_err().contains("needs a .one file"));
        assert!(dispatch(&args(&["onenote", "/nonexistent/x.one"])).unwrap_err().contains("file.openSection"));
        let report =
            onenote_report(&json!({"sections": 2, "imported": {"pages": 3}, "notImported": {"ink": 1}, "damaged": 1, "warnings": ["a.one: bad"]}));
        assert_eq!(report, "sections read: 2\nimported:\n       3  pages\nnot imported:\n       1  ink\ndamaged objects skipped: 1\n  a.one: bad\n");
        assert!(dispatch(&args(&["frobnicate"])).unwrap_err().contains("unknown command"));
        assert!(dispatch(&args(&["run", "--sample", "--cmd", "page.new={\"title\":\"T\"}"])).is_ok());
        assert!(dispatch(&args(&["run", "--sample", "--cmd", "page.new={bad"])).unwrap_err().contains("bad JSON"));
    }
}
