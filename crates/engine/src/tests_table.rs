//! Table commands: insert, select, delete, insert rows and columns, borders, shading, sort,
//! conversion to and from text, the keys that move through cells, undo and limits.

use notecraft_doc::{Id, PageObject, Table};
use serde_json::json;

use crate::Session;
use crate::selection::{Block, PageSel, Pos};
use crate::tests::{blank_page, run, sample};

fn table(s: &Session) -> Table {
    s.page().unwrap().objects.iter().find_map(PageObject::as_table).unwrap().clone()
}

fn grid(s: &Session) -> Vec<Vec<String>> {
    table(s).rows.iter().map(|r| r.cells.iter().map(|c| c.text()).collect()).collect()
}

fn caret_cell(s: &Session) -> (usize, usize) {
    let Block::Cell(c) = s.sel.caret().unwrap().block else { panic!("caret not in a cell: {:?}", s.sel.on_page) };
    let (_, r, col) = s.page().unwrap().table_of_cell(c).unwrap();
    (r, col)
}

/// A blank page with a `rows` × `cols` table filled with "r{row}c{col}".
fn filled(rows: usize, cols: usize) -> Session {
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "insert.table", json!({"rows": rows, "cols": cols}));
    for r in 0..rows {
        for c in 0..cols {
            let id = table(&s).rows[r].cells[c].id;
            run(&mut s, "caret.set", json!({"block": id.to_string(), "para": 0, "offset": 0}));
            run(&mut s, "text.insert", json!({"text": format!("r{r}c{c}")}));
        }
    }
    s
}

fn at(s: &mut Session, r: usize, c: usize) {
    let id = table(s).rows[r].cells[c].id;
    run(s, "caret.set", json!({"block": id.to_string(), "para": 0, "offset": 0}));
}

#[test]
fn insert_a_table_and_type_in_it() {
    // parity: R.Insert.Tables.TableInsertGallery
    let mut s = sample();
    blank_page(&mut s);
    let undo = s.undo_labels().len();
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 3}));
    assert_eq!(s.undo_labels().len(), undo + 1, "one undo step");
    let t = table(&s);
    assert_eq!((t.n_rows(), t.n_cols()), (2, 3));
    assert_eq!(caret_cell(&s), (0, 0), "the caret starts in the first cell");
    run(&mut s, "text.insert", json!({"text": "Name"}));
    assert_eq!(grid(&s)[0][0], "Name");
    assert!(s.page().unwrap().plain_text().contains("Name\t\t"), "page text has the table as tab-separated rows");
    // Search finds text in cells.
    run(&mut s, "search.page", json!({"query": "name"}));
    assert!(matches!(s.view.find.matches.first(), Some((Block::Cell(_), 0, 0, 4))), "{:?}", s.view.find.matches);
    // Hostile sizes are refused, not clamped into a huge allocation.
    for (r, c) in [(0, 2), (2, 0), (100_000, 2), (2, 65), (5000, 64)] {
        assert!(s.run("insert.table", json!({"rows": r, "cols": c})).is_err(), "{r}×{c}");
    }
    // Undo takes the whole table away.
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    assert!(s.page().unwrap().objects.iter().all(|o| o.as_table().is_none()));
}

