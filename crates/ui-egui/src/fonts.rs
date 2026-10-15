//! Fonts for the interface and the page.
//!
//! NoteCraft bundles no font files (AGENTS.md › Clean room). The **interface** and the **page**
//! use separate families (decision D1, `docs/ui-parity.md` §2.3):
//!
//! - Interface: Inter from `CRAFT_FONTS_DIR` (a craft-fonts checkout, read at run time or baked
//!   in at build time) or from the system, else Noto Sans, Cantarell, DejaVu Sans; in regular,
//!   medium and semibold, each weight falling back to the nearest one the family has.
//! - Page: for the reference's default "Calibri" the metric-compatible Carlito, then common open
//!   sans-serif families; "Consolas" (the Code style) maps to an open monospace family.
//!
//! Each file is validated with `skrifa` before egui sees it, because egui cannot recover from a
//! font it fails to parse. With no usable fonts (the web build, minimal systems) egui's built-in
//! fonts are used and bold is unavailable (a documented gap).
//!
//! Families registered: `Proportional` (interface regular), named `medium` and `semibold`
//! (interface), `page` and `bold` (page), `Monospace` and `mono-bold`. Fonts set with
//! `set_fonts` only take effect on the next pass, so callers ask [`ready`] before naming a family.

use egui::{FontData, FontDefinitions, FontFamily};

/// Interface faces, most preferred family first: (regular, medium, semibold) candidates per family.
/// Inter is looked up in `CRAFT_FONTS_DIR` before these system paths (see [`craft_inter`]).
const UI_FAMILIES: &[[&[&str]; 3]] = &[
    [
        &[
            "/usr/share/fonts/inter/Inter-Regular.ttf",
            "/usr/share/fonts/TTF/Inter-Regular.ttf",
            "/usr/share/fonts/truetype/inter/Inter-Regular.ttf",
            "/usr/share/fonts/opentype/inter/Inter-Regular.otf",
        ],
        &[
            "/usr/share/fonts/inter/Inter-Medium.ttf",
            "/usr/share/fonts/TTF/Inter-Medium.ttf",
            "/usr/share/fonts/truetype/inter/Inter-Medium.ttf",
            "/usr/share/fonts/opentype/inter/Inter-Medium.otf",
        ],
        &[
            "/usr/share/fonts/inter/Inter-SemiBold.ttf",
            "/usr/share/fonts/TTF/Inter-SemiBold.ttf",
            "/usr/share/fonts/truetype/inter/Inter-SemiBold.ttf",
            "/usr/share/fonts/opentype/inter/Inter-SemiBold.otf",
        ],
    ],
    [
        &[
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
        ],
        &[
            "/usr/share/fonts/noto/NotoSans-Medium.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Medium.ttf",
            "/usr/share/fonts/google-noto/NotoSans-Medium.ttf",
        ],
        &[
            "/usr/share/fonts/noto/NotoSans-SemiBold.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf",
            "/usr/share/fonts/google-noto/NotoSans-SemiBold.ttf",
            "/usr/share/fonts/noto/NotoSans-Bold.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
            "/usr/share/fonts/google-noto/NotoSans-Bold.ttf",
        ],
    ],
    [
        &["/usr/share/fonts/cantarell/Cantarell-Regular.otf", "/usr/share/fonts/opentype/cantarell/Cantarell-Regular.otf"],
        &[],
        &["/usr/share/fonts/cantarell/Cantarell-Bold.otf", "/usr/share/fonts/opentype/cantarell/Cantarell-Bold.otf"],
    ],
    [
        &[
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
        ],
        &[],
        &[
            "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
            "C:\\Windows\\Fonts\\arialbd.ttf",
        ],
    ],
];

/// Page candidate files per role, most preferred first (Linux and BSD paths; macOS and Windows
/// paths follow).
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

