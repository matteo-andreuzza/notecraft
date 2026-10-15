//! Fonts for the interface and the page.
//!
//! NoteCraft bundles no font files (AGENTS.md › Clean room). On the desktop it uses fonts
//! installed on the system: for the page's default "Calibri" it prefers the metric-compatible
//! Carlito, then common open sans-serif families; "Consolas" (the Code style) maps to an open
//! monospace family. Each file is validated with `skrifa` before egui sees it, because egui cannot
//! recover from a font it fails to parse. With no usable system fonts (the web build, minimal
//! systems) egui's built-in fonts are used and bold is unavailable (a documented gap).
//!
//! Families registered: `Proportional` (regular), `Monospace`, and named families `bold`,
//! `mono-bold`. Italic is synthesized by egui's text layout (`TextFormat::italics`).

use egui::{FontData, FontDefinitions, FontFamily};

/// Candidate files per role, most preferred first (Linux and BSD paths; macOS and Windows paths
/// follow).
const REGULAR: &[&str] = &[
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
const BOLD: &[&str] = &[
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
const MONO: &[&str] = &[
    "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
    "/usr/share/fonts/liberation/LiberationMono-Regular.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/System/Library/Fonts/Supplemental/Courier New.ttf",
    "C:\\Windows\\Fonts\\consola.ttf",
];
const MONO_BOLD: &[&str] = &[
    "/usr/share/fonts/noto/NotoSansMono-Bold.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansMono-Bold.ttf",
    "/usr/share/fonts/liberation/LiberationMono-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono-Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf",
    "C:\\Windows\\Fonts\\consolab.ttf",
];

/// Symbol and math fallbacks (arrows, geometric shapes, math operators) appended to every family,
/// so glyphs the main face lacks still render.
const SYMBOLS: &[&[&str]] = &[
    &["/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf", "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf"],
    &["/usr/share/fonts/noto/NotoSansSymbols-Regular.ttf", "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf"],
    &["/usr/share/fonts/noto/NotoSansMath-Regular.ttf", "/usr/share/fonts/truetype/noto/NotoSansMath-Regular.ttf"],
    &["/usr/share/fonts/TTF/DejaVuSans.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"],
];

/// Largest font file accepted (hostile or broken files are skipped).
#[cfg(not(target_arch = "wasm32"))]
const MAX_FONT_BYTES: u64 = 64 << 20;

#[cfg(not(target_arch = "wasm32"))]
fn load_first(paths: &[&str]) -> Option<Vec<u8>> {
    for p in paths {
        let Ok(meta) = std::fs::metadata(p) else { continue };
        if meta.len() > MAX_FONT_BYTES {
            continue;
        }
        let Ok(bytes) = std::fs::read(p) else { continue };
        if skrifa::FontRef::new(&bytes).is_ok() {
            return Some(bytes);
        }
        log::warn!("font {p} could not be parsed; skipped");
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn load_first(_: &[&str]) -> Option<Vec<u8>> {
    None
}

/// Install fonts into `ctx` (call once).
pub fn install(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let fallback_prop: Vec<String> = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let fallback_mono: Vec<String> = defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    let add = |defs: &mut FontDefinitions, name: &str, bytes: Vec<u8>| {
        defs.font_data.insert(name.to_string(), std::sync::Arc::new(FontData::from_owned(bytes)));
    };
    let regular = load_first(REGULAR);
    let bold = load_first(BOLD);
    let mono = load_first(MONO);
    let mono_bold = load_first(MONO_BOLD);
    if let Some(b) = regular {
        add(&mut defs, "nc-regular", b);
        let mut fam = vec!["nc-regular".to_string()];
        fam.extend(fallback_prop.iter().cloned());
        defs.families.insert(FontFamily::Proportional, fam);
    }
    if let Some(b) = bold {
        add(&mut defs, "nc-bold", b);
        let mut fam = vec!["nc-bold".to_string()];
        fam.extend(fallback_prop.iter().cloned());
        defs.families.insert(FontFamily::Name("bold".into()), fam);
    }
    if let Some(b) = mono {
        add(&mut defs, "nc-mono", b);
        let mut fam = vec!["nc-mono".to_string()];
        fam.extend(fallback_mono.iter().cloned());
        defs.families.insert(FontFamily::Monospace, fam);
    }
    if let Some(b) = mono_bold {
        add(&mut defs, "nc-mono-bold", b);
        let mut fam = vec!["nc-mono-bold".to_string()];
        fam.extend(fallback_mono.iter().cloned());
        defs.families.insert(FontFamily::Name("mono-bold".into()), fam);
    }
    let mut symbols = Vec::new();
    for (i, paths) in SYMBOLS.iter().enumerate() {
        if let Some(b) = load_first(paths) {
            let name = format!("nc-symbols-{i}");
            add(&mut defs, &name, b);
            symbols.push(name);
        }
    }
    for fam in defs.families.values_mut() {
        fam.extend(symbols.iter().cloned());
    }
    ctx.set_fonts(defs);
}

/// Whether a bold face is available.
pub fn has_bold(ctx: &egui::Context) -> bool {
    ctx.fonts(|f| f.families().iter().any(|fam| *fam == FontFamily::Name("bold".into())))
}

/// Whether a font name is a monospace family.
pub fn is_mono(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["consolas", "courier", "mono", "menlo", "lucida console", "source code"].iter().any(|m| n.contains(m))
}

/// The egui family for a run.
pub fn family(ctx: &egui::Context, font: &str, bold: bool) -> FontFamily {
    let mono = is_mono(font);
    if bold {
        let name = if mono { "mono-bold" } else { "bold" };
        if ctx.fonts(|f| f.families().iter().any(|fam| *fam == FontFamily::Name(name.into()))) {
            return FontFamily::Name(name.into());
        }
    }
    if mono { FontFamily::Monospace } else { FontFamily::Proportional }
}
