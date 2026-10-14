//! NoteCraft colours (layer **L0**: depends only on `serde`).
//!
//! An [`Rgba`] is 8-bit sRGB with straight alpha: that is what note text, highlights, section
//! tabs and page backgrounds need, and what the reference product stores (a `COLORREF`-style
//! 24-bit colour, [MS-ONE] `FontColor`, `Highlight`, `NotebookColor`). Pixel buffers for
//! pictures and ink rendering carry their own format at runtime (M2); this type is for UI and
//! document colour *values*.
//!
//! The palettes here are NoteCraft's own choices. Their *structure* follows the reference (16
//! section colours plus "None", a highlighter palette, a page-colour palette and a font-colour
//! grid with standard colours); the highlighter palette uses the colour names and values of the
//! public ECMA-376 `ST_HighlightColor` enumeration, which Office documents share.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An 8-bit sRGB colour with straight (non-premultiplied) alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgba {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha (255 = opaque).
    pub a: u8,
}

impl Rgba {
    /// Opaque black.
    pub const BLACK: Rgba = Rgba::rgb(0, 0, 0);
    /// Opaque white.
    pub const WHITE: Rgba = Rgba::rgb(255, 255, 255);
    /// Fully transparent.
    pub const TRANSPARENT: Rgba = Rgba { r: 0, g: 0, b: 0, a: 0 };

