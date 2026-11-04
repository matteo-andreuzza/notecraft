//! Rich text: runs, character formats, paragraphs, paragraph styles and lists.
//!
//! A [`Paragraph`] is a list of [`Run`]s (text with one [`CharFormat`]). Formats are
//! *overrides*: `None` means "inherit from the paragraph style", so a run in a Heading 2
//! paragraph can be explicitly un-bolded (`bold: Some(false)`). The effective format of a run is
//! [`StyleDef::char`] overlaid with the run's format ([`CharFormat::over`]); this mirrors the
//! reference format, where a paragraph references a paragraph-style object and text runs carry
//! their own formatting ([MS-ONE] `jcidParagraphStyleObject`, `TextRunFormatting`).
//!
//! Text conventions inside a paragraph: `'\n'` is a soft line break (Shift+Enter, catalog
//! `KB.General.LineBreak`), `'\t'` a tab; every other control character is dropped by
//! [`sanitize_text`] before it reaches the model. Offsets into a paragraph are **byte offsets
//! into the concatenated text, always on a char boundary** (editing in `notecraft-text` further
//! keeps them on grapheme boundaries).

use notecraft_color::Rgba;
use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::tags::TagUse;
use crate::time::Timestamp;

/// The most indent levels an outline supports: levels 0–8, i.e. the nine levels that
/// Alt+Shift+1…9 show (catalog `KB.Outline.Level1..9`).
pub const MAX_LEVEL: u8 = 8;

/// Largest font size accepted, in points (the reference's font-size box tops out around 1638).
pub const MAX_FONT_SIZE: f32 = 1638.0;
/// Smallest font size accepted, in points.
pub const MIN_FONT_SIZE: f32 = 1.0;

/// Vertical script position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Script {
    /// On the baseline.
    #[default]
    Normal,
    /// Superscript (catalog `R.Home.BasicText.Superscript`).
    Super,
    /// Subscript (catalog `R.Home.BasicText.Subscript`).
    Sub,
}

/// Character formatting overrides. Every field `None` = inherit.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CharFormat {
    /// Font family name as the user picked it (e.g. "Calibri"); the UI maps unavailable
    /// families to metric-compatible or installed fonts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// Size in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f32>,
    /// Bold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    /// Italic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    /// Underline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    /// Strikethrough.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    /// Superscript / subscript.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<Script>,
    /// Font colour (`None` = automatic).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgba>,
    /// Highlight colour (`None` = no highlight).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<Rgba>,
    /// Hyperlink target: an external URL or a `notecraft:` link to a notebook object.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

impl CharFormat {
    /// `self` overlaid on `base`: fields set in `self` win.
    pub fn over(&self, base: &CharFormat) -> CharFormat {
        CharFormat {
            font: self.font.clone().or_else(|| base.font.clone()),
            size: self.size.or(base.size),
            bold: self.bold.or(base.bold),
            italic: self.italic.or(base.italic),
            underline: self.underline.or(base.underline),
            strike: self.strike.or(base.strike),
            script: self.script.or(base.script),
            color: self.color.or(base.color),
            highlight: self.highlight.or(base.highlight),
            link: self.link.clone().or_else(|| base.link.clone()),
        }
    }

    /// This format with every number and string made safe: sizes clamped into
    /// [`MIN_FONT_SIZE`, `MAX_FONT_SIZE`] (NaN dropped), font names trimmed, control characters
    /// removed and capped at 128 chars, links capped at 4 KiB.
    pub fn sanitized(mut self) -> CharFormat {
        self.size = self.size.filter(|s| s.is_finite()).map(|s| s.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE));
        self.font =
            self.font.map(|f| f.chars().filter(|c| !c.is_control()).take(128).collect::<String>().trim().to_string()).filter(|f| !f.is_empty());
        self.link = self.link.map(|l| l.chars().filter(|c| !c.is_control()).take(4096).collect::<String>()).filter(|l| !l.is_empty());
        self
    }

    /// Whether every field is `None`.
    pub fn is_plain(&self) -> bool {
        *self == CharFormat::default()
    }
}

