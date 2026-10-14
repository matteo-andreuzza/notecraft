//! Design tokens: every colour and size the shell uses comes from here, never hard-coded at the
//! call site (AGENTS.md › Other rules).
//!
//! The layout follows the reference's "Colorful" Office theme (coloured title bar and tab strip,
//! light ribbon body, white page, light navigation panes) with NoteCraft's own violet as the
//! accent; the dark theme follows the reference's dark theme structure.

use egui::{Color32, CornerRadius, Stroke, Visuals};

/// Theme choice (File › Options › General › Office Theme; catalog `OPT.General.OfficeTheme`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ThemeKind {
    /// Coloured title bar and tab strip.
    #[default]
    Colorful,
    /// Dark chrome, dark panes.
    Dark,
    /// Light grey chrome.
    White,
}

/// The token set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tokens {
    /// Title bar and tab strip background.
    pub chrome: Color32,
    /// Text on the chrome.
    pub chrome_text: Color32,
    /// The selected ribbon tab's background (joins the ribbon body).
    pub tab_selected: Color32,
    /// Hovered tab on the strip.
    pub tab_hover: Color32,
    /// Ribbon body.
    pub ribbon: Color32,
    /// Ribbon text.
    pub ribbon_text: Color32,
    /// Disabled control text/icons.
    pub disabled: Color32,
    /// Group separators and pane borders.
    pub border: Color32,
    /// Button hover fill.
    pub hover: Color32,
    /// Button pressed/checked fill.
    pub checked: Color32,
    /// Accent (focus rings, selected page, links of the chrome).
    pub accent: Color32,
    /// Navigation panes background.
    pub pane: Color32,
    /// Text in panes.
    pub pane_text: Color32,
    /// Selected row in panes.
    pub pane_selected: Color32,
    /// The area around the page.
    pub canvas_back: Color32,
    /// The page.
    pub page: Color32,
    /// Default page text.
    pub page_text: Color32,
    /// Date/time under the title.
    pub page_meta: Color32,
    /// Note container frame (on hover/focus).
    pub container: Color32,
    /// Text selection fill.
    pub selection: Color32,
    /// Hyperlinks on the page.
    pub link: Color32,
    /// Default rule line colour.
    pub rule: Color32,
    /// Find-on-page match highlight.
    pub find: Color32,
    /// Corner radius of buttons.
    pub radius: CornerRadius,
}