    /// An opaque colour.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
        Rgba { r, g, b, a: 255 }
    }

    /// A colour with alpha.
    pub const fn with_alpha(r: u8, g: u8, b: u8, a: u8) -> Rgba {
        Rgba { r, g, b, a }
    }

    /// Parse `#rgb`, `#rrggbb` or `#rrggbbaa` (the `#` is optional). Returns `None` for anything
    /// else — colour strings come from files and agents and are untrusted.
    pub fn from_hex(s: &str) -> Option<Rgba> {
        let h = s.trim().strip_prefix('#').unwrap_or(s.trim());
        if !h.is_ascii() {
            return None;
        }
        let nib = |i: usize| h.get(i..i + 1).and_then(|c| u8::from_str_radix(c, 16).ok());
        let byte = |i: usize| h.get(i..i + 2).and_then(|c| u8::from_str_radix(c, 16).ok());
        match h.len() {
            3 => Some(Rgba::rgb(nib(0)? * 17, nib(1)? * 17, nib(2)? * 17)),
            6 => Some(Rgba::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Rgba::with_alpha(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => None,
        }
    }

    /// `#rrggbb` (or `#rrggbbaa` when not opaque).
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// Relative luminance (ITU-R BT.709 weights on gamma-encoded values; good enough to pick a
    /// readable text colour over a swatch).
    pub fn luma(self) -> f32 {
        (0.2126 * self.r as f32 + 0.7152 * self.g as f32 + 0.0722 * self.b as f32) / 255.0
    }

    /// Mix toward `other` by `t` in `[0, 1]` (clamped; NaN counts as 0).
    pub fn mix(self, other: Rgba, t: f32) -> Rgba {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8;
        Rgba { r: m(self.r, other.r), g: m(self.g, other.g), b: m(self.b, other.b), a: m(self.a, other.a) }
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Rgba, D::Error> {
        let s = String::deserialize(d)?;
        Rgba::from_hex(&s).ok_or_else(|| serde::de::Error::custom(format!("bad colour `{s}`")))
    }
}

/// A named palette entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Swatch {
    /// Functional name shown in tooltips.
    pub name: &'static str,
    /// The colour.
    pub color: Rgba,
}

const fn sw(name: &'static str, r: u8, g: u8, b: u8) -> Swatch {
    Swatch { name, color: Rgba::rgb(r, g, b) }
}

/// The 16 section colours, in menu order (catalog `CM.Section.NotebookColor1..16`). Index 0 is
/// "Color 1". Values are NoteCraft's own medium tones that keep black tab text readable.
pub const SECTION_COLORS: [Swatch; 16] = [
    sw("Blue", 0x8a, 0xa8, 0xe4),
    sw("Yellow", 0xf5, 0xd9, 0x6b),
    sw("Green", 0x9c, 0xcf, 0x7f),
    sw("Red", 0xee, 0x95, 0x88),
    sw("Purple", 0xb6, 0x9c, 0xd8),
    sw("Cyan", 0x84, 0xcf, 0xd9),
    sw("Orange", 0xf4, 0xb0, 0x6e),
    sw("Magenta", 0xe4, 0x96, 0xc8),
    sw("Light Blue", 0xb5, 0xc9, 0xec),
    sw("Lavender", 0xcf, 0xc3, 0xe6),
    sw("Tan", 0xd9, 0xc4, 0x9f),
    sw("Lime", 0xd2, 0xe3, 0x82),
    sw("Light Green", 0xb7, 0xe0, 0xb0),
    sw("Teal", 0x7f, 0xbf, 0xb4),
    sw("Rose", 0xe7, 0xb4, 0xb4),
    sw("Silver", 0xc4, 0xc8, 0xcc),
];

/// Neutral tab colour for a section whose colour is "None".
pub const SECTION_NONE: Rgba = Rgba::rgb(0xe8, 0xe8, 0xe8);

/// The text-highlighter palette: ECMA-376 Part 1 §17.18.40 `ST_HighlightColor` names and values
/// (public standard; the same set Office documents use). The first entry is the default
/// highlighter colour.
pub const HIGHLIGHT_COLORS: [Swatch; 15] = [
    sw("Yellow", 0xff, 0xff, 0x00),
    sw("Bright Green", 0x00, 0xff, 0x00),
    sw("Turquoise", 0x00, 0xff, 0xff),
    sw("Pink", 0xff, 0x00, 0xff),
    sw("Blue", 0x00, 0x00, 0xff),
    sw("Red", 0xff, 0x00, 0x00),
    sw("Dark Blue", 0x00, 0x00, 0x80),
    sw("Teal", 0x00, 0x80, 0x80),
    sw("Green", 0x00, 0x80, 0x00),
    sw("Violet", 0x80, 0x00, 0x80),
    sw("Dark Red", 0x80, 0x00, 0x00),
    sw("Dark Yellow", 0x80, 0x80, 0x00),
    sw("Gray 50%", 0x80, 0x80, 0x80),
    sw("Gray 25%", 0xc0, 0xc0, 0xc0),
    sw("Black", 0x00, 0x00, 0x00),
];

/// The default highlighter colour (yellow).
pub const DEFAULT_HIGHLIGHT: Rgba = Rgba::rgb(0xff, 0xff, 0x00);

/// Page background colours (View › Page Color), our own light tints; "No Color" is `None`.
pub const PAGE_COLORS: [Swatch; 16] = [
    sw("Light Blue", 0xdc, 0xe9, 0xf7),
    sw("Light Yellow", 0xfb, 0xf4, 0xd0),
    sw("Light Green", 0xe2, 0xf1, 0xd8),
    sw("Light Red", 0xf8, 0xdf, 0xdb),
    sw("Light Purple", 0xe8, 0xdf, 0xf3),
    sw("Light Cyan", 0xda, 0xf0, 0xf2),
    sw("Light Orange", 0xfb, 0xe6, 0xd2),
    sw("Light Magenta", 0xf6, 0xdd, 0xec),
    sw("Pale Blue", 0xe9, 0xef, 0xfa),
    sw("Pale Lavender", 0xf0, 0xec, 0xf7),
    sw("Pale Tan", 0xf3, 0xec, 0xdf),
    sw("Pale Lime", 0xf1, 0xf6, 0xd9),
    sw("Pale Green", 0xea, 0xf6, 0xe8),
    sw("Pale Teal", 0xdd, 0xee, 0xeb),
    sw("Pale Rose", 0xf7, 0xe9, 0xe9),
    sw("Pale Gray", 0xee, 0xef, 0xf0),
];

/// Base hues of the font-colour grid's top row (our own theme), left to right.
pub const FONT_THEME: [Swatch; 10] = [
    sw("White", 0xff, 0xff, 0xff),
    sw("Black", 0x00, 0x00, 0x00),
    sw("Paper", 0xe9, 0xe6, 0xdf),
    sw("Slate", 0x3b, 0x46, 0x55),
    sw("Violet", 0x6e, 0x3f, 0xb0),
    sw("Coral", 0xd9, 0x5d, 0x39),
    sw("Moss", 0x5c, 0x8a, 0x3a),
    sw("Gold", 0xd1, 0x9a, 0x14),
    sw("Ocean", 0x2a, 0x6f, 0xb0),
    sw("Teal", 0x1f, 0x8a, 0x80),
];

/// The "Standard Colors" row of the font-colour picker (functional names shared by Office
/// colour pickers; values are ours).
pub const STANDARD_COLORS: [Swatch; 10] = [
    sw("Dark Red", 0xc0, 0x00, 0x00),
    sw("Red", 0xff, 0x00, 0x00),
    sw("Orange", 0xff, 0xc0, 0x00),
    sw("Yellow", 0xff, 0xff, 0x00),
    sw("Light Green", 0x92, 0xd0, 0x50),
    sw("Green", 0x00, 0xb0, 0x50),
    sw("Light Blue", 0x00, 0xb0, 0xf0),
    sw("Blue", 0x00, 0x70, 0xc0),
    sw("Dark Blue", 0x00, 0x20, 0x60),
    sw("Purple", 0x70, 0x30, 0xa0),
];

/// The font-colour grid below the theme row: for each theme hue, five tints/shades (lighter
/// 80 %, 60 %, 40 %, darker 25 %, 50 %), as the reference's picker lays them out.
pub fn font_grid() -> Vec<[Rgba; 5]> {
    FONT_THEME
        .iter()
        .map(|s| {
            let c = s.color;
            // Pure black/white rows invert direction so every cell is distinguishable.
            if c == Rgba::WHITE {
                [c.mix(Rgba::BLACK, 0.05), c.mix(Rgba::BLACK, 0.15), c.mix(Rgba::BLACK, 0.25), c.mix(Rgba::BLACK, 0.35), c.mix(Rgba::BLACK, 0.5)]
            } else if c == Rgba::BLACK {
                [c.mix(Rgba::WHITE, 0.5), c.mix(Rgba::WHITE, 0.35), c.mix(Rgba::WHITE, 0.25), c.mix(Rgba::WHITE, 0.15), c.mix(Rgba::WHITE, 0.05)]
            } else {
                [c.mix(Rgba::WHITE, 0.8), c.mix(Rgba::WHITE, 0.6), c.mix(Rgba::WHITE, 0.4), c.mix(Rgba::BLACK, 0.25), c.mix(Rgba::BLACK, 0.5)]
            }
        })
        .collect()
}

/// The default font colour ("Automatic"): black text.
pub const AUTOMATIC_TEXT: Rgba = Rgba::BLACK;

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn hex_round_trip_and_rejects_junk() {
        assert_eq!(Rgba::from_hex("#ff8000"), Some(Rgba::rgb(255, 128, 0)));
        assert_eq!(Rgba::from_hex("f80"), Some(Rgba::rgb(255, 136, 0)));
        assert_eq!(Rgba::from_hex("#11223344"), Some(Rgba::with_alpha(0x11, 0x22, 0x33, 0x44)));
        for bad in ["", "#", "#12345", "zzzzzz", "#ééé", "#1234567890"] {
            assert_eq!(Rgba::from_hex(bad), None, "{bad}");
        }
        assert_eq!(Rgba::rgb(1, 2, 3).to_hex(), "#010203");
        assert_eq!(Rgba::with_alpha(1, 2, 3, 4).to_hex(), "#01020304");
    }

    #[test]
    fn serde_as_hex_string() {
        let s = serde_json::to_string(&Rgba::rgb(0xab, 0xcd, 0xef)).unwrap();
        assert_eq!(s, "\"#abcdef\"");
        assert_eq!(serde_json::from_str::<Rgba>(&s).unwrap(), Rgba::rgb(0xab, 0xcd, 0xef));
        assert!(serde_json::from_str::<Rgba>("\"nope\"").is_err());
    }

    #[test]
    fn palettes_have_the_reference_structure() {
        // 16 section colours + None (catalog CM.Section.NotebookColor1..16, NotebookColorNone).
        assert_eq!(SECTION_COLORS.len(), 16);
        assert_eq!(font_grid().len(), FONT_THEME.len());
        // Black tab text stays readable on every section colour.
        assert!(SECTION_COLORS.iter().all(|s| s.color.luma() > 0.45), "a section colour is too dark for black text");
        // Every swatch name is unique within its palette.
        for pal in [&SECTION_COLORS[..], &HIGHLIGHT_COLORS[..], &PAGE_COLORS[..], &STANDARD_COLORS[..]] {
            let mut names: Vec<_> = pal.iter().map(|s| s.name).collect();
            names.sort_unstable();
            names.dedup();
            assert_eq!(names.len(), pal.len());
        }
    }

    proptest! {
        // Property: any string either parses to a colour whose hex form parses back to it, or is rejected — never a panic.
        #[test]
        fn from_hex_never_panics(s in ".{0,12}") {
            if let Some(c) = Rgba::from_hex(&s) {
                prop_assert_eq!(Rgba::from_hex(&c.to_hex()), Some(c));
            }
        }
        #[test]
        fn mix_is_bounded(t in any::<f32>()) {
            let m = Rgba::BLACK.mix(Rgba::WHITE, t);
            prop_assert!(m.r == m.g && m.g == m.b);
        }
    }
}
