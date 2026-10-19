//! NoteCraft picture codecs (**standalone**: no workspace dependencies, so any layer may use it).
//!
//! Pictures on a page (decision D3, `docs/plan.md` §4.9) are stored as the bytes the user
//! inserted; this crate reads them: [`sniff`] names the format from its magic bytes, [`probe`]
//! reads only the header (format and size, cheap enough to run on insert), [`decode`] produces
//! RGBA8 pixels with the EXIF orientation applied, and [`encode_png`] turns clipboard pixels
//! into a file. PNG, JPEG, GIF (first frame), BMP and WebP are read by the pure-Rust decoders of
//! the `image` crate (no default features: no threads, builds for `wasm32-unknown-unknown`).
//!
//! **Every byte is hostile.** Sizes are checked against [`Limits`] from the header *before* any
//! pixel buffer is allocated, the decoders run under an allocation cap, and a decoder panic is
//! caught and reported as [`CodecError::Corrupt`] (last-resort guard; the decoders are not
//! expected to panic). Truncated, malformed or oversize input is an error, never a crash.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};

/// Errors.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// Not a picture format NoteCraft reads.
    #[error("not a supported picture (PNG, JPEG, GIF, BMP or WebP)")]
    Unsupported,
    /// Over a limit (bytes, pixels or side length).
    #[error("picture too large: {0}")]
    TooLarge(String),
    /// Truncated or malformed data.
    #[error("damaged picture: {0}")]
    Corrupt(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, CodecError>;

/// A picture format NoteCraft reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// Portable Network Graphics.
    Png,
    /// JPEG (JFIF/EXIF).
    Jpeg,
    /// GIF (the first frame of an animation).
    Gif,
    /// Windows bitmap.
    Bmp,
    /// WebP (lossy and lossless).
    WebP,
}

impl Format {
    /// Every format, in the order file dialogs list them.
    pub const ALL: [Format; 5] = [Format::Png, Format::Jpeg, Format::Gif, Format::Bmp, Format::WebP];

    /// The MIME type (for the clipboard and HTML export).
    pub fn mime(self) -> &'static str {
        match self {
            Format::Png => "image/png",
            Format::Jpeg => "image/jpeg",
            Format::Gif => "image/gif",
            Format::Bmp => "image/bmp",
            Format::WebP => "image/webp",
        }
    }

    /// The usual file extension (no dot).
    pub fn extension(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::Gif => "gif",
            Format::Bmp => "bmp",
            Format::WebP => "webp",
        }
    }

    /// Every extension the format is known by (lowercase, no dot), for file-dialog filters.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Format::Png => &["png"],
            Format::Jpeg => &["jpg", "jpeg", "jpe", "jfif"],
            Format::Gif => &["gif"],
            Format::Bmp => &["bmp", "dib"],
            Format::WebP => &["webp"],
        }
    }

    /// The format a MIME type names (`image/png`, …; parameters after `;` are ignored).
    pub fn from_mime(mime: &str) -> Option<Format> {
        let m = mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
        Format::ALL.into_iter().find(|f| f.mime() == m || (m == "image/jpg" && *f == Format::Jpeg))
    }

    fn image_format(self) -> ImageFormat {
        match self {
            Format::Png => ImageFormat::Png,
            Format::Jpeg => ImageFormat::Jpeg,
            Format::Gif => ImageFormat::Gif,
            Format::Bmp => ImageFormat::Bmp,
            Format::WebP => ImageFormat::WebP,
        }
    }
}

/// Name the format from the file's magic bytes (never from its name: a `.png` may be a JPEG).
pub fn sniff(bytes: &[u8]) -> Option<Format> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Format::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Format::Jpeg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(Format::Gif)
    } else if bytes.starts_with(b"BM") && bytes.len() >= 26 {
        Some(Format::Bmp)
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some(Format::WebP)
    } else {
        None
    }
}

/// Caps applied before and during decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Largest encoded file accepted.
    pub max_bytes: usize,
    /// Longest side in pixels.
    pub max_side: u32,
    /// Most pixels (width × height).
    pub max_pixels: u64,
}

impl Limits {
    /// The page-picture limits of decision D3: 64 MiB, 16,384 px per side, 64 Mpx (a 256 MiB
    /// RGBA buffer at most).
    pub const PICTURE: Limits = Limits { max_bytes: 64 << 20, max_side: 16_384, max_pixels: 64 << 20 };

    fn check_bytes(&self, n: usize) -> Result<()> {
        if n > self.max_bytes {
            return Err(CodecError::TooLarge(format!("{n} bytes (max {})", self.max_bytes)));
        }
        Ok(())
    }

    fn check_size(&self, w: u32, h: u32) -> Result<()> {
        if w == 0 || h == 0 {
            return Err(CodecError::Corrupt(format!("{w}×{h} pixels")));
        }
        if w > self.max_side || h > self.max_side {
            return Err(CodecError::TooLarge(format!("{w}×{h} pixels (max {} per side)", self.max_side)));
        }
        if u64::from(w) * u64::from(h) > self.max_pixels {
            return Err(CodecError::TooLarge(format!("{w}×{h} pixels (max {} pixels)", self.max_pixels)));
        }
        Ok(())
    }
}

