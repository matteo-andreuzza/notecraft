//! The walk from a resolved [`Store`] to a NoteCraft [`Section`], with a per-construct report of
//! what was kept and what was left out (the fidelity numbers of `docs/parity.md`).

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use notecraft_codecs::{Limits as CodecLimits, probe};
use notecraft_color::Rgba;
use notecraft_doc::id::IdGen;
use notecraft_doc::media::{Attachment, Blob, Image, MAX_ATTACHMENT_BYTES, MAX_PAGE_OBJECTS, MAX_PICTURE_BYTES};
use notecraft_doc::notebook::{Section, SectionColor};
use notecraft_doc::page::{MAX_PAGE_LEVEL, Outline, OutlineWidth, Page, PageObject};
use notecraft_doc::table::Table;
use notecraft_doc::tags::{TagSymbol, TagUse, builtin_tags};
use notecraft_doc::text::{Align, Bullet, CharFormat, ListFormat, MAX_LEVEL, Numbering, ParaStyle, Paragraph, Run, Script};
use notecraft_doc::time::Timestamp;
use notecraft_geom::Point;

use super::{jcid, prop};
use crate::Limits;
use crate::bytes::utf16_lossy;
use crate::error::{OneError, Result};
use crate::header::FileKind;
use crate::propset::{PropertySet, Value};
use crate::revision::{ObjectData, ObjectSpace, ROOT_CONTENT, ROOT_METADATA, Store};
use crate::types::ExtendedGuid;

