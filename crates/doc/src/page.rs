//! Pages and what is on them: note containers (outlines) on a free-form canvas.
//!
//! A page is a free-form canvas in page space (points, origin top-left; see `notecraft-geom`).
//! Everything on it is a [`PageObject`] with a position: an [`Outline`] (a "note container"),
//! a [`Table`], a picture ([`Image`], printouts included), an attached file ([`Attachment`]) or
//! an ink stroke ([`Ink`]).
//!
//! **Outlines are flat.** The reference stores a tree of outline elements ([MS-ONE]
//! `jcidOutlineElementNode` with `ElementChildNodes`); NoteCraft stores the same structure as a
//! flat list of [`Paragraph`]s each with an indent `level`, under the invariant
//! `level[i+1] <= level[i] + 1`. A paragraph's *children* are the paragraphs that follow it with
//! a deeper level, so the tree is recoverable exactly ([`Outline::subtree`]); flat storage makes
//! caret movement, multi-paragraph selection and deletion straightforward.

use notecraft_color::Rgba;
use notecraft_geom::{Point, finite, length};
use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::ink::Ink;
use crate::media::{Attachment, Blob, Image, MAX_PAGE_OBJECTS};
use crate::table::{Table, TableCell};
use crate::text::{MAX_LEVEL, ParaStyle, Paragraph};
use crate::time::Timestamp;

/// Deepest subpage level: a page can be indented two levels below a top-level page (catalog
/// `BEH.Pages.Subpages`; unverified exact maximum).
pub const MAX_PAGE_LEVEL: u8 = 2;

/// The default maximum width of an auto-sized note container before text wraps, in points
/// (catalog `BEH.Canvas.ContainerAutoWidth`; unverified: 6.5 inches).
pub const AUTO_WIDTH_MAX: f32 = 468.0;

/// Narrowest a container can be resized to, in points.
pub const MIN_OUTLINE_WIDTH: f32 = 36.0;

/// A note container's width.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum OutlineWidth {
    /// Grows with its text up to [`AUTO_WIDTH_MAX`], then wraps.
    #[default]
    Auto,
    /// Set by the user dragging the container's edge ([MS-ONE] `IsLayoutSizeSetByUser`).
    Fixed(f32),
}

/// A note container: positioned rich-text paragraphs with outline indentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outline {
    /// Stable id.
    pub id: Id,
    /// Top-left corner in page space.
    pub pos: Point,
    /// Width policy.
    #[serde(default)]
    pub width: OutlineWidth,
    /// The paragraphs (flat, with levels; see the module docs).
    pub paragraphs: Vec<Paragraph>,
}

impl Outline {
    /// A container at `pos` holding one empty paragraph.
    pub fn new(id: Id, pos: Point, first_paragraph: Id) -> Outline {
        Outline { id, pos: pos.sanitized(), width: OutlineWidth::Auto, paragraphs: vec![Paragraph::new(first_paragraph)] }
    }

    /// Enforce the invariants: position finite, fixed width clamped, every paragraph normalised,
    /// and `level[i+1] <= level[i] + 1` (levels that jump are pulled up).
    pub fn normalize(&mut self) {
        self.pos = self.pos.sanitized();
        if let OutlineWidth::Fixed(w) = self.width {
            self.width = OutlineWidth::Fixed(length(w).max(MIN_OUTLINE_WIDTH));
        }
        let mut prev: Option<u8> = None;
        for p in &mut self.paragraphs {
            p.normalize();
            if let Some(pl) = prev {
                p.level = p.level.min(pl.saturating_add(1)).min(MAX_LEVEL);
            }
            prev = Some(p.level);
        }
    }

    /// The index range `i..end` of paragraph `i` and all its descendants (empty if `i` is out
    /// of range).
    pub fn subtree(&self, i: usize) -> std::ops::Range<usize> {
        let Some(p) = self.paragraphs.get(i) else { return i..i };
        let lvl = p.level;
        let mut end = i + 1;
        while self.paragraphs.get(end).is_some_and(|q| q.level > lvl) {
            end += 1;
        }
        i..end
    }

    /// Whether paragraph `i` is hidden because an ancestor is collapsed.
    pub fn is_hidden(&self, i: usize) -> bool {
        hidden_under_collapsed(&self.paragraphs, i, |p| (p.level, p.collapsed))
    }

    /// Whether every paragraph is empty (an empty container disappears when the caret leaves
    /// it; catalog `BEH.Canvas.EmptyContainerRemoved`).
    pub fn is_empty(&self) -> bool {
        self.paragraphs.iter().all(|p| p.is_empty() && p.tags.is_empty() && p.list.is_none())
    }

