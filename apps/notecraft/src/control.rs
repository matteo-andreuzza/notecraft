//! Answering control-channel requests on the UI thread (`docs/control-protocol.md`).
//!
//! The connection threads of `notecraft_automation::control` hand requests over a channel;
//! [`Control::drain`] answers them between frames, where the session lives. Engine methods run
//! through [`NoteApp::try_run`], exactly as the ribbon would run them. `ui.key`, `ui.text` and
//! `ui.click` become real egui input events on the next frame ([`Control::take_input`]), so
//! shortcuts and widgets react as they would to the keyboard and mouse. `ui.screenshot` asks
//! the window for a frame and answers when it arrives.

use std::sync::mpsc::Receiver;

use notecraft_automation::control::Request;
use notecraft_automation::{base64, headless};
use notecraft_ui_egui::NoteApp;
use serde_json::{Value, json};

/// Frames to wait after a request before capturing, so queued input has been drawn.
const SHOT_DELAY_FRAMES: u32 = 3;
/// Seconds to wait for the captured frame before giving up (a hidden or minimised window may
/// present none).
const SHOT_TIMEOUT_S: f64 = 8.0;
/// Most events one request may queue (`ui.text` of a huge string, clicks).
const MAX_QUEUED_EVENTS: usize = 10_000;

/// A screenshot waiting for its frame.
struct Shot {
    token: u64,
    path: Option<String>,
    req: Request,
    frames: u32,
    sent: bool,
    deadline: f64,
}

/// The control channel's side of the app.
pub struct Control {
    rx: Receiver<Request>,
    input: Vec<egui::Event>,
    shots: Vec<Shot>,
    next_token: u64,
}

impl Control {
    /// Answer requests from `rx`.
    pub fn new(rx: Receiver<Request>) -> Control {
        Control { rx, input: Vec::new(), shots: Vec::new(), next_token: 1 }
    }

    /// Input events queued by `ui.*` requests, for `raw_input_hook`.
    pub fn take_input(&mut self) -> Vec<egui::Event> {
        std::mem::take(&mut self.input)
    }

    /// Answer every waiting request; issue and collect screenshots.
    pub fn drain(&mut self, app: &mut NoteApp, ctx: &egui::Context) {
        while let Ok(req) = self.rx.try_recv() {
            if req.method == "ui.screenshot" {
                let path = req.params.get("path").and_then(Value::as_str).map(str::to_string);
                let token = self.next_token;
                self.next_token += 1;
                let deadline = ctx.input(|i| i.time) + SHOT_TIMEOUT_S;
                self.shots.push(Shot { token, path, req, frames: 0, sent: false, deadline });
                continue;
            }
            let r = handle(app, ctx, &mut self.input, &req.method, &req.params);
            req.answer(r);
        }
        self.screenshots(ctx);
        if !self.input.is_empty() || !self.shots.is_empty() {
            ctx.request_repaint();
        }
    }

    fn screenshots(&mut self, ctx: &egui::Context) {
        if self.shots.is_empty() {
            return;
        }
        let images: Vec<(u64, std::sync::Arc<egui::ColorImage>)> = ctx.input(|i| {
            i.raw
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Screenshot { user_data, image, .. } => {
                        Some((user_data.data.as_ref()?.downcast_ref::<u64>().copied()?, image.clone()))
                    }
                    _ => None,
                })
                .collect()
        });
        let now = ctx.input(|i| i.time);
        let mut waiting = Vec::new();
        for mut shot in std::mem::take(&mut self.shots) {
            if let Some((_, image)) = images.iter().find(|(t, _)| *t == shot.token) {
                shot.req.answer(save_screenshot(image, shot.path.as_deref()));
                continue;
            }
            if now > shot.deadline {
                shot.req.answer(Err("no frame was presented (is the window hidden or minimised?)".to_string()));
                continue;
            }
            shot.frames += 1;
            if !shot.sent && shot.frames >= SHOT_DELAY_FRAMES {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(shot.token)));
                shot.sent = true;
            }
            waiting.push(shot);
        }
        self.shots = waiting;
    }
}

