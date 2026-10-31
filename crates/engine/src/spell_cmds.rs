//! Spelling (Review › Spelling, F7; catalog `R.Review.Proofing.SpellCheck`, `KB.General.Spelling`,
//! `UI.Panes.Spelling`, `R.Review.Language.SetLanguage`, `OPT.Proofing.*`).
//!
//! Dictionaries are Hunspell files installed on the system (`notecraft_spell`); none ships
//! with NoteCraft. The first check loads the dictionary of the proofing language from the
//! usual folders (native only); when none is installed spelling says so and nothing is
//! underlined. Words added to the dictionary are kept in the preferences; Ignore All lasts
//! for the session.

use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;

use notecraft_spell::Dictionary;
use serde_json::{Value, json};

use crate::selection::{Block, Pos};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// Most words added to the user's dictionary.
pub const MAX_CUSTOM_WORDS: usize = 10_000;

/// The loaded dictionary and the session's ignored words.
#[derive(Default)]
pub struct Speller {
    dict: Option<Arc<Dictionary>>,
    /// The language the dictionary is for (or that failed to load).
    lang: String,
    /// A load was tried for `lang` (so a missing dictionary isn't looked for every frame).
    tried: bool,
    /// Ignore All.
    ignored: HashSet<String>,
    /// Bumped whenever the answers may change (for the UI's cache).
    generation: u64,
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // R.Review.Proofing.SpellCheck — F7: the next misspelled word from the caret.
        CommandSpec::new("review.spelling", "R.Review.Proofing.SpellCheck", "Spelling", "Review/Proofing/Spelling", next_misspelled)
            .key("F7")
            .pure()
            .when(need_page),
        CommandSpec::new(
            "review.spellingSuggestion",
            "CM.MisspelledWord.NoSpellingSuggestions",
            "Spelling suggestions",
            "Context/Misspelled Word/Spelling suggestions",
            change_word,
        )
        .params(r#"{"word"?: replacement (default: the first suggestion)}"#)
        .when(|s| if s.view.spelling.as_ref().is_some_and(|x| !x.suggestions.is_empty()) { None } else { Some("Check spelling first (F7)") }),
        CommandSpec::new("review.ignoreAll", "UI.Panes.Spelling.IgnoreAll", "Ignore All", "Window/Panes/Spelling/Ignore All", |s, v| {
            let w = word_param(s, v)?;
            s.speller.ignored.insert(w.clone());
            s.speller.generation += 1;
            Ok(json!({"ignored": w}))
        })
        .params(r#"{"word"?: string (default: the selected word)}"#)
        .pure(),
        CommandSpec::new(
            "review.addToDictionary",
            "UI.Panes.Spelling.AddToDictionary",
            "Add to Dictionary",
            "Window/Panes/Spelling/Add to Dictionary",
            |s, v| {
                let w = word_param(s, v)?;
                if !s.prefs.custom_words.contains(&w) && s.prefs.custom_words.len() < MAX_CUSTOM_WORDS {
                    s.prefs.custom_words.push(w.clone());
                }
                Ok(json!({"added": w}))
            },
        )
        .params(r#"{"word"?: string (default: the selected word)}"#)
        .pure(),
        // R.Review.Language.SetLanguage — the proofing language (a dictionary name, `en_US`).
        CommandSpec::new(
            "review.setLanguage",
            "R.Review.Language.SetLanguage",
            "Set Proofing Language",
            "Review/Language/Set Proofing Language",
            set_language,
        )
        .params(r#"{"lang"?: dictionary name; without it, the installed dictionaries are listed}"#)
        .pure(),
    ]
}

fn need_page(s: &Session) -> Option<&'static str> {
    if s.page().is_some() { None } else { Some("Open a page first") }
}

fn word_param(s: &Session, v: &Value) -> Result<String, CmdError> {
    let w = match p::str(v, "word") {
        Some(w) => w.to_string(),
        None => s.selected_text().unwrap_or_default(),
    };
    let w = w.trim().to_string();
    if w.is_empty() || w.chars().count() > notecraft_spell::MAX_WORD || w.contains(char::is_whitespace) {
        return Err(CmdError::Params("give one word (`word`, or select it)".into()));
    }
    Ok(w)
}

/// The folders dictionaries are looked for in.
#[cfg(not(target_arch = "wasm32"))]
fn dictionary_dirs() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Some(d) = std::env::var_os("XDG_DATA_HOME") {
        out.push(std::path::PathBuf::from(d).join("hunspell"));
    }
    if let Some(h) = std::env::var_os("HOME") {
        out.push(std::path::PathBuf::from(h).join(".local/share/hunspell"));
    }
    for d in ["/usr/share/hunspell", "/usr/share/myspell/dicts", "/usr/share/myspell", "/usr/local/share/hunspell"] {
        out.push(d.into());
    }
    out
}

/// The installed dictionaries' names (`en_US`, `it_IT`…).
#[cfg(not(target_arch = "wasm32"))]
pub fn installed_languages() -> Vec<String> {
    let mut out: Vec<String> = dictionary_dirs()
        .into_iter()
        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten())
        .filter_map(|e| {
            let p = e.path();
            (p.extension().is_some_and(|x| x == "dic") && p.with_extension("aff").is_file())
                .then(|| p.file_stem()?.to_str().map(str::to_string))
                .flatten()
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(target_arch = "wasm32")]
pub fn installed_languages() -> Vec<String> {
    Vec::new()
}

/// Largest dictionary file read.
#[cfg(not(target_arch = "wasm32"))]
const MAX_DICTIONARY_BYTES: u64 = 64 << 20;

#[cfg(not(target_arch = "wasm32"))]
fn load(lang: &str) -> Option<Dictionary> {
    for d in dictionary_dirs() {
        let (aff, dic) = (d.join(format!("{lang}.aff")), d.join(format!("{lang}.dic")));
        let small = |p: &std::path::Path| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() <= MAX_DICTIONARY_BYTES);
        if small(&aff) && small(&dic) {
            let (Ok(a), Ok(b)) = (std::fs::read(&aff), std::fs::read(&dic)) else { continue };
            if let Ok(dict) = Dictionary::load(&a, &b) {
                return Some(dict);
            }
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn load(_: &str) -> Option<Dictionary> {
    None
}

impl Session {
    /// Use `dict` for spelling (tests, the web build, hosts with their own dictionaries).
    pub fn set_dictionary(&mut self, lang: &str, dict: Dictionary) {
        let generation = self.speller.generation + 1;
        let ignored = std::mem::take(&mut self.speller.ignored);
        self.speller = Speller { dict: Some(Arc::new(dict)), lang: lang.to_string(), tried: true, ignored, generation };
    }

    /// Changes whenever a word's spelling answer may change (Ignore All, Add to Dictionary, a
    /// new dictionary); with the proofing options it keys the UI's cache.
    pub fn spelling_generation(&self) -> u64 {
        self.speller.generation
    }

    /// The proofing dictionary, loaded on first use.
    fn dictionary(&mut self) -> Option<Arc<Dictionary>> {
        let lang = self.prefs.proofing_language.clone();
        if self.speller.lang != lang || !self.speller.tried {
            self.speller.lang = lang.clone();
            self.speller.tried = true;
            self.speller.dict = load(&lang).map(Arc::new);
        }
        self.speller.dict.clone()
    }

    /// Whether `word` is spelled right for this session (dictionary, user words, Ignore All,
    /// and the Proofing options).
    fn word_ok(&self, dict: &Dictionary, word: &str) -> bool {
        if self.prefs.ignore_uppercase && word.chars().filter(|c| c.is_alphabetic()).all(char::is_uppercase) {
            return true;
        }
        if self.prefs.ignore_numbers && word.chars().any(|c| c.is_ascii_digit()) {
            return true;
        }
        self.speller.ignored.contains(word)
            || self.prefs.custom_words.iter().any(|w| w == word || w.to_lowercase() == word.to_lowercase())
            || dict.check(word)
    }

    /// The byte ranges of misspelled words in `text`, for the squiggles of Check spelling as
    /// you type (`OPT.Proofing.CheckAsYouType`). Empty when spelling is off or hidden, or no
    /// dictionary is installed.
    pub fn misspelled(&mut self, text: &str) -> Vec<Range<usize>> {
        if !self.prefs.check_as_you_type || self.prefs.hide_spelling_errors {
            return Vec::new();
        }
        self.misspelled_in(text)
    }

    fn misspelled_in(&mut self, text: &str) -> Vec<Range<usize>> {
        let Some(dict) = self.dictionary() else { return Vec::new() };
        // OPT.Proofing.IgnoreUrls: words inside web and mail addresses.
        let mut skip: Vec<Range<usize>> = Vec::new();
        if self.prefs.ignore_urls {
            let mut at = 0;
            for tok in text.split_whitespace() {
                let start = text.get(at..).and_then(|t| t.find(tok)).map_or(at, |i| at + i);
                if tok.contains("://") || tok.starts_with("www.") || tok.contains('@') {
                    skip.push(start..start + tok.len());
                }
                at = start + tok.len();
            }
        }
        notecraft_spell::words(text)
            .into_iter()
            .filter(|(r, w)| !skip.iter().any(|s| s.start <= r.start && r.end <= s.end) && !self.word_ok(&dict, w))
            .map(|(r, _)| r)
            .collect()
    }

    /// The selected text, when the selection is in one paragraph.
    pub(crate) fn selected_text(&self) -> Option<String> {
        let r = self.text_range()?;
        let (a, b) = r.ordered();
        if a.block != b.block || a.para != b.para {
            return None;
        }
        let page = self.page()?;
        let text = Session::block_paras(page, a.block)?.get(a.para)?.text();
        text.get(a.offset..b.offset).map(str::to_string)
    }
}

/// Every text paragraph of the page in reading order: the title, then containers and table
/// cells in object order.
fn paragraphs(s: &Session) -> Vec<(Block, usize, String)> {
    let Some(page) = s.page() else { return Vec::new() };
    let mut out = vec![(Block::Title, 0, page.title.text())];
    for o in &page.objects {
        match o {
            notecraft_doc::PageObject::Outline(ol) => {
                out.extend(ol.paragraphs.iter().enumerate().filter(|(_, p)| p.math.is_none()).map(|(i, p)| (Block::Outline(ol.id), i, p.text())));
            }
            notecraft_doc::PageObject::Table(t) => {
                for c in t.rows.iter().flat_map(|r| r.cells.iter()) {
                    out.extend(c.paragraphs.iter().enumerate().map(|(i, p)| (Block::Cell(c.id), i, p.text())));
                }
            }
            _ => {}
        }
    }
    out
}

/// F7: select the next misspelled word after the caret (wrapping to the top of the page) and
/// open the Spelling pane with its suggestions.
fn next_misspelled(s: &mut Session, _: &Value) -> CmdResult {
    let Some(dict) = s.dictionary() else {
        let lang = s.prefs.proofing_language.clone();
        return Err(CmdError::Failed(format!(
            "No {lang} dictionary is installed. Install a Hunspell dictionary (for example the hunspell-en_us package) or choose another language."
        )));
    };
    s.view.spelling_pane = true;
    let paras = paragraphs(s);
    let caret = s.text_range().map(|r| r.ordered().1);
    let start = caret.and_then(|c| paras.iter().position(|(b, i, _)| *b == c.block && *i == c.para)).unwrap_or(0);
    let order: Vec<usize> = (start..paras.len()).chain(0..=start.min(paras.len().saturating_sub(1))).collect();
    for (k, &i) in order.iter().enumerate() {
        let Some((block, para, text)) = paras.get(i).cloned() else { continue };
        let from = if k == 0 { caret.map_or(0, |c| c.offset) } else { 0 };
        let upto = if k + 1 == order.len() && k > 0 { caret.map_or(text.len(), |c| c.offset) } else { text.len() };
        let bad = s.misspelled_in(&text);
        if let Some(r) = bad.into_iter().find(|r| r.start >= from && r.end <= upto.max(from)) {
            let word = text.get(r.clone()).unwrap_or("").to_string();
            s.set_range(Pos::new(block, para, r.start), Pos::new(block, para, r.end));
            s.scroll_request = Some(crate::ScrollRequest::Caret);
            let suggestions = dict.suggest(&word);
            s.view.spelling = Some(crate::session::SpellingState { word: word.clone(), suggestions: suggestions.clone() });
            return Ok(json!({"word": word, "suggestions": suggestions, "block": block_id(block), "para": para, "start": r.start, "end": r.end}));
        }
    }
    s.view.spelling = None;
    s.notice("The spelling check is complete.".to_string());
    Ok(json!({"done": true}))
}

fn block_id(b: Block) -> Value {
    match b {
        Block::Title => json!("title"),
        Block::Outline(id) | Block::Cell(id) => json!(id.to_string()),
    }
}

/// Replace the selected (misspelled) word with `word`.
fn change_word(s: &mut Session, v: &Value) -> CmdResult {
    let first = s.view.spelling.as_ref().and_then(|x| x.suggestions.first().cloned());
    let new = match p::str(v, "word") {
        Some(w) => w.to_string(),
        None => first.ok_or_else(|| CmdError::Params("no suggestion to use".into()))?,
    };
    let new = new.as_str();
    if new.is_empty() || new.chars().count() > notecraft_spell::MAX_WORD * 2 {
        return Err(CmdError::Params("`word` must be a short replacement".into()));
    }
    if s.selected_text().filter(|t| !t.is_empty()).is_none() {
        return Err(CmdError::Disabled("select the misspelled word first".into()));
    }
    // Typing over the selection replaces it (the word keeps its formatting).
    let caret = crate::ops::ensure_caret(s)?;
    let fmt = crate::ops::typing_fmt(s, caret);
    let end = crate::ops::insert_at(s, caret, new, &fmt)?;
    s.set_caret(end);
    s.view.spelling = None;
    Ok(json!({"changed": new}))
}

fn set_language(s: &mut Session, v: &Value) -> CmdResult {
    match p::str(v, "lang") {
        Some(l) if !l.is_empty() && l.len() <= 32 && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') => {
            s.prefs.proofing_language = l.to_string();
            s.speller.tried = false;
            s.speller.generation += 1;
            Ok(json!({"lang": l, "installed": s.dictionary().is_some()}))
        }
        Some(_) => Err(CmdError::Params("a language is a dictionary name such as en_US".into())),
        None => Ok(json!({"lang": s.prefs.proofing_language, "installed": installed_languages()})),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::tests::{blank_page, run, sample};

    fn dict() -> notecraft_spell::Dictionary {
        let aff = "SET UTF-8\nTRY esianrtolcdugmphbyfvkwz\nSFX S Y 1\nSFX S 0 s .\n";
        let dic = "6\nthe/S\ncat/S\nsat\non\nmat/S\nhello\n";
        notecraft_spell::Dictionary::load(aff.as_bytes(), dic.as_bytes()).unwrap()
    }

    fn session() -> crate::Session {
        let mut s = sample();
        s.set_dictionary("en_US", dict());
        blank_page(&mut s);
        s
    }

    #[test]
    fn f7_finds_the_next_misspelled_word_and_a_suggestion_fixes_it() {
        // parity: R.Review.Proofing.SpellCheck
        let mut s = session();
        run(&mut s, "text.insert", json!({"text": "the cta sat on teh mat"}));
        let o = s.page().unwrap().objects[0].id().to_string();
        run(&mut s, "caret.set", json!({"block": o, "para": 0, "offset": 0}));
        let r = run(&mut s, "review.spelling", json!({}));
        assert_eq!(r["word"], "cta", "KB.General.Spelling");
        assert!(r["suggestions"].as_array().unwrap().iter().any(|x| x == "cat"));
        assert!(s.view.spelling_pane, "UI.Panes.Spelling");
        run(&mut s, "review.spellingSuggestion", json!({"word": "cat"}));
        let r = run(&mut s, "review.spelling", json!({}));
        assert_eq!(r["word"], "teh");
        run(&mut s, "review.spellingSuggestion", json!({"word": "the"}));
        assert_eq!(run(&mut s, "review.spelling", json!({}))["done"], true);
        assert_eq!(
            s.page().unwrap().objects[0].as_outline().unwrap().paragraphs[0].text(),
            "the cat sat on the mat",
            "CM.MisspelledWord.NoSpellingSuggestions"
        );
    }

    #[test]
    fn squiggles_follow_the_options_ignore_all_and_the_user_dictionary() {
        let mut s = session();
        assert_eq!(s.misspelled("the cta sat"), vec![4..7], "OPT.Proofing.CheckAsYouType");
        assert!(s.misspelled("NASA and cats").len() == 1, "OPT.Proofing.IgnoreUppercase");
        let text = "see www.example.com or a@b.org";
        let flagged: Vec<&str> = s.misspelled(text).into_iter().map(|r| &text[r]).collect();
        assert_eq!(flagged, ["see", "or"], "OPT.Proofing.IgnoreUrls");
        assert!(s.misspelled("mat2 cats").is_empty(), "OPT.Proofing.IgnoreNumbers");
        run(&mut s, "review.ignoreAll", json!({"word": "cta"}));
        assert!(s.misspelled("cta").is_empty(), "UI.Panes.Spelling.IgnoreAll");
        run(&mut s, "review.addToDictionary", json!({"word": "NoteCraft"}));
        assert!(s.misspelled("NoteCraft").is_empty() && s.prefs.custom_words == ["NoteCraft"], "UI.Panes.Spelling.AddToDictionary");
        run(&mut s, "file.options", json!({"set": {"hide_spelling_errors": true}}));
        assert!(s.misspelled("zzzq").is_empty(), "OPT.Proofing.HideSpellingErrors");
        run(&mut s, "file.options", json!({"set": {"hide_spelling_errors": false, "check_as_you_type": false}}));
        assert!(s.misspelled("zzzq").is_empty(), "OPT.Proofing.CheckAsYouType");
        // A language without an installed dictionary: F7 explains, nothing is underlined.
        run(&mut s, "file.options", json!({"set": {"check_as_you_type": true}}));
        run(&mut s, "review.setLanguage", json!({"lang": "xx_NOPE"}));
        assert!(s.misspelled("zzzq").is_empty());
        let e = s.run("review.spelling", json!({})).unwrap_err();
        assert!(e.to_string().contains("xx_NOPE"), "R.Review.Language.SetLanguage: {e}");
        assert!(s.run("review.setLanguage", json!({"lang": "../etc"})).is_err());
    }
}
