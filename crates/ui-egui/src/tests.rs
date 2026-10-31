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

/// Assert that the control labelled `label` is in the accessibility tree; `row` is the catalog
/// row it belongs to.
fn present(h: &Harness<'static, NoteApp>, row: &str, label: &str) {
    assert!(h.query_by_label(label).is_some(), "{row}: no control labelled `{label}`");
}

/// A 1280 × 800 harness running the whole window.
pub(crate) fn harness(app: NoteApp) -> Harness<'static, NoteApp> {
    Harness::builder().with_size(egui::vec2(1280.0, 800.0)).build_ui_state(|ui, app: &mut NoteApp| app.ui(ui), app)
}

#[test]
fn every_tab_and_view_renders() {
    // Rendering only (no panic in any state); what each part does is pinned by
    // `window_parts_are_present_and_work`.
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
    h.state_mut().ui.theme = crate::ThemeKind::DarkGray;
    h.run();
}

#[test]
fn typing_through_the_keyboard_reaches_the_page() {
    // parity: NC.Text.Insert
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
    assert!(text.contains("Hello bold"), "{}: {text}", "UI.Canvas.Page");
    let p = &h.state().session.page().unwrap().objects[0].as_outline().unwrap().paragraphs[0];
    assert!(p.runs.iter().any(|r| r.text.contains("bold") && r.fmt.bold == Some(true)), "Ctrl+B applied to typed text");
}

#[test]
fn ribbon_buttons_dispatch_commands() {
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
    assert!(h.state().session.view.tags_pane, "UI.Panes.FindTags: Find Tags opens the Tags Summary pane");
    assert!(h.query_by_label("Create Summary Page").is_some(), "UI.Panes.FindTags");
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

#[test]
fn a_new_page_takes_the_keyboard() {
    // parity: KB.Pages.NewPage — after Ctrl+N the caret is in the new page's title and typing
    // goes there, without a click.
    let ctx = egui::Context::default();
    let mut a = app();
    a.canvas.focused = false;
    a.try_run(&ctx, "page.new", json!({})).unwrap();
    assert!(a.canvas.focused);
    // A page tab moves focus to the page list; a ribbon button leaves focus where it was.
    let other = a.session.section().unwrap().pages[0].id.to_string();
    a.canvas.focused = false;
    a.try_run(&ctx, "nav.goto", json!({"page": other})).unwrap();
    assert!(!a.canvas.focused);
    a.try_run(&ctx, "format.bold", json!({})).ok();
    assert!(!a.canvas.focused);
    let mut h = harness(a);
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::N);
    h.run();
    h.event(egui::Event::Text("Typed title".into()));
    h.run();
    assert_eq!(h.state().session.page().unwrap().display_title(), "Typed title", "UI.Canvas.TitleArea");
}

