//! Design tokens: every colour and size the shell uses comes from here, never hard-coded at the
//! call site (AGENTS.md › Other rules). Only this file and `icons.rs` may write a colour literal
//! (a test enforces it).
//!
//! The values implement the 2024 look (decision D1, `docs/ui-parity.md` §2): a neutral window
//! background, the ribbon and the panes as white cards, NoteCraft's own violet as the accent (not
//! the reference's), and the themes Colorful, White, Dark Gray, Black and "Use system setting".
//! The structure follows WordCraft's theme (MIT/Apache, same team), adapted.

use egui::{Color32, CornerRadius, FontFamily, FontId, Shadow, Stroke, Visuals};

/// Office Theme (File › Options › General; catalog `OPT.General.OfficeTheme`, `UI.Window.DarkMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ThemeKind {
    /// Light neutral chrome with violet accents (the default).
    #[default]
    Colorful,
    /// White chrome.
    White,
    /// Dark grey chrome and a dark page. Settings written before M1.5 call it `Dark`.
    #[serde(alias = "Dark")]
    DarkGray,
    /// Near-black chrome and a black page.
    Black,
    /// Colorful or Dark Gray, following the desktop's light/dark preference.
    System,
}

impl ThemeKind {
    /// Every choice, in the order the Options list shows them.
    pub const ALL: [ThemeKind; 5] = [ThemeKind::Colorful, ThemeKind::DarkGray, ThemeKind::Black, ThemeKind::White, ThemeKind::System];

    /// The label in the Options list.
    pub fn label(self) -> &'static str {
        match self {
            ThemeKind::Colorful => "Colorful",
            ThemeKind::White => "White",
            ThemeKind::DarkGray => "Dark Gray",
            ThemeKind::Black => "Black",
            ThemeKind::System => "Use system setting",
        }
    }

    /// The concrete theme to draw: `System` becomes Dark Gray when the desktop prefers dark and
    /// Colorful otherwise (also when it states no preference).
    pub fn resolve(self, system_dark: Option<bool>) -> ThemeKind {
        match self {
            ThemeKind::System if system_dark == Some(true) => ThemeKind::DarkGray,
            ThemeKind::System => ThemeKind::Colorful,
            other => other,
        }
    }

    /// Whether the (resolved) theme is dark.
    pub fn is_dark(self) -> bool {
        matches!(self, ThemeKind::DarkGray | ThemeKind::Black)
    }
}

/// Sizes in points (`docs/ui-parity.md` §2.2).
pub mod size {
    /// Title bar height.
    pub const TITLE_BAR: f32 = 40.0;
    /// Tab row height.
    pub const TAB_ROW: f32 = 32.0;
    /// Simplified ribbon card content height.
    pub const RIBBON_SIMPLIFIED: f32 = 38.0;
    /// Classic ribbon card content height (70 of controls, 16 of group label, padding).
    pub const RIBBON_CLASSIC: f32 = 92.0;
    /// The ☰ / Search notebooks row under the ribbon.
    pub const NAV_ROW: f32 = 28.0;
    /// A notebook tree row.
    pub const TREE_ROW: f32 = 28.0;
    /// A page column row.
    pub const PAGE_ROW: f32 = 28.0;
    /// Default navigation pane width.
    pub const NAV_WIDTH: f32 = 220.0;
    /// Default page column width.
    pub const PAGE_COLUMN_WIDTH: f32 = 200.0;
    /// Gap between the ribbon card and the window edges.
    pub const CARD_MARGIN: f32 = 6.0;
    /// Horizontal padding inside tree and page rows.
    pub const ROW_PAD: f32 = 8.0;
    /// Indent per tree or subpage level.
    pub const INDENT: f32 = 16.0;
    /// Thickness of the active tab's underline.
    pub const TAB_UNDERLINE: f32 = 3.0;
    /// Height of the Search boxes.
    pub const SEARCH_HEIGHT: f32 = 26.0;
    /// A small (simplified ribbon, menu) icon.
    pub const ICON: f32 = 16.0;
    /// A large (classic ribbon) icon.
    pub const ICON_LARGE: f32 = 32.0;
    /// A small square button.
    pub const BUTTON: f32 = 28.0;
}