    /// Index of the paragraph with `id`.
    pub fn paragraph_index(&self, id: Id) -> Option<usize> {
        self.paragraphs.iter().position(|p| p.id == id)
    }
}

/// Something placed on a page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PageObject {
    /// A note container.
    Outline(Outline),
    /// A table (`.ncraft` format 2 and later).
    Table(Table),
    /// A picture (format 3 and later; decision D3).
    Image(Image),
    /// An attached file (format 3 and later; decision D3).
    Attachment(Attachment),
    /// An ink stroke (format 4 and later; decision D4).
    Ink(Ink),
}

impl PageObject {
    /// The object's id.
    pub fn id(&self) -> Id {
        match self {
            PageObject::Outline(o) => o.id,
            PageObject::Table(t) => t.id,
            PageObject::Image(i) => i.id,
            PageObject::Attachment(a) => a.id,
            PageObject::Ink(k) => k.id,
        }
    }

    /// The object's position.
    pub fn pos(&self) -> Point {
        match self {
            PageObject::Outline(o) => o.pos,
            PageObject::Table(t) => t.pos,
            PageObject::Image(i) => i.pos,
            PageObject::Attachment(a) => a.pos,
            PageObject::Ink(k) => k.pos,
        }
    }

    /// Move the object (the position is sanitized).
    pub fn set_pos(&mut self, pos: Point) {
        match self {
            PageObject::Outline(o) => o.pos = pos.sanitized(),
            PageObject::Table(t) => t.pos = pos.sanitized(),
            PageObject::Image(i) => i.pos = pos.sanitized(),
            PageObject::Attachment(a) => a.pos = pos.sanitized(),
            PageObject::Ink(k) => k.pos = pos.sanitized(),
        }
    }

    /// The outline, if this is one.
    pub fn as_outline(&self) -> Option<&Outline> {
        match self {
            PageObject::Outline(o) => Some(o),
            _ => None,
        }
    }

    /// The outline, mutably, if this is one.
    pub fn as_outline_mut(&mut self) -> Option<&mut Outline> {
        match self {
            PageObject::Outline(o) => Some(o),
            _ => None,
        }
    }

    /// The table, if this is one.
    pub fn as_table(&self) -> Option<&Table> {
        match self {
            PageObject::Table(t) => Some(t),
            _ => None,
        }
    }

    /// The table, mutably, if this is one.
    pub fn as_table_mut(&mut self) -> Option<&mut Table> {
        match self {
            PageObject::Table(t) => Some(t),
            _ => None,
        }
    }

    /// Every paragraph in the object (a container's, or each cell's in reading order).
    pub fn paragraphs(&self) -> Box<dyn Iterator<Item = &Paragraph> + '_> {
        match self {
            PageObject::Outline(o) => Box::new(o.paragraphs.iter()),
            PageObject::Table(t) => Box::new(t.rows.iter().flat_map(|r| r.cells.iter()).flat_map(|c| c.paragraphs.iter())),
            PageObject::Image(_) | PageObject::Attachment(_) | PageObject::Ink(_) => Box::new(std::iter::empty()),
        }
    }

    /// The picture, if this is one.
    pub fn as_image(&self) -> Option<&Image> {
        match self {
            PageObject::Image(i) => Some(i),
            _ => None,
        }
    }

    /// The picture, mutably, if this is one.
    pub fn as_image_mut(&mut self) -> Option<&mut Image> {
        match self {
            PageObject::Image(i) => Some(i),
            _ => None,
        }
    }

    /// The attachment, if this is one.
    pub fn as_attachment(&self) -> Option<&Attachment> {
        match self {
            PageObject::Attachment(a) => Some(a),
            _ => None,
        }
    }

    /// The attachment, mutably, if this is one.
    pub fn as_attachment_mut(&mut self) -> Option<&mut Attachment> {
        match self {
            PageObject::Attachment(a) => Some(a),
            _ => None,
        }
    }

    /// The ink stroke, if this is one.
    pub fn as_ink(&self) -> Option<&Ink> {
        match self {
            PageObject::Ink(k) => Some(k),
            _ => None,
        }
    }

    /// The ink stroke, mutably, if this is one.
    pub fn as_ink_mut(&mut self) -> Option<&mut Ink> {
        match self {
            PageObject::Ink(k) => Some(k),
            _ => None,
        }
    }

    /// The hyperlink on a picture, attachment or ink stroke (`None` for containers and tables,
    /// whose links live in their text).
    pub fn link(&self) -> Option<&str> {
        match self {
            PageObject::Image(i) => i.link.as_deref(),
            PageObject::Attachment(a) => a.link.as_deref(),
            PageObject::Ink(k) => k.link.as_deref(),
            PageObject::Outline(_) | PageObject::Table(_) => None,
        }
    }

    /// The link slot of a picture, attachment or ink stroke.
    pub fn link_mut(&mut self) -> Option<&mut Option<String>> {
        match self {
            PageObject::Image(i) => Some(&mut i.link),
            PageObject::Attachment(a) => Some(&mut a.link),
            PageObject::Ink(k) => Some(&mut k.link),
            PageObject::Outline(_) | PageObject::Table(_) => None,
        }
    }

    /// The stored bytes of a picture or attachment.
    pub fn blob(&self) -> Option<&Blob> {
        match self {
            PageObject::Image(i) => Some(&i.blob),
            PageObject::Attachment(a) => Some(&a.blob),
            _ => None,
        }
    }
}