/// [MS-ONE] lengths are in half inches (§2.3.18): 36 points.
const HALF_INCH: f32 = 36.0;
/// The height given to each paragraph when placing content found inside an outline below it.
pub(crate) const LINE: f32 = 20.0;
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
    let mut w = Walk { store: &store, ids, report: Report::default(), now, depth: limits.max_depth, extras: Vec::new() };
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
    /// Pictures, tables and files met inside the outline being walked, with the number of
    /// paragraphs before them: NoteCraft keeps them as page objects beside the outline.
    extras: Vec<(usize, PageObject)>,
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
        self.extras.clear();
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
        let at = |p: &PropertySet| {
            Point::new(
                p.f32(prop::OFFSET_FROM_PARENT_HORIZ).unwrap_or(0.0) * HALF_INCH,
                p.f32(prop::OFFSET_FROM_PARENT_VERT).unwrap_or(0.0) * HALF_INCH,
            )
        };
        match object.jcid.0 {
            jcid::OUTLINE => {
                let mut paras = Vec::new();
                self.extras.clear();
                self.elements(space, props, 0, &mut paras);
                let pos = at(props);
                let extras = std::mem::take(&mut self.extras);
                if !paras.is_empty() {
                    let mut outline = Outline::new(self.ids.next_id(), pos, self.ids.next_id());
                    if props.bool(prop::IS_LAYOUT_SIZE_SET_BY_USER) == Some(true)
                        && let Some(w) = props.f32(prop::LAYOUT_MAX_WIDTH)
                    {
                        outline.width = OutlineWidth::Fixed(w * HALF_INCH);
                    }
                    outline.paragraphs = paras;
                    outline.normalize();
                    self.push(page, PageObject::Outline(outline));
                    self.report.keep("outlines");
                }
                // Content inside the outline goes below the paragraphs before it (an estimate
                // of their height: NoteCraft has no objects inside outlines).
                for (before, mut object) in extras {
                    object.set_pos(Point::new(pos.x, pos.y + before as f32 * LINE));
                    self.push(page, object);
                }
            }
            jcid::IMAGE => {
                if let Some(o) = self.image(space, props, at(props)) {
                    self.push(page, o);
                }
            }
            jcid::EMBEDDED_FILE => {
                if let Some(o) = self.attachment(space, props, at(props)) {
                    self.push(page, o);
                }
            }
            jcid::TABLE => {
                if let Some(o) = self.table(space, props, at(props)) {
                    self.push(page, o);
                }
            }
            _ => self.report.lose("other page objects (ink, audio…)"),
        }
    }

    /// Adds an object unless the page is full ([`MAX_PAGE_OBJECTS`]).
    fn push(&mut self, page: &mut Page, object: PageObject) {
        if page.objects.len() >= MAX_PAGE_OBJECTS {
            self.report.lose("objects over the page limit");
            return;
        }
        page.objects.push(object);
    }

    /// §2.2.24 jcidImageNode: the picture's bytes (PictureContainer → jcidPictureContainer14),
    /// its size (PictureWidth/Height, else LayoutMaxWidth/Height, in half inches; else 96 dpi),
    /// alt text and link.
    fn image(&mut self, space: &ObjectSpace, props: &PropertySet, pos: Point) -> Option<PageObject> {
        let Some(bytes) = file_data(self.store, space, props, prop::PICTURE_CONTAINER) else {
            self.report.lose("pictures whose data is missing");
            return None;
        };
        if bytes.len() > MAX_PICTURE_BYTES {
            self.report.lose("pictures over the size limit");
            return None;
        }
        let Ok(info) = probe(bytes, &CodecLimits::PICTURE) else {
            self.report.lose("pictures in formats NoteCraft can't show (EMF, WMF, TIFF…)");
            return None;
        };
        let pixels = (info.width, info.height);
        let width =
            props.f32(prop::PICTURE_WIDTH).or_else(|| props.f32(prop::LAYOUT_MAX_WIDTH)).map(|w| w * HALF_INCH).unwrap_or(pixels.0 as f32 * 0.75);
        let height =
            props.f32(prop::PICTURE_HEIGHT).or_else(|| props.f32(prop::LAYOUT_MAX_HEIGHT)).map(|h| h * HALF_INCH).unwrap_or(pixels.1 as f32 * 0.75);
        let blob = Blob::new(self.ids.next_id(), Arc::from(bytes));
        let mut image = Image::new(self.ids.next_id(), pos, blob, pixels, width, height);
        image.alt_text = props.bytes(prop::IMAGE_ALT_TEXT).map(wz).unwrap_or_default();
        image.link = props.bytes(prop::WZ_HYPERLINK_URL).map(wz);
        image.normalize();
        self.report.keep("pictures");
        Some(PageObject::Image(image))
    }

    /// §2.2.32 jcidEmbeddedFileNode: the file (EmbeddedFileContainer → §2.2.33) and its name.
    fn attachment(&mut self, space: &ObjectSpace, props: &PropertySet, pos: Point) -> Option<PageObject> {
        let Some(bytes) = file_data(self.store, space, props, prop::EMBEDDED_FILE_CONTAINER) else {
            // `<file>` references name a file beside the section, which an import can't reach.
            self.report.lose("attachments whose data is outside the section file");
            return None;
        };
        if bytes.len() > MAX_ATTACHMENT_BYTES {
            self.report.lose("attachments over the size limit");
            return None;
        }
        let name = props.bytes(prop::EMBEDDED_FILE_NAME).map(wz).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "attachment".into());
        let mut a = Attachment {
            id: self.ids.next_id(),
            pos,
            name,
            blob: Blob::new(self.ids.next_id(), Arc::from(bytes)),
            alt_text: String::new(),
            link: None,
        };
        a.normalize();
        self.report.keep("attachments");
        Some(PageObject::Attachment(a))
    }

    /// §2.2.26 jcidTableNode → rows (§2.2.27) → cells (§2.2.28) → outline elements; column
    /// widths from TableColumnWidths (§2.2.66: a count byte, then half-inch floats); cell
    /// backgrounds from CellShadingColor (§2.2.97). Merged cells are not in the public
    /// specification.
    fn table(&mut self, space: &ObjectSpace, props: &PropertySet, pos: Point) -> Option<PageObject> {
        let rows: Vec<Vec<(Vec<Paragraph>, Option<Rgba>)>> = props
            .objects(prop::ELEMENT_CHILD_NODES)
            .iter()
            .filter_map(|r| space.props(r))
            .map(|row| row.objects(prop::ELEMENT_CHILD_NODES).iter().filter_map(|c| space.props(c)).collect::<Vec<_>>())
            .collect::<Vec<_>>()
            .into_iter()
            .map(|cells| {
                cells
                    .into_iter()
                    .map(|cell| {
                        let mut paras = Vec::new();
                        self.elements(space, cell, 0, &mut paras);
                        (paras, cell.uint(prop::CELL_SHADING_COLOR).and_then(colorref))
                    })
                    .collect()
            })
            .collect();
        let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
        if rows.is_empty() || cols == 0 {
            self.report.lose("empty tables");
            return None;
        }
        let ids = &mut *self.ids;
        let mut table = Table::new(ids.next_id(), pos, rows.len(), cols, &mut || ids.next_id());
        for (r, row) in rows.into_iter().enumerate() {
            for (c, (paras, shading)) in row.into_iter().enumerate() {
                let Some(cell) = table.rows.get_mut(r).and_then(|row| row.cells.get_mut(c)) else { continue };
                if !paras.is_empty() {
                    cell.paragraphs = paras;
                }
                if shading.is_some() {
                    cell.shading = shading;
                    self.report.keep("shaded cells");
                }
            }
        }
        if let Some(b) = props.bytes(prop::TABLE_COLUMN_WIDTHS) {
            let n = usize::from(b.first().copied().unwrap_or(0));
            let widths: Vec<f32> = b
                .get(1..)
                .unwrap_or_default()
                .as_chunks::<4>()
                .0
                .iter()
                .take(n)
                .map(|c| f32::from_le_bytes(*c) * HALF_INCH)
                .filter(|w| w.is_finite())
                .collect();
            for (col, w) in table.columns.iter_mut().zip(widths) {
                *col = w;
            }
        }
        if let Some(v) = props.bool(prop::TABLE_BORDERS_VISIBLE) {
            table.borders = v;
        }
        // Pictures inside cells have no place in a NoteCraft cell.
        self.extras.retain(|_| false);
        table.normalize();
        self.report.keep("tables");
        Some(PageObject::Table(table))
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
                        let mut p = self.paragraph(space, rt);
                        p.level = u8::try_from(level).unwrap_or(MAX_LEVEL).min(MAX_LEVEL);
                        p.list = element.objects(prop::LIST_NODES).first().and_then(|l| space.props(l)).and_then(list_format);
                        if p.list.is_some() {
                            self.report.keep("list items");
                        }
                        p.tags = self.tags(space, rt);
                        out.push(p);
                        self.report.keep("paragraphs");
                    }
                    (jcid::IMAGE, Some(p)) => {
                        if let Some(o) = self.image(space, p, Point::new(0.0, 0.0)) {
                            self.extras.push((out.len(), o));
                        }
                    }
                    (jcid::TABLE, Some(p)) => {
                        let saved = std::mem::take(&mut self.extras);
                        let table = self.table(space, p, Point::new(0.0, 0.0));
                        self.extras = saved;
                        if let Some(o) = table {
                            self.extras.push((out.len(), o));
                        }
                    }
                    (jcid::EMBEDDED_FILE, Some(p)) => {
                        if let Some(o) = self.attachment(space, p, Point::new(0.0, 0.0)) {
                            self.extras.push((out.len(), o));
                        }
                    }
                    _ => self.report.lose("other outline content (ink, audio…)"),
                }
            }
            self.elements(space, element, level + 1, out);
        }
    }
}

