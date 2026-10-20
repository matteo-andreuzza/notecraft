//! Pictures and attached files on a page (decision D3, `docs/plan.md` §4.9).
//!
//! Both hold their bytes in a [`Blob`]: an id (the role [MS-ONESTORE]'s file-data-store GUID
//! plays in the reference) plus the bytes behind an `Arc`, so an undo snapshot, a copied
//! picture or a duplicated page shares them instead of copying megabytes.
//!
//! **Serialization.** A blob serializes as `{"id", "len"}` only: the bytes live beside the JSON
//! as zip entries `media/<id>` (see `notecraft-format`). The storage layer moves them across the
//! serde boundary with [`collect_blobs`] (every blob serialized inside the closure is handed
//! back for writing) and [`provide_blobs`] (blobs deserialized inside the closure take their
//! bytes from the given map). Outside those closures a blob deserializes with empty bytes — a
//! picture that draws as a placeholder, never an error — which is also what a missing or
//! mismatched zip entry gives.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use notecraft_geom::Point;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::id::Id;
use crate::text::sanitize_text;

/// Largest picture file accepted (D3).
pub const MAX_PICTURE_BYTES: usize = 64 << 20;
/// Largest attached file accepted (D3; unverified: the reference's own cap is undocumented).
pub const MAX_ATTACHMENT_BYTES: usize = 256 << 20;
/// Most objects on one page (D3); loaders drop the rest, insert commands refuse.
pub const MAX_PAGE_OBJECTS: usize = 10_000;
/// Smallest drawn side of a picture, points.
pub const MIN_PICTURE_SIZE: f32 = 4.0;
/// Largest drawn side of a picture, points.
pub const MAX_PICTURE_SIZE: f32 = 16_000.0;
/// Longest alt text kept, in characters.
pub const MAX_ALT_TEXT: usize = 4_096;
/// Longest attachment name, in bytes.
pub const MAX_NAME_BYTES: usize = 255;
/// The drawn size of an attachment's icon card (icon above a two-line name), points.
pub const ATTACHMENT_SIZE: (f32, f32) = (88.0, 66.0);
/// Pixels per point of a picture inserted at its natural size (96 dpi: 1 px = 0.75 pt).
pub const PT_PER_PX: f32 = 0.75;
/// Widest a picture is inserted (unverified: 6.5 in, the auto-width of a note container).
pub const INSERT_MAX_WIDTH: f32 = 468.0;

/// Stored bytes with a stable id (see the module docs).
#[derive(Clone)]
pub struct Blob {
    /// The id; keys the `media/<id>` zip entry.
    pub id: Id,
    /// The length the file declared (equals `bytes.len()` unless the bytes were lost on load).
    pub len: u64,
    /// The bytes (empty when they could not be loaded).
    pub bytes: Arc<[u8]>,
}

impl Blob {
    /// A blob holding `bytes`.
    pub fn new(id: Id, bytes: Arc<[u8]>) -> Blob {
        Blob { id, len: bytes.len() as u64, bytes }
    }

    /// Whether the bytes are missing (a damaged or truncated file).
    pub fn is_missing(&self) -> bool {
        self.bytes.is_empty() && self.len > 0
    }
}

impl PartialEq for Blob {
    fn eq(&self, other: &Blob) -> bool {
        self.id == other.id && self.len == other.len && (Arc::ptr_eq(&self.bytes, &other.bytes) || self.bytes == other.bytes)
    }
}

impl std::fmt::Debug for Blob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Blob({}, {} bytes{})", self.id, self.len, if self.is_missing() { ", missing" } else { "" })
    }
}

#[derive(Serialize, Deserialize)]
struct BlobRef {
    id: Id,
    #[serde(default)]
    len: u64,
}

thread_local! {
    static SINK: RefCell<Option<BTreeMap<Id, Arc<[u8]>>>> = const { RefCell::new(None) };
    static SOURCE: RefCell<Option<HashMap<Id, Arc<[u8]>>>> = const { RefCell::new(None) };
}

impl Serialize for Blob {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if !self.bytes.is_empty() {
            SINK.with(|k| {
                if let Some(m) = k.borrow_mut().as_mut() {
                    // One entry per id; the first blob wins if a hostile file reused an id.
                    m.entry(self.id).or_insert_with(|| self.bytes.clone());
                }
            });
        }
        BlobRef { id: self.id, len: self.len }.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Blob {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Blob, D::Error> {
        let r = BlobRef::deserialize(d)?;
        // Bytes whose length disagrees with the JSON are another blob's or truncated: dropped.
        let bytes = SOURCE.with(|k| k.borrow().as_ref().and_then(|m| m.get(&r.id)).filter(|b| b.len() as u64 == r.len).cloned());
        Ok(Blob { id: r.id, len: r.len, bytes: bytes.unwrap_or_else(|| Arc::from(&[][..])) })
    }
}

/// Restores the previous thread-local state when dropped (also on unwind).
struct Restore<T: 'static>(&'static std::thread::LocalKey<RefCell<Option<T>>>, Option<T>);

