//! Tables: a grid of cells, each holding paragraphs, placed on the page like a note container.
//!
//! The structure mirrors [MS-ONE] §2.2 so `.one` tables (M3) map one-to-one:
//! `jcidTableNode` (row and column counts, `TableColumnWidths`, `TableBordersVisible`) →
//! `jcidTableRowNode` → `jcidTableCellNode` (`CellShadingColor`, and outline elements as its
//! children, which NoteCraft stores as a flat list of [`Paragraph`]s with levels, like an
//! [`Outline`](crate::Outline)).
//!
//! **Placement.** A table is its own [`PageObject`](crate::PageObject) with a position (decision
//! in `docs/plan.md` §4.8): in the reference a table sits inside a note container among its
//! paragraphs; NoteCraft keeps it beside them, which leaves every text command untouched. The
//! importer places a contained table where its container's layout puts it.
//!
//! **Merged cells** (decision D9): a cell spans `row_span` × `col_span` grid places from its own;
//! the cells it covers stay in the grid, empty and hidden, so every row still has one cell per
//! column and row/column commands keep working ([MS-ONE] keeps the grid the same way).
//!
//! **Invariants** (restored by [`Table::normalize`]): at least one row and one column, every row
//! has exactly one cell per column, every cell at least one paragraph, the sizes stay within
//! [`MAX_ROWS`], [`MAX_COLS`] and [`MAX_CELLS`] (data from disk and from agents is hostile),
//! every width is finite and within [`MIN_COL_WIDTH`]..=[`MAX_COL_WIDTH`], and spans are at least
//! 1, stay inside the table and never overlap.

use notecraft_color::Rgba;
use notecraft_geom::{Point, length};
use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::text::{MAX_LEVEL, Paragraph};

/// Most rows in a table (a bound against hostile input; unverified reference limit).
pub const MAX_ROWS: usize = 5_000;
/// Most columns in a table (unverified reference limit).
pub const MAX_COLS: usize = 64;
/// Most cells in a table.
pub const MAX_CELLS: usize = 50_000;
/// Width of a new column, in points (unverified: about 1⅓ in).
pub const DEFAULT_COL_WIDTH: f32 = 96.0;
/// Narrowest column, in points.
pub const MIN_COL_WIDTH: f32 = 18.0;
/// Widest column, in points.
pub const MAX_COL_WIDTH: f32 = 2_000.0;

/// One cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    /// Stable id (the caret's block names it).
    pub id: Id,
    /// The cell's paragraphs (flat, with levels; never empty after `normalize`).
    pub paragraphs: Vec<Paragraph>,
    /// Background colour (Table Layout › Shading; [MS-ONE] `CellShadingColor`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shading: Option<Rgba>,
    /// Rows this cell covers, its own included (Merge Cells; 1 for an ordinary cell).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub row_span: u32,
    /// Columns this cell covers, its own included.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub col_span: u32,
}

fn one() -> u32 {
    1
}

fn is_one(n: &u32) -> bool {
    *n == 1
}

impl TableCell {
    /// An empty cell with one empty paragraph.
    pub fn new(id: Id, paragraph: Id) -> TableCell {
        TableCell { id, paragraphs: vec![Paragraph::new(paragraph)], shading: None, row_span: 1, col_span: 1 }
    }

    /// The rows and columns the cell spans (at least 1 each).
    pub fn span(&self) -> (usize, usize) {
        (self.row_span.max(1) as usize, self.col_span.max(1) as usize)
    }

    /// Whether the cell spans more than its own place.
    pub fn is_merged(&self) -> bool {
        self.span() != (1, 1)
    }

    /// The cell's text, paragraphs separated by `'\n'`.
    pub fn text(&self) -> String {
        self.paragraphs.iter().map(Paragraph::text).collect::<Vec<_>>().join("\n")
    }

    /// Whether the cell holds no text.
    pub fn is_empty(&self) -> bool {
        self.paragraphs.iter().all(|p| p.is_empty() && p.tags.is_empty() && p.list.is_none())
    }
}

/// One row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    /// Stable id.
    pub id: Id,
    /// One cell per column.
    pub cells: Vec<TableCell>,
}

impl TableRow {
    /// Whether every cell is empty.
    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(TableCell::is_empty)
    }
}

fn yes() -> bool {
    true
}