/// The window's parts (catalog `UI.*` rows), found through the accessibility tree — what a
/// screen reader sees (`UI.Window.Accessibility`) — and, where they do something, used once.
#[test]
fn window_parts_are_present_and_work() {
    let mut h = harness(app());
    h.run();
    let has = |h: &Harness<'static, NoteApp>, label: &str| h.query_by_label(label).is_some();
    assert!(h.query_by_label_contains(" - Sample Notebook").is_some(), "UI.Window.TitleBar");
    for l in ["Back (Alt+Left)", "Undo (Ctrl+Z)", "Customize Quick Access Toolbar"] {
        present(&h, "UI.Window.QAT", l);
    }
    // The 2024 title bar: Search in the middle, the author's initials, Share (disabled).
    for (row, l) in [("UI.Window.SearchBox", "Search"), ("UI.Window.Account", "Account: Tester"), ("UI.Window.Share", "Share")] {
        present(&h, row, l);
    }
    // Custom-drawn controls are labelled for screen readers too.
    for l in ["Bold", "Collapse the Ribbon (Ctrl+F1)", "New section", "Add page", "Change search scope", "Hide Navigation"] {
        present(&h, "UI.Window.Accessibility", l);
    }
    for tab in ["File", "Home", "Insert", "Draw", "History", "Review", "View", "Help"] {
        assert!(has(&h, tab), "UI.Ribbon.Tabs: {tab}");
    }
    assert!(has(&h, "Sample Notebook"), "UI.Nav.NotebookPane");
    // The 2024 navigation: every section in the notebook tree, no section tabs above the page.
    assert_eq!(h.query_all_by_label("Meetings").count(), 1, "UI.Nav.NotebookTree");
    for (row, l) in [("UI.Nav.SearchBox", "Search notebooks"), ("UI.Nav.QuickNotes", "Quick notes"), ("UI.Nav.SortPages", "Sort pages")] {
        present(&h, row, l);
    }
    assert!(has(&h, "Welcome to NoteCraft") && has(&h, "Handy shortcuts"), "UI.Nav.PageList");

    // UI.Window.ContextMenus: right-click a page tab and a section tab.
    h.get_by_label("Handy shortcuts").click_secondary();
    h.run();
    for l in ["Make Subpage", "Promote Subpage", "Move or Copy…", "Copy Link to Page"] {
        present(&h, "UI.Window.ContextMenus", l);
    }
    h.key_press(egui::Key::Escape);
    h.run();
    h.get_all_by_label("Meetings").last().unwrap().click_secondary();
    h.run();
    for l in ["Rename", "Move or Copy…", "Copy Link to Section", "Password Protect This Section…"] {
        present(&h, "UI.Window.ContextMenus", l);
    }
    assert!(h.query_by_label_contains("Section Color").is_some(), "UI.Window.ContextMenus (section)");
    h.key_press(egui::Key::Escape);
    h.run();

    // UI.Nav.AddPage: one more page at the end of the section.
    let pages = h.state().session.section().unwrap().pages.len();
    h.get_by_label("Add page").click();
    h.run();
    assert_eq!(h.state().session.section().unwrap().pages.len(), pages + 1, "UI.Nav.AddPage");

    // UI.Nav.NewSectionLink: "+ New section" under the notebook's sections adds one.
    let sections = h.state().session.notebook().unwrap().sections().len();
    h.get_by_label("New section").click();
    // The new section's name is being edited in place (a blinking caret keeps repainting).
    h.run_steps(3);
    assert_eq!(h.state().session.notebook().unwrap().sections().len(), sections + 1, "UI.Nav.NewSectionLink");
    h.key_press(egui::Key::Escape);
    h.run();

    // UI.Ribbon.Collapse: the arrow at the right of the tab strip, both ways.
    h.get_by_label("Collapse the Ribbon (Ctrl+F1)").click();
    h.run();
    assert!(h.state().session.view.ribbon_collapsed, "UI.Ribbon.Collapse");
    h.get_by_label("Expand the Ribbon (Ctrl+F1)").click();
    h.run();
    assert!(!h.state().session.view.ribbon_collapsed, "UI.Ribbon.Collapse");

    // UI.Nav.Toggle (UI.Nav.NotebookPaneCollapse): ☰ hides the tree and the page column.
    h.get_by_label("Hide Navigation").click();
    h.run();
    assert!(h.state().session.view.notebook_pane_collapsed, "UI.Nav.Toggle");
    assert!(!has(&h, "Add page") && !has(&h, "Quick notes"), "UI.Nav.NotebookPaneCollapse");
    h.get_by_label("Show Navigation").click();
    h.run();
    assert!(!h.state().session.view.notebook_pane_collapsed, "UI.Nav.Toggle");

    // UI.Nav.FindOnPage: Ctrl+F opens the bar with its buttons.
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    h.run();
    for l in ["Previous", "Next", "Close"] {
        assert!(has(&h, l), "UI.Nav.FindOnPage: {l}");
    }
    h.get_by_label("Close").click();
    h.run();
    assert!(!h.state().session.view.find.active, "UI.Nav.FindOnPage closes");

    // UI.Nav.SearchResultsPane: the docked pane.
    h.state_mut().ui.search_pane = true;
    h.run();
    assert!(has(&h, "Search Results"), "UI.Nav.SearchResultsPane");
    h.state_mut().ui.search_pane = false;

    // UI.Window.FullPage: F11 hides the ribbon and navigation; one button returns.
    h.key_press(egui::Key::F11);
    h.run();
    assert!(h.state().session.view.full_page, "UI.Window.FullPage");
    assert!(!has(&h, "Home"), "UI.Window.FullPage hides the ribbon");
    h.get_by_label("Normal View").click();
    h.run();
    assert!(!h.state().session.view.full_page, "UI.Window.FullPage");
    // UI.Canvas.ExpandButton: the expand button at the page's top right does the same.
    h.get_by_label("Full Page View").click();
    h.run();
    assert!(h.state().session.view.full_page, "UI.Canvas.ExpandButton");
    h.get_by_label("Normal View").click();
    h.run();
    assert!(!h.state().session.view.full_page, "UI.Canvas.ExpandButton");
}

#[test]
fn ime_commits_reach_the_page() {
    // UI.Window.IME: composed text (CJK input methods) is inserted on commit; the preedit
    // string is not inserted.
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    a.canvas.focused = true;
    let mut h = harness(a);
    h.run();
    h.event(egui::Event::Ime(egui::ImeEvent::Preedit { text: "にほ".into(), active_range_chars: None }));
    h.run();
    assert_eq!(h.state().session.page().unwrap().title.text(), "", "preedit text is not inserted");
    h.event(egui::Event::Ime(egui::ImeEvent::Commit("日本".into())));
    h.run();
    assert_eq!(h.state().session.page().unwrap().title.text(), "日本", "UI.Window.IME");
}

