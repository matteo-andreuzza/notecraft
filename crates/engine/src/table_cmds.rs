//! Tables: Insert › Table, the Table Layout contextual tab (select, delete, insert, borders,
//! shading, sort), the keys that move through cells and grow a table, and conversion between
//! tables and text.
//!
//! Where a table is "the current table" ([`table_sel`]): the caret is in one of its cells, its
//! cells are selected, or it is selected as an object. Every editing command here records one
//! undo step through `Session::run`; the selection commands are pure. Sizes are bounded by
//! `notecraft_doc::table::{MAX_ROWS, MAX_COLS, MAX_CELLS}` against hostile params.

use notecraft_doc::color::Rgba;
use notecraft_doc::geom::Point;
use notecraft_doc::table::{DEFAULT_COL_WIDTH, MAX_CELLS, MAX_COL_WIDTH, MAX_COLS, MAX_ROWS, MIN_COL_WIDTH};
use notecraft_doc::{CharFormat, Id, Outline, PageObject, Paragraph, Run, Table, TableCell};
use notecraft_text as t;
use serde_json::{Value, json};

use crate::ops::{self, need_page, no_page};
use crate::selection::{Block, PageSel, Pos};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// The table the selection is in, with the selected rows and columns (inclusive ranges).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TableSel {
    /// The table.
    pub table: Id,
    /// First and last row.
    pub rows: (usize, usize),
    /// First and last column.
    pub cols: (usize, usize),
}

/// The current table and selected cells (see the module docs).
pub(crate) fn table_sel(s: &Session) -> Option<TableSel> {
    let page = s.page()?;
    match &s.sel.on_page {
        PageSel::Text(r) => match r.head.block {
            Block::Cell(cell) => {
                let (t, row, col) = page.table_of_cell(cell)?;
                let (rows, cols) = with_spans(t, (row, row), (col, col));
                Some(TableSel { table: t.id, rows, cols })
            }
            _ => None,
        },
        PageSel::Cells { table, rows, cols } => {
            let t = page.table(*table)?;
            let (lr, lc) = (t.n_rows().checked_sub(1)?, t.n_cols().checked_sub(1)?);
            let r0 = rows.0.min(rows.1).min(lr);
            let c0 = cols.0.min(cols.1).min(lc);
            let (rows, cols) = with_spans(t, (r0, rows.0.max(rows.1).min(lr)), (c0, cols.0.max(cols.1).min(lc)));
            Some(TableSel { table: *table, rows, cols })
        }
        PageSel::Objects { ids } => {
            let t = ids.iter().find_map(|id| page.table(*id))?;
            Some(TableSel { table: t.id, rows: (0, t.n_rows().checked_sub(1)?), cols: (0, t.n_cols().checked_sub(1)?) })
        }
        PageSel::None => None,
    }
}

/// Grow the cell range `rows` × `cols` until it holds every merged cell it touches (decision D9).
fn with_spans(t: &Table, mut rows: (usize, usize), mut cols: (usize, usize)) -> ((usize, usize), (usize, usize)) {
    // The merged cells as (first row, last row, first column, last column), found once: the
    // loop below may pass over them several times, and runs in enabled predicates every frame.
    let merged: Vec<(usize, usize, usize, usize)> = t
        .rows
        .iter()
        .enumerate()
        .flat_map(|(r, row)| {
            row.cells.iter().enumerate().filter(|(_, c)| c.is_merged()).map(move |(c, cell)| {
                let (rs, cs) = cell.span();
                (r, r + rs - 1, c, c + cs - 1)
            })
        })
        .collect();
    loop {
        let mut grew = false;
        for &(r, r1, c, c1) in &merged {
            if r <= rows.1 && r1 >= rows.0 && c <= cols.1 && c1 >= cols.0 {
                let (nr, nc) = ((rows.0.min(r), rows.1.max(r1)), (cols.0.min(c), cols.1.max(c1)));
                if (nr, nc) != (rows, cols) {
                    (rows, cols) = (nr, nc);
                    grew = true;
                }
            }
        }
        if !grew {
            return (rows, cols);
        }
    }
}

/// Merge Cells: enabled on two or more selected cells.
fn need_cells(s: &Session) -> Option<&'static str> {
    if let Some(reason) = in_table(s) {
        return Some(reason);
    }
    match table_sel(s) {
        Some(ts) if ts.rows.0 == ts.rows.1 && ts.cols.0 == ts.cols.1 => Some("Select two or more cells to merge"),
        _ => None,
    }
}

/// Split Cells: enabled when the selection holds a merged cell.
fn need_merged(s: &Session) -> Option<&'static str> {
    if let Some(reason) = in_table(s) {
        return Some(reason);
    }
    let merged = table_sel(s).zip(s.page()).and_then(|(ts, p)| {
        let t = p.table(ts.table)?;
        Some((ts.rows.0..=ts.rows.1).any(|r| (ts.cols.0..=ts.cols.1).any(|c| t.cell(r, c).is_some_and(TableCell::is_merged))))
    });
    if merged == Some(true) { None } else { Some("Select a merged cell to split") }
}

/// Enabled when the selection is in a table.
pub(crate) fn in_table(s: &Session) -> Option<&'static str> {
    if s.sel.page.is_none() {
        return Some("No page is open");
    }
    if table_sel(s).is_some() { None } else { Some("Click in a table first") }
}

fn need_caret_in_cell(s: &Session) -> Option<&'static str> {
    if s.sel.page.is_none() {
        return Some("No page is open");
    }
    match s.text_range() {
        Some(r) if matches!(r.head.block, Block::Cell(_)) => None,
        _ => Some("Click in a table cell first"),
    }
}

