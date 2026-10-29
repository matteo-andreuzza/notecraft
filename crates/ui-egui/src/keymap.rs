//! Keyboard: egui key events → portable shortcuts (`Mod+B`, `Mod+Shift+>`) → commands.
//!
//! The shortcut table is the command registry itself (each command carries the reference's
//! keys, checked against the catalog's `KB.*` rows). A few keys depend on context, as in the
//! reference, and are routed here first:
//!
//! - Enter on a link opens it (`KB.Format.OpenLink`), in a table cell adds a row
//!   (`KB.Table.NewRowEnter`), otherwise starts a paragraph;
//! - Tab / Shift+Tab move between table cells (a new row or column from the last cell), and
//!   elsewhere indent and outdent at the start of a paragraph (`KB.Outline.IndentTab`);
//! - Alt+Shift+Up/Down move paragraphs on the canvas but pages in the page list;
//! - arrows, Home/End take their target from the canvas layout (visual lines);
//! - Esc closes Find, then cancels the selection;
//! - typed text, IME commits and the clipboard events go to the canvas when it has focus.

use egui::{Event, Key, Modifiers};
use notecraft_engine::commands;
use notecraft_engine::selection::{Block, FocusArea};
use serde_json::{Value, json};

use crate::NoteApp;

/// Portable name of a key (`None` for keys we never bind).
pub fn key_name(k: Key) -> Option<&'static str> {
    use Key::*;
    Some(match k {
        A => "A",
        B => "B",
        C => "C",
        D => "D",
        E => "E",
        F => "F",
        G => "G",
        H => "H",
        I => "I",
        J => "J",
        K => "K",
        L => "L",
        M => "M",
        N => "N",
        O => "O",
        P => "P",
        Q => "Q",
        R => "R",
        S => "S",
        T => "T",
        U => "U",
        V => "V",
        W => "W",
        X => "X",
        Y => "Y",
        Z => "Z",
        Num0 => "0",
        Num1 => "1",
        Num2 => "2",
        Num3 => "3",
        Num4 => "4",
        Num5 => "5",
        Num6 => "6",
        Num7 => "7",
        Num8 => "8",
        Num9 => "9",
        F1 => "F1",
        F2 => "F2",
        F3 => "F3",
        F4 => "F4",
        F5 => "F5",
        F6 => "F6",
        F7 => "F7",
        F8 => "F8",
        F9 => "F9",
        F10 => "F10",
        F11 => "F11",
        F12 => "F12",
        ArrowLeft => "Left",
        ArrowRight => "Right",
        ArrowUp => "Up",
        ArrowDown => "Down",
        Home => "Home",
        End => "End",
        PageUp => "PageUp",
        PageDown => "PageDown",
        Enter => "Enter",
        Tab => "Tab",
        Escape => "Escape",
        Backspace => "Backspace",
        Delete => "Delete",
        Plus => "Plus",
        Minus => "-",
        Equals => "=",
        Period => ".",
        Comma => ",",
        Slash => "/",
        OpenBracket => "[",
        CloseBracket => "]",
        _ => return None,
    })
}

/// Every spelling a key combination may have in the registry: `Mod+Shift+.` is also
/// `Mod+Shift+>`; `Mod+Shift+=` is `Mod+Shift+Plus`; `Shift+8` is `*`.
pub fn candidates(key: Key, m: Modifiers) -> Vec<String> {
    let Some(name) = key_name(key) else { return Vec::new() };
    let prefix = |shift: bool| {
        let mut s = String::new();
        if m.command || m.ctrl {
            s.push_str("Mod+");
        }
        if m.alt {
            s.push_str("Alt+");
        }
        if shift {
            s.push_str("Shift+");
        }
        s
    };
    let mut out = vec![format!("{}{name}", prefix(m.shift))];
    if m.shift {
        let shifted = match key {
            Key::Period => Some(">"),
            Key::Comma => Some("<"),
            Key::Num8 => Some("*"),
            Key::Equals => Some("Plus"),
            Key::Num0 => Some("0"),
            _ => None,
        };
        if let Some(sn) = shifted {
            out.push(format!("{}{sn}", prefix(true)));
            out.push(format!("{}{sn}", prefix(false)));
        }
        // Ctrl+Shift+[ / ] arrive as { } on some layouts.
        if key == Key::Minus {
            out.push(format!("{}-", prefix(true)));
        }
    }
    if key == Key::Plus {
        // A bare '+' (keypad) is Plus with or without Shift.
        out.push(format!("{}Plus", prefix(true)));
        out.push(format!("{}=", prefix(true)));
    }
    out
}

