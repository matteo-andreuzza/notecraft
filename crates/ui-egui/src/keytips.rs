//! KeyTips (`UI.Ribbon.Keytips`, `KB.App.Keytips`): F10, or Alt pressed and released on its
//! own, puts a letter on every tab; a tab's letter opens it and lists its commands' letters;
//! a command's letter runs it as a click would. Esc (or a click) leaves. The Insert letters
//! are Microsoft's documented sequences (Alt+N, F …); the tab letters and the rest are
//! unverified.

use egui::{Align2, Event, FontId, Key, Modifiers, Ui, vec2};

use crate::NoteApp;

/// Each tab's letter.
pub const TAB_KEYS: &[(&str, Key, &str)] = &[
    ("File", Key::F, "F"),
    ("Home", Key::H, "H"),
    ("Insert", Key::N, "N"),
    ("Draw", Key::D, "D"),
    ("History", Key::S, "S"),
    ("Review", Key::R, "R"),
    ("View", Key::W, "W"),
    ("Help", Key::Y, "Y"),
];

/// Commands' letters, by tab: (tab, key, letter, catalog id).
pub const COMMAND_KEYS: &[(&str, Key, &str, &str)] = &[
    // KB.Insert.FileKeytip, KB.Insert.PrintoutKeytip, KB.Insert.PictureKeytip.
    ("Insert", Key::F, "F", "R.Insert.Files.FileAttachmentInsert"),
    ("Insert", Key::O, "O", "R.Insert.Files.PrintoutFromFileInsert"),
    ("Insert", Key::P, "P", "R.Insert.Images.PictureFromFileInsert"),
];

/// The letter shown on tab `name` while KeyTips show the tabs.
pub fn tab_letter(app: &NoteApp, name: &str) -> Option<&'static str> {
    if app.ui.keytips.as_deref() != Some("") {
        return None;
    }
    TAB_KEYS.iter().find(|(t, _, _)| *t == name).map(|(_, _, l)| *l)
}

/// Read the keyboard for KeyTips before anything else does: start them, follow a letter, or
/// leave. While they show, letters and their text are taken from the input.
pub fn handle(app: &mut NoteApp, ui: &Ui) {
    let ctx = ui.ctx().clone();
    // Alt on its own: down, then up with nothing pressed in between.
    let (m, pressed_any) = ctx.input(|i| {
        (i.modifiers, i.events.iter().any(|e| matches!(e, Event::Key { pressed: true, .. } | Event::PointerButton { pressed: true, .. })))
    });
    let alt_only = m.alt && !m.ctrl && !m.shift && !m.command;
    if alt_only {
        if !app.ui.alt_down {
            app.ui.alt_alone = true;
        }
        if pressed_any {
            app.ui.alt_alone = false;
        }
    } else if app.ui.alt_down && app.ui.alt_alone && !m.any() {
        app.ui.keytips = if app.ui.keytips.is_some() { None } else { Some(String::new()) };
        app.ui.alt_alone = false;
    }
    app.ui.alt_down = alt_only;
    let Some(level) = app.ui.keytips.clone() else { return };
    let take = |k: Key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k) || i.consume_key(Modifiers::SHIFT, k));
    if take(Key::Escape) || ctx.input(|i| i.pointer.any_pressed()) {
        app.ui.keytips = None;
        return;
    }
    if level.is_empty() {
        for (tab, key, _) in TAB_KEYS {
            if take(*key) {
                if *tab == "File" {
                    app.ui.backstage = Some(crate::Backstage::Info);
                    app.ui.keytips = None;
                } else {
                    app.session.view.ribbon_collapsed = false;
                    app.ui.tab = tab.to_string();
                    app.ui.keytips = Some(tab.to_string());
                }
                break;
            }
        }
    } else {
        for (tab, key, _, id) in COMMAND_KEYS {
            if *tab == level && take(*key) {
                app.ui.keytips = None;
                crate::ribbon::fire(app, ui, id);
                break;
            }
        }
    }
    // Letters never reach the page while KeyTips show.
    if app.ui.keytips.is_some() {
        ctx.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Text(_))));
    }
}

/// The open tab's command letters, in a strip under the ribbon.
pub fn strip(app: &NoteApp, ui: &Ui, t: crate::Tokens) {
    let Some(level) = app.ui.keytips.as_deref().filter(|l| !l.is_empty()) else { return };
    let items: Vec<String> = COMMAND_KEYS.iter().filter(|c| c.0 == level).map(|c| format!("{}  {}", c.2, crate::ribbon::label(c.3))).collect();
    let text = if items.is_empty() { crate::i18n::tr("No key tips on this tab yet (Esc to leave)") } else { items.join("   ·   ") };
    let at = ui.max_rect().left_top() + vec2(12.0, 128.0);
    egui::Area::new(egui::Id::new("keytips_strip")).fixed_pos(at).order(egui::Order::Foreground).show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.label(egui::RichText::new(text).color(t.text));
        });
    });
}

/// A keytip badge at the bottom centre of `r`.
pub fn badge(ui: &Ui, r: egui::Rect, letter: &str, t: &crate::Tokens) {
    let c = egui::pos2(r.center().x, r.max.y - 4.0);
    let b = egui::Rect::from_center_size(c, vec2(16.0, 16.0));
    ui.painter().rect_filled(b, 2.0, t.text);
    ui.painter().text(c, Align2::CENTER_CENTER, letter, FontId::proportional(11.0), t.window_back);
}
