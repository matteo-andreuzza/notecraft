//! The simplified ribbon of the 2024 UI (catalog `UI.Ribbon.Simplified`; decision D1,
//! `docs/ui-parity.md` §3 row 7): one row of 28 pt buttons with 16 px icons, labels only where the
//! reference has them, the font and size boxes, split ▾ buttons and a **…** overflow menu.
//!
//! No sibling app has a simplified ribbon; this is NoteCraft's own design. The data below only
//! places controls: what a control is (label, command, kind) comes from its catalog row, and what
//! its drop-down holds comes from the classic layout (`ribbon_layout::controls`), so the two
//! layouts can't disagree. Every classic control is reachable: the ones listed here sit in the
//! row (or move into the overflow menu, lowest priority first, when the window is narrow), and
//! every other classic control is listed in the overflow menu under its classic group name
//! (`tests::every_classic_control_is_reachable_in_the_simplified_ribbon`). A tab with no table
//! here (the contextual tabs) gets a simplified layout derived from its classic one.
//!
//! The placements follow the reference screenshots of the desktop Home and Draw tabs and of the
//! web app's simplified tabs (observed only; `docs/ui-parity.md`).

use std::collections::HashSet;

use egui::{Align2, Popup, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};

use crate::ribbon::{self as rb};
use crate::ribbon_layout::{Control, Tab, controls};
use crate::theme::{TypeRung, Weight, radius, size};
use crate::{NoteApp, Tokens, icons};

/// How a control is drawn in the row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// Icon only (its label in the ScreenTip).
    Icon,
    /// Icon and label.
    Label,
    /// A combo box this wide.
    Combo(f32),
    /// A gallery shown as its label and ▾, opening the full gallery.
    Gallery,
}

/// One placed control: catalog id, how it is drawn, and its priority (higher stays in the row
/// longer when the window narrows).
#[derive(Debug, Clone, Copy)]
pub struct E(pub &'static str, pub Kind, pub u8);

/// A tab's simplified layout: groups of placed controls, separated by a rule.
#[derive(Debug, Clone, Copy)]
pub struct SimpleTab {
    /// The tab's name (as in `ribbon_layout::TABS`).
    pub name: &'static str,
    /// Groups, left to right.
    pub groups: &'static [&'static [E]],
}

