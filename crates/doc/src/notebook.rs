//! The notebook hierarchy: workspace → notebooks → section groups → sections → pages, and the
//! Notebook Recycle Bin.
//!
//! **Structural sharing.** Notebooks, sections and pages are held behind [`Arc`]s. Editing goes
//! through [`Arc::make_mut`], so cloning a [`Workspace`] is a cheap pointer copy and an edit
//! copies only the path from the root to the page it touches. The engine's undo history keeps
//! whole-workspace snapshots this way (docs/plan.md §4.2).
//!
//! **Addressing.** Objects are found by [`Id`]; [`SectionPath`] is the transient index path to
//! a section (notebook index, then child indices through nested section groups). Group nesting
//! is capped at [`MAX_GROUP_DEPTH`] so a hostile file cannot make lookups recurse without bound.

use std::sync::Arc;

use notecraft_color::{Rgba, SECTION_COLORS, SECTION_NONE};
use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::page::Page;
use crate::time::Timestamp;

/// Deepest section-group nesting accepted (deeper groups in a file are flattened on load).
pub const MAX_GROUP_DEPTH: usize = 16;
/// How long deleted items stay in the Notebook Recycle Bin, in days (catalog
/// `BEH.Recycle.DeletedPages`; unverified figure).
pub const RECYCLE_DAYS: i64 = 60;
/// Longest name accepted for a notebook, section group or section, in characters.
pub const MAX_NAME: usize = 255;

/// A section's tab colour: one of the 16 palette entries or none
/// (catalog `CM.Section.NotebookColor1..16`, `NotebookColorNone`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct SectionColor(pub Option<u8>);

impl SectionColor {
    /// The colour to draw (palette entry, or the neutral "None" grey).
    pub fn rgba(self) -> Rgba {
        self.0.and_then(|i| SECTION_COLORS.get(i as usize)).map(|s| s.color).unwrap_or(SECTION_NONE)
    }

    /// Palette entry `i` (0-based; out of range = none).
    pub fn index(i: usize) -> SectionColor {
        SectionColor(if i < SECTION_COLORS.len() { Some(i as u8) } else { None })
    }

    /// The colour a new section gets: the palette in order, rotating (catalog
    /// `BEH.Sections.Colors`; unverified rotation).
    pub fn rotation(n: usize) -> SectionColor {
        SectionColor::index(n % SECTION_COLORS.len())
    }
}

/// A section: an ordered list of pages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    /// Stable id.
    pub id: Id,
    /// Name (the tab label; also the file name on disk).
    pub name: String,
    /// Tab colour.
    #[serde(default)]
    pub color: SectionColor,
    /// Pages in page-list order (subpages follow their parent with a higher level).
    #[serde(default)]
    pub pages: Vec<Arc<Page>>,
    /// When the section was created.
    #[serde(default)]
    pub created: Timestamp,
    /// The template new pages of this section start from (Templates › Choose default template;
    /// decision D10): `builtin:<slug>` or `my:<name>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_template: Option<String>,
}

impl Section {
    /// A new empty section.
    pub fn new(id: Id, name: &str, color: SectionColor, now: Timestamp) -> Section {
        Section { id, name: clean_name(name), color, pages: Vec::new(), created: now, default_template: None }
    }

    /// Index of the page with `id`.
    pub fn page_index(&self, id: Id) -> Option<usize> {
        self.pages.iter().position(|p| p.id == id)
    }

    /// The index range of page `i` and its subpages.
    pub fn page_family(&self, i: usize) -> std::ops::Range<usize> {
        let Some(p) = self.pages.get(i) else { return i..i };
        let lvl = p.level;
        let mut end = i + 1;
        while self.pages.get(end).is_some_and(|q| q.level > lvl) {
            end += 1;
        }
        i..end
    }

    /// Whether page `i` is hidden in the page list because a parent page is collapsed.
    pub fn page_hidden(&self, i: usize) -> bool {
        let Some(p) = self.pages.get(i) else { return true };
        let mut need = p.level;
        for j in (0..i).rev() {
            let Some(q) = self.pages.get(j) else { break };
            if q.level < need {
                if q.collapsed {
                    return true;
                }
                need = q.level;
                if need == 0 {
                    break;
                }
            }
        }
        false
    }