/// A span of text with one format. Invariant inside a normalised [`Paragraph`]: `text` is not
/// empty and differs in format from its neighbours.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Run {
    /// The text (see the module docs for `'\n'` and `'\t'`).
    pub text: String,
    /// Its formatting overrides.
    #[serde(default, skip_serializing_if = "CharFormat::is_plain")]
    pub fmt: CharFormat,
}

impl Run {
    /// A run of `text` with format `fmt` (the text is sanitized).
    pub fn new(text: &str, fmt: CharFormat) -> Run {
        Run { text: sanitize_text(text), fmt }
    }
}

/// Remove control characters other than `'\n'` and `'\t'`; turn `"\r\n"` and `'\r'` into
/// `'\n'`. Text from files, the clipboard and agents goes through this before it is stored.
pub fn sanitize_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push('\n');
            }
            '\n' | '\t' => out.push(c),
            // U+2028/U+2029 are line/paragraph separators: treat as soft breaks.
            '\u{2028}' | '\u{2029}' => out.push('\n'),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// The built-in paragraph styles of the Styles gallery (catalog `R.Home.Styles.*`), in gallery
/// order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ParaStyle {
    /// Heading 1 (Ctrl+Alt+1).
    Heading1,
    /// Heading 2 (Ctrl+Alt+2).
    Heading2,
    /// Heading 3 (Ctrl+Alt+3).
    Heading3,
    /// Heading 4 (Ctrl+Alt+4).
    Heading4,
    /// Heading 5 (Ctrl+Alt+5).
    Heading5,
    /// Heading 6 (Ctrl+Alt+6).
    Heading6,
    /// Page Title.
    PageTitle,
    /// Citation.
    Citation,
    /// Quote.
    Quote,
    /// Code.
    Code,
    /// Normal (the default).
    #[default]
    Normal,
}

/// A paragraph style's definition: the base character format and paragraph spacing.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleDef {
    /// Display name in the Styles gallery.
    pub name: &'static str,
    /// Base character format (every field set, so effective formats are complete).
    pub char: CharFormat,
    /// Space before, in points.
    pub space_before: f32,
    /// Space after, in points.
    pub space_after: f32,
}

/// The default body font (catalog `BEH.Text.DefaultFont`; changeable in Options › Advanced).
pub const DEFAULT_FONT: &str = "Calibri";
/// The default body size in points.
pub const DEFAULT_SIZE: f32 = 11.0;

impl ParaStyle {
    /// Every style in gallery order.
    pub const ALL: [ParaStyle; 11] = [
        ParaStyle::Heading1,
        ParaStyle::Heading2,
        ParaStyle::Heading3,
        ParaStyle::Heading4,
        ParaStyle::Heading5,
        ParaStyle::Heading6,
        ParaStyle::PageTitle,
        ParaStyle::Citation,
        ParaStyle::Quote,
        ParaStyle::Code,
        ParaStyle::Normal,
    ];

