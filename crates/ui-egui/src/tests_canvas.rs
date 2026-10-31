//! Canvas tests: the paragraph handle, containers growing with their text and the page
//! growing with its content, driven by pointer events through the real canvas.

use egui::{Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use notecraft_engine::doc::Id;
use notecraft_engine::selection::Block;
use serde_json::json;

use crate::NoteApp;
use crate::tests::{app, harness};

/// A new page with one container at (60, 120) pt holding `text` (newlines = paragraphs).
pub(crate) fn page_with(text: &str) -> (NoteApp, Id) {
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    a.session.run("outline.new", json!({"x": 60.0, "y": 120.0})).unwrap();
    a.session.run("text.insert", json!({"text": text})).unwrap();
    let id = a.session.page().unwrap().objects[0].id();
    a.canvas.focused = true;
    (a, id)
}

pub(crate) fn texts(h: &Harness<'static, NoteApp>, id: Id) -> Vec<(String, u8)> {
    h.state().session.page().unwrap().outline(id).unwrap().paragraphs.iter().map(|p| (p.text(), p.level)).collect()
}

fn handle(h: &Harness<'static, NoteApp>, id: Id, para: usize) -> Pos2 {
    h.state().canvas.para(Block::Outline(id), para).unwrap().handle.center()
}

pub(crate) fn press(h: &mut Harness<'static, NoteApp>, p: Pos2, pressed: bool) {
    h.event(egui::Event::PointerMoved(p));
    h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    h.run();
}

pub(crate) fn click(h: &mut Harness<'static, NoteApp>, p: Pos2) {
    h.event(egui::Event::PointerMoved(p));
    h.run_steps(60);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
}

/// Two clicks, well after any earlier click. The harness clock advances by whole frames and
/// spreads queued events over frames, so the double-click window is widened for the test and
/// earlier clicks are pushed out of the triple-click window first.
pub(crate) fn double_click(h: &mut Harness<'static, NoteApp>, p: Pos2) {
    h.ctx.options_mut(|o| o.input_options.max_double_click_delay = 2.0);
    h.event(egui::Event::PointerMoved(p));
    h.run_steps(400);
    for pressed in [true, false, true, false] {
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
}

pub(crate) fn right_click(h: &mut Harness<'static, NoteApp>, p: Pos2) {
    h.event(egui::Event::PointerMoved(p));
    h.run_steps(60);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
}

pub(crate) fn drag(h: &mut Harness<'static, NoteApp>, from: Pos2, to: Pos2) {
    press(h, from, true);
    for k in 1..=6 {
        let t = k as f32 / 6.0;
        h.event(egui::Event::PointerMoved(from + (to - from) * t));
        h.run();
    }
    press(h, to, false);
}

#[test]
fn the_paragraph_handle_selects_moves_and_collapses() {
    // parity: UI.Canvas.ParagraphHandle
    let (a, o) = page_with("Alpha\nBeta\nGamma");
    let mut h = harness(a);
    h.run();
    // Click: the paragraph (with its children) is selected.
    let p = handle(&h, o, 1);
    click(&mut h, p);
    let r = h.state().session.text_range().unwrap();
    assert_eq!((r.ordered().0.para, r.ordered().1.para, r.ordered().1.offset), (1, 1, 4), "UI.Canvas.ParagraphHandle: click selects");
    // Drag: Gamma goes above Alpha, as one undo step.
    let undo = h.state().session.undo_labels().len();
    let from = handle(&h, o, 2);
    let to = h.state().canvas.para(Block::Outline(o), 0).unwrap().pos + egui::vec2(2.0, 1.0);
    drag(&mut h, from, to);
    assert_eq!(texts(&h, o).iter().map(|t| t.0.as_str()).collect::<Vec<_>>(), vec!["Gamma", "Alpha", "Beta"], "BEH.Outline.DragParagraph");
    assert_eq!(h.state().session.undo_labels().len(), undo + 1);
    // Dragging to the right of the paragraph above indents it under that paragraph.
    let from = handle(&h, o, 2);
    let beta = h.state().canvas.para(Block::Outline(o), 2).unwrap().clone();
    drag(&mut h, from, pos2(beta.pos.x + 40.0, beta.pos.y + 2.0));
    assert_eq!(texts(&h, o)[2], ("Beta".to_string(), 1), "dropped one level deeper");
    // Double-click on a parent's handle collapses its children; again expands them.
    let p = handle(&h, o, 1);
    double_click(&mut h, p);
    let page = h.state().session.page().unwrap().clone();
    assert!(page.outline(o).unwrap().paragraphs[1].collapsed, "BEH.Outline.CollapseExpand");
    assert!(h.state().canvas.para(Block::Outline(o), 2).is_none(), "the child is hidden");
    double_click(&mut h, p);
    assert!(!h.state().session.page().unwrap().outline(o).unwrap().paragraphs[1].collapsed);
    // Dropped far from every container: a new container there.
    let from = handle(&h, o, 0);
    drag(&mut h, from, pos2(900.0, 650.0));
    let page = h.state().session.page().unwrap().clone();
    assert_eq!(page.objects.len(), 2, "a new container holds the dropped paragraph");
    assert_eq!(page.objects[1].as_outline().unwrap().paragraphs[0].text(), "Gamma");
}

#[test]
fn containers_grow_with_their_text_and_the_page_grows_with_them() {
    // parity: BEH.Canvas.ContainerAutoWidth, BEH.Canvas.PageGrows
    let (a, o) = page_with("Hi");
    let mut h = harness(a);
    h.run();
    let w0 = h.state().canvas.outlines[0].rect.width();
    h.state_mut().session.run("text.insert", json!({"text": " there, a longer line of text"})).unwrap();
    h.run();
    let w1 = h.state().canvas.outlines[0].rect.width();
    assert!(w1 > w0 + 50.0, "BEH.Canvas.ContainerAutoWidth: the container widens with its text ({w0} → {w1})");
    // Up to the maximum width, then the text wraps and the container grows down.
    let long = "word ".repeat(200);
    h.state_mut().session.run("text.insert", json!({"text": long})).unwrap();
    h.run();
    let z = crate::canvas::PT;
    let r = h.state().canvas.outlines[0].rect;
    let max = notecraft_engine::doc::page::AUTO_WIDTH_MAX * z;
    assert!(r.width() <= max + 16.0 * z + 12.0, "capped at the maximum width: {} > {max}", r.width());
    assert!(r.height() > 80.0, "wrapped lines make it taller: {}", r.height());
    assert_eq!(h.state().canvas.paras.iter().filter(|p| p.block == Block::Outline(o)).count(), 1);
    // The page grows right and down: a container moved far away is still reachable.
    let id = o.to_string();
    h.state_mut().session.run("outline.move", json!({"id": id, "x": 3000.0, "y": 2500.0})).unwrap();
    h.run();
    let ext = crate::canvas::page_extent(h.state());
    assert!(ext.x > 3000.0 * z && ext.y > 2500.0 * z, "BEH.Canvas.PageGrows: extent {ext:?}");
}

#[test]
fn the_container_bar_moves_and_its_edge_resizes() {
    // parity: UI.Canvas.Container
    let (a, o) = page_with("Some text");
    let mut h = harness(a);
    h.run();
    // Hovering shows the frame with the move bar and the resize edge.
    let ol = h.state().canvas.outlines[0];
    h.event(egui::Event::PointerMoved(ol.rect.center()));
    h.run();
    // The bar moves the container (one undo step for the whole drag).
    let undo = h.state().session.undo_labels().len();
    let before = h.state().session.page().unwrap().outline(o).unwrap().pos;
    drag(&mut h, ol.bar.center(), ol.bar.center() + egui::vec2(120.0, 60.0));
    let after = h.state().session.page().unwrap().outline(o).unwrap().pos;
    let z = crate::canvas::PT;
    assert!(
        (after.x - before.x - 120.0 / z).abs() < 2.0 && (after.y - before.y - 60.0 / z).abs() < 2.0,
        "UI.Canvas.Container: {before:?} → {after:?}"
    );
    assert_eq!(h.state().session.undo_labels().len(), undo + 1);
    // The right edge sets a fixed width.
    let ol = h.state().canvas.outlines[0];
    drag(&mut h, ol.resize.center(), ol.resize.center() + egui::vec2(80.0, 0.0));
    let w = h.state().session.page().unwrap().outline(o).unwrap().width;
    assert!(matches!(w, notecraft_engine::doc::OutlineWidth::Fixed(_)), "{w:?}");
    // A click in the text still places the caret.
    let pl = h.state().canvas.para(Block::Outline(o), 0).unwrap().clone();
    click(&mut h, pl.pos + egui::vec2(3.0, 4.0));
    let c = h.state().session.sel.caret().unwrap();
    assert_eq!((c.block, c.para, c.offset), (Block::Outline(o), 0, 0));
}

/// A page with a 2 × 3 table at (60, 120) pt, the caret in its first cell.
fn page_with_table() -> (NoteApp, Id) {
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    let v = a.session.run("insert.table", json!({"rows": 2, "cols": 3, "x": 60.0, "y": 120.0})).unwrap();
    a.canvas.focused = true;
    (a, Id::parse(v["table"].as_str().unwrap()).unwrap())
}

fn table(h: &Harness<'static, NoteApp>, id: Id) -> notecraft_engine::doc::Table {
    h.state().session.page().unwrap().table(id).unwrap().clone()
}

#[test]
fn table_cells_take_clicks_keys_and_drags() {
    let (a, tid) = page_with_table();
    let mut h = harness(a);
    h.run();
    // Typing and Tab go through the keymap into the cells.
    h.event(egui::Event::Text("Name".into()));
    h.run();
    h.key_press(egui::Key::Tab);
    h.run();
    h.event(egui::Event::Text("Qty".into()));
    h.run();
    let t = table(&h, tid);
    assert_eq!((t.rows[0].cells[0].text(), t.rows[0].cells[1].text()), ("Name".to_string(), "Qty".to_string()));
    // The Table Layout tab shows while the caret is in the table.
    assert!(crate::chrome::contextual_tabs(h.state()).iter().any(|t| t.name == "Table Layout"), "UI.Ribbon.ContextualTabs");
    assert!(h.query_by_label("Table Layout").is_some());
    // A click anywhere in a cell puts the caret there.
    let tl = h.state().canvas.tables[0].clone();
    let cell = tl.cells.iter().find(|c| c.row == 1 && c.col == 2).copied().unwrap();
    click(&mut h, cell.rect.center() + egui::vec2(10.0, 2.0));
    assert_eq!(h.state().session.sel.caret().map(|c| c.block), Some(Block::Cell(cell.id)));
    // Enter adds a row (the keymap routes it to table.enter); on an empty last row it would
    // end the table instead.
    h.event(egui::Event::Text("x".into()));
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(table(&h, tid).n_rows(), 3, "KB.Table.NewRowEnter through the keymap");
    // A drag from one cell to another selects the cells.
    let tl = h.state().canvas.tables[0].clone();
    let a0 = tl.cells.iter().find(|c| c.row == 0 && c.col == 0).copied().unwrap();
    let b1 = tl.cells.iter().find(|c| c.row == 1 && c.col == 1).copied().unwrap();
    drag(&mut h, a0.rect.center(), b1.rect.center());
    assert_eq!(h.state().session.sel.on_page, notecraft_engine::selection::PageSel::Cells { table: tid, rows: (0, 1), cols: (0, 1) });
    // Dragging a column border sets its width (one undo step).
    let undo = h.state().session.undo_labels().len();
    let tl = h.state().canvas.tables[0].clone();
    let edge = egui::pos2(tl.col_edges[0], tl.rect.center().y);
    drag(&mut h, edge, edge + egui::vec2(60.0, 0.0));
    let w = table(&h, tid).columns[0];
    assert!((w - (notecraft_engine::doc::table::DEFAULT_COL_WIDTH + 60.0 / crate::canvas::PT)).abs() < 3.0, "NC.Table.ColumnWidth: {w}");
    assert_eq!(h.state().session.undo_labels().len(), undo + 1);
    // A Table Layout button runs its command.
    h.state_mut().ui.tab = "Table Layout".into();
    h.run();
    let c = table(&h, tid).rows[0].cells[0].id;
    h.state_mut().session.run("caret.set", json!({"block": c.to_string(), "para": 0, "offset": 0})).unwrap();
    h.run();
    h.get_by_label("Insert Below").click();
    h.run();
    assert_eq!(table(&h, tid).n_rows(), 4, "CT.TableLayout.Insert.TableInsertRowsBelow from the ribbon");
    // Outside the table the tab goes away and the ribbon falls back to Home.
    h.state_mut().session.run("nav.pageTitle", json!({})).unwrap();
    h.run();
    assert!(crate::chrome::contextual_tabs(h.state()).is_empty());
    assert_eq!(crate::chrome::current_tab(h.state()).map(|t| t.name), Some("Home"));
}

#[test]
fn merged_cells_lay_out_as_one_and_split_back() {
    let (mut a, tid) = page_with_table();
    a.session.run("ui.ribbonMode", json!({"mode": "classic"})).unwrap();
    let mut h = harness(a);
    h.run();
    let tl = h.state().canvas.tables[0].clone();
    let at = |r: usize, c: usize| tl.cells.iter().find(|x| x.row == r && x.col == c).copied().unwrap();
    let (a0, b0, a1) = (at(0, 0), at(0, 1), at(1, 0));
    // Drag across the first two cells of the top row, then Table Layout › Merge Cells.
    drag(&mut h, a0.rect.center(), b0.rect.center());
    h.state_mut().ui.tab = "Table Layout".into();
    h.run();
    h.get_by_label("Merge Cells").click();
    h.run();
    assert_eq!((table(&h, tid).rows[0].cells[0].col_span, table(&h, tid).rows[0].cells[0].row_span), (2, 1));
    let tl = h.state().canvas.tables[0].clone();
    assert_eq!(tl.cells.len(), 5, "the covered place isn't laid out");
    let merged = tl.cells.iter().find(|x| x.row == 0 && x.col == 0).copied().unwrap();
    assert!((merged.rect.width() - (a0.rect.width() + b0.rect.width())).abs() < 0.5, "it spans both columns");
    // Typing in it wraps at the merged width; the row below keeps its own cells.
    h.state_mut().session.run("text.insert", json!({"text": "wide"})).unwrap();
    h.run();
    assert!(tl.cells.iter().any(|x| x.row == 1 && x.col == 0 && (x.rect.width() - a1.rect.width()).abs() < 0.5));
    h.get_by_label("Split Cells").click();
    h.run();
    assert_eq!(h.state().canvas.tables[0].cells.len(), 6, "split back into single cells");
}

#[test]
fn equations_show_built_up_and_edit_as_linear_text() {
    // parity: R.Insert.Symbols.InsertBuildingBlocksEquationsGallery, CT.Equation.Tools.EquationInsertGallery, CM.Equation.Cut, CM.Equation.PasteGalleryMini
    let (mut a, o) = page_for_context_menu("Intro");
    a.ui.tab = "Insert".into();
    let mut h = harness(a);
    h.run();
    // Insert › Equation › a built-in equation: a new equation paragraph, the Equation tab shows.
    h.get_by_label("Equation").click();
    h.run();
    h.get_by_label("Quadratic formula").click();
    h.run();
    let para = |h: &Harness<'static, NoteApp>| h.state().session.page().unwrap().outline(o).unwrap().paragraphs[1].clone();
    assert_eq!((para(&h).text(), para(&h).math), (notecraft_engine::EQUATIONS[2].1.to_string(), Some(notecraft_engine::doc::MathView::Professional)));
    assert!(crate::chrome::contextual_tabs(h.state()).iter().any(|t| t.name == "Equation"), "the Equation tab while the caret is in it");
    let editing = h.state().canvas.para(Block::Outline(o), 1).unwrap().clone();
    assert!(editing.math.is_none(), "linear text while the caret is in it");
    // Elsewhere, it shows built up: taller than a line of text.
    h.state_mut().session.run("caret.set", json!({"block": o.to_string(), "para": 0, "offset": 0})).unwrap();
    h.run();
    let shown = h.state().canvas.para(Block::Outline(o), 1).unwrap().clone();
    let m = shown.math.clone().expect("built up");
    assert!(m.height() > editing.galley.size().y * 1.5, "a stacked fraction: {} vs {}", m.height(), editing.galley.size().y);
    // Its context menu: Linear, then the equation shows as typed even without the caret.
    right_click(&mut h, shown.rect().center());
    assert!(h.query_by_label("Professional").is_some() && h.query_by_label("Paste Options:").is_some(), "CM.Equation");
    h.get_by_label("Linear").click();
    h.run();
    assert_eq!(para(&h).math, Some(notecraft_engine::doc::MathView::Linear));
    h.state_mut().session.run("caret.set", json!({"block": o.to_string(), "para": 0, "offset": 0})).unwrap();
    h.run();
    assert!(h.state().canvas.para(Block::Outline(o), 1).unwrap().math.is_none(), "Linear shows the text");
}

#[test]
fn the_insert_table_grid_inserts_below_the_caret_container() {
    let (a, o) = page_with("Above the table");
    let mut h = harness(a);
    h.run();
    h.state_mut().ui.tab = "Insert".into();
    h.run();
    // The Table button opens the size grid; hovering 3 columns × 2 rows and clicking inserts.
    h.get_by_label("Table").click();
    h.run();
    let grid = h.get_by_label("Table size grid").rect();
    let cell = 18.0;
    let p = egui::pos2(grid.min.x + 2.0 + 2.5 * cell, grid.min.y + 22.0 + 1.5 * cell);
    h.event(egui::Event::PointerMoved(p));
    h.run();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
    let page = h.state().session.page().unwrap().clone();
    let t = page.objects.iter().find_map(|x| x.as_table()).expect("a table was inserted");
    assert_eq!((t.n_rows(), t.n_cols()), (2, 3));
    let ol = page.outline(o).unwrap();
    assert_eq!(t.pos.x, ol.pos.x);
    assert!(t.pos.y > ol.pos.y + 10.0, "below the container: {} vs {}", t.pos.y, ol.pos.y);
}

#[test]
fn the_mini_toolbar_shows_on_a_text_selection_and_formats_it() {
    // parity: UI.Canvas.MiniToolbar
    let (mut a, o) = page_with("Hello world");
    a.session.run("caret.set", json!({"block": o.to_string(), "para": 0, "offset": 0})).unwrap();
    a.session.run("caret.set", json!({"block": o.to_string(), "para": 0, "offset": 5, "extend": true})).unwrap();
    // The Home tab would show the same controls; switch away so the labels are unambiguous.
    a.ui.tab = "View".into();
    let mut h = harness(a);
    h.run();
    assert!(h.query_by_label("Bold").is_some(), "the mini toolbar shows on a text selection");
    h.get_by_label("Bold").click();
    h.run();
    let p = &h.state().session.page().unwrap().objects[0].as_outline().unwrap().paragraphs[0];
    assert!(p.runs.iter().any(|r| r.text == "Hello" && r.fmt.bold == Some(true)), "Bold in the mini toolbar applies to the selection");
    // A caret alone (no selection) hides it.
    h.state_mut().session.run("caret.set", json!({"block": o.to_string(), "para": 0, "offset": 0})).unwrap();
    h.run();
    assert!(h.query_by_label("Bold").is_none(), "no mini toolbar on a plain caret");
    // So does a selection in the page title.
    h.state_mut().session.run("nav.pageTitle", json!({})).unwrap();
    h.run();
    assert!(h.query_by_label("Bold").is_none(), "UI.Canvas.MiniToolbar: not in the title");
}

/// A fresh page with a container well down the page (room above it for the mini toolbar, which
/// pops up over a text selection and would otherwise cover the very click a test is about to
/// make), on the View tab (so the Home ribbon's identical catalog labels — Cut, Order… — don't
/// make a context-menu label ambiguous).
fn page_for_context_menu(text: &str) -> (NoteApp, Id) {
    let (mut a, o) = page_with(text);
    a.session.run("outline.move", json!({"id": o.to_string(), "x": 60.0, "y": 400.0})).unwrap();
    a.ui.tab = "View".into();
    (a, o)
}

fn table_texts(h: &Harness<'static, NoteApp>, o: Id) -> Vec<String> {
    h.state().session.page().unwrap().outline(o).unwrap().paragraphs.iter().map(|p| p.text()).collect()
}

// One right click per harness in these tests, each its own `app()`/`harness()`: a *second*
// secondary click on the canvas's one shared response, reopening the popup within the same
// kittest session, closes it again within a frame or two regardless of target or timing (a
// kittest/egui interaction, not a real one — a single right click stays open indefinitely, and
// real interactive use always has real frames with other input between two right clicks).

#[test]
fn right_click_on_text_moves_the_caret_and_opens_the_text_menu() {
    let (a, o) = page_for_context_menu("Alpha\nBeta");
    let mut h = harness(a);
    h.run();
    // No existing selection: the caret moves to the click (Cut/Copy are then correctly
    // disabled — honestly, not fake UI), and the Table submenu is there.
    let pl = h.state().canvas.para(Block::Outline(o), 1).unwrap().clone();
    right_click(&mut h, pl.pos + egui::vec2(4.0, 2.0));
    assert!(h.query_by_label("Cut").is_some(), "CM.Text: right click opens the context menu");
    // Submenu triggers carry a "▸" suffix (egui's own `menu_button`), matched with `_contains`.
    assert!(h.query_by_label_contains("Table").is_some(), "the Text menu has the Table submenu");
    let c = h.state().session.sel.caret().unwrap();
    assert_eq!((c.block, c.para, c.offset), (Block::Outline(o), 1, 0), "the caret follows the right click");
}

#[test]
fn right_click_inside_a_selection_keeps_it_and_cut_acts_on_it() {
    // parity: CM.Text.Cut
    let (mut a, o) = page_for_context_menu("Alpha\nBeta");
    a.session.run("caret.set", json!({"block": o.to_string(), "para": 1, "offset": 0})).unwrap();
    a.session.run("caret.set", json!({"block": o.to_string(), "para": 1, "offset": 4, "extend": true})).unwrap();
    let mut h = harness(a);
    h.run();
    let pl = h.state().canvas.para(Block::Outline(o), 1).unwrap().clone();
    right_click(&mut h, pl.pos + egui::vec2(4.0, 2.0));
    let r = h.state().session.text_range().unwrap();
    assert!(!r.is_caret(), "a right click inside a selection keeps it, unlike a click outside one");
    h.get_by_label("Cut").click();
    h.run();
    assert_eq!(table_texts(&h, o), vec!["Alpha", ""], "Cut from the context menu acted on the kept selection");
}

#[test]
fn right_click_on_the_paragraph_handle_selects_the_branch_and_opens_the_handle_menu() {
    let (a, o) = page_for_context_menu("Alpha\nBeta");
    let mut h = harness(a);
    h.run();
    let handle = h.state().canvas.para(Block::Outline(o), 0).unwrap().handle.center();
    right_click(&mut h, handle);
    assert!(h.query_by_label("Cut").is_some(), "CM.ParagraphHandle also has Cut");
    assert!(h.query_by_label_contains("Select Outline Levels").is_some());
    // CM.ParagraphHandle has its own Table submenu too (inserting a table from the handle).
    assert!(h.query_by_label_contains("Table").is_some());
    let r = h.state().session.text_range().unwrap();
    assert_eq!((r.ordered().0.para, r.ordered().1.para), (0, 0), "right-clicking the handle selects its paragraph");
}

#[test]
fn right_click_on_a_container_bar_selects_it_opens_the_note_container_menu_and_reorders() {
    // parity: CM.NoteContainer.ObjectBringToFront
    let (mut a, o) = page_for_context_menu("Alpha\nBeta");
    a.session.run("insert.table", json!({"rows": 1, "cols": 1, "x": 60.0, "y": 400.0})).unwrap();
    let mut h = harness(a);
    h.run();
    let ol = h.state().canvas.outlines.iter().find(|o| !o.table).copied().unwrap();
    right_click(&mut h, ol.bar.center());
    assert!(h.query_by_label_contains("Order").is_some(), "CM.NoteContainer has the Order submenu");
    assert!(h.query_by_label("Cut").is_some(), "CM.NoteContainer also has Cut");
    assert!(h.query_by_label_contains("Table").is_none(), "no Table submenu on a container's own bar");
    let sel = h.state().session.sel.on_page.clone();
    assert!(matches!(sel, notecraft_engine::selection::PageSel::Objects { ids } if ids == vec![o]), "the bar selects the container");
    let before = h.state().session.page().unwrap().objects.iter().map(|x| x.id()).collect::<Vec<_>>();
    h.get_by_label_contains("Order").click();
    h.run();
    h.get_by_label("Bring to Front").click();
    h.run();
    let after = h.state().session.page().unwrap().objects.iter().map(|x| x.id()).collect::<Vec<_>>();
    assert_ne!(before, after, "Order/Bring to Front from the context menu reordered the objects");
    assert_eq!(after.last(), Some(&o));
}

#[test]
fn the_container_menus_move_then_arrow_keys_nudge_the_container() {
    // parity: CM.NoteContainer.OrderObjectsMenu
    let (a, o) = page_for_context_menu("Alpha\nBeta");
    let mut h = harness(a);
    h.run();
    let ol = h.state().canvas.outlines.iter().find(|x| !x.table).copied().unwrap();
    right_click(&mut h, ol.bar.center());
    assert!(h.query_by_label_contains("Order").is_some(), "CM.NoteContainer.OrderObjectsMenu");
    h.get_by_label("Move").click();
    h.run();
    let at = |h: &Harness<'static, NoteApp>| h.state().session.page().unwrap().outline(o).unwrap().pos;
    let before = at(&h);
    for _ in 0..3 {
        h.key_press(egui::Key::ArrowRight);
        h.run();
    }
    h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::ArrowDown);
    h.run();
    assert_eq!((at(&h).x - before.x, at(&h).y - before.y), (3.0, 10.0), "CM.NoteContainer.ObjectMove: arrows nudge 1 pt, 10 with Ctrl");
}

#[test]
fn the_container_menus_resize_then_shift_arrows_change_its_width() {
    let (a, o) = page_for_context_menu("Alpha\nBeta");
    let mut h = harness(a);
    h.run();
    let ol = h.state().canvas.outlines.iter().find(|x| !x.table).copied().unwrap();
    right_click(&mut h, ol.bar.center());
    h.get_by_label("Resize").click();
    h.run();
    let width = |h: &Harness<'static, NoteApp>| match h.state().session.page().unwrap().outline(o).unwrap().width {
        notecraft_engine::doc::OutlineWidth::Fixed(w) => w,
        w => panic!("not resized: {w:?}"),
    };
    let auto = h.state().canvas.outlines.iter().find(|x| x.id == o).map(|x| (x.rect.max.x - 4.0 - x.left) / crate::canvas::PT).unwrap();
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::ArrowRight);
    h.run();
    let w1 = width(&h);
    assert!((w1 - auto - 1.0).abs() < 0.01, "from its laid-out width ({auto}) one wider: {w1}");
    for _ in 0..2 {
        h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::ArrowLeft);
        h.run();
    }
    assert!((width(&h) - (w1 - 2.0)).abs() < 0.01, "CM.NoteContainer.ObjectResize: Shift+Left narrows by 1 pt ({w1} → {})", width(&h));
    let undo = h.state().session.undo_labels().len();
    assert!(undo > 0);
}

