//! Ribbon and pane icons, drawn in code.
//!
//! Every icon is original geometry painted with egui shapes — no image files, no glyphs copied
//! from any other product (AGENTS.md › Clean room). Each takes a square `rect` and a colour, so
//! icons scale with the UI and follow the theme.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};

fn p(r: Rect, x: f32, y: f32) -> Pos2 {
    pos2(r.min.x + x * r.width(), r.min.y + y * r.height())
}

fn line(pa: &Painter, r: Rect, pts: &[(f32, f32)], s: Stroke) {
    pa.add(Shape::line(pts.iter().map(|&(x, y)| p(r, x, y)).collect(), s));
}

fn rect(pa: &Painter, r: Rect, x0: f32, y0: f32, x1: f32, y1: f32, fill: Color32, s: Stroke) {
    let rr = Rect::from_min_max(p(r, x0, y0), p(r, x1, y1));
    if fill != Color32::TRANSPARENT {
        pa.rect_filled(rr, 1.0, fill);
    }
    pa.rect_stroke(rr, 1.0, s, egui::StrokeKind::Inside);
}

fn text(pa: &Painter, r: Rect, t: &str, scale: f32, color: Color32, bold: bool) {
    let size = r.height() * scale;
    let weight = if bold { crate::theme::Weight::Semibold } else { crate::theme::Weight::Regular };
    let font = crate::theme::font(pa.ctx(), size, weight);
    pa.text(r.center(), egui::Align2::CENTER_CENTER, t, font, color);
}

/// The icon key for a catalog id (its control name), or a group of related controls.
pub fn key_for(id: &str) -> &str {
    id.rsplit('.').next().unwrap_or(id)
}

