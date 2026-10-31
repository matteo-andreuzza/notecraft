//! Password protection in the window (decision D15): the Password Protection pane (Review ›
//! Password, catalog `UI.Panes.Password`), the unlock prompt that takes the page's place while
//! the current section is locked (`BEH.Password.Lock`), and the lock drawn on a protected
//! section's row in the navigation tree. Passwords typed here live only in [`PasswordInput`],
//! which is never serialized, and are cleared as soon as they are used.

use egui::{RichText, Ui, vec2};
use serde_json::json;

use crate::chrome::{card_frame, card_header};
use crate::{NoteApp, Tokens};

/// What the pane is asking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PasswordForm {
    /// Just the buttons.
    #[default]
    None,
    /// Set Password: a password and its confirmation.
    Set,
    /// Change Password: the old one, the new one and its confirmation.
    Change,
    /// Remove Password: the current one.
    Remove,
}

/// Passwords being typed (never serialized; cleared after use).
#[derive(Debug, Clone, Default)]
pub struct PasswordInput {
    /// The open form of the pane.
    pub form: PasswordForm,
    /// The current (or only) password field.
    pub current: String,
    /// The new password.
    pub new: String,
    /// Its confirmation.
    pub confirm: String,
    /// The unlock prompt's field.
    pub unlock: String,
    /// The last problem to show under the form.
    pub problem: Option<String>,
}

fn field(ui: &mut Ui, label: &str, value: &mut String) -> egui::Response {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).password(true).desired_width(f32::INFINITY))
}

/// Run `cmd`; on failure keep the message for the form.
fn run(app: &mut NoteApp, ctx: &egui::Context, cmd: &str, params: serde_json::Value) -> bool {
    match app.session.run(cmd, params) {
        Ok(_) => {
            app.ui.password.problem = None;
            true
        }
        Err(e) => {
            app.ui.password.problem = Some(e.to_string());
            ctx.request_repaint();
            false
        }
    }
}

/// The Password Protection pane.
pub fn pane(app: &mut NoteApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    egui::Panel::right("password_pane")
        .default_size(270.0)
        .resizable(true)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(t.window_back).inner_margin(egui::Margin { left: 6, right: 8, top: 6, bottom: 8 }))
        .show(ui, |ui| {
            card_frame(&t).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                if card_header(ui, "Password Protection", &t) {
                    app.session.view.password_pane = false;
                    app.ui.password = PasswordInput::default();
                }
                ui.add_space(4.0);
                let Some(sec) = app.session.section().cloned() else {
                    ui.label(RichText::new("Open a section to protect it.").color(t.text_disabled));
                    return;
                };
                ui.label(RichText::new("Current Section").strong());
                ui.label(&sec.name);
                ui.add_space(6.0);
                let form = app.ui.password.form;
                match (form, sec.protection.is_some()) {
                    (PasswordForm::None, false) => {
                        if ui.button("Set Password…").clicked() {
                            app.ui.password.form = PasswordForm::Set;
                        }
                    }
                    (PasswordForm::None, true) => {
                        ui.label(
                            RichText::new(if sec.is_locked() { "This section is locked." } else { "This section has a password." })
                                .color(t.page_meta),
                        );
                        if ui.button("Change Password…").clicked() {
                            app.ui.password.form = PasswordForm::Change;
                        }
                        if ui.button("Remove Password…").clicked() {
                            app.ui.password.form = PasswordForm::Remove;
                        }
                    }
                    (PasswordForm::Set, _) => {
                        let pw = &mut app.ui.password;
                        field(ui, "Enter password:", &mut pw.new);
                        field(ui, "Confirm password:", &mut pw.confirm);
                        ui.horizontal(|ui| {
                            if ui.button("OK").clicked() {
                                if app.ui.password.new != app.ui.password.confirm {
                                    app.ui.password.problem = Some("The passwords don't match.".into());
                                } else {
                                    let p = json!({"password": app.ui.password.new.clone()});
                                    if run(app, &ctx, "section.setPassword", p) {
                                        app.ui.password = PasswordInput::default();
                                    }
                                }
                            }
                            if ui.button("Cancel").clicked() {
                                app.ui.password = PasswordInput::default();
                            }
                        });
                    }
                    (PasswordForm::Change, _) => {
                        let pw = &mut app.ui.password;
                        field(ui, "Old password:", &mut pw.current);
                        field(ui, "New password:", &mut pw.new);
                        field(ui, "Confirm new password:", &mut pw.confirm);
                        ui.horizontal(|ui| {
                            if ui.button("OK").clicked() {
                                if app.ui.password.new != app.ui.password.confirm {
                                    app.ui.password.problem = Some("The new passwords don't match.".into());
                                } else {
                                    let p = json!({"old": app.ui.password.current.clone(), "new": app.ui.password.new.clone()});
                                    if run(app, &ctx, "section.changePassword", p) {
                                        app.ui.password = PasswordInput::default();
                                    }
                                }
                            }
                            if ui.button("Cancel").clicked() {
                                app.ui.password = PasswordInput::default();
                            }
                        });
                    }
                    (PasswordForm::Remove, _) => {
                        field(ui, "Password:", &mut app.ui.password.current);
                        ui.horizontal(|ui| {
                            if ui.button("OK").clicked() {
                                let p = json!({"password": app.ui.password.current.clone()});
                                if run(app, &ctx, "section.removePassword", p) {
                                    app.ui.password = PasswordInput::default();
                                }
                            }
                            if ui.button("Cancel").clicked() {
                                app.ui.password = PasswordInput::default();
                            }
                        });
                    }
                }
                if let Some(problem) = &app.ui.password.problem {
                    let c = ui.visuals().error_fg_color;
                    ui.label(RichText::new(problem).color(c));
                }
                ui.add_space(10.0);
                ui.separator();
                ui.label(RichText::new("All Protected Sections").strong());
                let any_open = !app.session.unlocked_protected().is_empty();
                if ui.add_enabled(any_open, egui::Button::new("Lock All  (Ctrl+Alt+L)")).clicked() {
                    app.run(&ctx, "protect.lockAll", json!({}));
                }
                ui.add_space(10.0);
                ui.label(RichText::new("Password Options").strong());
                // OPT.Advanced.LockAfterIdle, OPT.Advanced.LockOnLeave (also in File › Options).
                let mut minutes = app.session.prefs.lock_after_minutes;
                let mut idle = minutes > 0;
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut idle, "Lock after").changed() {
                        minutes = if idle { 10 } else { 0 };
                    }
                    ui.add_enabled(idle, egui::DragValue::new(&mut minutes).range(1..=1440).suffix(" min"));
                });
                if minutes != app.session.prefs.lock_after_minutes {
                    app.run(&ctx, "file.options", json!({"set": {"lock_after_minutes": minutes}}));
                }
                let mut leave = app.session.prefs.lock_on_leave;
                if ui.checkbox(&mut leave, "Lock as soon as I leave a section").changed() {
                    app.run(&ctx, "file.options", json!({"set": {"lock_on_leave": leave}}));
                }
                ui.add_space(10.0);
                // BEH.Password.NoRecovery.
                ui.label(
                    RichText::new("If you forget a password, nobody can recover the section's notes, not even NoteCraft.")
                        .size(12.0)
                        .color(t.page_meta),
                );
            });
        });
}