#[test]
fn the_handle_menus_table_submenu_opens_insert_table() {
    // parity: CM.ParagraphHandle.TableMenu, CM.ParagraphHandle.TableInsert
    let (a, o) = page_for_context_menu("Alpha\nBeta");
    let mut h = harness(a);
    h.run();
    let handle = h.state().canvas.para(Block::Outline(o), 0).unwrap().handle.center();
    right_click(&mut h, handle);
    h.get_by_label_contains("Table").click();
    h.run();
    h.get_by_label("Insert Table").click();
    h.run();
    assert_eq!(h.state().ui.dialog, Some(crate::dialogs::Dialog::InsertTable(2, 2)), "Insert Table… from the handle's Table submenu");
}

#[test]
fn right_click_in_a_table_cell_opens_the_text_menu_with_the_table_submenu() {
    // parity: CM.Text.SelectTableOneNote
    let (mut a, tid) = page_with_table();
    a.ui.tab = "View".into();
    let mut h = harness(a);
    h.run();
    let cell = h.state().canvas.tables[0].cells[0];
    right_click(&mut h, cell.rect.center());
    assert_eq!(h.state().session.sel.caret().map(|c| c.block), Some(Block::Cell(cell.id)), "the caret follows the click into the cell");
    assert!(h.query_by_label("Cut").is_some(), "CM.Text in a table cell");
    // "Table ⏵" (the submenu trigger), not "Table Layout" (the contextual ribbon tab's name,
    // shown in the tab row even while another tab is selected) or "Select Table" (a child).
    h.get_by_label_contains("Table ⏵").click();
    h.run();
    h.get_by_label("Select Table").click();
    h.run();
    assert_eq!(h.state().session.sel.on_page, notecraft_engine::selection::PageSel::Cells { table: tid, rows: (0, 1), cols: (0, 2) });
}