/// Rule lines and grids (View › Rule Lines; catalog `BEH.View.RuleLines`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RuleLines {
    /// No lines.
    #[default]
    None,
    /// Narrow ruled.
    Narrow,
    /// College ruled.
    College,
    /// Standard ruled.
    Standard,
    /// Wide ruled.
    Wide,
    /// Small grid.
    GridSmall,
    /// Medium grid.
    GridMedium,
    /// Large grid.
    GridLarge,
    /// Very large grid.
    GridVeryLarge,
}

impl RuleLines {
    /// Spacing between lines in points (unverified values: 1/4 in for narrow, 9/32 in for
    /// college, 11/32 in standard, 7/16 in wide; grids 1/8, 1/4, 3/8, 1/2 in).
    pub fn spacing(self) -> f32 {
        match self {
            RuleLines::None => 0.0,
            RuleLines::Narrow => 18.0,
            RuleLines::College => 20.25,
            RuleLines::Standard => 24.75,
            RuleLines::Wide => 31.5,
            RuleLines::GridSmall => 9.0,
            RuleLines::GridMedium => 18.0,
            RuleLines::GridLarge => 27.0,
            RuleLines::GridVeryLarge => 36.0,
        }
    }

    /// Whether this draws a grid (vertical lines too).
    pub fn is_grid(self) -> bool {
        matches!(self, RuleLines::GridSmall | RuleLines::GridMedium | RuleLines::GridLarge | RuleLines::GridVeryLarge)
    }

    /// Every option in gallery order.
    pub const ALL: [RuleLines; 9] = [
        RuleLines::None,
        RuleLines::Narrow,
        RuleLines::College,
        RuleLines::Standard,
        RuleLines::Wide,
        RuleLines::GridSmall,
        RuleLines::GridMedium,
        RuleLines::GridLarge,
        RuleLines::GridVeryLarge,
    ];
}

/// A fixed paper size (View › Paper Size); `None` on a page means the page grows freely.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PaperSize {
    /// Width in points.
    pub width: f32,
    /// Height in points.
    pub height: f32,
    /// Print margins (top, right, bottom, left) in points.
    pub margins: [f32; 4],
}

/// Per-page appearance.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PageSetup {
    /// Page background colour (`None` = no colour).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgba>,
    /// Rule lines or grid.
    pub rule_lines: RuleLines,
    /// Rule line colour (`None` = default light blue).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_color: Option<Rgba>,
    /// Hide the page title (View › Hide Page Title).
    pub hide_title: bool,
    /// Fixed paper size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper: Option<PaperSize>,
}

/// A page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    /// Stable id.
    pub id: Id,
    /// The title (a Page Title paragraph; its text is the page tab name).
    pub title: Paragraph,
    /// Subpage level 0..=[`MAX_PAGE_LEVEL`].
    #[serde(default)]
    pub level: u8,
    /// When the page was created (shown under the title).
    pub created: Timestamp,
    /// When the page was last changed.
    pub modified: Timestamp,
    /// The date and time shown under the title (defaults to `created`; the user can change it).
    #[serde(default)]
    pub shown_time: Option<Timestamp>,
    /// Who created the page.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    /// Objects on the page, in z-order (back to front).
    #[serde(default)]
    pub objects: Vec<PageObject>,
    /// Appearance.
    #[serde(default)]
    pub setup: PageSetup,
    /// Whether its subpages are collapsed in the page list.
    #[serde(default)]
    pub collapsed: bool,
}

/// What an untitled page is called in the page list (catalog `BEH.Pages.Title`).
pub const UNTITLED: &str = "Untitled page";

