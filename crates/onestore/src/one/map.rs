//! The walk from a resolved [`Store`] to a NoteCraft [`Section`], with a per-construct report of
//! what was kept and what was left out (the fidelity numbers of `docs/parity.md`).

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use notecraft_doc::id::IdGen;
use notecraft_doc::notebook::{Section, SectionColor};
use notecraft_doc::page::{MAX_PAGE_LEVEL, Outline, OutlineWidth, Page, PageObject};
use notecraft_doc::text::{MAX_LEVEL, ParaStyle, Paragraph};
use notecraft_doc::time::Timestamp;
use notecraft_geom::Point;

use super::{jcid, prop};
use crate::Limits;
use crate::bytes::utf16_lossy;
use crate::error::{OneError, Result};
use crate::header::FileKind;
use crate::propset::PropertySet;
use crate::revision::{ObjectSpace, ROOT_CONTENT, ROOT_METADATA, Store};
use crate::types::ExtendedGuid;

/// [MS-ONE] lengths are in half inches (§2.3.18): 36 points.
const HALF_INCH: f32 = 36.0;
/// Milliseconds from 1601-01-01 (FILETIME, [MS-DTYP] §2.3.3) to 1970-01-01.
const FILETIME_TO_UNIX_MS: i64 = 11_644_473_600_000;
/// Milliseconds from 1970-01-01 to 1980-01-01 ([MS-ONE] §2.3.1 Time32 epoch).
const TIME32_EPOCH_MS: i64 = 315_532_800_000;

/// An imported section and what the import kept or left out.
#[derive(Debug, Clone)]
pub struct Import {
    /// The section (new ids; the original file is untouched).
    pub section: Section,
    /// What was kept, degraded or lost.
    pub report: Report,
}

/// The per-construct fidelity report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Constructs read into the section, by name ("pages", "paragraphs", …).
    pub kept: BTreeMap<&'static str, usize>,
    /// Constructs this reader does not import yet, by name ("pictures", "tables", …).
    pub lost: BTreeMap<&'static str, usize>,
    /// Damage found on the way (objects that could not be decoded, missing references).
    pub warnings: Vec<String>,
}

impl Report {
    fn keep(&mut self, what: &'static str) {
        *self.kept.entry(what).or_default() += 1;
    }

    fn lose(&mut self, what: &'static str) {
        *self.lost.entry(what).or_default() += 1;
    }

    /// One line per construct, for notices and the CLI.
    pub fn summary(&self) -> String {
        let list = |m: &BTreeMap<&str, usize>| m.iter().map(|(k, v)| format!("{v} {k}")).collect::<Vec<_>>().join(", ");
        let mut s = format!("imported {}", if self.kept.is_empty() { "nothing".into() } else { list(&self.kept) });
        if !self.lost.is_empty() {
            s.push_str(&format!("; not imported yet: {}", list(&self.lost)));
        }
        if !self.warnings.is_empty() {
            s.push_str(&format!("; {} damaged objects skipped", self.warnings.len()));
        }
        s
    }
}

/// Reads a `.one` section file into a new section called `name`.
pub fn read_section(file: &[u8], name: &str, ids: &mut IdGen, now: Timestamp) -> Result<Import> {
    let limits = Limits::default();
    let store = Store::read(file, &limits)?;
    if store.kind() != FileKind::Section {
        return Err(OneError::Unsupported("this is a notebook's table of contents (.onetoc2), not a section".into()));
    }
    let mut w = Walk { store: &store, ids, report: Report::default(), now, depth: limits.max_depth };
    w.report.warnings = store.warnings.clone();
    let mut section = Section::new(w.ids.next_id(), name, SectionColor::default(), now);
    for space_id in w.page_spaces() {
        match store.space(&space_id) {
            Some(space) => {
                let page = w.page(space);
                section.pages.push(Arc::new(page));
                w.report.keep("pages");
            }
            None => w.report.warnings.push(format!("page object space {space_id:?} is missing")),
        }
    }
    section.normalize_pages();
    Ok(Import { section, report: w.report })
}

struct Walk<'s, 'a> {
    store: &'s Store<'a>,
    ids: &'s mut IdGen,
    report: Report,
    now: Timestamp,
    depth: usize,
}