#[test]
fn paste_options_shows_after_a_paste_and_offers_the_three_modes() {
    // parity: UI.Canvas.PasteOptions
    let (mut a, o) = page_with("Hello");
    a.ui.tab = "View".into();
    let mut h = harness(a);
    h.run();
    h.state_mut().session.run("caret.set", json!({"block": o.to_string(), "para": 0, "offset": 0})).unwrap();
    assert!(h.state().ui.paste_shown_at.is_none(), "nothing pasted yet");
    let ctx = h.ctx.clone();
    h.state_mut().try_run(&ctx, "edit.paste", json!({"text": "Hi "})).unwrap();
    h.run();
    assert_eq!(table_texts(&h, o), vec!["Hi Hello"]);
    assert!(h.state().ui.paste_shown_at.is_some(), "UI.Canvas.PasteOptions: shown after a paste");
    h.get_by_label("Paste Options").click();
    h.run();
    assert!(h.query_by_label("Paste: Keep Text Only").is_some());
    h.get_by_label("Paste: Keep Text Only").click();
    h.run();
    // Still "Hi Hello": Keep Text Only re-pastes the same plain text, just stripped of any
    // formatting (there is none here yet — no rich clipboard, catalog gaps.md item 3).
    assert_eq!(table_texts(&h, o), vec!["Hi Hello"]);
}

