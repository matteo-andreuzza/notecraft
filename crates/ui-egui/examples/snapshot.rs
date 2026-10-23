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
//! Table Layout tab in both layouts, selected cells), a hovered paragraph handle, a page with a
//! picture and an attachment (plain, selected, dark, its context menu), and writes
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

/// A synthetic landscape (sky, sun, hills), drawn here so no photo is needed (clean room).
fn landscape(w: u32, h: u32) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let hill = 0.62 + 0.08 * (fx * 9.0).sin() + 0.05 * (fx * 23.0).cos();
            let sun = ((fx - 0.72).powi(2) + (fy - 0.28).powi(2)).sqrt() < 0.09;
            let c = if fy > hill {
                [(60.0 + 40.0 * fy) as u8, (120.0 + 60.0 * (1.0 - fy)) as u8, 70]
            } else if sun {
                [255, 214, 90]
            } else {
                [(120.0 + 100.0 * fy) as u8, (170.0 + 60.0 * fy) as u8, 235]
            };
            px.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    notecraft_codecs::encode_png(&notecraft_codecs::Rgba8::new(w, h, px).unwrap(), &Default::default()).unwrap()
}

/// A new page with a line of text, a picture below it and an attached file beside it; the
/// picture is selected when `select`.
fn picture_page(a: &mut NoteApp, select: bool) {
    let s = &mut a.session;
    s.run("page.new", json!({})).unwrap();
    s.run("text.insert", json!({"text": "Trip photos"})).unwrap();
    s.run("outline.new", json!({"x": 36.0, "y": 100.0})).unwrap();
    s.run("text.insert", json!({"text": "The view from the ridge, and the itinerary:"})).unwrap();
    let r = s.run("insert.picture", json!({"data": notecraft_engine::b64::encode(&landscape(480, 300)), "x": 36.0, "y": 130.0})).unwrap();
    let pic = r["picture"].as_str().unwrap().to_string();
    s.run("object.altText", json!({"text": "Hills under a clear sky"})).unwrap();
    s.run("insert.file", json!({"data": notecraft_engine::b64::encode(b"Day 1: ridge walk"), "name": "Itinerary.pdf", "x": 420.0, "y": 140.0}))
        .unwrap();
    if select {
        s.run("object.select", json!({"ids": [pic]})).unwrap();
    } else {
        s.sel.on_page = notecraft_engine::selection::PageSel::None;
    }
    a.canvas.focused = true;
}