/// The simplified layout of each built-in tab, left to right, groups separated by a rule.
pub const SIMPLE_TABS: &[SimpleTab] = &[
    SimpleTab {
        name: "Home",
        groups: &[
            &[E("R.Home.Clipboard.PasteMenu", Kind::Label, 9), E("R.Home.Clipboard.FormatPainter", Kind::Icon, 6)],
            &[
                E("R.Home.BasicText.Font", Kind::Combo(118.0), 8),
                E("R.Home.BasicText.FontSize", Kind::Combo(46.0), 8),
                E("R.Home.BasicText.Bold", Kind::Icon, 9),
                E("R.Home.BasicText.Italic", Kind::Icon, 9),
                E("R.Home.BasicText.Underline", Kind::Icon, 9),
                E("R.Home.BasicText.Strikethrough", Kind::Icon, 5),
                E("R.Home.BasicText.TextHighlightColorPicker", Kind::Icon, 7),
                E("R.Home.BasicText.FontColorPicker", Kind::Icon, 7),
                E("R.Home.BasicText.ClearFormatting", Kind::Icon, 3),
                E("R.Home.BasicText.BulletsGallery", Kind::Icon, 8),
                E("R.Home.BasicText.NumberingGallery", Kind::Icon, 7),
                E("R.Home.BasicText.OutdentClassic", Kind::Icon, 5),
                E("R.Home.BasicText.IndentClassic", Kind::Icon, 5),
                E("R.Home.BasicText.ParagraphAlignmentMenu", Kind::Icon, 6),
            ],
            &[
                E("R.Home.Tags.TagsGallery", Kind::Gallery, 8),
                E("R.Home.Tags.ToDoTag", Kind::Icon, 6),
                E("R.Home.Tags.FindTaggedNotes", Kind::Icon, 4),
            ],
            &[E("R.Home.Styles.QuickStylesGallery", Kind::Gallery, 7)],
            &[
                E("R.Home.Email.SendNotesViaEmail", Kind::Label, 4),
                E("R.Home.Meetings.MeetingDetailsInsertGallery", Kind::Label, 4),
                E("R.Home.Voice.Dictate", Kind::Icon, 3),
            ],
        ],
    },
    SimpleTab {
        name: "Insert",
        groups: &[
            &[E("R.Insert.Insert.PenInsertSpace", Kind::Icon, 4), E("R.Insert.Tables.TableInsertGallery", Kind::Label, 9)],
            &[
                E("R.Insert.Files.FileAttachmentInsert", Kind::Label, 7),
                E("R.Insert.Files.PrintoutFromFileInsert", Kind::Label, 5),
                E("R.Insert.Images.PictureFromFileInsert", Kind::Label, 8),
                E("R.Insert.Images.ScreenCaptureInsert", Kind::Icon, 4),
            ],
            &[
                E("R.Insert.Links.HyperlinkCreateHere", Kind::Label, 8),
                E("R.Insert.Recording.AudioRecordingInsert", Kind::Label, 6),
                E("R.Insert.TimeStamp.DateAndTimeInsertOneNote", Kind::Label, 5),
            ],
            &[
                E("R.Insert.Pages.PageTemplatesSplitButton", Kind::Label, 4),
                E("R.Insert.Symbols.InsertBuildingBlocksEquationsGallery", Kind::Label, 6),
                E("R.Insert.Symbols.SymbolInsertGallery", Kind::Label, 6),
            ],
            &[E("R.Insert.Addins.OfficeExtensionsAppStore", Kind::Label, 3), E("R.Insert.Stickers.Sticker", Kind::Label, 3)],
        ],
    },
    SimpleTab {
        name: "Draw",
        groups: &[
            &[
                E("R.Draw.Tools.SelectAndTypeMode", Kind::Icon, 9),
                E("R.Draw.Tools.LassoSelect", Kind::Icon, 8),
                E("R.Draw.Tools.PanningHand", Kind::Icon, 5),
                E("R.Draw.Tools.EraserMenu", Kind::Icon, 8),
            ],
            &[
                E("R.Draw.Tools.InkStylesGallery", Kind::Gallery, 8),
                E("R.Draw.Tools.AddPen", Kind::Icon, 6),
                E("R.Draw.Tools.MoreColorAndThicknessOptions", Kind::Icon, 4),
            ],
            &[E("R.Draw.Tools.Ruler", Kind::Label, 6), E("R.Draw.Shapes.InkShapeGallery", Kind::Gallery, 6)],
            &[
                E("R.Draw.Convert.InkToShape", Kind::Label, 5),
                E("R.Draw.Convert.SelectionToTextConvert", Kind::Label, 5),
                E("R.Draw.Math.Math", Kind::Label, 6),
                E("R.Draw.Replay.InkReplay", Kind::Label, 4),
            ],
        ],
    },
    SimpleTab {
        name: "History",
        groups: &[
            &[E("R.History.Unread.GoToNextUnreadPage", Kind::Label, 8), E("R.History.Unread.UnreadMenu", Kind::Icon, 6)],
            &[
                E("R.History.Authors.FindByDateMenu", Kind::Label, 7),
                E("R.History.Authors.FindUnread", Kind::Label, 6),
                E("R.History.Authors.AuthorHighlightingHide", Kind::Label, 5),
            ],
            &[E("R.History.History.PageVersionSplitButton", Kind::Label, 7), E("R.History.History.RecycleBinMenu", Kind::Label, 7)],
        ],
    },
    SimpleTab {
        name: "Review",
        groups: &[
            &[
                E("R.Review.Proofing.SpellCheck", Kind::Label, 9),
                E("R.Review.Proofing.ResearchPane", Kind::Label, 5),
                E("R.Review.Proofing.Thesaurus", Kind::Label, 6),
            ],
            &[E("R.Review.Accessibility.AccessibilityChecker", Kind::Label, 7)],
            &[E("R.Review.Language.TranslateMenu", Kind::Label, 6), E("R.Review.Language.LanguageCommands", Kind::Label, 5)],
            &[E("R.Review.Section.PasswordProtectSection", Kind::Label, 6), E("R.Review.Notes.LinkedNotes", Kind::Label, 4)],
        ],
    },
    SimpleTab {
        name: "View",
        groups: &[
            &[
                E("R.View.Views.NavigationPanes", Kind::Label, 8),
                E("R.View.Views.FullPageView", Kind::Label, 7),
                E("R.View.Views.CurrentWindowDock", Kind::Label, 3),
                E("R.View.Views.SwitchBackground", Kind::Label, 3),
            ],
            &[E("R.View.Immersive.ImmersiveReader", Kind::Label, 6)],
            &[
                E("R.View.Zoom.ZoomOut", Kind::Icon, 5),
                E("R.View.Zoom.ZoomClassic", Kind::Combo(64.0), 8),
                E("R.View.Zoom.ZoomIn", Kind::Icon, 5),
                E("R.View.Zoom.ZoomCurrent100", Kind::Label, 5),
            ],
            &[
                E("R.View.PageSetup.PageColorGallery", Kind::Label, 7),
                E("R.View.PageSetup.RuleLinesMenu", Kind::Label, 7),
                E("R.View.PageSetup.PageTitleHide", Kind::Label, 4),
                E("R.View.PageSetup.PaperSize", Kind::Label, 4),
            ],
            &[E("R.View.Window.NewSideNote", Kind::Label, 4), E("R.View.Window.WindowNew", Kind::Label, 3)],
        ],
    },
    SimpleTab {
        name: "Help",
        groups: &[&[
            E("R.Help.Help.Help", Kind::Label, 9),
            E("R.Help.Help.ContactSupport", Kind::Label, 6),
            E("R.Help.Help.Feedback", Kind::Label, 6),
            E("R.Help.Help.ShowTraining", Kind::Label, 5),
            E("R.Help.Help.WhatsNew", Kind::Label, 5),
        ]],
    },
];