    /// Enforce invariants: the first page is top level and no page is more than one level
    /// deeper than the page before it; every page normalised.
    pub fn normalize(&mut self) {
        self.name = clean_name(&self.name);
        let mut prev: Option<u8> = None;
        for p in &mut self.pages {
            let want = match prev {
                None => 0,
                Some(pl) => p.level.min(pl.saturating_add(1)),
            };
            if p.level != want || !page_is_normal(p) {
                let pm = Arc::make_mut(p);
                pm.normalize();
                pm.level = want.min(crate::page::MAX_PAGE_LEVEL);
            }
            prev = Some(p.level);
        }
    }
}

impl Section {
    /// Normalise every page in full (outlines, tables, paragraphs, positions), not only the
    /// page order: for data loaded from disk, which is hostile. `normalize` stays cheap for the
    /// engine, which keeps pages normal as it edits them.
    pub fn normalize_pages(&mut self) {
        for p in &mut self.pages {
            let pm = Arc::make_mut(p);
            pm.normalize();
            pm.sanitize_positions();
        }
        self.normalize();
    }
}

impl Notebook {
    /// [`Section::normalize_pages`] for every section, then [`Notebook::normalize`].
    pub fn normalize_pages(&mut self) {
        fn walk(nodes: &mut [NavNode], depth: usize) {
            if depth > MAX_GROUP_DEPTH {
                return;
            }
            for n in nodes {
                match n {
                    NavNode::Group(g) => walk(&mut g.children, depth + 1),
                    NavNode::Section(s) => Arc::make_mut(s).normalize_pages(),
                }
            }
        }
        walk(&mut self.children, 0);
        self.normalize();
    }
}

fn page_is_normal(p: &Page) -> bool {
    p.level <= crate::page::MAX_PAGE_LEVEL
}

/// A section group: a folder of sections and further groups.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionGroup {
    /// Stable id.
    pub id: Id,
    /// Name.
    pub name: String,
    /// Children in tab order.
    #[serde(default)]
    pub children: Vec<NavNode>,
}

/// A child of a notebook or section group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum NavNode {
    /// A section.
    Section(Arc<Section>),
    /// A section group.
    Group(SectionGroup),
}

impl NavNode {
    /// The node's id.
    pub fn id(&self) -> Id {
        match self {
            NavNode::Section(s) => s.id,
            NavNode::Group(g) => g.id,
        }
    }

    /// The node's name.
    pub fn name(&self) -> &str {
        match self {
            NavNode::Section(s) => &s.name,
            NavNode::Group(g) => &g.name,
        }
    }
}

/// A deleted page waiting in the recycle bin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeletedPage {
    /// The page as it was.
    pub page: Arc<Page>,
    /// The section it came from (it may no longer exist).
    pub from_section: Id,
    /// That section's name when the page was deleted.
    pub from_section_name: String,
    /// When it was deleted.
    pub deleted: Timestamp,
}

/// A deleted section waiting in the recycle bin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeletedSection {
    /// The section as it was.
    pub section: Arc<Section>,
    /// When it was deleted.
    pub deleted: Timestamp,
}

/// The Notebook Recycle Bin (catalog `R.History.History.RecycleBin`, `BEH.Recycle.*`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RecycleBin {
    /// Deleted pages, oldest first.
    pub pages: Vec<DeletedPage>,
    /// Deleted sections, oldest first.
    pub sections: Vec<DeletedSection>,
}

impl RecycleBin {
    /// Drop items deleted more than [`RECYCLE_DAYS`] before `now`; returns how many were purged.
    pub fn purge(&mut self, now: Timestamp) -> usize {
        let cutoff = now.0.saturating_sub(RECYCLE_DAYS * 86_400_000);
        let before = self.pages.len() + self.sections.len();
        self.pages.retain(|d| d.deleted.0 >= cutoff);
        self.sections.retain(|d| d.deleted.0 >= cutoff);
        before - self.pages.len() - self.sections.len()
    }

    /// Whether the bin is empty.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty() && self.sections.is_empty()
    }
}

