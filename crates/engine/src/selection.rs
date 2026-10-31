//! Where the user is: the current notebook/section/page, and the caret or selection on the
//! page.
//!
//! Text positions are `(block, paragraph index, byte offset)`. A *block* is the page title, one
//! note container (outline) or one table cell; a text selection stays inside one block, as in
//! the reference, where a drag across two containers selects the containers as objects rather
//! than a text range, and a drag across table cells selects the cells. Positions are always validated against the page before use
//! ([`Session::clamp_pos`](crate::Session::clamp_pos)), because commands arrive with stale or
//! hostile values.

use notecraft_doc::Id;
use serde::{Deserialize, Serialize};

/// A text block on a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id")]
pub enum Block {
    /// The page title.
    Title,
    /// A note container.
    Outline(Id),
    /// A table cell (the cell's id; its table is found on the page).
    Cell(Id),
}

/// A caret position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pos {
    /// The block.
    pub block: Block,
    /// Paragraph index in the block (always 0 for the title).
    pub para: usize,
    /// Byte offset in the paragraph text (on a grapheme boundary).
    pub offset: usize,
}

impl Pos {
    /// A position.
    pub fn new(block: Block, para: usize, offset: usize) -> Pos {
        Pos { block, para, offset }
    }
}

/// A text selection: from `anchor` (where it started) to `head` (where the caret is). Both are
/// in the same block. `anchor == head` is a plain caret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRange {
    /// Fixed end.
    pub anchor: Pos,
    /// Moving end (the caret).
    pub head: Pos,
}

impl TextRange {
    /// A collapsed range (a caret).
    pub fn caret(p: Pos) -> TextRange {
        TextRange { anchor: p, head: p }
    }

    /// Whether nothing is selected.
    pub fn is_caret(&self) -> bool {
        self.anchor == self.head
    }

    /// The range ordered start ≤ end (by paragraph, then offset).
    pub fn ordered(&self) -> (Pos, Pos) {
        let a = self.anchor;
        let h = self.head;
        if (a.para, a.offset) <= (h.para, h.offset) { (a, h) } else { (h, a) }
    }
}

/// What is selected on the page.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PageSel {
    /// Nothing (no caret on the page).
    #[default]
    None,
    /// A caret or text selection.
    Text(TextRange),
    /// Whole note containers selected as objects (Ctrl+A twice, clicking a container's bar).
    Objects {
        /// The selected objects.
        ids: Vec<Id>,
    },
    /// A rectangle of table cells (Table Layout › Select, or a drag across cells).
    Cells {
        /// The table.
        table: Id,
        /// First and last selected row (inclusive).
        rows: (usize, usize),
        /// First and last selected column (inclusive).
        cols: (usize, usize),
    },
}

/// Which pane has keyboard focus (decides what Alt+Shift+Up does, for example).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FocusArea {
    /// The page canvas.
    #[default]
    Canvas,
    /// The page list.
    PageList,
    /// The section tabs.
    SectionTabs,
    /// The notebook pane.
    NotebookPane,
}

/// The full selection state.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Selection {
    /// Current notebook.
    pub notebook: Option<Id>,
    /// Current section.
    pub section: Option<Id>,
    /// Current page.
    pub page: Option<Id>,
    /// Extra pages selected in the page list (Shift/Ctrl+click), besides `page`.
    #[serde(default)]
    pub pages: Vec<Id>,
    /// What is selected on the page.
    #[serde(default)]
    pub on_page: PageSel,
    /// Focused pane.
    #[serde(default)]
    pub focus: FocusArea,
}

impl Selection {
    /// The text range, if the selection is text.
    pub fn text(&self) -> Option<TextRange> {
        match self.on_page {
            PageSel::Text(r) => Some(r),
            _ => None,
        }
    }

    /// The caret (head of the text range).
    pub fn caret(&self) -> Option<Pos> {
        self.text().map(|r| r.head)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering() {
        let a = Pos::new(Block::Title, 0, 5);
        let b = Pos::new(Block::Title, 0, 2);
        let r = TextRange { anchor: a, head: b };
        assert_eq!(r.ordered(), (b, a));
        assert!(!r.is_caret());
        assert!(TextRange::caret(a).is_caret());
        let s = Selection { on_page: PageSel::Text(r), ..Default::default() };
        assert_eq!(s.caret(), Some(b));
    }
}