    /// The style's definition. Values are catalog `BEH.Text.HeadingSizes` (unverified: chosen
    /// to match the reference's user-visible look; colours are our own blues and greys).
    pub fn def(self) -> StyleDef {
        let blue = Rgba::rgb(0x23, 0x4f, 0x7d);
        let lblue = Rgba::rgb(0x2e, 0x6b, 0xa3);
        let grey = Rgba::rgb(0x59, 0x59, 0x59);
        let f = |font: &str, size: f32, bold: bool, italic: bool, color: Rgba| CharFormat {
            font: Some(font.to_string()),
            size: Some(size),
            bold: Some(bold),
            italic: Some(italic),
            underline: Some(false),
            strike: Some(false),
            script: Some(Script::Normal),
            color: Some(color),
            highlight: None,
            link: None,
        };
        let light = "Calibri Light";
        match self {
            ParaStyle::Heading1 => StyleDef { name: "Heading 1", char: f(light, 16.0, false, false, blue), space_before: 0.0, space_after: 0.0 },
            ParaStyle::Heading2 => StyleDef { name: "Heading 2", char: f(light, 14.0, false, false, lblue), space_before: 0.0, space_after: 0.0 },
            ParaStyle::Heading3 => {
                StyleDef { name: "Heading 3", char: f(DEFAULT_FONT, 12.0, true, false, lblue), space_before: 0.0, space_after: 0.0 }
            }
            ParaStyle::Heading4 => {
                StyleDef { name: "Heading 4", char: f(DEFAULT_FONT, 12.0, true, true, lblue), space_before: 0.0, space_after: 0.0 }
            }
            ParaStyle::Heading5 => StyleDef {
                name: "Heading 5",
                char: f(DEFAULT_FONT, 11.0, true, false, Rgba::rgb(0x38, 0x76, 0x1d)),
                space_before: 0.0,
                space_after: 0.0,
            },
            ParaStyle::Heading6 => StyleDef {
                name: "Heading 6",
                char: f(DEFAULT_FONT, 11.0, true, true, Rgba::rgb(0x38, 0x76, 0x1d)),
                space_before: 0.0,
                space_after: 0.0,
            },
            ParaStyle::PageTitle => {
                StyleDef { name: "Page Title", char: f(light, 20.0, false, false, Rgba::BLACK), space_before: 0.0, space_after: 0.0 }
            }
            ParaStyle::Citation => StyleDef { name: "Citation", char: f(DEFAULT_FONT, 9.0, false, false, grey), space_before: 0.0, space_after: 0.0 },
            ParaStyle::Quote => StyleDef { name: "Quote", char: f(DEFAULT_FONT, 11.0, false, true, grey), space_before: 0.0, space_after: 0.0 },
            ParaStyle::Code => StyleDef { name: "Code", char: f("Consolas", 10.0, false, false, Rgba::BLACK), space_before: 0.0, space_after: 0.0 },
            ParaStyle::Normal => {
                StyleDef { name: "Normal", char: f(DEFAULT_FONT, DEFAULT_SIZE, false, false, Rgba::BLACK), space_before: 0.0, space_after: 0.0 }
            }
        }
    }

    /// The style named `name` (case-insensitive, spaces optional: "heading1", "Heading 1").
    pub fn from_name(name: &str) -> Option<ParaStyle> {
        let key: String = name.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
        ParaStyle::ALL.into_iter().find(|s| s.def().name.replace(' ', "").to_lowercase() == key)
    }

    /// Whether this is one of Heading 1–6.
    pub fn is_heading(self) -> bool {
        matches!(
            self,
            ParaStyle::Heading1 | ParaStyle::Heading2 | ParaStyle::Heading3 | ParaStyle::Heading4 | ParaStyle::Heading5 | ParaStyle::Heading6
        )
    }
}

/// Paragraph alignment (catalog `R.Home.BasicText.AlignLeft/AlignCenter/AlignRight`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Align {
    /// Left (the default).
    #[default]
    Left,
    /// Centred.
    Center,
    /// Right.
    Right,
}

/// Bullet glyphs offered by the Bullets gallery (our own selection of common glyphs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Bullet {
    /// ● (the default for level 1).
    #[default]
    Disc,
    /// ○ (default for level 2).
    Circle,
    /// ■ (default for level 3).
    Square,
    /// –
    Dash,
    /// ➢
    Arrow,
    /// ◆
    Diamond,
    /// ✓
    Check,
    /// ★
    Star,
}