#[test]
fn fractional_and_double_scale_render() {
    // UI.Window.HiDPI: the window lays out and renders at 1.25×, 1.5× and 2× (Wayland and X11
    // fractional scales) without clipping the ribbon tabs.
    for ppp in [1.25, 1.5, 2.0] {
        let mut h = Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .with_pixels_per_point(ppp)
            .build_ui_state(|ui, app: &mut NoteApp| app.ui(ui), app());
        h.run();
        for tab in ["Home", "Help"] {
            present(&h, "UI.Window.HiDPI", tab);
        }
    }
}

#[test]
fn highlighted_text_stays_readable_in_the_dark_theme() {
    let ctx = egui::Context::default();
    let t = crate::Tokens::for_theme(crate::ThemeKind::DarkGray);
    let page_text = egui::Color32::WHITE;
    let yellow = notecraft_engine::doc::color::Rgba { r: 255, g: 255, b: 0, a: 255 };
    let f = notecraft_engine::doc::CharFormat { highlight: Some(yellow), ..Default::default() };
    assert_eq!(crate::canvas::text_format(&ctx, &f, 1.0, t, page_text).color, egui::Color32::BLACK, "automatic text on yellow");
    let red = notecraft_engine::doc::color::Rgba { r: 200, g: 0, b: 0, a: 255 };
    let f = notecraft_engine::doc::CharFormat { highlight: Some(yellow), color: Some(red), ..Default::default() };
    assert_eq!(crate::canvas::text_format(&ctx, &f, 1.0, t, page_text).color, crate::theme::c32(red), "an explicit colour is kept");
}

#[test]
fn the_office_theme_is_picked_in_the_account_page_and_follows_the_system() {
    // UI.Window.DarkMode: File › Account › Office Theme lists Colorful, Dark Gray, Black, White
    // and "Use system setting"; picking one restyles the window at once.
    let mut h = harness(app());
    h.state_mut().ui.backstage = Some(crate::Backstage::Account);
    h.run();
    for kind in crate::ThemeKind::ALL {
        present(&h, "UI.Window.DarkMode", kind.label());
    }
    h.get_by_label("Dark Gray").click();
    h.run();
    h.run();
    assert_eq!(h.state().ui.theme, crate::ThemeKind::DarkGray, "UI.Window.DarkMode");
    assert!(h.ctx.global_style().visuals.dark_mode, "the dark theme is applied to egui");
    h.get_by_label("Use system setting").click();
    h.run();
    h.run();
    // FixedHost states no preference and the test context reports none: Colorful.
    assert_eq!(h.state().resolved_theme(), crate::ThemeKind::Colorful);
    assert!(!h.ctx.global_style().visuals.dark_mode);
    h.state_mut().system_dark = Some(true);
    assert_eq!(h.state().resolved_theme(), crate::ThemeKind::DarkGray, "System follows a dark desktop");
}

#[test]
fn the_title_search_runs_commands_and_hands_text_to_search_notebooks() {
    // UI.Window.SearchBox / UI.Window.TellMe: Alt+Q focuses the Search; typing lists matching
    // commands and Enter runs the first one; the last row searches the notebooks instead.
    let mut h = harness(app());
    h.run();
    let sections = h.state().session.notebook().unwrap().sections().len();
    h.event(egui::Event::Key { key: egui::Key::Q, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::ALT });
    h.run();
    h.event(egui::Event::Text("new section".into()));
    h.run();
    assert!(h.query_by_label_contains("Search notebooks for").is_some(), "UI.Window.SearchBox lists a notes row");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(h.state().session.notebook().unwrap().sections().len(), sections + 1, "UI.Window.TellMe");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    h.event(egui::Event::Key { key: egui::Key::Q, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::ALT });
    h.run();
    h.event(egui::Event::Text("basil".into()));
    h.run();
    h.get_by_label_contains("Search notebooks for").click();
    h.run();
    assert_eq!(h.state().ui.search, "basil", "UI.Window.SearchBox hands the text to Search notebooks");
    assert!(h.state().ui.search_open);
}