impl Walk<'_, '_> {
    /// A rich text node (§2.2.23) as a paragraph: its text cut into runs at TextRunIndex
    /// (§2.2.76, positions in UTF-16 code units) with the formats of TextRunFormatting
    /// (§2.2.77), and the paragraph style (§2.2.80) and alignment (§2.3.94).
    fn paragraph(&mut self, space: &ObjectSpace, rt: &PropertySet) -> Paragraph {
        let mut p = Paragraph::new(self.ids.next_id());
        let units: Vec<u16> = text_of(rt).encode_utf16().collect();
        let formats = rt.objects(prop::TEXT_RUN_FORMATTING);
        let ends: Vec<usize> = rt
            .bytes(prop::TEXT_RUN_INDEX)
            .map(|b| b.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c) as usize).collect())
            .unwrap_or_default();
        let mut start = 0;
        for (i, fid) in formats.iter().enumerate() {
            // The last run ends at the end of the text; ends are clamped to stay increasing.
            let end = ends.get(i).copied().unwrap_or(units.len()).clamp(start, units.len());
            let piece = String::from_utf16_lossy(units.get(start..end).unwrap_or_default());
            start = end;
            let style = space.props(fid);
            if style.and_then(|s| s.bool(prop::HIDDEN)) == Some(true) {
                self.report.lose("hidden text runs");
                continue;
            }
            if style.and_then(|s| s.bool(prop::HYPERLINK)) == Some(true) {
                // The link target of a text run is not in the public specification.
                self.report.lose("text hyperlinks (text kept)");
            }
            if !piece.is_empty() {
                p.runs.push(Run::new(&piece, style.map(char_format).unwrap_or_default()));
            }
            if end == units.len() {
                break;
            }
        }
        if start < units.len() {
            // Text past the last formatted run (or no TextRunFormatting at all) is plain.
            p.runs.push(Run::new(&String::from_utf16_lossy(units.get(start..).unwrap_or_default()), CharFormat::default()));
        }
        let style = rt.objects(prop::PARAGRAPH_STYLE).first().and_then(|s| space.props(s)).cloned();
        if let Some(id) = style.as_ref().and_then(|s| s.bytes(prop::PARAGRAPH_STYLE_ID)).map(wz) {
            p.style = para_style(&id);
        }
        let align = rt.uint(prop::PARAGRAPH_ALIGNMENT).or_else(|| style.as_ref().and_then(|s| s.uint(prop::PARAGRAPH_ALIGNMENT)));
        p.align = match align {
            Some(1) => Align::Center,
            Some(2) => Align::Right,
            _ => Align::Left,
        };
        p.normalize();
        if p.runs.iter().any(|r| r.fmt != CharFormat::default()) {
            self.report.keep("formatted paragraphs");
        }
        p
    }
}

