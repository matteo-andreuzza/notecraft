//! The rich system clipboard (`UI.Window.Clipboard`): copied notes go out as text with HTML
//! (tables as tables) or as a picture, and a paste brings in another application's HTML or
//! picture. The host does the platform work ([`crate::Host::read_clipboard`],
//! [`crate::Host::write_clipboard`]: `arboard` on the desktop, nothing on the web build, whose
//! clipboard stays plain text through egui).
//!
//! **Ctrl+V with only a picture on the clipboard.** egui-winit turns Ctrl+V into a paste event
//! only when the clipboard holds text; otherwise it swallows the key press. Its *release* still
//! arrives as a key event, so [`watch`] treats a V release with Ctrl held (or released a moment
//! before), with no paste event and no typed "v" since the last release, as a paste that found
//! no text, and asks the host for a picture.
//!
//! **Own or foreign picture.** A picture NoteCraft copied also sits in its own clipboard as an
//! object (alt text, size, rotation intact). A fingerprint of the pixels put on the system
//! clipboard tells, on paste, whether the system picture is still that one (paste the object)
//! or something another program copied since (paste the new picture, `"external": true`).

use egui::{Event, Key};
use notecraft_engine::doc::PageObject;
use serde_json::{Value, json};

use crate::NoteApp;

/// A cheap fingerprint of a picture's pixels (size plus a sparse sample: enough to tell two
/// clipboard pictures apart, never stored).
pub(crate) fn fingerprint(img: &notecraft_codecs::Rgba8) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ (u64::from(img.width) << 32) ^ u64::from(img.height);
    let step = (img.pixels.len() / 4096).max(1);
    for b in img.pixels.iter().step_by(step) {
        h = (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// After Copy or Cut: put the result on the system clipboard — the picture, when exactly one
/// picture was copied, else the text with its HTML. Falls back to egui's plain-text clipboard.
pub(crate) fn after_copy(app: &mut NoteApp, ctx: &egui::Context, v: &Value) {
    let text = v.get("text").and_then(Value::as_str).unwrap_or("").to_string();
    let html = v.get("html").and_then(Value::as_str).unwrap_or("").to_string();
    let clip = &app.session.clipboard;
    let picture = match (clip.paragraphs.is_empty(), clip.objects.as_slice()) {
        (true, [PageObject::Image(im)]) => notecraft_codecs::decode(&im.blob.bytes, &notecraft_codecs::Limits::PICTURE).ok(),
        _ => None,
    };
    if app.host.write_clipboard(&text, &html, picture.as_ref()) {
        app.canvas.copied_image = picture.as_ref().map(fingerprint);
    } else if !text.is_empty() {
        ctx.copy_text(text);
    }
}

/// Paste with what the system clipboard holds: `text` from egui's paste event when there was
/// one, the host's HTML, and — only when there is no text, or for Paste: Picture — its picture.
pub(crate) fn paste(app: &mut NoteApp, ctx: &egui::Context, cmd: &str, text: Option<String>) {
    let as_picture = cmd == "edit.pastePicture";
    let rich = app.host.read_clipboard(text.is_none() || as_picture);
    // BEH.Paste.Files: files copied in a file manager go on the page like dropped files (their
    // paths also arrive as text, which a plain paste would type in). Paste as text only keeps
    // the paths.
    if !rich.files.is_empty() && cmd != "edit.pasteTextOnly" && app.session.page().is_some() {
        // No point given: the files go where the caret is (`ops::insert_pos`).
        crate::pictures::insert_files(app, ctx, &rich.files, None);
        return;
    }
    let text = text.or(rich.text);
    let mut params = json!({});
    if let Some(t) = &text {
        params["text"] = json!(t);
    }
    if let Some(h) = rich.html {
        params["html"] = json!(h);
    }
    if (as_picture || text.as_deref().is_none_or(|t| t.trim().is_empty()))
        && let Some(img) = rich.image
        && app.canvas.copied_image != Some(fingerprint(&img))
    {
        match notecraft_codecs::encode_png(&img, &notecraft_codecs::Limits::PICTURE) {
            Ok(png) => {
                params["image"] = json!(notecraft_engine::b64::encode(&png));
                params["text"] = json!("");
                params["external"] = json!(true);
            }
            Err(e) => {
                app.ui.status = Some((e.to_string(), ctx.input(|i| i.time)));
                return;
            }
        }
    }
    app.run(ctx, cmd, params);
}

/// Track this frame's events for the picture-only Ctrl+V (see the module docs). Returns `true`
/// when such a paste should run now.
pub(crate) fn watch(app: &mut NoteApp, ctx: &egui::Context, events: &[Event]) -> bool {
    let (now, ctrl) = ctx.input(|i| (i.time, i.modifiers.command || i.modifiers.ctrl));
    let probe = &mut app.canvas.paste_probe;
    if ctrl {
        probe.ctrl_at = now;
    }
    let mut fire = false;
    for ev in events {
        match ev {
            Event::Paste(_) => probe.seen_paste = true,
            Event::Text(t) if t.contains(['v', 'V']) => probe.typed_v = true,
            Event::Key { key: Key::V, pressed: false, modifiers, .. } => {
                let held = modifiers.command || modifiers.ctrl || now - probe.ctrl_at < 0.5;
                fire = held && !probe.seen_paste && !probe.typed_v;
                probe.seen_paste = false;
                probe.typed_v = false;
            }
            _ => {}
        }
    }
    fire
}

/// State for [`watch`].
#[derive(Default)]
pub(crate) struct PasteProbe {
    seen_paste: bool,
    typed_v: bool,
    ctrl_at: f64,
}