impl Walk<'_, '_> {
    /// The page object spaces in page order: section node → page series → their spaces.
    fn page_spaces(&mut self) -> Vec<ExtendedGuid> {
        let Some(root) = self.store.root_space() else {
            self.report.warnings.push("the root object space is missing".into());
            return Vec::new();
        };
        let Some(section) = root.roots.get(&ROOT_CONTENT).and_then(|id| root.props(id)) else {
            self.report.warnings.push("the section node is missing".into());
            return Vec::new();
        };
        let mut seen = HashSet::new();
        section
            .objects(prop::ELEMENT_CHILD_NODES)
            .iter()
            .filter_map(|s| root.props(s))
            .flat_map(|series| series.spaces(prop::CHILD_GRAPH_SPACE_ELEMENT_NODES))
            // A page listed twice is imported once.
            .filter(|s| seen.insert(*s))
            .collect()
    }

    fn page(&mut self, space: &ObjectSpace) -> Page {
        let meta = space.roots.get(&ROOT_METADATA).and_then(|id| space.props(id));
        let created = meta.and_then(|m| m.uint(prop::TOPOLOGY_CREATION_TIME_STAMP)).map(filetime).unwrap_or(self.now);
        let mut page = Page::new(self.ids.next_id(), self.ids.next_id(), created, "");
        // §2.3.74 PageLevel 1–3 → subpage levels 0–2.
        if let Some(level) = meta.and_then(|m| m.uint(prop::PAGE_LEVEL)) {
            page.level = u8::try_from(level.saturating_sub(1)).unwrap_or(MAX_PAGE_LEVEL).min(MAX_PAGE_LEVEL);
        }
        let node = space
            .roots
            .get(&ROOT_CONTENT)
            .and_then(|id| space.props(id))
            .and_then(|m| m.objects(prop::CONTENT_CHILD_NODES).first().and_then(|p| space.props(p)));
        let mut title = None;
        if let Some(node) = node {
            if let Some(t) = node.uint(prop::LAST_MODIFIED_TIME) {
                page.modified = time32(t);
            }
            if let Some(a) = node.bytes(prop::AUTHOR) {
                page.author = wz(a);
            }
            title = node.objects(prop::STRUCTURE_ELEMENT_CHILD_NODES).first().and_then(|t| self.title(space, t));
            for child in node.objects(prop::ELEMENT_CHILD_NODES) {
                self.content(space, &child, &mut page);
            }
        }
        // The title outline's text, else the cached title (§2.2.62).
        let title = title.or_else(|| meta.and_then(|m| m.bytes(prop::CACHED_TITLE_STRING)).map(wz)).unwrap_or_default();
        if !title.is_empty() {
            let mut p = Paragraph::with_text(page.title.id, &title);
            p.style = ParaStyle::PageTitle;
            page.title = p;
        }
        page
    }

    /// §2.1.16: the text of the title node's IsTitleText outline.
    fn title(&mut self, space: &ObjectSpace, id: &ExtendedGuid) -> Option<String> {
        let node = space.props(id)?;
        let outlines = node.objects(prop::ELEMENT_CHILD_NODES);
        let outline = outlines.iter().filter_map(|o| space.props(o)).find(|o| o.bool(prop::IS_TITLE_TEXT) == Some(true))?;
        let mut paras = Vec::new();
        self.elements(space, outline, 0, &mut paras);
        let text = paras.iter().map(Paragraph::text).collect::<Vec<_>>().join(" ");
        Some(text).filter(|t| !t.trim().is_empty())
    }

    /// One child of the page node.
    fn content(&mut self, space: &ObjectSpace, id: &ExtendedGuid, page: &mut Page) {
        let Some(object) = space.objects.get(id) else { return };
        let Some(props) = space.props(id) else {
            self.report.warnings.push(format!("page content {id:?} could not be read"));
            return;
        };
        match object.jcid.0 {
            jcid::OUTLINE => {
                let mut paras = Vec::new();
                self.elements(space, props, 0, &mut paras);
                if paras.is_empty() {
                    return;
                }
                let x = props.f32(prop::OFFSET_FROM_PARENT_HORIZ).unwrap_or(0.0) * HALF_INCH;
                let y = props.f32(prop::OFFSET_FROM_PARENT_VERT).unwrap_or(0.0) * HALF_INCH;
                let mut outline = Outline::new(self.ids.next_id(), Point::new(x, y), self.ids.next_id());
                if props.bool(prop::IS_LAYOUT_SIZE_SET_BY_USER) == Some(true)
                    && let Some(w) = props.f32(prop::LAYOUT_MAX_WIDTH)
                {
                    outline.width = OutlineWidth::Fixed(w * HALF_INCH);
                }
                outline.paragraphs = paras;
                outline.normalize();
                page.objects.push(PageObject::Outline(outline));
                self.report.keep("outlines");
            }
            jcid::IMAGE => self.report.lose("pictures"),
            jcid::EMBEDDED_FILE => self.report.lose("attachments"),
            jcid::TABLE => self.report.lose("tables"),
            _ => self.report.lose("other page objects"),
        }
    }

