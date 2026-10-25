//! `cargo xtask parity [--check]`: measure NoteCraft against the reference product.
//!
//! The source of truth is `docs/parity-catalog.md` (brief §0): one table row per ribbon control,
//! context-menu item, backstage entry, keyboard shortcut, option, documented behaviour, file
//! format construct and UI surface of Microsoft OneNote. This tool joins the catalog with the
//! live command registry (`notecraft-cli commands --json`) and with the test sources, then:
//!
//! - **fails** when a row marked `wired` or `verified` names a command the registry lacks, when
//!   a command's primary row (its `catalog` id) disagrees with the catalog on menu path or
//!   shortcut, when a `wired`/`verified` row has no test that mentions its command id or catalog
//!   id, when a `verified` row has no test that mentions its catalog id (`// parity: <ID>`),
//!   when a registered command points at a catalog id that does not exist, or when a command
//!   exists for a row still marked `missing` (so the catalog never lags the code);
//! - writes `docs/parity-checklist.md` (per-area coverage, the missing P0/P1 work list) and the
//!   generated block of `docs/parity.md` with the two headline numbers: **wiring coverage**
//!   (`wired` + `verified`) and **behavioural coverage** (`verified` only).
//!
//! `--check` regenerates in memory and fails if either file is stale (CI).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Catalog status of one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    /// Not implemented.
    Missing,
    /// Present in the UI for layout parity but disabled (does nothing yet).
    Disabled,
    /// A command/control exists and works at least for the main case.
    Wired,
    /// Behaviour verified by tests against the documented behaviour.
    Verified,
    /// Depends on a Microsoft cloud service or Windows-only component (OneDrive sign-in, Office
    /// Store, Outlook): shown disabled when it has a UI placement, excluded from the in-scope
    /// denominator but never from the all-rows denominator.
    OutOfScope,
}

impl Status {
    fn parse(s: &str) -> Option<Status> {
        Some(match s.trim() {
            "missing" => Status::Missing,
            "disabled" => Status::Disabled,
            "wired" => Status::Wired,
            "verified" => Status::Verified,
            "out-of-scope" => Status::OutOfScope,
            _ => return None,
        })
    }
    fn is_wired(self) -> bool {
        matches!(self, Status::Wired | Status::Verified)
    }
}

/// One catalog row.
#[derive(Debug, Clone)]
pub struct Row {
    pub id: String,
    pub label: String,
    pub path: String,
    pub shortcut: String,
    pub command: String,
    pub priority: String,
    pub status: Status,
    /// 1-based line in the catalog, for error messages.
    pub line: usize,
}

/// Split a Markdown table row into trimmed cells, honouring `\|` escapes.
fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cur.push('|');
                chars.next();
            }
            '|' => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

/// Strip Markdown code ticks from a cell (`` `Mod+B` `` → `Mod+B`).
fn plain(s: &str) -> String {
    s.trim().trim_matches('`').trim().to_string()
}

/// Parse every catalog table (any table whose header has `ID` and `Status` columns).
pub fn parse(md: &str) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    let mut header: Option<HashMap<String, usize>> = None;
    let mut seen = HashSet::new();
    for (i, line) in md.lines().enumerate() {
        let t = line.trim();
        if !t.starts_with('|') {
            header = None;
            continue;
        }
        let cs = cells(t);
        if cs.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')) {
            continue; // separator row
        }
        if cs.first().map(String::as_str) == Some("ID") && cs.iter().any(|c| c == "Status") {
            header = Some(cs.iter().enumerate().map(|(i, c)| (c.clone(), i)).collect());
            continue;
        }
        let Some(h) = &header else { continue };
        let get = |name: &str| h.get(name).and_then(|&i| cs.get(i)).map(|s| plain(s)).unwrap_or_default();
        let id = get("ID");
        if id.is_empty() {
            continue;
        }
        let status = Status::parse(&get("Status")).ok_or_else(|| format!("parity-catalog.md:{}: {id}: unknown status `{}`", i + 1, get("Status")))?;
        if !seen.insert(id.clone()) {
            return Err(format!("parity-catalog.md:{}: duplicate ID {id}", i + 1));
        }
        rows.push(Row {
            id,
            label: get("Label"),
            path: get("Path"),
            shortcut: get("Shortcut"),
            command: get("Command"),
            priority: get("Pri"),
            status,
            line: i + 1,
        });
    }
    Ok(rows)
}

