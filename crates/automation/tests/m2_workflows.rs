//! M2 acceptance (docs/plan.md §2): the two Insert and Draw workflows, driven headlessly the way
//! an agent drives NoteCraft (MCP tools over a headless session on a folder notebook), then
//! checked on a fresh session that reads the notebook back from disk.
//!
//! 1. A page with a table and pictures: a table filled with Tab (rows added in the last cell),
//!    a header row, sorted, the header shaded; a picture from bytes resized with alt text, a
//!    picture from a file turned, and a file attachment.
//! 2. Handwritten notes: pen strokes, a shape, a straight line along the ruler, a lasso
//!    selection tagged as a to-do and moved, and the stroke eraser.

mod common;

use common::{Agent, notebook_dir, reopen};
use notecraft_automation::base64;
use serde_json::{Value, json};

/// A 16×8 red PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x08,
    0x08, 0x02, 0x00, 0x00, 0x00, 0x7f, 0x14, 0xe8, 0xc0, 0x00, 0x00, 0x00, 0x13, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x38, 0xa1, 0xa1, 0x41,
    0x12, 0x62, 0x18, 0xd5, 0x40, 0x0b, 0x0d, 0x00, 0x87, 0xcd, 0x8c, 0x01, 0xa1, 0x78, 0x34, 0x71, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

/// The objects of `kind` in an inspected page.
fn objects<'a>(page: &'a Value, kind: &str) -> Vec<&'a Value> {
    page["objects"].as_array().unwrap().iter().filter(|o| o["kind"] == kind).collect()
}

/// The object with this id in an inspected page.
fn object<'a>(page: &'a Value, id: &str) -> &'a Value {
    page["objects"].as_array().unwrap().iter().find(|o| o["id"] == id).unwrap_or_else(|| panic!("no object {id}"))
}

fn id_of(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_else(|| panic!("no {key} in {v}")).to_string()
}

/// The page as a fresh session reads it back from the notebook at `dir`.
fn page_on_disk(dir: &std::path::Path, title: &str) -> Value {
    let (fresh, id) = reopen(dir, title);
    notecraft_automation::inspect::inspect(&fresh, &json!({"page": id, "tree": false})).unwrap()["page"].clone()
}