#[test]
fn typing_then_tab_makes_a_table_and_tab_adds_columns() {
    // KB.Table.CreateWithTab, through the real keymap.
    let (a, _) = page_with("Fruit");
    let mut h = harness(a);
    h.run();
    h.key_press(egui::Key::Tab);
    h.run();
    h.event(egui::Event::Text("Qty".into()));
    h.run();
    h.key_press(egui::Key::Tab);
    h.run();
    let page = h.state().session.page().unwrap();
    let t = page.objects.iter().find_map(|o| o.as_table()).unwrap();
    let row: Vec<String> = t.rows[0].cells.iter().map(|c| c.text()).collect();
    assert_eq!(row, ["Fruit", "Qty", ""], "BEH.Table.TabToCreate");
    assert!(page.objects.iter().all(|o| o.as_outline().is_none()), "the container became the table");
    assert_eq!(h.state().canvas.tables.len(), 1, "drawn as a table");
}

#[test]
fn open_link_is_enabled_only_on_a_link() {
    let (mut a, o) = page_with("see ");
    let caret = |a: &mut NoteApp, offset: usize| a.session.run("caret.set", json!({"block": o.to_string(), "para": 0, "offset": offset})).unwrap();
    caret(&mut a, 2);
    assert!(a.disabled_reason("link.open").is_some(), "KB.Format.OpenLink is disabled off a link");
    a.session.run("insert.link", json!({"url": "https://example.org", "text": "example"})).unwrap();
    caret(&mut a, 6);
    assert_eq!(a.disabled_reason("link.open"), None, "and enabled on one");
    // A click on a link still opens it: the command itself takes the url.
    assert!(a.session.run("link.open", json!({"url": "https://example.org"})).is_ok());
}