/// A new page with a line of text and ink over it (decision D4): a highlighter across the
/// text, a red pen circling a word, handwriting with varying pressure and a blue arrow; the
/// Draw tab shows with the first pen chosen, and everything is lassoed when `lasso`.
fn ink_page(a: &mut NoteApp, lasso: bool) {
    let s = &mut a.session;
    s.run("page.new", json!({})).unwrap();
    s.run("text.insert", json!({"text": "Sketch"})).unwrap();
    s.run("outline.new", json!({"x": 36.0, "y": 100.0})).unwrap();
    s.run("text.insert", json!({"text": "Ideas for the garden: a bench under the old pear tree."})).unwrap();
    s.sel.on_page = notecraft_engine::selection::PageSel::None;
    let stroke = |s: &mut Session, pen: usize, pts: Vec<[f32; 3]>| {
        s.run("draw.stroke", json!({"pen": pen, "points": pts})).unwrap();
    };
    // Yellow highlighter along the text.
    stroke(s, 3, (0..=40).map(|i| [40.0 + i as f32 * 4.5, 112.0 + (i as f32 * 0.3).sin(), 0.5]).collect());
    // Red circle around "bench".
    stroke(
        s,
        1,
        (0..=64)
            .map(|i| {
                let a = i as f32 / 64.0 * std::f32::consts::TAU * 1.05;
                [258.0 + 30.0 * a.cos(), 110.0 + 13.0 * a.sin(), 0.5]
            })
            .collect(),
    );
    // Handwriting-like loops with pressure.
    stroke(
        s,
        0,
        (0..=160)
            .map(|i| {
                let t = i as f32 / 160.0;
                [60.0 + t * 260.0 + 9.0 * (t * 60.0).sin(), 190.0 - 14.0 * (t * 30.0).sin().abs(), 0.25 + 0.7 * (t * 9.0).sin().abs()]
            })
            .collect(),
    );
    // Blue arrow.
    stroke(s, 2, vec![[340.0, 200.0, 0.6], [430.0, 150.0, 0.6]]);
    stroke(s, 2, vec![[412.0, 148.0, 0.6], [430.0, 150.0, 0.6], [424.0, 168.0, 0.6]]);
    // Shapes are ink too.
    s.run("draw.shape", json!({"kind": "rectangle", "from": [470, 130], "to": [570, 200], "pen": 2})).unwrap();
    s.run("draw.shape", json!({"kind": "oval", "from": [600, 130], "to": [680, 200], "pen": 1})).unwrap();
    s.run("draw.pen", json!({"index": 0})).unwrap();
    // Tags on ink (decision D5): a to-do and a star on the rectangle, a question on the oval.
    s.run("draw.lassoSelect", json!({"points": [[460, 120], [580, 120], [580, 210], [460, 210]]})).unwrap();
    s.run("tag.todo", json!({})).unwrap();
    s.run("tag.important", json!({})).unwrap();
    s.run("draw.lassoSelect", json!({"points": [[590, 120], [690, 120], [690, 210], [590, 210]]})).unwrap();
    s.run("tag.question", json!({})).unwrap();
    s.sel.on_page = notecraft_engine::selection::PageSel::None;
    if lasso {
        s.run("draw.lassoSelect", json!({"points": [[20, 80], [460, 80], [460, 230], [20, 230]]})).unwrap();
    }
    a.ui.tab = "Draw".into();
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
    shot("picture-page", &out, 1.0, |a| picture_page(a, false));
    shot("picture-selected", &out, 1.0, |a| picture_page(a, true));
    shot("picture-selected-dark", &out, 1.0, |a| {
        picture_page(a, true);
        a.ui.theme = ThemeKind::Black;
    });
    shot_then(
        "context-menu-picture",
        &out,
        1.0,
        egui::vec2(1366.0, 820.0),
        |a| {
            picture_page(a, false);
            a.ui.tab = "View".into();
        },
        |h| {
            if let Some(m) = h.state().canvas.media.iter().find(|m| m.picture).copied() {
                let pos = m.rect.center();
                h.event(egui::Event::PointerMoved(pos));
                for pressed in [true, false] {
                    h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
                }
            }
        },
    );
    shot("printout-page", &out, 1.0, |a| {
        let s = &mut a.session;
        s.run("page.new", json!({})).unwrap();
        s.run("text.insert", json!({"text": "Quarterly report"})).unwrap();
        let bytes = notecraft_codecs::pdf::sample_pdf(&[(420.0, 260.0, [0x2b, 0x57, 0x9a]), (420.0, 260.0, [0xc0, 0x50, 0x20])]);
        s.run("insert.printout", json!({"data": notecraft_engine::b64::encode(&bytes), "name": "Report.pdf", "x": 36.0, "y": 100.0})).unwrap();
        s.sel.on_page = notecraft_engine::selection::PageSel::None;
        a.ui.tab = "Insert".into();
    });
    shot("ink-page", &out, 1.0, |a| ink_page(a, false));
    shot("ink-page-classic", &out, 1.0, |a| {
        ink_page(a, false);
        a.session.run("ui.ribbonMode", json!({"mode": "classic"})).unwrap();
    });
    shot("ink-page-dark", &out, 1.0, |a| {
        ink_page(a, false);
        a.ui.theme = ThemeKind::Black;
    });
    shot("ink-lasso", &out, 1.0, |a| ink_page(a, true));
    // Draw › Ruler (decision D7), turned 30° counterclockwise, with a line drawn along it.
    shot("ink-ruler", &out, 1.0, |a| {
        ink_page(a, false);
        a.session.run("draw.ruler", json!({"x": 420, "y": 330, "angle": -30})).unwrap();
        a.session.run("draw.stroke", json!({"points": [[300, 380], [520, 250]]})).unwrap();
    });
    shot("ink-zoom-2x", &out, 2.0, |a| {
        ink_page(a, false);
        a.session.run("view.zoom", json!({"percent": 150})).unwrap();
    });
    for (name, mode) in [("table-layout", "simplified"), ("classic-table-layout", "classic")] {
        shot(name, &out, 1.0, |a| {
            table_page(a);
            a.session.run("ui.ribbonMode", json!({"mode": mode})).unwrap();
            a.ui.tab = "Table Layout".into();
        });
    }
    // Merged cells (decision D9): the header across all three columns, a cell over two rows.
    shot("table-merged", &out, 1.0, |a| {
        table_page(a);
        let t = a.session.page().and_then(|p| p.objects.iter().find_map(|o| o.as_table().map(|t| t.id.to_string()))).unwrap();
        // A row under "Review the budget", then Luca's cell over both rows.
        a.session.run("table.insertBelow", json!({})).unwrap();
        a.session.run("table.selectRange", json!({"table": t, "rows": [0, 0], "cols": [0, 2]})).unwrap();
        a.session.run("table.merge", json!({})).unwrap();
        a.session.run("table.selectRange", json!({"table": t, "rows": [2, 3], "cols": [1, 1]})).unwrap();
        a.session.run("table.merge", json!({})).unwrap();
        a.ui.tab = "Table Layout".into();
    });
    // Table Layout › Sort › Sort…, with the header row's names in the column lists.
    shot("table-sort-dialog", &out, 1.0, |a| {
        table_page(a);
        a.session.run("table.headerRow", json!({})).unwrap();
        a.ui.tab = "Table Layout".into();
        a.ui.dialog = Some(notecraft_ui_egui::dialogs::Dialog::SortTable { by: [Some(1), Some(2)], descending: [false, true], header: true });
    });
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
    shot("paste-options", &out, 1.0, |a| {
        let pid = a.session.notebook().unwrap().sections()[1].pages[0].id;
        a.session.run("nav.goto", json!({"page": pid.to_string()})).unwrap();
        a.canvas.focused = true;
        let oid = a.session.page().unwrap().objects[0].id();
        a.session.run("caret.set", json!({"block": oid.to_string(), "para": 0, "offset": 0})).unwrap();
        a.session.run("edit.paste", json!({"text": "pasted text"})).unwrap();
        a.ui.paste_shown_at = Some(0.0);
    });
}
