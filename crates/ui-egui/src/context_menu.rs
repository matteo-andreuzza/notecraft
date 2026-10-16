//! Right-click context menus on text, note containers and paragraph handles (catalog
//! `CM.Text.*`, `CM.NoteContainer.*`, `CM.ParagraphHandle.*`). Built straight from the catalog's
//! own rows (menu structure mirrors their `Path` column) instead of a hand-kept copy, so a
//! catalog edit can't drift out of sync with the menu; [`tests::menus_cover_every_context_row`]
//! checks it both ways.
//!
//! Every entry is one of the ribbon's own catalog controls, rendered with the same `pub(crate)`
//! helpers (`ribbon::menu_item`, `ribbon::special`) — same commands, `enabled`, tooltips. A
//! control whose command isn't registered yet (ink, spelling, OCR, linked notes, rotation of
//! objects…) is drawn **disabled** with the reason in its tooltip (AGENTS.md › No fake UI);
//! nothing here invents new behaviour.
//!
//! **Which menu opens where** (`show`, called once from `canvas.rs` on the whole-canvas
//! response): the paragraph handle if the click is on one, else the note container's drag bar,
//! else the text/cell under the pointer (`canvas::hit_para`, which already resolves table cells
//! too — so a cell's right-click gets the Text menu with a working Table submenu). The click also
//! places the caret or selects the container there, as a left click would, unless the pointer is
//! already inside an existing text selection in the same block (so Cut/Copy of a selection still
//! works from a right click inside it).

use egui::{Pos2, Ui};
use notecraft_engine::selection::Block;
use serde_json::json;

use crate::canvas::{self, ContextTarget};
use crate::ribbon;
use crate::{NoteApp, Tokens};

