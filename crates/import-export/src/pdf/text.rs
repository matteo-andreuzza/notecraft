//! Text for the PDF: the page's faces, glyph lookup with fallback, and paragraph layout
//! (greedy line breaking at spaces, the way the canvas wraps), measured with `skrifa` so what
//! is measured is exactly what is drawn.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use notecraft_doc::color::Rgba;
use notecraft_doc::{Paragraph, Script};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};

/// A link's colour (the canvas's light theme).
pub(super) const LINK: Rgba = Rgba::rgb(0x05, 0x63, 0xC1);
/// Default text colour.
pub(super) const TEXT: Rgba = Rgba::rgb(0, 0, 0);
/// Tab stops every half inch.
const TAB: f32 = 36.0;

/// One font file.
pub(super) struct Face {
    pub(super) font: krilla::text::Font,
    data: Arc<[u8]>,
    upem: f32,
    /// Ascent, descent (positive, below the baseline) and line gap, per point of size.
    pub(super) ascent: f32,
    pub(super) descent: f32,
    gap: f32,
    cache: HashMap<char, Option<(u32, f32)>>,
}

impl Face {
    fn new(data: &Arc<[u8]>) -> Option<Face> {
        let fr = skrifa::FontRef::new(data).ok()?;
        let m = fr.metrics(Size::unscaled(), LocationRef::default());
        let upem = f32::from(m.units_per_em.max(1));
        let font = krilla::text::Font::new(krilla::Data::from(data.to_vec()), 0)?;
        let (ascent, descent, gap) = (m.ascent / upem, -m.descent / upem, m.leading / upem);
        let ok = |v: f32| v.is_finite() && v.abs() < 4.0;
        Some(Face {
            font,
            data: data.clone(),
            upem,
            ascent: if ok(ascent) && ascent > 0.0 { ascent } else { 0.8 },
            descent: if ok(descent) && descent >= 0.0 { descent } else { 0.2 },
            gap: if ok(gap) && gap >= 0.0 { gap } else { 0.0 },
            cache: HashMap::new(),
        })
    }

    /// The glyph and its advance (per point of size) for `c`, if the face has it.
    fn glyph(&mut self, c: char) -> Option<(u32, f32)> {
        if let Some(g) = self.cache.get(&c) {
            return *g;
        }
        let found = skrifa::FontRef::new(&self.data).ok().and_then(|fr| {
            let gid = fr.charmap().map(c)?;
            if gid.to_u32() == 0 {
                return None;
            }
            let adv = fr.glyph_metrics(Size::unscaled(), LocationRef::default()).advance_width(gid).unwrap_or(0.0) / self.upem;
            Some((gid.to_u32(), if adv.is_finite() { adv.clamp(0.0, 8.0) } else { 0.0 }))
        });
        self.cache.insert(c, found);
        found
    }

    /// Font units per em.
    pub(super) fn upem(&self) -> f32 {
        self.upem
    }

    /// Draw glyph `gid`'s outline, in font units with y up, into `pen`; false when the face
    /// has no such glyph or it fails to draw.
    pub(super) fn draw_outline(&self, gid: u32, pen: &mut impl skrifa::outline::OutlinePen) -> bool {
        let Ok(fr) = skrifa::FontRef::new(&self.data) else { return false };
        let Some(g) = fr.outline_glyphs().get(skrifa::GlyphId::new(gid)) else { return false };
        g.draw(skrifa::outline::DrawSettings::unhinted(Size::unscaled(), LocationRef::default()), pen).is_ok()
    }

    /// Line height per point of size.
    fn line(&self) -> f32 {
        self.ascent + self.descent + self.gap
    }
}

/// The faces of one export: regular, bold, monospace, bold monospace and fallbacks.
pub(super) struct Faces {
    pub(super) faces: Vec<Face>,
    regular: usize,
    bold: Option<usize>,
    mono: Option<usize>,
    mono_bold: Option<usize>,
    fallbacks: Vec<usize>,
}

/// Whether a font name means a monospace face (as the canvas decides).
fn is_mono(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["consolas", "courier", "mono", "menlo", "lucida console", "source code"].iter().any(|m| n.contains(m))
}