/// Width of a group separator.
const SEPARATOR: f32 = 9.0;
/// Width of the ▾ part of a split button.
const ARROW: f32 = 13.0;

/// Contextual tabs laid out by hand (the rest follow their classic layout).
pub const SIMPLE_CONTEXTUAL: &[SimpleTab] = &[SimpleTab {
    name: "Equation",
    groups: &[
        &[
            E("CT.Equation.Tools.EquationInsertGallery", Kind::Label, 8),
            E("CT.Equation.Tools.EquationProfessional", Kind::Label, 7),
            E("CT.Equation.Tools.EquationLinearFormat", Kind::Label, 7),
            E("CT.Equation.Tools.EquationNormalText", Kind::Icon, 3),
        ],
        &[E("CT.Equation.Symbols.EquationSymbolsInsertGallery", Kind::Label, 6)],
        &[
            E("CT.Equation.Structures.EquationFractionGallery", Kind::Label, 6),
            E("CT.Equation.Structures.EquationScriptGallery", Kind::Label, 6),
            E("CT.Equation.Structures.EquationRadicalGallery", Kind::Label, 6),
            E("CT.Equation.Structures.EquationIntegralGallery", Kind::Icon, 5),
            E("CT.Equation.Structures.EquationLargeOperatorGallery", Kind::Icon, 5),
            E("CT.Equation.Structures.EquationDelimiterGallery", Kind::Icon, 5),
            E("CT.Equation.Structures.EquationFunctionGallery", Kind::Icon, 4),
            E("CT.Equation.Structures.EquationAccentGallery", Kind::Icon, 4),
            E("CT.Equation.Structures.EquationLimitGallery", Kind::Icon, 4),
            E("CT.Equation.Structures.EquationOperatorGallery", Kind::Icon, 3),
            E("CT.Equation.Structures.EquationMatrixGallery", Kind::Icon, 4),
        ],
    ],
}];

