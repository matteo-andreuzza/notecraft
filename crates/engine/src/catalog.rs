//! The parity catalog, embedded.
//!
//! `docs/parity-catalog.md` is the source of truth for parity (brief §0). The engine embeds it
//! so tests can check, at `cargo test` time, that every command points at a real catalog row
//! whose path and shortcut agree with it, and that wiring never regresses below the floor; and
//! so the UI can build the ribbon from the catalog's ribbon rows. `cargo xtask parity` reads the
//! same file.

/// The catalog Markdown.
pub const CATALOG_MD: &str = include_str!("../../../docs/parity-catalog.md");

/// One catalog row (the columns the engine and UI need).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Stable id.
    pub id: String,
    /// Functional label.
    pub label: String,
    /// `Tab/Group/Control` path.
    pub path: String,
    /// Control kind (button, toggle, gallery, split, menu, combo, …).
    pub kind: String,
    /// Shortcut (portable form).
    pub shortcut: String,
    /// Implementing command id.
    pub command: String,
    /// Priority (`P0`–`P3`).
    pub priority: String,
    /// Status (`missing`, `disabled`, `wired`, `verified`, `out-of-scope`).
    pub status: String,
    /// Notes.
    pub notes: String,
}

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
            '|' => out.push(std::mem::take(&mut cur).trim().trim_matches('`').to_string()),
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().trim_matches('`').to_string());
    out
}

/// Parse every catalog table row (tolerant: malformed lines are skipped).
pub fn rows() -> Vec<Row> {
    let mut out = Vec::new();
    let mut header: Option<Vec<String>> = None;
    for line in CATALOG_MD.lines() {
        let t = line.trim();
        if !t.starts_with('|') {
            header = None;
            continue;
        }
        let cs = cells(t);
        if cs.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')) {
            continue;
        }
        if cs.first().map(String::as_str) == Some("ID") {
            header = Some(cs);
            continue;
        }
        let Some(h) = &header else { continue };
        let get = |name: &str| h.iter().position(|c| c == name).and_then(|i| cs.get(i)).cloned().unwrap_or_default();
        let id = get("ID");
        if id.is_empty() {
            continue;
        }
        out.push(Row {
            id,
            label: get("Label"),
            path: get("Path"),
            kind: get("Kind"),
            shortcut: get("Shortcut"),
            command: get("Command"),
            priority: get("Pri"),
            status: get("Status"),
            notes: get("Notes"),
        });
    }
    out
}

/// The row with `id`.
pub fn row(id: &str) -> Option<Row> {
    rows().into_iter().find(|r| r.id == id)
}