fn caret_on_link(app: &NoteApp) -> bool {
    let s = &app.session;
    let Some(c) = s.sel.caret() else { return false };
    let Some(page) = s.page() else { return false };
    let Some(p) = notecraft_engine::Session::block_paras(page, c.block).and_then(|ps| ps.get(c.para)) else { return false };
    // Strictly inside a link run (not at its very end, where typing continues normal text).
    c.offset < p.len() && p.fmt_at(c.offset + 1).link.is_some() && p.fmt_at(c.offset).link.is_some() && c.offset > 0
}

fn run(app: &mut NoteApp, ctx: &egui::Context, id: &str, params: Value) {
    // Open Section's shortcut asks the host for the file first; Print's opens File › Print.
    if matches!(id, "file.openSection" | "file.print") && crate::pictures::host_action(app, ctx, id) {
        return;
    }
    app.run(ctx, id, params);
    app.canvas.caret_moved(ctx);
}

/// Route a key that depends on context. Returns true when handled.
/// Whether the caret is in a table cell (Tab, Enter and the arrows then move through cells).
fn caret_in_cell(app: &NoteApp) -> bool {
    app.session.text_range().is_some_and(|r| matches!(r.head.block, Block::Cell(_)))
}

fn contextual(app: &mut NoteApp, ctx: &egui::Context, key: Key, m: Modifiers) -> bool {
    let canvas = app.canvas.focused;
    let page_list = app.session.sel.focus == FocusArea::PageList;
    let plain = !m.command && !m.ctrl && !m.alt;
    match key {
        Key::Escape if plain => {
            if app.session.view.find.active {
                run(app, ctx, "search.dismiss", json!({}));
            } else if app.session.view.full_page {
                run(app, ctx, "view.fullPage", json!({}));
            } else {
                run(app, ctx, "edit.cancelSelection", json!({}));
            }
            true
        }
        Key::Enter if canvas && plain && !m.shift => {
            if caret_on_link(app) {
                run(app, ctx, "link.open", json!({}));
            } else if caret_in_cell(app) {
                run(app, ctx, "table.enter", json!({}));
            } else {
                run(app, ctx, "text.newline", json!({}));
            }
            true
        }
        Key::Enter if canvas && plain && m.shift => {
            run(app, ctx, "text.lineBreak", json!({}));
            true
        }
        Key::Tab if canvas && plain && caret_in_cell(app) => {
            run(app, ctx, if m.shift { "table.prevCell" } else { "table.nextCell" }, json!({}));
            true
        }
        Key::Tab if canvas && plain => {
            if m.shift {
                run(app, ctx, "para.outdent", json!({}));
            } else {
                run(app, ctx, "para.indent", json!({"key": "tab"}));
            }
            true
        }
        Key::Backspace if canvas && plain => {
            run(app, ctx, "text.backspace", json!({}));
            true
        }
        Key::Delete if canvas && plain && !m.shift => {
            run(app, ctx, "text.deleteForward", json!({}));
            true
        }
        // A selected container or table: arrows nudge it, Shift+Left/Right make a container
        // narrower or wider (CM.NoteContainer.ObjectMove / ObjectResize; unverified step).
        Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown
            if canvas && !m.alt && !media_selected(app) && block_selected(app).is_some() =>
        {
            let step = if m.command || m.ctrl { 10.0 } else { 1.0 };
            let (dx, dy) = match key {
                Key::ArrowLeft => (-step, 0.0),
                Key::ArrowRight => (step, 0.0),
                Key::ArrowUp => (0.0, -step),
                _ => (0.0, step),
            };
            if !m.shift {
                run(app, ctx, "object.move", json!({"dx": dx, "dy": dy}));
            } else if let Some(o) = block_selected(app).filter(|o| !o.table)
                && dx != 0.0
            {
                // The text column's width: from the text's left edge to the frame's right padding.
                let w = (o.rect.max.x - 4.0 - o.left) / (crate::canvas::PT * app.canvas.zoom.max(0.1)) + dx;
                run(app, ctx, "object.resize", json!({"id": o.id.to_string(), "width": w.max(notecraft_engine::doc::page::MIN_OUTLINE_WIDTH)}));
            }
            true
        }
        // A selected picture or attachment: arrows nudge it, Shift+arrows resize a picture
        // (CM.Picture.ObjectMove / ObjectResize; unverified step: 1 pt, 10 with Ctrl).
        Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown if canvas && !m.alt && media_selected(app) => {
            let step = if m.command || m.ctrl { 10.0 } else { 1.0 };
            let (dx, dy) = match key {
                Key::ArrowLeft => (-step, 0.0),
                Key::ArrowRight => (step, 0.0),
                Key::ArrowUp => (0.0, -step),
                _ => (0.0, step),
            };
            if m.shift {
                run(app, ctx, "object.resize", json!({"dw": dx, "dh": dy}));
            } else {
                run(app, ctx, "object.move", json!({"dx": dx, "dy": dy}));
            }
            true
        }
        Key::ArrowLeft | Key::ArrowRight if canvas && !m.alt => {
            let word = m.command || m.ctrl;
            let id = match (key, word) {
                (Key::ArrowLeft, false) => "caret.left",
                (Key::ArrowRight, false) => "caret.right",
                (Key::ArrowLeft, true) => "caret.wordLeft",
                _ => "caret.wordRight",
            };
            app.canvas.desired_x = None;
            run(app, ctx, id, json!({"extend": m.shift}));
            true
        }
        Key::ArrowUp | Key::ArrowDown if canvas && !m.alt && !(m.command || m.ctrl) => {
            let kind = if key == Key::ArrowUp { "up" } else { "down" };
            let target = crate::canvas::visual_target(app, kind);
            let mut params = json!({"extend": m.shift});
            if let Some((para, offset)) = target {
                params["para"] = json!(para);
                params["offset"] = json!(offset);
            }
            let id = if key == Key::ArrowUp { "caret.up" } else { "caret.down" };
            app.run(ctx, id, params);
            app.canvas.caret_moved(ctx);
            true
        }
        Key::Home | Key::End if canvas && !m.alt && !(m.command || m.ctrl) => {
            let kind = if key == Key::Home { "home" } else { "end" };
            let target = crate::canvas::visual_target(app, kind);
            let mut params = json!({"extend": m.shift});
            if let Some((para, offset)) = target {
                params["para"] = json!(para);
                params["offset"] = json!(offset);
            }
            app.canvas.desired_x = None;
            run(app, ctx, if key == Key::Home { "caret.lineStart" } else { "caret.lineEnd" }, params);
            true
        }
        Key::ArrowUp | Key::ArrowDown if m.alt && m.shift && !(m.command || m.ctrl) => {
            let up = key == Key::ArrowUp;
            if page_list {
                run(app, ctx, if up { "page.moveUp" } else { "page.moveDown" }, json!({}));
            } else {
                run(app, ctx, if up { "outline.moveUp" } else { "outline.moveDown" }, json!({}));
            }
            true
        }
        Key::ArrowUp | Key::ArrowDown if page_list && plain && !m.shift => {
            run(app, ctx, if key == Key::ArrowUp { "nav.prevPage" } else { "nav.nextPage" }, json!({}));
            true
        }
        Key::Q if m.alt && !m.command && !m.ctrl && !m.shift => {
            // Alt+Q: the title-bar Search (UI.Window.SearchBox), as in the reference.
            ctx.memory_mut(|mem| mem.request_focus(egui::Id::new(crate::chrome::SEARCH_ID)));
            app.canvas.focused = false;
            true
        }
        Key::E if (m.command || m.ctrl) && !m.alt && !m.shift => {
            // Ctrl+E: the search box.
            ctx.memory_mut(|mem| mem.request_focus(egui::Id::new("search_box")));
            app.canvas.focused = false;
            true
        }
        Key::F if (m.command || m.ctrl) && !m.alt && !m.shift => {
            let q = app.ui.find.clone();
            run(app, ctx, "search.page", json!({"query": q}));
            ctx.memory_mut(|mem| mem.request_focus(egui::Id::new("find_box")));
            app.canvas.focused = false;
            true
        }
        Key::K if (m.command || m.ctrl) && !m.alt && !m.shift => {
            app.ui.dialog = Some(crate::dialogs::Dialog::Link { url: String::new(), text: selected_text(app), filter: String::new() });
            true
        }
        Key::M if (m.command || m.ctrl) && m.alt && !m.shift => {
            if let Some(p) = app.session.sel.page {
                app.ui.dialog = Some(crate::dialogs::Dialog::MovePage(p, String::new()));
            }
            true
        }
        Key::G if (m.command || m.ctrl) && !m.alt && !m.shift => {
            // UI.Nav.NotebookList: the notebook drop-down (every notebook is already in the
            // nav tree in this UI, but the reference's quick switcher still gets its shortcut).
            app.ui.dialog = Some(crate::dialogs::Dialog::NotebookList);
            true
        }
        _ => false,
    }
}

