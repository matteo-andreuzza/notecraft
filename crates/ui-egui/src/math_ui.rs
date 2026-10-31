//! Equations on screen (decision D11): the built-up layout of crate `math` measured and drawn
//! with egui's fonts, for the canvas and the Equation galleries' previews.

use std::sync::Arc;

use egui::text::LayoutJob;
use egui::{Color32, FontId, Painter, Pos2, Shape, Stroke, TextFormat, pos2, vec2};
use notecraft_math::{Item, MathBox, Measure};

/// Text measured and laid out by egui.
struct EguiMeasure<'a> {
    ctx: &'a egui::Context,
}

fn galley(ctx: &egui::Context, text: &str, size: f32, italic: bool, color: Color32) -> Arc<egui::Galley> {
    let mut job = LayoutJob::default();
    job.append(text, 0.0, TextFormat { font_id: FontId::proportional(size.max(1.0)), color, italics: italic, ..Default::default() });
    ctx.fonts_mut(|f| f.layout_job(job))
}

/// The distance from a one-line galley's top to its baseline.
fn baseline(g: &egui::Galley, size: f32) -> f32 {
    g.rows.first().and_then(|r| r.row.glyphs.first().map(|gl| r.pos.y + gl.pos.y)).unwrap_or(size * 0.8)
}

impl Measure for EguiMeasure<'_> {
    fn measure(&self, text: &str, size: f32, italic: bool) -> (f32, f32, f32) {
        let g = galley(self.ctx, text, size, italic, Color32::WHITE);
        let b = baseline(&g, size);
        // Italic letters lean right: a little room so the next one doesn't touch.
        let lean = if italic { size * 0.06 } else { 0.0 };
        (g.size().x + lean, b, (g.size().y - b).max(0.0))
    }
}

/// Lay out the linear format `text` at font `size` (screen points).
pub(crate) fn layout(ctx: &egui::Context, text: &str, size: f32) -> MathBox {
    notecraft_math::layout(&notecraft_math::parse(text), size, &EguiMeasure { ctx })
}

/// Draw `b` with its top-left corner at `top_left`.
pub(crate) fn draw(ctx: &egui::Context, painter: &Painter, top_left: Pos2, b: &MathBox, color: Color32) {
    let origin = top_left + vec2(0.0, b.ascent);
    for it in &b.items {
        match it {
            Item::Text { text, x, y, size, italic } => {
                let g = galley(ctx, text, *size, *italic, color);
                let top = origin.y + y - baseline(&g, *size);
                painter.galley(pos2(origin.x + x, top), g, color);
            }
            Item::Path { points, width } => {
                let pts: Vec<Pos2> = points.iter().map(|(x, y)| pos2(origin.x + x, origin.y + y)).collect();
                painter.add(Shape::line(pts, Stroke::new(width.max(0.8), color)));
            }
            Item::Slot { x, y, w, h } => {
                let r = egui::Rect::from_min_size(pos2(origin.x + x, origin.y + y), vec2(*w, *h));
                let stroke = Stroke::new(1.0, color.gamma_multiply(0.6));
                let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                for pair in corners.windows(2) {
                    if let [a, c] = pair {
                        painter.extend(Shape::dashed_line(&[*a, *c], stroke, 2.0, 2.0));
                    }
                }
            }
        }
    }
}

/// A built-up preview of `text` in a menu, named `label` (shown above it when `caption`): draws
/// it and returns the clickable response.
pub(crate) fn preview(ui: &mut egui::Ui, label: &str, text: &str, size: f32, caption: bool, t: &crate::Tokens) -> egui::Response {
    let b = layout(ui.ctx(), text, size);
    let pad = 6.0;
    let cap = if caption { 16.0 } else { 0.0 };
    let (rect, resp) = ui
        .allocate_exact_size(vec2((b.width + 2.0 * pad).max(if caption { 180.0 } else { 40.0 }), b.height() + 2.0 * pad + cap), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    if resp.hovered() {
        ui.painter().rect_filled(rect, crate::theme::radius::CONTROL, t.hover);
    }
    if caption {
        ui.painter().text(rect.min + vec2(pad, 2.0), egui::Align2::LEFT_TOP, label, FontId::proportional(11.0), t.text_dim);
    }
    draw(ui.ctx(), ui.painter(), rect.min + vec2(pad, pad + cap), &b, t.text);
    resp
}