#[test]
fn tab_enter_and_arrows_move_through_cells() {
    // parity: NC.Table.NextCell, NC.Table.PrevCell, KB.Table.TabInLastCell, KB.Table.NewColumnTab, KB.Table.NewRowEnter, KB.Table.ParagraphInCell
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "insert.table", json!({"rows": 1, "cols": 2}));
    run(&mut s, "text.insert", json!({"text": "a"}));
    run(&mut s, "table.nextCell", json!({}));
    assert_eq!(caret_cell(&s), (0, 1));
    run(&mut s, "text.insert", json!({"text": "b"}));
    // Tab in the last cell of a one-row table adds a column.
    run(&mut s, "table.nextCell", json!({}));
    assert_eq!((table(&s).n_cols(), caret_cell(&s)), (3, (0, 2)), "KB.Table.NewColumnTab");
    run(&mut s, "table.prevCell", json!({}));
    assert_eq!(caret_cell(&s), (0, 1));
    assert_eq!(s.sel.caret().unwrap().offset, 1, "Shift+Tab lands at the end of the cell");
    // Enter: a new row, the caret in its first cell.
    run(&mut s, "table.enter", json!({}));
    assert_eq!((table(&s).n_rows(), caret_cell(&s)), (2, (1, 0)), "KB.Table.NewRowEnter");
    // Tab from the last cell of a longer table adds a row.
    for _ in 0..3 {
        run(&mut s, "table.nextCell", json!({}));
    }
    assert_eq!((table(&s).n_rows(), caret_cell(&s)), (3, (2, 0)), "KB.Table.TabInLastCell");
    // Enter on the empty last row ends the table: the row goes, typing starts below.
    run(&mut s, "table.enter", json!({}));
    assert_eq!(table(&s).n_rows(), 2);
    assert!(s.sel.caret().is_none() && s.pending_outline.is_some());
    run(&mut s, "text.insert", json!({"text": "after"}));
    let o = s.page().unwrap().objects.iter().find_map(PageObject::as_outline).unwrap().clone();
    assert!(o.pos.y > table(&s).pos.y, "the new container is below the table");
    // Enter at the start of a later row inserts a row above it.
    at(&mut s, 1, 0);
    run(&mut s, "text.insert", json!({"text": "x"}));
    run(&mut s, "caret.lineStart", json!({}));
    run(&mut s, "table.enter", json!({}));
    assert_eq!(grid(&s)[1], vec!["", "", ""], "KB.Table.RowAboveEnter: a row above");
    assert_eq!(grid(&s)[2][0], "x");
    // Alt+Enter: another paragraph in the same cell.
    at(&mut s, 0, 0);
    run(&mut s, "caret.lineEnd", json!({}));
    run(&mut s, "table.newParagraphInCell", json!({}));
    run(&mut s, "text.insert", json!({"text": "second"}));
    assert_eq!(table(&s).rows[0].cells[0].paragraphs.len(), 2);
    assert_eq!(grid(&s)[0][0], "a\nsecond");
    // Arrows cross cell edges.
    at(&mut s, 0, 1);
    run(&mut s, "caret.left", json!({}));
    assert_eq!(caret_cell(&s), (0, 0), "Left at a cell's start goes to the previous cell");
    at(&mut s, 0, 1);
    run(&mut s, "caret.down", json!({}));
    assert_eq!(caret_cell(&s), (1, 1), "Down from a cell's last line goes to the cell below");
}

#[test]
fn select_rows_columns_cells_and_the_table() {
    // parity: CT.TableLayout.Select.SelectTableOneNote, CT.TableLayout.Select.SelectColumns, CT.TableLayout.Select.SelectRows, CT.TableLayout.Select.SelectCell, NC.Table.SelectRange
    let mut s = filled(3, 3);
    let tid = table(&s).id;
    at(&mut s, 1, 2);
    run(&mut s, "table.selectRows", json!({}));
    assert_eq!(s.sel.on_page, PageSel::Cells { table: tid, rows: (1, 1), cols: (0, 2) });
    at(&mut s, 1, 2);
    run(&mut s, "table.selectColumns", json!({}));
    assert_eq!(s.sel.on_page, PageSel::Cells { table: tid, rows: (0, 2), cols: (2, 2) });
    at(&mut s, 1, 2);
    run(&mut s, "table.selectCell", json!({}));
    assert_eq!(s.sel.on_page, PageSel::Cells { table: tid, rows: (1, 1), cols: (2, 2) });
    run(&mut s, "table.selectTable", json!({}));
    assert_eq!(s.sel.on_page, PageSel::Cells { table: tid, rows: (0, 2), cols: (0, 2) });
    run(&mut s, "table.selectRange", json!({"table": tid.to_string(), "rows": [2, 1], "cols": [0, 1]}));
    assert_eq!(s.sel.on_page, PageSel::Cells { table: tid, rows: (1, 2), cols: (0, 1) });
    assert!(s.run("table.selectRange", json!({"table": tid.to_string(), "rows": [0, 9], "cols": [0, 0]})).is_err());
    // Formatting applies to every selected cell; Copy gives tab-separated rows; Delete empties.
    run(&mut s, "format.bold", json!({}));
    let t = table(&s);
    assert!(t.rows[1].cells[0].paragraphs[0].runs.iter().all(|r| r.fmt.bold == Some(true)));
    assert!(t.rows[0].cells[0].paragraphs[0].runs.iter().all(|r| r.fmt.bold != Some(true)));
    let copied = s.run("edit.copy", json!({})).unwrap();
    assert_eq!(copied["text"], "r1c0\tr1c1\nr2c0\tr2c1");
    run(&mut s, "edit.delete", json!({}));
    assert_eq!(grid(&s)[2], vec!["", "", "r2c2"]);
    // The Table Layout alignment buttons are the paragraph alignment commands, in a cell.
    at(&mut s, 0, 0);
    for (cmd, row, align) in [
        ("para.alignCenter", "CT.TableLayout.Alignment.AlignCenter", notecraft_doc::Align::Center),
        ("para.alignRight", "CT.TableLayout.Alignment.AlignRight", notecraft_doc::Align::Right),
        ("para.alignLeft", "CT.TableLayout.Alignment.AlignLeft", notecraft_doc::Align::Left),
    ] {
        run(&mut s, cmd, json!({}));
        assert_eq!(table(&s).rows[0].cells[0].paragraphs[0].align, align, "{row}");
    }
    // Selection commands are disabled outside a table.
    run(&mut s, "nav.pageTitle", json!({}));
    assert!(s.run("table.selectTable", json!({})).is_err());
}