fn selected_text(app: &NoteApp) -> String {
    let s = &app.session;
    let Some(r) = s.text_range() else { return String::new() };
    if r.is_caret() {
        return String::new();
    }
    let (a, b) = r.ordered();
    if a.para != b.para {
        return String::new();
    }
    s.page()
        .and_then(|p| notecraft_engine::Session::block_paras(p, a.block))
        .and_then(|ps| ps.get(a.para))
        .map(|p| p.text().get(a.offset..b.offset).unwrap_or("").to_string())
        .unwrap_or_default()
}

/// Process this frame's keyboard, text, IME and clipboard events.
pub fn handle_input(app: &mut NoteApp, ctx: &egui::Context) {
    if app.ui.dialog.is_some() {
        return;
    }
    let text_widget_focused = ctx.memory(|m| m.focused().is_some()) && ctx.egui_wants_keyboard_input();
    let events = ctx.input(|i| i.events.clone());
    // Ctrl+V with no text on the clipboard (a picture): see `clipboard.rs`.
    if crate::clipboard::watch(app, ctx, &events) && app.canvas.focused && !text_widget_focused {
        crate::clipboard::paste(app, ctx, "edit.paste", None);
    }
    let mut consumed: Vec<(Modifiers, Key)> = Vec::new();
    for ev in events {
        match ev {
            Event::Key { key, pressed: true, modifiers, .. } => {
                if text_widget_focused {
                    // Text fields keep their keys, except the global ones below.
                    let global = (modifiers.command || modifiers.ctrl) && matches!(key, Key::N | Key::T | Key::E | Key::M);
                    if !global {
                        continue;
                    }
                }
                // Clipboard keys arrive as Copy/Cut/Paste events; don't run them twice.
                if (modifiers.command || modifiers.ctrl) && !modifiers.alt && matches!(key, Key::C | Key::X) {
                    continue;
                }
                if (modifiers.command || modifiers.ctrl) && !modifiers.alt && key == Key::V {
                    continue;
                }
                if contextual(app, ctx, key, modifiers) {
                    consumed.push((modifiers, key));
                    continue;
                }
                // Canvas-only commands need the canvas focused; the rest are global.
                let found = candidates(key, modifiers).into_iter().find_map(|c| commands::by_shortcut(&c));
                if let Some(cmd) = found {
                    let canvas_only = cmd.id.starts_with("text.") || cmd.id.starts_with("caret.");
                    if canvas_only && !app.canvas.focused {
                        continue;
                    }
                    let id = cmd.id;
                    if id == "edit.selectAll" && !app.canvas.focused {
                        continue;
                    }
                    run(app, ctx, id, json!({}));
                    consumed.push((modifiers, key));
                }
            }
            Event::Text(t) if app.canvas.focused && !text_widget_focused => {
                if !t.is_empty() && !t.chars().all(char::is_control) {
                    run(app, ctx, "text.insert", json!({"text": t}));
                }
            }
            Event::Ime(egui::ImeEvent::Preedit { text, .. }) if app.canvas.focused => {
                app.canvas.preedit = text;
            }
            Event::Ime(egui::ImeEvent::Commit(t)) if app.canvas.focused => {
                app.canvas.preedit.clear();
                if !t.is_empty() {
                    run(app, ctx, "text.insert", json!({"text": t}));
                }
            }
            Event::Copy if (app.canvas.focused || app.session.sel.focus == FocusArea::PageList) && !text_widget_focused => {
                run(app, ctx, "edit.copy", json!({}));
            }
            Event::Cut if (app.canvas.focused || app.session.sel.focus == FocusArea::PageList) && !text_widget_focused => {
                run(app, ctx, "edit.cut", json!({}));
            }
            Event::Paste(t) if (app.canvas.focused || app.session.sel.focus == FocusArea::PageList) && !text_widget_focused => {
                let shift = ctx.input(|i| i.modifiers.shift);
                crate::clipboard::paste(app, ctx, if shift { "edit.pasteTextOnly" } else { "edit.paste" }, Some(t));
            }
            _ => {}
        }
    }
    if !consumed.is_empty() {
        // egui also reads Tab as "focus the next widget" at the end of the frame; a Tab the page
        // used (indent, next cell) must not hand the keyboard to the search box.
        if consumed.iter().any(|(_, k)| *k == Key::Tab) {
            ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
        }
        ctx.input_mut(|i| {
            for (m, k) in consumed {
                i.consume_key(m, k);
            }
        });
    }
}