/// Corner radii (`docs/ui-parity.md` §2.2).
pub mod radius {
    use egui::CornerRadius;
    /// Ribbon card, pane cards, dialogs, toasts.
    pub const CARD: CornerRadius = CornerRadius::same(8);
    /// Buttons, rows, inputs.
    pub const CONTROL: CornerRadius = CornerRadius::same(4);
    /// Menus and drop-downs.
    pub const MENU: CornerRadius = CornerRadius::same(6);
    /// The Search boxes.
    pub const SEARCH: CornerRadius = CornerRadius::same(6);
}

/// Every colour the shell uses (`docs/ui-parity.md` §2.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tokens {
    /// Whether this is a dark theme.
    pub dark: bool,
    /// Title bar, tab row, the area around the ribbon card, the navigation pane.
    pub window_back: Color32,
    /// The ribbon card (and pane cards).
    pub ribbon_card: Color32,
    /// Hairline around cards.
    pub card_border: Color32,
    /// Card shadow colour (with alpha).
    pub card_shadow: Color32,
    /// The page column.
    pub page_list: Color32,
    /// Hovered tree or page row.
    pub nav_hover: Color32,
    /// Selected tree or page row.
    pub nav_selected: Color32,
    /// Labels.
    pub text: Color32,
    /// Secondary labels (group labels, shortcuts, dates).
    pub text_dim: Color32,
    /// Disabled labels and icons.
    pub text_disabled: Color32,
    /// The line colour of icons.
    pub icon: Color32,
    /// Accent: tab underline, primary buttons, focus.
    pub accent: Color32,
    /// Accent used as text (links such as "+ New section").
    pub accent_text: Color32,
    /// Text and icons drawn on `accent`.
    pub on_accent: Color32,
    /// Input boxes (Search, font, size).
    pub input: Color32,
    /// Input box outline.
    pub input_border: Color32,
    /// Hovered button.
    pub hover: Color32,
    /// Pressed button.
    pub pressed: Color32,
    /// Checked (toggled-on) button.
    pub checked: Color32,
    /// Separators between groups and panes.
    pub border: Color32,
    /// Menus and popups.
    pub menu: Color32,
    /// The page (default page colour).
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
    /// The red margin line of ruled paper.
    pub margin_line: Color32,
    /// Find-on-page match highlight.
    pub find: Color32,
}

impl Tokens {
    /// The tokens for a concrete theme (`System` is drawn as Colorful; resolve it first).
    pub fn for_theme(kind: ThemeKind) -> Tokens {
        match kind {
            ThemeKind::Colorful | ThemeKind::System => Tokens::light(),
            ThemeKind::White => Tokens {
                window_back: Color32::WHITE,
                card_border: Color32::from_rgb(0xD6, 0xD6, 0xD6),
                card_shadow: Color32::from_black_alpha(31),
                page_list: Color32::from_rgb(0xFA, 0xFA, 0xFA),
                nav_hover: Color32::from_rgb(0xF0, 0xF0, 0xF0),
                nav_selected: Color32::from_rgb(0xE6, 0xE6, 0xE6),
                input_border: Color32::from_rgb(0x8A, 0x8A, 0x8A),
                hover: Color32::from_rgb(0xF0, 0xF0, 0xF0),
                pressed: Color32::from_rgb(0xE0, 0xE0, 0xE0),
                checked: Color32::from_rgb(0xEB, 0xE3, 0xF5),
                ..Tokens::light()
            },
            ThemeKind::DarkGray => Tokens::dark_gray(),
            ThemeKind::Black => Tokens {
                window_back: Color32::from_rgb(0x0A, 0x0A, 0x0A),
                ribbon_card: Color32::from_rgb(0x1F, 0x1F, 0x1F),
                card_border: Color32::from_rgb(0x33, 0x33, 0x33),
                card_shadow: Color32::from_black_alpha(153),
                page_list: Color32::from_rgb(0x14, 0x14, 0x14),
                nav_hover: Color32::from_rgb(0x26, 0x26, 0x26),
                nav_selected: Color32::from_rgb(0x33, 0x33, 0x33),
                text: Color32::WHITE,
                accent: Color32::from_rgb(0xC3, 0xA6, 0xF5),
                accent_text: Color32::from_rgb(0xC3, 0xA6, 0xF5),
                on_accent: Color32::BLACK,
                input: Color32::from_rgb(0x14, 0x14, 0x14),
                hover: Color32::from_rgb(0x2E, 0x2E, 0x2E),
                pressed: Color32::from_rgb(0x3B, 0x3B, 0x3B),
                checked: Color32::from_rgb(0x3A, 0x2E, 0x4D),
                border: Color32::from_rgb(0x3A, 0x3A, 0x3A),
                menu: Color32::from_rgb(0x1F, 0x1F, 0x1F),
                page: Color32::BLACK,
                ..Tokens::dark_gray()
            },
        }
    }

