//! M1 acceptance (docs/plan.md): the four core workflows, driven headlessly the way an agent
//! drives NoteCraft (MCP tools over a headless session on a folder notebook), then checked on
//! a fresh session that reads the notebook back from disk.
//!
//! 1. Meeting notes: a page with a title, a heading, a bulleted agenda and bold text.
//! 2. A task list with To Do tags, one checked; Find Tags lists the unchecked ones.
//! 3. Find a note by search and open it.
//! 4. Reorganise pages: reorder, make a subpage, move a page to another section, delete and
//!    restore from the Notebook Recycle Bin.

use notecraft_automation::Headless;
use notecraft_automation::mcp::Server;
use notecraft_engine::{Clock, Session};
use serde_json::{Value, json};

fn clock() -> Clock {
    Clock { now_ms: 1_792_007_100_000, utc_offset_min: 60 }
}

struct Agent {
    srv: Server,
    next: i64,
}

impl Agent {
    fn new() -> Agent {
        Agent { srv: Server::new(Box::new(Headless::new(Session::new(clock()), Box::new(clock)))), next: 1 }
    }

    /// Call a tool; panics with the tool's error text when it fails.
    fn tool(&mut self, name: &str, args: Value) -> Value {
        self.next += 1;
        let v =
            self.srv.handle(json!({"jsonrpc": "2.0", "id": self.next, "method": "tools/call", "params": {"name": name, "arguments": args}})).unwrap();
        let r = &v["result"];
        let text = r["content"][0]["text"].as_str().unwrap_or("").to_string();
        assert_eq!(r["isError"], false, "{name} {args}: {text}");
        serde_json::from_str(&text).unwrap_or(Value::String(text))
    }

    fn exec(&mut self, command: &str, params: Value) -> Value {
        self.tool("execute", json!({"command": command, "params": params}))
    }

    fn page_text(&mut self) -> String {
        self.tool("get_page_text", json!({})).as_str().unwrap_or("").to_string()
    }

    fn inspect(&mut self) -> Value {
        self.tool("inspect", json!({}))
    }
}

/// Titles of the pages of the section named `section` in an inspect tree, with their levels.
fn pages_of(tree: &Value, section: &str) -> Vec<(String, u64)> {
    fn walk(nodes: &Value, section: &str) -> Option<Vec<(String, u64)>> {
        for n in nodes.as_array()? {
            if n["kind"] == "section" && n["name"] == section {
                return Some(
                    n["pages"]
                        .as_array()?
                        .iter()
                        .map(|p| (p["title"].as_str().unwrap_or("").to_string(), p["level"].as_u64().unwrap_or(0)))
                        .collect(),
                );
            }
            if let Some(found) = walk(&n["children"], section) {
                return Some(found);
            }
        }
        None
    }
    tree["notebooks"].as_array().unwrap().iter().find_map(|nb| walk(&nb["children"], section)).unwrap_or_default()
}

fn section_id(tree: &Value, name: &str) -> String {
    fn walk(nodes: &Value, name: &str) -> Option<String> {
        for n in nodes.as_array()? {
            if n["kind"] == "section" && n["name"] == name {
                return n["id"].as_str().map(str::to_string);
            }
            if let Some(found) = walk(&n["children"], name) {
                return Some(found);
            }
        }
        None
    }
    tree["notebooks"].as_array().unwrap().iter().find_map(|nb| walk(&nb["children"], name)).unwrap()
}

