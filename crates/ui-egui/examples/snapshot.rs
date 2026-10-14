//! Offscreen screenshots of the main UI states (no window, no focus stealing): AGENTS.md › Verify
//! UI changes visually.
//!
//! ```sh
//! cargo run -p notecraft-ui-egui --example snapshot -- [out_dir]   # default target/screenshots
//! ```
//!
//! Renders the sample notebook with each ribbon tab, the Tags Summary pane, the backstage, the
//! dark theme, full page view and a 2× HiDPI frame, and writes one PNG per state.
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
    let mut a = app();
    setup(&mut a);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1366.0, 820.0))
        .with_pixels_per_point(ppp)
        .wgpu()
        .build_ui_state(|ui, app: &mut NoteApp| app.ui(ui), a);
    h.run_steps(4);
    match h.render() {
        Ok(img) => {
            let p = out.join(format!("{name}.png"));
            img.save(&p).unwrap();
            println!("wrote {}", p.display());
        }
        Err(e) => eprintln!("{name}: render failed: {e}"),
    }
}

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "target/screenshots".into()));
    std::fs::create_dir_all(&out).unwrap();
    for tab in ["Home", "Insert", "Draw", "History", "Review", "View", "Help"] {
        shot(&format!("tab-{}", tab.to_lowercase()), &out, 1.0, |a| {
            a.ui.tab = tab.into();
            // Open the meeting page with a caret so the formatting controls are live.
            let pid = a.session.notebook().unwrap().sections()[1].pages[0].id;
            a.session.run("nav.goto", json!({"page": pid.to_string()})).unwrap();
        });
    }
    shot("tags-summary", &out, 1.0, |a| {
        a.session.view.tags_pane = true;
    });
    shot("backstage-info", &out, 1.0, |a| a.ui.backstage = Some(Backstage::Info));
    shot("backstage-options", &out, 1.0, |a| a.ui.backstage = Some(Backstage::Options));
    shot("dark", &out, 1.0, |a| a.ui.theme = ThemeKind::Dark);
    shot("full-page", &out, 1.0, |a| a.session.view.full_page = true);
    shot("hidpi-2x", &out, 2.0, |_| {});
    shot("rule-lines", &out, 1.0, |a| {
        a.session.run("view.ruleLines", json!({"kind": "college"})).unwrap();
    });
}