    /// Colorful: the light default.
    fn light() -> Tokens {
        let violet = Color32::from_rgb(0x7A, 0x3D, 0xB8);
        Tokens {
            dark: false,
            window_back: Color32::from_rgb(0xF3, 0xF2, 0xF5),
            ribbon_card: Color32::WHITE,
            card_border: Color32::from_rgb(0xE3, 0xE1, 0xE6),
            card_shadow: Color32::from_black_alpha(26),
            page_list: Color32::WHITE,
            nav_hover: Color32::from_rgb(0xEA, 0xE8, 0xEE),
            nav_selected: Color32::from_rgb(0xE3, 0xE0, 0xE8),
            text: Color32::from_rgb(0x24, 0x24, 0x24),
            text_dim: Color32::from_rgb(0x61, 0x61, 0x61),
            text_disabled: Color32::from_rgb(0xA6, 0xA6, 0xA6),
            icon: Color32::from_rgb(0x42, 0x42, 0x42),
            accent: violet,
            accent_text: violet,
            on_accent: Color32::WHITE,
            input: Color32::WHITE,
            input_border: Color32::from_rgb(0x8A, 0x8A, 0x8A),
            hover: Color32::from_rgb(0xEC, 0xEA, 0xF0),
            pressed: Color32::from_rgb(0xE0, 0xDD, 0xE5),
            checked: Color32::from_rgb(0xE6, 0xDD, 0xF3),
            border: Color32::from_rgb(0xE1, 0xDF, 0xE4),
            menu: Color32::WHITE,
            page: Color32::WHITE,
            page_text: Color32::BLACK,
            page_meta: Color32::from_rgb(0x61, 0x61, 0x61),
            container: Color32::from_rgb(0xC8, 0xC6, 0xC4),
            selection: Color32::from_rgba_unmultiplied(0x7A, 0x3D, 0xB8, 60),
            link: Color32::from_rgb(0x05, 0x63, 0xC1),
            rule: Color32::from_rgb(0xB4, 0xCF, 0xE6),
            margin_line: Color32::from_rgb(0xE6, 0x9A, 0x9A),
            find: Color32::from_rgb(0xFF, 0xE5, 0x8F),
        }
    }

    /// Dark Gray.
    fn dark_gray() -> Tokens {
        let lilac = Color32::from_rgb(0xB7, 0x94, 0xF0);
        Tokens {
            dark: true,
            window_back: Color32::from_rgb(0x29, 0x29, 0x29),
            ribbon_card: Color32::from_rgb(0x3B, 0x3B, 0x3B),
            card_border: Color32::from_rgb(0x4A, 0x4A, 0x4A),
            card_shadow: Color32::from_black_alpha(102),
            page_list: Color32::from_rgb(0x33, 0x33, 0x33),
            nav_hover: Color32::from_rgb(0x38, 0x38, 0x38),
            nav_selected: Color32::from_rgb(0x45, 0x45, 0x45),
            text: Color32::from_rgb(0xF5, 0xF5, 0xF5),
            text_dim: Color32::from_rgb(0xC8, 0xC8, 0xC8),
            text_disabled: Color32::from_rgb(0x80, 0x80, 0x80),
            icon: Color32::from_rgb(0xE0, 0xE0, 0xE0),
            accent: lilac,
            accent_text: Color32::from_rgb(0xC9, 0xAE, 0xF5),
            // Dark text on the light dark-theme accent (white on a lilac stays under 3:1).
            on_accent: Color32::from_rgb(0x1A, 0x1A, 0x1A),
            input: Color32::from_rgb(0x2E, 0x2E, 0x2E),
            input_border: Color32::from_rgb(0x8C, 0x8C, 0x8C),
            hover: Color32::from_rgb(0x47, 0x47, 0x47),
            pressed: Color32::from_rgb(0x52, 0x52, 0x52),
            checked: Color32::from_rgb(0x4A, 0x3D, 0x5C),
            border: Color32::from_rgb(0x52, 0x52, 0x52),
            menu: Color32::from_rgb(0x33, 0x33, 0x33),
            page: Color32::from_rgb(0x1F, 0x1F, 0x1F),
            page_text: Color32::from_rgb(0xF3, 0xF2, 0xF1),
            page_meta: Color32::from_rgb(0xA6, 0xA6, 0xA6),
            container: Color32::from_rgb(0x60, 0x5E, 0x5C),
            selection: Color32::from_rgba_unmultiplied(0xB7, 0x94, 0xF0, 70),
            link: Color32::from_rgb(0x6C, 0xB8, 0xF6),
            rule: Color32::from_rgb(0x3A, 0x4D, 0x63),
            margin_line: Color32::from_rgb(0x8A, 0x4A, 0x4A),
            find: Color32::from_rgb(0x8A, 0x6D, 0x0B),
        }
    }