/// The tab's simplified groups: its table, or one derived from the classic layout (large controls
/// with their label, combos and galleries as they are, the rest as icons).
pub fn groups(tab: &Tab) -> Vec<Vec<E>> {
    if let Some(st) = SIMPLE_TABS.iter().chain(SIMPLE_CONTEXTUAL).find(|s| s.name == tab.name) {
        return st.groups.iter().map(|g| g.to_vec()).collect();
    }
    let mut out: Vec<Vec<E>> = Vec::new();
    let mut last = "";
    for c in controls(tab) {
        let kind = match (c.combo, c.gallery, c.large) {
            (Some(w), _, _) => Kind::Combo(w),
            (_, Some(_), _) => Kind::Gallery,
            (_, _, true) => Kind::Label,
            _ => Kind::Icon,
        };
        if c.group != last || out.is_empty() {
            out.push(Vec::new());
            last = c.group;
        }
        if let Some(g) = out.last_mut() {
            g.push(E(c.id, kind, 5));
        }
    }
    out
}

/// Whether a control has a drop-down (children, a gallery, or its own content).
fn has_menu(c: Option<&Control>, id: &str) -> bool {
    c.is_some_and(|c| !c.kids.is_empty() || c.gallery.is_some()) || rb::special_menu(id)
}

/// A control's label in the row.
fn text(id: &str) -> String {
    rb::label(id)
}

/// The width an entry takes in the row.
fn width(ui: &Ui, e: &E, c: Option<&Control>) -> f32 {
    if e.0 == crate::ink::PENS_ID {
        return crate::ink::gallery_width();
    }
    let menu = if has_menu(c, e.0) { ARROW } else { 0.0 };
    let font = TypeRung::Body.font(ui.ctx(), Weight::Regular);
    let label_w = |s: String| ui.painter().layout_no_wrap(s, font.clone(), egui::Color32::PLACEHOLDER).size().x;
    match e.1 {
        Kind::Icon => size::BUTTON + menu,
        Kind::Label | Kind::Gallery => size::BUTTON + label_w(text(e.0)) + 6.0 + menu,
        Kind::Combo(w) => w + 6.0,
    }
}