#[test]
fn insert_and_delete_rows_and_columns() {
    // parity: CT.TableLayout.Insert.TableInsertRowsAbove, CT.TableLayout.Insert.TableInsertRowsBelow, CT.TableLayout.Insert.TableInsertColumnsLeft, CT.TableLayout.Insert.TableInsertColumnsRight, CT.TableLayout.Delete.DeleteRowsOneNote, CT.TableLayout.Delete.DeleteColumnsOneNote, CT.TableLayout.Delete.DeleteTableOneNote
    let mut s = filled(2, 2);
    at(&mut s, 0, 1);
    let undo = s.undo_labels().len();
    run(&mut s, "table.insertAbove", json!({}));
    assert_eq!(grid(&s), vec![vec!["", ""], vec!["r0c0", "r0c1"], vec!["r1c0", "r1c1"]]);
    assert_eq!(caret_cell(&s), (0, 1), "the caret goes to the new row");
    run(&mut s, "table.insertBelow", json!({}));
    assert_eq!(grid(&s)[1], vec!["", ""]);
    run(&mut s, "table.insertLeft", json!({}));
    assert_eq!(grid(&s)[2], vec!["r0c0", "", "r0c1"]);
    assert_eq!(table(&s).columns.len(), 3);
    run(&mut s, "table.insertRight", json!({}));
    assert_eq!(grid(&s)[2], vec!["r0c0", "", "", "r0c1"]);
    assert_eq!(s.undo_labels().len(), undo + 4, "one undo step each");
    // Delete the column and rows just added.
    at(&mut s, 2, 1);
    let tid = table(&s).id.to_string();
    run(&mut s, "table.selectRange", json!({"table": tid, "rows": [0, 0], "cols": [1, 2]}));
    run(&mut s, "table.deleteColumns", json!({}));
    assert_eq!(grid(&s)[2], vec!["r0c0", "r0c1"]);
    at(&mut s, 0, 0);
    run(&mut s, "table.deleteRows", json!({}));
    at(&mut s, 0, 0);
    run(&mut s, "table.deleteRows", json!({}));
    assert_eq!(grid(&s), vec![vec!["r0c0", "r0c1"], vec!["r1c0", "r1c1"]]);
    // Deleting every row deletes the table.
    run(&mut s, "table.selectColumns", json!({}));
    run(&mut s, "table.deleteRows", json!({}));
    assert!(s.page().unwrap().objects.iter().all(|o| o.as_table().is_none()));
    run(&mut s, "edit.undo", json!({}));
    at(&mut s, 0, 0);
    run(&mut s, "table.delete", json!({}));
    assert!(s.page().unwrap().objects.iter().all(|o| o.as_table().is_none()), "Delete Table");
}

