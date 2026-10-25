//! Pictures and file attachments on the canvas (decision D3, `docs/plan.md` §4.9): where they
//! are on screen, drawing them (rotation and flips applied through texture coordinates, the
//! stored bytes never change), the selection frame with its eight sizing handles, the bounded
//! texture cache, and the commands that need the host first (file dialogs, opening a file).
//!
//! **Texture cache.** A picture is decoded by `notecraft-codecs` the first time it is drawn,
//! shrunk so neither side exceeds [`MAX_TEXTURE_SIDE`] pixels, and uploaded once, keyed by its
//! blob id (a copied picture shares the texture). The cache holds at most [`CACHE_BYTES`] of
//! pixels and drops the least recently drawn textures beyond that. A picture that can't be
//! decoded (missing bytes after a damaged file, an unsupported format) is remembered as failed
//! and drawn as a placeholder with the reason in its tooltip — never retried every frame.

use std::collections::HashMap;

use egui::{Color32, FontId, Pos2, Rect, Stroke, TextureHandle, pos2, vec2};
use notecraft_engine::doc::geom::Point;
use notecraft_engine::doc::media::ATTACHMENT_SIZE;
use notecraft_engine::doc::{Blob, Id, Image, Page, PageObject};
use serde_json::json;

use crate::canvas::PT;
use crate::{NoteApp, Tokens};

/// Longest side of an uploaded texture, pixels.
pub const MAX_TEXTURE_SIDE: u32 = 4096;
/// Most pixel bytes the texture cache keeps.
pub const CACHE_BYTES: usize = 256 << 20;
/// Side of a sizing handle, screen points.
const HANDLE: f32 = 8.0;

enum Entry {
    Ready { tex: TextureHandle, bytes: usize, used: u64 },
    Failed(String),
}

/// Decoded pictures as GPU textures, keyed by blob id (see the module docs).
#[derive(Default)]
pub struct TextureCache {
    entries: HashMap<Id, Entry>,
    tick: u64,
    bytes: usize,
}

impl TextureCache {
    /// The texture for `blob`, decoding and uploading it on first use; the reason it can't be
    /// shown otherwise.
    pub fn get(&mut self, ctx: &egui::Context, blob: &Blob) -> Result<TextureHandle, String> {
        self.tick += 1;
        let tick = self.tick;
        if let Some(e) = self.entries.get_mut(&blob.id) {
            return match e {
                Entry::Ready { tex, used, .. } => {
                    *used = tick;
                    Ok(tex.clone())
                }
                Entry::Failed(why) => Err(why.clone()),
            };
        }
        if blob.bytes.is_empty() {
            let why = "The picture's data is missing (the notebook file was damaged).".to_string();
            self.entries.insert(blob.id, Entry::Failed(why.clone()));
            return Err(why);
        }
        match notecraft_codecs::decode_fit(&blob.bytes, &notecraft_codecs::Limits::PICTURE, MAX_TEXTURE_SIDE) {
            Ok(img) => {
                let size = [img.width as usize, img.height as usize];
                let bytes = img.pixels.len();
                let color = egui::ColorImage::from_rgba_unmultiplied(size, &img.pixels);
                let tex = ctx.load_texture(format!("picture-{}", blob.id), color, egui::TextureOptions::LINEAR);
                self.bytes = self.bytes.saturating_add(bytes);
                self.entries.insert(blob.id, Entry::Ready { tex: tex.clone(), bytes, used: tick });
                self.evict(blob.id);
                Ok(tex)
            }
            Err(e) => {
                let why = e.to_string();
                self.entries.insert(blob.id, Entry::Failed(why.clone()));
                Err(why)
            }
        }
    }

    /// Drop the least recently used textures until the cache fits (never `keep`, the one just
    /// loaded).
    fn evict(&mut self, keep: Id) {
        while self.bytes > CACHE_BYTES {
            let oldest = self
                .entries
                .iter()
                .filter_map(|(id, e)| match e {
                    Entry::Ready { used, .. } if *id != keep => Some((*used, *id)),
                    _ => None,
                })
                .min();
            let Some((_, id)) = oldest else { break };
            if let Some(Entry::Ready { bytes, .. }) = self.entries.remove(&id) {
                self.bytes = self.bytes.saturating_sub(bytes);
            }
        }
    }

    /// How many pictures are cached as textures, and their pixel bytes.
    pub fn stats(&self) -> (usize, usize) {
        (self.entries.values().filter(|e| matches!(e, Entry::Ready { .. })).count(), self.bytes)
    }
}