impl Walk<'_, '_> {
    /// §2.2.88 NoteTagStates: one NoteTagState (§2.2.42) per tag, each naming its shared
    /// definition (§2.2.41: label, shape) and its status (§2.3.91: bit 0 Completed).
    fn tags(&mut self, space: &ObjectSpace, rt: &PropertySet) -> Vec<TagUse> {
        let Some(Value::Array(states)) = rt.get(prop::NOTE_TAG_STATES) else { return Vec::new() };
        let builtins = builtin_tags();
        let mut out: Vec<TagUse> = Vec::new();
        for state in states {
            let Some(def) = state.objects(prop::NOTE_TAG_DEFINITION_OID).first().and_then(|d| space.props(d)) else {
                self.report.warnings.push("a note tag names no definition".into());
                continue;
            };
            let label = def.bytes(prop::NOTE_TAG_LABEL).map(wz).unwrap_or_default();
            let shape = def.uint(prop::NOTE_TAG_SHAPE).or_else(|| state.uint(prop::NOTE_TAG_SHAPE)).unwrap_or(0);
            let checkable = is_checkable(shape);
            let completed = state.uint(prop::ACTION_ITEM_STATUS).is_some_and(|s| s & 1 != 0);
            // A built-in tag with the same name keeps NoteCraft's key and icon; others get a
            // key from their label and the nearest icon for their shape.
            let tag = match builtins.iter().find(|b| b.name.eq_ignore_ascii_case(label.trim())) {
                Some(b) => TagUse { key: b.key.clone(), name: b.name.clone(), symbol: b.symbol, checked: false },
                None => TagUse {
                    key: slug(&label, shape),
                    name: if label.is_empty() { "Tag".into() } else { label },
                    symbol: symbol(shape),
                    checked: false,
                },
            };
            let tag = TagUse { checked: checkable && completed, ..tag };
            if out.iter().any(|t| t.key == tag.key) {
                continue;
            }
            out.push(tag);
            self.report.keep("note tags");
        }
        out
    }
}

/// §2.3.86: the shapes whose icon is a check box.
fn is_checkable(shape: u64) -> bool {
    matches!(shape, 1..=12 | 28 | 30 | 32 | 48 | 50 | 52 | 69 | 71 | 73 | 89..=99)
}

/// The NoteCraft icon nearest to a §2.3.86 shape (colours are not kept).
fn symbol(shape: u64) -> TagSymbol {
    use TagSymbol as S;
    match shape {
        28 | 48 | 69 => S::CheckboxOne,
        30 | 50 | 71 => S::CheckboxTwo,
        94..=96 => S::CheckboxPerson,
        89..=93 | 97..=99 => S::CheckboxFlag,
        1..=12 | 32 | 52 | 73 => S::Checkbox,
        13 | 34 | 40 | 54 | 61 | 75 => S::Star,
        15 | 111 => S::Question,
        17 => S::Exclamation,
        18 | 114 | 118 => S::Person,
        21 => S::Bulb,
        23 => S::House,
        27 => S::Key,
        109 | 110 => S::Phone,
        124 | 125 => S::Globe,
        106..=108 => S::Envelope,
        121 => S::Music,
        122 => S::Film,
        14 => S::FlagA,
        _ => S::None,
    }
}

/// A key for a tag that is not built in: its label in lowercase words joined by dashes.
fn slug(label: &str, shape: u64) -> String {
    let s: String = label.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|w| !w.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { format!("onenote-tag-{shape}") } else { format!("onenote-{s}") }
}