    /// The outline elements under `parent` (an outline or an outline element), flattened into
    /// paragraphs with levels.
    fn elements(&mut self, space: &ObjectSpace, parent: &PropertySet, level: usize, out: &mut Vec<Paragraph>) {
        if level >= self.depth {
            self.report.warnings.push("outline elements nested too deeply".into());
            return;
        }
        for id in parent.objects(prop::ELEMENT_CHILD_NODES) {
            let Some(element) = space.props(&id) else { continue };
            for content in element.objects(prop::CONTENT_CHILD_NODES) {
                let Some(object) = space.objects.get(&content) else { continue };
                match (object.jcid.0, space.props(&content)) {
                    (jcid::RICH_TEXT_OE, Some(rt)) => {
                        let mut p = Paragraph::with_text(self.ids.next_id(), &text_of(rt));
                        p.level = u8::try_from(level).unwrap_or(MAX_LEVEL).min(MAX_LEVEL);
                        out.push(p);
                        self.report.keep("paragraphs");
                    }
                    (jcid::IMAGE, _) => self.report.lose("pictures"),
                    (jcid::TABLE, _) => self.report.lose("tables"),
                    (jcid::EMBEDDED_FILE, _) => self.report.lose("attachments"),
                    _ => self.report.lose("other outline content"),
                }
            }
            self.elements(space, element, level + 1, out);
        }
    }
}

/// A rich text node's text: RichEditTextUnicode (§2.2.5) or TextExtendedAscii (§2.2.89).
fn text_of(rt: &PropertySet) -> String {
    if let Some(b) = rt.bytes(prop::RICH_EDIT_TEXT_UNICODE) {
        return wz(b);
    }
    rt.bytes(prop::TEXT_EXTENDED_ASCII).map(windows_1252).unwrap_or_default()
}

/// §2.2.9 WzInAtom: UTF-16LE up to the first NUL.
fn wz(bytes: &[u8]) -> String {
    let s = utf16_lossy(bytes);
    match s.find('\0') {
        Some(i) => s.get(..i).unwrap_or_default().to_string(),
        None => s,
    }
}

/// "Extended ASCII" (§2.2.89) read as Windows-1252, the code page of the Western editions.
fn windows_1252(bytes: &[u8]) -> String {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—',
        '˜', '™', 'š', '›', 'œ', '\u{9D}', 'ž', 'Ÿ',
    ];
    bytes.iter().map(|&b| if (0x80..0xA0).contains(&b) { HIGH.get(usize::from(b - 0x80)).copied().unwrap_or('?') } else { char::from(b) }).collect()
}

/// FILETIME (100 ns since 1601) → [`Timestamp`], clamped to the model's range.
fn filetime(ft: u64) -> Timestamp {
    Timestamp(i64::try_from(ft / 10_000).unwrap_or(i64::MAX).saturating_sub(FILETIME_TO_UNIX_MS)).clamped()
}

/// §2.3.1 Time32 (seconds since 1980) → [`Timestamp`].
fn time32(t: u64) -> Timestamp {
    Timestamp(i64::try_from(t).unwrap_or(0).saturating_mul(1000).saturating_add(TIME32_EPOCH_MS)).clamped()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_convert_from_both_epochs() {
        assert_eq!(time32(0), Timestamp(TIME32_EPOCH_MS));
        // 2000-01-01T00:00:00Z as FILETIME.
        assert_eq!(filetime(125_911_584_000_000_000), Timestamp(946_684_800_000));
        let _ = filetime(u64::MAX);
        let _ = time32(u64::MAX);
    }

    #[test]
    fn strings_decode() {
        assert_eq!(wz(&[b'a', 0, 0, 0, b'b', 0]), "a");
        assert_eq!(windows_1252(&[b'C', 0x80, 0xE9]), "C€é");
    }
}