/// Draw the icon for catalog id `id` into `r`.
pub fn draw(pa: &Painter, r: Rect, id: &str, color: Color32) {
    let s = Stroke::new((r.width() / 14.0).max(1.0), color);
    let thin = Stroke::new((r.width() / 20.0).max(1.0), color);
    let accent = Color32::from_rgb(0x8a, 0x4f, 0xc8);
    let k = key_for(id);
    match k {
        "PasteMenu" | "Paste" | "PasteKeepSource" | "PasteMerge" | "PasteText" | "PastePicture" | "PasteGallery" => {
            // Clipboard board with a sheet.
            rect(pa, r, 0.18, 0.18, 0.72, 0.92, Color32::from_rgb(0xd9, 0xb8, 0x7a), s);
            rect(pa, r, 0.34, 0.1, 0.56, 0.24, color, s);
            rect(pa, r, 0.42, 0.38, 0.88, 0.94, Color32::WHITE, s);
            if k == "PasteText" {
                text(pa, Rect::from_min_max(p(r, 0.42, 0.38), p(r, 0.88, 0.94)), "A", 0.7, color, false);
            }
        }
        "Cut" => {
            line(pa, r, &[(0.3, 0.1), (0.62, 0.62)], s);
            line(pa, r, &[(0.7, 0.1), (0.38, 0.62)], s);
            pa.circle_stroke(p(r, 0.3, 0.76), r.width() * 0.13, s);
            pa.circle_stroke(p(r, 0.7, 0.76), r.width() * 0.13, s);
        }
        "CopyOneNote" => {
            rect(pa, r, 0.14, 0.1, 0.62, 0.66, Color32::TRANSPARENT, s);
            rect(pa, r, 0.36, 0.32, 0.86, 0.9, Color32::WHITE, s);
        }
        "FormatPainter" => {
            rect(pa, r, 0.12, 0.12, 0.78, 0.38, Color32::from_rgb(0xf4, 0xc4, 0x30), s);
            line(pa, r, &[(0.78, 0.25), (0.9, 0.25), (0.9, 0.5), (0.5, 0.5), (0.5, 0.62)], s);
            rect(pa, r, 0.42, 0.62, 0.58, 0.92, color, s);
        }
        "Bold" => text(pa, r, "B", 0.8, color, true),
        "Italic" => text(pa, r, "I", 0.8, color, false),
        "Underline" => {
            text(pa, Rect::from_min_max(r.min, p(r, 1.0, 0.85)), "U", 0.7, color, false);
            line(pa, r, &[(0.2, 0.92), (0.8, 0.92)], s);
        }
        "Strikethrough" => {
            text(pa, r, "ab", 0.65, color, false);
            line(pa, r, &[(0.1, 0.52), (0.9, 0.52)], thin);
        }
        "ExtendedTextFormattingMenu" | "Subscript" | "ExtendedTextFormattingMostRecenltyUsed" => {
            text(pa, Rect::from_min_max(r.min, p(r, 0.7, 1.0)), "x", 0.75, color, false);
            text(pa, Rect::from_min_max(p(r, 0.55, 0.5), r.max), "2", 0.45, color, false);
        }
        "Superscript" => {
            text(pa, Rect::from_min_max(r.min, p(r, 0.7, 1.0)), "x", 0.75, color, false);
            text(pa, Rect::from_min_max(p(r, 0.55, 0.0), p(r, 1.0, 0.5)), "2", 0.45, color, false);
        }
        "TextHighlightColorPicker" | "HighlightColorNone" => {
            line(pa, r, &[(0.25, 0.75), (0.7, 0.15), (0.85, 0.28), (0.4, 0.86), (0.25, 0.75)], s);
            rect(pa, r, 0.08, 0.86, 0.92, 0.98, Color32::from_rgb(0xff, 0xff, 0x00), Stroke::NONE);
        }
        "FontColorPicker" => {
            text(pa, Rect::from_min_max(r.min, p(r, 1.0, 0.85)), "A", 0.75, color, false);
            rect(pa, r, 0.12, 0.86, 0.88, 0.98, Color32::from_rgb(0xc0, 0x00, 0x00), Stroke::NONE);
        }
        "BulletsGallery" => {
            for (i, y) in [0.22, 0.5, 0.78].iter().enumerate() {
                pa.circle_filled(p(r, 0.18, *y), r.width() * 0.07, color);
                line(pa, r, &[(0.36, *y), (0.92, *y)], if i == 0 { s } else { thin });
            }
        }
        "NumberingGallery" | "NumberingCustomize" => {
            for (i, y) in [0.22, 0.5, 0.78].iter().enumerate() {
                text(pa, Rect::from_center_size(p(r, 0.16, *y), vec2(r.width() * 0.3, r.height() * 0.3)), &format!("{}", i + 1), 0.9, color, false);
                line(pa, r, &[(0.36, *y), (0.92, *y)], thin);
            }
        }
        "OutdentClassic" | "IndentClassic" => {
            for y in [0.15, 0.38, 0.62, 0.85] {
                let x0 = if (0.3..0.7).contains(&y) { 0.48 } else { 0.1 };
                line(pa, r, &[(x0, y), (0.92, y)], thin);
            }
            if k == "IndentClassic" {
                line(pa, r, &[(0.08, 0.36), (0.3, 0.5), (0.08, 0.64)], s);
            } else {
                line(pa, r, &[(0.3, 0.36), (0.08, 0.5), (0.3, 0.64)], s);
            }
        }
        "ClearFormatting" => {
            text(pa, Rect::from_min_max(r.min, p(r, 0.7, 0.8)), "A", 0.7, color, false);
            line(pa, r, &[(0.5, 0.95), (0.9, 0.55)], Stroke::new(s.width, Color32::from_rgb(0xd0, 0x40, 0x60)));
            line(pa, r, &[(0.62, 0.62), (0.82, 0.82)], Stroke::new(s.width, Color32::from_rgb(0xd0, 0x40, 0x60)));
        }
        "ParagraphAlignmentMenu"
        | "AlignLeft"
        | "AlignCenter"
        | "AlignRight"
        | "ParagraphSpacingOptions"
        | "TextDirectionLeftToRight"
        | "TextDirectionRightToLeft" => {
            for (i, y) in [0.18, 0.4, 0.62, 0.84].iter().enumerate() {
                let w = if i % 2 == 0 { 0.8 } else { 0.55 };
                let (a, b) = match k {
                    "AlignCenter" => (0.5 - w / 2.0, 0.5 + w / 2.0),
                    "AlignRight" | "TextDirectionRightToLeft" => (0.9 - w, 0.9),
                    _ => (0.1, 0.1 + w),
                };
                line(pa, r, &[(a, *y), (b, *y)], thin);
            }
        }
        "Delete" | "DeleteTableOneNote" | "DeleteColumnsOneNote" | "DeleteRowsOneNote" => {
            let red = Color32::from_rgb(0xd1, 0x34, 0x38);
            line(pa, r, &[(0.2, 0.2), (0.8, 0.8)], Stroke::new(s.width * 1.3, red));
            line(pa, r, &[(0.8, 0.2), (0.2, 0.8)], Stroke::new(s.width * 1.3, red));
        }
        "QuickStylesGallery" => text(pa, r, "Aa", 0.6, color, false),
        "TagsGallery" | "ToDoTag" | "TagsCustomize" | "TagsRemoveAll" => {
            rect(pa, r, 0.18, 0.18, 0.82, 0.82, Color32::WHITE, s);
            if k != "TagsRemoveAll" {
                line(pa, r, &[(0.3, 0.5), (0.45, 0.66), (0.72, 0.32)], Stroke::new(s.width * 1.2, accent));
            }
        }
        "FindTaggedNotes" => {
            rect(pa, r, 0.1, 0.1, 0.5, 0.5, Color32::WHITE, s);
            line(pa, r, &[(0.18, 0.3), (0.28, 0.4), (0.42, 0.2)], thin);
            pa.circle_stroke(p(r, 0.62, 0.62), r.width() * 0.18, s);
            line(pa, r, &[(0.75, 0.75), (0.92, 0.92)], s);
        }
        "OutlookTasksMenu" => {
            rect(pa, r, 0.12, 0.18, 0.88, 0.86, Color32::WHITE, s);
            line(pa, r, &[(0.3, 0.52), (0.45, 0.66), (0.72, 0.36)], s);
        }
        "SendNotesViaEmail" => {
            rect(pa, r, 0.08, 0.22, 0.92, 0.8, Color32::WHITE, s);
            line(pa, r, &[(0.08, 0.22), (0.5, 0.56), (0.92, 0.22)], s);
        }
        "MeetingDetailsInsertGallery" | "DateInsert" | "MeetingDetailsOtherDayInsert" => {
            rect(pa, r, 0.12, 0.18, 0.88, 0.9, Color32::WHITE, s);
            rect(pa, r, 0.12, 0.18, 0.88, 0.36, accent, s);
            for (x, y) in [(0.3, 0.55), (0.5, 0.55), (0.7, 0.55), (0.3, 0.74), (0.5, 0.74)] {
                pa.circle_filled(p(r, x, y), r.width() * 0.04, color);
            }
        }
        "TimeInsert" | "RefreshMeetingDetails" => {
            pa.circle_stroke(r.center(), r.width() * 0.4, s);
            line(pa, r, &[(0.5, 0.25), (0.5, 0.5), (0.7, 0.62)], s);
        }
        "DateAndTimeInsertOneNote" => {
            rect(pa, r, 0.08, 0.14, 0.7, 0.76, Color32::WHITE, s);
            rect(pa, r, 0.08, 0.14, 0.7, 0.3, accent, s);
            pa.circle_filled(p(r, 0.72, 0.72), r.width() * 0.24, Color32::WHITE);
            pa.circle_stroke(p(r, 0.72, 0.72), r.width() * 0.24, s);
            line(pa, r, &[(0.72, 0.58), (0.72, 0.72), (0.82, 0.78)], thin);
        }
        "PenInsertSpace" => {
            line(pa, r, &[(0.1, 0.5), (0.9, 0.5)], Stroke::new(s.width, accent));
            line(pa, r, &[(0.5, 0.1), (0.5, 0.36)], s);
            line(pa, r, &[(0.38, 0.24), (0.5, 0.36), (0.62, 0.24)], s);
            line(pa, r, &[(0.5, 0.64), (0.5, 0.9)], s);
            line(pa, r, &[(0.38, 0.76), (0.5, 0.64), (0.62, 0.76)], s);
        }
        "TableInsertGallery" | "TableInsert" | "SelectTableOneNote" | "SelectColumns" | "SelectRows" | "SelectCell" => {
            rect(pa, r, 0.1, 0.14, 0.9, 0.86, Color32::WHITE, s);
            rect(pa, r, 0.1, 0.14, 0.9, 0.32, accent, s);
            line(pa, r, &[(0.1, 0.5), (0.9, 0.5)], thin);
            line(pa, r, &[(0.1, 0.68), (0.9, 0.68)], thin);
            line(pa, r, &[(0.36, 0.14), (0.36, 0.86)], thin);
            line(pa, r, &[(0.63, 0.14), (0.63, 0.86)], thin);
        }
        "PrintoutFromFileInsert" => {
            rect(pa, r, 0.18, 0.08, 0.82, 0.92, Color32::WHITE, s);
            for y in [0.3, 0.45, 0.6, 0.75] {
                line(pa, r, &[(0.3, y), (0.7, y)], thin);
            }
        }
        "FileAttachmentInsert" => {
            line(pa, r, &[(0.62, 0.3), (0.62, 0.72), (0.5, 0.86), (0.38, 0.72), (0.38, 0.2), (0.46, 0.1), (0.54, 0.2), (0.54, 0.66)], s);
        }
        "InsertExcelSpreadsheetMenu" | "InsertExistingExcelSpreadsheet" | "InsertNewExcelSpreadsheetWithPreview" | "ConvertToExcelSpreadsheet" => {
            rect(pa, r, 0.14, 0.1, 0.86, 0.9, Color32::WHITE, s);
            for y in [0.3, 0.5, 0.7] {
                line(pa, r, &[(0.14, y), (0.86, y)], thin);
            }
            line(pa, r, &[(0.4, 0.1), (0.4, 0.9)], thin);
            rect(pa, r, 0.4, 0.3, 0.86, 0.5, Color32::from_rgb(0x9c, 0xcf, 0x7f), Stroke::NONE);
        }
        "InsertVisioDrawingMenu" | "InsertExistingVisioDrawing" | "InsertNewVisioDrawingWithPreview" => {
            rect(pa, r, 0.1, 0.12, 0.42, 0.38, Color32::WHITE, s);
            rect(pa, r, 0.58, 0.62, 0.9, 0.88, Color32::WHITE, s);
            line(pa, r, &[(0.26, 0.38), (0.26, 0.75), (0.58, 0.75)], s);
        }
        "ScreenCaptureInsert" => {
            for (a, b) in [
                ((0.1, 0.3), (0.1, 0.1)),
                ((0.1, 0.1), (0.3, 0.1)),
                ((0.7, 0.1), (0.9, 0.1)),
                ((0.9, 0.1), (0.9, 0.3)),
                ((0.9, 0.7), (0.9, 0.9)),
                ((0.9, 0.9), (0.7, 0.9)),
                ((0.3, 0.9), (0.1, 0.9)),
                ((0.1, 0.9), (0.1, 0.7)),
            ] {
                line(pa, r, &[a, b], s);
            }
            rect(pa, r, 0.3, 0.3, 0.7, 0.7, accent, Stroke::NONE);
        }
        "PictureFromFileInsert" | "ClipArtInsertDialog" => {
            rect(pa, r, 0.08, 0.16, 0.92, 0.84, Color32::WHITE, s);
            pa.add(Shape::convex_polygon(
                vec![p(r, 0.14, 0.78), p(r, 0.42, 0.42), p(r, 0.62, 0.66), p(r, 0.72, 0.54), p(r, 0.86, 0.78)],
                Color32::from_rgb(0x5c, 0x8a, 0x3a),
                Stroke::NONE,
            ));
            pa.circle_filled(p(r, 0.7, 0.34), r.width() * 0.07, Color32::from_rgb(0xf4, 0xc4, 0x30));
            if k == "ClipArtInsertDialog" {
                pa.circle_stroke(p(r, 0.78, 0.22), r.width() * 0.16, thin);
            }
        }
        "HyperlinkCreateHere" => {
            pa.add(Shape::circle_stroke(p(r, 0.36, 0.58), r.width() * 0.18, s));
            pa.add(Shape::circle_stroke(p(r, 0.64, 0.42), r.width() * 0.18, s));
            line(pa, r, &[(0.4, 0.55), (0.6, 0.45)], s);
        }
        "AudioRecordingInsert" | "RecordingPlay" | "AudioNotePlayback" => {
            rect(pa, r, 0.38, 0.1, 0.62, 0.58, color, s);
            line(pa, r, &[(0.26, 0.42), (0.26, 0.6), (0.5, 0.74), (0.74, 0.6), (0.74, 0.42)], s);
            line(pa, r, &[(0.5, 0.74), (0.5, 0.9)], s);
        }
        "VideoRecordingInsert" | "InsertOnlineVideo" => {
            rect(pa, r, 0.08, 0.28, 0.64, 0.72, Color32::WHITE, s);
            pa.add(Shape::convex_polygon(vec![p(r, 0.66, 0.5), p(r, 0.92, 0.3), p(r, 0.92, 0.7)], color, Stroke::NONE));
        }
        "PageTemplatesSplitButton" | "PageTemplatesTaskpane" | "NewPageFromStationery" | "StationeryMostRecentlyUsedList" => {
            rect(pa, r, 0.18, 0.08, 0.82, 0.92, Color32::WHITE, s);
            rect(pa, r, 0.28, 0.2, 0.72, 0.4, accent, Stroke::NONE);
            for y in [0.55, 0.68, 0.81] {
                line(pa, r, &[(0.28, y), (0.72, y)], thin);
            }
        }
        "InsertBuildingBlocksEquationsGallery" | "EquationInsertNew" | "EquationInsertGallery" => text(pa, r, "π", 0.8, color, false),
        "SymbolInsertGallery" | "SymbolsDialog" | "EquationSymbolsInsertGallery" => text(pa, r, "Ω", 0.8, color, false),
        "Sticker" => {
            pa.circle_filled(r.center(), r.width() * 0.4, Color32::from_rgb(0xf4, 0xc4, 0x30));
            pa.circle_filled(p(r, 0.38, 0.42), r.width() * 0.05, color);
            pa.circle_filled(p(r, 0.62, 0.42), r.width() * 0.05, color);
            line(pa, r, &[(0.32, 0.6), (0.5, 0.72), (0.68, 0.6)], thin);
        }
        "SelectAndTypeMode" => {
            pa.add(Shape::convex_polygon(
                vec![p(r, 0.25, 0.1), p(r, 0.25, 0.8), p(r, 0.42, 0.64), p(r, 0.55, 0.92), p(r, 0.65, 0.88), p(r, 0.53, 0.6), p(r, 0.75, 0.6)],
                Color32::WHITE,
                s,
            ));
        }
        "LassoSelect" => {
            pa.add(Shape::dashed_line(
                &[p(r, 0.2, 0.3), p(r, 0.5, 0.12), p(r, 0.82, 0.3), p(r, 0.75, 0.62), p(r, 0.4, 0.68), p(r, 0.2, 0.3)],
                thin,
                3.0,
                2.0,
            ));
            line(pa, r, &[(0.4, 0.68), (0.35, 0.92)], thin);
        }
        "PanningHand" => {
            for x in [0.3, 0.42, 0.54, 0.66] {
                line(pa, r, &[(x, 0.2), (x, 0.55)], s);
            }
            line(pa, r, &[(0.22, 0.55), (0.3, 0.88), (0.66, 0.88), (0.78, 0.5)], s);
        }
        "EraserMenu" | "EraserLastUsed" | "SmallEraser" | "MediumEraser" | "LargeEraser" | "StrokeEraserOneNote" => {
            pa.add(Shape::convex_polygon(
                vec![p(r, 0.12, 0.62), p(r, 0.5, 0.22), p(r, 0.88, 0.5), p(r, 0.5, 0.9)],
                Color32::from_rgb(0xee, 0x95, 0x88),
                s,
            ));
            line(pa, r, &[(0.3, 0.42), (0.68, 0.72)], thin);
        }
        "InkStylesGallery" | "AddPen" | "FingerPaintingMode" | "PenMode" => {
            line(pa, r, &[(0.2, 0.85), (0.3, 0.6), (0.75, 0.15), (0.88, 0.28), (0.42, 0.72), (0.2, 0.85)], s);
            if k == "AddPen" {
                line(pa, r, &[(0.62, 0.72), (0.9, 0.72)], s);
                line(pa, r, &[(0.76, 0.58), (0.76, 0.86)], s);
            }
        }
        "MoreColorAndThicknessOptions" | "ShapeColorAndThickness" => {
            for (i, w) in [1.0, 2.0, 3.5].iter().enumerate() {
                let y = 0.25 + i as f32 * 0.25;
                line(pa, r, &[(0.15, y), (0.85, y)], Stroke::new(*w * s.width / 1.5, color));
            }
        }
        "Ruler" => {
            pa.add(Shape::convex_polygon(vec![p(r, 0.08, 0.7), p(r, 0.7, 0.08), p(r, 0.92, 0.3), p(r, 0.3, 0.92)], Color32::WHITE, s));
            for t in [0.25, 0.4, 0.55, 0.7] {
                line(pa, r, &[(t, 0.95 - t), (t + 0.06, 1.01 - t)], thin);
            }
        }
        "InkShapeGallery" | "ShapesDuplicate" | "GridSnapTo" | "InkToShape" => {
            pa.circle_stroke(p(r, 0.32, 0.36), r.width() * 0.2, s);
            rect(pa, r, 0.46, 0.5, 0.88, 0.88, Color32::TRANSPARENT, s);
        }
        "ObjectsArrangeMenu" | "ObjectBringForward" | "ObjectBringToFront" | "ObjectSendBackward" | "ObjectSendToBack" => {
            rect(pa, r, 0.1, 0.1, 0.6, 0.6, Color32::WHITE, s);
            rect(pa, r, 0.4, 0.4, 0.9, 0.9, accent, s);
        }
        "RotateMenuOneNote"
        | "RotateRight45OneNote"
        | "RotateLeft45OneNote"
        | "ObjectRotateRight90"
        | "ObjectRotateLeft90"
        | "ObjectFlipHorizontal"
        | "ObjectFlipVertical" => {
            pa.add(Shape::line(
                (0..=12)
                    .map(|i| {
                        let a = std::f32::consts::PI * (0.15 + 1.35 * i as f32 / 12.0);
                        r.center() + Vec2::angled(a) * r.width() * 0.35
                    })
                    .collect(),
                s,
            ));
            line(pa, r, &[(0.84, 0.36), (0.88, 0.58), (0.66, 0.56)], s);
        }
        "SelectionToTextConvert" => {
            line(pa, r, &[(0.1, 0.4), (0.2, 0.2), (0.3, 0.42), (0.4, 0.22)], s);
            line(pa, r, &[(0.5, 0.32), (0.62, 0.32)], thin);
            text(pa, Rect::from_min_max(p(r, 0.55, 0.45), r.max), "T", 0.5, color, false);
        }
        "SelectionToMathConvert" | "Math" | "InkEquation" => text(pa, r, "√x", 0.6, color, false),
        "InkReplay" => {
            pa.circle_stroke(r.center(), r.width() * 0.4, s);
            pa.add(Shape::convex_polygon(vec![p(r, 0.42, 0.32), p(r, 0.7, 0.5), p(r, 0.42, 0.68)], color, Stroke::NONE));
        }
        "GoToNextUnreadPage"
        | "UnreadMenu"
        | "MarkPageAsReadContextual"
        | "MarkPageAsUnreadContextual"
        | "MarkAllPagesAsRead"
        | "UnreadNotebookChangesShow" => {
            rect(pa, r, 0.18, 0.08, 0.82, 0.92, Color32::WHITE, s);
            for y in [0.3, 0.5] {
                line(pa, r, &[(0.3, y), (0.7, y)], Stroke::new(s.width * 1.5, color));
            }
            line(pa, r, &[(0.3, 0.7), (0.6, 0.7)], thin);
        }
        "FindByDateMenu"
        | "FindToday"
        | "FindSinceYesterday"
        | "FindSinceSevenDays"
        | "FindLastTwoWeeks"
        | "FindLastMonth"
        | "FindLastThreeMonths"
        | "FindLasteSixMonths"
        | "FindAllChangesByDate" => {
            rect(pa, r, 0.1, 0.1, 0.66, 0.82, Color32::WHITE, s);
            pa.circle_filled(p(r, 0.72, 0.7), r.width() * 0.22, Color32::WHITE);
            pa.circle_stroke(p(r, 0.72, 0.7), r.width() * 0.22, s);
            line(pa, r, &[(0.72, 0.56), (0.72, 0.7), (0.82, 0.76)], thin);
        }
        "FindUnread" | "AuthorHighlightingHide" | "Contact" => {
            pa.circle_stroke(p(r, 0.5, 0.32), r.width() * 0.17, s);
            line(pa, r, &[(0.18, 0.9), (0.24, 0.66), (0.5, 0.56), (0.76, 0.66), (0.82, 0.9)], s);
            if k == "AuthorHighlightingHide" {
                line(pa, r, &[(0.1, 0.1), (0.9, 0.9)], Stroke::new(s.width, Color32::from_rgb(0xd1, 0x34, 0x38)));
            }
        }
        "PageVersionSplitButton"
        | "PageVersions"
        | "DeleteAllPageVersionsInSection"
        | "DeleteAllPageVersionsInFolder"
        | "DeleteAllPageVersionsInNotebook"
        | "DisableVersionHistory"
        | "DisableVersionHistory2" => {
            rect(pa, r, 0.3, 0.06, 0.88, 0.72, Color32::WHITE, s);
            rect(pa, r, 0.12, 0.28, 0.7, 0.94, Color32::WHITE, s);
        }
        "RecycleBinMenu" | "RecycleBin" | "EmptyRecycleBin" => {
            line(pa, r, &[(0.12, 0.24), (0.88, 0.24)], s);
            line(pa, r, &[(0.38, 0.24), (0.42, 0.1), (0.58, 0.1), (0.62, 0.24)], s);
            pa.add(Shape::convex_polygon(vec![p(r, 0.2, 0.28), p(r, 0.8, 0.28), p(r, 0.72, 0.92), p(r, 0.28, 0.92)], Color32::WHITE, s));
            for x in [0.4, 0.6] {
                line(pa, r, &[(x, 0.4), (x, 0.8)], thin);
            }
        }
        "SpellCheck" => {
            text(pa, Rect::from_min_max(r.min, p(r, 1.0, 0.6)), "abc", 0.4, color, false);
            line(pa, r, &[(0.3, 0.72), (0.45, 0.88), (0.8, 0.58)], Stroke::new(s.width * 1.3, Color32::from_rgb(0x38, 0x76, 0x1d)));
        }
        "ResearchPane" | "Thesaurus" => {
            rect(pa, r, 0.1, 0.12, 0.62, 0.88, Color32::WHITE, s);
            pa.circle_stroke(p(r, 0.64, 0.56), r.width() * 0.2, s);
            line(pa, r, &[(0.78, 0.7), (0.92, 0.88)], s);
        }
        "AccessibilityChecker" => {
            pa.circle_filled(p(r, 0.5, 0.18), r.width() * 0.08, color);
            line(pa, r, &[(0.18, 0.34), (0.82, 0.34)], s);
            line(pa, r, &[(0.5, 0.34), (0.5, 0.6), (0.32, 0.9)], s);
            line(pa, r, &[(0.5, 0.6), (0.68, 0.9)], s);
        }
        "TranslateMenu" | "TranslateSelected" | "MiniTranslator" | "TranslationLanguageOptions" => {
            text(pa, Rect::from_min_max(r.min, p(r, 0.6, 0.6)), "A", 0.5, color, false);
            text(pa, Rect::from_min_max(p(r, 0.4, 0.4), r.max), "あ", 0.5, accent, false);
        }
        "LanguageCommands" | "SetLanguage" | "LanguagePreferences" => {
            pa.circle_stroke(r.center(), r.width() * 0.4, s);
            line(pa, r, &[(0.1, 0.5), (0.9, 0.5)], thin);
            pa.add(Shape::ellipse_stroke(r.center(), vec2(r.width() * 0.16, r.height() * 0.4), thin));
        }
        "PasswordProtectSection" | "Password" => {
            rect(pa, r, 0.2, 0.44, 0.8, 0.9, accent, s);
            line(pa, r, &[(0.32, 0.44), (0.32, 0.26), (0.5, 0.1), (0.68, 0.26), (0.68, 0.44)], s);
        }
        "LinkedNotes" => {
            rect(pa, r, 0.08, 0.14, 0.48, 0.86, Color32::WHITE, s);
            rect(pa, r, 0.52, 0.14, 0.92, 0.86, Color32::WHITE, s);
            line(pa, r, &[(0.38, 0.5), (0.62, 0.5)], Stroke::new(s.width, accent));
        }
        "NormalViewOneNote" | "NavigationPanes" => {
            rect(pa, r, 0.06, 0.14, 0.94, 0.86, Color32::WHITE, s);
            rect(pa, r, 0.06, 0.14, 0.3, 0.86, accent, Stroke::NONE);
            rect(pa, r, 0.74, 0.14, 0.94, 0.86, Color32::from_rgb(0xd2, 0xd0, 0xce), Stroke::NONE);
        }
        "FullPageView" => {
            rect(pa, r, 0.1, 0.1, 0.9, 0.9, Color32::WHITE, s);
            line(pa, r, &[(0.2, 0.36), (0.2, 0.2), (0.36, 0.2)], s);
            line(pa, r, &[(0.64, 0.8), (0.8, 0.8), (0.8, 0.64)], s);
        }
        "CurrentWindowDock" | "NewDockedWindow" => {
            rect(pa, r, 0.06, 0.14, 0.94, 0.86, Color32::WHITE, s);
            rect(pa, r, 0.62, 0.14, 0.94, 0.86, accent, Stroke::NONE);
        }
        "SwitchBackground" => {
            pa.circle_filled(r.center(), r.width() * 0.38, color);
            pa.circle_filled(p(r, 0.62, 0.42), r.width() * 0.3, Color32::WHITE);
        }
        "ImmersiveReader" => {
            rect(pa, r, 0.08, 0.2, 0.5, 0.84, Color32::WHITE, s);
            rect(pa, r, 0.5, 0.2, 0.92, 0.84, Color32::WHITE, s);
            text(pa, Rect::from_min_max(p(r, 0.08, 0.2), p(r, 0.5, 0.84)), "A", 0.5, accent, false);
        }
        "PageColorGallery" => {
            rect(pa, r, 0.16, 0.08, 0.84, 0.82, Color32::from_rgb(0xdc, 0xe9, 0xf7), s);
            rect(pa, r, 0.08, 0.86, 0.92, 0.98, Color32::from_rgb(0x8a, 0xa8, 0xe4), Stroke::NONE);
        }
        "RuleLinesMenu" | "RuleLineColorMenu" | "RuleLineColorGallery" | "NewPagesWithRuleLines" => {
            rect(pa, r, 0.14, 0.08, 0.86, 0.92, Color32::WHITE, s);
            for y in [0.28, 0.44, 0.6, 0.76] {
                line(pa, r, &[(0.14, y), (0.86, y)], Stroke::new(thin.width, Color32::from_rgb(0x6c, 0x9c, 0xd8)));
            }
            line(pa, r, &[(0.3, 0.08), (0.3, 0.92)], Stroke::new(thin.width, Color32::from_rgb(0xd1, 0x34, 0x38)));
        }
        "PageTitleHide" => {
            rect(pa, r, 0.14, 0.08, 0.86, 0.92, Color32::WHITE, s);
            line(pa, r, &[(0.26, 0.22), (0.74, 0.22)], Stroke::new(s.width * 1.5, color));
            line(pa, r, &[(0.1, 0.4), (0.9, 0.04)], Stroke::new(thin.width, Color32::from_rgb(0xd1, 0x34, 0x38)));
        }
        "PaperSize" => {
            rect(pa, r, 0.22, 0.08, 0.78, 0.92, Color32::WHITE, s);
            line(pa, r, &[(0.08, 0.2), (0.08, 0.8)], thin);
            line(pa, r, &[(0.3, 0.97), (0.7, 0.97)], thin);
        }
        "ZoomOut" | "ZoomIn" | "ZoomClassic" | "ZoomCurrent100" | "ZoomPageWidth" => {
            pa.circle_stroke(p(r, 0.42, 0.42), r.width() * 0.3, s);
            line(pa, r, &[(0.64, 0.64), (0.9, 0.9)], Stroke::new(s.width * 1.4, color));
            line(pa, r, &[(0.28, 0.42), (0.56, 0.42)], s);
            if k == "ZoomIn" {
                line(pa, r, &[(0.42, 0.28), (0.42, 0.56)], s);
            }
        }
        "WindowNew" | "NewSideNote" | "WindowKeepOnTop" => {
            rect(pa, r, 0.06, 0.18, 0.7, 0.8, Color32::WHITE, s);
            rect(pa, r, 0.36, 0.36, 0.94, 0.94, if k == "NewSideNote" { Color32::from_rgb(0xf5, 0xd9, 0x6b) } else { Color32::WHITE }, s);
        }
        "Help" | "ContactSupport" | "ShowTraining" | "WhatsNew" | "Feedback" => {
            pa.circle_stroke(r.center(), r.width() * 0.4, s);
            text(
                pa,
                r,
                if k == "Help" {
                    "?"
                } else if k == "Feedback" {
                    "!"
                } else {
                    "i"
                },
                0.55,
                color,
                true,
            );
        }
        "Dictate" => {
            rect(pa, r, 0.38, 0.1, 0.62, 0.58, Color32::WHITE, s);
            line(pa, r, &[(0.26, 0.42), (0.26, 0.6), (0.5, 0.74), (0.74, 0.6), (0.74, 0.42)], s);
            line(pa, r, &[(0.5, 0.74), (0.5, 0.9)], s);
        }
        _ => {
            // Generic glyph: a page with a corner fold.
            line(pa, r, &[(0.24, 0.1), (0.62, 0.1), (0.78, 0.26), (0.78, 0.9), (0.24, 0.9), (0.24, 0.1)], thin);
            line(pa, r, &[(0.62, 0.1), (0.62, 0.26), (0.78, 0.26)], thin);
        }
    }
}