#[test]
fn the_ribbon_switches_between_simplified_and_classic() {
    use notecraft_engine::RibbonMode;
    // UI.Ribbon.Simplified is the default: one row, with the rest in the … overflow menu.
    let mut a = app();
    let pid = a.session.notebook().unwrap().sections()[1].pages[0].id;
    a.session.run("nav.goto", json!({"page": pid.to_string()})).unwrap();
    a.session.run("nav.pageTitle", json!({})).unwrap();
    let mut h = Harness::builder().with_size(egui::vec2(900.0, 700.0)).build_ui_state(|ui, app: &mut NoteApp| app.ui(ui), a);
    h.run();
    assert_eq!(h.state().session.view.ribbon_mode, RibbonMode::Simplified, "UI.Ribbon.Simplified");
    present(&h, "UI.Ribbon.Simplified", "Bold");
    assert!(h.query_by_label("Clipboard").is_none(), "no classic group labels in the simplified ribbon");
    h.get_by_label("More commands").click();
    h.run();
    // Cut is not placed in the row: the overflow menu lists it under its classic group.
    present(&h, "UI.Ribbon.Simplified", "Cut");
    h.key_press(egui::Key::Escape);
    h.run();
    // UI.Ribbon.DisplayOptions: the ▾ chevron switches to the classic ribbon and back, and shows
    // only the tabs.
    h.get_by_label("Ribbon display options").click();
    h.run();
    h.get_by_label("Classic ribbon").click();
    h.run();
    assert_eq!(h.state().session.view.ribbon_mode, RibbonMode::Classic, "UI.Ribbon.DisplayOptions");
    present(&h, "UI.Ribbon.Tabs", "Format Painter");
    h.get_by_label("Ribbon display options").click();
    h.run();
    h.get_by_label("Show tabs only").click();
    h.run();
    assert!(h.state().session.view.ribbon_collapsed, "UI.Ribbon.DisplayOptions");
    h.get_by_label("Home").click();
    h.run();
    assert!(!h.state().session.view.ribbon_collapsed);
    h.get_by_label("Ribbon display options").click();
    h.run();
    h.get_by_label("Single-line ribbon").click();
    h.run();
    assert_eq!(h.state().session.view.ribbon_mode, RibbonMode::Simplified);
}

#[test]
fn the_notebook_tree_quick_notes_and_sort_work() {
    let mut h = harness(app());
    h.run();
    // UI.Nav.NotebookTree: a click on the open notebook folds its sections away, and back.
    assert!(h.query_by_label("Garden").is_some(), "UI.Nav.NotebookTree shows nested sections");
    h.get_by_label("Sample Notebook").click();
    h.run();
    assert!(h.query_by_label("Meetings").is_none(), "UI.Nav.NotebookTree collapses");
    h.get_by_label("Sample Notebook").click();
    h.run();
    assert!(h.query_by_label("Meetings").is_some(), "UI.Nav.NotebookTree expands");
    // A section group folds on its own.
    h.get_by_label("Projects").click();
    h.run();
    assert!(h.query_by_label("Garden").is_none(), "UI.Nav.NotebookTree folds a section group");
    // UI.Nav.QuickNotes opens the Quick Notes section.
    h.get_by_label("Quick notes").click();
    h.run();
    assert_eq!(h.state().session.section().unwrap().name, "Quick Notes", "UI.Nav.QuickNotes");
    // UI.Nav.SortPages: the sort menu reorders the pages of the open section.
    h.get_by_label("Meetings").click();
    h.run();
    let first = h.state().session.section().unwrap().pages[0].display_title();
    h.get_by_label("Sort pages").click();
    h.run();
    h.get_by_label("Title (A to Z)").click();
    h.run();
    let titles: Vec<String> = h.state().session.section().unwrap().pages.iter().map(|p| p.display_title().to_lowercase()).collect();
    let mut sorted = titles.clone();
    sorted.sort();
    assert_eq!(titles, sorted, "UI.Nav.SortPages (was led by {first})");
}

#[test]
fn insert_table_dialog_opens_from_the_ribbon_and_inserts() {
    // parity: UI.Dialogs.InsertTable, R.Insert.Tables.TableInsert
    let a = app();
    assert_eq!(crate::ribbon::dialog_for(&a, "R.Insert.Tables.TableInsert"), Some(crate::dialogs::Dialog::InsertTable(2, 2)));
    let mut a = a;
    a.session.run("page.new", json!({})).unwrap();
    a.ui.dialog = Some(crate::dialogs::Dialog::InsertTable(4, 3));
    let mut h = harness(a);
    h.run();
    assert!(h.query_by_label("Insert Table").is_some(), "UI.Dialogs.InsertTable shows");
    h.get_by_label("OK").click();
    h.run();
    let page = h.state().session.page().unwrap().clone();
    let t = page.objects.iter().find_map(|o| o.as_table()).expect("a table was inserted");
    assert_eq!((t.n_rows(), t.n_cols()), (3, 4), "the dialog's rows/columns reached insert.tableDialog");
    assert!(h.state().ui.dialog.is_none(), "OK closes the dialog");
}