#[test]
fn borders_shading_header_and_column_width() {
    // parity: CT.TableLayout.Format.TableBordersHide, CT.TableLayout.Format.ShadingColorPicker, CT.TableLayout.Data.HeaderRowNoFormatting, NC.Table.ColumnWidth
    let mut s = filled(2, 2);
    at(&mut s, 0, 0);
    run(&mut s, "table.hideBorders", json!({}));
    assert!(!table(&s).borders);
    run(&mut s, "table.hideBorders", json!({}));
    assert!(table(&s).borders);
    run(&mut s, "table.selectRows", json!({}));
    run(&mut s, "table.shading", json!({"color": "#FFF2CC"}));
    let t = table(&s);
    assert_eq!(t.rows[0].cells[1].shading.map(|c| c.to_hex()), Some("#fff2cc".to_string()));
    assert_eq!(t.rows[1].cells[0].shading, None);
    assert!(s.run("table.shading", json!({"color": "#zz"})).is_err());
    run(&mut s, "table.shading", json!({"color": null}));
    assert_eq!(table(&s).rows[0].cells[0].shading, None);
    run(&mut s, "table.headerRow", json!({}));
    assert!(table(&s).header_row);
    let tid = table(&s).id.to_string();
    run(&mut s, "table.columnWidth", json!({"table": tid, "col": 1, "width": 1e12}));
    assert_eq!(table(&s).columns[1], notecraft_doc::table::MAX_COL_WIDTH);
    run(&mut s, "table.columnWidth", json!({"table": tid, "col": 1, "width": 150.0}));
    assert_eq!(table(&s).columns[1], 150.0);
    assert!(s.run("table.columnWidth", json!({"table": tid, "col": 9, "width": 50.0})).is_err());
    // The table moves like a container.
    run(&mut s, "outline.move", json!({"id": tid, "x": 300.0, "y": -10.0}));
    assert_eq!((table(&s).pos.x, table(&s).pos.y), (300.0, 0.0));
}

#[test]
fn sort_by_the_caret_column() {
    // CT.TableLayout.Data.SortUp / SortDown (the collation is unverified, so not `parity:`).
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "outline.new", json!({"x": 40.0, "y": 100.0}));
    run(&mut s, "text.insert", json!({"text": "Item\tQty\npear\t10\nApple\t9\n\t3\nbanana\t100"}));
    run(&mut s, "edit.selectAll", json!({}));
    run(&mut s, "edit.selectAll", json!({}));
    run(&mut s, "insert.convertTextToTable", json!({}));
    at(&mut s, 1, 1);
    run(&mut s, "table.headerRow", json!({}));
    run(&mut s, "table.sortAscending", json!({}));
    let col: Vec<String> = grid(&s).iter().map(|r| r[1].clone()).collect();
    assert_eq!(col, vec!["Qty", "3", "9", "10", "100"], "numbers sort as numbers; the header row stays");
    at(&mut s, 1, 0);
    run(&mut s, "table.sortDescending", json!({}));
    let col: Vec<String> = grid(&s).iter().map(|r| r[0].clone()).collect();
    assert_eq!(col, vec!["Item", "pear", "banana", "Apple", ""], "text sorts ignoring case; empty cells last");
}

#[test]
fn the_sort_dialog_sorts_by_several_columns_in_turn() {
    // CT.TableLayout.Data.ShowSortTableRowsDialog (collation unverified, so not `parity:`).
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "outline.new", json!({"x": 40.0, "y": 100.0}));
    run(&mut s, "text.insert", json!({"text": "Fruit\tShop\tPrice\npear\tB\t3\napple\tA\t2\nfig\tB\t1\nkiwi\tA\t5"}));
    run(&mut s, "edit.selectAll", json!({}));
    run(&mut s, "edit.selectAll", json!({}));
    run(&mut s, "insert.convertTextToTable", json!({}));
    at(&mut s, 2, 0);
    let undo = s.undo_labels().len();
    // By shop, then by price falling; the first row becomes the header on the way.
    let keys = json!([{"col": 1}, {"col": 2, "descending": true}]);
    assert_eq!(run(&mut s, "table.sortDialog", json!({"keys": keys, "headerRow": true}))["headerRow"], true);
    assert_eq!(s.undo_labels().len(), undo + 1, "one undo step");
    let fruit: Vec<String> = grid(&s).iter().map(|r| r[0].clone()).collect();
    assert_eq!(fruit, vec!["Fruit", "kiwi", "apple", "pear", "fig"]);
    // Without keys: the caret's column, ascending; without a header row every row sorts.
    at(&mut s, 1, 0);
    run(&mut s, "table.sortDialog", json!({"headerRow": false}));
    let fruit: Vec<String> = grid(&s).iter().map(|r| r[0].clone()).collect();
    assert_eq!(fruit, vec!["apple", "fig", "Fruit", "kiwi", "pear"]);
    // Hostile keys change nothing and add no undo step.
    let undo = s.undo_labels().len();
    assert!(s.run("table.sortDialog", json!({"keys": [{"col": 3}]})).is_err());
    assert!(s.run("table.sortDialog", json!({"keys": []})).is_err());
    assert!(s.run("table.sortDialog", json!({"keys": [{"col": 0}, {"col": 1}, {"col": 2}, {"col": 0}]})).is_err());
    assert!(s.run("table.sortDialog", json!({"keys": [{"col": "x"}]})).is_err());
    assert_eq!(s.undo_labels().len(), undo);
}