/// Draw a tag symbol (catalog `R.Home.Tags.*`); `checked` ticks checkbox tags.
pub fn tag(pa: &Painter, r: Rect, symbol: notecraft_engine::doc::TagSymbol, checked: bool, color: Color32) {
    use notecraft_engine::doc::TagSymbol as S;
    let s = Stroke::new((r.width() / 10.0).max(1.0), color);
    let violet = Color32::from_rgb(0x6e, 0x3f, 0xb0);
    let checkbox = |extra: &str| {
        rect(pa, r, 0.12, 0.12, 0.88, 0.88, Color32::WHITE, Stroke::new(s.width, violet));
        if checked {
            line(pa, r, &[(0.26, 0.5), (0.44, 0.7), (0.78, 0.3)], Stroke::new(s.width * 1.4, Color32::from_rgb(0x38, 0x76, 0x1d)));
        } else if !extra.is_empty() {
            text(pa, r, extra, 0.55, violet, true);
        }
    };
    match symbol {
        S::None => {}
        S::Checkbox => checkbox(""),
        S::CheckboxOne => checkbox("1"),
        S::CheckboxTwo => checkbox("2"),
        S::CheckboxPerson => checkbox("•"),
        S::CheckboxCalendar => checkbox("▪"),
        S::CheckboxPhone => checkbox("☏"),
        S::CheckboxFlag => checkbox("⚑"),
        S::Star => {
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let a = std::f32::consts::PI * (i as f32 / 5.0) - std::f32::consts::FRAC_PI_2;
                    let rad = if i % 2 == 0 { 0.45 } else { 0.2 } * r.width();
                    r.center() + Vec2::angled(a) * rad
                })
                .collect();
            pa.add(Shape::convex_polygon(pts.clone(), Color32::from_rgb(0xf4, 0xb4, 0x00), Stroke::NONE));
            pa.add(Shape::closed_line(pts, Stroke::new(1.0, Color32::from_rgb(0xb0, 0x7a, 0x00))));
        }
        S::Question => {
            pa.circle_filled(r.center(), r.width() * 0.45, violet);
            text(pa, r, "?", 0.7, Color32::WHITE, true);
        }
        S::Exclamation => {
            pa.circle_filled(r.center(), r.width() * 0.45, Color32::from_rgb(0xd1, 0x34, 0x38));
            text(pa, r, "!", 0.7, Color32::WHITE, true);
        }
        other => {
            // The remaining tags reuse ribbon glyphs for their meaning.
            let key = match other {
                S::Person => "Contact",
                S::House => "Address",
                S::Phone => "Phone",
                S::Globe => "LanguageCommands",
                S::Bulb => "Sticker",
                S::Key => "Password",
                S::FlagA | S::FlagB => "Flag",
                S::Film => "VideoRecordingInsert",
                S::Book => "Thesaurus",
                S::Music => "AudioRecordingInsert",
                S::Document => "PrintoutFromFileInsert",
                S::Pen => "InkStylesGallery",
                S::Envelope => "SendNotesViaEmail",
                _ => "",
            };
            if key == "Phone" {
                // A handset.
                line(
                    pa,
                    r,
                    &[
                        (0.22, 0.2),
                        (0.38, 0.14),
                        (0.46, 0.34),
                        (0.36, 0.42),
                        (0.58, 0.64),
                        (0.66, 0.54),
                        (0.86, 0.62),
                        (0.8, 0.8),
                        (0.62, 0.86),
                        (0.2, 0.42),
                        (0.22, 0.2),
                    ],
                    s,
                );
            } else if key == "Address" {
                line(pa, r, &[(0.12, 0.5), (0.5, 0.14), (0.88, 0.5)], s);
                line(pa, r, &[(0.24, 0.42), (0.24, 0.88), (0.76, 0.88), (0.76, 0.42)], s);
            } else if key == "Flag" {
                let c = if other == S::FlagA { Color32::from_rgb(0xd1, 0x34, 0x38) } else { Color32::from_rgb(0xf4, 0xb4, 0x00) };
                line(pa, r, &[(0.24, 0.1), (0.24, 0.92)], s);
                pa.add(Shape::convex_polygon(vec![p(r, 0.26, 0.12), p(r, 0.84, 0.3), p(r, 0.26, 0.5)], c, Stroke::NONE));
            } else {
                draw(pa, r, key, color);
            }
        }
    }
}