#[test]
fn the_text_menu_rows_reach_working_commands_in_the_right_state() {
    // Each row sits in the Text menu and runs its command when the state calls for it.
    let ids = |list: &[crate::context_menu::Entry]| -> Vec<&'static str> { list.iter().flat_map(crate::context_menu::ids).collect() };
    let in_menu = ids(crate::context_menu::TEXT_MENU);
    let check = |a: &NoteApp, row: &str| {
        assert!(in_menu.contains(&row), "{row} is in the Text menu");
        let cmd = crate::ribbon::command(row).unwrap_or_else(|| panic!("{row} has a command"));
        assert_eq!(a.disabled_reason(cmd), None, "{row} → {cmd} is enabled");
    };
    // A picture selected: the Rotate submenu, background and alt text.
    let mut a = app();
    a.session.run("page.new", json!({})).unwrap();
    let data = notecraft_engine::b64::encode(&crate::tests_pictures::png(16, 8));
    a.session.run("insert.picture", json!({"data": data})).unwrap();
    for row in [
        "CM.Text.RotateInkMenu",
        "CM.Text.ObjectRotateRight90",
        "CM.Text.ObjectRotateLeft90",
        "CM.Text.ObjectFlipHorizontal",
        "CM.Text.ObjectFlipVertical",
        "CM.Text.PictureAsBackground",
        "CM.Text.AlternativeText",
    ] {
        if row == "CM.Text.RotateInkMenu" {
            assert!(in_menu.contains(&row), "CM.Text.RotateInkMenu");
            continue;
        }
        check(&a, row);
    }
    // The caret in a table: the Table submenu.
    let (a, _) = page_with_table();
    assert!(in_menu.contains(&"CM.Text.TableMenu"), "CM.Text.TableMenu");
    check(&a, "CM.Text.TableInsert");
    // Words selected: Link to Page.
    let (mut a, o) = page_with("Garden");
    a.session.run("caret.set", json!({"block": o.to_string(), "para": 0, "offset": 0})).unwrap();
    a.session.run("caret.set", json!({"block": o.to_string(), "para": 0, "offset": 6, "extend": true})).unwrap();
    check(&a, "CM.Text.ToPageLink");
}