impl<T: 'static> Drop for Restore<T> {
    fn drop(&mut self) {
        let prev = self.1.take();
        self.0.with(|k| *k.borrow_mut() = prev);
    }
}

/// Run `f` (a serialization), returning its result and every non-empty blob serialized inside
/// it, one per id, in id order.
pub fn collect_blobs<T>(f: impl FnOnce() -> T) -> (T, Vec<(Id, Arc<[u8]>)>) {
    let prev = SINK.with(|k| k.borrow_mut().replace(BTreeMap::new()));
    let guard = Restore(&SINK, prev);
    let out = f();
    let blobs = SINK.with(|k| k.borrow_mut().take()).unwrap_or_default();
    drop(guard);
    (out, blobs.into_iter().collect())
}

/// Run `f` (a deserialization) with blobs taking their bytes from `blobs`.
pub fn provide_blobs<T>(blobs: HashMap<Id, Arc<[u8]>>, f: impl FnOnce() -> T) -> T {
    let prev = SOURCE.with(|k| k.borrow_mut().replace(blobs));
    let _guard = Restore(&SOURCE, prev);
    f()
}

/// `v` clamped to the drawable range; `fallback` when it is not a positive finite number.
fn clamp_size(v: f32, fallback: f32) -> f32 {
    if v.is_finite() && v > 0.0 { v.clamp(MIN_PICTURE_SIZE, MAX_PICTURE_SIZE) } else { fallback.clamp(MIN_PICTURE_SIZE, MAX_PICTURE_SIZE) }
}

fn clean_alt(s: &mut String) {
    *s = sanitize_text(s);
    if let Some((cut, _)) = s.char_indices().nth(MAX_ALT_TEXT) {
        s.truncate(cut);
    }
}

/// A picture on the page ([MS-ONE] `jcidImageNode`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Image {
    /// Stable id.
    pub id: Id,
    /// Top-left corner in page space.
    pub pos: Point,
    /// Drawn width in points (after rotation).
    pub width: f32,
    /// Drawn height in points (after rotation).
    pub height: f32,
    /// The picture file (PNG, JPEG, GIF, BMP or WebP).
    pub blob: Blob,
    /// The decoded size of the stored picture, pixels (0 when unknown); Restore to Original
    /// Size goes back to it.
    #[serde(default)]
    pub pixels: (u32, u32),
    /// Clockwise quarter turns applied when drawing (0–3).
    #[serde(default)]
    pub quarter_turns: u8,
    /// Mirrored left to right (applied before the turns).
    #[serde(default)]
    pub flip_h: bool,
    /// Mirrored top to bottom (applied before the turns).
    #[serde(default)]
    pub flip_v: bool,
    /// Alternative text for screen readers (Alt Text).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub alt_text: String,
    /// Set as the page background: behind everything, not selected by a left click.
    #[serde(default)]
    pub background: bool,
    /// A page of a file printout: the attachment holding the printed file (its [MS-ONE]
    /// counterpart is the picture's link to the embedded file; a printout is the attachment
    /// plus one picture per page, decision D3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub printout: Option<Id>,
    /// A hyperlink on the picture (Link… on its menu; [MS-ONE] `WzHyperlinkUrl`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

impl Image {
    /// A picture at `pos` drawn `width × height` points.
    pub fn new(id: Id, pos: Point, blob: Blob, pixels: (u32, u32), width: f32, height: f32) -> Image {
        let mut im = Image {
            id,
            pos,
            width,
            height,
            blob,
            pixels,
            quarter_turns: 0,
            flip_h: false,
            flip_v: false,
            alt_text: String::new(),
            background: false,
            printout: None,
            link: None,
        };
        im.normalize();
        im
    }

    /// The natural drawn size, points: the pixel size at 96 dpi, scaled down to at most
    /// [`INSERT_MAX_WIDTH`] wide, sides swapped by an odd number of quarter turns.
    pub fn natural_size(pixels: (u32, u32), quarter_turns: u8) -> (f32, f32) {
        let (pw, ph) = (pixels.0.max(1) as f32 * PT_PER_PX, pixels.1.max(1) as f32 * PT_PER_PX);
        let k = if pw > INSERT_MAX_WIDTH { INSERT_MAX_WIDTH / pw } else { 1.0 };
        let (w, h) = (clamp_size(pw * k, 96.0), clamp_size(ph * k, 96.0));
        if quarter_turns % 2 == 1 { (h, w) } else { (w, h) }
    }

    /// Enforce the invariants (hostile files): finite position, sizes in range, turns 0–3,
    /// bounded alt text.
    pub fn normalize(&mut self) {
        self.pos = self.pos.sanitized();
        let (nw, nh) = Image::natural_size(self.pixels, self.quarter_turns);
        self.width = clamp_size(self.width, nw);
        self.height = clamp_size(self.height, nh);
        self.quarter_turns %= 4;
        self.pixels = (self.pixels.0.min(1 << 20), self.pixels.1.min(1 << 20));
        clean_alt(&mut self.alt_text);
        clean_link(&mut self.link);
    }
}