impl Page {
    /// A new empty page.
    pub fn new(id: Id, title_id: Id, now: Timestamp, author: &str) -> Page {
        let mut title = Paragraph::new(title_id);
        title.style = ParaStyle::PageTitle;
        Page {
            id,
            title,
            level: 0,
            created: now,
            modified: now,
            shown_time: None,
            author: author.to_string(),
            objects: Vec::new(),
            setup: PageSetup::default(),
            collapsed: false,
        }
    }

    /// The title text (may be empty).
    pub fn title_text(&self) -> String {
        self.title.text()
    }

    /// The name shown in the page list: the title, or [`UNTITLED`] when it is blank. Soft line
    /// breaks become spaces.
    pub fn display_title(&self) -> String {
        let t = self.title_text().replace('\n', " ");
        let t = t.trim();
        if t.is_empty() { UNTITLED.to_string() } else { t.to_string() }
    }

    /// The date/time shown under the title.
    pub fn shown_time(&self) -> Timestamp {
        self.shown_time.unwrap_or(self.created)
    }

    /// Enforce invariants: level clamped, title normalised and styled, every object
    /// normalised, empty outlines and tables removed, at most [`MAX_PAGE_OBJECTS`] objects.
    pub fn normalize(&mut self) {
        self.level = self.level.min(MAX_PAGE_LEVEL);
        self.title.normalize();
        self.title.style = ParaStyle::PageTitle;
        self.title.level = 0;
        for o in &mut self.objects {
            match o {
                PageObject::Outline(ol) => ol.normalize(),
                PageObject::Table(t) => t.normalize(),
                PageObject::Image(i) => i.normalize(),
                PageObject::Attachment(a) => a.normalize(),
                PageObject::Ink(k) => k.normalize(),
            }
        }
        self.objects.retain(|o| match o {
            PageObject::Outline(ol) => !ol.paragraphs.is_empty(),
            PageObject::Table(t) => !t.is_empty(),
            PageObject::Image(_) | PageObject::Attachment(_) => true,
            PageObject::Ink(k) => !k.points.is_empty(),
        });
        self.objects.truncate(MAX_PAGE_OBJECTS);
    }

    /// Index of the object with `id`.
    pub fn object_index(&self, id: Id) -> Option<usize> {
        self.objects.iter().position(|o| o.id() == id)
    }

    /// The outline with `id`.
    pub fn outline(&self, id: Id) -> Option<&Outline> {
        self.objects.iter().find_map(|o| o.as_outline().filter(|ol| ol.id == id))
    }

    /// The outline with `id`, mutably.
    pub fn outline_mut(&mut self, id: Id) -> Option<&mut Outline> {
        self.objects.iter_mut().find_map(|o| o.as_outline_mut().filter(|ol| ol.id == id))
    }

    /// The table with `id`.
    pub fn table(&self, id: Id) -> Option<&Table> {
        self.objects.iter().find_map(|o| o.as_table().filter(|t| t.id == id))
    }

    /// The table with `id`, mutably.
    pub fn table_mut(&mut self, id: Id) -> Option<&mut Table> {
        self.objects.iter_mut().find_map(|o| o.as_table_mut().filter(|t| t.id == id))
    }

    /// The table holding the cell `cell`, with the cell's row and column.
    pub fn table_of_cell(&self, cell: Id) -> Option<(&Table, usize, usize)> {
        self.objects.iter().filter_map(PageObject::as_table).find_map(|t| t.locate(cell).map(|(r, c)| (t, r, c)))
    }

    /// The cell with `id` in any table on the page.
    pub fn cell(&self, id: Id) -> Option<&TableCell> {
        self.objects.iter().filter_map(PageObject::as_table).find_map(|t| t.cell_by_id(id))
    }

    /// The cell with `id`, mutably.
    pub fn cell_mut(&mut self, id: Id) -> Option<&mut TableCell> {
        self.objects.iter_mut().filter_map(PageObject::as_table_mut).find_map(|t| t.cell_by_id_mut(id))
    }

    /// The picture with `id`.
    pub fn image(&self, id: Id) -> Option<&Image> {
        self.objects.iter().find_map(|o| o.as_image().filter(|i| i.id == id))
    }

    /// The picture with `id`, mutably.
    pub fn image_mut(&mut self, id: Id) -> Option<&mut Image> {
        self.objects.iter_mut().find_map(|o| o.as_image_mut().filter(|i| i.id == id))
    }

    /// The attachment with `id`.
    pub fn attachment(&self, id: Id) -> Option<&Attachment> {
        self.objects.iter().find_map(|o| o.as_attachment().filter(|a| a.id == id))
    }