/// A table on a page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    /// Stable id.
    pub id: Id,
    /// Top-left corner in page space.
    pub pos: Point,
    /// Column widths in points ([MS-ONE] `TableColumnWidths`).
    pub columns: Vec<f32>,
    /// The rows, top to bottom.
    pub rows: Vec<TableRow>,
    /// Whether borders are drawn (Table Layout › Hide Borders; [MS-ONE] `TableBordersVisible`).
    #[serde(default = "yes")]
    pub borders: bool,
    /// The first row is a header row, left in place by Sort (Table Layout › Sort › Header Row).
    #[serde(default)]
    pub header_row: bool,
}

impl Table {
    /// A `rows` × `cols` table of empty cells at `pos` (sizes clamped to the limits); `mint`
    /// supplies fresh ids.
    pub fn new(id: Id, pos: Point, rows: usize, cols: usize, mint: &mut dyn FnMut() -> Id) -> Table {
        let cols = cols.clamp(1, MAX_COLS);
        let rows = rows.clamp(1, MAX_ROWS).min((MAX_CELLS / cols).max(1));
        let rows = (0..rows).map(|_| Table::new_row(cols, mint)).collect();
        Table { id, pos: pos.sanitized(), columns: vec![DEFAULT_COL_WIDTH; cols], rows, borders: true, header_row: false }
    }

    /// A row of `cols` empty cells.
    pub fn new_row(cols: usize, mint: &mut dyn FnMut() -> Id) -> TableRow {
        TableRow { id: mint(), cells: (0..cols).map(|_| TableCell::new(mint(), mint())).collect() }
    }

    /// Number of rows.
    pub fn n_rows(&self) -> usize {
        self.rows.len()
    }

    /// Number of columns.
    pub fn n_cols(&self) -> usize {
        self.columns.len()
    }

    /// Row and column of the cell with `id`.
    pub fn locate(&self, id: Id) -> Option<(usize, usize)> {
        self.rows.iter().enumerate().find_map(|(r, row)| row.cells.iter().position(|c| c.id == id).map(|c| (r, c)))
    }

    /// The cell at `row`, `col`.
    pub fn cell(&self, row: usize, col: usize) -> Option<&TableCell> {
        self.rows.get(row)?.cells.get(col)
    }

    /// The cell at `row`, `col`, mutably.
    pub fn cell_mut(&mut self, row: usize, col: usize) -> Option<&mut TableCell> {
        self.rows.get_mut(row)?.cells.get_mut(col)
    }

    /// The cell with `id`.
    pub fn cell_by_id(&self, id: Id) -> Option<&TableCell> {
        self.rows.iter().flat_map(|r| r.cells.iter()).find(|c| c.id == id)
    }

    /// The cell with `id`, mutably.
    pub fn cell_by_id_mut(&mut self, id: Id) -> Option<&mut TableCell> {
        self.rows.iter_mut().flat_map(|r| r.cells.iter_mut()).find(|c| c.id == id)
    }