impl Default for Limits {
    fn default() -> Limits {
        Limits::PICTURE
    }
}

/// What [`probe`] learns from a header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
    /// The format.
    pub format: Format,
    /// Width in pixels as displayed (after the EXIF orientation).
    pub width: u32,
    /// Height in pixels as displayed.
    pub height: u32,
}

/// Decoded pixels: RGBA, 8 bits per channel, not premultiplied, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba8 {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes.
    pub pixels: Vec<u8>,
}

impl Rgba8 {
    /// Wrap pixels, checking the buffer length (clipboard images arrive from other programs).
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Rgba8> {
        let want = u64::from(width) * u64::from(height) * 4;
        if width == 0 || height == 0 || pixels.len() as u64 != want {
            return Err(CodecError::Corrupt(format!("{width}×{height} pixels in {} bytes", pixels.len())));
        }
        Ok(Rgba8 { width, height, pixels })
    }
}

/// Run a decoder step, turning an escaped panic into an error.
fn guarded<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| Err(CodecError::Corrupt("the decoder failed".into())))
}

fn corrupt(e: impl std::fmt::Display) -> CodecError {
    match e.to_string() {
        s if s.contains("limit") => CodecError::TooLarge(s),
        s => CodecError::Corrupt(s),
    }
}

/// Open a decoder for `bytes` with the allocation cap derived from `limits`.
fn decoder<'a>(bytes: &'a [u8], limits: &Limits) -> Result<(Format, impl ImageDecoder + 'a)> {
    limits.check_bytes(bytes.len())?;
    let format = sniff(bytes).ok_or(CodecError::Unsupported)?;
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format.image_format());
    let mut l = image::Limits::default();
    l.max_image_width = Some(limits.max_side);
    l.max_image_height = Some(limits.max_side);
    // The pixel buffer plus slack for the decoders' own working memory.
    l.max_alloc = Some(limits.max_pixels.saturating_mul(4).saturating_add(64 << 20));
    reader.limits(l);
    let dec = reader.into_decoder().map_err(corrupt)?;
    Ok((format, dec))
}

/// Read the format and displayed size from the header, validating them against `limits`,
/// without decoding pixels.
pub fn probe(bytes: &[u8], limits: &Limits) -> Result<Info> {
    guarded(|| {
        let (format, mut dec) = decoder(bytes, limits)?;
        let (w, h) = dec.dimensions();
        limits.check_size(w, h)?;
        let (w, h) = if swaps_sides(dec.orientation().ok()) { (h, w) } else { (w, h) };
        Ok(Info { format, width: w, height: h })
    })
}

fn swaps_sides(o: Option<image::metadata::Orientation>) -> bool {
    use image::metadata::Orientation::*;
    matches!(o, Some(Rotate90 | Rotate270 | Rotate90FlipH | Rotate270FlipH))
}

/// Decode to RGBA8 with the EXIF orientation applied.
pub fn decode(bytes: &[u8], limits: &Limits) -> Result<Rgba8> {
    guarded(|| {
        let (_, mut dec) = decoder(bytes, limits)?;
        let (w, h) = dec.dimensions();
        limits.check_size(w, h)?;
        let orientation = dec.orientation().ok();
        let mut img = DynamicImage::from_decoder(dec).map_err(corrupt)?;
        if let Some(o) = orientation {
            img.apply_orientation(o);
        }
        let rgba = img.into_rgba8();
        let (width, height) = rgba.dimensions();
        Ok(Rgba8 { width, height, pixels: rgba.into_raw() })
    })
}

/// Decode, then shrink (keeping the aspect ratio) so neither side exceeds `max_side` pixels:
/// what the canvas uploads as a texture.
pub fn decode_fit(bytes: &[u8], limits: &Limits, max_side: u32) -> Result<Rgba8> {
    let img = decode(bytes, limits)?;
    let max_side = max_side.max(1);
    if img.width <= max_side && img.height <= max_side {
        return Ok(img);
    }
    guarded(|| {
        let buf = image::RgbaImage::from_raw(img.width, img.height, img.pixels).ok_or_else(|| CodecError::Corrupt("pixel buffer".into()))?;
        let small = DynamicImage::ImageRgba8(buf).thumbnail(max_side, max_side).into_rgba8();
        let (width, height) = small.dimensions();
        Ok(Rgba8 { width, height, pixels: small.into_raw() })
    })
}

/// Encode RGBA8 pixels as a PNG file (pasted screenshots become pictures this way).
pub fn encode_png(img: &Rgba8, limits: &Limits) -> Result<Vec<u8>> {
    limits.check_size(img.width, img.height)?;
    if img.pixels.len() as u64 != u64::from(img.width) * u64::from(img.height) * 4 {
        return Err(CodecError::Corrupt("pixel buffer length".into()));
    }
    guarded(|| {
        let mut out = Cursor::new(Vec::new());
        image::write_buffer_with_format(&mut out, &img.pixels, img.width, img.height, image::ExtendedColorType::Rgba8, ImageFormat::Png)
            .map_err(corrupt)?;
        let bytes = out.into_inner();
        limits.check_bytes(bytes.len())?;
        Ok(bytes)
    })
}

#[cfg(test)]
mod tests;