/// A registered command as `notecraft-cli commands --json` reports it.
#[derive(Debug, Clone, Default)]
pub struct Cmd {
    pub id: String,
    pub catalog: String,
    pub menu: String,
    pub shortcut: String,
}

/// Parse the registry JSON (an array of objects).
pub fn registry_from_json(v: &serde_json::Value) -> Vec<Cmd> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|c| Cmd {
            id: c["id"].as_str().unwrap_or_default().to_string(),
            catalog: c["catalog"].as_str().unwrap_or_default().to_string(),
            menu: c["menu"].as_str().unwrap_or_default().to_string(),
            shortcut: c["shortcut"].as_str().unwrap_or_default().to_string(),
        })
        .collect()
}

/// Normalise a shortcut for comparison: case-insensitive, spaces removed.
fn norm_key(k: &str) -> String {
    k.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase()
}

/// The individual key combinations of a shortcut field (`A / B`).
fn keys(field: &str) -> Vec<String> {
    field.split(" / ").map(norm_key).filter(|k| !k.is_empty()).collect()
}

/// Strings that test code mentions: every double-quoted literal, plus every token after
/// `parity:` in a comment.
#[derive(Debug, Default)]
pub struct TestIndex {
    pub mentions: HashSet<String>,
}

impl TestIndex {
    /// Index one file's test code.
    pub fn add_test_code(&mut self, code: &str) {
        let mut rest = code;
        while let Some(start) = rest.find('"') {
            let after = &rest[start + 1..];
            let Some(end) = after.find('"') else { break };
            let lit = &after[..end];
            if !lit.is_empty() && lit.len() < 128 && !lit.contains(char::is_whitespace) {
                self.mentions.insert(lit.to_string());
            }
            rest = &after[end + 1..];
        }
        for line in code.lines() {
            if let Some(pos) = line.find("parity:") {
                for tok in line[pos + "parity:".len()..].split(|c: char| c.is_whitespace() || c == ',') {
                    let tok = tok.trim_matches(|c: char| c == '`' || c == '.' || c == ';');
                    if !tok.is_empty() {
                        self.mentions.insert(tok.to_string());
                    }
                }
            }
        }
    }

    /// The part of a source file that is test code: the whole file for integration tests and
    /// `tests*.rs` modules, otherwise everything after the first `#[cfg(test)]`.
    pub fn test_part<'a>(path: &Path, text: &'a str) -> &'a str {
        let in_tests_dir = path.components().any(|c| c.as_os_str() == "tests");
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if in_tests_dir || name.starts_with("tests") || name.ends_with("_tests.rs") {
            return text;
        }
        match text.find("#[cfg(test)]") {
            Some(i) => &text[i..],
            None => "",
        }
    }
}

fn walk_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target" || n.to_string_lossy().starts_with('.')) {
                continue;
            }
            walk_rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Index the test code of `crates/` and `apps/`.
pub fn index_tests(root: &Path) -> TestIndex {
    let mut files = Vec::new();
    walk_rs(&root.join("crates"), &mut files);
    walk_rs(&root.join("apps"), &mut files);
    let mut idx = TestIndex::default();
    for f in files {
        if let Ok(text) = std::fs::read_to_string(&f) {
            idx.add_test_code(TestIndex::test_part(&f, &text));
        }
    }
    idx
}

/// Coverage counts for one area.
#[derive(Debug, Default, Clone, Copy)]
pub struct Tally {
    pub total: usize,
    pub out_of_scope: usize,
    pub disabled: usize,
    pub wired: usize,
    pub verified: usize,
}

