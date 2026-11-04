//! Stickers (Insert › Stickers): small pictures drawn here, in code, as PNG — original artwork,
//! nothing copied from any other product. A sticker is an ordinary picture once inserted.
//! Without a `sticker` param the next one in the list is chosen, so repeated clicks vary.

use notecraft_codecs::{Limits, Rgba8, encode_png};
use serde_json::{Value, json};

use crate::ops::{need_page, no_page};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

const SIZE: usize = 128;
/// The stickers, in the order the button cycles through them.
const NAMES: [&str; 6] = ["smile", "heart", "star", "check", "cross", "important"];

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // R.Insert.Stickers.Sticker — unverified: the reference opens a gallery of stickers.
        CommandSpec::new("insert.sticker", "R.Insert.Stickers.Sticker", "Sticker", "Insert/Stickers/Sticker", insert_sticker)
            .params(r#"{"sticker"?: "smile" | "heart" | "star" | "check" | "cross" | "important", "x"?: pt, "y"?: pt}"#)
            .when(need_page),
    ]
}

fn insert_sticker(s: &mut Session, v: &Value) -> CmdResult {
    let n = s.page().ok_or_else(no_page)?.objects.len();
    let name = match p::str(v, "sticker") {
        Some(name) => NAMES.iter().copied().find(|k| *k == name).ok_or_else(|| CmdError::Params(format!("no sticker named {name}")))?,
        None => NAMES[n % NAMES.len()],
    };
    let bytes = png(render(name))?;
    let id = crate::picture_cmds::add_picture(s, v, bytes, (SIZE as u32, SIZE as u32))?;
    Ok(json!({"picture": id.to_string(), "sticker": name}))
}

type Rgba = [f32; 4];

fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy).max(1e-9)).clamp(0.0, 1.0);
    ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
}

/// Colour of sticker `name` at (x, y) in -1..1 (y down); alpha 0 outside the shape.
fn shade(name: &str, x: f32, y: f32) -> Rgba {
    const CLEAR: Rgba = [0.0; 4];
    let white: Rgba = [1.0, 1.0, 1.0, 1.0];
    let r = (x * x + y * y).sqrt();
    let disc = |fill: Rgba, edge: Rgba| -> Rgba {
        if r > 0.92 {
            CLEAR
        } else if r > 0.82 {
            edge
        } else {
            fill
        }
    };
    match name {
        "smile" => {
            let c = disc([0.98, 0.80, 0.10, 1.0], [0.78, 0.55, 0.05, 1.0]);
            if c[3] == 0.0 {
                return c;
            }
            let dark: Rgba = [0.25, 0.17, 0.05, 1.0];
            if ((x + 0.32).powi(2) / 0.012 + (y + 0.2).powi(2) / 0.05) < 1.0 || ((x - 0.32).powi(2) / 0.012 + (y + 0.2).powi(2) / 0.05) < 1.0 {
                return dark;
            }
            let mr = ((x * x) + (y + 0.05).powi(2)).sqrt();
            if (mr - 0.5).abs() < 0.06 && y > 0.15 {
                return dark;
            }
            c
        }
        "heart" => {
            let (hx, hy) = (x * 1.25, -(y - 0.05) * 1.3);
            let f = (hx * hx + hy * hy - 1.0).powi(3) - hx * hx * hy.powi(3);
            if f <= 0.0 {
                if hx * hx + hy * hy > 0.0 && (x + 0.38).powi(2) + (y + 0.3).powi(2) < 0.02 { [1.0, 0.65, 0.7, 1.0] } else { [0.88, 0.12, 0.22, 1.0] }
            } else {
                CLEAR
            }
        }
        "star" => {
            let ang = y.atan2(x) + std::f32::consts::FRAC_PI_2;
            let k = std::f32::consts::PI / 5.0;
            let a = ang.rem_euclid(2.0 * k) - k; // -k..k within one point's wedge
            let (outer, inner) = (0.95_f32, 0.4_f32);
            // Boundary between the outer tip (at a = 0) and the inner notch (at |a| = k).
            let t = a.abs() / k;
            let edge = outer + (inner - outer) * t;
            if r <= edge { if r > edge - 0.1 { [0.85, 0.6, 0.0, 1.0] } else { [1.0, 0.8, 0.1, 1.0] } } else { CLEAR }
        }
        "check" | "cross" | "important" => {
            let fill = match name {
                "check" => [0.18, 0.62, 0.34, 1.0],
                "cross" => [0.82, 0.2, 0.2, 1.0],
                _ => [0.43, 0.25, 0.69, 1.0],
            };
            let c = disc(fill, [fill[0] * 0.7, fill[1] * 0.7, fill[2] * 0.7, 1.0]);
            if c[3] == 0.0 {
                return c;
            }
            let pt = (x, y);
            let w = 0.1;
            let on = match name {
                "check" => seg_dist(pt, (-0.42, 0.02), (-0.12, 0.34)) < w || seg_dist(pt, (-0.12, 0.34), (0.45, -0.32)) < w,
                "cross" => seg_dist(pt, (-0.34, -0.34), (0.34, 0.34)) < w || seg_dist(pt, (0.34, -0.34), (-0.34, 0.34)) < w,
                _ => seg_dist(pt, (0.0, -0.5), (0.0, 0.12)) < w * 1.3 || (x * x + (y - 0.45).powi(2)).sqrt() < 0.12,
            };
            if on { white } else { c }
        }
        _ => CLEAR,
    }
}

/// `SIZE`×`SIZE` RGBA pixels, 3×3 supersampled.
fn render(name: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(SIZE * SIZE * 4);
    for py in 0..SIZE {
        for px in 0..SIZE {
            let mut acc = [0.0f32; 4];
            for sy in 0..3 {
                for sx in 0..3 {
                    let x = ((px as f32 + (sx as f32 + 0.5) / 3.0) / SIZE as f32) * 2.0 - 1.0;
                    let y = ((py as f32 + (sy as f32 + 0.5) / 3.0) / SIZE as f32) * 2.0 - 1.0;
                    let c = shade(name, x, y);
                    for i in 0..3 {
                        acc[i] += c[i] * c[3];
                    }
                    acc[3] += c[3];
                }
            }
            let a = acc[3];
            for ch in acc.iter().take(3) {
                out.push(if a > 0.0 { (ch / a * 255.0).round() as u8 } else { 0 });
            }
            out.push((a / 9.0 * 255.0).round() as u8);
        }
    }
    out
}

fn png(rgba: Vec<u8>) -> Result<Vec<u8>, CmdError> {
    let failed = |e: String| CmdError::Failed(e);
    let image = Rgba8::new(SIZE as u32, SIZE as u32, rgba).map_err(|e| failed(e.to_string()))?;
    encode_png(&image, &Limits::PICTURE).map_err(|e| failed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::tests::{blank_page, run, sample};

    #[test]
    fn every_sticker_is_a_decodable_picture() {
        // parity: R.Insert.Stickers.Sticker
        let mut s = sample();
        blank_page(&mut s);
        for name in super::NAMES {
            let r = run(&mut s, "insert.sticker", json!({"sticker": name}));
            assert!(r["picture"].is_string(), "{name}");
        }
        assert!(s.run("insert.sticker", json!({"sticker": "nope"})).is_err());
        let bytes = super::png(super::render("star")).unwrap();
        assert_eq!(crate::picture_cmds::probe(&bytes).unwrap(), (128, 128));
    }
}