fn spans(s: &Session) -> Vec<Vec<(u32, u32)>> {
    table(s).rows.iter().map(|r| r.cells.iter().map(|c| (c.row_span, c.col_span)).collect()).collect()
}

#[test]
fn merge_cells_joins_their_text_and_split_cells_undoes_the_merge() {
    // parity: CT.TableLayout.Merge.MergeCells, CT.TableLayout.Merge.UnmergeCells
    let mut s = filled(3, 3);
    let tid = table(&s).id.to_string();
    let enabled = |s: &Session, id: &str| (crate::commands::find(id).unwrap().enabled)(s);
    at(&mut s, 0, 0);
    assert_eq!(enabled(&s, "table.merge"), Some("Select two or more cells to merge"));
    assert_eq!(enabled(&s, "table.split"), Some("Select a merged cell to split"));
    // Merge the top-left 2 × 2: one undo step, the text joined in reading order.
    run(&mut s, "table.selectRange", json!({"table": tid, "rows": [0, 1], "cols": [0, 1]}));
    let undo = s.undo_labels().len();
    run(&mut s, "table.merge", json!({}));
    assert_eq!(s.undo_labels().len(), undo + 1);
    assert_eq!(spans(&s)[0][0], (2, 2));
    assert_eq!(grid(&s)[0][0], "r0c0\nr0c1\nr1c0\nr1c1");
    assert!(grid(&s)[1][1].is_empty() && table(&s).is_covered(1, 1));
    assert_eq!(caret_cell(&s), (0, 0), "the caret is in the merged cell");
    // Tab skips the places it covers; Shift+Tab comes back.
    run(&mut s, "table.nextCell", json!({}));
    assert_eq!(caret_cell(&s), (0, 2));
    run(&mut s, "table.nextCell", json!({}));
    assert_eq!(caret_cell(&s), (1, 2), "row 1 starts under the merged cell");
    run(&mut s, "table.prevCell", json!({}));
    run(&mut s, "table.prevCell", json!({}));
    assert_eq!(caret_cell(&s), (0, 0));
    // Rows can't be sorted across it.
    assert!(s.run("table.sortAscending", json!({})).is_err());
    // Insert Below from the merged cell goes below all its rows; a row inserted inside joins it.
    run(&mut s, "table.insertBelow", json!({}));
    assert_eq!((table(&s).n_rows(), spans(&s)[0][0]), (4, (2, 2)));
    at(&mut s, 0, 2);
    run(&mut s, "table.insertBelow", json!({}));
    assert_eq!(spans(&s)[0][0], (3, 2), "a row added inside the merged cell joins it");
    // Deleting its first row keeps the merged cell, one row shorter, with its text.
    at(&mut s, 0, 2);
    run(&mut s, "table.deleteRows", json!({}));
    assert_eq!((spans(&s)[0][0], grid(&s)[0][0].starts_with("r0c0")), ((2, 2), true));
    // Split Cells: single cells again, the text in the first.
    at(&mut s, 0, 0);
    assert_eq!(enabled(&s, "table.split"), None);
    run(&mut s, "table.split", json!({}));
    assert!(spans(&s).iter().flatten().all(|x| *x == (1, 1)));
    assert!(grid(&s)[0][0].contains("r1c1"));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(spans(&s)[0][0], (2, 2), "undo merges it again");
}