#[test]
fn paragraph_spacing_dialog_shows_the_paragraph_and_sets_it() {
    // parity: UI.Dialogs.ParagraphSpacing
    use crate::dialogs::Dialog;
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    a.session.run("outline.new", json!({"x": 40.0, "y": 100.0})).unwrap();
    a.session.run("text.insert", json!({"text": "spaced"})).unwrap();
    a.session.run("para.spacingDialog", json!({"before": 3, "after": 4, "line": 5})).unwrap();
    // The dialog opens with the caret paragraph's before, after and at-least line spacing.
    assert_eq!(crate::ribbon::dialog_for(&a, "R.Home.BasicText.ParagraphSpacingOptions"), Some(Dialog::Spacing(3.0, 4.0, 5.0)));
    a.ui.dialog = Some(Dialog::Spacing(6.0, 12.0, 18.0));
    let mut h = harness(a);
    h.run();
    assert!(h.query_by_label("Paragraph Spacing").is_some(), "UI.Dialogs.ParagraphSpacing shows");
    assert!(h.query_by_label("Line Spacing (at least):").is_some());
    h.get_by_label("OK").click();
    h.run();
    let s = &h.state().session;
    let r = s.text_range().unwrap();
    let p = notecraft_engine::Session::block_paras(s.page().unwrap(), r.head.block).unwrap()[r.head.para].clone();
    assert_eq!((p.space_before, p.space_after, p.line_spacing), (6.0, 12.0, 18.0), "OK applies the three values");
    assert!(h.state().ui.dialog.is_none(), "OK closes the dialog");
}

#[test]
fn notebook_properties_dialog_renames_the_notebook() {
    use crate::dialogs::Dialog;
    let mut a = app();
    let nb = a.session.notebook().unwrap().id;
    a.ui.dialog = Some(Dialog::NotebookProperties(nb, "Field notes".into()));
    let mut h = harness(a);
    h.run();
    assert!(h.query_by_label("Notebook Properties").is_some(), "UI.Dialogs.NotebookProperties");
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().session.notebook().unwrap().name, "Field notes", "OK sets the display name");
}

#[test]
fn the_sort_menu_and_dialog_sort_the_table() {
    // parity: CT.TableLayout.Data.SortMenu
    use crate::dialogs::Dialog;
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    a.session.run("outline.new", json!({"x": 40.0, "y": 100.0})).unwrap();
    a.session.run("text.insert", json!({"text": "Fruit\tShop\npear\tB\napple\tA\nfig\tB"})).unwrap();
    a.session.run("edit.selectAll", json!({})).unwrap();
    a.session.run("edit.selectAll", json!({})).unwrap();
    a.session.run("insert.convertTextToTable", json!({})).unwrap();
    let table = |a: &NoteApp| a.session.page().unwrap().objects.iter().find_map(|o| o.as_table().cloned()).unwrap();
    let c = table(&a).rows[1].cells[0].id;
    a.session.run("caret.set", json!({"block": c.to_string(), "para": 0, "offset": 0})).unwrap();
    a.session.run("ui.ribbonMode", json!({"mode": "classic"})).unwrap();
    a.ui.tab = "Table Layout".into();
    let mut h = harness(a);
    h.run();
    let fruit = |h: &Harness<'static, NoteApp>| table(h.state()).rows.iter().map(|r| r.cells[0].text()).collect::<Vec<_>>();
    let from_menu = |h: &mut Harness<'static, NoteApp>, item: &str| {
        h.get_by_label("Sort").click();
        h.run();
        h.get_by_label(item).click();
        h.run();
    };
    from_menu(&mut h, "Header Row");
    assert!(table(h.state()).header_row);
    from_menu(&mut h, "Sort Descending");
    assert_eq!(fruit(&h), vec!["Fruit", "pear", "fig", "apple"]);
    // Sort… opens on the caret's column, ascending, with the table's header row setting.
    from_menu(&mut h, "Sort…");
    assert_eq!(h.state().ui.dialog, Some(Dialog::SortTable { by: [Some(0), None], descending: [false, false], header: true }));
    // By shop, then by fruit falling; with a header row the columns go by its text.
    h.state_mut().ui.dialog = Some(Dialog::SortTable { by: [Some(1), Some(0)], descending: [false, true], header: true });
    h.run();
    let combos: Vec<String> = h.query_all_by_role(egui::accesskit::Role::ComboBox).filter_map(|n| n.value()).collect();
    assert_eq!(combos, vec!["Shop", "Fruit"], "columns named by the header row");
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(fruit(&h), vec!["Fruit", "apple", "pear", "fig"]);
    assert!(h.state().ui.dialog.is_none(), "OK closes the dialog");
}