    /// The card's shadow.
    pub fn card_shadow(&self) -> Shadow {
        Shadow { offset: [0, 2], blur: 8, spread: 0, color: self.card_shadow }
    }

    /// egui visuals matching the tokens (menus, popups, text fields).
    pub fn visuals(&self) -> Visuals {
        let t = self;
        let mut v = if t.dark { Visuals::dark() } else { Visuals::light() };
        v.panel_fill = t.window_back;
        v.window_fill = t.menu;
        v.extreme_bg_color = t.input;
        v.faint_bg_color = t.page_list;
        v.window_stroke = Stroke::new(1.0, t.card_border);
        v.window_corner_radius = radius::CARD;
        v.menu_corner_radius = radius::MENU;
        v.selection.bg_fill = t.selection;
        v.selection.stroke = Stroke::new(1.0, t.accent);
        v.hyperlink_color = t.link;
        v.override_text_color = None;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
        v.widgets.inactive.bg_fill = t.input;
        v.widgets.inactive.weak_bg_fill = t.input;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, t.input_border);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, t.text);
        v.widgets.inactive.corner_radius = radius::CONTROL;
        v.widgets.hovered.bg_fill = t.hover;
        v.widgets.hovered.weak_bg_fill = t.hover;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.input_border);
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, t.text);
        v.widgets.hovered.corner_radius = radius::CONTROL;
        v.widgets.active.bg_fill = t.pressed;
        v.widgets.active.weak_bg_fill = t.pressed;
        v.widgets.active.fg_stroke = Stroke::new(1.0, t.text);
        v.widgets.active.corner_radius = radius::CONTROL;
        v.widgets.open.bg_fill = t.pressed;
        v.widgets.open.weak_bg_fill = t.pressed;
        v.widgets.open.corner_radius = radius::CONTROL;
        v.popup_shadow = Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(if t.dark { 120 } else { 40 }) };
        v.window_shadow = Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(if t.dark { 140 } else { 50 }) };
        v
    }
}

/// Apply a theme to egui: visuals, spacing and the text styles of the type ramp. Call when the
/// resolved theme changes (it rewrites the style).
pub fn apply(ctx: &egui::Context, t: &Tokens) {
    ctx.set_visuals(t.visuals());
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(8.0, 3.0);
        s.spacing.interact_size = egui::vec2(24.0, 22.0);
        s.spacing.menu_margin = egui::Margin::same(6);
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(TypeRung::Body.size()));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(TypeRung::Body.size()));
        s.text_styles.insert(egui::TextStyle::Small, FontId::proportional(TypeRung::Caption.size()));
        s.text_styles.insert(egui::TextStyle::Heading, FontId::proportional(TypeRung::Heading.size()));
        s.interaction.tooltip_delay = 0.45;
    });
}

/// A UI font weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    /// Regular.
    Regular,
    /// Medium.
    Medium,
    /// Semibold.
    Semibold,
}

/// A rung of the interface type ramp (`docs/ui-parity.md` §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeRung {
    /// Classic group labels, the date under the title, ScreenTip details.
    Caption,
    /// Buttons, tree and page rows, menus.
    Body,
    /// The tab row.
    Tab,
    /// The window title in the title bar.
    Title,
    /// Pane and dialog titles.
    Heading,
}

impl TypeRung {
    /// Size in points.
    pub fn size(self) -> f32 {
        match self {
            TypeRung::Caption => 11.0,
            TypeRung::Body | TypeRung::Title => 13.0,
            TypeRung::Tab => 13.5,
            TypeRung::Heading => 15.0,
        }
    }

    /// The font for this rung in a weight.
    pub fn font(self, ctx: &egui::Context, weight: Weight) -> FontId {
        font(ctx, self.size(), weight)
    }
}

