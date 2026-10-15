//! UI tests: the window renders headlessly in every major state without panicking, ribbon
//! controls dispatch their commands, and the keymap reaches the documented shortcuts.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use notecraft_engine::{Clock, Session};
use serde_json::json;

use crate::{FixedHost, NoteApp};

pub(crate) fn clock() -> Clock {
    Clock { now_ms: 1_792_007_100_000, utc_offset_min: 0 }
}

pub(crate) fn app() -> NoteApp {
    let mut s = Session::with_sample(clock());
    s.prefs.author = "Tester".into();
    NoteApp::new(s, Box::new(FixedHost(clock())))
}

fn harness(app: NoteApp) -> Harness<'static, NoteApp> {
    Harness::builder().with_size(egui::vec2(1280.0, 800.0)).build_ui_state(|ui, app: &mut NoteApp| app.ui(ui), app)
}

#[test]
fn every_tab_and_view_renders() {
    // parity: UI.Ribbon.Tabs, UI.Nav.NotebookPane, UI.Nav.SectionTabs, UI.Nav.PageList, UI.Canvas.Page
    let mut h = harness(app());
    h.run();
    for tab in ["Home", "Insert", "Draw", "History", "Review", "View", "Help"] {
        h.state_mut().ui.tab = tab.to_string();
        h.run();
    }
    h.state_mut().session.view.tags_pane = true;
    h.run();
    h.state_mut().session.view.full_page = true;
    h.run();
    h.state_mut().session.view.full_page = false;
    h.state_mut().ui.backstage = Some(crate::Backstage::Options);
    h.run();
    h.state_mut().ui.backstage = None;
    h.state_mut().ui.theme = crate::ThemeKind::Dark;
    h.run();
}

#[test]
fn typing_through_the_keyboard_reaches_the_page() {
    // parity: NC.Text.Insert, UI.Canvas.Container
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    a.session.run("outline.new", json!({"x": 60.0, "y": 120.0})).unwrap();
    a.canvas.focused = true;
    let mut h = harness(a);
    h.run();
    h.event(egui::Event::Text("Hello".into()));
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
    h.event(egui::Event::Text(" bold".into()));
    h.run();
    let text = h.state().session.page().unwrap().plain_text();
    assert!(text.contains("Hello bold"), "{text}");
    let p = &h.state().session.page().unwrap().objects[0].as_outline().unwrap().paragraphs[0];
    assert!(p.runs.iter().any(|r| r.text.contains("bold") && r.fmt.bold == Some(true)), "Ctrl+B applied to typed text");
}

#[test]
fn ribbon_buttons_dispatch_commands() {
    // parity: UI.Ribbon.Galleries
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    // The caret starts in the new page's title.
    a.session.run("text.insert", json!({"text": "Agenda"})).unwrap();
    a.session.run("edit.selectAll", json!({})).unwrap();
    let mut h = harness(a);
    h.run();
    h.get_by_label("Bold").click();
    h.run();
    let title = &h.state().session.page().unwrap().title;
    assert_eq!(title.text(), "Agenda");
    assert!(title.runs.iter().all(|r| r.fmt.bold == Some(true)), "the ribbon's Bold ran format.bold on the selection");
    // A tag from the in-ribbon gallery.
    h.state_mut().session.run("nav.pageTitle", json!({})).unwrap();
    h.run();
    h.get_by_label("Find Tags").click();
    h.run();
    assert!(h.state().session.view.tags_pane, "Find Tags opens the Tags Summary pane");
}

#[test]
fn try_run_hands_errors_back_and_run_shows_them() {
    let ctx = egui::Context::default();
    let mut a = app();
    let v = a.try_run(&ctx, "page.new", json!({"title": "From control"})).unwrap();
    assert!(v.get("page").is_some());
    assert_eq!(a.session.page().unwrap().display_title(), "From control");
    let e = a.try_run(&ctx, "no.such.command", json!({})).unwrap_err();
    assert!(e.contains("no.such.command"), "{e}");
    assert!(a.ui.status.is_none(), "try_run leaves the status bar alone");
    assert!(a.run(&ctx, "no.such.command", json!({})).is_none());
    assert!(a.ui.status.is_some());
    let ui = a.inspect_ui(&ctx);
    assert_eq!(ui["tab"], "Home");
    assert!(ui["status"].is_string());
}
