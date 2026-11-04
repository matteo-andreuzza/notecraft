//! The page's font files (layer **L0**: no workspace dependencies; the interface (L6) draws
//! with them and the PDF export (L4) embeds them, so the discovery lives below both).
//!
//! NoteCraft bundles no font files (AGENTS.md › Clean room). For the reference's default
//! "Calibri" the metric-compatible Carlito is preferred, then common open sans-serif
//! families; "Consolas" (the Code style) maps to an open monospace family; symbol and math
//! faces cover what the main face lacks (bullets, arrows, operators). Every file is checked
//! with `skrifa` before use and bounded in size: a broken or hostile file is skipped, never
//! a crash. The web build has no file system: every face is `None`.

use std::sync::{Arc, OnceLock};

/// Page candidate files per role, most preferred first (Linux and BSD paths; macOS and Windows
/// paths follow).
pub const REGULAR: &[&str] = &[
    "/usr/share/fonts/crosextra/Carlito-Regular.ttf",
    "/usr/share/fonts/truetype/crosextra/Carlito-Regular.ttf",
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "C:\\Windows\\Fonts\\segoeui.ttf",
];
/// Bold page faces, in the order of [`REGULAR`].
pub const BOLD: &[&str] = &[
    "/usr/share/fonts/crosextra/Carlito-Bold.ttf",
    "/usr/share/fonts/truetype/crosextra/Carlito-Bold.ttf",
    "/usr/share/fonts/noto/NotoSans-Bold.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
    "/usr/share/fonts/google-noto/NotoSans-Bold.ttf",
    "/usr/share/fonts/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    "C:\\Windows\\Fonts\\segoeuib.ttf",
];
/// Monospace faces (the Code style, "Consolas").
pub const MONO: &[&str] = &[
    "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
    "/usr/share/fonts/liberation/LiberationMono-Regular.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/System/Library/Fonts/Supplemental/Courier New.ttf",
    "C:\\Windows\\Fonts\\consola.ttf",
];
/// Bold monospace faces.
pub const MONO_BOLD: &[&str] = &[
    "/usr/share/fonts/noto/NotoSansMono-Bold.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansMono-Bold.ttf",
    "/usr/share/fonts/liberation/LiberationMono-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono-Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf",
    "C:\\Windows\\Fonts\\consolab.ttf",
];
/// Symbol and math fallbacks (arrows, geometric shapes, math operators), each a list of
/// candidate paths for one face.
pub const SYMBOLS: &[&[&str]] = &[
    &["/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf", "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf"],
    &["/usr/share/fonts/noto/NotoSansSymbols-Regular.ttf", "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf"],
    &["/usr/share/fonts/noto/NotoSansMath-Regular.ttf", "/usr/share/fonts/truetype/noto/NotoSansMath-Regular.ttf"],
    &["/usr/share/fonts/TTF/DejaVuSans.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"],
];

/// Largest font file accepted (hostile or broken files are skipped).
pub const MAX_FONT_BYTES: u64 = 64 << 20;

/// Whether `bytes` parse as a font.
pub fn is_font(bytes: &[u8]) -> bool {
    skrifa::FontRef::new(bytes).is_ok()
}

/// The first of `paths` that reads and parses as a font.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_first(paths: &[&str]) -> Option<Vec<u8>> {
    for p in paths {
        let Ok(meta) = std::fs::metadata(p) else { continue };
        if meta.len() > MAX_FONT_BYTES {
            continue;
        }
        let Ok(bytes) = std::fs::read(p) else { continue };
        if is_font(&bytes) {
            return Some(bytes);
        }
        log::warn!("font {p} could not be parsed; skipped");
    }
    None
}

/// The web build reads no files.
#[cfg(target_arch = "wasm32")]
pub fn load_first(_: &[&str]) -> Option<Vec<u8>> {
    None
}

/// The page's faces. A missing bold borrows nothing: the caller synthesizes it.
#[derive(Debug, Clone, Default)]
pub struct PageFonts {
    /// Regular text.
    pub regular: Option<Arc<[u8]>>,
    /// Bold text.
    pub bold: Option<Arc<[u8]>>,
    /// Monospace text.
    pub mono: Option<Arc<[u8]>>,
    /// Bold monospace text.
    pub mono_bold: Option<Arc<[u8]>>,
    /// Fallbacks for characters the faces above lack, in order.
    pub symbols: Vec<Arc<[u8]>>,
}

impl PageFonts {
    /// Look the faces up now (reads files; a few megabytes).
    pub fn find() -> PageFonts {
        let arc = |v: Vec<u8>| Arc::<[u8]>::from(v);
        let mut symbols: Vec<Arc<[u8]>> = Vec::new();
        for paths in SYMBOLS {
            if let Some(b) = load_first(paths) {
                symbols.push(arc(b));
            }
        }
        PageFonts {
            regular: load_first(REGULAR).map(arc),
            bold: load_first(BOLD).map(arc),
            mono: load_first(MONO).map(arc),
            mono_bold: load_first(MONO_BOLD).map(arc),
            symbols,
        }
    }
}

/// The page's faces, looked up once per process.
pub fn page_fonts() -> &'static PageFonts {
    static FONTS: OnceLock<PageFonts> = OnceLock::new();
    FONTS.get_or_init(PageFonts::find)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junk_and_missing_files_are_skipped() {
        assert!(!is_font(b"not a font"));
        assert!(!is_font(&[]));
        assert_eq!(load_first(&["/nonexistent/font.ttf", "/dev/null"]), None);
        // Whatever this machine has, every face found parses.
        let f = PageFonts::find();
        for face in [&f.regular, &f.bold, &f.mono, &f.mono_bold].into_iter().flatten().chain(&f.symbols) {
            assert!(is_font(face));
        }
    }
}
