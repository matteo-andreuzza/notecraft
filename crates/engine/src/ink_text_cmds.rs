//! Ink to Text: the selected handwriting is drawn black on white (`draw.inkImage`), the host
//! reads it with its text recogniser (Tesseract, as for pictures) and `draw.inkToText` swaps
//! the strokes for a note container holding the text. Recognition quality is the recogniser's:
//! neat, separate lines of writing work best (unverified against the reference's recogniser).

use notecraft_codecs::{Limits, Rgba8, encode_png};
use notecraft_doc::PageObject;
use notecraft_doc::geom::Point;
use serde_json::{Value, json};

use crate::draw_cmds::selected_ink;
use crate::{CmdError, CmdResult, CommandSpec, Session, ops, p};

/// Pixels per point in the image handed to the recogniser, and the cap on its side.
const SCALE: f32 = 3.0;
const MAX_SIDE: f32 = 3000.0;
/// White margin around the writing, points.
const MARGIN: f32 = 10.0;

fn need_ink(s: &Session) -> Option<&'static str> {
    if s.sel.page.is_none() {
        return Some("No page is open");
    }
    if selected_ink(s).is_empty() { Some("Select some ink first") } else { None }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // R.Draw.Convert.SelectionToTextConvert — the UI asks the host to recognise the image,
        // then calls this with the text.
        CommandSpec::new("draw.inkToText", "R.Draw.Convert.SelectionToTextConvert", "Ink to Text", "Draw/Convert/Ink to Text", ink_to_text)
            .params(r#"{"text": recognised text}"#)
            .when(need_ink),
        CommandSpec::new("draw.inkImage", "NC.Draw.InkImage", "Selected Ink as Picture", "Draw/Convert/Ink Image", ink_image)
            .params(r#"{} → {"png": base64 picture of the selected ink, black on white}"#)
            .pure()
            .when(need_ink),
    ]
}

fn ink_image(s: &mut Session, _: &Value) -> CmdResult {
    let page = s.page().ok_or_else(ops::no_page)?;
    let strokes: Vec<_> = selected_ink(s).into_iter().filter_map(|id| page.ink(id)).collect();
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for ink in &strokes {
        let b = ink.bounds();
        (x0, y0, x1, y1) = (x0.min(b.min.x), y0.min(b.min.y), x1.max(b.max.x), y1.max(b.max.y));
    }
    if strokes.is_empty() || !(x1 > x0 || y1 > y0) {
        return Err(CmdError::Failed("There is no ink to read.".into()));
    }
    let (x0, y0) = (x0 - MARGIN, y0 - MARGIN);
    let scale = SCALE.min(MAX_SIDE / (x1 - x0 + 2.0 * MARGIN).max(1.0)).min(MAX_SIDE / (y1 - y0 + 2.0 * MARGIN).max(1.0));
    let (w, h) = ((((x1 - x0) + MARGIN) * scale).ceil().max(8.0) as usize, (((y1 - y0) + MARGIN) * scale).ceil().max(8.0) as usize);
    let mut px = vec![255u8; w * h * 4];
    let mut dot = |cx: f32, cy: f32, r: f32| {
        let (ix0, ix1) = (((cx - r).floor().max(0.0)) as usize, ((cx + r).ceil().max(0.0) as usize).min(w.saturating_sub(1)));
        let (iy0, iy1) = (((cy - r).floor().max(0.0)) as usize, ((cy + r).ceil().max(0.0) as usize).min(h.saturating_sub(1)));
        for y in iy0..=iy1 {
            for x in ix0..=ix1 {
                if (x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2) <= r * r {
                    let i = (y * w + x) * 4;
                    px[i..i + 3].fill(0);
                }
            }
        }
    };
    for ink in &strokes {
        let pts: Vec<Point> = ink.page_points().collect();
        let r = (ink.width * scale / 2.0).max(1.6);
        for pair in pts.windows(2).chain(std::iter::once(&pts[..1]).filter(|_| pts.len() == 1)) {
            let (a, b) = (pair[0], *pair.last().unwrap_or(&pair[0]));
            let n = (((b.x - a.x).hypot(b.y - a.y)) * scale / (r * 0.6).max(1.0)).ceil().max(1.0) as usize;
            for i in 0..=n {
                let t = i as f32 / n as f32;
                dot((a.x + (b.x - a.x) * t - x0) * scale, (a.y + (b.y - a.y) * t - y0) * scale, r);
            }
        }
    }
    let image = Rgba8::new(w as u32, h as u32, px).map_err(|e| CmdError::Failed(e.to_string()))?;
    let png = encode_png(&image, &Limits::PICTURE).map_err(|e| CmdError::Failed(e.to_string()))?;
    Ok(json!({"png": crate::b64::encode(&png)}))
}

fn ink_to_text(s: &mut Session, v: &Value) -> CmdResult {
    let text = p::str(v, "text").unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Err(CmdError::Failed("No text could be recognised in the selected ink.".into()));
    }
    let ids = selected_ink(s);
    let page = s.page().ok_or_else(ops::no_page)?;
    let origin = ids
        .iter()
        .filter_map(|id| page.ink(*id))
        .map(|i| i.bounds().min)
        .fold(Point::new(f32::MAX, f32::MAX), |a, b| Point::new(a.x.min(b.x), a.y.min(b.y)));
    let mut paras = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let mut para = ops::new_para(s);
        notecraft_text::insert(&mut para, 0, line.trim(), &notecraft_doc::CharFormat::default());
        paras.push(para);
    }
    let first = paras.first().map(|q| q.id).ok_or_else(|| CmdError::Failed("No text could be recognised.".into()))?;
    let oid = s.new_id();
    let mut outline = notecraft_doc::Outline::new(oid, origin, first);
    outline.paragraphs = paras;
    let page = s.page_mut().ok_or_else(ops::no_page)?;
    page.objects.retain(|o| !ids.contains(&o.id()));
    page.objects.push(PageObject::Outline(outline));
    s.pending_outline = None;
    s.sel.on_page = crate::selection::PageSel::None;
    Ok(json!({"text": text}))
}

#[cfg(test)]
mod tests {
    use notecraft_doc::PageObject;
    use serde_json::json;

    use crate::tests::{blank_page, run, sample};

    #[test]
    fn handwriting_is_drawn_for_the_recogniser_and_replaced_by_its_text() {
        // parity: R.Draw.Convert.SelectionToTextConvert
        let mut s = sample();
        blank_page(&mut s);
        assert!(s.run("draw.inkToText", json!({"text": "hi"})).is_err(), "needs ink");
        run(&mut s, "draw.stroke", json!({"points": [[20, 20], [60, 40], [100, 20]]}));
        run(&mut s, "edit.selectAll", json!({}));
        let r = run(&mut s, "draw.inkImage", json!({}));
        assert!(r["png"].as_str().is_some_and(|p| p.len() > 100));
        assert!(s.run("draw.inkToText", json!({"text": "  "})).is_err());
        run(&mut s, "draw.inkToText", json!({"text": "hello\nworld"}));
        let page = s.page().unwrap();
        assert!(page.objects.iter().all(|o| !matches!(o, PageObject::Ink(_))));
        assert!(page.objects.iter().any(|o| matches!(o, PageObject::Outline(_))));
    }
}