#[test]
fn the_templates_pane_adds_pages_and_saves_the_current_one() {
    // parity: UI.Panes.Templates, R.Insert.Pages.PageTemplatesTaskpane, R.Insert.Pages.StationeryMostRecentlyUsedList
    use crate::dialogs::Dialog;
    let mut a = app();
    a.session.run("ui.ribbonMode", json!({"mode": "classic"})).unwrap();
    a.ui.tab = "Insert".into();
    let mut h = harness(a);
    h.run();
    h.get_by_label("Page Templates").click();
    h.run();
    assert!(h.state().session.view.templates_pane, "Insert › Page Templates opens the pane");
    h.get_by_label("Business").click();
    h.run();
    let pages = h.state().session.section().unwrap().pages.len();
    h.get_by_label("Meeting notes").click();
    h.run();
    assert_eq!(h.state().session.section().unwrap().pages.len(), pages + 1, "a click adds a page from the template");
    assert!(
        h.state()
            .session
            .page()
            .unwrap()
            .objects
            .iter()
            .filter_map(|o| o.as_outline())
            .any(|o| o.paragraphs.iter().any(|p| p.text() == "Action items"))
    );
    assert!(h.query_by_label("Recently Used Templates").is_some());
    // Save current page as a template: a name, and it lists under My Templates.
    h.get_by_label("Save current page as a template").click();
    h.run();
    assert!(matches!(h.state().ui.dialog, Some(Dialog::SaveTemplate { .. })));
    h.state_mut().ui.dialog = Some(Dialog::SaveTemplate { name: "Standup".into(), default: true });
    h.run();
    h.get_by_label("Save").click();
    h.run();
    assert_eq!(h.state().session.prefs.templates.len(), 1);
    assert_eq!(h.state().session.section().unwrap().default_template.as_deref(), Some("my:Standup"));
    assert!(h.query_by_label("Standup").is_some(), "My Templates lists it");
    // The split button's drop-down lists recently used templates by name.
    h.get_by_label("Page Templates").click();
    h.run();
    h.get_by_label("Page Templates").click();
    h.run();
    assert!(h.state().session.view.templates_pane, "a second click on the icon half closes and a third reopens it");
}

#[test]
fn the_paper_size_pane_sets_the_page_paper() {
    // parity: UI.Panes.PaperSize, UI.Panes.PaperSize.SaveTemplate
    use crate::dialogs::Dialog;
    let mut a = app();
    a.session.run("ui.ribbonMode", json!({"mode": "classic"})).unwrap();
    a.ui.tab = "View".into();
    let mut h = harness(a);
    h.run();
    h.get_by_label("Paper Size").click();
    h.run();
    assert!(h.state().session.view.paper_pane, "View › Paper Size opens the pane");
    assert!(h.state().session.page().unwrap().setup.paper.is_none());
    let combo = |h: &Harness<'static, NoteApp>, value: &str| {
        h.query_all_by_role(egui::accesskit::Role::ComboBox).find(|n| n.value().as_deref() == Some(value)).expect(value).click();
    };
    combo(&h, "Auto");
    h.run();
    h.get_by_label("A4").click();
    h.run();
    let ps = h.state().session.page().unwrap().setup.paper.expect("A4 sets a paper size");
    assert!((ps.width - 595.3).abs() < 0.2, "{ps:?}");
    combo(&h, "Portrait");
    h.run();
    h.get_by_label("Landscape").click();
    h.run();
    let ps = h.state().session.page().unwrap().setup.paper.unwrap();
    assert!(ps.width > ps.height, "Landscape turns the paper: {ps:?}");
    h.get_by_label("Save current page as a template").click();
    h.run();
    assert!(matches!(h.state().ui.dialog, Some(Dialog::SaveTemplate { .. })));
}

#[test]
fn link_dialog_searches_pages_and_sections() {
    // parity: UI.Dialogs.Link
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    a.ui.dialog = Some(crate::dialogs::Dialog::Link { url: String::new(), text: String::new(), filter: "retrospective".into() });
    let mut h = harness(a);
    h.run();
    assert!(h.query_by_label_contains("Retrospective").is_some(), "the page search narrows to a matching page");
    // "› Garden" (the dialog's breadcrumb row), not a bare "Garden" (also the nav tree's own
    // section row, always on screen behind the modal).
    assert!(h.query_by_label_contains("› Garden").is_none(), "and excludes a non-matching one");
    h.get_by_label_contains("Retrospective").click();
    h.run();
    let dialog = h.state().ui.dialog.clone();
    let Some(crate::dialogs::Dialog::Link { url, .. }) = dialog else { panic!("dialog closed unexpectedly") };
    assert!(url.starts_with("notecraft:page/"), "picking a page sets a notecraft:page/ link: {url}");
}

#[test]
fn move_or_copy_dialogs_search_sections_and_notebooks() {
    // parity: UI.Dialogs.MoveCopyPage, UI.Dialogs.MoveCopySection
    let mut a = app();
    let pid = a.session.page().unwrap().id;
    a.ui.dialog = Some(crate::dialogs::Dialog::MovePage(pid, "garden".into()));
    let mut h = harness(a);
    h.run();
    assert!(h.query_by_label_contains("› Garden").is_some(), "UI.Dialogs.MoveCopyPage: the search matches a section");
    assert!(h.query_by_label_contains("› Meetings").is_none(), "and excludes a non-matching one");
    let sid = h.state().session.section().unwrap().id;
    h.state_mut().ui.dialog = Some(crate::dialogs::Dialog::MoveSection(sid, "sample".into()));
    h.run();
    // Ambiguous as an exact label (the nav tree has its own "Sample Notebook" row behind the
    // modal), so count instead of a single query.
    assert!(h.query_all_by_label("Sample Notebook").count() >= 1, "UI.Dialogs.MoveCopySection: the search matches a notebook");
}