impl Tally {
    fn add(&mut self, s: Status) {
        self.total += 1;
        match s {
            Status::OutOfScope => self.out_of_scope += 1,
            Status::Disabled => self.disabled += 1,
            Status::Wired => self.wired += 1,
            Status::Verified => {
                self.wired += 1;
                self.verified += 1;
            }
            Status::Missing => {}
        }
    }
    fn in_scope(&self) -> usize {
        self.total - self.out_of_scope
    }
}

fn pct(n: usize, d: usize) -> String {
    if d == 0 { "—".into() } else { format!("{:.1}%", n as f64 * 100.0 / d as f64) }
}

/// The reporting area of a row: the ID's surface prefix plus, for ribbon rows, the tab.
pub fn area(row: &Row) -> String {
    let prefix = row.id.split('.').next().unwrap_or_default();
    let name = match prefix {
        "R" => return format!("Ribbon › {}", row.path.split('/').next().unwrap_or("?")),
        "CT" => return format!("Contextual tab › {}", row.path.split('/').next().unwrap_or("?")),
        "BS" => "Backstage (File)",
        "QAT" => "Quick Access Toolbar & commands not in the ribbon",
        "CM" => "Context menus",
        "KB" => "Keyboard shortcuts",
        "OPT" => "Options",
        "BEH" => "Behaviour",
        "FMT" => "File formats",
        "UI" => "Window layout & UI surfaces",
        "DOCK" => "Docked window ribbon",
        "NC" => "NoteCraft additions",
        _ => "Other",
    };
    name.to_string()
}

/// Result of joining the catalog, the registry and the tests.
#[derive(Debug, Default)]
pub struct Report {
    pub errors: Vec<String>,
    pub areas: BTreeMap<String, Tally>,
    pub all: Tally,
    pub commands: usize,
    pub p01_missing: Vec<(String, String, String, String, String)>,
}