/// A file attached to the page ([MS-ONE] `jcidEmbeddedFileNode`), drawn as an icon with its
/// name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    /// Stable id.
    pub id: Id,
    /// Top-left corner in page space.
    pub pos: Point,
    /// The file name shown (and suggested by Save As): one safe path component.
    pub name: String,
    /// The file's bytes.
    pub blob: Blob,
    /// Alternative text for screen readers.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub alt_text: String,
    /// A hyperlink on the file's icon (Link… on its menu).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

/// Longest stored hyperlink on an object, bytes.
pub const MAX_LINK: usize = 4_096;

/// A stored object link from a file: control characters removed, at most [`MAX_LINK`] bytes
/// (cut at a character boundary), empty → none.
pub fn clean_link(link: &mut Option<String>) {
    if let Some(l) = link.as_mut() {
        l.retain(|c| !c.is_control());
        if l.len() > MAX_LINK {
            let mut cut = MAX_LINK;
            while !l.is_char_boundary(cut) {
                cut -= 1;
            }
            l.truncate(cut);
        }
    }
    if link.as_deref().is_some_and(|l| l.trim().is_empty()) {
        *link = None;
    }
}

impl Attachment {
    /// Enforce the invariants: finite position, a safe non-empty name, bounded alt text.
    pub fn normalize(&mut self) {
        self.pos = self.pos.sanitized();
        self.name = safe_file_name(&self.name);
        clean_alt(&mut self.alt_text);
        clean_link(&mut self.link);
    }
}

/// `name` as one harmless file-name component: the last path segment, control characters and
/// reserved characters replaced by `_`, leading dots trimmed (no hidden files, no `..`), at
/// most [`MAX_NAME_BYTES`] bytes, never empty.
pub fn safe_file_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or("");
    let mut s: String = last.chars().map(|c| if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect();
    s = s.trim().trim_start_matches('.').to_string();
    while s.len() > MAX_NAME_BYTES {
        s.pop();
    }
    if s.is_empty() { "Attachment".to_string() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(n: u128, bytes: &[u8]) -> Blob {
        Blob::new(Id(n), Arc::from(bytes))
    }

    #[test]
    fn blobs_cross_serde_only_inside_the_closures() {
        let im = Image::new(Id(1), Point::new(10.0, 20.0), blob(7, b"png bytes"), (40, 20), 30.0, 15.0);
        let (json, blobs) = collect_blobs(|| serde_json::to_string(&im).unwrap());
        assert!(json.contains(r#""blob":{"id":"00000000000000000000000000000007","len":9}"#), "{json}");
        assert_eq!(blobs.len(), 1);
        assert_eq!(&*blobs[0].1, b"png bytes");
        // Without a source the bytes are missing, not an error.
        let bare: Image = serde_json::from_str(&json).unwrap();
        assert!(bare.blob.is_missing());
        let back: Image = provide_blobs(blobs.into_iter().collect(), || serde_json::from_str(&json).unwrap());
        assert_eq!(back, im);
        // A length mismatch (another blob's or truncated bytes) leaves it missing.
        let wrong: Image = provide_blobs([(Id(7), Arc::from(&b"short"[..]))].into_iter().collect(), || serde_json::from_str(&json).unwrap());
        assert!(wrong.blob.is_missing());
        // The thread-locals are restored afterwards.
        let (_, none) = collect_blobs(|| ());
        assert!(none.is_empty());
        assert!(serde_json::from_str::<Image>(&json).unwrap().blob.is_missing());
    }

    #[test]
    fn hostile_pictures_and_names_are_repaired() {
        let mut im = Image::new(Id(1), Point::new(f32::NAN, 1e30), blob(2, b"x"), (u32::MAX, 3), f32::INFINITY, -5.0);
        im.quarter_turns = 9;
        im.alt_text = "a\u{0}b".repeat(5_000);
        im.normalize();
        assert!(im.pos.x.is_finite() && im.pos.y.is_finite());
        assert!((MIN_PICTURE_SIZE..=MAX_PICTURE_SIZE).contains(&im.width) && (MIN_PICTURE_SIZE..=MAX_PICTURE_SIZE).contains(&im.height));
        assert_eq!(im.quarter_turns, 1);
        assert!(im.alt_text.chars().count() <= MAX_ALT_TEXT && !im.alt_text.contains('\u{0}'));
        assert_eq!(safe_file_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_file_name("C:\\Users\\me\\report?.pdf"), "report_.pdf");
        assert_eq!(safe_file_name(".."), "Attachment");
        assert_eq!(safe_file_name(".bashrc"), "bashrc");
        assert!(safe_file_name(&"é".repeat(300)).len() <= MAX_NAME_BYTES);
    }

    #[test]
    fn natural_size_is_96_dpi_capped_and_turned() {
        assert_eq!(Image::natural_size((200, 100), 0), (150.0, 75.0));
        assert_eq!(Image::natural_size((200, 100), 1), (75.0, 150.0));
        let (w, h) = Image::natural_size((2000, 1000), 0);
        assert_eq!((w, h), (INSERT_MAX_WIDTH, INSERT_MAX_WIDTH / 2.0));
    }
}