/// Draw the simplified row for `tab` inside `rect`, with the overflow menu at its end.
pub fn show(app: &mut NoteApp, ui: &mut Ui, tab: &Tab, rect: Rect, t: Tokens) {
    let all = controls(tab);
    let find = |id: &str| all.iter().find(|c| c.id == id).copied();
    let groups = groups(tab);
    let listed: HashSet<&str> = groups.iter().flatten().map(|e| e.0).collect();
    let unlisted: Vec<Control> = all.iter().filter(|c| !listed.contains(c.id)).copied().collect();
    // Decide what stays in the row: drop the lowest priority (the rightmost among equals) until
    // the row and the overflow button fit.
    let widths: Vec<Vec<f32>> = groups.iter().map(|g| g.iter().map(|e| width(ui, e, find(e.0).as_ref())).collect()).collect();
    let mut shown: Vec<Vec<bool>> = groups.iter().map(|g| vec![true; g.len()]).collect();
    let avail = rect.width() - size::BUTTON - 4.0;
    loop {
        let mut total = 0.0;
        let mut visible_groups: usize = 0;
        for (gi, g) in widths.iter().enumerate() {
            let w: f32 = g.iter().zip(&shown[gi]).filter(|(_, s)| **s).map(|(w, _)| *w + 2.0).sum();
            if w > 0.0 {
                total += w;
                visible_groups += 1;
            }
        }
        total += SEPARATOR * visible_groups.saturating_sub(1) as f32;
        if total <= avail {
            break;
        }
        let victim = groups
            .iter()
            .enumerate()
            .flat_map(|(gi, g)| g.iter().enumerate().map(move |(ei, e)| (gi, ei, e.2)))
            .filter(|(gi, ei, _)| shown[*gi][*ei])
            .min_by_key(|(gi, ei, prio)| (*prio, std::cmp::Reverse((*gi, *ei))));
        match victim {
            Some((gi, ei, _)) => shown[gi][ei] = false,
            None => break,
        }
    }
    ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
    let mut first = true;
    for (gi, g) in groups.iter().enumerate() {
        if !shown[gi].iter().any(|s| *s) {
            continue;
        }
        if !first {
            let (r, _) = ui.allocate_exact_size(vec2(SEPARATOR - 2.0, size::BUTTON), Sense::hover());
            ui.painter().line_segment([pos2(r.center().x, r.min.y + 5.0), pos2(r.center().x, r.max.y - 5.0)], Stroke::new(1.0, t.border));
        }
        first = false;
        for (ei, e) in g.iter().enumerate() {
            if shown[gi][ei] {
                entry(app, ui, e, find(e.0).as_ref(), t);
            }
        }
    }
    let hidden: Vec<E> = groups
        .iter()
        .enumerate()
        .flat_map(|(gi, g)| g.iter().enumerate().filter(|(ei, _)| !shown[gi][*ei]).map(|(_, e)| *e).collect::<Vec<_>>())
        .collect();
    if hidden.is_empty() && unlisted.is_empty() {
        return;
    }
    let (r, resp) = ui.allocate_exact_size(vec2(size::BUTTON, size::BUTTON), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "More commands"));
    if resp.hovered() {
        ui.painter().rect_filled(r, radius::CONTROL, t.hover);
    }
    for dx in [-5.0, 0.0, 5.0] {
        ui.painter().circle_filled(r.center() + vec2(dx, 0.0), 1.4, t.icon);
    }
    let resp = resp.on_hover_text("More commands");
    Popup::menu(&resp).show(|ui| {
        ui.set_min_width(220.0);
        egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
            for e in &hidden {
                overflow_item(app, ui, e.0, find(e.0).as_ref(), t);
            }
            let mut group = "";
            for c in &unlisted {
                if c.group != group {
                    if !hidden.is_empty() || !group.is_empty() {
                        ui.separator();
                    }
                    ui.label(RichText::new(c.group).size(TypeRung::Caption.size()).color(t.text_dim));
                    group = c.group;
                }
                overflow_item(app, ui, c.id, Some(c), t);
            }
        });
    });
}

/// The drop-down content of a control: its own content (colour grids…), its children, its
/// gallery.
fn drop_content(app: &mut NoteApp, ui: &mut Ui, id: &str, c: Option<&Control>, t: Tokens) {
    ui.set_min_width(180.0);
    rb::special(app, ui, id, t);
    if let Some(c) = c {
        for k in c.kids {
            rb::menu_item(app, ui, k, t);
        }
        if let Some((items, extra)) = c.gallery {
            rb::gallery_menu(app, ui, id, items, extra, t);
        }
    }
}

/// A control inside the overflow menu: a submenu when it has a drop-down, its box for a combo, a
/// menu item otherwise.
fn overflow_item(app: &mut NoteApp, ui: &mut Ui, id: &str, c: Option<&Control>, t: Tokens) {
    if let Some(w) = c.and_then(|c| c.combo) {
        ui.horizontal(|ui| {
            ui.label(text(id));
            rb::combo(app, ui, id, w, t);
        });
    } else if has_menu(c, id) {
        let on = rb::enabled(app, id) || c.is_some_and(|c| c.kids.iter().any(|k| rb::enabled(app, k)) || c.gallery.is_some()) || rb::special_menu(id);
        ui.add_enabled_ui(on, |ui| {
            ui.menu_button(text(id), |ui| drop_content(app, ui, id, c, t));
        });
    } else {
        rb::menu_item(app, ui, id, t);
    }
}

