//! The mini toolbar (catalog `UI.Canvas.MiniToolbar`): a small card of formatting controls that
//! pops up above a text selection, as in the reference (Options › General).
//!
//! It only reads the canvas layout computed this frame (`CanvasState.paras`); it never touches
//! `canvas.rs`'s layout or hit-testing. Every control is one of the Home ribbon's catalog
//! controls, rendered with the same `pub(crate)` helpers the ribbon uses (`ribbon::combo`,
//! `ribbon::small`, `ribbon::gallery_menu`), so behaviour, enabling and tooltips stay identical
//! to their ribbon counterparts — nothing new is wired here.

use egui::{Margin, Rect, Ui, pos2, vec2};
use notecraft_engine::selection::Block;

use crate::chrome::card_frame;
use crate::ribbon_layout::{STYLE_ITEMS, TAG_ITEMS};
use crate::{NoteApp, Tokens, ribbon};

/// The id the floating area is drawn under.
const AREA_ID: &str = "mini_toolbar";

/// Where the toolbar should appear this frame (just above the selection, clamped to the page
/// area), or `None` when it shouldn't show: no selection, a caret only, a drag in progress (the
/// reference hides it while the mouse is still down), or the selection isn't text on the page.
fn anchor(app: &NoteApp, area: Rect) -> Option<Rect> {
    if app.canvas.is_dragging() {
        return None;
    }
    let r = app.session.text_range()?;
    if r.is_caret() {
        return None;
    }
    let (a, b) = r.ordered();
    if !matches!(a.block, Block::Outline(_) | Block::Cell(_)) {
        return None;
    }
    let mut bounds: Option<Rect> = None;
    for pl in app.canvas.paras.iter().filter(|p| p.block == a.block && p.para >= a.para && p.para <= b.para) {
        let pr = pl.rect();
        bounds = Some(match bounds {
            Some(u) => u.union(pr),
            None => pr,
        });
    }
    let bounds = bounds?;
    let pos = pos2(bounds.center().x - 220.0, bounds.min.y - 42.0).max(area.min + vec2(4.0, 4.0));
    let pos = pos2(pos.x.min(area.max.x - 440.0).max(area.min.x + 4.0), pos.y);
    Some(Rect::from_min_size(pos, vec2(440.0, 34.0)))
}

/// Draw the mini toolbar if the current selection calls for it.
pub fn show(app: &mut NoteApp, ui: &mut Ui, area: Rect, t: Tokens) {
    // OPT.General.ShowMiniToolbar.
    if !app.canvas.focused || !app.session.prefs.show_mini_toolbar {
        return;
    }
    let Some(r) = anchor(app, area) else { return };
    let ctx = ui.ctx().clone();
    egui::Area::new(egui::Id::new(AREA_ID)).fixed_pos(r.min).order(egui::Order::Foreground).show(&ctx, |ui| {
        card_frame(&t).inner_margin(Margin::symmetric(6, 4)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                ribbon::combo(app, ui, "R.Home.BasicText.Font", 110.0, t);
                ribbon::combo(app, ui, "R.Home.BasicText.FontSize", 44.0, t);
                ui.separator();
                for id in [
                    "R.Home.BasicText.Bold",
                    "R.Home.BasicText.Italic",
                    "R.Home.BasicText.Underline",
                    "R.Home.BasicText.TextHighlightColorPicker",
                    "R.Home.BasicText.FontColorPicker",
                ] {
                    let kids: &[&str] = if id.ends_with("TextHighlightColorPicker") { &["R.Home.BasicText.HighlightColorNone"] } else { &[] };
                    ribbon::small(app, ui, id, kids, false, t);
                }
                ui.separator();
                ribbon::small(app, ui, "R.Home.BasicText.OutdentClassic", &[], false, t);
                ribbon::small(app, ui, "R.Home.BasicText.IndentClassic", &[], false, t);
                ribbon::small(app, ui, "R.Home.BasicText.BulletsGallery", &[], false, t);
                ribbon::small(app, ui, "R.Home.BasicText.NumberingGallery", &[], false, t);
                ui.separator();
                gallery_button(app, ui, "R.Home.Styles.QuickStylesGallery", STYLE_ITEMS, &["R.Home.Styles.ClearFormatting"], t);
                ribbon::small(app, ui, "R.Home.Tags.ToDoTag", &[], false, t);
                gallery_button(app, ui, "R.Home.Tags.TagsGallery", TAG_ITEMS, &["R.Home.Tags.TagsCustomize", "R.Home.Tags.TagsRemoveAll"], t);
            });
        });
    });
}

/// A small icon button that opens `id`'s full gallery content in a drop-down (Styles, Tags): the
/// ribbon shows these galleries inline, but the toolbar is too narrow for that.
fn gallery_button(app: &mut NoteApp, ui: &mut Ui, id: &str, items: &[&str], extra: &[&str], t: Tokens) {
    let on = ribbon::enabled(app, id);
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), egui::Sense::click());
    let label = ribbon::label(id);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, on, &label));
    if on && resp.hovered() {
        ui.painter().rect_filled(rect, crate::theme::radius::CONTROL, t.hover);
    }
    let (line, accent) = if on { (t.icon, t.accent) } else { (t.text_disabled, t.text_disabled) };
    crate::icons::draw(ui.painter(), rect.shrink(3.0), id, line, accent, t.ribbon_card);
    let resp = resp.on_hover_text(ribbon::tooltip(app, id));
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(170.0);
        ribbon::gallery_menu(app, ui, id, items, extra, t);
    });
}
