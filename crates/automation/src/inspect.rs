//! The session as JSON for agents: the notebook tree, the selection, and a page's content
//! (`notebook.inspect` on the control channel, the `inspect` MCP tool).

use notecraft_engine::Session;
use notecraft_engine::doc::{Id, NavNode, Page, PageObject};
use serde_json::{Value, json};

/// Pages listed per section before the list is cut short (`truncated: true`).
const MAX_PAGES_LISTED: usize = 2000;

/// Inspect the session. Params: `page` (id; default the current page), `runs` (include each
/// paragraph's formatted runs), `tree` (default true: include the notebook tree).
pub fn inspect(s: &Session, params: &Value) -> Result<Value, String> {
    let runs = params.get("runs").and_then(Value::as_bool).unwrap_or(false);
    let tree = params.get("tree").and_then(Value::as_bool).unwrap_or(true);
    let page = match params.get("page").and_then(Value::as_str) {
        Some(id) => {
            let id = Id::parse(id).ok_or_else(|| format!("bad page id `{id}`"))?;
            Some(s.ws.page(id).ok_or_else(|| format!("no page {id}"))?.as_ref())
        }
        None => s.page().map(|p| p.as_ref()),
    };
    let mut out = json!({
        "selection": serde_json::to_value(&s.sel).unwrap_or(Value::Null),
        "page": page.map(|p| page_json(p, runs)),
        "view": serde_json::to_value(&s.view).unwrap_or(Value::Null),
        "canUndo": s.can_undo(),
        "canRedo": s.can_redo(),
        "undo": s.undo_labels(),
        "dirty": s.is_dirty(),
        "revision": s.revision,
    });
    if tree {
        out["notebooks"] = Value::Array(
            s.ws.notebooks
                .iter()
                .map(|nb| {
                    json!({
                        "id": nb.id,
                        "name": nb.name,
                        "current": s.sel.notebook == Some(nb.id),
                        "path": notebook_path(s, nb.id),
                        "children": nodes(&nb.children, s),
                        "recycleBin": {"pages": nb.recycle.pages.len(), "sections": nb.recycle.sections.len()},
                    })
                })
                .collect(),
        );
    }
    Ok(out)
}

/// The folder a notebook is stored in (none on the web, where notebooks live in memory).
fn notebook_path(s: &Session, id: Id) -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    return s.notebook_path(id).map(|p| p.display().to_string());
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (s, id);
        None
    }
}

fn nodes(children: &[NavNode], s: &Session) -> Value {
    Value::Array(
        children
            .iter()
            .map(|n| match n {
                NavNode::Section(sec) => json!({
                    "kind": "section",
                    "id": sec.id,
                    "name": sec.name,
                    "color": sec.color.0,
                    "current": s.sel.section == Some(sec.id),
                    "pages": sec.pages.iter().take(MAX_PAGES_LISTED).map(|p| json!({"id": p.id, "title": p.display_title(), "level": p.level})).collect::<Vec<_>>(),
                    "truncated": sec.pages.len() > MAX_PAGES_LISTED,
                }),
                NavNode::Group(g) => json!({"kind": "group", "id": g.id, "name": g.name, "children": nodes(&g.children, s)}),
            })
            .collect(),
    )
}

/// One page: title, dates, setup and every note container's paragraphs.
pub fn page_json(p: &Page, runs: bool) -> Value {
    let outlines: Vec<Value> = p
        .objects
        .iter()
        .map(|o| match o {
            PageObject::Outline(ol) => {
                let labels = notecraft_engine::text::list_labels(&ol.paragraphs);
                let paras: Vec<Value> = ol
                    .paragraphs
                    .iter()
                    .zip(labels)
                    .enumerate()
                    .map(|(i, (para, label))| {
                        let mut v = json!({
                            "index": i,
                            "text": para.text(),
                            "level": para.level,
                            "style": serde_json::to_value(para.style).unwrap_or(Value::Null),
                            "align": serde_json::to_value(para.align).unwrap_or(Value::Null),
                            "list": label,
                            "tags": para.tags.iter().map(|t| json!({"key": t.key, "name": t.name, "checked": t.checked})).collect::<Vec<_>>(),
                        });
                        if runs {
                            v["runs"] = serde_json::to_value(&para.runs).unwrap_or(Value::Null);
                        }
                        v
                    })
                    .collect();
                json!({"kind": "outline", "id": ol.id, "x": ol.pos.x, "y": ol.pos.y, "width": serde_json::to_value(ol.width).unwrap_or(Value::Null), "paragraphs": paras})
            }
            PageObject::Table(t) => {
                let rows: Vec<Value> = t
                    .rows
                    .iter()
                    .map(|r| {
                        let cells: Vec<Value> = r
                            .cells
                            .iter()
                            .map(|c| json!({"id": c.id, "text": c.text(), "shading": c.shading.map(|x| x.to_hex())}))
                            .collect();
                        json!({"id": r.id, "cells": cells})
                    })
                    .collect();
                json!({"kind": "table", "id": t.id, "x": t.pos.x, "y": t.pos.y, "columns": t.columns, "borders": t.borders, "headerRow": t.header_row, "rows": rows})
            }
        })
        .collect();
    json!({
        "id": p.id,
        "title": p.title_text(),
        "level": p.level,
        "created": p.created.0,
        "modified": p.modified.0,
        "author": p.author,
        "ruleLines": serde_json::to_value(p.setup.rule_lines).unwrap_or(Value::Null),
        "hideTitle": p.setup.hide_title,
        "objects": outlines,
    })
}

/// A page as plain text: `# Title`, then each paragraph indented by level with its tags and
/// list label (what `notecraft-cli run --print` shows).
pub fn page_text(p: &Page) -> String {
    let mut out = format!("# {}\n", p.display_title());
    for o in &p.objects {
        let ol = match o {
            PageObject::Outline(ol) => ol,
            PageObject::Table(t) => {
                // A table prints as tab-separated rows.
                out.push_str(&t.plain_text());
                out.push_str("\n\n");
                continue;
            }
        };
        let labels = notecraft_engine::text::list_labels(&ol.paragraphs);
        for (para, label) in ol.paragraphs.iter().zip(labels) {
            let tags: String = para.tags.iter().map(|t| format!("[{}{}]", t.name, if t.checked { " ✓" } else { "" })).collect();
            let tags = if tags.is_empty() { tags } else { format!("{tags} ") };
            let label = if label.is_empty() { label } else { format!("{label} ") };
            out.push_str(&format!("{}{tags}{label}{}\n", "  ".repeat(para.level as usize), para.text()));
        }
        out.push('\n');
    }
    out
}