/// §2.2.25 jcidNumberListNode → a list format. §2.3.20 NumberListFormat: UTF-16 units, the
/// first is the length; 0xFFFD followed by an MSONFC ([MS-OSHARED] §2.2.1.3) is the number.
fn list_format(node: &PropertySet) -> Option<ListFormat> {
    let units: Vec<u16> = node.bytes(prop::NUMBER_LIST_FORMAT)?.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
    let len = usize::from(*units.first()?);
    let chars = units.get(1..).unwrap_or_default();
    let chars = chars.get(..len.min(chars.len())).unwrap_or_default();
    if let Some(at) = chars.iter().position(|&u| u == 0xFFFD) {
        let nfc = chars.get(at + 1).copied().unwrap_or(0);
        let paren = chars.get(at + 2) == Some(&u16::from(b')'));
        let style = match (nfc, paren) {
            (1, _) => Numbering::UpperRoman,
            (2, _) => Numbering::LowerRoman,
            (3, _) => Numbering::UpperAlpha,
            (4, true) => Numbering::LowerAlphaParen,
            (4, false) => Numbering::LowerAlpha,
            (_, true) => Numbering::DecimalParen,
            _ => Numbering::Decimal,
        };
        let restart = node.uint(prop::LIST_RESTART).and_then(|r| u32::try_from(r).ok()).filter(|r| *r > 0);
        return Some(ListFormat::Number { style, restart });
    }
    let bullet = match char::decode_utf16(chars.iter().copied()).next().and_then(|r| r.ok()) {
        Some('o' | '◦' | '○') => Bullet::Circle,
        Some('▪' | '■' | '§' | '□') => Bullet::Square,
        Some('-' | '–' | '—') => Bullet::Dash,
        Some('➢' | '➤' | '→' | 'Ø' | '>') => Bullet::Arrow,
        Some('◆' | '♦' | 'v') => Bullet::Diamond,
        _ => Bullet::Disc,
    };
    Some(ListFormat::Bullet(bullet))
}

/// §2.2.43 jcidParagraphStyleObjectForText → a character format (unset properties stay unset).
fn char_format(s: &PropertySet) -> CharFormat {
    let script = match (s.bool(prop::SUPERSCRIPT), s.bool(prop::SUBSCRIPT)) {
        (Some(true), _) => Some(Script::Super),
        (_, Some(true)) => Some(Script::Sub),
        _ => None,
    };
    CharFormat {
        font: s.bytes(prop::FONT).map(wz).filter(|f| !f.is_empty()),
        // §2.3.16: half points, 12–288.
        size: s.uint(prop::FONT_SIZE).map(|h| (h.clamp(2, 1000) as f32) / 2.0),
        bold: s.bool(prop::BOLD),
        italic: s.bool(prop::ITALIC),
        underline: s.bool(prop::UNDERLINE),
        strike: s.bool(prop::STRIKETHROUGH),
        script,
        color: s.uint(prop::FONT_COLOR).and_then(colorref),
        highlight: s.uint(prop::HIGHLIGHT).and_then(colorref),
        link: None,
    }
}

/// §2.2.8 COLORREF: 0x00BBGGRR, or 0xFF000000 for "automatic" (`None`).
fn colorref(v: u64) -> Option<Rgba> {
    if v > 0x00FF_FFFF {
        return None;
    }
    let [r, g, b, _] = (v as u32).to_le_bytes();
    Some(Rgba::rgb(r, g, b))
}

/// §2.2.83 ParagraphStyleId → a NoteCraft style. The spec's examples show "p" and
/// "PageTitle"; the heading, citation, quote and code names are the HTML-like ones the
/// reference is believed to use (unverified, `docs/parity.md`).
fn para_style(id: &str) -> ParaStyle {
    match id {
        "h1" => ParaStyle::Heading1,
        "h2" => ParaStyle::Heading2,
        "h3" => ParaStyle::Heading3,
        "h4" => ParaStyle::Heading4,
        "h5" => ParaStyle::Heading5,
        "h6" => ParaStyle::Heading6,
        "PageTitle" => ParaStyle::PageTitle,
        "cite" => ParaStyle::Citation,
        "blockquote" => ParaStyle::Quote,
        "code" => ParaStyle::Code,
        _ => ParaStyle::Normal,
    }
}

/// The bytes of the file data object that property `id` of `holder` names.
fn file_data<'a>(store: &Store<'a>, space: &ObjectSpace, holder: &PropertySet, id: u32) -> Option<&'a [u8]> {
    let oid = holder.objects(id).into_iter().next()?;
    match &space.objects.get(&oid)?.data {
        ObjectData::File { reference, .. } => store.file_bytes(reference),
        _ => None,
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