impl Tokens {
    /// The tokens for a theme.
    pub fn for_theme(kind: ThemeKind) -> Tokens {
        let violet = Color32::from_rgb(0x6e, 0x3f, 0xb0);
        match kind {
            ThemeKind::Colorful | ThemeKind::White => {
                let (chrome, chrome_text, tab_hover) = if kind == ThemeKind::Colorful {
                    (Color32::from_rgb(0x5e, 0x2f, 0x96), Color32::WHITE, Color32::from_rgb(0x74, 0x47, 0xab))
                } else {
                    (Color32::from_rgb(0xf3, 0xf2, 0xf1), Color32::from_rgb(0x25, 0x24, 0x23), Color32::from_rgb(0xe1, 0xdf, 0xdd))
                };
                Tokens {
                    chrome,
                    chrome_text,
                    tab_selected: Color32::from_rgb(0xf3, 0xf2, 0xf1),
                    tab_hover,
                    ribbon: Color32::from_rgb(0xf3, 0xf2, 0xf1),
                    ribbon_text: Color32::from_rgb(0x25, 0x24, 0x23),
                    disabled: Color32::from_rgb(0xa1, 0x9f, 0x9d),
                    border: Color32::from_rgb(0xd2, 0xd0, 0xce),
                    hover: Color32::from_rgb(0xe1, 0xdf, 0xdd),
                    checked: Color32::from_rgb(0xc8, 0xc6, 0xc4),
                    accent: violet,
                    pane: Color32::from_rgb(0xfa, 0xf9, 0xf8),
                    pane_text: Color32::from_rgb(0x25, 0x24, 0x23),
                    pane_selected: Color32::from_rgb(0xe8, 0xdf, 0xf5),
                    canvas_back: Color32::from_rgb(0xff, 0xff, 0xff),
                    page: Color32::WHITE,
                    page_text: Color32::BLACK,
                    page_meta: Color32::from_rgb(0x60, 0x5e, 0x5c),
                    container: Color32::from_rgb(0xc8, 0xc6, 0xc4),
                    selection: Color32::from_rgba_unmultiplied(0x6e, 0x3f, 0xb0, 60),
                    link: Color32::from_rgb(0x05, 0x63, 0xc1),
                    rule: Color32::from_rgb(0xb4, 0xcf, 0xe6),
                    find: Color32::from_rgb(0xff, 0xe5, 0x8f),
                    radius: CornerRadius::same(2),
                }
            }
            ThemeKind::Dark => Tokens {
                chrome: Color32::from_rgb(0x1f, 0x1f, 0x1f),
                chrome_text: Color32::from_rgb(0xf3, 0xf2, 0xf1),
                tab_selected: Color32::from_rgb(0x2d, 0x2c, 0x2b),
                tab_hover: Color32::from_rgb(0x3b, 0x3a, 0x39),
                ribbon: Color32::from_rgb(0x2d, 0x2c, 0x2b),
                ribbon_text: Color32::from_rgb(0xf3, 0xf2, 0xf1),
                disabled: Color32::from_rgb(0x79, 0x77, 0x75),
                border: Color32::from_rgb(0x48, 0x46, 0x44),
                hover: Color32::from_rgb(0x3b, 0x3a, 0x39),
                checked: Color32::from_rgb(0x55, 0x52, 0x50),
                accent: Color32::from_rgb(0xb3, 0x8c, 0xf0),
                pane: Color32::from_rgb(0x25, 0x24, 0x23),
                pane_text: Color32::from_rgb(0xf3, 0xf2, 0xf1),
                pane_selected: Color32::from_rgb(0x44, 0x34, 0x5e),
                canvas_back: Color32::from_rgb(0x1b, 0x1a, 0x19),
                page: Color32::from_rgb(0x1b, 0x1a, 0x19),
                page_text: Color32::from_rgb(0xf3, 0xf2, 0xf1),
                page_meta: Color32::from_rgb(0xa1, 0x9f, 0x9d),
                container: Color32::from_rgb(0x60, 0x5e, 0x5c),
                selection: Color32::from_rgba_unmultiplied(0xb3, 0x8c, 0xf0, 70),
                link: Color32::from_rgb(0x6c, 0xb8, 0xf6),
                rule: Color32::from_rgb(0x3a, 0x4d, 0x63),
                find: Color32::from_rgb(0x8a, 0x6d, 0x0b),
                radius: CornerRadius::same(2),
            },
        }
    }

    /// egui visuals matching the tokens (menus, popups, text fields).
    pub fn visuals(&self, kind: ThemeKind) -> Visuals {
        let mut v = if kind == ThemeKind::Dark { Visuals::dark() } else { Visuals::light() };
        v.panel_fill = self.ribbon;
        v.window_fill = self.ribbon;
        v.extreme_bg_color = if kind == ThemeKind::Dark { Color32::from_rgb(0x1f, 0x1f, 0x1f) } else { Color32::WHITE };
        v.selection.bg_fill = self.selection;
        v.selection.stroke = Stroke::new(1.0, self.accent);
        v.hyperlink_color = self.link;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.border);
        v.widgets.hovered.weak_bg_fill = self.hover;
        v.widgets.hovered.bg_fill = self.hover;
        v.widgets.active.weak_bg_fill = self.checked;
        v.widgets.active.bg_fill = self.checked;
        v.window_corner_radius = CornerRadius::same(4);
        v.menu_corner_radius = CornerRadius::same(3);
        v
    }
}

/// The page-text colour to use over a page colour (black, or white on dark pages).
pub fn text_on(bg: Color32) -> Color32 {
    let l = 0.2126 * bg.r() as f32 + 0.7152 * bg.g() as f32 + 0.0722 * bg.b() as f32;
    if l > 128.0 { Color32::BLACK } else { Color32::WHITE }
}

/// Convert a model colour to egui.
pub fn c32(c: notecraft_engine::doc::color::Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}