#[test]
fn right_click_on_a_tag_icon_opens_the_tag_menu_for_that_tag() {
    // parity: CM.Tag.TagsRemove
    let (mut a, o) = page_for_context_menu("call the plumber");
    a.session.run("tag.todo", json!({})).unwrap();
    a.session.run("tag.important", json!({})).unwrap();
    let mut h = harness(a);
    h.run();
    let pl = h.state().canvas.para(Block::Outline(o), 0).unwrap().clone();
    assert_eq!(pl.tag_icons.len(), 2);
    let (r, k) = pl.tag_icons[1];
    right_click(&mut h, r.center());
    for item in ["Check Tag", "Remove Tag", "Find Tags", "Customize Tags"] {
        assert!(h.query_by_label(item).is_some(), "CM.Tag.FindTaggedNotes / CM.Tag.TagsCustomize: {item}");
    }
    let key = h.state().session.page().unwrap().outline(o).unwrap().paragraphs[0].tags[k].key.clone();
    assert_eq!(h.state().session.view.context_tag.as_deref(), Some(key.as_str()));
    h.get_by_label("Remove Tag").click();
    h.run();
    let left: Vec<String> = h.state().session.page().unwrap().outline(o).unwrap().paragraphs[0].tags.iter().map(|t| t.key.clone()).collect();
    assert_eq!(left.len(), 1, "CM.Tag.TagsRemove removes the tag that was clicked");
    assert!(!left.contains(&key));
}

#[test]
fn right_click_on_a_misspelled_word_offers_its_corrections() {
    // parity: CM.MisspelledWord.NoSpellingSuggestions
    let (mut a, o) = page_for_context_menu("the cta sat");
    a.session.set_dictionary("en_US", crate::tests::test_dictionary());
    let mut h = harness(a);
    h.run();
    let pl = h.state().canvas.para(Block::Outline(o), 0).unwrap().clone();
    right_click(&mut h, pl.caret_rect(5).center());
    assert!(h.query_by_label("cat").is_some(), "CM.Text.NoSpellingSuggestions: the suggestion is in the menu");
    assert!(h.query_by_label("Add to Dictionary").is_some());
    h.get_by_label("cat").click();
    h.run();
    let text = h.state().session.page().unwrap().outline(o).unwrap().paragraphs[0].text();
    assert_eq!(text, "the cat sat", "CM.MisspelledWord.NoSpellingSuggestions");
}
