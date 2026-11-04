//! MyThes thesaurus files (`th_en_US_v2.dat`, as LibreOffice installs them): a first line
//! naming the encoding, then entries `word|count`, each followed by `count` meaning lines
//! `(part of speech)|synonym|synonym…`. The files are read from the system at run time.

use std::collections::HashMap;

/// Most entries kept from one file.
pub const MAX_ENTRIES: usize = 1_000_000;
/// Most meanings kept for one word, and synonyms kept for one meaning.
pub const MAX_MEANINGS: usize = 64;

/// One sense of a word: its part of speech (`noun`, may be empty) and its synonyms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meaning {
    /// The part of speech, without brackets.
    pub pos: String,
    /// The synonyms, in the file's order.
    pub words: Vec<String>,
}

/// A loaded thesaurus: lower-case word → its meanings.
#[derive(Debug, Default)]
pub struct Thesaurus {
    map: HashMap<String, Vec<Meaning>>,
}

impl Thesaurus {
    /// Read a `.dat` file. UTF-8 files are read as such; anything else (ISO-8859-1, the other
    /// common encoding) byte by byte as Latin-1. Malformed lines are skipped, never an error.
    pub fn parse(dat: &[u8]) -> Thesaurus {
        let first = dat.split(|b| *b == b'\n').next().unwrap_or_default();
        let utf8 = String::from_utf8_lossy(first).trim().eq_ignore_ascii_case("utf-8");
        let text: String = if utf8 { String::from_utf8_lossy(dat).into_owned() } else { dat.iter().map(|b| char::from(*b)).collect() };
        let mut map: HashMap<String, Vec<Meaning>> = HashMap::new();
        let mut lines = text.lines().skip(1);
        while let Some(line) = lines.next() {
            if map.len() >= MAX_ENTRIES {
                break;
            }
            let Some((word, count)) = line.split_once('|') else { continue };
            let Ok(count) = count.trim().parse::<usize>() else { continue };
            let mut meanings = Vec::new();
            for _ in 0..count {
                let Some(m) = lines.next() else { break };
                let mut parts = m.split('|');
                let pos = parts.next().unwrap_or("").trim().trim_start_matches('(').trim_end_matches(')').trim();
                let pos = if pos == "-" { "" } else { pos };
                let words: Vec<String> = parts.map(str::trim).filter(|w| !w.is_empty()).take(MAX_MEANINGS).map(str::to_string).collect();
                if !words.is_empty() && meanings.len() < MAX_MEANINGS {
                    meanings.push(Meaning { pos: pos.to_string(), words });
                }
            }
            let key = word.trim().to_lowercase();
            if !key.is_empty() && !meanings.is_empty() {
                map.entry(key).or_insert(meanings);
            }
        }
        Thesaurus { map }
    }

    /// The meanings of `word` (any case); empty when it isn't listed.
    pub fn lookup(&self, word: &str) -> &[Meaning] {
        self.map.get(&word.trim().to_lowercase()).map_or(&[], Vec::as_slice)
    }

    /// How many words are listed.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether no word is listed.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_entries_in_either_encoding_and_skips_junk() {
        let t =
            Thesaurus::parse(b"UTF-8\nhappy|2\n(adj)|glad|content\n(adj)|felicitous\nbroken line\ncaf\xc3\xa9|1\n-|bistro\nshort|5\n(adj)|brief\n");
        assert_eq!(t.lookup("Happy").len(), 2);
        assert_eq!(t.lookup("happy")[0], Meaning { pos: "adj".into(), words: vec!["glad".into(), "content".into()] });
        assert_eq!(t.lookup("café")[0].pos, "", "a dash is no part of speech");
        assert_eq!(t.lookup("short")[0].words, ["brief"], "a count past the end of the file");
        assert!(t.lookup("missing").is_empty());
        let latin = Thesaurus::parse(b"ISO8859-1\ncaf\xe9|1\n(noun)|bar\n");
        assert_eq!(latin.lookup("CAFÉ")[0].words, ["bar"]);
        assert!(Thesaurus::parse(b"").is_empty());
    }
}