    /// Whether the table has no rows (such a table is removed from its page).
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty() || self.columns.is_empty()
    }

    /// The table as text: one line per row, cells separated by tabs (paragraphs inside a cell
    /// by spaces).
    pub fn plain_text(&self) -> String {
        self.rows
            .iter()
            .map(|r| r.cells.iter().map(|c| c.paragraphs.iter().map(Paragraph::text).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\t"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Restore the invariants (see the module docs). Cells added to pad a short row get ids
    /// derived from the row's id, so a hostile file can't make this mint ids.
    pub fn normalize(&mut self) {
        self.pos = self.pos.sanitized();
        if self.columns.is_empty() {
            let widest = self.rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
            self.columns = vec![DEFAULT_COL_WIDTH; widest];
        }
        self.columns.truncate(MAX_COLS);
        for w in &mut self.columns {
            *w = length(*w).clamp(MIN_COL_WIDTH, MAX_COL_WIDTH);
        }
        let cols = self.columns.len();
        if cols == 0 {
            self.rows.clear();
            return;
        }
        self.rows.truncate(MAX_ROWS.min(MAX_CELLS / cols));
        for row in &mut self.rows {
            row.cells.truncate(cols);
            let mut k = row.cells.len();
            while row.cells.len() < cols {
                let derived = |salt: u128| Id(row.id.0.rotate_left(29) ^ (k as u128 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt);
                row.cells.push(TableCell::new(derived(1), derived(2)));
                k += 1;
            }
            for (c, cell) in row.cells.iter_mut().enumerate() {
                if cell.paragraphs.is_empty() {
                    cell.paragraphs.push(Paragraph::new(Id(cell.id.0.rotate_left(17) ^ (c as u128 + 3))));
                }
                let mut prev: Option<u8> = None;
                for p in &mut cell.paragraphs {
                    p.normalize();
                    p.level = match prev {
                        Some(pl) => p.level.min(pl.saturating_add(1)).min(MAX_LEVEL),
                        None => 0,
                    };
                    prev = Some(p.level);
                }
            }
        }
        self.normalize_spans();
    }

    /// Total width in points.
    pub fn width(&self) -> f32 {
        self.columns.iter().sum()
    }

    /// Row by row, each cell's span when it shows, `None` when a merged cell covers it.
    pub fn span_grid(&self) -> Vec<Vec<Option<(usize, usize)>>> {
        let mut g: Vec<Vec<Option<(usize, usize)>>> = self.rows.iter().map(|r| r.cells.iter().map(|c| Some(c.span())).collect()).collect();
        for r in 0..g.len() {
            for c in 0..g.get(r).map_or(0, Vec::len) {
                let Some(Some((rs, cs))) = g.get(r).and_then(|row| row.get(c)).copied() else { continue };
                for rr in r..r.saturating_add(rs) {
                    for cc in c..c.saturating_add(cs) {
                        if (rr, cc) != (r, c)
                            && let Some(x) = g.get_mut(rr).and_then(|row| row.get_mut(cc))
                        {
                            *x = None;
                        }
                    }
                }
            }
        }
        g
    }

    /// The place of the cell that shows at `row`, `col`: the merged cell covering it, else itself.
    pub fn anchor(&self, row: usize, col: usize) -> (usize, usize) {
        for (r, rw) in self.rows.iter().enumerate().take(row.saturating_add(1)) {
            for (c, cell) in rw.cells.iter().enumerate().take(col.saturating_add(1)) {
                let (rs, cs) = cell.span();
                if (rs, cs) != (1, 1) && row < r.saturating_add(rs) && col < c.saturating_add(cs) {
                    return (r, c);
                }
            }
        }
        (row, col)
    }

    /// Whether a merged cell covers the place `row`, `col` (it is hidden).
    pub fn is_covered(&self, row: usize, col: usize) -> bool {
        self.anchor(row, col) != (row, col)
    }

    /// Whether any cell spans more than one row.
    pub fn has_row_spans(&self) -> bool {
        self.rows.iter().flat_map(|r| r.cells.iter()).any(|c| c.row_span > 1)
    }

    /// Grow the spans that cross place `at` after `n` rows were inserted there (or columns, with
    /// `rows` false): a row added inside a merged cell joins it.
    pub fn grow_spans(&mut self, rows: bool, at: usize, n: usize) {
        for (r, row) in self.rows.iter_mut().enumerate() {
            for (c, cell) in row.cells.iter_mut().enumerate() {
                let (start, span) = if rows { (r, &mut cell.row_span) } else { (c, &mut cell.col_span) };
                if start < at && at < start.saturating_add(*span as usize) {
                    *span = span.saturating_add(n as u32);
                }
            }
        }
    }

    /// Remove rows `a..=b` (or columns, with `rows` false), keeping merged cells that reach past
    /// them: a span crossing the range shrinks, and a merged cell whose own row (column) goes
    /// moves, with its text, to the first place left below (right of) the range.
    pub fn remove_lines(&mut self, rows: bool, a: usize, b: usize) {
        let total = if rows { self.n_rows() } else { self.n_cols() };
        if a > b || a >= total {
            return;
        }
        let b = b.min(total - 1);
        let anchors: Vec<(usize, usize, usize, usize)> = self
            .rows
            .iter()
            .enumerate()
            .flat_map(|(r, row)| {
                row.cells.iter().enumerate().filter(|(_, c)| c.is_merged()).map(move |(c, cell)| (r, c, cell.span().0, cell.span().1))
            })
            .collect();
        for (r, c, rs, cs) in anchors {
            let (start, span) = if rows { (r, rs) } else { (c, cs) };
            let end = start + span - 1;
            if end < a || start > b {
                continue;
            }
            let kept = span - (end.min(b) + 1 - start.max(a));
            if kept == 0 {
                continue;
            }
            if start >= a {
                // Its own row (column) goes: it moves to the first place after the range.
                let (to_r, to_c) = if rows { (b + 1, c) } else { (r, b + 1) };
                let moved = self.cell(r, c).cloned();
                let covered = self.cell(to_r, to_c).cloned();
                if let (Some(mut m), Some(cv)) = (moved, covered) {
                    if rows {
                        m.row_span = kept as u32;
                    } else {
                        m.col_span = kept as u32;
                    }
                    if let Some(x) = self.cell_mut(to_r, to_c) {
                        *x = m;
                    }
                    if let Some(x) = self.cell_mut(r, c) {
                        *x = TableCell { row_span: 1, col_span: 1, ..cv };
                    }
                }
            } else if let Some(x) = self.cell_mut(r, c) {
                if rows {
                    x.row_span = kept as u32;
                } else {
                    x.col_span = kept as u32;
                }
            }
        }
        if rows {
            self.rows.drain(a..=b);
        } else {
            self.columns.drain(a..=b);
            for r in &mut self.rows {
                let n = r.cells.len();
                if a < n {
                    r.cells.drain(a..=b.min(n - 1));
                }
            }
        }
    }

    /// Keep spans at least 1, inside the table and apart: row by row, a cell covered by an
    /// earlier one loses its own span, and a span that would reach a covered place stops short.
    fn normalize_spans(&mut self) {
        let (nr, nc) = (self.rows.len(), self.columns.len());
        let mut covered = vec![vec![false; nc]; nr];
        for r in 0..nr {
            for c in 0..nc {
                let is_cov = covered.get(r).and_then(|row| row.get(c)).copied().unwrap_or(true);
                let Some(cell) = self.rows.get_mut(r).and_then(|row| row.cells.get_mut(c)) else { continue };
                if is_cov {
                    cell.row_span = 1;
                    cell.col_span = 1;
                    continue;
                }
                let free = |rr: usize, cc: usize| !covered.get(rr).and_then(|row| row.get(cc)).copied().unwrap_or(true);
                let mut cs = (cell.col_span.max(1) as usize).min(nc - c);
                if let Some(k) = (c..c + cs).position(|x| !free(r, x)) {
                    cs = k.max(1);
                }
                let mut rs = (cell.row_span.max(1) as usize).min(nr - r);
                if let Some(k) = (r + 1..r + rs).position(|rr| (c..c + cs).any(|x| !free(rr, x))) {
                    rs = k + 1;
                }
                cell.row_span = rs as u32;
                cell.col_span = cs as u32;
                for row in covered.iter_mut().skip(r).take(rs) {
                    for x in row.iter_mut().skip(c).take(cs) {
                        *x = true;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minter() -> impl FnMut() -> Id {
        let mut n = 1000u128;
        move || {
            n += 1;
            Id(n)
        }
    }

    #[test]
    fn new_tables_are_clamped_and_complete() {
        let mut m = minter();
        let t = Table::new(Id(1), Point::new(10.0, 20.0), 2, 3, &mut m);
        assert_eq!((t.n_rows(), t.n_cols()), (2, 3));
        assert!(t.rows.iter().all(|r| r.cells.len() == 3 && r.cells.iter().all(|c| c.paragraphs.len() == 1)));
        let big = Table::new(Id(2), Point::ZERO, usize::MAX, usize::MAX, &mut m);
        assert_eq!(big.n_cols(), MAX_COLS);
        assert!(big.n_rows() * big.n_cols() <= MAX_CELLS);
        let none = Table::new(Id(3), Point::ZERO, 0, 0, &mut m);
        assert_eq!((none.n_rows(), none.n_cols()), (1, 1));
    }

    #[test]
    fn normalize_repairs_hostile_tables() {
        let mut m = minter();
        let mut t = Table::new(Id(1), Point::new(f32::NAN, f32::INFINITY), 3, 2, &mut m);
        t.columns = vec![f32::NAN, -5.0, 1e9];
        t.rows[0].cells.truncate(1);
        t.rows[1].cells[0].paragraphs.clear();
        t.rows[2].cells[1].paragraphs[0].level = 7;
        t.normalize();
        assert_eq!(t.columns, vec![MIN_COL_WIDTH, MIN_COL_WIDTH, MAX_COL_WIDTH]);
        assert!(t.rows.iter().all(|r| r.cells.len() == 3), "short rows padded");
        assert_eq!(t.rows[1].cells[0].paragraphs.len(), 1, "a cell always has a paragraph");
        assert_eq!(t.rows[2].cells[1].paragraphs[0].level, 0);
        assert!(t.pos.x.is_finite() && t.pos.y.is_finite());
        let padded: Vec<Id> = t.rows[0].cells.iter().map(|c| c.id).collect();
        assert_ne!(padded[1], padded[2], "padding cells get distinct ids");
        // No columns and no cells: the table is empty (its page drops it).
        let mut e = Table { id: Id(9), pos: Point::ZERO, columns: vec![], rows: vec![], borders: true, header_row: false };
        e.normalize();
        assert!(e.is_empty());
    }

    fn spans(t: &Table) -> Vec<Vec<(u32, u32)>> {
        t.rows.iter().map(|r| r.cells.iter().map(|c| (c.row_span, c.col_span)).collect()).collect()
    }

    #[test]
    fn merged_cells_cover_their_places_and_hostile_spans_are_repaired() {
        let mut m = minter();
        let mut t = Table::new(Id(1), Point::ZERO, 3, 3, &mut m);
        t.rows[0].cells[0].row_span = 2;
        t.rows[0].cells[0].col_span = 2;
        // Overlaps the first one, reaches outside the table, and a zero span.
        t.rows[1].cells[1].col_span = 2;
        t.rows[0].cells[2].row_span = 99;
        t.rows[2].cells[0].col_span = 0;
        t.normalize();
        assert_eq!(spans(&t), vec![vec![(2, 2), (1, 1), (3, 1)], vec![(1, 1), (1, 1), (1, 1)], vec![(1, 1), (1, 1), (1, 1)]]);
        let g = t.span_grid();
        assert_eq!(g[0], vec![Some((2, 2)), None, Some((3, 1))]);
        assert_eq!(g[1], vec![None, None, None]);
        assert_eq!(g[2], vec![Some((1, 1)), Some((1, 1)), None]);
        assert_eq!((t.anchor(1, 1), t.anchor(2, 2), t.anchor(2, 0)), ((0, 0), (0, 2), (2, 0)));
        assert!(t.is_covered(1, 0) && !t.is_covered(2, 1) && t.has_row_spans());
        // A span that would reach a covered place stops short.
        let mut u = Table::new(Id(2), Point::ZERO, 2, 3, &mut m);
        u.rows[0].cells[1].row_span = 2;
        u.rows[1].cells[0].col_span = 3;
        u.normalize();
        assert_eq!(spans(&u)[1][0], (1, 1));
    }

    #[test]
    fn rows_and_columns_added_inside_a_merged_cell_join_it_and_removed_ones_leave_it() {
        let mut m = minter();
        let mut t = Table::new(Id(1), Point::ZERO, 4, 3, &mut m);
        t.rows[0].cells[1].row_span = 3;
        t.rows[0].cells[1].paragraphs[0] = Paragraph::with_text(Id(70), "merged");
        t.normalize();
        // A row inserted at 2 (inside rows 0–2) joins the span; one at 0 or 3 doesn't.
        let row = Table::new_row(3, &mut m);
        t.rows.insert(2, row);
        t.grow_spans(true, 2, 1);
        assert_eq!(t.rows[0].cells[1].row_span, 4);
        // Removing rows 0–1 takes the merged cell's own row: it moves to row 2 with its text.
        let id = t.rows[0].cells[1].id;
        t.remove_lines(true, 0, 1);
        assert_eq!((t.n_rows(), t.locate(id)), (3, Some((0, 1))));
        assert_eq!((t.rows[0].cells[1].row_span, t.rows[0].cells[1].text()), (2, "merged".to_string()));
        // Columns: a span across removed columns shrinks.
        t.rows[2].cells[0].col_span = 3;
        t.normalize();
        t.remove_lines(false, 1, 1);
        assert_eq!((t.n_cols(), t.rows[2].cells[0].col_span), (2, 2));
        t.normalize();
        assert!(t.rows.iter().all(|r| r.cells.len() == 2));
        // Out of range and reversed ranges change nothing.
        let before = t.clone();
        t.remove_lines(true, 9, 12);
        t.remove_lines(false, 1, 0);
        assert_eq!(t, before);
    }

    #[test]
    fn locate_and_text() {
        let mut m = minter();
        let mut t = Table::new(Id(1), Point::ZERO, 2, 2, &mut m);
        t.rows[1].cells[0].paragraphs[0] = Paragraph::with_text(Id(77), "b1");
        t.rows[0].cells[1].paragraphs[0] = Paragraph::with_text(Id(78), "a2");
        let id = t.rows[1].cells[0].id;
        assert_eq!(t.locate(id), Some((1, 0)));
        assert_eq!(t.cell_by_id(id).map(TableCell::text), Some("b1".to_string()));
        assert_eq!(t.plain_text(), "\ta2\nb1\t");
        assert!(t.rows[0].cells[0].is_empty() && !t.rows[1].is_empty());
    }
}