/// One menu entry: a catalog row, or a submenu whose header is a catalog row (its label and
/// enabled state come from that row) holding more entries.
pub enum Entry {
    /// A catalog row rendered as a single control.
    Item(&'static str),
    /// A submenu: the header catalog id, then its entries.
    Submenu(&'static str, &'static [Entry]),
}

/// Right-click menu on page text (and table cells; catalog `CM.Text.*`).
pub const TEXT_MENU: &[Entry] = &[
    Entry::Item("CM.Text.NoSpellingSuggestions"),
    Entry::Item("CM.Text.InkAlternate1"),
    Entry::Item("CM.Text.InkAlternate2"),
    Entry::Item("CM.Text.InkAlternate3"),
    Entry::Item("CM.Text.InkAlternate4"),
    Entry::Item("CM.Text.InkAlternate5"),
    Entry::Item("CM.Text.MoreInkResults"),
    Entry::Item("CM.Text.Cut"),
    Entry::Item("CM.Text.CopyOneNote"),
    Entry::Item("CM.Text.PasteGalleryMini"),
    Entry::Item("CM.Text.HyperlinkOpen"),
    Entry::Item("CM.Text.LinkEdit"),
    Entry::Item("CM.Text.HyperlinkCreate"),
    Entry::Item("CM.Text.ToPageLink"),
    Entry::Item("CM.Text.LinkCopyOneNote"),
    Entry::Item("CM.Text.LinkSelect"),
    Entry::Item("CM.Text.LinkRemove"),
    Entry::Item("CM.Text.CopyTextFromPicture"),
    Entry::Item("CM.Text.PrintoutPageTextCopy"),
    Entry::Item("CM.Text.CopyTextFromPrintoutPages"),
    Entry::Submenu(
        "CM.Text.RotateInkMenu",
        &[
            Entry::Item("CM.Text.RotateRight45OneNote"),
            Entry::Item("CM.Text.RotateLeft45OneNote"),
            Entry::Item("CM.Text.ObjectRotateRight90"),
            Entry::Item("CM.Text.ObjectRotateLeft90"),
            Entry::Item("CM.Text.ObjectFlipHorizontal"),
            Entry::Item("CM.Text.ObjectFlipVertical"),
        ],
    ),
    Entry::Submenu("CM.Text.TreatInkAsMenu", &[Entry::Item("CM.Text.TreatInkAsHandwriting"), Entry::Item("CM.Text.TreatInkAsDrawing")]),
    Entry::Submenu("CM.Text.ConvertInkMenu", &[Entry::Item("CM.Text.SelectionToTextConvert"), Entry::Item("CM.Text.SelectionToMathConvert")]),
    Entry::Item("CM.Text.LookUp"),
    Entry::Item("CM.Text.Translate"),
    Entry::Item("CM.Text.PictureAsBackground"),
    Entry::Item("CM.Text.AlternativeText"),
    Entry::Item("CM.Text.OCRLanguageListMenu"),
    Entry::Submenu(
        "CM.Text.TableMenu",
        &[
            Entry::Item("CM.Text.TableInsert"),
            Entry::Item("CM.Text.TableInsertColumnsLeft"),
            Entry::Item("CM.Text.TableInsertColumnsRight"),
            Entry::Item("CM.Text.TableInsertRowsAbove"),
            Entry::Item("CM.Text.TableInsertRowsBelow"),
            Entry::Item("CM.Text.DeleteTableOneNote"),
            Entry::Item("CM.Text.DeleteColumnsOneNote"),
            Entry::Item("CM.Text.DeleteRowsOneNote"),
            Entry::Item("CM.Text.SelectTableOneNote"),
            Entry::Item("CM.Text.SelectColumns"),
            Entry::Item("CM.Text.SelectRows"),
            Entry::Item("CM.Text.SelectCell"),
            Entry::Item("CM.Text.TableBordersHide"),
            Entry::Item("CM.Text.ShadingColorPicker"),
            Entry::Item("CM.Text.SortUp"),
            Entry::Item("CM.Text.SortDown"),
            Entry::Item("CM.Text.ShowSortTableRowsDialog"),
            Entry::Item("CM.Text.HeaderRowNoFormatting"),
        ],
    ),
    Entry::Item("CM.Text.ConvertToExcelSpreadsheet"),
    Entry::Submenu(
        "CM.Text.LinkedNotesMenu",
        &[
            Entry::Item("CM.Text.LinkedFileOpen"),
            Entry::Item("CM.Text.LinkedNotesTargetUrlCopy"),
            Entry::Item("CM.Text.NoteLinkRepairLink"),
            Entry::Item("CM.Text.NoteLinkReLink"),
            Entry::Item("CM.Text.NoteLinkDelete"),
            Entry::Item("CM.Text.LinkedNotesOptions"),
        ],
    ),
    Entry::Item("CM.Text.LastModifiedBy"),
];

/// Right-click menu on a note container's drag bar (catalog `CM.NoteContainer.*`).
pub const CONTAINER_MENU: &[Entry] = &[
    Entry::Item("CM.NoteContainer.Cut"),
    Entry::Item("CM.NoteContainer.CopyOneNote"),
    Entry::Item("CM.NoteContainer.PasteGalleryMini"),
    Entry::Item("CM.NoteContainer.Delete"),
    Entry::Item("CM.NoteContainer.HyperlinkCreate"),
    Entry::Submenu(
        "CM.NoteContainer.SelectOutlineLevelsMenu",
        &[
            Entry::Item("CM.NoteContainer.OutlineSelectLevel1"),
            Entry::Item("CM.NoteContainer.OutlineSelectLevel2"),
            Entry::Item("CM.NoteContainer.OutlineSelectLevel3"),
            Entry::Item("CM.NoteContainer.OutlineSelectLevel4"),
            Entry::Item("CM.NoteContainer.OutlineSelectLevel5"),
            Entry::Item("CM.NoteContainer.OutlineSelectLevelAll"),
        ],
    ),
    Entry::Submenu(
        "CM.NoteContainer.OrderObjectsMenu",
        &[
            Entry::Item("CM.NoteContainer.ObjectBringForward"),
            Entry::Item("CM.NoteContainer.ObjectBringToFront"),
            Entry::Item("CM.NoteContainer.ObjectSendBackward"),
            Entry::Item("CM.NoteContainer.ObjectSendToBack"),
        ],
    ),
    Entry::Item("CM.NoteContainer.ObjectMove"),
    Entry::Item("CM.NoteContainer.ObjectResize"),
    Entry::Submenu(
        "CM.NoteContainer.ConvertInkMenu",
        &[Entry::Item("CM.NoteContainer.SelectionToTextConvert"), Entry::Item("CM.NoteContainer.SelectionToMathConvert")],
    ),
];

/// Right-click menu on a paragraph handle (catalog `CM.ParagraphHandle.*`).
pub const HANDLE_MENU: &[Entry] = &[
    Entry::Item("CM.ParagraphHandle.Cut"),
    Entry::Item("CM.ParagraphHandle.CopyOneNote"),
    Entry::Item("CM.ParagraphHandle.PasteGalleryMini"),
    Entry::Item("CM.ParagraphHandle.Delete"),
    Entry::Item("CM.ParagraphHandle.HyperlinkCreate"),
    Entry::Item("CM.ParagraphHandle.CopyTextFromPicture"),
    Entry::Item("CM.ParagraphHandle.AlternativeText"),
    Entry::Item("CM.ParagraphHandle.ImageSaveAs"),
    Entry::Submenu(
        "CM.ParagraphHandle.TableMenu",
        &[
            Entry::Item("CM.ParagraphHandle.TableInsert"),
            Entry::Item("CM.ParagraphHandle.TableInsertColumnsLeft"),
            Entry::Item("CM.ParagraphHandle.TableInsertColumnsRight"),
            Entry::Item("CM.ParagraphHandle.TableInsertRowsAbove"),
            Entry::Item("CM.ParagraphHandle.TableInsertRowsBelow"),
            Entry::Item("CM.ParagraphHandle.DeleteTableOneNote"),
            Entry::Item("CM.ParagraphHandle.DeleteColumnsOneNote"),
            Entry::Item("CM.ParagraphHandle.DeleteRowsOneNote"),
            Entry::Item("CM.ParagraphHandle.SelectTableOneNote"),
            Entry::Item("CM.ParagraphHandle.SelectColumns"),
            Entry::Item("CM.ParagraphHandle.SelectRows"),
            Entry::Item("CM.ParagraphHandle.SelectCell"),
            Entry::Item("CM.ParagraphHandle.TableBordersHide"),
            Entry::Item("CM.ParagraphHandle.ShadingColorPicker"),
            Entry::Item("CM.ParagraphHandle.SortUp"),
            Entry::Item("CM.ParagraphHandle.SortDown"),
            Entry::Item("CM.ParagraphHandle.ShowSortTableRowsDialog"),
            Entry::Item("CM.ParagraphHandle.HeaderRowNoFormatting"),
        ],
    ),
    Entry::Submenu(
        "CM.ParagraphHandle.SelectOutlineLevelsMenu",
        &[
            Entry::Item("CM.ParagraphHandle.OutlineSelectLevel1"),
            Entry::Item("CM.ParagraphHandle.OutlineSelectLevel2"),
            Entry::Item("CM.ParagraphHandle.OutlineSelectLevel3"),
            Entry::Item("CM.ParagraphHandle.OutlineSelectLevel4"),
            Entry::Item("CM.ParagraphHandle.OutlineSelectLevel5"),
            Entry::Item("CM.ParagraphHandle.OutlineSelectLevelAll"),
        ],
    ),
    Entry::Submenu(
        "CM.ParagraphHandle.ConvertInkMenu",
        &[Entry::Item("CM.ParagraphHandle.SelectionToTextConvert"), Entry::Item("CM.ParagraphHandle.SelectionToMathConvert")],
    ),
    Entry::Submenu(
        "CM.ParagraphHandle.LinkedNotesMenu",
        &[
            Entry::Item("CM.ParagraphHandle.LinkedFileOpen"),
            Entry::Item("CM.ParagraphHandle.LinkedNotesTargetUrlCopy"),
            Entry::Item("CM.ParagraphHandle.NoteLinkRepairLink"),
            Entry::Item("CM.ParagraphHandle.NoteLinkReLink"),
            Entry::Item("CM.ParagraphHandle.NoteLinkDelete"),
            Entry::Item("CM.ParagraphHandle.LinkedNotesOptions"),
        ],
    ),
    Entry::Item("CM.ParagraphHandle.OCRLanguageListMenu"),
    Entry::Item("CM.ParagraphHandle.LastModifiedBy"),
];

/// Every catalog id an entry (and its submenu, if any) reaches, for the coverage test.
pub fn ids(e: &Entry) -> Vec<&'static str> {
    match e {
        Entry::Item(id) => vec![id],
        Entry::Submenu(header, children) => std::iter::once(*header).chain(children.iter().flat_map(ids)).collect(),
    }
}

