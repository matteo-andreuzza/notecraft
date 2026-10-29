//! `--automation-read-root` / `--automation-write-root`: keep an agent to the folders it was
//! given (the same flags as the other Craft apps, so one client config works across the suite).
//!
//! Every path a command reads must lie inside the read root, every path it writes inside the
//! write root. A notebook opened through automation is saved in place as it changes, so opening
//! one needs both. A root left out grants none of its access. Paths are made absolute against
//! the working directory and their links followed; for a path that doesn't exist yet, its
//! nearest existing folder's links are followed and the rest must be plain names.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

/// What a command does with a path parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    Read,
    Write,
    ReadWrite,
}

/// The path parameters of the commands that touch the file system: (command, param, access).
const PATH_PARAMS: &[(&str, &str, Access)] = &[
    ("file.openNotebook", "path", Access::ReadWrite),
    ("file.newNotebook", "path", Access::Write),
    ("file.exportNotebook", "path", Access::Write),
    // Markdown also writes a `<name>_files` folder beside `path` (never through a link).
    ("file.exportPage", "path", Access::Write),
    ("file.exportSection", "path", Access::Write),
    ("file.exportMht", "path", Access::Write),
    ("file.print", "dir", Access::Write),
    ("file.printPreview", "dir", Access::Write),
    ("file.backupNow", "folder", Access::Write),
    ("file.openBackups", "folder", Access::Read),
];

/// Parameter names that carry a file-system path in any command.
const PATH_KEYS: &[&str] = &["path", "folder", "file", "dir"];

/// The folders automation may read and write.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Roots {
    read: Option<PathBuf>,
    write: Option<PathBuf>,
}

impl Roots {
    /// Roots from the two flags. Each must be an existing folder.
    pub fn new(read: Option<&Path>, write: Option<&Path>) -> Result<Roots, String> {
        let dir = |p: Option<&Path>, what: &str| -> Result<Option<PathBuf>, String> {
            let Some(p) = p else { return Ok(None) };
            let c = absolute(p).canonicalize().map_err(|e| format!("--automation-{what}-root {}: {e}", p.display()))?;
            if !c.is_dir() {
                return Err(format!("--automation-{what}-root {}: not a folder", p.display()));
            }
            Ok(Some(c))
        };
        Ok(Roots { read: dir(read, "read")?, write: dir(write, "write")? })
    }

    /// The read root, if any.
    pub fn read_root(&self) -> Option<&Path> {
        self.read.as_deref()
    }

    /// The write root, if any.
    pub fn write_root(&self) -> Option<&Path> {
        self.write.as_deref()
    }

    /// Check that `path` may be read; returns it resolved.
    pub fn check_read(&self, path: &str) -> Result<PathBuf, String> {
        check(self.read.as_deref(), "read", path)
    }

    /// Check that `path` may be written; returns it resolved.
    pub fn check_write(&self, path: &str) -> Result<PathBuf, String> {
        check(self.write.as_deref(), "write", path)
    }

    /// Check a command's path parameters before it runs. A command that takes a path this
    /// table doesn't know is refused rather than let through.
    pub fn check_command(&self, id: &str, params: &Value) -> Result<(), String> {
        let Some(obj) = params.as_object() else { return Ok(()) };
        for key in PATH_KEYS {
            let Some(v) = obj.get(*key) else { continue };
            let Some(path) = v.as_str() else { continue };
            let access = PATH_PARAMS.iter().find(|(c, k, _)| *c == id && k == key).map(|(_, _, a)| *a);
            match access {
                Some(Access::Read) => {
                    self.check_read(path)?;
                }
                Some(Access::Write) => {
                    self.check_write(path)?;
                }
                Some(Access::ReadWrite) => {
                    self.check_read(path)?;
                    self.check_write(path)?;
                }
                None => return Err(format!("automation path rejected: `{id}` takes a `{key}` the automation roots don't cover: {path}")),
            }
        }
        Ok(())
    }
}

fn absolute(p: &Path) -> PathBuf {
    if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().map(|d| d.join(p)).unwrap_or_else(|_| p.to_path_buf()) }
}

/// Resolve `path`: links followed for the part that exists; the part that doesn't must be plain
/// names (no `..`, no `.`, no dangling link to write through).
pub fn resolve(path: &str) -> Result<PathBuf, String> {
    if path.is_empty() {
        return Err("automation path rejected: empty path".to_string());
    }
    let abs = absolute(Path::new(path));
    if let Ok(c) = abs.canonicalize() {
        return Ok(c);
    }
    let mut existing = abs.as_path();
    let mut rest: Vec<&std::ffi::OsStr> = Vec::new();
    loop {
        if existing.symlink_metadata().is_ok() {
            break;
        }
        let Some(name) = existing.file_name() else { return Err(format!("automation path rejected: no existing folder above {path}")) };
        rest.push(name);
        let Some(parent) = existing.parent() else { return Err(format!("automation path rejected: no existing folder above {path}")) };
        existing = parent;
    }
    // A link that leads nowhere: writing through it would create its target.
    let base = existing.canonicalize().map_err(|_| format!("automation path rejected: a link that leads nowhere: {}", existing.display()))?;
    // The names below it came from `file_name`, which never yields `..`: a `..` below a folder
    // that doesn't exist stops the walk above with an error instead.
    if rest.iter().any(|n| Path::new(n).components().any(|c| !matches!(c, Component::Normal(_)))) {
        return Err(format!("automation path rejected: not a plain name below {}: {path}", existing.display()));
    }
    let mut out = base;
    for name in rest.into_iter().rev() {
        out.push(name);
    }
    Ok(out)
}

fn check(root: Option<&Path>, what: &str, path: &str) -> Result<PathBuf, String> {
    let resolved = resolve(path)?;
    let Some(root) = root else {
        return Err(format!("automation filesystem access is not granted: {what} authority is absent: {}", resolved.display()));
    };
    if resolved.starts_with(root) {
        Ok(resolved)
    } else {
        Err(format!("automation path rejected: outside the {what} root {}: {}", root.display(), resolved.display()))
    }
}