/// A notebook.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notebook {
    /// Stable id.
    pub id: Id,
    /// Display name (Notebook Properties › Display name).
    pub name: String,
    /// Notebook colour (its icon in the notebook pane).
    #[serde(default)]
    pub color: SectionColor,
    /// Sections and section groups in tab order.
    #[serde(default)]
    pub children: Vec<NavNode>,
    /// The recycle bin.
    #[serde(default)]
    pub recycle: RecycleBin,
}

impl Notebook {
    /// A new empty notebook.
    pub fn new(id: Id, name: &str, color: SectionColor) -> Notebook {
        Notebook { id, name: clean_name(name), color, children: Vec::new(), recycle: RecycleBin::default() }
    }

    /// Every section in tab order, depth-first through groups (depth-capped).
    pub fn sections(&self) -> Vec<&Arc<Section>> {
        fn walk<'a>(nodes: &'a [NavNode], depth: usize, out: &mut Vec<&'a Arc<Section>>) {
            if depth > MAX_GROUP_DEPTH {
                return;
            }
            for n in nodes {
                match n {
                    NavNode::Section(s) => out.push(s),
                    NavNode::Group(g) => walk(&g.children, depth + 1, out),
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.children, 0, &mut out);
        out
    }

    /// The child list at `group_path` (indices through nested groups; empty = the notebook's
    /// own children).
    pub fn children_at(&self, group_path: &[usize]) -> Option<&Vec<NavNode>> {
        let mut cur = &self.children;
        for &i in group_path.iter().take(MAX_GROUP_DEPTH + 1) {
            match cur.get(i)? {
                NavNode::Group(g) => cur = &g.children,
                NavNode::Section(_) => return None,
            }
        }
        Some(cur)
    }

    /// The child list at `group_path`, mutably.
    pub fn children_at_mut(&mut self, group_path: &[usize]) -> Option<&mut Vec<NavNode>> {
        let mut cur = &mut self.children;
        for &i in group_path.iter().take(MAX_GROUP_DEPTH + 1) {
            match cur.get_mut(i)? {
                NavNode::Group(g) => cur = &mut g.children,
                NavNode::Section(_) => return None,
            }
        }
        Some(cur)
    }

    /// The path (child indices) to the node with `id`.
    pub fn find(&self, id: Id) -> Option<Vec<usize>> {
        fn walk(nodes: &[NavNode], id: Id, depth: usize, path: &mut Vec<usize>) -> bool {
            if depth > MAX_GROUP_DEPTH {
                return false;
            }
            for (i, n) in nodes.iter().enumerate() {
                path.push(i);
                if n.id() == id {
                    return true;
                }
                if let NavNode::Group(g) = n
                    && walk(&g.children, id, depth + 1, path)
                {
                    return true;
                }
                path.pop();
            }
            false
        }
        let mut path = Vec::new();
        walk(&self.children, id, 0, &mut path).then_some(path)
    }

    /// The node at a full path.
    pub fn node_at(&self, path: &[usize]) -> Option<&NavNode> {
        let (last, parent) = path.split_last()?;
        self.children_at(parent)?.get(*last)
    }

    /// The node at a full path, mutably.
    pub fn node_at_mut(&mut self, path: &[usize]) -> Option<&mut NavNode> {
        let (last, parent) = path.split_last()?;
        self.children_at_mut(parent)?.get_mut(*last)
    }

    /// The section with `id`.
    pub fn section(&self, id: Id) -> Option<&Arc<Section>> {
        match self.node_at(&self.find(id)?)? {
            NavNode::Section(s) => Some(s),
            NavNode::Group(_) => None,
        }
    }

    /// The section with `id`, mutably (copy-on-write).
    pub fn section_mut(&mut self, id: Id) -> Option<&mut Section> {
        let path = self.find(id)?;
        match self.node_at_mut(&path)? {
            NavNode::Section(s) => Some(Arc::make_mut(s)),
            NavNode::Group(_) => None,
        }
    }

    /// The group with `id`.
    pub fn group(&self, id: Id) -> Option<&SectionGroup> {
        match self.node_at(&self.find(id)?)? {
            NavNode::Group(g) => Some(g),
            NavNode::Section(_) => None,
        }
    }

    /// Flatten groups nested deeper than [`MAX_GROUP_DEPTH`] into their ancestor at the cap,
    /// and normalise every section. Run on load.
    pub fn normalize(&mut self) {
        fn walk(nodes: &mut Vec<NavNode>, depth: usize) {
            let mut i = 0;
            while i < nodes.len() {
                let flatten = matches!(nodes.get(i), Some(NavNode::Group(_))) && depth >= MAX_GROUP_DEPTH;
                if flatten {
                    // Splice the group's children in its place and re-examine them at this depth.
                    if let NavNode::Group(g) = nodes.remove(i) {
                        for (k, c) in g.children.into_iter().enumerate() {
                            nodes.insert(i + k, c);
                        }
                    }
                    continue;
                }
                match nodes.get_mut(i) {
                    Some(NavNode::Group(g)) => {
                        g.name = clean_name(&g.name);
                        walk(&mut g.children, depth + 1);
                    }
                    Some(NavNode::Section(s)) => Arc::make_mut(s).normalize(),
                    None => {}
                }
                i += 1;
            }
        }
        self.name = clean_name(&self.name);
        walk(&mut self.children, 0);
    }
}

/// Every open notebook.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Workspace {
    /// Open notebooks in notebook-pane order.
    #[serde(default)]
    pub notebooks: Vec<Arc<Notebook>>,
}

