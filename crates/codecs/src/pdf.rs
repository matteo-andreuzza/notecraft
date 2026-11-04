//! PDF pages as pictures, for file printouts (decision D3's "later": a printout is the attached
//! file plus one picture per page). Rendering is done by `hayro`, a pure-Rust PDF rasterizer
//! (MIT OR Apache-2.0; it compiles in PDFium's BSD-3-Clause standard-14 substitute fonts, see
//! `ATTRIBUTION.md`), on the CPU, with no system libraries, so it also builds for wasm.
//!
//! **Every byte is hostile**, as for pictures: the file size, the number of pages and each
//! page's pixel size are capped by [`PdfLimits`] before anything is rendered (a page's scale is
//! lowered until it fits, never a huge allocation), and a renderer panic is caught and reported
//! as [`CodecError::Corrupt`]. Encrypted files are refused with a message.

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::{LoadPdfError, Pdf};
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{RenderCache, RenderSettings};

use crate::{CodecError, Result, Rgba8, guarded};

/// Caps on a PDF rendered as a printout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfLimits {
    /// Largest file accepted, bytes.
    pub max_bytes: usize,
    /// Pages rendered at most (the rest are left out, and the caller says so).
    pub max_pages: usize,
    /// Longest side of a rendered page, pixels.
    pub max_side: u32,
    /// Most pixels per rendered page.
    pub max_pixels: u64,
    /// Pixels per PDF point before the caps (1.5 = 108 dpi).
    pub scale: f32,
}

impl PdfLimits {
    /// File printouts: 256 MiB, 200 pages, 3,000 px per side, 8 Mpx per page, 108 dpi.
    pub const PRINTOUT: PdfLimits = PdfLimits { max_bytes: 256 << 20, max_pages: 200, max_side: 3_000, max_pixels: 8 << 20, scale: 1.5 };
}

/// Whether `bytes` start like a PDF file (`%PDF-`, allowing up to 1 KiB of junk before it, as
/// readers do).
pub fn is_pdf(bytes: &[u8]) -> bool {
    bytes.windows(5).take(1024).any(|w| w == b"%PDF-")
}

fn load(bytes: &[u8], limits: &PdfLimits) -> Result<Pdf> {
    if bytes.len() > limits.max_bytes {
        return Err(CodecError::TooLarge(format!("the PDF is larger than {} MiB", limits.max_bytes >> 20)));
    }
    if !is_pdf(bytes) {
        return Err(CodecError::Unsupported);
    }
    match Pdf::new(bytes.to_vec()) {
        Ok(pdf) => Ok(pdf),
        Err(LoadPdfError::Decryption(_)) => Err(CodecError::Corrupt("the PDF is password-protected".into())),
        Err(LoadPdfError::Invalid) => Err(CodecError::Corrupt("the PDF can't be read".into())),
    }
}

/// The number of pages, and each page's size in PDF points (1/72 inch).
pub fn page_sizes(bytes: &[u8], limits: &PdfLimits) -> Result<Vec<(f32, f32)>> {
    guarded(|| {
        let pdf = load(bytes, limits)?;
        Ok(pdf.pages().iter().map(|p| p.render_dimensions()).collect())
    })
}

/// The pixel scale for a page of `w × h` points under `limits` (lowered until the page fits
/// the side and pixel caps).
fn fit_scale(w: f32, h: f32, limits: &PdfLimits) -> f32 {
    let (w, h) = (w.max(1.0), h.max(1.0));
    let mut s = limits.scale.clamp(0.01, 8.0);
    s = s.min(limits.max_side as f32 / w).min(limits.max_side as f32 / h);
    let px = f64::from(w * s) * f64::from(h * s);
    if px > limits.max_pixels as f64 {
        s *= (limits.max_pixels as f64 / px).sqrt() as f32;
    }
    s.max(0.01)
}

/// Render the first [`PdfLimits::max_pages`] pages on white, one [`Rgba8`] per page, with each
/// page's size in points. Pages with no area are skipped.
pub fn render(bytes: &[u8], limits: &PdfLimits) -> Result<Vec<(Rgba8, (f32, f32))>> {
    guarded(|| {
        let pdf = load(bytes, limits)?;
        let cache = RenderCache::new();
        let settings = InterpreterSettings::default();
        let mut out = Vec::new();
        for page in pdf.pages().iter().take(limits.max_pages) {
            let (w, h) = page.render_dimensions();
            if !(w.is_finite() && h.is_finite()) || w < 1.0 || h < 1.0 {
                continue;
            }
            let s = fit_scale(w, h, limits);
            let px = hayro::render(page, &cache, &settings, &RenderSettings { x_scale: s, y_scale: s, bg_color: WHITE, ..Default::default() });
            let (pw, ph) = (u32::from(px.width()), u32::from(px.height()));
            if pw == 0 || ph == 0 {
                continue;
            }
            // Opaque white underneath: premultiplied and straight alpha are the same bytes.
            out.push((Rgba8::new(pw, ph, px.data_as_u8_slice().to_vec())?, (w, h)));
        }
        if out.is_empty() {
            return Err(CodecError::Corrupt("the PDF has no pages".into()));
        }
        Ok(out)
    })
}

/// A tiny valid PDF written here (no fixture files): one page per entry, `w × h` points, each
/// filled with a rectangle of the given colour inset by 10 pt. For tests and examples.
pub fn sample_pdf(pages: &[(f32, f32, [u8; 3])]) -> Vec<u8> {
    let mut objs: Vec<String> = Vec::new();
    let n = pages.len();
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 3 + i * 2)).collect();
    objs.push("<< /Type /Catalog /Pages 2 0 R >>".into());
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")));
    for (i, (w, h, c)) in pages.iter().enumerate() {
        let content = format!(
            "{:.3} {:.3} {:.3} rg 10 10 {} {} re f",
            f32::from(c[0]) / 255.0,
            f32::from(c[1]) / 255.0,
            f32::from(c[2]) / 255.0,
            (w - 20.0).max(1.0),
            (h - 20.0).max(1.0)
        );
        objs.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] /Contents {} 0 R >>", 4 + i * 2));
        objs.push(format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()));
    }
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{o}\nendobj\n", i + 1));
    }
    let xref = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1));
    for off in offsets {
        pdf.push_str(&format!("{off:010} 00000 n \n"));
    }
    pdf.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1));
    pdf.into_bytes()
}