#[test]
fn m1_core_workflows_headless() {
    let dir = std::env::temp_dir().join(format!("notecraft-m1-workflows-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut a = Agent::new();
    a.tool("new_notebook", json!({"name": "Work", "path": dir.display().to_string()}));
    a.exec("section.rename", json!({"name": "Meetings"}));

    // 1. Meeting notes.
    a.tool("new_page", json!({"title": "Weekly sync"}));
    a.tool("type_text", json!({"text": "\nAgenda"}));
    a.exec("style.heading2", json!({}));
    a.tool("type_text", json!({"text": "\n* Budget review\nHiring plan\nOffsite dates"}));
    a.exec("text.newline", json!({}));
    a.exec("text.newline", json!({}));
    a.exec("format.bold", json!({}));
    a.tool("type_text", json!({"text": "Decision: ship on Friday"}));
    let text = a.page_text();
    assert!(text.starts_with("# Weekly sync\n"), "{text}");
    for line in ["Agenda", "● Budget review", "● Hiring plan", "● Offsite dates", "Decision: ship on Friday"] {
        assert!(text.contains(line), "meeting notes: missing {line:?} in\n{text}");
    }
    let page = &a.tool("inspect", json!({"runs": true}))["page"];
    let paras = page["objects"][0]["paragraphs"].as_array().unwrap();
    assert_eq!(paras[0]["style"], "Heading2", "{}", paras[0]);
    let last = paras.last().unwrap();
    assert!(last["list"].as_str().unwrap_or("").is_empty(), "Enter on an empty bullet ended the list");
    assert!(last["runs"].as_array().unwrap().iter().all(|r| r["fmt"]["bold"] == true), "{last}");

    // 2. A task list with To Do tags.
    a.tool("new_page", json!({"title": "Launch tasks"}));
    a.tool("type_text", json!({"text": "\nWrite release notes"}));
    a.exec("tag.todo", json!({}));
    // Enter after a To Do item starts another, unchecked (as in OneNote).
    a.tool("type_text", json!({"text": "\nTag the build"}));
    a.exec("tag.check", json!({"key": "todo"}));
    a.tool("type_text", json!({"text": "\nTell support"}));
    let text = a.page_text();
    assert!(
        text.contains("[To Do] Write release notes") && text.contains("[To Do ✓] Tag the build") && text.contains("[To Do] Tell support"),
        "{text}"
    );
    let open = a.tool("find_tags", json!({"unchecked": true, "scope": "notebook"}));
    let todo: Vec<&str> = open["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|g| g["group"] == "To Do")
        .flat_map(|g| g["items"].as_array().unwrap())
        .filter_map(|i| i["text"].as_str())
        .collect();
    assert_eq!(todo, vec!["Write release notes", "Tell support"], "only unchecked To Do items");

    // 3. Find a note by search, then open it.
    let found = a.tool("search", json!({"query": "budg", "scope": "all"}));
    let hit = &found["results"][0];
    assert_eq!(hit["title"], "Weekly sync", "{found}");
    a.tool("go_to_page", json!({"page": hit["page"]}));
    assert!(a.page_text().starts_with("# Weekly sync"));

    // 4. Reorganise pages.
    a.tool("new_page", json!({"title": "Notes A"}));
    a.tool("new_page", json!({"title": "Notes B"}));
    let tree = a.inspect();
    let titles: Vec<String> = pages_of(&tree, "Meetings").into_iter().map(|(t, _)| t).collect();
    let first_titles: Vec<&str> = titles.iter().map(String::as_str).filter(|t| *t != "Untitled page").collect();
    assert_eq!(first_titles, vec!["Weekly sync", "Launch tasks", "Notes A", "Notes B"]);
    // Notes B first, then make Notes A a subpage of Launch tasks.
    let b = a.inspect()["page"]["id"].as_str().unwrap().to_string();
    a.exec("page.move", json!({"page": b, "index": 0}));
    a.tool("go_to_page", json!({"title": "Notes A"}));
    a.exec("page.demote", json!({}));
    let pages = pages_of(&a.inspect(), "Meetings");
    let order: Vec<(&str, u64)> = pages.iter().map(|(t, l)| (t.as_str(), *l)).filter(|(t, _)| *t != "Untitled page").collect();
    assert_eq!(order, vec![("Notes B", 0), ("Weekly sync", 0), ("Launch tasks", 0), ("Notes A", 1)]);
    // Move Notes B to a new section.
    a.exec("section.new", json!({"name": "Archive"}));
    let tree = a.inspect();
    let archive = section_id(&tree, "Archive");
    a.tool("go_to_page", json!({"title": "Notes B"}));
    a.exec("page.moveOrCopy", json!({"section": archive}));
    let tree = a.inspect();
    assert!(pages_of(&tree, "Meetings").iter().all(|(t, _)| t != "Notes B"));
    assert!(pages_of(&tree, "Archive").iter().any(|(t, _)| t == "Notes B"));
    // Delete a page, then restore it from the Notebook Recycle Bin.
    a.tool("go_to_page", json!({"title": "Weekly sync"}));
    let weekly = a.inspect()["page"]["id"].as_str().unwrap().to_string();
    a.exec("page.delete", json!({}));
    assert!(pages_of(&a.inspect(), "Meetings").iter().all(|(t, _)| t != "Weekly sync"));
    a.exec("history.restore", json!({"item": weekly}));
    assert!(pages_of(&a.inspect(), "Meetings").iter().any(|(t, _)| t == "Weekly sync"));

    // Everything is on disk: a fresh session sees the same notebook.
    let mut fresh = Session::new(clock());
    fresh.open_path(&dir.display().to_string()).unwrap();
    let tree = notecraft_automation::inspect::inspect(&fresh, &json!({})).unwrap();
    let mut meetings: Vec<(String, u64)> = pages_of(&tree, "Meetings").into_iter().filter(|(t, _)| t != "Untitled page").collect();
    meetings.sort();
    assert_eq!(meetings, vec![("Launch tasks".to_string(), 0), ("Notes A".to_string(), 1), ("Weekly sync".to_string(), 0)]);
    assert!(pages_of(&tree, "Archive").iter().any(|(t, _)| t == "Notes B"));
    let weekly_page =
        fresh.ws.notebooks[0].sections().iter().flat_map(|s| s.pages.iter()).find(|p| p.display_title() == "Weekly sync").unwrap().clone();
    let text = notecraft_automation::inspect::page_text(&weekly_page);
    assert!(text.contains("● Hiring plan") && text.contains("Decision: ship on Friday"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}