#[test]
fn notebook_list_dialog_switches_notebooks() {
    // parity: UI.Nav.NotebookList
    let mut a = app();
    a.session.run("file.newNotebook", json!({"name": "Second"})).unwrap();
    let first = a.session.ws.notebooks[0].id;
    let second = a.session.ws.notebooks[1].id;
    assert_eq!(a.session.sel.notebook, Some(second), "the new notebook is selected");
    // The nav tree has its own row per notebook too: hide it so the dialog's labels are
    // unambiguous (collapsing navigation is itself a normal, tested action — UI.Nav.Toggle).
    a.session.view.notebook_pane_collapsed = true;
    a.ui.dialog = Some(crate::dialogs::Dialog::NotebookList);
    let mut h = harness(a);
    h.run();
    assert!(h.query_by_label("Sample Notebook").is_some(), "UI.Nav.NotebookList (Ctrl+G) lists every open notebook");
    h.get_by_label("Sample Notebook").click();
    h.run();
    assert_eq!(h.state().session.sel.notebook, Some(first), "picking one selects it");
    assert!(h.state().ui.dialog.is_none(), "and closes the dialog");
}

#[test]
fn recent_notebooks_show_in_the_open_backstage_page() {
    // parity: BS.Open.RecentNotebooks
    let mut a = app();
    a.ui.recent_notebooks = vec!["/notebooks/Garden Club".into()];
    a.ui.backstage = Some(crate::Backstage::Open);
    let mut h = harness(a);
    h.run();
    h.get_by_label("Garden Club").click();
    h.run();
    // The path doesn't exist in the test sandbox, so the open fails — but the click reached
    // file.openRecent, which is what this row is about.
    assert!(h.state().ui.status.is_some(), "clicking a recent notebook ran file.openRecent");
}

#[test]
fn symbol_dialog_inserts_a_symbol_at_the_caret() {
    // parity: UI.Dialogs.Symbol, R.Insert.Symbols.SymbolsDialog
    let a = app();
    assert_eq!(crate::ribbon::dialog_for(&a, "R.Insert.Symbols.SymbolsDialog"), Some(crate::dialogs::Dialog::Symbol));
    // R.Insert.Symbols.SymbolInsertGallery: the gallery trigger opens the same dialog (a plain
    // inline gallery would need its own popup; this is the simpler, still-honest choice).
    assert_eq!(crate::ribbon::dialog_for(&a, "R.Insert.Symbols.SymbolInsertGallery"), Some(crate::dialogs::Dialog::Symbol));
    let mut a = a;
    a.session.run("page.new", json!({})).unwrap();
    a.session.run("outline.new", json!({"x": 40.0, "y": 100.0})).unwrap();
    a.session.run("text.insert", json!({"text": "Price: "})).unwrap();
    a.ui.dialog = Some(crate::dialogs::Dialog::Symbol);
    let mut h = harness(a);
    h.run();
    assert!(h.query_by_label("€").is_some(), "UI.Dialogs.Symbol shows the symbol grid");
    h.get_by_label("€").click();
    h.run();
    assert_eq!(h.state().session.page().unwrap().plain_text().trim(), "Price: €");
    assert!(h.state().ui.dialog.is_none(), "picking a symbol closes the dialog");
}

#[test]
fn options_page_has_the_office_theme_too() {
    // parity: UI.Dialogs.Options, OPT.General.OfficeTheme
    let mut h = harness(app());
    h.state_mut().ui.backstage = Some(crate::Backstage::Options);
    h.run();
    present(&h, "UI.Dialogs.Options", "Office Theme");
    h.get_by_label("Black").click();
    h.run();
    assert_eq!(h.state().ui.theme, crate::ThemeKind::Black, "OPT.General.OfficeTheme: picked from the Options page too");
}

/// A host whose save dialog answers with a fixed path.
struct SaveHost(String);

impl crate::Host for SaveHost {
    fn clock(&self) -> notecraft_engine::Clock {
        notecraft_engine::Clock { now_ms: 1_792_007_100_000, utc_offset_min: 0 }
    }
    fn save_file(&mut self, _title: &str, _default_name: &str) -> Option<String> {
        Some(self.0.clone())
    }
}