/// The prompt shown instead of the page while the current section is locked. Returns whether
/// it was shown (the canvas is then skipped).
pub fn locked_view(app: &mut NoteApp, ui: &mut Ui, t: Tokens) -> bool {
    let Some(sec) = app.session.section().filter(|s| s.is_locked()).cloned() else { return false };
    let ctx = ui.ctx().clone();
    let area = ui.max_rect();
    let card = egui::Rect::from_center_size(area.center() - vec2(0.0, area.height() * 0.15), vec2(380.0, 170.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(card), |ui| {
        card_frame(&t).show(ui, |ui| {
            ui.set_width(350.0);
            ui.label(RichText::new(format!("{} is password protected", sec.name)).size(15.0).strong());
            ui.add_space(4.0);
            ui.label(RichText::new("Enter the password to unlock this section.").color(t.page_meta));
            ui.add_space(6.0);
            let resp =
                ui.add(egui::TextEdit::singleline(&mut app.ui.password.unlock).password(true).hint_text("Password").desired_width(f32::INFINITY));
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                if (ui.button("Unlock").clicked() || enter) && !app.ui.password.unlock.is_empty() {
                    let p = json!({"section": sec.id.to_string(), "password": app.ui.password.unlock.clone()});
                    app.ui.password.unlock.clear();
                    run(app, &ctx, "section.unlock", p);
                }
            });
            if let Some(problem) = &app.ui.password.problem {
                let c = ui.visuals().error_fg_color;
                ui.label(RichText::new(problem).color(c));
            }
        });
    });
    true
}

/// The lock beside a protected section's name in the navigation tree (closed when locked).
pub fn draw_lock(painter: &egui::Painter, at: egui::Pos2, locked: bool, t: &Tokens) {
    let body = egui::Rect::from_min_size(at + vec2(0.0, 5.0), vec2(9.0, 7.0));
    painter.rect_filled(body, 1.5, t.page_meta);
    let shackle_right = if locked { body.max.x - 2.0 } else { body.max.x + 2.0 };
    let pts = vec![
        egui::pos2(body.min.x + 2.0, body.min.y),
        egui::pos2(body.min.x + 2.0, at.y + 1.5),
        egui::pos2(body.center().x, at.y),
        egui::pos2(shackle_right, at.y + 1.5),
        egui::pos2(shackle_right, if locked { body.min.y } else { at.y + 3.5 }),
    ];
    painter.add(egui::Shape::line(pts, egui::Stroke::new(1.3, t.page_meta)));
}
