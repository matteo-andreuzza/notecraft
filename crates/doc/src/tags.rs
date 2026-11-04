//! Note tags: the built-in tag set, custom tags, and tags applied to paragraphs.
//!
//! A [`TagDef`] says what a tag looks like (name, symbol, font/highlight colour) and whether it
//! is a to-do tag (has a checkbox). A paragraph carries [`TagUse`]s: which definition, whether
//! it is checked, plus a snapshot of the name and symbol so a paragraph tagged with a custom tag
//! still renders and searches correctly on a machine that lacks that custom definition — the
//! reference format does the same by storing tag definitions alongside the content
//! ([MS-ONE] `jcidNoteTagSharedDefinitionContainer`).
//!
//! The built-in list, its order and the Ctrl+1…9 assignments follow the reference's Tags
//! gallery and keyboard table (catalog `R.Home.Tags.*`, `KB.Tag.*`); the symbols are NoteCraft's
//! own vector drawings (`ui-egui`), named here by meaning only.

use notecraft_color::Rgba;
use serde::{Deserialize, Serialize};

/// What a tag's icon depicts. The UI draws each kind with its own original vector icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TagSymbol {
    /// No icon (highlight-only tags such as Remember for later).
    None,
    /// An empty checkbox that can be ticked.
    Checkbox,
    /// Checkbox with a "1".
    CheckboxOne,
    /// Checkbox with a "2".
    CheckboxTwo,
    /// Checkbox with a person.
    CheckboxPerson,
    /// Checkbox with a calendar.
    CheckboxCalendar,
    /// Checkbox with a phone.
    CheckboxPhone,
    /// Checkbox with a flag.
    CheckboxFlag,
    /// A star.
    Star,
    /// A question mark.
    Question,
    /// A person.
    Person,
    /// A house.
    House,
    /// A phone.
    Phone,
    /// A globe.
    Globe,
    /// A light bulb.
    Bulb,
    /// A key.
    Key,
    /// An exclamation mark.
    Exclamation,
    /// A flag (first colour).
    FlagA,
    /// A flag (second colour).
    FlagB,
    /// A film strip.
    Film,
    /// A book.
    Book,
    /// A music note.
    Music,
    /// A document.
    Document,
    /// A pen.
    Pen,
    /// An envelope.
    Envelope,
}

impl TagSymbol {
    /// Whether this symbol is a checkbox (to-do style).
    pub fn is_checkbox(self) -> bool {
        matches!(
            self,
            TagSymbol::Checkbox
                | TagSymbol::CheckboxOne
                | TagSymbol::CheckboxTwo
                | TagSymbol::CheckboxPerson
                | TagSymbol::CheckboxCalendar
                | TagSymbol::CheckboxPhone
                | TagSymbol::CheckboxFlag
        )
    }
}

/// A tag definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagDef {
    /// Stable key: built-ins use readable keys (`todo`, `important`…), custom tags `custom-<n>`.
    pub key: String,
    /// Display name.
    pub name: String,
    /// Icon.
    pub symbol: TagSymbol,
    /// Font colour applied to the tagged paragraph, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_color: Option<Rgba>,
    /// Highlight colour applied to the tagged paragraph, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<Rgba>,
}

impl TagDef {
    /// Whether tagged paragraphs show a checkbox.
    pub fn is_todo(&self) -> bool {
        self.symbol.is_checkbox()
    }
}

/// A tag applied to a paragraph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagUse {
    /// The definition's key.
    pub key: String,
    /// Name snapshot (for rendering/searching without the definition).
    pub name: String,
    /// Symbol snapshot.
    pub symbol: TagSymbol,
    /// Checked state (to-do tags).
    #[serde(default)]
    pub checked: bool,
}

impl TagUse {
    /// A use of `def`, unchecked.
    pub fn of(def: &TagDef) -> TagUse {
        TagUse { key: def.key.clone(), name: def.name.clone(), symbol: def.symbol, checked: false }
    }
}

const YELLOW: Rgba = Rgba::rgb(0xff, 0xff, 0x00);
const GREEN: Rgba = Rgba::rgb(0x00, 0xff, 0x00);
const BLUE_TEXT: Rgba = Rgba::rgb(0x00, 0x5a, 0xc8);
const RED_TEXT: Rgba = Rgba::rgb(0xc0, 0x00, 0x00);

