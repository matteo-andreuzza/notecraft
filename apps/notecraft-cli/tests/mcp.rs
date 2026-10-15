//! `notecraft-cli mcp` end to end: JSON-RPC lines piped through the real binary.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

/// Run `notecraft-cli mcp <args>` with `lines` on stdin; the replies, and stderr.
fn mcp(args: &[&str], lines: &[Value]) -> (Vec<Value>, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_notecraft-cli"))
        .arg("mcp")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for l in lines {
        writeln!(stdin, "{l}").unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let replies = String::from_utf8(out.stdout).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    (replies, String::from_utf8_lossy(&out.stderr).into_owned())
}

fn tool(id: i64, name: &str, args: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}})
}

fn text(v: &Value) -> &str {
    v["result"]["content"][0]["text"].as_str().unwrap_or("")
}

#[test]
fn stdout_carries_only_protocol_replies() {
    let (replies, _) = mcp(
        &["--sample"],
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            tool(3, "new_page", json!({"title": "Standup"})),
            tool(4, "type_text", json!({"text": "\nShip the build"})),
            tool(5, "execute", json!({"command": "tag.todo"})),
            tool(6, "get_page_text", json!({})),
        ],
    );
    assert_eq!(replies.len(), 6, "{replies:?}");
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "notecraft");
    assert!(replies[1]["result"]["tools"].as_array().unwrap().iter().all(|t| t["name"] != "screenshot"));
    for r in &replies[2..] {
        assert_eq!(r["result"]["isError"], false, "{r}");
    }
    assert_eq!(text(&replies[5]), "# Standup\n[To Do] Ship the build\n\n");
}

#[test]
fn folder_notebooks_are_saved_and_roots_confine_paths() {
    let base = std::env::temp_dir().join(format!("notecraft-cli-mcp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("allowed")).unwrap();
    let allowed = base.join("allowed");
    let nb = allowed.join("Work");
    let root = allowed.display().to_string();
    let (replies, stderr) = mcp(
        &["--automation-read-root", &root, "--automation-write-root", &root],
        &[
            tool(1, "new_notebook", json!({"name": "Work", "path": nb.display().to_string()})),
            tool(2, "new_page", json!({"title": "Saved by MCP"})),
            tool(3, "new_notebook", json!({"name": "Escape", "path": base.join("Escape").display().to_string()})),
        ],
    );
    assert!(stderr.contains("files confined"), "{stderr}");
    assert_eq!(replies[0]["result"]["isError"], false, "{}", replies[0]);
    assert_eq!(replies[1]["result"]["isError"], false, "{}", replies[1]);
    assert_eq!(replies[2]["result"]["isError"], true);
    assert!(text(&replies[2]).contains("outside the write root"), "{}", replies[2]);
    assert!(!base.join("Escape").exists());
    // The notebook is on disk: the CLI opens it, and a new MCP session sees the page.
    let out = Command::new(env!("CARGO_BIN_EXE_notecraft-cli")).args(["run", "--notebook", &nb.display().to_string(), "--print"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    // Opening shows the section's first page (the starter page); the new one is listed after it.
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("# "));
    let (replies, _) = mcp(&["--notebook", &nb.display().to_string()], &[tool(1, "inspect", json!({}))]);
    assert!(text(&replies[0]).contains("Saved by MCP"), "{}", replies[0]);
}

#[test]
fn bad_option_combinations_fail_before_serving() {
    for args in [&["--connect", "127.0.0.1:9", "--automation-read-root", "/"][..], &["--connect", "127.0.0.1:9", "--sample"], &["--bogus"]] {
        let out = Command::new(env!("CARGO_BIN_EXE_notecraft-cli")).arg("mcp").args(args).stdin(Stdio::null()).output().unwrap();
        assert!(!out.status.success(), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}");
    }
}
