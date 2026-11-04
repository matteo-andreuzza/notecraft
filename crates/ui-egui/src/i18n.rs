//! The display language (catalog `OPT.Language.DisplayLanguage`): English or Italian.
//!
//! Labels are written in English everywhere (the catalog, the shell); [`tr`] gives the
//! Italian label when Italian is chosen and one is known, else the English one. The
//! translations are NoteCraft's own (`i18n_it.rs`). The language is a per-thread setting
//! the app sets every frame from [`crate::UiState::lang`] on the UI thread, so drawing code
//! needs no extra parameter (and tests running side by side don't see each other's).

use std::cell::Cell;

/// A display language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Lang {
    /// English.
    #[default]
    En,
    /// Italiano.
    It,
}

impl Lang {
    /// Every language, in the order the Options list them.
    pub const ALL: [Lang; 2] = [Lang::En, Lang::It];

    /// The language's own name.
    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::It => "Italiano",
        }
    }

    /// The language a system locale (`LANG`, e.g. `it_IT.UTF-8`) asks for.
    pub fn from_locale(locale: &str) -> Lang {
        if locale.to_ascii_lowercase().starts_with("it") { Lang::It } else { Lang::En }
    }
}

thread_local! {
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::En) };
}

/// Make `lang` the language [`tr`] answers in (on this thread).
pub fn set(lang: Lang) {
    CURRENT.with(|c| c.set(lang));
}

/// The current display language.
pub fn current() -> Lang {
    CURRENT.with(Cell::get)
}

/// `label` in the current language (the English text when no translation is known).
pub fn tr(label: &str) -> String {
    if current() == Lang::En {
        return label.to_string();
    }
    let table = crate::i18n_it::IT;
    if let Ok(i) = table.binary_search_by(|(k, _)| (*k).cmp(label)) {
        return table.get(i).map_or(label, |(_, v)| *v).to_string();
    }
    for (en, it) in crate::i18n_it::IT_PREFIXES {
        if let Some(rest) = label.strip_prefix(en)
            && rest.chars().all(|c| c.is_ascii_digit())
            && !rest.is_empty()
        {
            return format!("{it}{rest}");
        }
    }
    label.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_translate_when_italian_is_chosen() {
        // parity: OPT.Language.DisplayLanguage
        let table = crate::i18n_it::IT;
        assert!(table.windows(2).all(|w| w[0].0 < w[1].0), "the table is sorted and has no duplicates");
        set(Lang::It);
        assert_eq!(tr("Bold"), "Grassetto");
        assert_eq!(tr("Section Color 12"), "Colore sezione 12");
        assert_eq!(tr("No such label"), "No such label");
        set(Lang::En);
        assert_eq!(tr("Bold"), "Bold");
        assert_eq!(Lang::from_locale("it_IT.UTF-8"), Lang::It);
        assert_eq!(Lang::from_locale("C"), Lang::En);
    }
}