/// The layout of the note container or table selected as an object, if one is.
fn block_selected(app: &NoteApp) -> Option<crate::canvas::OutlineLayout> {
    let notecraft_engine::selection::PageSel::Objects { ids } = &app.session.sel.on_page else { return None };
    app.canvas.outlines.iter().find(|o| ids.contains(&o.id)).copied()
}

/// Whether the page selection holds a picture or attachment (arrows move it then).
fn media_selected(app: &NoteApp) -> bool {
    let (Some(page), notecraft_engine::selection::PageSel::Objects { ids }) = (app.session.page(), &app.session.sel.on_page) else { return false };
    ids.iter().any(|id| page.image(*id).is_some() || page.attachment(*id).is_some() || page.ink(*id).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punctuation_spellings() {
        let m = Modifiers { ctrl: true, command: true, shift: true, ..Default::default() };
        let c = candidates(Key::Period, m);
        assert!(c.contains(&"Mod+Shift+>".to_string()), "{c:?}");
        let c = candidates(Key::Plus, Modifiers { ctrl: true, command: true, ..Default::default() });
        assert!(c.contains(&"Mod+Plus".to_string()));
        assert_eq!(candidates(Key::B, Modifiers { ctrl: true, command: true, ..Default::default() })[0], "Mod+B");
        assert_eq!(candidates(Key::ArrowLeft, Modifiers { alt: true, ..Default::default() })[0], "Alt+Left");
    }

    #[test]
    fn every_registry_shortcut_is_typeable() {
        // Each key in the registry parses to a combination this keymap can produce, so no
        // documented shortcut is unreachable from the keyboard (side-specific modifiers excepted).
        for c in commands::all() {
            for k in c.keys() {
                if k.contains("LeftShift") || k.contains("RightShift") {
                    continue;
                }
                let last = k.rsplit('+').next().unwrap_or("");
                let ok =
                    last.is_empty() || ["Plus", ">", "<", "*", "-", "=", ".", "/", "[", "]", ","].contains(&last) || key_from_name(last).is_some();
                assert!(ok, "{}: `{k}` has an unknown key `{last}`", c.id);
            }
        }
    }

    fn key_from_name(n: &str) -> Option<Key> {
        Key::ALL.iter().copied().find(|k| key_name(*k) == Some(n))
    }
}
