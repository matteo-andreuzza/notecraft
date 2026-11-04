//! The Paste Options button (catalog `UI.Canvas.PasteOptions`): after a paste, a small button
//! appears near the caret for a few seconds, opening the same three choices as the ribbon's
//! Paste gallery (`R.Home.Clipboard.PasteKeepSource/PasteMerge/PasteText`) so the choice can be
//! changed after the fact, as in the reference. Only text is on the clipboard today (no rich
//! HTML/picture paste yet; `docs/gaps.md`), so all three choices are meaningful but the result
//! often looks the same until the rich clipboard lands.

use egui::{Margin, Rect, Ui, vec2};

use crate::chrome::card_frame;
use crate::{NoteApp, Tokens, ribbon};

/// Seconds the button stays up after a paste (matches the toast's own decay in `lib.rs`).
const SHOWN_SECS: f64 = 6.0;

const CHOICES: &[&str] = &["R.Home.Clipboard.PasteKeepSource", "R.Home.Clipboard.PasteMerge", "R.Home.Clipboard.PasteText"];

/// Draw the button if a paste happened recently, positioned just after the caret.
pub fn show(app: &mut NoteApp, ui: &mut Ui, area: Rect, t: Tokens) {
    let Some(at) = app.ui.paste_shown_at else { return };
    let now = ui.ctx().input(|i| i.time);
    if now - at > SHOWN_SECS {
        app.ui.paste_shown_at = None;
        return;
    }
    let Some(c) = app.session.sel.caret() else { return };
    let Some(pl) = app.canvas.para(c.block, c.para) else { return };
    let caret = pl.caret_rect(c.offset);
    let pos = (caret.max + vec2(4.0, -caret.height())).max(area.min + vec2(4.0, 4.0));
    let ctx = ui.ctx().clone();
    egui::Area::new(egui::Id::new("paste_options")).fixed_pos(pos).order(egui::Order::Foreground).show(&ctx, |ui| {
        card_frame(&t).inner_margin(Margin::symmetric(3, 3)).show(ui, |ui| {
            let id = "R.Home.Clipboard.PasteGallery";
            let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), egui::Sense::click());
            let label = ribbon::label(id);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
            if resp.hovered() {
                ui.painter().rect_filled(rect, crate::theme::radius::CONTROL, t.hover);
            }
            crate::icons::draw(ui.painter(), rect.shrink(3.0), "PasteMenu", t.icon, t.accent, t.ribbon_card);
            let resp = resp.on_hover_text("Paste Options");
            egui::Popup::menu(&resp).show(|ui| {
                ui.set_min_width(190.0);
                for choice in CHOICES {
                    ribbon::menu_item(app, ui, choice, t);
                }
            });
        });
    });
}