impl Workspace {
    /// Index of the notebook with `id`.
    pub fn notebook_index(&self, id: Id) -> Option<usize> {
        self.notebooks.iter().position(|n| n.id == id)
    }

    /// The notebook with `id`.
    pub fn notebook(&self, id: Id) -> Option<&Arc<Notebook>> {
        self.notebooks.iter().find(|n| n.id == id)
    }

    /// The notebook with `id`, mutably (copy-on-write).
    pub fn notebook_mut(&mut self, id: Id) -> Option<&mut Notebook> {
        self.notebooks.iter_mut().find(|n| n.id == id).map(Arc::make_mut)
    }

    /// The notebook that contains the section or group with `id`.
    pub fn notebook_of(&self, node: Id) -> Option<&Arc<Notebook>> {
        self.notebooks.iter().find(|n| n.find(node).is_some())
    }

    /// The section with `id` in any notebook.
    pub fn section(&self, id: Id) -> Option<&Arc<Section>> {
        self.notebooks.iter().find_map(|n| n.section(id))
    }

    /// The section with `id`, mutably (copy-on-write along the path).
    pub fn section_mut(&mut self, id: Id) -> Option<&mut Section> {
        let nb = self.notebooks.iter().position(|n| n.find(id).is_some())?;
        Arc::make_mut(self.notebooks.get_mut(nb)?).section_mut(id)
    }

    /// The section that holds the page with `id`, and the page's index in it.
    pub fn locate_page(&self, id: Id) -> Option<(Id, usize)> {
        for nb in &self.notebooks {
            for s in nb.sections() {
                if let Some(i) = s.page_index(id) {
                    return Some((s.id, i));
                }
            }
        }
        None
    }

    /// The page with `id`.
    pub fn page(&self, id: Id) -> Option<&Arc<Page>> {
        let (sid, i) = self.locate_page(id)?;
        self.section(sid)?.pages.get(i)
    }

    /// The page with `id`, mutably (copy-on-write along the path).
    pub fn page_mut(&mut self, id: Id) -> Option<&mut Page> {
        let (sid, i) = self.locate_page(id)?;
        self.section_mut(sid)?.pages.get_mut(i).map(Arc::make_mut)
    }
}