/// Answer one request (everything but `ui.screenshot`). `input` collects the events that
/// `ui.key`, `ui.text` and `ui.click` queue for the next frame.
pub fn handle(app: &mut NoteApp, ctx: &egui::Context, input: &mut Vec<egui::Event>, method: &str, p: &Value) -> Result<Value, String> {
    if let Some(r) = headless::engine_query(&app.session, method, p) {
        return r;
    }
    let s = |k: &str| p.get(k).and_then(Value::as_str);
    let f = |k: &str| p.get(k).and_then(Value::as_f64).filter(|v| v.is_finite()).map(|v| v as f32);
    let room = |input: &Vec<egui::Event>, n: usize| {
        if input.len().saturating_add(n) > MAX_QUEUED_EVENTS {
            Err("too much input queued; wait for the window to catch up".to_string())
        } else {
            Ok(())
        }
    };
    match method {
        "engine.execute" | "command" => {
            let id = s("command").ok_or("missing `command`")?;
            let params = p.get("params").cloned().filter(|v| !v.is_null()).unwrap_or(json!({}));
            app.try_run(ctx, id, params).map_err(|e| format!("{id}: {e}"))
        }
        "notebook.save" => {
            let errors = app.session.save_dirty();
            if errors.is_empty() { Ok(json!({"saved": true})) } else { Err(errors.join("; ")) }
        }
        "ui.inspect" => Ok(app.inspect_ui(ctx)),
        "ui.key" => {
            let name = s("key").ok_or("missing `key`")?;
            let key = key_from(name).ok_or_else(|| format!("unknown key `{name}`"))?;
            room(input, 2)?;
            let m = mods(p);
            input.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: m });
            input.push(egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: m });
            Ok(json!({"queued": true}))
        }
        "ui.text" => {
            let text = s("text").ok_or("missing `text`")?;
            room(input, 1)?;
            input.push(egui::Event::Text(text.to_string()));
            Ok(json!({"queued": true}))
        }
        "ui.click" => {
            let (Some(x), Some(y)) = (f("x"), f("y")) else { return Err("missing `x`/`y`".into()) };
            let button = match s("button") {
                Some("secondary" | "right") => egui::PointerButton::Secondary,
                _ => egui::PointerButton::Primary,
            };
            let count = p.get("count").and_then(Value::as_u64).unwrap_or(1).clamp(1, 3) as usize;
            room(input, 1 + 2 * count)?;
            let (pos, m) = (egui::pos2(x, y), mods(p));
            input.push(egui::Event::PointerMoved(pos));
            for _ in 0..count {
                input.push(egui::Event::PointerButton { pos, button, pressed: true, modifiers: m });
                input.push(egui::Event::PointerButton { pos, button, pressed: false, modifiers: m });
            }
            Ok(json!({"queued": true}))
        }
        "app.quit" => {
            app.quit_requested = true;
            Ok(json!({}))
        }
        m if m.starts_with("ui.") => Err(format!("unknown method `{m}`")),
        // Anything else is a command id.
        other => app.try_run(ctx, other, p.clone()).map_err(|e| format!("{other}: {e}")),
    }
}

/// A key by name: egui's names (`A`, `Enter`, `ArrowLeft`, `F3`…) and common aliases.
pub fn key_from(name: &str) -> Option<egui::Key> {
    egui::Key::from_name(name).or(match name.to_ascii_lowercase().as_str() {
        "enter" | "return" => Some(egui::Key::Enter),
        "esc" | "escape" => Some(egui::Key::Escape),
        "delete" | "del" => Some(egui::Key::Delete),
        "backspace" => Some(egui::Key::Backspace),
        "left" => Some(egui::Key::ArrowLeft),
        "right" => Some(egui::Key::ArrowRight),
        "up" => Some(egui::Key::ArrowUp),
        "down" => Some(egui::Key::ArrowDown),
        "space" => Some(egui::Key::Space),
        "tab" => Some(egui::Key::Tab),
        "home" => Some(egui::Key::Home),
        "end" => Some(egui::Key::End),
        "pageup" => Some(egui::Key::PageUp),
        "pagedown" => Some(egui::Key::PageDown),
        n if n.len() == 1 => egui::Key::from_name(&n.to_ascii_uppercase()),
        _ => None,
    })
}

/// Modifiers from `shift`, `ctrl` (or `cmd`), `alt`.
fn mods(p: &Value) -> egui::Modifiers {
    let b = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
    let cmd = b("ctrl") || b("cmd") || b("command");
    egui::Modifiers {
        alt: b("alt"),
        ctrl: cmd && !cfg!(target_os = "macos"),
        shift: b("shift"),
        mac_cmd: cmd && cfg!(target_os = "macos"),
        command: cmd,
    }
}

/// Encode a window capture as PNG: written to `path`, or returned as base64 `png` (never a
/// fixed name in a shared temporary folder).
pub fn save_screenshot(image: &egui::ColorImage, path: Option<&str>) -> Result<Value, String> {
    let [w, h] = image.size;
    let png = encode_png(image)?;
    match path {
        Some(path) => {
            std::fs::write(path, &png).map_err(|e| format!("{path}: {e}"))?;
            Ok(json!({"path": path, "width": w, "height": h}))
        }
        None => Ok(json!({"png": base64::encode(&png), "width": w, "height": h})),
    }
}

