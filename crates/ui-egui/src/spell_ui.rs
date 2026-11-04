//! Spelling in the window: the wavy underline under misspelled words while typing
//! (`OPT.Proofing.CheckAsYouType`) and the Spelling pane (Review › Spelling, F7; catalog
//! `UI.Panes.Spelling`). The engine decides what is misspelled (`Session::misspelled`); this
//! module caches the answer per paragraph text so a frame doesn't recheck unchanged text.

use std::collections::HashMap;
use std::ops::Range;

use egui::{Painter, RichText, Ui, pos2};
use serde_json::json;

use crate::canvas::ParaLayout;
use crate::chrome::{card_frame, card_header};
use crate::{NoteApp, Tokens};

/// Most paragraph texts remembered.
const CACHE_CAP: usize = 4096;

/// Misspelled ranges per paragraph text, valid for one spelling signature.
#[derive(Default)]
pub struct SpellCache {
    signature: u64,
    map: HashMap<String, Vec<Range<usize>>>,
}

/// What can change the answer for the same text: language, user words, Ignore All, options.
fn signature(app: &NoteApp) -> u64 {
    use std::hash::{Hash, Hasher};
    let p = &app.session.prefs;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (&p.proofing_language, p.custom_words.len(), p.check_as_you_type, p.hide_spelling_errors, p.ignore_uppercase, p.ignore_numbers, p.ignore_urls)
        .hash(&mut h);
    app.session.spelling_generation().hash(&mut h);
    h.finish()
}

/// The misspelled byte ranges of `text`.
pub(crate) fn ranges(app: &mut NoteApp, text: &str) -> Vec<Range<usize>> {
    if text.is_empty() {
        return Vec::new();
    }
    let sig = signature(app);
    if app.canvas.spell.signature != sig {
        app.canvas.spell = SpellCache { signature: sig, map: HashMap::new() };
    }
    if let Some(r) = app.canvas.spell.map.get(text) {
        return r.clone();
    }
    let r = app.session.misspelled(text);
    if app.canvas.spell.map.len() >= CACHE_CAP {
        app.canvas.spell.map.clear();
    }
    app.canvas.spell.map.insert(text.to_string(), r.clone());
    r
}

/// Draw the wavy underline under `r` of a laid-out paragraph (one wave per line it spans).
pub(crate) fn underline(painter: &Painter, pl: &ParaLayout, r: Range<usize>) {
    let color = painter.ctx().global_style().visuals.error_fg_color;
    let (a, b) = (pl.caret_rect(r.start), pl.caret_rect(r.end));
    let lines: Vec<(f32, f32, f32)> = if (a.center().y - b.center().y).abs() < 1.0 {
        vec![(a.min.x, b.min.x, a.max.y)]
    } else {
        // Wrapped: to the end of the first line, and from the line start on the last.
        let first_end = pl.rect().max.x;
        vec![(a.min.x, first_end, a.max.y), (pl.pos.x, b.min.x, b.max.y)]
    };
    for (x0, x1, y) in lines {
        if x1 - x0 < 2.0 {
            continue;
        }
        let mut pts = Vec::new();
        let mut x = x0;
        let mut up = false;
        while x <= x1 {
            pts.push(pos2(x, y - 0.5 + if up { -1.5 } else { 0.0 }));
            x += 2.0;
            up = !up;
        }
        painter.add(egui::Shape::line(pts, egui::Stroke::new(1.0, color)));
    }
}

/// The Spelling pane.
pub fn pane(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    egui::Panel::right("spelling_pane")
        .default_size(260.0)
        .resizable(true)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(t.window_back).inner_margin(egui::Margin { left: 6, right: 8, top: 6, bottom: 8 }))
        .show(ui, |ui| {
            card_frame(&t).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                if card_header(ui, "Spelling", &t) {
                    app.session.view.spelling_pane = false;
                    app.session.view.spelling = None;
                }
                ui.add_space(4.0);
                match app.session.view.spelling.clone() {
                    None => {
                        ui.label(RichText::new("No misspelled word selected.").color(t.text_disabled));
                        if ui.button("Check Spelling  (F7)").clicked() {
                            app.run(&ctx, "review.spelling", json!({}));
                        }
                    }
                    Some(st) => {
                        ui.label(RichText::new("Not in Dictionary").color(t.page_meta));
                        ui.label(RichText::new(&st.word).size(16.0).strong());
                        ui.add_space(4.0);
                        ui.label(RichText::new("Suggestions").color(t.page_meta));
                        if st.suggestions.is_empty() {
                            ui.label(RichText::new("(no suggestions)").color(t.text_disabled));
                        }
                        for s in &st.suggestions {
                            if ui.button(s).clicked() && app.run(&ctx, "review.spellingSuggestion", json!({"word": s})).is_some() {
                                app.run(&ctx, "review.spelling", json!({}));
                            }
                        }
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Ignore").clicked() {
                                app.run(&ctx, "review.spelling", json!({}));
                            }
                            if ui.button("Ignore All").clicked() && app.run(&ctx, "review.ignoreAll", json!({"word": st.word})).is_some() {
                                app.run(&ctx, "review.spelling", json!({}));
                            }
                            if ui.button("Add to Dictionary").clicked() && app.run(&ctx, "review.addToDictionary", json!({"word": st.word})).is_some()
                            {
                                app.run(&ctx, "review.spelling", json!({}));
                            }
                        });
                    }
                }
                ui.add_space(10.0);
                ui.separator();
                // R.Review.Language.SetLanguage: the installed dictionaries.
                // Listed once per pane opening, not every frame.
                let id = egui::Id::new("spell_langs");
                let mut langs: Vec<String> = ui.data_mut(|d| d.get_temp(id)).unwrap_or_else(|| {
                    let l = notecraft_engine::installed_languages();
                    ui.data_mut(|d| d.insert_temp(id, l.clone()));
                    l
                });
                let cur = app.session.prefs.proofing_language.clone();
                if !langs.contains(&cur) {
                    langs.insert(0, cur.clone());
                }
                ui.horizontal(|ui| {
                    ui.label("Language:");
                    egui::ComboBox::from_id_salt("spell_lang").selected_text(cur.clone()).show_ui(ui, |ui| {
                        for l in langs {
                            if ui.selectable_label(l == cur, &l).clicked() {
                                app.run(&ctx, "review.setLanguage", json!({"lang": l}));
                            }
                        }
                    });
                });
            });
        });
}