/// Inter's three weights from a craft-fonts checkout: `CRAFT_FONTS_DIR` at run time, else the
/// one baked in at build time. The checkout may keep the files at its root or under `inter/`.
#[cfg(not(target_arch = "wasm32"))]
fn craft_inter() -> Option<[Option<Vec<u8>>; 3]> {
    let dir = std::env::var("CRAFT_FONTS_DIR").ok().or_else(|| option_env!("CRAFT_FONTS_DIR").map(str::to_string))?;
    let base = std::path::Path::new(&dir);
    for sub in ["", "inter", "fonts/inter", "Inter"] {
        let at = |w: &str| base.join(sub).join(format!("Inter-{w}.ttf")).to_string_lossy().into_owned();
        let paths = [at("Regular"), at("Medium"), at("SemiBold")];
        let regular = load_first(&[paths[0].as_str()]);
        if regular.is_some() {
            return Some([regular, load_first(&[paths[1].as_str()]), load_first(&[paths[2].as_str()])]);
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn craft_inter() -> Option<[Option<Vec<u8>>; 3]> {
    None
}

/// The interface family's (regular, medium, semibold) faces: the first family whose regular face
/// loads; a missing weight borrows the nearest one (medium → regular, semibold → medium).
fn ui_faces() -> Option<[Vec<u8>; 3]> {
    let found = craft_inter().or_else(|| {
        UI_FAMILIES.iter().find_map(|[r, m, s]| {
            let regular = load_first(r)?;
            Some([Some(regular), load_first(m), load_first(s)])
        })
    })?;
    let [regular, medium, semibold] = found;
    let regular = regular?;
    let medium = medium.unwrap_or_else(|| regular.clone());
    let semibold = semibold.unwrap_or_else(|| medium.clone());
    Some([regular, medium, semibold])
}

/// Install fonts into `ctx` (call once). They take effect on the next pass.
pub fn install(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let fallback_prop: Vec<String> = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let fallback_mono: Vec<String> = defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    let add = |defs: &mut FontDefinitions, name: &str, bytes: Vec<u8>| {
        defs.font_data.insert(name.to_string(), std::sync::Arc::new(FontData::from_owned(bytes)));
    };
    let family =
        |first: Option<&str>, rest: &[String]| -> Vec<String> { first.map(str::to_string).into_iter().chain(rest.iter().cloned()).collect() };
    // Interface: Proportional is the regular weight, so every egui widget uses it.
    let ui = ui_faces();
    let mut ui_names: [Option<&str>; 3] = [None, None, None];
    if let Some([regular, medium, semibold]) = ui {
        add(&mut defs, "nc-ui", regular);
        add(&mut defs, "nc-ui-medium", medium);
        add(&mut defs, "nc-ui-semibold", semibold);
        ui_names = [Some("nc-ui"), Some("nc-ui-medium"), Some("nc-ui-semibold")];
    }
    defs.families.insert(FontFamily::Proportional, family(ui_names[0], &fallback_prop));
    defs.families.insert(FontFamily::Name("medium".into()), family(ui_names[1], &fallback_prop));
    defs.families.insert(FontFamily::Name("semibold".into()), family(ui_names[2], &fallback_prop));
    // Page.
    let page = load_first(REGULAR).map(|b| add(&mut defs, "nc-page", b)).map(|_| "nc-page");
    defs.families.insert(FontFamily::Name("page".into()), family(page, &fallback_prop));
    if let Some(b) = load_first(BOLD) {
        add(&mut defs, "nc-bold", b);
        defs.families.insert(FontFamily::Name("bold".into()), family(Some("nc-bold"), &fallback_prop));
    }
    if let Some(b) = load_first(MONO) {
        add(&mut defs, "nc-mono", b);
        defs.families.insert(FontFamily::Monospace, family(Some("nc-mono"), &fallback_mono));
    }
    if let Some(b) = load_first(MONO_BOLD) {
        add(&mut defs, "nc-mono-bold", b);
        defs.families.insert(FontFamily::Name("mono-bold".into()), family(Some("nc-mono-bold"), &fallback_mono));
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
    let names: Vec<String> = defs.families.keys().filter_map(|f| if let FontFamily::Name(n) = f { Some(n.to_string()) } else { None }).collect();
    ctx.set_fonts(defs);
    let installed = Installed { pass: ctx.cumulative_pass_nr(), names: std::sync::Arc::new(names) };
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(INSTALLED), installed));
}

/// Key of the [`Installed`] record in egui's memory.
const INSTALLED: &str = "notecraft-fonts-installed";

/// Which named families [`install`] registered, and in which pass.
#[derive(Clone, Default)]
struct Installed {
    pass: u64,
    names: std::sync::Arc<Vec<String>>,
}

/// Whether the named family is active: installed, and past the pass that installed it (fonts set
/// with `set_fonts` only take effect on the next pass). Naming an unknown family makes egui panic,
/// so every named-family use goes through this. It never asks egui's font store, which itself
/// panics before the first pass.
pub fn ready(ctx: &egui::Context, name: &str) -> bool {
    let installed: Option<Installed> = ctx.data(|d| d.get_temp(egui::Id::new(INSTALLED)));
    installed.is_some_and(|i| ctx.cumulative_pass_nr() > i.pass && i.names.iter().any(|n| n == name))
}

/// Whether a bold face is available.
pub fn has_bold(ctx: &egui::Context) -> bool {
    ready(ctx, "bold")
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
        if ready(ctx, name) {
            return FontFamily::Name(name.into());
        }
    }
    if mono {
        FontFamily::Monospace
    } else if ready(ctx, "page") {
        FontFamily::Name("page".into())
    } else {
        FontFamily::Proportional
    }
}