/// Evaluate the catalog (pure; unit-tested).
pub fn evaluate(rows: &[Row], registry: &[Cmd], tests: &TestIndex) -> Report {
    let mut r = Report { commands: registry.len(), ..Report::default() };
    let by_id: HashMap<&str, &Cmd> = registry.iter().map(|c| (c.id.as_str(), c)).collect();
    let row_ids: HashMap<&str, &Row> = rows.iter().map(|row| (row.id.as_str(), row)).collect();
    for row in rows {
        r.all.add(row.status);
        r.areas.entry(area(row)).or_default().add(row.status);
        let at = format!("parity-catalog.md:{}: {}", row.line, row.id);
        if row.status.is_wired() {
            if !row.command.is_empty() {
                match by_id.get(row.command.as_str()) {
                    None => r.errors.push(format!("{at}: marked {:?} but command `{}` is not registered", row.status, row.command)),
                    Some(cmd) => {
                        if !row.shortcut.is_empty() {
                            let have = keys(&cmd.shortcut);
                            for k in keys(&row.shortcut) {
                                if !have.contains(&k) {
                                    r.errors.push(format!(
                                        "{at}: catalog shortcut `{}` not bound on `{}` (has `{}`)",
                                        row.shortcut, cmd.id, cmd.shortcut
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            let tested = tests.mentions.contains(&row.id) || (!row.command.is_empty() && tests.mentions.contains(&row.command));
            if !tested {
                r.errors.push(format!(
                    "{at}: marked {:?} but no test mentions `{}`{}",
                    row.status,
                    row.id,
                    if row.command.is_empty() { String::new() } else { format!(" or `{}`", row.command) }
                ));
            }
            if row.status == Status::Verified && !tests.mentions.contains(&row.id) {
                r.errors.push(format!("{at}: marked verified but no test carries `// parity: {}`", row.id));
            }
        } else if (row.priority == "P0" || row.priority == "P1") && row.status != Status::OutOfScope {
            r.p01_missing.push((row.priority.clone(), row.id.clone(), row.label.clone(), row.path.clone(), row.command.clone()));
        }
        if row.status == Status::Missing
            && let Some(cmd) = by_id.get(row.command.as_str())
            && cmd.catalog == row.id
        {
            r.errors.push(format!("{at}: command `{}` exists; mark the row wired (and test it)", row.command));
        }
    }
    for cmd in registry {
        if cmd.catalog.is_empty() {
            r.errors.push(format!("command `{}` has no parity-catalog id", cmd.id));
            continue;
        }
        let Some(row) = row_ids.get(cmd.catalog.as_str()) else {
            r.errors.push(format!("command `{}` points at catalog id `{}`, which docs/parity-catalog.md lacks", cmd.id, cmd.catalog));
            continue;
        };
        if !row.command.is_empty() && row.command != cmd.id {
            r.errors.push(format!("command `{}` claims {} but that row names `{}`", cmd.id, row.id, row.command));
        }
        if !row.path.is_empty() && !cmd.menu.is_empty() && row.path != cmd.menu {
            r.errors.push(format!("command `{}`: menu path `{}` differs from the catalog's `{}` ({})", cmd.id, cmd.menu, row.path, row.id));
        }
    }
    r.p01_missing.sort();
    r
}

/// The generated checklist (`docs/parity-checklist.md`).
pub fn render_checklist(r: &Report) -> String {
    let mut s = String::new();
    s.push_str("# Parity checklist (generated)\n\n");
    s.push_str("> Generated by `cargo xtask parity` from `docs/parity-catalog.md` and the command registry. Do not edit by hand.\n\n");
    s.push_str(&format!(
        "**Wiring coverage: {}/{} ({}) · behavioural coverage: {}/{} ({}) · {} registered commands.**\n\n",
        r.all.wired,
        r.all.total,
        pct(r.all.wired, r.all.total),
        r.all.verified,
        r.all.total,
        pct(r.all.verified, r.all.total),
        r.commands
    ));
    s.push_str(&format!(
        "In-scope only (excluding {} rows that need Microsoft cloud services or Outlook): wiring {}, behaviour {}.\n\n",
        r.all.out_of_scope,
        pct(r.all.wired, r.all.in_scope()),
        pct(r.all.verified, r.all.in_scope())
    ));
    s.push_str("| Area | Rows | Wired | Verified | Disabled | Out of scope | Wiring | Behaviour |\n|---|---|---|---|---|---|---|---|\n");
    for (name, t) in &r.areas {
        s.push_str(&format!(
            "| {name} | {} | {} | {} | {} | {} | {} | {} |\n",
            t.total,
            t.wired,
            t.verified,
            t.disabled,
            t.out_of_scope,
            pct(t.wired, t.total),
            pct(t.verified, t.total)
        ));
    }
    s.push_str("\n## Missing P0/P1 rows (the work list)\n\n| Pri | ID | Label | Path | Planned command |\n|---|---|---|---|---|\n");
    for (p, id, label, path, cmd) in &r.p01_missing {
        s.push_str(&format!("| {p} | {id} | {label} | {path} | {} |\n", if cmd.is_empty() { "—".to_string() } else { format!("`{cmd}`") }));
    }
    s
}

const BEGIN: &str = "<!-- parity:begin (generated by cargo xtask parity) -->";
const END: &str = "<!-- parity:end -->";

/// The generated block of `docs/parity.md`.
pub fn render_block(r: &Report) -> String {
    format!(
        "{BEGIN}\n| Number | Value |\n|---|---|\n| **Wiring coverage** (row has a working command or control) | **{}/{} ({})** |\n| **Behavioural coverage** (verified by tests against documented behaviour) | **{}/{} ({})** |\n| In-scope wiring / behaviour (excluding {} cloud/Outlook rows) | {} / {} |\n| Registered commands | {} |\n{END}",
        r.all.wired,
        r.all.total,
        pct(r.all.wired, r.all.total),
        r.all.verified,
        r.all.total,
        pct(r.all.verified, r.all.total),
        r.all.out_of_scope,
        pct(r.all.wired, r.all.in_scope()),
        pct(r.all.verified, r.all.in_scope()),
        r.commands
    )
}

/// Replace the generated block inside `doc` (between the markers).
pub fn splice(doc: &str, block: &str) -> Option<String> {
    let start = doc.find(BEGIN)?;
    let end = doc[start..].find(END)? + start + END.len();
    Some(format!("{}{}{}", &doc[..start], block, &doc[end..]))
}

fn registry(root: &Path) -> Result<Vec<Cmd>, String> {
    if !root.join("apps/notecraft-cli/Cargo.toml").exists() {
        return Ok(Vec::new());
    }
    let out = crate::cargo()
        .args(["run", "-q", "-p", "notecraft-cli", "--", "commands", "--json"])
        .output()
        .map_err(|e| format!("cargo run notecraft-cli: {e}"))?;
    if !out.status.success() {
        return Err(format!("notecraft-cli commands --json failed:\n{}", String::from_utf8_lossy(&out.stderr)));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("notecraft-cli commands --json: bad JSON: {e}"))?;
    Ok(registry_from_json(&v))
}

/// Entry point.
pub fn run(root: &Path, check: bool) -> Result<(), String> {
    let md = std::fs::read_to_string(root.join("docs/parity-catalog.md")).map_err(|e| format!("docs/parity-catalog.md: {e}"))?;
    let rows = parse(&md)?;
    let reg = registry(root)?;
    let tests = index_tests(root);
    let report = evaluate(&rows, &reg, &tests);
    let checklist = render_checklist(&report);
    let parity_path = root.join("docs/parity.md");
    let parity_doc = std::fs::read_to_string(&parity_path).map_err(|e| format!("docs/parity.md: {e}"))?;
    let spliced = splice(&parity_doc, &render_block(&report)).ok_or("docs/parity.md lacks the parity:begin/parity:end markers")?;
    let checklist_path = root.join("docs/parity-checklist.md");
    for (name, t) in &report.areas {
        println!("{name:<56} {:>4} rows  wiring {:>6}  behaviour {:>6}", t.total, pct(t.wired, t.total), pct(t.verified, t.total));
    }
    println!(
        "\nwiring coverage {}/{} ({}), behavioural coverage {}/{} ({}), {} commands",
        report.all.wired,
        report.all.total,
        pct(report.all.wired, report.all.total),
        report.all.verified,
        report.all.total,
        pct(report.all.verified, report.all.total),
        report.commands
    );
    let mut errors = report.errors;
    if check {
        if std::fs::read_to_string(&checklist_path).unwrap_or_default() != checklist {
            errors.push("docs/parity-checklist.md is stale: run `cargo xtask parity` and commit it".into());
        }
        if parity_doc != spliced {
            errors.push("docs/parity.md's generated block is stale: run `cargo xtask parity` and commit it".into());
        }
    } else {
        std::fs::write(&checklist_path, &checklist).map_err(|e| format!("{}: {e}", checklist_path.display()))?;
        std::fs::write(&parity_path, &spliced).map_err(|e| format!("{}: {e}", parity_path.display()))?;
        println!("wrote docs/parity-checklist.md and docs/parity.md");
    }
    if errors.is_empty() {
        Ok(())
    } else {
        for e in &errors {
            eprintln!("  - {e}");
        }
        Err(format!("{} parity error(s)", errors.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MD: &str = "\
# Catalog

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| R.Home.Bold | Bold | Home/Basic Text/Bold | toggle | `Mod+B` | format.bold | P0 | wired | FLUENT19:Bold | |
| R.Home.Italic | Italic | Home/Basic Text/Italic | toggle | `Mod+I` | format.italic | P0 | missing | FLUENT19:Italic | |
| KB.Format.Pipe | Odd \\| key | Keyboard/Format/x | shortcut | | | P3 | out-of-scope | KBD365 | |
";

    fn cmd(id: &str, catalog: &str, menu: &str, shortcut: &str) -> Cmd {
        Cmd { id: id.into(), catalog: catalog.into(), menu: menu.into(), shortcut: shortcut.into() }
    }

    #[test]
    fn parses_rows_and_escapes() {
        let rows = parse(MD).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].shortcut, "Mod+B");
        assert_eq!(rows[2].label, "Odd | key");
        assert_eq!(rows[2].status, Status::OutOfScope);
    }

    #[test]
    fn duplicate_ids_and_bad_status_rejected() {
        let dup = format!("{MD}| R.Home.Bold | x | y | z | | | P0 | missing | s | |\n");
        assert!(parse(&dup).unwrap_err().contains("duplicate"));
        let bad = MD.replace("| P0 | wired |", "| P0 | done |");
        assert!(parse(&bad).unwrap_err().contains("unknown status"));
    }

    #[test]
    fn wired_row_needs_command_and_test() {
        let rows = parse(MD).unwrap();
        let mut t = TestIndex::default();
        let r = evaluate(&rows, &[], &t);
        assert!(r.errors.iter().any(|e| e.contains("not registered")));
        let reg = [cmd("format.bold", "R.Home.Bold", "Home/Basic Text/Bold", "Mod+B")];
        let r = evaluate(&rows, &reg, &t);
        assert!(r.errors.iter().any(|e| e.contains("no test mentions")), "{:?}", r.errors);
        t.add_test_code("#[test] fn b() { run(\"format.bold\"); }");
        let r = evaluate(&rows, &reg, &t);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(r.all.wired, 1);
        assert_eq!(r.p01_missing.len(), 1);
    }

    #[test]
    fn shortcut_and_menu_divergence_flagged() {
        let rows = parse(MD).unwrap();
        let mut t = TestIndex::default();
        t.add_test_code("\"format.bold\"");
        let reg = [cmd("format.bold", "R.Home.Bold", "Home/Font/Bold", "Mod+Shift+B")];
        let r = evaluate(&rows, &reg, &t);
        assert!(r.errors.iter().any(|e| e.contains("not bound")));
        assert!(r.errors.iter().any(|e| e.contains("differs from the catalog")));
    }

    #[test]
    fn verified_needs_parity_comment() {
        let rows = parse(&MD.replace("| P0 | wired |", "| P0 | verified |")).unwrap();
        let mut t = TestIndex::default();
        t.add_test_code("\"format.bold\"");
        let reg = [cmd("format.bold", "R.Home.Bold", "Home/Basic Text/Bold", "Mod+B")];
        assert!(evaluate(&rows, &reg, &t).errors.iter().any(|e| e.contains("parity: R.Home.Bold")));
        t.add_test_code("// parity: R.Home.Bold, R.Home.Other");
        assert!(evaluate(&rows, &reg, &t).errors.is_empty());
    }

    #[test]
    fn existing_command_on_missing_row_and_orphans_flagged() {
        let rows = parse(MD).unwrap();
        let mut t = TestIndex::default();
        t.add_test_code("\"format.bold\" \"format.italic\"");
        let reg = [
            cmd("format.bold", "R.Home.Bold", "Home/Basic Text/Bold", "Mod+B"),
            cmd("format.italic", "R.Home.Italic", "Home/Basic Text/Italic", "Mod+I"),
            cmd("ghost", "R.Nope", "", ""),
        ];
        let r = evaluate(&rows, &reg, &t);
        assert!(r.errors.iter().any(|e| e.contains("mark the row wired")));
        assert!(r.errors.iter().any(|e| e.contains("lacks")));
    }

    #[test]
    fn test_part_selection() {
        assert_eq!(TestIndex::test_part(Path::new("crates/a/src/lib.rs"), "x\n#[cfg(test)]\ny"), "#[cfg(test)]\ny");
        assert_eq!(TestIndex::test_part(Path::new("crates/a/tests/t.rs"), "all"), "all");
        assert_eq!(TestIndex::test_part(Path::new("crates/a/src/lib.rs"), "no tests"), "");
    }

    #[test]
    fn splice_replaces_block() {
        let doc = format!("a\n{BEGIN}\nold\n{END}\nb");
        let out = splice(&doc, "NEW").unwrap();
        assert_eq!(out, "a\nNEW\nb");
        assert!(splice("no markers", "x").is_none());
    }
}