impl Faces {
    /// The faces found on the system; `None` without a usable regular face.
    pub(super) fn new(f: &notecraft_fonts::PageFonts) -> Option<Faces> {
        let mut faces = Vec::new();
        let mut add = |d: &Option<Arc<[u8]>>| -> Option<usize> {
            let face = Face::new(d.as_ref()?)?;
            faces.push(face);
            Some(faces.len() - 1)
        };
        let regular = add(&f.regular)?;
        let (bold, mono, mono_bold) = (add(&f.bold), add(&f.mono), add(&f.mono_bold));
        let fallbacks = f.symbols.iter().filter_map(|d| add(&Some(d.clone()))).collect();
        Some(Faces { faces, regular, bold, mono, mono_bold, fallbacks })
    }

    /// The face for a run and whether bold must be drawn (no bold face).
    fn pick(&self, font: &str, bold: bool) -> (usize, bool) {
        let (base, b) = if is_mono(font) { (self.mono.unwrap_or(self.regular), self.mono_bold) } else { (self.regular, self.bold) };
        match (bold, b) {
            (true, Some(b)) => (b, false),
            (true, None) => (base, true),
            (false, _) => (base, false),
        }
    }

    /// The face, glyph and advance (per point) for `c`: the run's face, then the regular face,
    /// then the fallbacks; else the run face's missing glyph (`.notdef`).
    fn glyph(&mut self, c: char, face: usize) -> (usize, u32, f32) {
        let order: Vec<usize> = std::iter::once(face).chain(std::iter::once(self.regular)).chain(self.fallbacks.iter().copied()).collect();
        for f in order {
            if let Some((g, a)) = self.faces.get_mut(f).and_then(|x| x.glyph(c)) {
                return (f, g, a);
            }
        }
        (face, 0, 0.5)
    }

    pub(super) fn face(&self, i: usize) -> Option<&Face> {
        self.faces.get(i)
    }
}

/// How a run looks.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Look {
    pub(super) size: f32,
    pub(super) color: Rgba,
    pub(super) italic: bool,
    pub(super) fake_bold: bool,
    pub(super) underline: bool,
    pub(super) strike: bool,
    pub(super) highlight: Option<Rgba>,
    pub(super) link: Option<String>,
    /// Baseline shift up (superscript) or down (negative, subscript), points.
    pub(super) rise: f32,
}

/// One character placed on a line.
#[derive(Debug, Clone)]
pub(super) struct Glyph {
    pub(super) c: char,
    pub(super) face: usize,
    pub(super) gid: u32,
    /// Advance, points.
    pub(super) adv: f32,
    pub(super) look: usize,
    space: bool,
    hard_break: bool,
    tab: bool,
}

/// A laid-out line: its glyphs and metrics (points).
#[derive(Debug, Clone)]
pub(super) struct Line {
    pub(super) glyphs: Range<usize>,
    pub(super) width: f32,
    pub(super) ascent: f32,
    pub(super) height: f32,
}

/// A laid-out paragraph.
#[derive(Debug, Clone)]
pub(super) struct Laid {
    pub(super) glyphs: Vec<Glyph>,
    pub(super) looks: Vec<Look>,
    pub(super) lines: Vec<Line>,
    pub(super) height: f32,
    /// The widest line.
    pub(super) width: f32,
}

impl Laid {
    /// The first line's height (tags and list labels are centred on it).
    pub(super) fn first_line(&self) -> f32 {
        self.lines.first().map_or(14.0, |l| l.height)
    }
}

/// Most characters laid out per paragraph (the rest are dropped).
const MAX_CHARS: usize = 200_000;