/// A picture or attachment laid out on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MediaLayout {
    /// The object's id.
    pub id: Id,
    /// Its rectangle (screen).
    pub rect: Rect,
    /// A picture (else an attachment).
    pub picture: bool,
    /// A background picture (behind everything, only reachable by right click).
    pub background: bool,
    /// Drawn over the text: in front of every container and table it overlaps ([`stack`]).
    pub above: bool,
}

/// Which pictures and attachments [`draw`] paints.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    /// Background pictures, under everything.
    Background,
    /// The others under the text.
    Under,
    /// Those over the text ([`MediaLayout::above`]).
    Over,
}

/// Mark the pictures and attachments drawn over the text: those that overlap at least one
/// container or table (`text`, screen frames) and come after every one they overlap in the
/// page's order (Order › Bring to Front, or moved onto older text). The rest stay under the
/// text, which keeps it readable and clickable. Two layers approximate the full order: a
/// picture between two overlapping containers is drawn over both (unverified).
pub(crate) fn stack(page: &Page, media: &mut [MediaLayout], text: &[crate::canvas::OutlineLayout]) {
    if media.is_empty() || text.is_empty() {
        return;
    }
    let order: HashMap<Id, usize> = page.objects.iter().enumerate().map(|(i, o)| (o.id(), i)).collect();
    for m in media.iter_mut().filter(|m| !m.background) {
        let Some(&mi) = order.get(&m.id) else { continue };
        let mut over = text.iter().filter(|o| o.rect.intersects(m.rect)).peekable();
        m.above = over.peek().is_some() && over.all(|o| order.get(&o.id).is_some_and(|&oi| oi < mi));
    }
}

/// Lay out the page's pictures and attachments (screen rectangles, in z-order).
pub(crate) fn layout(page: &Page, origin: Pos2, zoom: f32) -> Vec<MediaLayout> {
    let z = PT * zoom;
    let rect = |p: Point, w: f32, h: f32| Rect::from_min_size(pos2(origin.x + p.x * z, origin.y + p.y * z), vec2(w * z, h * z));
    page.objects
        .iter()
        .filter_map(|o| match o {
            PageObject::Image(im) => {
                Some(MediaLayout { id: im.id, rect: rect(im.pos, im.width, im.height), picture: true, background: im.background, above: false })
            }
            PageObject::Attachment(a) => Some(MediaLayout {
                id: a.id,
                rect: rect(a.pos, ATTACHMENT_SIZE.0, ATTACHMENT_SIZE.1),
                picture: false,
                background: false,
                above: false,
            }),
            _ => None,
        })
        .collect()
}

/// Texture coordinates of the drawn rectangle's corners (top-left, top-right, bottom-right,
/// bottom-left): the stored picture flipped in its own axes, then turned clockwise.
pub(crate) fn corner_uvs(im: &Image) -> [Pos2; 4] {
    let mut out = [Pos2::ZERO; 4];
    for (i, (x, y)) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)].into_iter().enumerate() {
        // A clockwise turn shows at display (x, y) the source point (y, 1 − x).
        let (mut u, mut v) = (x, y);
        for _ in 0..im.quarter_turns % 4 {
            (u, v) = (v, 1.0 - u);
        }
        if im.flip_h {
            u = 1.0 - u;
        }
        if im.flip_v {
            v = 1.0 - v;
        }
        if let Some(o) = out.get_mut(i) {
            *o = pos2(u, v);
        }
    }
    out
}

/// Draw the pictures and attachments of `pass` (`true`: background pictures only; `false`:
/// everything else), in z-order.
pub(crate) fn draw(app: &mut NoteApp, painter: &egui::Painter, page: &Page, media: &[MediaLayout], layer: Layer, t: Tokens) {
    let ctx = painter.ctx().clone();
    let shown = |m: &&MediaLayout| match layer {
        Layer::Background => m.background,
        Layer::Under => !m.background && !m.above,
        Layer::Over => m.above,
    };
    for m in media.iter().filter(shown) {
        if !painter.clip_rect().intersects(m.rect) {
            continue;
        }
        match page.objects.iter().find(|o| o.id() == m.id) {
            Some(PageObject::Image(im)) => match app.canvas.textures.get(&ctx, &im.blob) {
                Ok(tex) => {
                    let uv = corner_uvs(im);
                    let r = m.rect;
                    let mut mesh = egui::Mesh::with_texture(tex.id());
                    for (p, uv) in [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].into_iter().zip(uv) {
                        mesh.vertices.push(egui::epaint::Vertex { pos: p, uv, color: Color32::WHITE });
                    }
                    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                    painter.add(egui::Shape::mesh(mesh));
                }
                Err(_) => placeholder(painter, m.rect, t),
            },
            Some(PageObject::Attachment(a)) => attachment_card(painter, m.rect, &a.name, app.canvas.zoom, t),
            _ => {}
        }
    }
}

