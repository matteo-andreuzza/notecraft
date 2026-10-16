//! Offscreen screenshots of the main UI states (no window, no focus stealing): AGENTS.md › Verify
//! UI changes visually.
//!
//! ```sh
//! cargo run -p notecraft-ui-egui --example snapshot -- [out_dir]   # default target/screenshots
//! ```
//!
//! Renders the sample notebook with each ribbon tab in the simplified and the classic layout, a
//! narrow window (the overflow menu), the Tags Summary pane, the backstage, the
//! White, Dark Gray and Black themes, full page view, a 2× HiDPI frame, a page with a table (the
//! Table Layout tab in both layouts, selected cells) and a hovered paragraph handle, and writes
//! one PNG per state. Set `CRAFT_FONTS_DIR` to a folder with Inter to see the preferred interface font.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use egui_kittest::Harness;
use notecraft_engine::{Clock, Session};
use notecraft_ui_egui::{Backstage, FixedHost, NoteApp, ThemeKind};
use serde_json::json;

fn app() -> NoteApp {
    let clock = Clock { now_ms: 1_792_007_100_000, utc_offset_min: 120 };
    let mut s = Session::with_sample(clock);
    s.prefs.author = "Sample".into();
    NoteApp::new(s, Box::new(FixedHost(clock)))
}

fn shot(name: &str, out: &std::path::Path, ppp: f32, setup: impl FnOnce(&mut NoteApp)) {
    shot_sized(name, out, ppp, egui::vec2(1366.0, 820.0), setup);
}

fn shot_sized(name: &str, out: &std::path::Path, ppp: f32, size: egui::Vec2, setup: impl FnOnce(&mut NoteApp)) {
    shot_then(name, out, ppp, size, setup, |_| {});
}