/// Direction of a chevron.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    /// Pointing down (drop-downs, expanded).
    Down,
    /// Pointing right (collapsed).
    Right,
    /// Pointing up.
    Up,
    /// Pointing left.
    Left,
}

/// A small filled triangle (drop-down arrow, expand/collapse), drawn so it never depends on a
/// font having the glyph.
pub fn triangle(pa: &Painter, center: Pos2, size: f32, dir: Dir, color: Color32) {
    let h = size / 2.0;
    let pts = match dir {
        Dir::Down => vec![center + vec2(-h, -h * 0.5), center + vec2(h, -h * 0.5), center + vec2(0.0, h * 0.6)],
        Dir::Up => vec![center + vec2(-h, h * 0.5), center + vec2(h, h * 0.5), center + vec2(0.0, -h * 0.6)],
        Dir::Right => vec![center + vec2(-h * 0.5, -h), center + vec2(-h * 0.5, h), center + vec2(h * 0.6, 0.0)],
        Dir::Left => vec![center + vec2(h * 0.5, -h), center + vec2(h * 0.5, h), center + vec2(-h * 0.6, 0.0)],
    };
    pa.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// A plus sign in a circle (Add Page, collapsed paragraphs).
pub fn plus_circle(pa: &Painter, center: Pos2, radius: f32, color: Color32) {
    let s = Stroke::new((radius / 5.0).max(1.0), color);
    pa.circle_stroke(center, radius, s);
    pa.line_segment([center - vec2(radius * 0.55, 0.0), center + vec2(radius * 0.55, 0.0)], s);
    pa.line_segment([center - vec2(0.0, radius * 0.55), center + vec2(0.0, radius * 0.55)], s);
}

/// A four-way move handle (paragraph handle).
pub fn move_handle(pa: &Painter, center: Pos2, size: f32, color: Color32) {
    let s = Stroke::new(1.2, color);
    let h = size / 2.0;
    pa.line_segment([center - vec2(h, 0.0), center + vec2(h, 0.0)], s);
    pa.line_segment([center - vec2(0.0, h), center + vec2(0.0, h)], s);
    for d in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
        let off = match d {
            Dir::Up => vec2(0.0, -h),
            Dir::Down => vec2(0.0, h),
            Dir::Left => vec2(-h, 0.0),
            Dir::Right => vec2(h, 0.0),
        };
        triangle(pa, center + off, size * 0.38, d, color);
    }
}