fn encode_png(image: &egui::ColorImage) -> Result<Vec<u8>, String> {
    let [w, h] = image.size;
    let (w32, h32) = (u32::try_from(w).map_err(|_| "image too wide")?, u32::try_from(h).map_err(|_| "image too tall")?);
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w32, h32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use notecraft_engine::{Clock, Session};
    use notecraft_ui_egui::FixedHost;

    fn app() -> NoteApp {
        NoteApp::new(Session::with_sample(Clock::default()), Box::new(FixedHost(Clock::default())))
    }

    #[test]
    fn engine_methods_run_through_the_app() {
        let (mut a, ctx, mut input) = (app(), egui::Context::default(), Vec::new());
        let v = handle(&mut a, &ctx, &mut input, "engine.execute", &json!({"command": "page.new", "params": {"title": "Wired"}})).unwrap();
        assert!(v.get("page").is_some());
        let t = handle(&mut a, &ctx, &mut input, "page.text", &json!({})).unwrap();
        assert!(t["text"].as_str().unwrap().starts_with("# Wired"));
        assert!(handle(&mut a, &ctx, &mut input, "engine.commands", &json!({})).unwrap().as_array().is_some_and(|c| !c.is_empty()));
        assert!(handle(&mut a, &ctx, &mut input, "notebook.inspect", &json!({})).unwrap()["notebooks"].is_array());
        // A bare command id works too, and errors name the command.
        assert!(handle(&mut a, &ctx, &mut input, "edit.undo", &json!({})).is_ok());
        assert!(handle(&mut a, &ctx, &mut input, "no.such", &json!({})).unwrap_err().starts_with("no.such:"));
        assert!(handle(&mut a, &ctx, &mut input, "engine.execute", &json!({})).unwrap_err().contains("missing"));
        assert_eq!(handle(&mut a, &ctx, &mut input, "notebook.save", &json!({})).unwrap(), json!({"saved": true}));
        let ui = handle(&mut a, &ctx, &mut input, "ui.inspect", &json!({})).unwrap();
        assert_eq!(ui["tab"], "Home");
        // The 2024 layout: the simplified ribbon and the navigation are reported.
        assert_eq!(ui["ribbonMode"], "simplified");
        assert_eq!(ui["navigation"], true);
        handle(&mut a, &ctx, &mut input, "engine.execute", &json!({"command": "ui.ribbonMode", "params": {"mode": "classic"}})).unwrap();
        assert_eq!(handle(&mut a, &ctx, &mut input, "ui.inspect", &json!({})).unwrap()["ribbonMode"], "classic");
        assert!(handle(&mut a, &ctx, &mut input, "ui.nope", &json!({})).is_err());
        assert!(input.is_empty());
        handle(&mut a, &ctx, &mut input, "app.quit", &json!({})).unwrap();
        assert!(a.quit_requested);
    }

    #[test]
    fn ui_methods_queue_real_input() {
        let (mut a, ctx, mut input) = (app(), egui::Context::default(), Vec::new());
        handle(&mut a, &ctx, &mut input, "ui.key", &json!({"key": "b", "ctrl": true})).unwrap();
        assert!(matches!(input[0], egui::Event::Key { key: egui::Key::B, pressed: true, modifiers, .. } if modifiers.command));
        assert!(matches!(input[1], egui::Event::Key { pressed: false, .. }));
        handle(&mut a, &ctx, &mut input, "ui.text", &json!({"text": "hi"})).unwrap();
        handle(&mut a, &ctx, &mut input, "ui.click", &json!({"x": 10, "y": 20, "count": 2, "button": "secondary"})).unwrap();
        assert_eq!(input.len(), 2 + 1 + 1 + 4);
        assert!(handle(&mut a, &ctx, &mut input, "ui.key", &json!({"key": "NotAKey"})).unwrap_err().contains("unknown key"));
        assert!(handle(&mut a, &ctx, &mut input, "ui.click", &json!({"x": "a"})).is_err());
        assert!(handle(&mut a, &ctx, &mut input, "ui.click", &json!({"x": f64::MAX, "y": 1})).is_ok());
        input.resize(MAX_QUEUED_EVENTS, egui::Event::Copy);
        assert!(handle(&mut a, &ctx, &mut input, "ui.text", &json!({"text": "more"})).unwrap_err().contains("too much input"));
    }

    #[test]
    fn keys_by_name() {
        for (n, k) in [
            ("Enter", egui::Key::Enter),
            ("esc", egui::Key::Escape),
            ("Left", egui::Key::ArrowLeft),
            ("F3", egui::Key::F3),
            ("1", egui::Key::Num1),
            ("z", egui::Key::Z),
            ("PageDown", egui::Key::PageDown),
        ] {
            assert_eq!(key_from(n), Some(k), "{n}");
        }
        assert_eq!(key_from(""), None);
        assert_eq!(key_from("Hyper"), None);
    }

    #[test]
    fn screenshots_encode_to_png() {
        let img = egui::ColorImage::new([3, 2], vec![egui::Color32::from_rgb(10, 20, 30); 6]);
        let v = save_screenshot(&img, None).unwrap();
        assert_eq!((v["width"].as_u64(), v["height"].as_u64()), (Some(3), Some(2)));
        assert!(v["png"].as_str().unwrap().starts_with("iVBORw0KGgo"), "PNG signature in base64");
        let dir = std::env::temp_dir().join(format!("notecraft-shot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("shot.png");
        let v = save_screenshot(&img, Some(&p.display().to_string())).unwrap();
        assert_eq!(v["path"], p.display().to_string());
        assert!(std::fs::read(&p).unwrap().starts_with(b"\x89PNG"));
        assert!(save_screenshot(&img, Some(&dir.join("missing/dir/x.png").display().to_string())).is_err());
    }
}
