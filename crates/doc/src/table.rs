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
//! **Invariants** (restored by [`Table::normalize`]): at least one row and one column, every row
//! has exactly one cell per column, every cell at least one paragraph, the sizes stay within
//! [`MAX_ROWS`], [`MAX_COLS`] and [`MAX_CELLS`] (data from disk and from agents is hostile), and
//! every width is finite and within [`MIN_COL_WIDTH`]..=[`MAX_COL_WIDTH`].

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
}

impl TableCell {
    /// An empty cell with one empty paragraph.
    pub fn new(id: Id, paragraph: Id) -> TableCell {
        TableCell { id, paragraphs: vec![Paragraph::new(paragraph)], shading: None }
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
    }

    /// Total width in points.
    pub fn width(&self) -> f32 {
        self.columns.iter().sum()
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
