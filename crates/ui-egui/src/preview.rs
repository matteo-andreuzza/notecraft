//! A page version or a deleted page shown read-only in place of the page (decision D14;
//! catalog `NC.Versions.Open`, `NC.Recycle.Preview`): a bar saying what it is, with Restore,
//! Delete Version and Close, over a picture of the page drawn as the PNG export draws it.
//! Nothing on it can be edited: the canvas is not drawn and loses the keyboard while it shows.

use egui::{Frame, Margin, RichText, Ui, vec2};
use notecraft_engine::Preview;
use serde_json::json;

use crate::{NoteApp, Tokens};

/// The picture is rendered at 2 px per point; the canvas draws 4/3 px per point at 100%.
const PICTURE_TO_SCREEN: f32 = (4.0 / 3.0) / 2.0;

/// The rendered picture of the preview, kept until the preview or its page changes.
#[derive(Clone)]
struct Cached {
    key: String,
    picture: Result<egui::TextureHandle, String>,
}

/// Draw the preview; does nothing without one.
pub fn show(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    let Some(preview) = app.session.view.preview else { return };
    app.canvas.focused = false;
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, t.window_back);
    let off = app.session.clock.utc_offset_min;
    let (what, version) = match preview {
        Preview::Version { .. } => match app.session.preview_version() {
            Some(v) => {
                let who = if v.author.is_empty() { String::new() } else { format!(" by {}", v.author) };
                (format!("Page version from {} {}{who}. Restore it to make it the current page.", v.at.short_date(off), v.at.short_time(off)), true)
            }
            None => {
                app.session.view.preview = None;
                return;
            }
        },
        Preview::Deleted { .. } => ("This page is in the Notebook Recycle Bin. Restore it to edit it.".to_string(), false),
    };
    Frame::NONE.fill(t.ribbon_card).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(what).color(t.text));
            ui.add_space(8.0);
            if version {
                if ui.button("Restore Version").clicked() {
                    app.run(&ctx, "history.restoreVersion", json!({}));
                }
                if ui.button("Delete Version").clicked() {
                    app.run(&ctx, "history.deleteVersion", json!({}));
                }
            } else if let Preview::Deleted { page } = preview
                && ui.button("Restore").clicked()
            {
                app.run(&ctx, "history.restore", json!({"item": page.to_string()}));
            }
            if ui.button("Close").clicked() {
                app.run(&ctx, "history.closePreview", json!({}));
            }
        });
    });
    // The preview may have closed above.
    if app.session.view.preview != Some(preview) {
        return;
    }
    let key = format!("{preview:?}|{:?}", app.session.preview_page().map(|p| p.modified));
    let id = egui::Id::new("page_preview");
    let cached: Option<Cached> = ctx.data(|d| d.get_temp(id));
    let picture = match cached {
        Some(c) if c.key == key => c.picture,
        _ => {
            let picture = match app.session.preview_picture() {
                Some(Ok(img)) => {
                    let size = [img.width as usize, img.height as usize];
                    Ok(ctx.load_texture("page_preview", egui::ColorImage::from_rgba_unmultiplied(size, &img.pixels), egui::TextureOptions::LINEAR))
                }
                Some(Err(e)) => Err(e.to_string()),
                None => Err("Nothing to show.".to_string()),
            };
            ctx.data_mut(|d| d.insert_temp(id, Cached { key, picture: picture.clone() }));
            picture
        }
    };
    let zoom = app.session.view.zoom.clamp(0.1, 4.0);
    egui::ScrollArea::both().id_salt("preview_scroll").auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(12.0);
        match picture {
            Ok(tex) => {
                let s = tex.size_vec2() * PICTURE_TO_SCREEN * zoom;
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.add(egui::Image::new((tex.id(), vec2(s.x, s.y))));
                });
            }
            Err(e) => {
                ui.label(RichText::new(format!("The page can't be drawn: {e}")).color(t.text_dim));
            }
        }
    });
}