/// A user-typed name made safe: control characters removed, trimmed, capped at [`MAX_NAME`]
/// characters. File-system-reserved characters are handled by the storage layer, not here (a
/// section may be called "A/B" in the UI).
pub fn clean_name(s: &str) -> String {
    let t: String = s.chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
    t.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(id: u128, name: &str) -> NavNode {
        NavNode::Section(Arc::new(Section::new(Id(id), name, SectionColor::index(0), Timestamp(0))))
    }

    fn sample() -> Notebook {
        let mut nb = Notebook::new(Id(1), "Work", SectionColor::default());
        nb.children = vec![
            section(10, "A"),
            NavNode::Group(SectionGroup {
                id: Id(20),
                name: "G".into(),
                children: vec![section(21, "B"), NavNode::Group(SectionGroup { id: Id(22), name: "H".into(), children: vec![section(23, "C")] })],
            }),
            section(30, "D"),
        ];
        nb
    }

    #[test]
    fn find_and_walk() {
        let nb = sample();
        assert_eq!(nb.find(Id(23)), Some(vec![1, 1, 0]));
        assert_eq!(nb.section(Id(23)).map(|s| s.name.as_str()), Some("C"));
        assert!(nb.section(Id(20)).is_none(), "a group is not a section");
        let names: Vec<_> = nb.sections().iter().map(|s| s.name.clone()).collect();
        assert_eq!(names, vec!["A", "B", "C", "D"]);
        assert!(nb.children_at(&[0]).is_none(), "a section has no children");
        assert!(nb.children_at(&[9]).is_none());
    }

    #[test]
    fn copy_on_write_shares_untouched_sections() {
        let mut ws = Workspace::default();
        ws.notebooks.push(Arc::new(sample()));
        let snapshot = ws.clone();
        ws.section_mut(Id(30)).unwrap().name = "D2".into();
        assert_eq!(snapshot.section(Id(30)).map(|s| s.name.as_str()), Some("D"));
        assert_eq!(ws.section(Id(30)).map(|s| s.name.as_str()), Some("D2"));
        // Untouched sections are still the same allocation.
        assert!(Arc::ptr_eq(snapshot.section(Id(10)).unwrap(), ws.section(Id(10)).unwrap()));
    }

    #[test]
    fn deep_groups_are_flattened() {
        let mut nb = Notebook::new(Id(1), "Deep", SectionColor::default());
        let mut node = section(9999, "leaf");
        for i in 0..40u128 {
            node = NavNode::Group(SectionGroup { id: Id(100 + i), name: format!("g{i}"), children: vec![node] });
        }
        nb.children = vec![node];
        nb.normalize();
        assert!(nb.find(Id(9999)).is_some(), "the leaf survives flattening");
        assert!(nb.find(Id(9999)).unwrap().len() <= MAX_GROUP_DEPTH + 1);
    }

    #[test]
    fn page_families_and_levels() {
        let mut s = Section::new(Id(1), "S", SectionColor::default(), Timestamp(0));
        for (i, lvl) in [0u8, 1, 2, 2, 0, 2].iter().enumerate() {
            let mut p = Page::new(Id(100 + i as u128), Id(200 + i as u128), Timestamp(0), "");
            p.level = *lvl;
            s.pages.push(Arc::new(p));
        }
        s.normalize();
        let lv: Vec<u8> = s.pages.iter().map(|p| p.level).collect();
        assert_eq!(lv, vec![0, 1, 2, 2, 0, 1], "a level jump after a top page is pulled up");
        assert_eq!(s.page_family(0), 0..4);
        assert_eq!(s.page_family(1), 1..4);
        Arc::make_mut(&mut s.pages[0]).collapsed = true;
        assert!(s.page_hidden(2));
        assert!(!s.page_hidden(4));
    }

    #[test]
    fn recycle_purge() {
        let mut bin = RecycleBin::default();
        let page = Arc::new(Page::new(Id(1), Id(2), Timestamp(0), ""));
        bin.pages.push(DeletedPage { page: page.clone(), from_section: Id(3), from_section_name: "S".into(), deleted: Timestamp(0) });
        bin.pages.push(DeletedPage { page, from_section: Id(3), from_section_name: "S".into(), deleted: Timestamp(59 * 86_400_000) });
        assert_eq!(bin.purge(Timestamp(61 * 86_400_000)), 1);
        assert_eq!(bin.pages.len(), 1);
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  a\u{7}b  "), "ab");
        assert_eq!(clean_name(&"x".repeat(1000)).chars().count(), MAX_NAME);
        assert_eq!(SectionColor::index(99), SectionColor(None));
        assert_eq!(SectionColor::rotation(17), SectionColor(Some(1)));
    }
}