/// The built-in tags in gallery order: `(key, name, symbol, font colour, highlight)`. The first
/// nine are on Ctrl+1…Ctrl+9 (catalog `KB.Tag.*`).
pub fn builtin_tags() -> Vec<TagDef> {
    use TagSymbol as S;
    let t = |key: &str, name: &str, symbol: TagSymbol, font_color: Option<Rgba>, highlight: Option<Rgba>| TagDef {
        key: key.to_string(),
        name: name.to_string(),
        symbol,
        font_color,
        highlight,
    };
    vec![
        t("todo", "To Do", S::Checkbox, None, None),
        t("important", "Important", S::Star, None, None),
        t("question", "Question", S::Question, None, None),
        t("remember-for-later", "Remember for later", S::None, None, Some(YELLOW)),
        t("definition", "Definition", S::None, None, Some(GREEN)),
        t("highlight", "Highlight", S::None, None, Some(YELLOW)),
        t("contact", "Contact", S::Person, None, None),
        t("address", "Address", S::House, None, None),
        t("phone-number", "Phone number", S::Phone, None, None),
        t("web-site-to-visit", "Web site to visit", S::Globe, None, None),
        t("idea", "Idea", S::Bulb, None, None),
        t("password", "Password", S::Key, None, None),
        t("critical", "Critical", S::Exclamation, Some(RED_TEXT), None),
        t("project-a", "Project A", S::FlagA, None, None),
        t("project-b", "Project B", S::FlagB, None, None),
        t("movie-to-see", "Movie to see", S::Film, None, None),
        t("book-to-read", "Book to read", S::Book, None, None),
        t("music-to-listen-to", "Music to listen to", S::Music, None, None),
        t("source-for-article", "Source for article", S::Document, None, None),
        t("remember-for-blog", "Remember for blog", S::Pen, None, None),
        t("discuss-with-person-a", "Discuss with <Person A>", S::CheckboxPerson, None, None),
        t("discuss-with-person-b", "Discuss with <Person B>", S::CheckboxPerson, None, None),
        t("discuss-with-manager", "Discuss with manager", S::CheckboxPerson, Some(BLUE_TEXT), None),
        t("send-in-email", "Send in email", S::Envelope, None, None),
        t("schedule-meeting", "Schedule meeting", S::CheckboxCalendar, None, None),
        t("call-back", "Call back", S::CheckboxPhone, None, None),
        t("todo-priority-1", "To Do priority 1", S::CheckboxOne, None, None),
        t("todo-priority-2", "To Do priority 2", S::CheckboxTwo, None, None),
        t("client-request", "Client request", S::CheckboxFlag, None, None),
    ]
}

/// The key of the built-in tag on Ctrl+`n` (1–9).
pub fn shortcut_tag(n: u8) -> Option<&'static str> {
    const KEYS: [&str; 9] = ["todo", "important", "question", "remember-for-later", "definition", "highlight", "contact", "address", "phone-number"];
    KEYS.get((n as usize).wrapping_sub(1)).copied()
}

/// Maximum custom tags a user can define (hostile settings files are capped here).
pub const MAX_CUSTOM_TAGS: usize = 256;

/// Maximum tags on one object (hostile files are capped here).
pub const MAX_OBJECT_TAGS: usize = 32;

/// Keep the first use of each tag key, at most [`MAX_OBJECT_TAGS`] of them.
pub fn clean_uses(tags: &mut Vec<TagUse>) {
    let mut seen: Vec<String> = Vec::new();
    tags.retain(|t| {
        if seen.len() >= MAX_OBJECT_TAGS || seen.contains(&t.key) {
            return false;
        }
        seen.push(t.key.clone());
        true
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_list_matches_reference_structure() {
        // parity: R.Home.Tags.ToDo, R.Home.Tags.Important, R.Home.Tags.Question
        let tags = builtin_tags();
        assert_eq!(tags.len(), 29);
        assert_eq!(tags[0].name, "To Do");
        assert!(tags[0].is_todo());
        assert!(!tags[1].is_todo());
        for n in 1..=9 {
            let key = shortcut_tag(n).unwrap();
            assert_eq!(tags[(n - 1) as usize].key, key, "Ctrl+{n} is the {n}th gallery tag");
        }
        assert_eq!(shortcut_tag(0), None);
        assert_eq!(shortcut_tag(10), None);
        let mut keys: Vec<_> = tags.iter().map(|t| t.key.as_str()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 29, "keys are unique");
    }
}