impl Bullet {
    /// The glyph drawn before the paragraph.
    pub fn glyph(self) -> &'static str {
        match self {
            Bullet::Disc => "●",
            Bullet::Circle => "○",
            Bullet::Square => "■",
            Bullet::Dash => "–",
            Bullet::Arrow => "➢",
            Bullet::Diamond => "◆",
            Bullet::Check => "✓",
            Bullet::Star => "★",
        }
    }

    /// The default bullet for an indent level (●, ○, ■ repeating; catalog
    /// `BEH.Outline.NumberingSequence`, unverified).
    pub fn for_level(level: u8) -> Bullet {
        match level % 3 {
            0 => Bullet::Disc,
            1 => Bullet::Circle,
            _ => Bullet::Square,
        }
    }

    /// Every gallery bullet.
    pub const ALL: [Bullet; 8] =
        [Bullet::Disc, Bullet::Circle, Bullet::Square, Bullet::Dash, Bullet::Arrow, Bullet::Diamond, Bullet::Check, Bullet::Star];
}

/// Number formats offered by the Numbering gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Numbering {
    /// 1. 2. 3.
    #[default]
    Decimal,
    /// 1) 2) 3)
    DecimalParen,
    /// a. b. c.
    LowerAlpha,
    /// A. B. C.
    UpperAlpha,
    /// i. ii. iii.
    LowerRoman,
    /// I. II. III.
    UpperRoman,
    /// a) b) c)
    LowerAlphaParen,
}

impl Numbering {
    /// Every gallery format.
    pub const ALL: [Numbering; 7] = [
        Numbering::Decimal,
        Numbering::DecimalParen,
        Numbering::LowerAlpha,
        Numbering::UpperAlpha,
        Numbering::LowerRoman,
        Numbering::UpperRoman,
        Numbering::LowerAlphaParen,
    ];

    /// The default format for an indent level (1., a., i. repeating; unverified).
    pub fn for_level(level: u8) -> Numbering {
        match level % 3 {
            0 => Numbering::Decimal,
            1 => Numbering::LowerAlpha,
            _ => Numbering::LowerRoman,
        }
    }

    /// The label for item number `n` (1-based; 0 is treated as 1).
    pub fn label(self, n: u32) -> String {
        let n = n.max(1);
        match self {
            Numbering::Decimal => format!("{n}."),
            Numbering::DecimalParen => format!("{n})"),
            Numbering::LowerAlpha => format!("{}.", alpha(n)),
            Numbering::UpperAlpha => format!("{}.", alpha(n).to_uppercase()),
            Numbering::LowerRoman => format!("{}.", roman(n)),
            Numbering::UpperRoman => format!("{}.", roman(n).to_uppercase()),
            Numbering::LowerAlphaParen => format!("{})", alpha(n)),
        }
    }
}