#[test]
fn convert_between_text_and_tables() {
    // parity: NC.Table.ConvertTextToTable, NC.Table.ConvertToText
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "outline.new", json!({"x": 40.0, "y": 100.0}));
    run(&mut s, "text.insert", json!({"text": "before\na\tb\tc\nd\te\nafter"}));
    let oid = match s.sel.caret().unwrap().block {
        Block::Outline(id) => id,
        b => panic!("caret not in a container: {b:?}"),
    };
    // Bold "b" to check that formatting survives.
    run(&mut s, "caret.set", json!({"block": oid.to_string(), "para": 1, "offset": 2}));
    run(&mut s, "caret.set", json!({"block": oid.to_string(), "para": 1, "offset": 3, "extend": true}));
    run(&mut s, "format.bold", json!({}));
    run(&mut s, "caret.set", json!({"block": oid.to_string(), "para": 1, "offset": 0}));
    run(&mut s, "caret.set", json!({"block": oid.to_string(), "para": 2, "offset": 1, "extend": true}));
    let undo = s.undo_labels().len();
    run(&mut s, "insert.convertTextToTable", json!({}));
    assert_eq!(grid(&s), vec![vec!["a", "b", "c"], vec!["d", "e", ""]]);
    assert_eq!(s.undo_labels().len(), undo + 1, "{:?}", s.undo_labels());
    assert!(table(&s).rows[0].cells[1].paragraphs[0].runs.iter().all(|r| r.fmt.bold == Some(true)), "formatting kept");
    let page = s.page().unwrap().clone();
    let texts: Vec<String> = page.objects.iter().filter_map(PageObject::as_outline).map(|o| o.paragraphs[0].text()).collect();
    assert_eq!(texts, vec!["before", "after"], "the paragraphs around the selection stay in containers");
    assert_eq!(caret_cell(&s), (0, 0));
    // Back to text: a paragraph per row, tabs between cells, formatting kept.
    run(&mut s, "table.convertToText", json!({}));
    let page = s.page().unwrap().clone();
    assert!(page.objects.iter().all(|o| o.as_table().is_none()));
    let o = page.objects.iter().filter_map(PageObject::as_outline).find(|o| o.paragraphs.len() == 2).unwrap();
    assert_eq!(o.paragraphs.iter().map(|p| p.text()).collect::<Vec<_>>(), vec!["a\tb\tc", "d\te\t"]);
    assert!(o.paragraphs[0].runs.iter().any(|r| r.text == "b" && r.fmt.bold == Some(true)));
    // A whole container converts in place.
    run(&mut s, "edit.selectAll", json!({}));
    run(&mut s, "edit.selectAll", json!({}));
    run(&mut s, "insert.convertTextToTable", json!({}));
    assert_eq!(grid(&s).len(), 2);
    // Disabled without a text selection in a container.
    run(&mut s, "nav.pageTitle", json!({}));
    assert!(s.run("insert.convertTextToTable", json!({})).is_err());
}

#[test]
fn tables_copy_with_their_page_and_survive_undo() {
    let mut s = filled(2, 2);
    let pid = s.sel.page.unwrap();
    let sid = s.sel.section.unwrap();
    run(&mut s, "page.moveOrCopy", json!({"section": sid.to_string(), "copy": true}));
    let ids: Vec<Id> =
        s.ws.section(sid)
            .unwrap()
            .pages
            .iter()
            .filter(|p| p.id != pid)
            .filter_map(|p| p.objects.iter().find_map(PageObject::as_table).map(|t| t.id))
            .collect();
    assert_eq!(ids.len(), 1, "the copy has the table");
    run(&mut s, "nav.goto", json!({"page": pid.to_string()}));
    let orig = table(&s).id;
    assert!(!ids.contains(&orig), "a copied page gets fresh table ids");
    // Every caret position in a cell is clamped like any other block.
    let cell = table(&s).rows[0].cells[0].id;
    assert_eq!(s.clamp_pos(Pos::new(Block::Cell(cell), 9, 999)).map(|p| (p.para, p.offset)), Some((0, 4)));
}

#[test]
fn delete_at_the_start_of_an_empty_row_removes_it() {
    // parity: KB.Table.DeleteEmptyRow
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "insert.table", json!({"rows": 3, "cols": 2}));
    at(&mut s, 0, 0);
    run(&mut s, "text.insert", json!({"text": "top"}));
    at(&mut s, 2, 0);
    run(&mut s, "text.insert", json!({"text": "bottom"}));
    at(&mut s, 1, 0);
    run(&mut s, "text.deleteForward", json!({}));
    assert_eq!(grid(&s), vec![vec!["top", ""], vec!["bottom", ""]], "the empty middle row is gone");
    assert_eq!(caret_cell(&s), (1, 0), "the caret lands in the row that took its place");
    // A table with only one row is left alone (nothing to collapse into).
    let mut s2 = sample();
    blank_page(&mut s2);
    run(&mut s2, "insert.table", json!({"rows": 1, "cols": 2}));
    at(&mut s2, 0, 0);
    run(&mut s2, "text.deleteForward", json!({}));
    assert_eq!(table(&s2).n_rows(), 1, "the only row stays");
    assert!(s2.page().unwrap().objects.iter().any(|o| o.as_table().is_some()), "the table stays");
}