/// Whether `e` (or something under it) has a working command — a submenu with every child
/// disabled is itself drawn disabled, rather than opening onto nothing but grey text.
fn entry_enabled(app: &NoteApp, e: &Entry) -> bool {
    match e {
        Entry::Item(id) => ribbon::enabled(app, id) || ribbon::special_menu(id) && app.session.page().is_some(),
        Entry::Submenu(header, children) => ribbon::enabled(app, header) || children.iter().any(|c| entry_enabled(app, c)),
    }
}

/// Draw one entry: a catalog control (`ribbon::menu_item`), or — when it's a colour/shading
/// picker (`ribbon::special_menu`) — a submenu whose content is `ribbon::special`.
fn render_item(app: &mut NoteApp, ui: &mut Ui, id: &'static str, t: Tokens) {
    if ribbon::special_menu(id) {
        let on = ribbon::enabled(app, id);
        ui.add_enabled_ui(on, |ui| {
            ui.menu_button(ribbon::label(id), |ui| {
                ui.set_min_width(170.0);
                ribbon::special(app, ui, id, t);
            });
        })
        .response
        .on_hover_text(ribbon::tooltip(app, id));
        return;
    }
    ribbon::menu_item(app, ui, id, t);
}

/// Draw a list of entries, in catalog order.
pub(crate) fn render(app: &mut NoteApp, ui: &mut Ui, entries: &[Entry], t: Tokens) {
    for e in entries {
        match e {
            Entry::Item(id) => render_item(app, ui, id, t),
            Entry::Submenu(header, children) => {
                let on = entry_enabled(app, e);
                ui.add_enabled_ui(on, |ui| {
                    ui.menu_button(ribbon::label(header), |ui| {
                        ui.set_min_width(190.0);
                        render(app, ui, children, t);
                    });
                });
            }
        }
    }
}