/// A picture that can't be shown: a framed box with a crossed-picture glyph.
fn placeholder(painter: &egui::Painter, r: Rect, t: Tokens) {
    painter.rect(r, 0.0, t.hover, Stroke::new(1.0, t.card_border), egui::StrokeKind::Inside);
    let s = r.width().min(r.height()).clamp(8.0, 32.0);
    let icon = Rect::from_center_size(r.center(), vec2(s, s));
    crate::icons::draw(painter, icon, "R.Insert.Images.PictureFromFileInsert", t.text_disabled, t.text_disabled, t.page);
    painter.line_segment([icon.left_bottom(), icon.right_top()], Stroke::new(1.5, t.text_disabled));
}

/// An attachment: the file icon above its (shortened) name.
fn attachment_card(painter: &egui::Painter, r: Rect, name: &str, zoom: f32, t: Tokens) {
    let s = (32.0 * zoom).clamp(12.0, 96.0);
    let icon = Rect::from_center_size(pos2(r.center().x, r.min.y + s / 2.0 + 4.0 * zoom), vec2(s, s));
    crate::icons::draw(painter, icon, "R.Insert.Files.FileAttachmentInsert", t.icon, t.accent, t.page);
    let font = FontId::proportional((10.0 * zoom * PT).clamp(6.0, 40.0));
    let mut job = egui::text::LayoutJob::simple(name.to_string(), font, t.page_text, r.width());
    job.halign = egui::Align::Center;
    job.wrap.max_rows = 2;
    let galley = painter.ctx().fonts_mut(|f| f.layout_job(job));
    painter.galley(pos2(r.center().x, icon.max.y + 2.0 * zoom), galley, t.page_text);
}

/// The selection frame and, for a picture, its eight sizing handles (corners first).
pub(crate) fn draw_selection(painter: &egui::Painter, m: &MediaLayout, t: Tokens) {
    painter.rect_stroke(m.rect, 0.0, Stroke::new(1.0, t.accent), egui::StrokeKind::Outside);
    if !m.picture {
        painter.rect_filled(m.rect, 0.0, t.selection);
        return;
    }
    for h in handles(m.rect) {
        painter.rect(h, 1.0, t.page, Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
    }
}

/// The resize cursor over sizing handle `i` (in [`handles`] order).
pub(crate) fn handle_cursor(i: usize) -> egui::CursorIcon {
    use egui::CursorIcon;
    match i {
        0 | 2 => CursorIcon::ResizeNwSe,
        1 | 3 => CursorIcon::ResizeNeSw,
        4 | 6 => CursorIcon::ResizeVertical,
        _ => CursorIcon::ResizeHorizontal,
    }
}

/// The eight sizing handles: corners (top-left, top-right, bottom-right, bottom-left), then
/// edges (top, right, bottom, left).
pub(crate) fn handles(r: Rect) -> [Rect; 8] {
    let at = |p: Pos2| Rect::from_center_size(p, vec2(HANDLE, HANDLE));
    [
        at(r.left_top()),
        at(r.right_top()),
        at(r.right_bottom()),
        at(r.left_bottom()),
        at(r.center_top()),
        at(r.right_center()),
        at(r.center_bottom()),
        at(r.left_center()),
    ]
}

/// The new page rectangle `(x, y, width, height)` after dragging handle `h` (see [`handles`])
/// of `start` by `d` page points. Corners keep the proportions; edges change one side. The
/// opposite corner or edge stays put.
pub(crate) fn resized(start: (f32, f32, f32, f32), h: usize, d: (f32, f32)) -> (f32, f32, f32, f32) {
    let (x, y, w, hgt) = start;
    let min = notecraft_engine::doc::media::MIN_PICTURE_SIZE;
    let ratio = if w > 0.0 { hgt / w } else { 1.0 };
    // Signs: which way a positive pointer move grows the picture, per handle.
    let (sx, sy) = match h {
        0 => (-1.0, -1.0),
        1 => (1.0, -1.0),
        2 => (1.0, 1.0),
        3 => (-1.0, 1.0),
        4 => (0.0, -1.0),
        5 => (1.0, 0.0),
        6 => (0.0, 1.0),
        _ => (-1.0, 0.0),
    };
    let (mut nw, mut nh) = (w + sx * d.0, hgt + sy * d.1);
    if h < 4 {
        // Follow whichever axis moved further, proportionally.
        if (nw - w).abs() * ratio >= (nh - hgt).abs() {
            nh = nw * ratio;
        } else {
            nw = nh / ratio.max(1e-6);
        }
    }
    let (nw, nh) = (nw.max(min), nh.max(min));
    let nx = if sx < 0.0 { x + w - nw } else { x };
    let ny = if sy < 0.0 { y + hgt - nh } else { y };
    (nx.max(0.0), ny.max(0.0), nw, nh)
}

/// The file-dialog filter of every picture format the codecs read.
fn picture_filter() -> Vec<&'static str> {
    notecraft_codecs::Format::ALL.iter().flat_map(|f| f.extensions().iter().copied()).collect()
}