#[test]
fn m2_a_page_with_a_table_and_pictures() {
    let dir = notebook_dir("m2-table-pictures");
    let mut a = Agent::new();
    a.tool("new_notebook", json!({"name": "Travel", "path": dir.display().to_string()}));
    a.exec("section.rename", json!({"name": "Trips"}));
    a.tool("new_page", json!({"title": "Packing list"}));
    a.tool("type_text", json!({"text": "\nPack by Thursday."}));

    // The table: Tab moves cell to cell, and in the last cell adds a row.
    let table = id_of(&a.exec("insert.table", json!({"rows": 2, "cols": 3})), "table");
    let cells = ["Item", "Qty", "Packed", "Tent", "1", "yes", "Map", "1", "yes", "Boots", "2", "no"];
    for (i, text) in cells.iter().enumerate() {
        if i > 0 {
            a.exec("table.nextCell", json!({}));
        }
        a.tool("type_text", json!({"text": text}));
    }
    a.exec("table.headerRow", json!({}));
    // Sort by the first column (the header row stays on top), then shade the header.
    a.exec("table.selectRange", json!({"table": table, "rows": [1, 1], "cols": [0, 0]}));
    a.exec("table.sortAscending", json!({}));
    a.exec("table.selectRange", json!({"table": table, "rows": [0, 0], "cols": [0, 2]}));
    a.exec("table.shading", json!({"color": "#DDEBF7"}));

    // Pictures beside it: one from bytes, resized with alt text; one from a file, turned.
    let tent = id_of(&a.exec("insert.picture", json!({"data": base64::encode(PNG), "x": 460, "y": 100})), "picture");
    a.exec("object.resize", json!({"width": 160}));
    a.exec("object.altText", json!({"text": "The red tent"}));
    let file = dir.with_extension("png");
    std::fs::write(&file, PNG).unwrap();
    let map = id_of(&a.exec("insert.picture", json!({"path": file.display().to_string(), "x": 460, "y": 220})), "picture");
    a.exec("object.rotateRight90", json!({}));
    let tips =
        id_of(&a.exec("insert.file", json!({"data": base64::encode(b"Bring a torch.\n"), "name": "tips.txt", "x": 460, "y": 380})), "attachment");

    let text = a.page_text();
    for line in ["# Packing list", "Pack by Thursday.", "Item\tQty\tPacked\nBoots\t2\tno\nMap\t1\tyes\nTent\t1\tyes", "The red tent"] {
        assert!(text.contains(line), "missing {line:?} in\n{text}");
    }
    let live = a.inspect()["page"].clone();
    let t = objects(&live, "table")[0];
    assert_eq!(t["headerRow"], true);
    let rows = t["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 4, "Tab in the last cell added two rows");
    assert!(rows[0]["cells"].as_array().unwrap().iter().all(|c| c["shading"] == "#ddebf7"), "{t}");
    assert!(rows[1]["cells"][0]["shading"].is_null());
    let p = object(&live, &tent);
    assert_eq!((p["width"].as_f64(), p["height"].as_f64(), p["pixels"].clone()), (Some(160.0), Some(80.0), json!([16, 8])), "{p}");
    assert_eq!(p["altText"], "The red tent");
    assert_eq!(object(&live, &map)["quarterTurns"], 1);
    let att = object(&live, &tips);
    assert_eq!((att["name"].as_str(), att["bytes"].as_u64()), (Some("tips.txt"), Some(15)));

    // On disk: the same page, the pictures' and the attachment's bytes included.
    let disk = page_on_disk(&dir, "Packing list");
    assert_eq!(disk["objects"], live["objects"], "the page read back from disk");
    assert!(disk["objects"].as_array().unwrap().iter().all(|o| o["missing"] != true), "no blob went missing");
    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn m2_handwritten_notes() {
    let dir = notebook_dir("m2-handwritten");
    let mut a = Agent::new();
    a.tool("new_notebook", json!({"name": "Sketchbook", "path": dir.display().to_string()}));
    a.exec("section.rename", json!({"name": "Ideas"}));
    a.tool("new_page", json!({"title": "Garden plan"}));

    // Two handwritten words with the second pen, and a box beside them.
    a.exec("draw.pen", json!({"index": 1}));
    let word1 = id_of(&a.exec("draw.stroke", json!({"points": [[80, 160], [90, 140], [100, 170, 0.8], [110, 145], [120, 165]]})), "ink");
    let word2 = id_of(&a.exec("draw.stroke", json!({"points": [[140, 150], [150, 170], [165, 140], [180, 165]]})), "ink");
    let boxed = id_of(&a.exec("draw.shape", json!({"kind": "rectangle", "from": [40, 120], "to": [60, 190]})), "ink");

    // A straight line along the ruler's top edge (y 373), though the hand wobbles.
    a.exec("draw.ruler", json!({"x": 300, "y": 400}));
    let line = id_of(&a.exec("draw.stroke", json!({"points": [[150, 371], [260, 366], [420, 377]], "width": 2.0})), "ink");
    a.exec("draw.ruler", json!({}));

    // Lasso the two words, tag them as a to-do, check it, and move them right.
    let picked = a.exec("draw.lassoSelect", json!({"points": [[75, 130], [190, 130], [190, 180], [75, 180]]}));
    let mut ids: Vec<&str> = picked["ids"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    ids.sort();
    let mut words = [word1.as_str(), word2.as_str()];
    words.sort();
    assert_eq!(ids, words, "{picked}");
    a.exec("tag.todo", json!({}));
    a.exec("tag.click", json!({"block": word1}));
    a.exec("object.move", json!({"dx": 30, "dy": 0}));
    let found = a.tool("find_tags", json!({"scope": "notebook"}));
    let item = &found["groups"][0]["items"][0];
    assert_eq!((item["ink"].as_bool(), item["block"].as_str(), item["checked"].as_bool()), (Some(true), Some(word1.as_str()), Some(true)), "{found}");

    // The stroke eraser takes the box away.
    a.exec("draw.erase", json!({"points": [[40, 150]], "kind": "stroke"}));

    let live = a.inspect()["page"].clone();
    let ink = objects(&live, "ink");
    assert_eq!(ink.len(), 3, "two words and the line; the box is erased");
    assert!(ink.iter().all(|k| k["id"] != boxed.as_str()));
    let w1 = object(&live, &word1);
    assert_eq!(w1["tags"], json!([{"key": "todo", "name": "To Do", "checked": true}]), "decision D5: on the leftmost stroke");
    assert!(object(&live, &word2)["tags"].as_array().unwrap().is_empty());
    assert!((w1["bounds"][0].as_f64().unwrap() - 110.0).abs() < 3.0, "moved 30 pt right: {w1}");
    let l = object(&live, &line);
    let b: Vec<f64> = l["bounds"].as_array().unwrap().iter().filter_map(Value::as_f64).collect();
    // Straight (its height is the pen's width) and laid 1 pt (half the width) above the edge.
    assert!(b[3] <= 2.01 && (b[1] + b[3] / 2.0 - 372.0).abs() < 0.5 && b[2] > 250.0, "the line runs along the ruler: {l}");
    assert_eq!(w1["color"], l["color"], "the same pen");

    // On disk: the same strokes, tags and pen.
    let disk = page_on_disk(&dir, "Garden plan");
    assert_eq!(disk["objects"], live["objects"], "the page read back from disk");
    let _ = std::fs::remove_dir_all(&dir);
}