    /// The ink stroke with `id`.
    pub fn ink(&self, id: Id) -> Option<&Ink> {
        self.objects.iter().find(|o| o.id() == id).and_then(PageObject::as_ink)
    }

    /// The ink stroke with `id`, mutably.
    pub fn ink_mut(&mut self, id: Id) -> Option<&mut Ink> {
        self.objects.iter_mut().find(|o| o.id() == id).and_then(PageObject::as_ink_mut)
    }

    /// The attachment with `id`, mutably.
    pub fn attachment_mut(&mut self, id: Id) -> Option<&mut Attachment> {
        self.objects.iter_mut().find_map(|o| o.as_attachment_mut().filter(|a| a.id == id))
    }

    /// All plain text on the page (title first), for search and exports. A table's cells are
    /// separated by tabs, its rows by line breaks; an attachment contributes its file name
    /// (pictures and ink contribute nothing until OCR and handwriting recognition exist).
    pub fn plain_text(&self) -> String {
        let mut s = self.title_text();
        for o in &self.objects {
            match o {
                PageObject::Outline(ol) => {
                    for p in &ol.paragraphs {
                        s.push('\n');
                        s.push_str(&p.text());
                    }
                }
                PageObject::Table(t) => {
                    s.push('\n');
                    s.push_str(&t.plain_text());
                }
                PageObject::Attachment(a) => {
                    s.push('\n');
                    s.push_str(&a.name);
                }
                PageObject::Image(_) | PageObject::Ink(_) => {}
            }
        }
        s
    }

    /// Clamp every coordinate so a hostile file can't place objects at ±∞.
    pub fn sanitize_positions(&mut self) {
        for o in &mut self.objects {
            let p = o.pos();
            o.set_pos(Point::new(finite(p.x), finite(p.y)));
        }
    }
}

/// Whether item `i` of a levelled list (paragraphs of an outline, pages of a section) is hidden
/// because an ancestor — the nearest earlier item at each lower level — is collapsed. An index
/// past the end counts as hidden.
pub(crate) fn hidden_under_collapsed<T>(items: &[T], i: usize, level_collapsed: impl Fn(&T) -> (u8, bool)) -> bool {
    let Some(item) = items.get(i) else { return true };
    let mut need = level_collapsed(item).0;
    for q in items.get(..i).unwrap_or_default().iter().rev() {
        let (level, collapsed) = level_collapsed(q);
        if level < need {
            if collapsed {
                return true;
            }
            need = level;
            if need == 0 {
                break;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outline(levels: &[u8]) -> Outline {
        let mut o = Outline::new(Id(100), Point::ZERO, Id(1));
        o.paragraphs = levels
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let mut p = Paragraph::with_text(Id(i as u128 + 1), &format!("p{i}"));
                p.level = *l;
                p
            })
            .collect();
        o
    }

    #[test]
    fn subtree_and_hidden() {
        let mut o = outline(&[0, 1, 2, 1, 0]);
        assert_eq!(o.subtree(0), 0..4);
        assert_eq!(o.subtree(1), 1..3);
        assert_eq!(o.subtree(4), 4..5);
        assert_eq!(o.subtree(99), 99..99);
        o.paragraphs[1].collapsed = true;
        assert!(o.is_hidden(2));
        assert!(!o.is_hidden(3));
        assert!(!o.is_hidden(1));
    }

    #[test]
    fn normalize_pulls_up_level_jumps() {
        let mut o = outline(&[0, 3, 7, 1]);
        o.width = OutlineWidth::Fixed(-5.0);
        o.normalize();
        let lv: Vec<u8> = o.paragraphs.iter().map(|p| p.level).collect();
        assert_eq!(lv, vec![0, 1, 2, 1]);
        assert_eq!(o.width, OutlineWidth::Fixed(MIN_OUTLINE_WIDTH));
    }

    #[test]
    fn page_title_and_text() {
        // parity: BEH.Pages.Title
        let mut p = Page::new(Id(1), Id(2), Timestamp(0), "me");
        assert_eq!(p.display_title(), UNTITLED);
        p.title = Paragraph::with_text(Id(2), "  Meeting\nnotes ");
        p.level = 9;
        p.objects.push(PageObject::Outline(outline(&[0, 1])));
        p.normalize();
        assert_eq!(p.display_title(), "Meeting notes");
        assert_eq!(p.level, MAX_PAGE_LEVEL);
        assert_eq!(p.title.style, ParaStyle::PageTitle);
        assert!(p.plain_text().contains("p1"));
    }
}