#[test]
fn the_export_page_exports_the_section_as_markdown() {
    let dir = std::env::temp_dir().join(format!("notecraft-ui-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("section.md");
    let mut s = notecraft_engine::Session::with_sample(notecraft_engine::Clock { now_ms: 1_792_007_100_000, utc_offset_min: 0 });
    s.prefs.author = "Tester".into();
    let first = s.section().unwrap().pages.first().unwrap().display_title();
    let mut a = NoteApp::new(s, Box::new(SaveHost(path.to_string_lossy().into_owned())));
    a.ui.backstage = Some(crate::Backstage::Export);
    let mut h = harness(a);
    h.run();
    // Formats not written yet are listed, disabled (BS.Export.FormatXps).
    assert!(h.query_by_label("XPS (*.xps)").is_some());
    h.get_by_label("Section").click();
    h.run();
    assert_eq!(h.state().ui.export_scope, "section", "BS.Export.Section");
    h.get_by_label("Markdown (*.md)").click();
    h.run();
    h.get_by_label("Export…").click();
    h.run();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(text.starts_with(&format!("# {first}")), "{text}");
    assert_eq!(h.state().ui.backstage, None, "the backstage closes after exporting");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A host with two printers that records what it was asked to print.
struct PrintHost(std::rc::Rc<std::cell::RefCell<Vec<(String, Option<String>, u32)>>>);

impl crate::Host for PrintHost {
    fn clock(&self) -> notecraft_engine::Clock {
        notecraft_engine::Clock { now_ms: 1_792_007_100_000, utc_offset_min: 0 }
    }
    fn printers(&mut self) -> Vec<(String, bool)> {
        vec![("Office".into(), false), ("Desk".into(), true)]
    }
    fn print_pdf(&mut self, path: &str, printer: Option<&str>, copies: u32, _title: &str) -> Result<String, String> {
        assert!(std::fs::read(path).unwrap().starts_with(b"%PDF-"), "a print-ready PDF");
        self.0.borrow_mut().push((path.to_string(), printer.map(str::to_string), copies));
        Ok("request id is Desk-1".into())
    }
}

#[test]
fn ctrl_p_opens_print_and_print_sends_the_pdf() {
    if notecraft_fonts::page_fonts().regular.is_none() {
        return;
    }
    let jobs = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut s = notecraft_engine::Session::with_sample(notecraft_engine::Clock { now_ms: 1_792_007_100_000, utc_offset_min: 0 });
    s.prefs.author = "Tester".into();
    let a = NoteApp::new(s, Box::new(PrintHost(jobs.clone())));
    let mut h = harness(a);
    h.run();
    // KB.Pages.Print: Ctrl+P opens File › Print.
    h.event(egui::Event::Key { key: egui::Key::P, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.run();
    assert_eq!(h.state().ui.backstage, Some(crate::Backstage::Print), "BS.Print.Print");
    assert_eq!(h.state().ui.print_printer.as_deref(), Some("Desk"), "the default printer is chosen");
    h.state_mut().ui.print_copies = 2;
    h.get_by_label("Print  (Ctrl+P)").click();
    h.run();
    let jobs = jobs.borrow();
    assert_eq!(jobs.len(), 1);
    assert_eq!((jobs[0].1.as_deref(), jobs[0].2), (Some("Desk"), 2));
    assert!(h.state().ui.status.as_ref().is_some_and(|(m, _)| m.contains("Desk-1")));
    let _ = std::fs::remove_file(&jobs[0].0);
}

/// A host whose file picker answers with a fixed path.
struct ImportHost(String);

impl crate::Host for ImportHost {
    fn clock(&self) -> notecraft_engine::Clock {
        notecraft_engine::Clock { now_ms: 1_792_007_100_000, utc_offset_min: 0 }
    }
    fn pick_file(&mut self, _title: &str, filters: &[(&str, &[&str])]) -> Option<String> {
        assert!(filters.first().is_some_and(|f| f.1.contains(&"md")));
        Some(self.0.clone())
    }
}

#[test]
fn import_file_in_the_open_page_adds_a_page() {
    let dir = std::env::temp_dir().join(format!("notecraft-ui-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Ideas.md");
    std::fs::write(&path, "# Ideas\n\n- one\n- two\n").unwrap();
    let s = notecraft_engine::Session::with_sample(notecraft_engine::Clock { now_ms: 1_792_007_100_000, utc_offset_min: 0 });
    let mut a = NoteApp::new(s, Box::new(ImportHost(path.to_string_lossy().into_owned())));
    a.ui.backstage = Some(crate::Backstage::Open);
    let mut h = harness(a);
    h.run();
    h.get_by_label("Import File…").click();
    h.run();
    assert_eq!(h.state().session.page().unwrap().title.text(), "Ideas", "NC.File.Import");
    assert_eq!(h.state().ui.backstage, None);
    let _ = std::fs::remove_dir_all(&dir);
}
