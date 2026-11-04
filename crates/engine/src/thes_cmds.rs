//! Review › Thesaurus (Shift+F7): synonyms from a MyThes file installed on the system
//! (`/usr/share/mythes`, …), in the proofing language.

use std::sync::Arc;

use notecraft_spell::Thesaurus;
use serde_json::{Value, json};

use crate::selection::{PageSel, Pos, TextRange};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// The loaded thesaurus.
#[derive(Default)]
pub struct Thes {
    data: Option<Arc<Thesaurus>>,
    /// The language it is for (or that failed to load).
    lang: String,
    /// A load was tried for `lang`.
    tried: bool,
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // R.Review.Proofing.Thesaurus, KB.General.Thesaurus: the selected word, or the word at
        // the caret (which it selects, so a synonym replaces it).
        CommandSpec::new("review.thesaurus", "R.Review.Proofing.Thesaurus", "Thesaurus", "Review/Proofing/Thesaurus", thesaurus)
            .key("Shift+F7")
            .params(r#"{"word"?: word to look up (default: the selection or the word at the caret)} → {"word", "meanings": [{"pos", "words"}], "installed": bool}"#)
            .pure()
            .when(|s| if s.page().is_some() { None } else { Some("Open a page first") }),
    ]
}

fn thesaurus(s: &mut Session, v: &Value) -> CmdResult {
    let word = match p::str(v, "word") {
        Some(w) => w.trim().to_string(),
        None => match s.selected_text().filter(|t| !t.trim().is_empty()) {
            Some(t) => t.trim().to_string(),
            None => select_word_at_caret(s).unwrap_or_default(),
        },
    };
    if word.is_empty() || word.chars().count() > notecraft_spell::MAX_WORD {
        return Err(CmdError::Params("select a word, or give `word`".into()));
    }
    let t = s.thesaurus();
    let meanings: Vec<Value> = t.as_ref().map_or(&[][..], |t| t.lookup(&word)).iter().map(|m| json!({"pos": m.pos, "words": m.words})).collect();
    s.view.thesaurus = Some(word.clone());
    Ok(json!({"word": word, "meanings": meanings, "installed": t.is_some()}))
}

/// Select the word the caret is in (or touches) and return it.
fn select_word_at_caret(s: &mut Session) -> Option<String> {
    let r = s.text_range()?;
    let h = r.head;
    let page = s.page()?;
    let text = Session::block_paras(page, h.block)?.get(h.para)?.text();
    let (range, w) = notecraft_spell::words(&text).into_iter().find(|(r, _)| r.start <= h.offset && h.offset <= r.end)?;
    let w = w.to_string();
    s.sel.on_page = PageSel::Text(TextRange {
        anchor: Pos { block: h.block, para: h.para, offset: range.start },
        head: Pos { block: h.block, para: h.para, offset: range.end },
    });
    Some(w)
}

/// The folders thesauri are looked for in.
#[cfg(not(target_arch = "wasm32"))]
fn dirs() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Some(d) = std::env::var_os("XDG_DATA_HOME") {
        out.push(std::path::PathBuf::from(d).join("mythes"));
    }
    if let Some(h) = std::env::var_os("HOME") {
        out.push(std::path::PathBuf::from(h).join(".local/share/mythes"));
    }
    for d in ["/usr/share/mythes", "/usr/share/myspell/dicts", "/usr/share/myspell", "/usr/local/share/mythes"] {
        out.push(d.into());
    }
    out
}

/// Largest thesaurus file read.
#[cfg(not(target_arch = "wasm32"))]
const MAX_BYTES: u64 = 128 << 20;

#[cfg(not(target_arch = "wasm32"))]
fn load(lang: &str) -> Option<Thesaurus> {
    let short = lang.split(['_', '-']).next().unwrap_or(lang);
    for d in dirs() {
        for name in [format!("th_{lang}_v2.dat"), format!("th_{lang}.dat"), format!("th_{short}_v2.dat"), format!("th_{short}.dat")] {
            let f = d.join(name);
            if std::fs::metadata(&f).is_ok_and(|m| m.is_file() && m.len() <= MAX_BYTES)
                && let Ok(bytes) = std::fs::read(&f)
            {
                let t = Thesaurus::parse(&bytes);
                if !t.is_empty() {
                    return Some(t);
                }
            }
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn load(_: &str) -> Option<Thesaurus> {
    None
}

impl Session {
    /// Use `t` as the thesaurus (tests, hosts with their own data).
    pub fn set_thesaurus(&mut self, lang: &str, t: Thesaurus) {
        self.thes = Thes { data: Some(Arc::new(t)), lang: lang.to_string(), tried: true };
    }

    /// The thesaurus for the proofing language, loaded on first use.
    fn thesaurus(&mut self) -> Option<Arc<Thesaurus>> {
        let lang = if self.prefs.proofing_language.is_empty() { "en_US".to_string() } else { self.prefs.proofing_language.clone() };
        if self.thes.lang != lang || !self.thes.tried {
            self.thes = Thes { data: load(&lang).map(Arc::new), lang, tried: true };
        }
        self.thes.data.clone()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::tests::{run, sample};

    #[test]
    fn the_thesaurus_looks_up_the_word_at_the_caret_and_a_synonym_replaces_it() {
        // parity: R.Review.Proofing.Thesaurus KB.General.Thesaurus
        let mut s = sample();
        s.set_thesaurus("en_US", notecraft_spell::Thesaurus::parse(b"UTF-8\nhappy|1\n(adj)|glad|content\n"));
        run(&mut s, "page.new", json!({}));
        run(&mut s, "text.insert", json!({"text": "so happy"}));
        run(&mut s, "caret.left", json!({}));
        let r = run(&mut s, "review.thesaurus", json!({}));
        assert_eq!(r["word"], "happy");
        assert_eq!(r["meanings"][0]["words"], json!(["glad", "content"]));
        assert_eq!(s.view.thesaurus.as_deref(), Some("happy"), "the pane opens");
        run(&mut s, "text.insert", json!({"text": "glad"}));
        assert_eq!(s.page().unwrap().title.text(), "so glad", "the word was selected, so the synonym replaced it");
        let r = run(&mut s, "review.thesaurus", json!({"word": "sad"}));
        assert!(r["meanings"].as_array().unwrap().is_empty() && r["installed"] == true);
    }
}