/// a, b, …, z, aa, ab, … (bijective base 26).
fn alpha(mut n: u32) -> String {
    let mut s = Vec::new();
    while n > 0 {
        n -= 1;
        s.push(b'a' + (n % 26) as u8);
        n /= 26;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// Lower-case Roman numerals; above 3999 falls back to decimal.
fn roman(n: u32) -> String {
    if n == 0 || n > 3999 {
        return n.to_string();
    }
    const T: [(u32, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut n = n;
    let mut s = String::new();
    for (v, r) in T {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

/// A paragraph's list membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListFormat {
    /// Bulleted.
    Bullet(Bullet),
    /// Numbered; `restart` restarts the count at this paragraph (context menu Restart Numbering).
    Number {
        /// Number format.
        style: Numbering,
        /// Restart the numbering at this item with this value.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        restart: Option<u32>,
    },
}

/// A paragraph of rich text inside a note container (or a page title).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    /// Stable id (Copy Link to Paragraph).
    pub id: Id,
    /// The text runs (normalised: see [`Paragraph::normalize`]).
    #[serde(default)]
    pub runs: Vec<Run>,
    /// Paragraph style.
    #[serde(default)]
    pub style: ParaStyle,
    /// Indent level 0..=[`MAX_LEVEL`] within its outline.
    #[serde(default)]
    pub level: u8,
    /// Alignment.
    #[serde(default)]
    pub align: Align,
    /// List membership.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list: Option<ListFormat>,
    /// Note tags on this paragraph, in application order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagUse>,
    /// Extra space before, in points (Paragraph Spacing Options).
    #[serde(default)]
    pub space_before: f32,
    /// Extra space after, in points.
    #[serde(default)]
    pub space_after: f32,
    /// Minimum line spacing ("at least"), in points; 0 = single.
    #[serde(default)]
    pub line_spacing: f32,
    /// Right-to-left paragraph direction.
    #[serde(default)]
    pub rtl: bool,
    /// Whether this paragraph's children (deeper paragraphs that follow) are hidden.
    #[serde(default)]
    pub collapsed: bool,
    /// Who last edited it (History › authors).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    /// When it was last edited.
    #[serde(default)]
    pub modified: Timestamp,
    /// An equation (decision D11): the paragraph's text is the equation's linear format
    /// (UnicodeMath), shown built-up or as typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub math: Option<MathView>,
}

/// How an equation shows (Equation › Professional / Linear).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MathView {
    /// Built up: fractions stacked, scripts raised (the text shows while the caret is in it).
    #[default]
    Professional,
    /// As typed, on one line.
    Linear,
}

impl Paragraph {
    /// An empty Normal paragraph.
    pub fn new(id: Id) -> Paragraph {
        Paragraph {
            id,
            runs: Vec::new(),
            style: ParaStyle::Normal,
            level: 0,
            align: Align::Left,
            list: None,
            tags: Vec::new(),
            space_before: 0.0,
            space_after: 0.0,
            line_spacing: 0.0,
            rtl: false,
            collapsed: false,
            author: String::new(),
            modified: Timestamp(0),
            math: None,
        }
    }

    /// A paragraph holding plain `text`.
    pub fn with_text(id: Id, text: &str) -> Paragraph {
        let mut p = Paragraph::new(id);
        let t = sanitize_text(text);
        if !t.is_empty() {
            p.runs.push(Run { text: t, fmt: CharFormat::default() });
        }
        p
    }

    /// The concatenated text.
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }

    /// [`Paragraph::text`] without a copy when the paragraph has at most one run (the common
    /// case), for hot read-only paths.
    pub fn text_cow(&self) -> std::borrow::Cow<'_, str> {
        match self.runs.as_slice() {
            [] => std::borrow::Cow::Borrowed(""),
            [r] => std::borrow::Cow::Borrowed(r.text.as_str()),
            _ => std::borrow::Cow::Owned(self.text()),
        }
    }

    /// Length of the text in bytes.
    pub fn len(&self) -> usize {
        self.runs.iter().map(|r| r.text.len()).sum()
    }

    /// Whether the paragraph has no text.
    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(|r| r.text.is_empty())
    }

    /// Merge adjacent runs with equal formats and drop empty runs; clamp the level and
    /// spacing. Every editing operation ends with this, so the invariants hold.
    pub fn normalize(&mut self) {
        let mut out: Vec<Run> = Vec::with_capacity(self.runs.len());
        for r in self.runs.drain(..) {
            if r.text.is_empty() {
                continue;
            }
            match out.last_mut() {
                Some(last) if last.fmt == r.fmt => last.text.push_str(&r.text),
                _ => out.push(r),
            }
        }
        self.runs = out;
        self.level = self.level.min(MAX_LEVEL);
        let clamp = |v: f32| if v.is_finite() { v.clamp(0.0, 1000.0) } else { 0.0 };
        self.space_before = clamp(self.space_before);
        self.space_after = clamp(self.space_after);
        self.line_spacing = clamp(self.line_spacing);
    }

    /// The format overrides in effect at byte `offset`: the run containing it, or for an offset
    /// on a run boundary the run *before* it (typing continues the format on the left, as word
    /// processors do). An empty paragraph has no overrides.
    pub fn fmt_at(&self, offset: usize) -> CharFormat {
        let mut pos = 0;
        let mut last: Option<&Run> = None;
        for r in &self.runs {
            let end = pos + r.text.len();
            if offset > pos && offset <= end {
                return r.fmt.clone();
            }
            if offset == 0 && pos == 0 {
                return r.fmt.clone();
            }
            pos = end;
            last = Some(r);
        }
        last.map(|r| r.fmt.clone()).unwrap_or_default()
    }

    /// The effective (fully resolved) character format at `offset`: style base overlaid with
    /// the overrides.
    pub fn effective_fmt_at(&self, offset: usize) -> CharFormat {
        self.fmt_at(offset).over(&self.style.def().char)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_controls_keeps_breaks() {
        assert_eq!(sanitize_text("a\u{0}b\r\nc\rd\te\u{2028}f\u{1b}"), "ab\nc\nd\te\nf");
    }

    #[test]
    fn normalize_merges_and_drops_empty() {
        let mut p = Paragraph::new(Id(1));
        let b = CharFormat { bold: Some(true), ..Default::default() };
        p.runs = vec![Run::new("a", CharFormat::default()), Run::new("", b.clone()), Run::new("b", CharFormat::default()), Run::new("c", b.clone())];
        p.level = 200;
        p.space_before = f32::NAN;
        p.normalize();
        assert_eq!(p.runs.len(), 2);
        assert_eq!(p.runs[0].text, "ab");
        assert_eq!(p.level, MAX_LEVEL);
        assert_eq!(p.space_before, 0.0);
    }

    #[test]
    fn fmt_at_prefers_left_run_on_boundaries() {
        let mut p = Paragraph::new(Id(1));
        let b = CharFormat { bold: Some(true), ..Default::default() };
        p.runs = vec![Run::new("ab", b.clone()), Run::new("cd", CharFormat::default())];
        assert_eq!(p.fmt_at(0), b);
        assert_eq!(p.fmt_at(2), b);
        assert_eq!(p.fmt_at(3), CharFormat::default());
        assert_eq!(p.fmt_at(99), CharFormat::default());
        assert_eq!(Paragraph::new(Id(2)).fmt_at(0), CharFormat::default());
    }

    #[test]
    fn styles_resolve_and_parse() {
        // parity: R.Home.Styles.Heading1
        assert_eq!(ParaStyle::from_name("heading 1"), Some(ParaStyle::Heading1));
        assert_eq!(ParaStyle::from_name("PageTitle"), Some(ParaStyle::PageTitle));
        assert_eq!(ParaStyle::from_name("nope"), None);
        for s in ParaStyle::ALL {
            let d = s.def();
            assert!(d.char.size.is_some() && d.char.font.is_some() && d.char.bold.is_some(), "{s:?} base format must be complete");
        }
        let mut p = Paragraph::with_text(Id(1), "x");
        p.style = ParaStyle::Heading3;
        assert_eq!(p.effective_fmt_at(0).bold, Some(true));
        p.runs[0].fmt.bold = Some(false);
        assert_eq!(p.effective_fmt_at(0).bold, Some(false));
    }

    #[test]
    fn list_labels() {
        assert_eq!(Numbering::Decimal.label(3), "3.");
        assert_eq!(Numbering::LowerAlpha.label(27), "aa.");
        assert_eq!(Numbering::UpperRoman.label(14), "XIV.");
        assert_eq!(Numbering::LowerRoman.label(4000), "4000.");
        assert_eq!(Numbering::DecimalParen.label(0), "1)");
        assert_eq!(Bullet::for_level(4), Bullet::Circle);
    }

    #[test]
    fn charformat_sanitized() {
        let f = CharFormat { size: Some(f32::INFINITY), font: Some("  \u{0}  ".into()), link: Some(String::new()), ..Default::default() }.sanitized();
        assert_eq!(f.size, None);
        assert_eq!(f.font, None);
        assert_eq!(f.link, None);
        let g = CharFormat { size: Some(99999.0), ..Default::default() }.sanitized();
        assert_eq!(g.size, Some(MAX_FONT_SIZE));
    }
}