/// Commands that need the host before (a file to read or write) or after (open a file) they
/// run. Returns `true` when `cmd` was handled here.
pub(crate) fn host_action(app: &mut NoteApp, ctx: &egui::Context, cmd: &str) -> bool {
    match cmd {
        "insert.picture" => {
            let exts = picture_filter();
            if let Some(path) = app.host.pick_file("Insert Picture", &[("Pictures", &exts)]) {
                app.run(ctx, cmd, json!({"path": path}));
            }
        }
        "insert.file" => {
            if let Some(path) = app.host.pick_file("Insert File", &[]) {
                app.run(ctx, cmd, json!({"path": path}));
            }
        }
        "insert.screenClipping" => match app.host.start_screen_clip() {
            Ok(()) => app.ui.status = Some(("Select an area of the screen to clip.".into(), ctx.input(|i| i.time))),
            Err(e) => app.ui.status = Some((e, ctx.input(|i| i.time))),
        },
        "insert.printout" => {
            let mut exts = vec!["pdf"];
            exts.extend(picture_filter());
            if let Some(path) = app.host.pick_file("Insert File Printout", &[("PDF files and pictures", &exts)]) {
                app.run(ctx, cmd, json!({"path": path}));
            }
        }
        "object.saveAs" | "attachment.saveAs" => {
            let name = default_name(app, cmd == "object.saveAs");
            if let Some(path) = app.host.save_file("Save As", &name) {
                app.run(ctx, cmd, json!({"path": path}));
            }
        }
        // Draw › Ruler shows the ruler in the middle of what the page shows.
        "draw.ruler" if app.session.draw.ruler.is_none() => {
            let c = app.canvas.to_page(app.canvas.viewport.center());
            app.run(ctx, cmd, json!({"x": c.x, "y": c.y}));
        }
        // The ribbon's and menus' Paste read the system clipboard as Ctrl+V does.
        "edit.paste" | "edit.pasteKeepSource" | "edit.pasteMerge" | "edit.pasteTextOnly" | "edit.pastePicture" => {
            crate::clipboard::paste(app, ctx, cmd, None)
        }
        "attachment.open" | "printout.openOriginal" => {
            if let Some(path) = app.run(ctx, cmd, json!({})).and_then(|v| v.get("path").and_then(|p| p.as_str()).map(str::to_string))
                && let Err(e) = app.host.open_path(&path)
            {
                app.ui.status = Some((e, ctx.input(|i| i.time)));
            }
        }
        _ => return false,
    }
    true
}

/// Most files taken from one drop.
const MAX_DROPPED: usize = 50;