/// Lay out `p` to wrap at `wrap` points.
pub(super) fn layout(faces: &mut Faces, p: &Paragraph, wrap: f32) -> Laid {
    let base = p.style.def().char;
    let mut glyphs = Vec::new();
    let mut looks: Vec<Look> = Vec::new();
    let look_of = |f: &notecraft_doc::CharFormat| -> (Look, String, bool) {
        let size = f.size.filter(|s| s.is_finite()).unwrap_or(11.0).clamp(1.0, 1638.0);
        let script = f.script.unwrap_or(Script::Normal);
        let (shown, rise) = match script {
            Script::Normal => (size, 0.0),
            Script::Super => (size * 0.65, size * 0.33),
            Script::Sub => (size * 0.65, -size * 0.12),
        };
        let link = f.link.clone().filter(|l| !l.trim().is_empty());
        let look = Look {
            size: shown,
            color: if link.is_some() { LINK } else { f.color.unwrap_or(TEXT) },
            italic: f.italic == Some(true),
            fake_bold: false,
            underline: f.underline == Some(true) || link.is_some(),
            strike: f.strike == Some(true),
            highlight: f.highlight,
            link,
            rise,
        };
        (look, f.font.clone().unwrap_or_default(), f.bold == Some(true))
    };
    let mut n = 0usize;
    for r in &p.runs {
        let eff = r.fmt.over(&base);
        let (mut look, font, bold) = look_of(&eff);
        let (face, fake) = faces.pick(&font, bold);
        look.fake_bold = fake;
        let li = match looks.iter().position(|l| *l == look) {
            Some(i) => i,
            None => {
                looks.push(look.clone());
                looks.len() - 1
            }
        };
        for c in r.text.chars() {
            n += 1;
            if n > MAX_CHARS {
                break;
            }
            let (hard_break, tab) = (c == '\n', c == '\t');
            let shown = if tab || hard_break { ' ' } else { c };
            let (f, gid, adv) = faces.glyph(shown, face);
            glyphs.push(Glyph { c: shown, face: f, gid, adv: adv * look.size, look: li, space: shown == ' ', hard_break, tab });
        }
    }
    // The empty paragraph's line gets its style's height.
    let (empty_look, font, bold) = look_of(&base);
    let empty_face = faces.pick(&font, bold).0;
    let mut lines = Vec::new();
    let mut start = 0;
    let mut x = 0.0f32;
    let mut last_space: Option<usize> = None;
    let mut i = 0;
    let wrap = if wrap.is_finite() { wrap.max(8.0) } else { 8.0 };
    while i < glyphs.len() {
        if glyphs[i].hard_break {
            lines.push(start..i);
            glyphs[i].adv = 0.0;
            start = i + 1;
            x = 0.0;
            last_space = None;
            i += 1;
            continue;
        }
        if glyphs[i].tab {
            let size = looks.get(glyphs[i].look).map_or(11.0, |l| l.size);
            glyphs[i].adv = (TAB - x.rem_euclid(TAB)).max(size * 0.25);
        }
        let adv = glyphs[i].adv;
        if !glyphs[i].space && x + adv > wrap && i > start {
            match last_space.filter(|&b| b > start) {
                Some(b) => {
                    lines.push(start..b);
                    start = b;
                }
                None => {
                    lines.push(start..i);
                    start = i;
                }
            }
            x = glyphs[start..i].iter().map(|g| g.adv).sum();
            last_space = None;
            continue;
        }
        x += adv;
        if glyphs[i].space {
            last_space = Some(i + 1);
        }
        i += 1;
    }
    lines.push(start..glyphs.len());
    let mut out_lines = Vec::with_capacity(lines.len());
    let mut height = 0.0;
    let mut widest: f32 = 0.0;
    for r in lines {
        let gs = &glyphs[r.clone()];
        // Trailing spaces hang past the edge.
        let shown = gs.iter().rposition(|g| !g.space).map_or(0, |k| k + 1);
        let width: f32 = gs[..shown].iter().map(|g| g.adv).sum();
        let (mut asc, mut desc, mut lh): (f32, f32, f32) = (0.0, 0.0, 0.0);
        let mut metric = |face: usize, look: &Look| {
            if let Some(f) = faces.face(face) {
                asc = asc.max(f.ascent * look.size + look.rise.max(0.0));
                desc = desc.max(f.descent * look.size - look.rise.min(0.0));
                lh = lh.max(f.line() * look.size + look.rise.abs());
            }
        };
        if gs.is_empty() {
            metric(empty_face, &empty_look);
        }
        for g in gs {
            if let Some(l) = looks.get(g.look) {
                metric(g.face, l);
            }
        }
        let natural = lh.max(asc + desc);
        let h = if p.line_spacing > 0.0 && p.line_spacing.is_finite() { natural.max(p.line_spacing) } else { natural };
        // Extra line spacing goes above the text.
        out_lines.push(Line { glyphs: r, width, ascent: asc + (h - natural), height: h });
        height += h;
        widest = widest.max(width);
    }
    Laid { glyphs, looks, lines: out_lines, height, width: widest }
}