fn need_text_selection(s: &Session) -> Option<&'static str> {
    if s.sel.page.is_none() {
        return Some("No page is open");
    }
    match (&s.sel.on_page, s.text_range()) {
        (PageSel::Text(_), Some(r)) if matches!(r.head.block, Block::Outline(_)) => None,
        (PageSel::Objects { ids }, _) if ids.len() == 1 && s.page().is_some_and(|p| p.outline(ids[0]).is_some()) => None,
        _ => Some("Select paragraphs in a note container first"),
    }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // R.Insert.Tables.TableInsertGallery — the size grid under Insert › Table.
        CommandSpec::new("insert.table", "R.Insert.Tables.TableInsertGallery", "Table", "Insert/Tables/Table", insert_table)
            .params(r#"{"rows": int (1..5000, default 2), "cols": int (1..64, default 2), "x"?: pt, "y"?: pt}"#)
            .when(need_page),
        // R.Insert.Tables.TableInsert — Insert Table…, the dialog version (same insert, just
        // chosen from a dialog's row/column fields instead of the size grid).
        CommandSpec::new("insert.tableDialog", "R.Insert.Tables.TableInsert", "Insert Table", "Insert/Tables/Table/Insert Table", insert_table)
            .params(r#"{"rows": int (1..5000, default 2), "cols": int (1..64, default 2)}"#)
            .when(need_page),
        // NC.Table.ConvertTextToTable — the selected paragraphs become rows, tabs split cells.
        CommandSpec::new(
            "insert.convertTextToTable",
            "NC.Table.ConvertTextToTable",
            "Convert Text to Table",
            "Insert/Tables/Table/Convert Text to Table",
            convert_text_to_table,
        )
        .when(need_text_selection),
        // CT.TableLayout.Select.*
        CommandSpec::new(
            "table.selectTable",
            "CT.TableLayout.Select.SelectTableOneNote",
            "Select Table",
            "Table Layout/Select/Select Table",
            |s, _| select(s, true, true),
        )
        .pure()
        .when(in_table),
        CommandSpec::new(
            "table.selectColumns",
            "CT.TableLayout.Select.SelectColumns",
            "Select Columns",
            "Table Layout/Select/Select Columns",
            |s, _| select(s, true, false),
        )
        .pure()
        .when(in_table),
        CommandSpec::new("table.selectRows", "CT.TableLayout.Select.SelectRows", "Select Rows", "Table Layout/Select/Select Rows", |s, _| {
            select(s, false, true)
        })
        .pure()
        .when(in_table),
        CommandSpec::new("table.selectCell", "CT.TableLayout.Select.SelectCell", "Select Cell", "Table Layout/Select/Select Cell", |s, _| {
            select(s, false, false)
        })
        .pure()
        .when(in_table),
        // NC.Table.SelectRange — a drag across cells.
        CommandSpec::new("table.selectRange", "NC.Table.SelectRange", "Select Cells", "Mouse/Drag across table cells", select_range)
            .params(r#"{"table": table id, "rows": [first, last], "cols": [first, last]}"#)
            .pure()
            .when(need_page),
        // CT.TableLayout.Delete.*
        CommandSpec::new(
            "table.delete",
            "CT.TableLayout.Delete.DeleteTableOneNote",
            "Delete Table",
            "Table Layout/Delete/Delete Table",
            delete_table,
        )
        .when(in_table),
        CommandSpec::new(
            "table.deleteColumns",
            "CT.TableLayout.Delete.DeleteColumnsOneNote",
            "Delete Columns",
            "Table Layout/Delete/Delete Columns",
            |s, _| delete_lines(s, false),
        )
        .when(in_table),
        CommandSpec::new("table.deleteRows", "CT.TableLayout.Delete.DeleteRowsOneNote", "Delete Rows", "Table Layout/Delete/Delete Rows", |s, _| {
            delete_lines(s, true)
        })
        .when(in_table),
        // CT.TableLayout.Insert.*
        CommandSpec::new(
            "table.insertAbove",
            "CT.TableLayout.Insert.TableInsertRowsAbove",
            "Insert Above",
            "Table Layout/Insert/Insert Above",
            |s, _| insert_row(s, false),
        )
        .when(in_table),
        CommandSpec::new(
            "table.insertBelow",
            "CT.TableLayout.Insert.TableInsertRowsBelow",
            "Insert Below",
            "Table Layout/Insert/Insert Below",
            |s, _| insert_row(s, true),
        )
        .key("Mod+Enter")
        .when(in_table),
        CommandSpec::new(
            "table.insertLeft",
            "CT.TableLayout.Insert.TableInsertColumnsLeft",
            "Insert Left",
            "Table Layout/Insert/Insert Left",
            |s, _| insert_col(s, false),
        )
        .key("Mod+Alt+E")
        .when(in_table),
        CommandSpec::new(
            "table.insertRight",
            "CT.TableLayout.Insert.TableInsertColumnsRight",
            "Insert Right",
            "Table Layout/Insert/Insert Right",
            |s, _| insert_col(s, true),
        )
        .key("Mod+Alt+R")
        .when(in_table),
        // CT.TableLayout.Format.*
        CommandSpec::new(
            "table.hideBorders",
            "CT.TableLayout.Format.TableBordersHide",
            "Hide Borders",
            "Table Layout/Format/Hide Borders",
            hide_borders,
        )
        .when(in_table),
        CommandSpec::new("table.shading", "CT.TableLayout.Format.ShadingColorPicker", "Shading", "Table Layout/Format/Shading", shading)
            .params(r##"{"color": "#RRGGBB" | null (no colour)}"##)
            .when(in_table),
        // CT.TableLayout.Data.*
        CommandSpec::new("table.sortAscending", "CT.TableLayout.Data.SortUp", "Sort Ascending", "Table Layout/Data/Sort/Sort Ascending", |s, _| {
            sort(s, false)
        })
        .when(in_table),
        CommandSpec::new(
            "table.sortDescending",
            "CT.TableLayout.Data.SortDown",
            "Sort Descending",
            "Table Layout/Data/Sort/Sort Descending",
            |s, _| sort(s, true),
        )
        .when(in_table),
        CommandSpec::new("table.merge", "CT.TableLayout.Merge.MergeCells", "Merge Cells", "Table Layout/Merge/Merge Cells", merge_cells)
            .when(need_cells),
        CommandSpec::new("table.split", "CT.TableLayout.Merge.UnmergeCells", "Split Cells", "Table Layout/Merge/Split Cells", split_cells)
            .when(need_merged),
        CommandSpec::new("table.sortDialog", "CT.TableLayout.Data.ShowSortTableRowsDialog", "Sort…", "Table Layout/Data/Sort/Sort…", sort_by_keys)
            .params(r#"{"keys"?: [{"col": int, "descending"?: bool}, …] 1–3 columns in turn (default: the caret's, ascending), "headerRow"?: bool}"#)
            .when(in_table),
        CommandSpec::new(
            "table.headerRow",
            "CT.TableLayout.Data.HeaderRowNoFormatting",
            "Header Row (excluded from sort)",
            "Table Layout/Data/Sort/Header Row (excluded from sort)",
            header_row,
        )
        .when(in_table),
        // NC.Table.ConvertToText — rows become paragraphs, cells are joined by tabs.
        CommandSpec::new("table.convertToText", "NC.Table.ConvertToText", "Convert to Text", "Table Layout/Convert/Convert to Text", convert_to_text)
            .when(in_table),
        // NC.Table.ColumnWidth — dragging a column border.
        CommandSpec::new("table.columnWidth", "NC.Table.ColumnWidth", "Column Width", "Mouse/Drag a table column border", column_width)
            .params(r#"{"table": table id, "col": int, "width": pt}"#)
            .when(need_page),
        // NC.Table.NextCell / PrevCell — Tab and Shift+Tab in a cell (KB.Table.TabInLastCell,
        // KB.Table.NewColumnTab).
        CommandSpec::new("table.nextCell", "NC.Table.NextCell", "Next Cell", "Keyboard/Tables/Move to the next cell", next_cell)
            .key("Tab")
            .when(need_caret_in_cell),
        CommandSpec::new("table.prevCell", "NC.Table.PrevCell", "Previous Cell", "Keyboard/Tables/Move to the previous cell", prev_cell)
            .key("Shift+Tab")
            .pure()
            .when(need_caret_in_cell),
        // KB.Table.NewRowEnter (with KB.Table.RowAboveEnter) — Enter in a cell.
        CommandSpec::new(
            "table.enter",
            "KB.Table.NewRowEnter",
            "New Row",
            "Keyboard/Tables/Create another row at the end cell: Enter (again to finish)",
            enter,
        )
        .when(need_caret_in_cell),
        // KB.Table.ParagraphInCell — Alt+Enter.
        CommandSpec::new(
            "table.newParagraphInCell",
            "KB.Table.ParagraphInCell",
            "New Paragraph in Cell",
            "Keyboard/Tables/Create another paragraph in the same cell",
            crate::text_cmds::newline,
        )
        .key("Alt+Enter")
        .when(need_caret_in_cell),
    ]
}

fn mint(s: &mut Session) -> impl FnMut() -> Id + '_ {
    move || s.ids.next_id()
}

fn stamp(s: &Session, cell: &mut TableCell) {
    for p in &mut cell.paragraphs {
        p.author = s.author();
        p.modified = s.now();
    }
}

fn caret_in(s: &mut Session, cell: Id, at_end: bool) {
    let (para, offset) = match s.page().and_then(|p| p.cell(cell)) {
        Some(c) if at_end => {
            let last = c.paragraphs.len().saturating_sub(1);
            (last, c.paragraphs.get(last).map(Paragraph::len).unwrap_or(0))
        }
        _ => (0, 0),
    };
    s.set_caret(Pos::new(Block::Cell(cell), para, offset));
}

fn table_mut(s: &mut Session, id: Id) -> Result<&mut Table, CmdError> {
    s.page_mut().ok_or_else(no_page)?.table_mut(id).ok_or_else(|| CmdError::Failed("the table is gone".into()))
}

/// KB.Table.DeleteEmptyRow — Delete at the very start of a wholly empty row removes that row
/// (simplified to one press, not the reference's reported two: the end state is the same and a
/// cross-call key-repeat counter would be a lot of state for a P2 behaviour). Called from
/// `text_cmds::delete_forward` when there is nothing else left for Delete to do.
pub(crate) fn delete_empty_row(s: &mut Session) -> bool {
    let Some(caret) = s.text_range().map(|r| r.head) else { return false };
    if caret.para != 0 || caret.offset != 0 {
        return false;
    }
    let Block::Cell(cell) = caret.block else { return false };
    let Some((t, row, _)) = s.page().and_then(|p| p.table_of_cell(cell)) else { return false };
    if t.n_rows() <= 1 || !t.rows[row].is_empty() {
        return false;
    }
    let table_id = t.id;
    let Some(t) = s.page_mut().and_then(|p| p.table_mut(table_id)) else { return false };
    t.remove_lines(true, row, row);
    let n_rows = t.n_rows();
    let target = t.cell(row.min(n_rows - 1), 0).map(|c| c.id);
    if let Some(c) = target {
        caret_in(s, c, false);
    }
    true
}

fn cur(s: &Session) -> Result<TableSel, CmdError> {
    table_sel(s).ok_or_else(|| CmdError::Disabled("click in a table first".into()))
}

pub(crate) fn insert_table(s: &mut Session, v: &Value) -> CmdResult {
    let rows = p::usize(v, "rows").unwrap_or(2);
    let cols = p::usize(v, "cols").unwrap_or(2);
    if rows == 0 || cols == 0 || rows > MAX_ROWS || cols > MAX_COLS || rows.saturating_mul(cols) > MAX_CELLS {
        return Err(CmdError::Params(format!("a table has 1–{MAX_ROWS} rows, 1–{MAX_COLS} columns and at most {MAX_CELLS} cells")));
    }
    let page = s.page().ok_or_else(no_page)?;
    // Below the caret's container when there is one (unverified: the reference puts the table
    // inside that container, after the caret paragraph; see docs/plan.md §4.8).
    let pos = ops::insert_pos(s, page, v);
    s.pending_outline = None;
    let id = s.new_id();
    let mut table = Table::new(id, pos, rows, cols, &mut mint(s));
    for row in &mut table.rows {
        for c in &mut row.cells {
            stamp(s, c);
        }
    }
    let first = table.rows.first().and_then(|r| r.cells.first()).map(|c| c.id);
    s.page_mut().ok_or_else(no_page)?.objects.push(PageObject::Table(table));
    if let Some(c) = first {
        caret_in(s, c, false);
    }
    Ok(json!({"table": id.to_string()}))
}

fn select(s: &mut Session, all_rows: bool, all_cols: bool) -> CmdResult {
    let ts = cur(s)?;
    let t = s.page().and_then(|p| p.table(ts.table)).ok_or_else(no_page)?;
    let rows = if all_rows { (0, t.n_rows().saturating_sub(1)) } else { ts.rows };
    let cols = if all_cols { (0, t.n_cols().saturating_sub(1)) } else { ts.cols };
    s.sel.on_page = PageSel::Cells { table: ts.table, rows, cols };
    s.pending_fmt = None;
    Ok(Value::Null)
}

fn pair(v: &Value, k: &str) -> Result<(usize, usize), CmdError> {
    let a = v.get(k).and_then(Value::as_array).ok_or_else(|| CmdError::Params(format!("`{k}` must be [first, last]")))?;
    let n = |i: usize| a.get(i).and_then(Value::as_u64).map(|x| x.min(usize::MAX as u64) as usize);
    match (n(0), n(1)) {
        (Some(x), Some(y)) => Ok((x.min(y), x.max(y))),
        _ => Err(CmdError::Params(format!("`{k}` must be [first, last]"))),
    }
}

fn select_range(s: &mut Session, v: &Value) -> CmdResult {
    let table = p::req_id(v, "table")?;
    let (rows, cols) = (pair(v, "rows")?, pair(v, "cols")?);
    let t = s.page().ok_or_else(no_page)?.table(table).ok_or_else(|| CmdError::Params("no such table on this page".into()))?;
    if rows.1 >= t.n_rows() || cols.1 >= t.n_cols() {
        return Err(CmdError::Params("the range is outside the table".into()));
    }
    s.sel.on_page = PageSel::Cells { table, rows, cols };
    s.pending_fmt = None;
    Ok(Value::Null)
}

fn delete_table(s: &mut Session, _: &Value) -> CmdResult {
    let ts = cur(s)?;
    let page = s.page_mut().ok_or_else(no_page)?;
    page.objects.retain(|o| o.id() != ts.table);
    s.sel.on_page = PageSel::None;
    Ok(Value::Null)
}

fn delete_lines(s: &mut Session, rows: bool) -> CmdResult {
    let ts = cur(s)?;
    let t = table_mut(s, ts.table)?;
    let (a, b) = if rows { ts.rows } else { ts.cols };
    let total = if rows { t.n_rows() } else { t.n_cols() };
    if a == 0 && b + 1 >= total {
        // Every row (or column): the table goes.
        return delete_table(s, &Value::Null);
    }
    // Merged cells reaching past the range keep their text (decision D9).
    t.remove_lines(rows, a, b);
    let (row, col) =
        if rows { (a.min(t.n_rows() - 1), ts.cols.0.min(t.n_cols() - 1)) } else { (ts.rows.0.min(t.n_rows() - 1), a.min(t.n_cols() - 1)) };
    let target = t.cell(row, col).map(|c| c.id);
    if let Some(c) = target {
        caret_in(s, c, false);
    }
    Ok(Value::Null)
}

fn room(rows: usize, cols: usize) -> Result<(), CmdError> {
    if rows > MAX_ROWS || cols > MAX_COLS || rows.saturating_mul(cols) > MAX_CELLS {
        return Err(CmdError::Failed(format!("a table holds at most {MAX_ROWS} rows, {MAX_COLS} columns and {MAX_CELLS} cells")));
    }
    Ok(())
}

fn insert_row(s: &mut Session, below: bool) -> CmdResult {
    let ts = cur(s)?;
    let (n_rows, n_cols) = s.page().and_then(|p| p.table(ts.table)).map(|t| (t.n_rows(), t.n_cols())).ok_or_else(no_page)?;
    let mut row = Table::new_row(n_cols, &mut mint(s));
    for c in &mut row.cells {
        stamp(s, c);
    }
    let at = if below { ts.rows.1 + 1 } else { ts.rows.0 };
    let target = row.cells.get(ts.cols.0).map(|c| c.id);
    room(n_rows + 1, n_cols)?;
    let t = table_mut(s, ts.table)?;
    let at = at.min(t.rows.len());
    t.rows.insert(at, row);
    t.grow_spans(true, at, 1);
    // The caret goes to the new row, same column (unverified).
    if let Some(c) = target {
        caret_in(s, c, false);
    }
    Ok(Value::Null)
}

fn insert_col(s: &mut Session, right: bool) -> CmdResult {
    let ts = cur(s)?;
    let (n_rows, n_cols) = s.page().and_then(|p| p.table(ts.table)).map(|t| (t.n_rows(), t.n_cols())).ok_or_else(no_page)?;
    let mut cells: Vec<TableCell> = Vec::with_capacity(n_rows);
    for _ in 0..n_rows {
        let (a, b) = (s.new_id(), s.new_id());
        let mut c = TableCell::new(a, b);
        stamp(s, &mut c);
        cells.push(c);
    }
    let at = if right { ts.cols.1 + 1 } else { ts.cols.0 };
    let target = cells.get(ts.rows.0).map(|c| c.id);
    room(n_rows, n_cols + 1)?;
    let t = table_mut(s, ts.table)?;
    let at = at.min(t.columns.len());
    t.columns.insert(at, DEFAULT_COL_WIDTH);
    for (r, c) in t.rows.iter_mut().zip(cells) {
        let k = at.min(r.cells.len());
        r.cells.insert(k, c);
    }
    t.grow_spans(false, at, 1);
    if let Some(c) = target {
        caret_in(s, c, false);
    }
    Ok(Value::Null)
}

fn hide_borders(s: &mut Session, _: &Value) -> CmdResult {
    let ts = cur(s)?;
    let t = table_mut(s, ts.table)?;
    t.borders = !t.borders;
    Ok(json!({"borders": t.borders}))
}

fn shading(s: &mut Session, v: &Value) -> CmdResult {
    let color = match v.get("color") {
        None | Some(Value::Null) => None,
        Some(Value::String(h)) => Some(Rgba::from_hex(h).ok_or_else(|| CmdError::Params("`color` must be #RRGGBB or null".into()))?),
        Some(_) => return Err(CmdError::Params("`color` must be #RRGGBB or null".into())),
    };
    let ts = cur(s)?;
    let t = table_mut(s, ts.table)?;
    for r in t.rows.iter_mut().skip(ts.rows.0).take(ts.rows.1 - ts.rows.0 + 1) {
        for c in r.cells.iter_mut().skip(ts.cols.0).take(ts.cols.1 - ts.cols.0 + 1) {
            c.shading = color;
        }
    }
    Ok(Value::Null)
}

/// Sort key: numbers before text and compared as numbers, text case-insensitively, empty
/// cells last (unverified: the reference's collation).
fn sort_key(text: &str) -> (u8, f64, String) {
    let tr = text.trim();
    if tr.is_empty() {
        return (2, 0.0, String::new());
    }
    match tr.replace(',', "").parse::<f64>() {
        Ok(n) if n.is_finite() => (0, n, String::new()),
        _ => (1, 0.0, tr.to_lowercase()),
    }
}

/// Rows can't be reordered while a merged cell spans several of them (as in other editors).
fn sortable(s: &Session, table: Id) -> Result<(), CmdError> {
    if s.page().and_then(|p| p.table(table)).is_some_and(Table::has_row_spans) {
        return Err(CmdError::Failed("This table has cells merged across rows; split them to sort".into()));
    }
    Ok(())
}

fn sort(s: &mut Session, descending: bool) -> CmdResult {
    let ts = cur(s)?;
    sortable(s, ts.table)?;
    sort_rows(table_mut(s, ts.table)?, &[(ts.cols.0, descending)]);
    Ok(Value::Null)
}

/// Sort… (`table.sortDialog`): the body rows by up to three columns in turn; `headerRow` first
/// sets whether the first row is a header (it stays on top).
fn sort_by_keys(s: &mut Session, v: &Value) -> CmdResult {
    let ts = cur(s)?;
    sortable(s, ts.table)?;
    let keys: Vec<(usize, bool)> = match v.get("keys") {
        None => vec![(ts.cols.0, false)],
        Some(Value::Array(a)) if (1..=3).contains(&a.len()) => {
            a.iter().map(|k| Ok((p::req_usize(k, "col")?, p::bool(k, "descending").unwrap_or(false)))).collect::<Result<_, CmdError>>()?
        }
        Some(_) => return Err(CmdError::Params(r#"keys: 1 to 3 of {"col", "descending"?}"#.into())),
    };
    // Checked before `table_mut`, which would record an undo step.
    let cols = s.page().and_then(|p| p.table(ts.table)).map(Table::n_cols).unwrap_or(0);
    if let Some((c, _)) = keys.iter().find(|(c, _)| *c >= cols) {
        return Err(CmdError::Params(format!("no column {c}: the table has {cols}")));
    }
    let header = p::bool(v, "headerRow");
    let t = table_mut(s, ts.table)?;
    if let Some(h) = header {
        t.header_row = h;
    }
    sort_rows(t, &keys);
    Ok(json!({"headerRow": t.header_row}))
}

/// Sort the rows below the header row by `keys` (column, descending) in turn; a stable sort, so
/// rows that tie keep their order.
fn sort_rows(t: &mut Table, keys: &[(usize, bool)]) {
    let skip = usize::from(t.header_row).min(t.rows.len());
    let Some(body) = t.rows.get_mut(skip..) else { return };
    body.sort_by(|a, b| {
        keys.iter().fold(std::cmp::Ordering::Equal, |ord, &(col, descending)| {
            ord.then_with(|| {
                let ka = sort_key(&a.cells.get(col).map(TableCell::text).unwrap_or_default());
                let kb = sort_key(&b.cells.get(col).map(TableCell::text).unwrap_or_default());
                let o = ka.0.cmp(&kb.0).then(ka.1.total_cmp(&kb.1)).then_with(|| ka.2.cmp(&kb.2));
                // Empty cells stay last either way.
                if descending && ka.0 != 2 && kb.0 != 2 { o.reverse() } else { o }
            })
        })
    });
}

fn header_row(s: &mut Session, _: &Value) -> CmdResult {
    let ts = cur(s)?;
    let t = table_mut(s, ts.table)?;
    t.header_row = !t.header_row;
    Ok(json!({"headerRow": t.header_row}))
}

fn column_width(s: &mut Session, v: &Value) -> CmdResult {
    let table = p::req_id(v, "table")?;
    let col = p::req_usize(v, "col")?;
    let width = p::req_f32(v, "width")?.clamp(MIN_COL_WIDTH, MAX_COL_WIDTH);
    let t = s.page_mut().ok_or_else(no_page)?.table_mut(table).ok_or_else(|| CmdError::Params("no such table on this page".into()))?;
    let w = t.columns.get_mut(col).ok_or_else(|| CmdError::Params("no such column".into()))?;
    *w = width;
    s.coalesce = Some(format!("colw:{table}:{col}"));
    Ok(Value::Null)
}

/// The cells of a rectangle of `t` as paragraph lists with its merged cells (clipped to the
/// rectangle), for the HTML clipboard.
pub(crate) fn cells_of(t: &Table, rows: (usize, usize), cols: (usize, usize)) -> notecraft_import_export::html::TableBlock {
    notecraft_import_export::html::TableBlock::from_table(t, rows, cols)
}

/// A new table at `pos` holding `block` (rows of cells of paragraphs and merged cells, from
/// pasted HTML): ragged rows are padded, the table limits applied, every id fresh.
pub(crate) fn table_from_cells(s: &mut Session, pos: Point, block: notecraft_import_export::html::TableBlock) -> Table {
    let cells = block.cells;
    let cols = cells.iter().map(Vec::len).max().unwrap_or(1).clamp(1, MAX_COLS);
    let rows = cells.len().clamp(1, MAX_ROWS).min(MAX_CELLS / cols);
    let id = s.new_id();
    let mut table = Table::new(id, pos, rows, cols, &mut mint(s));
    for (r, row) in cells.into_iter().take(rows).enumerate() {
        for (c, paras) in row.into_iter().take(cols).enumerate() {
            if paras.is_empty() {
                continue;
            }
            let fresh: Vec<Paragraph> = paras
                .into_iter()
                .map(|mut p| {
                    p.id = s.new_id();
                    p
                })
                .collect();
            if let Some(cell) = table.rows.get_mut(r).and_then(|row| row.cells.get_mut(c)) {
                cell.paragraphs = fresh;
            }
        }
    }
    for row in &mut table.rows {
        for c in &mut row.cells {
            stamp(s, c);
        }
    }
    for sp in block.spans {
        if let Some(cell) = table.cell_mut(sp.row, sp.col) {
            cell.row_span = u32::try_from(sp.rows).unwrap_or(u32::MAX);
            cell.col_span = u32::try_from(sp.cols).unwrap_or(u32::MAX);
        }
    }
    // Spans past the clamped size, or overlapping, are repaired here (decision D9).
    table.normalize();
    table
}

/// Give a copied table fresh ids throughout (pasting it twice must not duplicate ids).
pub(crate) fn refresh_ids(s: &mut Session, t: &mut Table) {
    t.id = s.new_id();
    for row in &mut t.rows {
        row.id = s.new_id();
        for c in &mut row.cells {
            c.id = s.new_id();
            for p in &mut c.paragraphs {
                p.id = s.new_id();
            }
        }
    }
}

/// Put the caret at the start of a table's first cell.
/// Tab after text (BEH.Table.TabToCreate, KB.Table.CreateWithTab; decision D6, `docs/plan.md`
/// §4.8): with the caret at the end of a paragraph's text in a container, the paragraph becomes
/// a one-row, two-column table, its text in the first cell and the caret in the second. The
/// table takes the paragraph's place: the container keeps the paragraphs before it, those after
/// it move to a new container below the table, and a container left empty goes. No "creating a
/// table" state is kept: further Tabs add columns and Enter adds rows through the table's own
/// keys (KB.Table.NewColumnTab, KB.Table.NewRowEnter). List items, empty paragraphs and a caret
/// inside the text are left to Tab's other meanings: returns `false` then.
pub(crate) fn tab_to_table(s: &mut Session) -> Result<bool, CmdError> {
    // OPT.Advanced.CreateTableOnTab.
    if !s.prefs.create_table_on_tab {
        return Ok(false);
    }
    let Some(c) = s.text_range().filter(|r| r.is_caret()).map(|r| r.head) else { return Ok(false) };
    let Block::Outline(oid) = c.block else { return Ok(false) };
    let page = s.page().ok_or_else(no_page)?;
    let Some(ol) = page.outline(oid) else { return Ok(false) };
    let Some(para) = ol.paragraphs.get(c.para) else { return Ok(false) };
    if c.offset == 0 || c.offset != para.len() || para.list.is_some() || para.text().trim().is_empty() {
        return Ok(false);
    }
    let room = notecraft_doc::media::MAX_PAGE_OBJECTS.saturating_sub(page.objects.len());
    let mut before = ol.paragraphs.clone();
    let after = before.split_off(c.para + 1);
    if room < 1 + usize::from(!after.is_empty()) {
        return Err(CmdError::Failed(format!("a page holds at most {} objects", notecraft_doc::media::MAX_PAGE_OBJECTS)));
    }
    let Some(mut first) = before.pop() else { return Ok(false) };
    first.level = 0;
    let (origin, width) = (ol.pos, ol.width);
    let id = s.new_id();
    let mut table = Table::new(id, origin, 1, 2, &mut mint(s));
    for cell in table.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
        stamp(s, cell);
    }
    let second = table.cell(0, 1).map(|x| x.id);
    if let Some(cell) = table.rows.first_mut().and_then(|r| r.cells.first_mut()) {
        cell.paragraphs = vec![first];
    }
    let tail = if after.is_empty() { None } else { Some(s.new_id()) };
    let page = s.page_mut().ok_or_else(no_page)?;
    let Some(at) = page.objects.iter().position(|o| o.id() == oid) else { return Ok(false) };
    let kept = !before.is_empty();
    if !kept {
        page.objects.remove(at);
    } else if let Some(ol) = page.outline_mut(oid) {
        ol.paragraphs = before;
    }
    if let Some(o) = page.objects.get(at).filter(|_| kept) {
        table.pos = Point::new(origin.x, ops::object_bottom(o) + 12.0);
    }
    let at = if kept { at + 1 } else { at };
    let below = Point::new(origin.x, ops::object_bottom(&PageObject::Table(table.clone())) + 12.0);
    page.objects.insert(at.min(page.objects.len()), PageObject::Table(table));
    if let Some(tid) = tail {
        let mut rest = Outline::new(tid, below, Id::NIL);
        rest.paragraphs = after;
        rest.width = width;
        rest.normalize();
        page.objects.insert((at + 1).min(page.objects.len()), PageObject::Outline(rest));
    }
    s.pending_outline = None;
    if let Some(cell) = second {
        caret_in(s, cell, false);
    }
    Ok(true)
}

pub(crate) fn caret_in_first_cell(s: &mut Session, t: Id) {
    if let Some(c) = s.page().and_then(|p| p.table(t)).and_then(|t| t.rows.first()).and_then(|r| r.cells.first()).map(|c| c.id) {
        caret_in(s, c, false);
    }
}

/// Rows `rows` of `t` as paragraphs: cells joined by tabs, a cell's paragraphs by spaces, the
/// runs (and so the formatting) kept. Used by Convert to Text and by Copy of cells.
pub(crate) fn rows_as_paragraphs(t: &Table, rows: (usize, usize), cols: (usize, usize), mint: &mut dyn FnMut() -> Id) -> Vec<Paragraph> {
    let mut out = Vec::new();
    for r in t.rows.iter().skip(rows.0).take(rows.1.saturating_sub(rows.0) + 1) {
        let mut p = Paragraph::new(mint());
        for (k, c) in r.cells.iter().skip(cols.0).take(cols.1.saturating_sub(cols.0) + 1).enumerate() {
            if k > 0 {
                p.runs.push(Run::new("\t", CharFormat::default()));
            }
            for (j, cp) in c.paragraphs.iter().enumerate() {
                if j > 0 {
                    p.runs.push(Run::new(" ", CharFormat::default()));
                }
                p.runs.extend(cp.runs.iter().cloned());
            }
        }
        p.normalize();
        out.push(p);
    }
    out
}

fn convert_to_text(s: &mut Session, _: &Value) -> CmdResult {
    let ts = cur(s)?;
    let t = s.page().and_then(|p| p.table(ts.table)).cloned().ok_or_else(no_page)?;
    let paras = rows_as_paragraphs(&t, (0, t.n_rows().saturating_sub(1)), (0, t.n_cols().saturating_sub(1)), &mut mint(s));
    let oid = s.new_id();
    let mut o = Outline::new(oid, t.pos, Id::NIL);
    o.paragraphs = paras;
    o.normalize();
    let page = s.page_mut().ok_or_else(no_page)?;
    let idx = page.object_index(ts.table).ok_or_else(|| CmdError::Failed("the table is gone".into()))?;
    if let Some(slot) = page.objects.get_mut(idx) {
        *slot = PageObject::Outline(o);
    }
    s.set_caret(Pos::new(Block::Outline(oid), 0, 0));
    Ok(json!({"outline": oid.to_string()}))
}

/// Split a paragraph's runs at tab characters (each piece keeps its formatting).
fn split_tabs(p: &Paragraph) -> Vec<Vec<Run>> {
    let text = p.text();
    let mut out = Vec::new();
    let mut start = 0;
    for (i, ch) in text.char_indices() {
        if ch == '\t' {
            out.push(t::slice(p, start..i));
            start = i + 1;
        }
    }
    out.push(t::slice(p, start..text.len()));
    out
}

fn convert_text_to_table(s: &mut Session, _: &Value) -> CmdResult {
    let (oid, first, last) = match (&s.sel.on_page, s.text_range()) {
        (PageSel::Text(_), Some(r)) => match r.head.block {
            Block::Outline(id) => {
                let (a, b) = r.ordered();
                (id, a.para, b.para)
            }
            _ => return Err(CmdError::Disabled("select paragraphs in a note container first".into())),
        },
        (PageSel::Objects { ids }, _) => {
            let id = *ids.first().ok_or_else(|| CmdError::Disabled("select a note container first".into()))?;
            let n = s.page().and_then(|p| p.outline(id)).map(|o| o.paragraphs.len()).unwrap_or(0);
            (id, 0, n.saturating_sub(1))
        }
        _ => return Err(CmdError::Disabled("select paragraphs in a note container first".into())),
    };
    let o = s.page().and_then(|p| p.outline(oid)).cloned().ok_or_else(|| CmdError::Failed("note container not found".into()))?;
    let last = last.min(o.paragraphs.len().saturating_sub(1));
    let picked = o.paragraphs.get(first..=last).ok_or_else(|| CmdError::Failed("selection out of range".into()))?;
    let pieces: Vec<Vec<Vec<Run>>> = picked.iter().map(split_tabs).collect();
    let cols = pieces.iter().map(Vec::len).max().unwrap_or(1).clamp(1, MAX_COLS);
    let rows = pieces.len();
    if rows > MAX_ROWS || rows.saturating_mul(cols) > MAX_CELLS {
        return Err(CmdError::Failed(format!("a table holds at most {MAX_ROWS} rows and {MAX_CELLS} cells")));
    }
    // Where the table goes: the container's place when it is converted whole, otherwise below
    // the paragraphs before the selection (estimated: the engine has no layout).
    let before: f32 = o.paragraphs.get(..first).map(|ps| ps.iter().map(|p| 1.0 + p.text().matches('\n').count() as f32).sum()).unwrap_or(0.0);
    let pos = Point::new(o.pos.x, o.pos.y + before * 20.0);
    let tid = s.new_id();
    let mut table = Table::new(tid, pos, rows, cols, &mut mint(s));
    for (row, cells) in table.rows.iter_mut().zip(pieces) {
        for (cell, runs) in row.cells.iter_mut().zip(cells) {
            if let Some(p) = cell.paragraphs.first_mut() {
                p.runs = runs;
                p.normalize();
            }
        }
        for c in &mut row.cells {
            stamp(s, c);
        }
    }
    let tail: Vec<Paragraph> = o.paragraphs.get(last + 1..).map(<[Paragraph]>::to_vec).unwrap_or_default();
    let below = ops::object_bottom(&PageObject::Table(table.clone())) + 12.0;
    let tail_id = if tail.is_empty() || first == 0 { None } else { Some(s.new_id()) };
    let first_cell = table.cell(0, 0).map(|c| c.id);
    let page = s.page_mut().ok_or_else(no_page)?;
    let idx = page.object_index(oid).ok_or_else(|| CmdError::Failed("note container not found".into()))?;
    match (first, tail.is_empty()) {
        (0, true) => {
            if let Some(slot) = page.objects.get_mut(idx) {
                *slot = PageObject::Table(table);
            }
        }
        (0, false) => {
            // The selection was the start: the rest of the container moves below the table.
            if let Some(ol) = page.outline_mut(oid) {
                ol.paragraphs = tail;
                ol.pos = Point::new(ol.pos.x, below);
            }
            page.objects.insert(idx, PageObject::Table(table));
        }
        _ => {
            if let Some(ol) = page.outline_mut(oid) {
                ol.paragraphs.truncate(first);
            }
            page.objects.insert(idx + 1, PageObject::Table(table));
            if let Some(nid) = tail_id {
                let mut rest = Outline::new(nid, Point::new(o.pos.x, below), Id::NIL);
                rest.paragraphs = tail;
                rest.normalize();
                page.objects.insert(idx + 2, PageObject::Outline(rest));
            }
        }
    }
    if let Some(c) = first_cell {
        caret_in(s, c, false);
    }
    Ok(json!({"table": tid.to_string()}))
}

/// The caret's cell: `(table, row, col)`.
fn caret_cell(s: &Session) -> Result<(Id, usize, usize, Pos), CmdError> {
    let c = s.text_range().map(|r| r.head).ok_or_else(|| CmdError::Disabled("click in a table cell first".into()))?;
    let Block::Cell(cell) = c.block else { return Err(CmdError::Disabled("click in a table cell first".into())) };
    let (t, r, col) = s.page().and_then(|p| p.table_of_cell(cell)).ok_or_else(|| CmdError::Failed("the table is gone".into()))?;
    Ok((t.id, r, col, c))
}

/// The next (or, with `back`, previous) cell that shows after `r`, `c` in reading order:
/// merged cells count once (decision D9).
fn shown_after(t: &Table, r: usize, c: usize, back: bool) -> Option<(usize, usize)> {
    let grid = t.span_grid();
    let n_cols = t.n_cols();
    let here = r * n_cols + c;
    let shown = |k: usize| grid.get(k / n_cols).and_then(|row| row.get(k % n_cols)).copied().flatten().is_some();
    let total = t.n_rows() * n_cols;
    let k = if back { (0..here).rev().find(|k| shown(*k))? } else { (here + 1..total).find(|k| shown(*k))? };
    Some((k / n_cols, k % n_cols))
}

fn next_cell(s: &mut Session, _: &Value) -> CmdResult {
    let (tid, r, c, _) = caret_cell(s)?;
    let t = s.page().and_then(|p| p.table(tid)).cloned().ok_or_else(no_page)?;
    let target = match shown_after(&t, r, c, false) {
        Some((nr, nc)) => t.cell(nr, nc).map(|x| x.id),
        // The last cell: a single-row table grows a column (KB.Table.NewColumnTab), a longer one
        // a row (KB.Table.TabInLastCell).
        None if t.n_rows() == 1 => {
            let ts = TableSel { table: tid, rows: (0, 0), cols: (c, c) };
            s.sel.on_page = PageSel::Cells { table: ts.table, rows: ts.rows, cols: ts.cols };
            insert_col(s, true)?;
            return Ok(Value::Null);
        }
        None => {
            let last = t.n_rows() - 1;
            s.sel.on_page = PageSel::Cells { table: tid, rows: (last, last), cols: (0, 0) };
            insert_row(s, true)?;
            return Ok(Value::Null);
        }
    };
    if let Some(cell) = target {
        // The caret lands at the end of the next cell's text (unverified).
        caret_in(s, cell, true);
    }
    Ok(Value::Null)
}

fn prev_cell(s: &mut Session, _: &Value) -> CmdResult {
    let (tid, r, c, _) = caret_cell(s)?;
    let target = s.page().and_then(|p| p.table(tid)).and_then(|t| shown_after(t, r, c, true).and_then(|(pr, pc)| t.cell(pr, pc)).map(|x| x.id));
    if let Some(cell) = target {
        caret_in(s, cell, true);
    }
    Ok(Value::Null)
}

/// Merge Cells (decision D9): the selected cells become one, spanning them all; their text joins
/// in reading order in the first cell (empty cells add nothing) and the others are left empty
/// and hidden. One undo step.
fn merge_cells(s: &mut Session, _: &Value) -> CmdResult {
    let ts = cur(s)?;
    if ts.rows.0 == ts.rows.1 && ts.cols.0 == ts.cols.1 {
        return Err(CmdError::Disabled("select two or more cells to merge".into()));
    }
    let n = (ts.rows.1 - ts.rows.0 + 1) * (ts.cols.1 - ts.cols.0 + 1);
    let fresh: Vec<Id> = (0..n).map(|_| s.new_id()).collect();
    let t = table_mut(s, ts.table)?;
    let mut joined: Vec<Paragraph> = Vec::new();
    let mut k = 0;
    for r in ts.rows.0..=ts.rows.1 {
        for c in ts.cols.0..=ts.cols.1 {
            let Some(cell) = t.cell_mut(r, c) else { continue };
            if !cell.is_empty() {
                joined.append(&mut cell.paragraphs);
            }
            if (r, c) != (ts.rows.0, ts.cols.0) {
                cell.paragraphs = vec![Paragraph::new(fresh.get(k).copied().unwrap_or(cell.id))];
                cell.row_span = 1;
                cell.col_span = 1;
            }
            k += 1;
        }
    }
    let Some(first) = t.cell_mut(ts.rows.0, ts.cols.0) else { return Err(CmdError::Failed("the table is gone".into())) };
    if !joined.is_empty() {
        first.paragraphs = joined;
    }
    first.row_span = (ts.rows.1 - ts.rows.0 + 1) as u32;
    first.col_span = (ts.cols.1 - ts.cols.0 + 1) as u32;
    let id = first.id;
    t.normalize();
    caret_in(s, id, true);
    Ok(json!({"cell": id.to_string()}))
}

/// Split Cells (decision D9): every merged cell in the selection goes back to single cells; its
/// text stays in its first cell.
fn split_cells(s: &mut Session, _: &Value) -> CmdResult {
    let ts = cur(s)?;
    let t = table_mut(s, ts.table)?;
    let mut first = None;
    for r in ts.rows.0..=ts.rows.1 {
        for c in ts.cols.0..=ts.cols.1 {
            if let Some(cell) = t.cell_mut(r, c).filter(|x| x.is_merged()) {
                cell.row_span = 1;
                cell.col_span = 1;
                first.get_or_insert(cell.id);
            }
        }
    }
    let Some(id) = first else { return Err(CmdError::Disabled("select a merged cell to split".into())) };
    caret_in(s, id, true);
    Ok(Value::Null)
}

fn enter(s: &mut Session, _: &Value) -> CmdResult {
    let (tid, r, c, caret) = caret_cell(s)?;
    let t = s.page().and_then(|p| p.table(tid)).cloned().ok_or_else(no_page)?;
    let at_row_start = c == 0 && caret.para == 0 && caret.offset == 0;
    if at_row_start && r > 0 && !t.rows.get(r).is_some_and(|row| row.is_empty()) {
        // KB.Table.RowAboveEnter: a row above; the caret stays in its cell.
        s.sel.on_page = PageSel::Cells { table: tid, rows: (r, r), cols: (0, 0) };
        insert_row(s, false)?;
        s.set_caret(caret);
        return Ok(Value::Null);
    }
    let last_row = r + 1 == t.n_rows();
    if last_row && t.n_rows() > 1 && t.rows.get(r).is_some_and(|row| row.is_empty()) {
        // Enter again on the new, empty last row: the row goes and the caret leaves the table;
        // typing starts a container below it.
        let tm = table_mut(s, tid)?;
        tm.remove_lines(true, r, r);
        let bottom = s.page().and_then(|p| p.objects.iter().find(|o| o.id() == tid).map(ops::object_bottom)).unwrap_or(t.pos.y);
        s.sel.on_page = PageSel::None;
        s.pending_outline = Some(Point::new(t.pos.x, bottom + 12.0));
        return Ok(Value::Null);
    }
    // Anywhere else: a new row below, the caret in its first cell (unverified for cells other
    // than the last one).
    s.sel.on_page = PageSel::Cells { table: tid, rows: (r, r), cols: (0, 0) };
    insert_row(s, true)?;
    Ok(Value::Null)
}

/// The neighbouring cell for caret moves across cell edges: `dr`/`dc` of -1, 0 or 1; left and
/// right wrap to the previous/next row.
pub(crate) fn neighbour_cell(s: &Session, cell: Id, dr: i8, dc: i8) -> Option<Id> {
    let page = s.page()?;
    let (t, r, c) = page.table_of_cell(cell)?;
    let (rs, _) = t.cell(r, c)?.span();
    // Merged cells count once: sideways in reading order, up and down to the cell showing there.
    let (nr, nc) = match (dr, dc) {
        (0, 1) => shown_after(t, r, c, false)?,
        (0, -1) => shown_after(t, r, c, true)?,
        (1, 0) => t.anchor(r.checked_add(rs).filter(|x| *x < t.n_rows())?, c),
        (-1, 0) => t.anchor(r.checked_sub(1)?, c),
        _ => return None,
    };
    t.cell(nr, nc).map(|x| x.id)
}