/// The target under a screen point: the note container's drag bar, a paragraph handle, or the
/// text (or table cell) there (`canvas::hit_para`). `None` over empty canvas.
fn detect(app: &NoteApp, p: Pos2) -> Option<ContextTarget> {
    if let Some(o) = app.canvas.outlines.iter().find(|o| o.bar.contains(p)) {
        return Some(ContextTarget::Container(o.id));
    }
    if let Some(pl) = app.canvas.paras.iter().find(|pl| matches!(pl.block, Block::Outline(_)) && pl.handle.contains(p)) {
        return Some(ContextTarget::Handle(pl.block, pl.para));
    }
    let pl = canvas::hit_para(app, p)?;
    let offset = pl.offset_at(p);
    Some(ContextTarget::Text(pl.block, pl.para, offset))
}

/// Whether `(block, para, offset)` falls inside the current text selection, so a right click
/// there keeps it (for Cut/Copy of a deliberate selection) instead of collapsing the caret.
fn inside_selection(app: &NoteApp, block: Block, para: usize, offset: usize) -> bool {
    let Some(r) = app.session.text_range() else { return false };
    if r.is_caret() || r.anchor.block != block {
        return false;
    }
    let (a, b) = r.ordered();
    (a.para, a.offset) <= (para, offset) && (para, offset) <= (b.para, b.offset)
}

fn block_param(b: Block) -> String {
    match b {
        Block::Title => "title".to_string(),
        Block::Outline(id) | Block::Cell(id) => id.to_string(),
    }
}

/// Place the caret or selection a right click's target implies, as a left click would — unless
/// the pointer landed inside an existing text selection, which a right click leaves alone.
fn place_for(app: &mut NoteApp, ctx: &egui::Context, target: ContextTarget) {
    match target {
        ContextTarget::Container(id) => {
            app.run(ctx, "object.select", json!({"ids": [id.to_string()]}));
        }
        ContextTarget::Handle(block, para) => {
            if let Block::Outline(id) = block {
                app.run(ctx, "caret.set", json!({"block": id.to_string(), "para": para, "offset": 0}));
                app.run(ctx, "outline.selectBranch", json!({}));
            }
        }
        ContextTarget::Text(block, para, offset) => {
            if !inside_selection(app, block, para, offset) {
                app.run(ctx, "caret.set", json!({"block": block_param(block), "para": para, "offset": offset}));
            }
        }
    }
}

/// Open the right-click context menu for the canvas (`resp` is the whole-canvas response):
/// detect the target on a fresh secondary click, place the caret/selection there, then keep
/// showing the matching menu while it stays open.
pub fn show(app: &mut NoteApp, ui: &Ui, resp: &egui::Response, t: Tokens) {
    // Only ever move to a *new* target, never clear one: kittest's settle passes can replay a
    // secondary click at a point that no longer resolves (geometry shifted a hair between
    // passes), which must not silently close an already-open menu.
    if resp.secondary_clicked()
        && let Some(p) = resp.interact_pointer_pos()
        && let Some(target) = detect(app, p)
    {
        let ctx = ui.ctx().clone();
        app.canvas.context_target = Some(target);
        place_for(app, &ctx, target);
    }
    let Some(target) = app.canvas.context_target else { return };
    resp.context_menu(|ui| {
        ui.set_min_width(190.0);
        match target {
            ContextTarget::Container(_) => render(app, ui, CONTAINER_MENU, t),
            ContextTarget::Handle(..) => render(app, ui, HANDLE_MENU, t),
            ContextTarget::Text(..) => render(app, ui, TEXT_MENU, t),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menus_cover_every_context_row() {
        // This only checks placement (every catalog row reachable, none twice, none orphaned):
        // no `// parity:` token here, so it never claims a row's documented behaviour is pinned.
        let rows = notecraft_engine::catalog::rows();
        for (prefix, menu) in [("CM.Text.", TEXT_MENU), ("CM.NoteContainer.", CONTAINER_MENU), ("CM.ParagraphHandle.", HANDLE_MENU)] {
            let mut placed: Vec<&str> = menu.iter().flat_map(ids).collect();
            let n = placed.len();
            placed.sort_unstable();
            placed.dedup();
            assert_eq!(placed.len(), n, "{prefix}: a row is placed twice");
            for id in &placed {
                assert!(rows.iter().any(|r| r.id == *id), "{id} is not a catalog row");
            }
            for r in rows.iter().filter(|r| r.id.starts_with(prefix)) {
                assert!(placed.contains(&r.id.as_str()), "catalog row {} is not placed in its context menu", r.id);
            }
        }
    }
}