/// One control in the row.
fn entry(app: &mut NoteApp, ui: &mut Ui, e: &E, c: Option<&Control>, t: Tokens) {
    let id = e.0;
    if let Kind::Combo(w) = e.1 {
        rb::combo(app, ui, id, w, t);
        return;
    }
    if id == crate::ink::PENS_ID {
        crate::ink::gallery(app, ui, size::BUTTON, c.and_then(|c| c.gallery.map(|g| g.1)).unwrap_or(&[]), t);
        return;
    }
    let lab = text(id);
    let menu = has_menu(c, id);
    let w = width(ui, e, c);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, size::BUTTON), Sense::click());
    let main_runs = rb::enabled(app, id) && !rb::special_menu(id) && e.1 != Kind::Gallery && !id.ends_with("PasteGallery");
    let on = main_runs
        || rb::enabled(app, id)
        || c.is_some_and(|c| c.kids.iter().any(|k| rb::enabled(app, k)) || c.gallery.is_some()) && app.session.page().is_some()
        || rb::special_menu(id) && app.session.page().is_some();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, on, &lab));
    let is_checked = rb::command(id).is_some_and(|cmd| rb::checked(app, cmd));
    let p = ui.painter();
    if is_checked {
        p.rect_filled(rect, radius::CONTROL, t.checked);
    } else if on && resp.is_pointer_button_down_on() {
        p.rect_filled(rect, radius::CONTROL, t.pressed);
    } else if on && resp.hovered() {
        p.rect_filled(rect, radius::CONTROL, t.hover);
    }
    let color = if on { t.icon } else { t.text_disabled };
    let icon = Rect::from_center_size(pos2(rect.min.x + size::BUTTON / 2.0, rect.center().y), vec2(size::ICON, size::ICON));
    icons::draw(p, icon, id, color, if on { t.accent } else { color }, t.ribbon_card);
    if matches!(e.1, Kind::Label | Kind::Gallery) {
        let font = TypeRung::Body.font(ui.ctx(), Weight::Regular);
        p.text(pos2(rect.min.x + size::BUTTON - 2.0, rect.center().y), Align2::LEFT_CENTER, &lab, font, if on { t.text } else { t.text_disabled });
    }
    let arrow = Rect::from_min_max(pos2(rect.max.x - ARROW, rect.min.y), rect.max);
    if menu {
        icons::chevron(p, Rect::from_center_size(arrow.center(), vec2(11.0, 11.0)), icons::Dir::Down, color);
    }
    let resp = resp.on_hover_text(rb::tooltip(app, id));
    if !menu {
        if resp.clicked() && on {
            rb::fire(app, ui, id);
        }
        return;
    }
    // Split: the body runs the command, the ▾ opens the drop-down; a menu-only control opens the
    // drop-down from anywhere.
    let on_arrow = resp.interact_pointer_pos().is_some_and(|p| arrow.contains(p));
    if resp.clicked() && main_runs && !on_arrow {
        rb::fire(app, ui, id);
        return;
    }
    Popup::menu(&resp).show(|ui| drop_content(app, ui, id, c, t));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ribbon_layout::{CONTEXTUAL, TABS};

    /// UI.Ribbon.Simplified: every control of every classic tab is reachable in the simplified
    /// ribbon — placed in the row (with its drop-down) or listed in the overflow menu — and every
    /// placed id is a control of that tab, so its label, command and drop-down are the classic
    /// ones.
    #[test]
    fn every_classic_control_is_reachable_in_the_simplified_ribbon() {
        for tab in TABS.iter().chain(CONTEXTUAL) {
            let classic = controls(tab);
            let ids: HashSet<&str> = classic.iter().map(|c| c.id).collect();
            let placed: Vec<E> = groups(tab).into_iter().flatten().collect();
            for e in &placed {
                assert!(ids.contains(e.0), "{}: simplified places {} which the classic tab lacks", tab.name, e.0);
                assert!(crate::ribbon::row(e.0).is_some(), "{}: {} is not in the catalog", tab.name, e.0);
            }
            let mut seen = HashSet::new();
            for e in &placed {
                assert!(seen.insert(e.0), "{}: {} placed twice", tab.name, e.0);
            }
            // Placed or (by construction) in the overflow menu: the union is the classic tab.
            let unlisted = classic.iter().filter(|c| !seen.contains(c.id)).count();
            assert_eq!(placed.len() + unlisted, classic.len(), "{}: every classic control is reachable", tab.name);
        }
        assert_eq!(SIMPLE_TABS.len(), TABS.len(), "UI.Ribbon.Simplified");
    }
}