/// A close cross.
pub fn cross(pa: &Painter, r: Rect, color: Color32) {
    let s = Stroke::new(1.4, color);
    pa.line_segment([p(r, 0.25, 0.25), p(r, 0.75, 0.75)], s);
    pa.line_segment([p(r, 0.75, 0.25), p(r, 0.25, 0.75)], s);
}

/// Draw a list bullet glyph as shapes (catalog `R.Home.BasicText.BulletsGallery`): disc, circle,
/// square, dash, arrow, diamond, check, star. `size` is the text height.
pub fn bullet(pa: &Painter, center: Pos2, size: f32, b: notecraft_engine::doc::Bullet, color: Color32) {
    use notecraft_engine::doc::Bullet as B;
    let r = size * 0.16;
    let s = Stroke::new((size / 12.0).max(1.0), color);
    match b {
        B::Disc => {
            pa.circle_filled(center, r, color);
        }
        B::Circle => {
            pa.circle_stroke(center, r, s);
        }
        B::Square => {
            pa.rect_filled(Rect::from_center_size(center, vec2(r * 1.8, r * 1.8)), 0.0, color);
        }
        B::Dash => {
            pa.line_segment([center - vec2(r * 1.4, 0.0), center + vec2(r * 1.4, 0.0)], s);
        }
        B::Arrow => triangle(pa, center, r * 2.6, Dir::Right, color),
        B::Diamond => {
            pa.add(Shape::convex_polygon(
                vec![center + vec2(0.0, -r * 1.4), center + vec2(r * 1.4, 0.0), center + vec2(0.0, r * 1.4), center + vec2(-r * 1.4, 0.0)],
                color,
                Stroke::NONE,
            ));
        }
        B::Check => {
            pa.add(Shape::line(vec![center + vec2(-r * 1.4, 0.0), center + vec2(-r * 0.3, r * 1.1), center + vec2(r * 1.5, -r * 1.3)], s));
        }
        B::Star => {
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let a = std::f32::consts::PI * (i as f32 / 5.0) - std::f32::consts::FRAC_PI_2;
                    let rad = if i % 2 == 0 { r * 1.6 } else { r * 0.7 };
                    center + Vec2::angled(a) * rad
                })
                .collect();
            pa.add(Shape::convex_polygon(pts.clone(), color, Stroke::NONE));
            pa.add(Shape::closed_line(pts, Stroke::new(1.0, color)));
        }
    }
}