#[test]
fn tab_after_text_turns_the_paragraph_into_a_table() {
    // parity: BEH.Table.TabToCreate
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "outline.new", json!({"x": 60.0, "y": 120.0}));
    run(&mut s, "text.insert", json!({"text": "Before\nFruit\nAfter"}));
    let oid = s.page().unwrap().objects[0].id();
    run(&mut s, "caret.set", json!({"block": oid.to_string(), "para": 1, "offset": 5}));
    let undo = s.undo_labels().len();
    run(&mut s, "para.indent", json!({"key": "tab"}));
    assert_eq!(s.undo_labels().len(), undo + 1, "one undo step");
    assert_eq!(grid(&s), vec![vec!["Fruit".to_string(), String::new()]], "KB.Table.CreateWithTab");
    assert_eq!(caret_cell(&s), (0, 1), "the caret goes to the second cell");
    // The table takes the paragraph's place: before it the container, after it a new one.
    let kinds: Vec<_> = s
        .page()
        .unwrap()
        .objects
        .iter()
        .map(|o| match o {
            PageObject::Outline(ol) => ol.paragraphs.iter().map(|p| p.text()).collect::<Vec<_>>().join("|"),
            PageObject::Table(_) => "table".into(),
            _ => "other".into(),
        })
        .collect();
    assert_eq!(kinds, ["Before", "table", "After"]);
    let ys: Vec<f32> = s.page().unwrap().objects.iter().map(|o| o.pos().y).collect();
    assert!(ys[0] < ys[1] && ys[1] < ys[2], "stacked top to bottom: {ys:?}");
    // Then the table's own keys: Tab in the last cell of a one-row table adds a column.
    run(&mut s, "text.insert", json!({"text": "Qty"}));
    run(&mut s, "table.nextCell", json!({}));
    assert_eq!(grid(&s)[0].len(), 3, "KB.Table.NewColumnTab");
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.page().unwrap().objects.len(), 1, "undo puts the paragraph back");
    // A lone paragraph: the container goes, the table sits where it was.
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "outline.new", json!({"x": 60.0, "y": 120.0}));
    run(&mut s, "text.insert", json!({"text": "Only"}));
    run(&mut s, "para.indent", json!({"key": "tab"}));
    let objects = &s.page().unwrap().objects;
    assert!(objects.len() == 1 && objects[0].as_table().is_some_and(|t| t.pos.y == 120.0));
}

#[test]
fn tab_elsewhere_keeps_its_other_meanings() {
    let mut s = sample();
    blank_page(&mut s);
    run(&mut s, "outline.new", json!({"x": 60.0, "y": 120.0}));
    run(&mut s, "text.insert", json!({"text": "Inside"}));
    let oid = s.page().unwrap().objects[0].id();
    // In the middle of the text: a tab character.
    run(&mut s, "caret.set", json!({"block": oid.to_string(), "para": 0, "offset": 2}));
    run(&mut s, "para.indent", json!({"key": "tab"}));
    assert_eq!(s.page().unwrap().outline(oid).unwrap().paragraphs[0].text(), "In\tside");
    // A list item at its end: indented, not a table.
    run(&mut s, "para.bullets", json!({}));
    let len = s.page().unwrap().outline(oid).unwrap().paragraphs[0].len();
    run(&mut s, "caret.set", json!({"block": oid.to_string(), "para": 0, "offset": len}));
    run(&mut s, "para.indent", json!({"key": "tab"}));
    assert!(s.page().unwrap().objects.iter().all(|o| o.as_table().is_none()));
    // A selection, not a caret: no table either.
    run(&mut s, "caret.set", json!({"block": oid.to_string(), "para": 0, "offset": 0}));
    run(&mut s, "caret.set", json!({"block": oid.to_string(), "para": 0, "offset": len, "extend": true}));
    run(&mut s, "para.indent", json!({"key": "tab"}));
    assert!(s.page().unwrap().objects.iter().all(|o| o.as_table().is_none()));
}