/// Files dropped on the page (`BEH.Paste.Files`): a picture file becomes a picture, anything
/// else (or a picture that can't be read) an attachment, at the drop point, several stacked
/// diagonally. Printouts don't exist yet, so nothing asks "attach or print"; the web build,
/// whose dropped files can only be read asynchronously, says so instead.
pub(crate) fn handle_drops(app: &mut NoteApp, ctx: &egui::Context) {
    let files = ctx.input(|i| i.raw.dropped_files.clone());
    if files.is_empty() || app.session.page().is_none() {
        return;
    }
    if cfg!(target_arch = "wasm32") {
        app.ui.status = Some(("Dropping files on the page isn't available in the web version yet.".into(), ctx.input(|i| i.time)));
        return;
    }
    let at = ctx.pointer_latest_pos().filter(|p| app.canvas.viewport.contains(*p)).map(|p| app.canvas.to_page(p));
    let pictures = picture_filter();
    let mut ask = Vec::new();
    for (k, f) in files.iter().take(MAX_DROPPED).enumerate() {
        let path = f.path();
        let mut params = json!({"path": path.display().to_string()});
        if let Some(pt) = at {
            let d = k as f32 * 24.0;
            params["x"] = json!(pt.x.max(0.0) + d);
            params["y"] = json!(pt.y.max(0.0) + d);
        }
        let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
        if pictures.contains(&ext.as_str()) && app.try_run(ctx, "insert.picture", params.clone()).is_ok() {
            continue;
        }
        // BEH.Paste.Files: a file NoteCraft can print asks first (attach it, or insert its
        // printout); unless another dialog is open, when it is attached as before.
        if ext == "pdf" && app.ui.dialog.is_none() {
            ask.push(params);
            continue;
        }
        app.run(ctx, "insert.file", params);
    }
    if !ask.is_empty() {
        app.ui.dialog = Some(crate::dialogs::Dialog::InsertFile(ask));
    }
}

/// The selected picture's or attachment's suggested file name.
fn default_name(app: &NoteApp, picture: bool) -> String {
    let Some(page) = app.session.page() else { return String::new() };
    let notecraft_engine::selection::PageSel::Objects { ids } = &app.session.sel.on_page else { return String::new() };
    for id in ids {
        match page.objects.iter().find(|o| o.id() == *id) {
            Some(PageObject::Image(im)) if picture => {
                let ext = notecraft_codecs::sniff(&im.blob.bytes).map_or("png", |f| f.extension());
                return format!("Picture.{ext}");
            }
            Some(PageObject::Attachment(a)) if !picture => return a.name.clone(),
            _ => {}
        }
    }
    String::new()
}

/// The topmost picture or attachment under `p`; background pictures only when `background`.
pub(crate) fn hit(app: &NoteApp, p: Pos2, background: bool) -> Option<MediaLayout> {
    app.canvas.media.iter().rev().find(|m| (background || !m.background) && m.rect.contains(p)).copied()
}

/// The hover text of a picture (its alt text) or an attachment (its name and size).
pub(crate) fn tooltip(page: &Page, id: Id) -> Option<String> {
    let o = page.objects.iter().find(|o| o.id() == id)?;
    let base = match o {
        PageObject::Image(im) if !im.alt_text.is_empty() => Some(im.alt_text.clone()),
        PageObject::Attachment(a) => Some(format!("{} ({} KB)", a.name, a.blob.len.div_ceil(1024))),
        _ => None,
    };
    // A linked object shows where its link goes (Open Link is on its menu, Enter opens it).
    match (base, o.link()) {
        (Some(b), Some(l)) => Some(format!("{b}\n{l}")),
        (None, Some(l)) => Some(l.to_string()),
        (b, None) => b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uvs_turn_and_flip() {
        let mut im = Image::new(Id(1), Point::ZERO, Blob::new(Id(2), std::sync::Arc::from(&b"x"[..])), (4, 2), 30.0, 15.0);
        assert_eq!(corner_uvs(&im), [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)]);
        im.quarter_turns = 1;
        // Turned right: the source's bottom-left shows at the top-left.
        assert_eq!(corner_uvs(&im)[0], pos2(0.0, 1.0));
        im.quarter_turns = 0;
        im.flip_h = true;
        assert_eq!(corner_uvs(&im)[0], pos2(1.0, 0.0));
    }

    #[test]
    fn corner_handles_keep_proportions_and_the_opposite_corner() {
        // Dragging the bottom-right corner right by 20 grows a 40×20 picture to 60×30.
        assert_eq!(resized((10.0, 10.0, 40.0, 20.0), 2, (20.0, 0.0)), (10.0, 10.0, 60.0, 30.0));
        // The top-left corner: the bottom-right stays at (140, 120).
        let (x, y, w, h) = resized((100.0, 100.0, 40.0, 20.0), 0, (-20.0, 0.0));
        assert_eq!((x + w, y + h, w), (140.0, 120.0, 60.0));
        // An edge handle changes one side only, never below the minimum.
        assert_eq!(resized((10.0, 10.0, 40.0, 20.0), 5, (-100.0, 50.0)).2, notecraft_engine::doc::media::MIN_PICTURE_SIZE);
        assert_eq!(resized((10.0, 10.0, 40.0, 20.0), 6, (5.0, 10.0)), (10.0, 10.0, 40.0, 30.0));
    }
}