/// A shot with something done to the running harness first (hovering, for example).
fn shot_then(
    name: &str,
    out: &std::path::Path,
    ppp: f32,
    size: egui::Vec2,
    setup: impl FnOnce(&mut NoteApp),
    then: impl FnOnce(&mut Harness<'static, NoteApp>),
) {
    let mut a = app();
    setup(&mut a);
    let mut h = Harness::builder().with_size(size).with_pixels_per_point(ppp).wgpu().build_ui_state(|ui, app: &mut NoteApp| app.ui(ui), a);
    h.run_steps(4);
    then(&mut h);
    h.run_steps(2);
    match h.render() {
        Ok(img) => {
            let p = out.join(format!("{name}.png"));
            img.save(&p).unwrap();
            println!("wrote {}", p.display());
        }
        Err(e) => eprintln!("{name}: render failed: {e}"),
    }
}

/// A new page with a heading paragraph and a filled 4 × 3 table; the caret ends in a cell.
fn table_page(a: &mut NoteApp) {
    let s = &mut a.session;
    s.run("page.new", json!({"title": "Project plan"})).unwrap();
    s.run("outline.new", json!({"x": 36.0, "y": 90.0})).unwrap();
    s.run("text.insert", json!({"text": "Tasks for this week"})).unwrap();
    s.run("insert.table", json!({"rows": 4, "cols": 3, "x": 36.0, "y": 118.0})).unwrap();
    let rows = [["Task", "Owner", "Due"], ["Draft the brief", "Ana", "Mon"], ["Review the budget", "Luca", "Wed"], ["Book the room", "Sam", "Fri"]];
    for (r, row) in rows.iter().enumerate() {
        for (c, text) in row.iter().enumerate() {
            s.run("text.insert", json!({"text": text})).unwrap();
            if !(r == 3 && c == 2) {
                s.run("table.nextCell", json!({})).unwrap();
            }
        }
    }
    s.run("table.selectRange", json!({"table": table_id(s), "rows": [0, 0], "cols": [0, 2]})).unwrap();
    s.run("table.shading", json!({"color": "#DEEBF7"})).unwrap();
    s.run("format.bold", json!({})).unwrap();
    let cell = s.page().unwrap().objects.iter().find_map(|o| o.as_table()).unwrap().rows[2].cells[1].id;
    s.run("caret.set", json!({"block": cell.to_string(), "para": 0, "offset": 4})).unwrap();
    a.canvas.focused = true;
}

fn table_id(s: &Session) -> String {
    s.page().unwrap().objects.iter().find_map(|o| o.as_table()).unwrap().id.to_string()
}

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "target/screenshots".into()));
    std::fs::create_dir_all(&out).unwrap();
    for (prefix, mode) in [("tab", "simplified"), ("classic", "classic")] {
        for tab in ["Home", "Insert", "Draw", "History", "Review", "View", "Help"] {
            shot(&format!("{prefix}-{}", tab.to_lowercase()), &out, 1.0, |a| {
                a.ui.tab = tab.into();
                a.session.run("ui.ribbonMode", json!({"mode": mode})).unwrap();
                // Open the meeting page with a caret so the formatting controls are live.
                let pid = a.session.notebook().unwrap().sections()[1].pages[0].id;
                a.session.run("nav.goto", json!({"page": pid.to_string()})).unwrap();
                a.session.run("nav.pageTitle", json!({})).unwrap();
            });
        }
    }
    shot_sized("narrow-home", &out, 1.0, egui::vec2(820.0, 640.0), |_| {});
    shot("nav-hidden", &out, 1.0, |a| {
        a.session.run("view.navigationPanes", json!({})).unwrap();
    });
    shot("tags-summary", &out, 1.0, |a| {
        a.session.view.tags_pane = true;
    });
    shot("backstage-info", &out, 1.0, |a| a.ui.backstage = Some(Backstage::Info));
    shot("backstage-options", &out, 1.0, |a| a.ui.backstage = Some(Backstage::Options));
    for (name, kind) in [("theme-white", ThemeKind::White), ("theme-dark-gray", ThemeKind::DarkGray), ("theme-black", ThemeKind::Black)] {
        shot(name, &out, 1.0, |a| a.ui.theme = kind);
    }
    shot("full-page", &out, 1.0, |a| a.session.view.full_page = true);
    shot("hidpi-2x", &out, 2.0, |_| {});
    shot("rule-lines", &out, 1.0, |a| {
        a.session.run("view.ruleLines", json!({"kind": "college"})).unwrap();
    });
    shot("table-page", &out, 1.0, table_page);
    for (name, mode) in [("table-layout", "simplified"), ("classic-table-layout", "classic")] {
        shot(name, &out, 1.0, |a| {
            table_page(a);
            a.session.run("ui.ribbonMode", json!({"mode": mode})).unwrap();
            a.ui.tab = "Table Layout".into();
        });
    }
    shot("table-cells-selected", &out, 1.0, |a| {
        table_page(a);
        a.session.run("table.selectColumns", json!({})).unwrap();
        a.session.run("table.hideBorders", json!({})).unwrap();
        a.ui.tab = "Table Layout".into();
    });
    shot_then(
        "mini-toolbar",
        &out,
        1.0,
        egui::vec2(1366.0, 820.0),
        |a| {
            let pid = a.session.notebook().unwrap().sections()[1].pages[0].id;
            a.session.run("nav.goto", json!({"page": pid.to_string()})).unwrap();
            let oid = a.session.page().unwrap().objects[0].id();
            a.session.run("caret.set", json!({"block": oid.to_string(), "para": 0, "offset": 0})).unwrap();
            a.session.run("caret.set", json!({"block": oid.to_string(), "para": 0, "offset": 20, "extend": true})).unwrap();
            a.canvas.focused = true;
        },
        |_| {},
    );
    shot_then(
        "context-menu-text",
        &out,
        1.0,
        egui::vec2(1366.0, 820.0),
        |a| {
            let pid = a.session.notebook().unwrap().sections()[1].pages[0].id;
            a.session.run("nav.goto", json!({"page": pid.to_string()})).unwrap();
            a.canvas.focused = true;
        },
        |h| {
            let pl =
                h.state().canvas.paras.iter().find(|p| matches!(p.block, notecraft_engine::selection::Block::Outline(_)) && p.para == 1).cloned();
            if let Some(pl) = pl {
                let pos = pl.pos + egui::vec2(6.0, 4.0);
                h.event(egui::Event::PointerMoved(pos));
                for pressed in [true, false] {
                    h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
                }
            }
        },
    );
    shot_then(
        "paragraph-handle",
        &out,
        1.0,
        egui::vec2(1366.0, 820.0),
        |a| {
            let pid = a.session.notebook().unwrap().sections()[1].pages[0].id;
            a.session.run("nav.goto", json!({"page": pid.to_string()})).unwrap();
        },
        |h| {
            let pl =
                h.state().canvas.paras.iter().find(|p| matches!(p.block, notecraft_engine::selection::Block::Outline(_)) && p.para == 1).cloned();
            if let Some(pl) = pl {
                h.event(egui::Event::PointerMoved(pl.pos + egui::vec2(6.0, 4.0)));
            }
        },
    );
}