/// The interface font of `size` in `weight`. Medium and semibold fall back to regular until the
/// fonts are active (egui panics on an unknown family; see `fonts::ready`).
pub fn font(ctx: &egui::Context, size: f32, weight: Weight) -> FontId {
    let name = match weight {
        Weight::Regular => return FontId::proportional(size),
        Weight::Medium => "medium",
        Weight::Semibold => "semibold",
    };
    if crate::fonts::ready(ctx, name) { FontId::new(size, FontFamily::Name(name.into())) } else { FontId::proportional(size) }
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

/// Convenience: a corner radius of `r` points.
pub fn round(r: u8) -> CornerRadius {
    CornerRadius::same(r)
}

#[cfg(test)]
mod tests {
    use super::{ThemeKind, Tokens};
    use egui::Color32;

    /// WCAG contrast ratio between two opaque colours.
    pub(crate) fn contrast(a: Color32, b: Color32) -> f32 {
        let lum = |c: Color32| {
            let ch = |v: u8| {
                let v = f32::from(v) / 255.0;
                if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    /// `docs/ui-parity.md` §2.1 contrast floors, in every theme: text ≥ 7:1 and secondary text ≥
    /// 4.5:1 on every chrome background, the accent ≥ 4.5:1 where it is text, text on the accent
    /// ≥ 4.5:1, and input outlines ≥ 3:1.
    #[test]
    fn every_theme_meets_the_contrast_floors() {
        for kind in ThemeKind::ALL {
            let t = Tokens::for_theme(kind.resolve(None));
            for (name, bg) in
                [("window", t.window_back), ("card", t.ribbon_card), ("pages", t.page_list), ("menu", t.menu), ("selected", t.nav_selected)]
            {
                let r = contrast(t.text, bg);
                assert!(r >= 7.0, "{kind:?}: text on {name} is {r:.2}:1");
                let r = contrast(t.text_dim, bg);
                assert!(r >= 4.5, "{kind:?}: text_dim on {name} is {r:.2}:1");
                let r = contrast(t.accent_text, bg);
                assert!(r >= 4.5, "{kind:?}: accent_text on {name} is {r:.2}:1");
            }
            for (name, bg) in [("window", t.window_back), ("card", t.ribbon_card)] {
                let r = contrast(t.accent, bg);
                assert!(r >= 4.5, "{kind:?}: accent on {name} is {r:.2}:1");
                let r = contrast(t.input_border, bg);
                assert!(r >= 3.0, "{kind:?}: input_border on {name} is {r:.2}:1");
            }
            let r = contrast(t.on_accent, t.accent);
            assert!(r >= 4.5, "{kind:?}: on_accent on accent is {r:.2}:1");
            let r = contrast(t.page_text, t.page);
            assert!(r >= 7.0, "{kind:?}: page text is {r:.2}:1");
        }
    }

    /// `System` follows the desktop: dark → Dark Gray, light or no answer → Colorful. Settings
    /// written before M1.5 (`"Dark"`) still load.
    #[test]
    fn the_system_theme_resolves_and_old_settings_load() {
        assert_eq!(ThemeKind::System.resolve(Some(true)), ThemeKind::DarkGray);
        assert_eq!(ThemeKind::System.resolve(Some(false)), ThemeKind::Colorful);
        assert_eq!(ThemeKind::System.resolve(None), ThemeKind::Colorful);
        assert_eq!(ThemeKind::Black.resolve(Some(false)), ThemeKind::Black);
        let old: ThemeKind = serde_json::from_str("\"Dark\"").unwrap();
        assert_eq!(old, ThemeKind::DarkGray);
        assert!(ThemeKind::DarkGray.is_dark() && ThemeKind::Black.is_dark() && !ThemeKind::White.is_dark());
    }

    /// AGENTS.md › colours come from `theme::Tokens`: no colour literal in the shell outside this
    /// file and `icons.rs`.
    #[test]
    fn colours_come_from_the_theme() {
        let banned = ["Color32::from_rgb(", "Color32::from_rgba_unmultiplied(", "Color32::from_gray(", "Color32::from_rgba_premultiplied("];
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if !name.ends_with(".rs") || name == "theme.rs" || name == "icons.rs" || name == "tests.rs" {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            for b in banned {
                assert!(!src.contains(b), "{name} writes a colour literal ({b}); use theme::Tokens");
            }
        }
    }
}
