//! NoteCraft spelling (layer **L2**, no dependencies): Hunspell-format dictionaries read from
//! their public format description (`hunspell(5)`), written from that description alone (no
//! Hunspell code is used or copied), checking words and suggesting corrections.
//!
//! Dictionaries are not shipped with NoteCraft: the desktop app reads the ones installed on
//! the system (`/usr/share/hunspell`, …) at run time, so their licences stay with them.
//!
//! What is read: `SET` (UTF-8 and the ISO 8859 single-byte sets), `FLAG` (characters, `long`,
//! `num`, `UTF-8`), `AF` flag aliases, `PFX`/`SFX` classes with stripping, continuation
//! classes and conditions, cross products (one prefix with one suffix), twofold suffixes (a
//! suffix on a suffix), `KEEPCASE`, `NEEDAFFIX`, `FORBIDDENWORD`, `NOSUGGEST`,
//! `ONLYINCOMPOUND`, `TRY` and `REP`. Not read (unverified gaps): compounding rules, `ICONV`/
//! `OCONV`, `IGNORE`, `CIRCUMFIX`, morphological fields, `CHECKSHARPS`; a word that needs
//! compounding is reported as misspelled.
//!
//! Input is hostile (any file on disk): line, rule and word counts are capped, conditions are
//! bounded, and nothing here panics.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::collections::{HashMap, HashSet};

/// Most dictionary words read.
pub const MAX_WORDS: usize = 2_000_000;
/// Most affix rules read.
pub const MAX_RULES: usize = 100_000;
/// Longest word checked (characters); longer ones are accepted unchecked.
pub const MAX_WORD: usize = 100;
/// Most suggestions returned.
pub const MAX_SUGGESTIONS: usize = 8;

/// A flag (characters, long pairs, numbers and UTF-8 all map to a number).
type Flag = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum FlagMode {
    #[default]
    Char,
    Long,
    Num,
    Utf8,
}

/// One bracket or dot of a condition.
#[derive(Debug, Clone, PartialEq)]
enum CondPart {
    Any,
    Set(Vec<char>, bool),
}

#[derive(Debug, Clone)]
struct Rule {
    strip: String,
    add: String,
    cont: Vec<Flag>,
    cond: Vec<CondPart>,
}

#[derive(Debug, Clone)]
struct Class {
    cross: bool,
    rules: Vec<Rule>,
}

/// A loaded dictionary.
#[derive(Debug, Default)]
pub struct Dictionary {
    words: HashMap<String, Vec<Vec<Flag>>>,
    prefixes: HashMap<Flag, Class>,
    suffixes: HashMap<Flag, Class>,
    keepcase: Option<Flag>,
    needaffix: Option<Flag>,
    forbidden: Option<Flag>,
    nosuggest: Option<Flag>,
    onlyincompound: Option<Flag>,
    try_chars: Vec<char>,
    rep: Vec<(String, String)>,
    /// Words added by the user (Add to Dictionary) or ignored for the session.
    extra: HashSet<String>,
}

/// Decode a file in the encoding `SET` names (UTF-8 if none; single-byte sets as Latin-1,
/// which is right for ISO 8859-1 and near enough for the others' letters).
fn decode(bytes: &[u8], set: &str) -> String {
    if set.eq_ignore_ascii_case("UTF-8") || set.is_empty() {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        bytes.iter().map(|&b| char::from(b)).collect()
    }
}

fn find_set(aff: &[u8]) -> String {
    let head = String::from_utf8_lossy(aff.get(..aff.len().min(4096)).unwrap_or_default()).into_owned();
    head.lines().find_map(|l| l.trim().strip_prefix("SET ").map(|v| v.trim().to_string())).unwrap_or_default()
}

fn parse_flags(s: &str, mode: FlagMode) -> Vec<Flag> {
    match mode {
        FlagMode::Char => s.bytes().map(Flag::from).collect(),
        FlagMode::Utf8 => s.chars().map(|c| c as Flag).collect(),
        FlagMode::Long => {
            let b: Vec<char> = s.chars().collect();
            b.chunks(2).map(|c| c.iter().fold(0u32, |acc, ch| acc.wrapping_mul(65536).wrapping_add(*ch as u32))).collect()
        }
        FlagMode::Num => s.split(',').filter_map(|n| n.trim().parse().ok()).collect(),
    }
}

/// Flag alias `i` (1-based, as `AF` numbers them).
fn alias(aliases: &[Vec<Flag>], i: usize) -> Vec<Flag> {
    i.checked_sub(1).and_then(|k| aliases.get(k)).cloned().unwrap_or_default()
}

fn parse_cond(c: &str) -> Option<Vec<CondPart>> {
    if c == "." {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    let mut it = c.chars().peekable();
    while let Some(ch) = it.next() {
        match ch {
            '.' => out.push(CondPart::Any),
            '[' => {
                let neg = it.peek() == Some(&'^');
                if neg {
                    it.next();
                }
                let mut set = Vec::new();
                loop {
                    match it.next() {
                        Some(']') => break,
                        Some(x) => set.push(x),
                        None => return None,
                    }
                    if set.len() > 512 {
                        return None;
                    }
                }
                out.push(CondPart::Set(set, neg));
            }
            x => out.push(CondPart::Set(vec![x], false)),
        }
        if out.len() > 64 {
            return None;
        }
    }
    Some(out)
}

fn part_matches(p: &CondPart, c: char) -> bool {
    match p {
        CondPart::Any => true,
        CondPart::Set(set, neg) => set.contains(&c) != *neg,
    }
}

/// Whether `cond` matches the end (suffixes) or the start (prefixes) of `stem`.
fn cond_ok(cond: &[CondPart], stem: &str, at_end: bool) -> bool {
    let chars: Vec<char> = stem.chars().collect();
    if cond.len() > chars.len() {
        return false;
    }
    if at_end {
        let tail = &chars[chars.len() - cond.len()..];
        cond.iter().zip(tail).all(|(p, c)| part_matches(p, *c))
    } else {
        cond.iter().zip(&chars).all(|(p, c)| part_matches(p, *c))
    }
}

impl Dictionary {
    /// Load a dictionary from its `.aff` and `.dic` files.
    pub fn load(aff: &[u8], dic: &[u8]) -> Result<Dictionary, String> {
        let set = find_set(aff);
        let aff = decode(aff, &set);
        let dic = decode(dic, &set);
        let mut d = Dictionary::default();
        let mut mode = FlagMode::Char;
        let mut aliases: Vec<Vec<Flag>> = Vec::new();
        let mut af_seen = false;
        let mut rules = 0usize;
        let mut pending: Option<(bool, Flag, usize)> = None;
        for line in aff.lines() {
            let line = line.trim_end();
            let mut f = line.split_whitespace();
            let Some(key) = f.next() else { continue };
            if key.starts_with('#') {
                continue;
            }
            let arg = f.next().unwrap_or("");
            match key {
                "FLAG" => {
                    mode = match arg {
                        "long" => FlagMode::Long,
                        "num" => FlagMode::Num,
                        "UTF-8" => FlagMode::Utf8,
                        _ => FlagMode::Char,
                    }
                }
                "AF" => {
                    // "AF n" first (the count), then one "AF flags" line per alias; the
                    // dictionary and rules name aliases from 1.
                    if !af_seen && arg.parse::<usize>().is_ok() {
                        af_seen = true;
                    } else if aliases.len() < 65_000 {
                        aliases.push(parse_flags(arg, mode));
                    }
                }
                "KEEPCASE" => d.keepcase = parse_flags(arg, mode).first().copied(),
                "NEEDAFFIX" | "PSEUDOROOT" => d.needaffix = parse_flags(arg, mode).first().copied(),
                "FORBIDDENWORD" => d.forbidden = parse_flags(arg, mode).first().copied(),
                "NOSUGGEST" => d.nosuggest = parse_flags(arg, mode).first().copied(),
                "ONLYINCOMPOUND" => d.onlyincompound = parse_flags(arg, mode).first().copied(),
                "TRY" => d.try_chars = arg.chars().take(200).collect(),
                "REP" => {
                    if let Some(to) = f.next()
                        && d.rep.len() < 10_000
                    {
                        d.rep.push((arg.replace('_', " "), to.replace('_', " ")));
                    }
                }
                "PFX" | "SFX" => {
                    let prefix = key == "PFX";
                    let flag = parse_flags(arg, mode).first().copied().unwrap_or(0);
                    let fields: Vec<&str> = f.collect();
                    match (&pending, fields.as_slice()) {
                        // A class header: cross product and count.
                        (None, [cross, count, ..]) if count.parse::<usize>().is_ok() && (*cross == "Y" || *cross == "N") => {
                            let n = count.parse::<usize>().unwrap_or(0).min(MAX_RULES);
                            let class = Class { cross: *cross == "Y", rules: Vec::new() };
                            if prefix {
                                d.prefixes.insert(flag, class);
                            } else {
                                d.suffixes.insert(flag, class);
                            }
                            pending = (n > 0).then_some((prefix, flag, n));
                        }
                        (Some((p, fl, left)), [strip, add, rest @ ..]) if *p == prefix && *fl == flag => {
                            rules += 1;
                            if rules > MAX_RULES {
                                return Err("too many affix rules".into());
                            }
                            let cond = rest.first().copied().unwrap_or(".");
                            let (add, cont) = match add.split_once('/') {
                                Some((a, c)) => {
                                    let c = match (mode, c.parse::<usize>()) {
                                        (_, Ok(i)) if !aliases.is_empty() => alias(&aliases, i),
                                        _ => parse_flags(c, mode),
                                    };
                                    (a, c)
                                }
                                None => (*add, Vec::new()),
                            };
                            let rule = Rule {
                                strip: if *strip == "0" { String::new() } else { (*strip).to_string() },
                                add: if add == "0" { String::new() } else { add.to_string() },
                                cont,
                                cond: parse_cond(cond).unwrap_or_default(),
                            };
                            let map = if prefix { &mut d.prefixes } else { &mut d.suffixes };
                            if let Some(c) = map.get_mut(fl) {
                                c.rules.push(rule);
                            }
                            let left = left - 1;
                            pending = (left > 0).then_some((prefix, flag, left));
                        }
                        _ => pending = None,
                    }
                }
                _ => {}
            }
        }
        let mut lines = dic.lines();
        let _count = lines.next();
        for line in lines.take(MAX_WORDS) {
            let line = line.trim_end_matches(['\r', '\n']);
            // Morphological fields follow a tab (or spaces before a "xx:" field).
            let entry = line.split('\t').next().unwrap_or("");
            let entry = entry.split(" po:").next().unwrap_or(entry).split(" st:").next().unwrap_or(entry).trim();
            if entry.is_empty() {
                continue;
            }
            let (word, flags) = split_entry(entry);
            let flags = match flags {
                Some(fl) if !aliases.is_empty() && fl.parse::<usize>().is_ok() => alias(&aliases, fl.parse::<usize>().unwrap_or(0)),
                Some(fl) => parse_flags(fl, mode),
                None => Vec::new(),
            };
            if word.chars().count() > MAX_WORD {
                continue;
            }
            d.words.entry(word).or_default().push(flags);
        }
        if d.words.is_empty() {
            return Err("the dictionary has no words".into());
        }
        Ok(d)
    }

    /// Accept `word` from now on (Add to Dictionary, Ignore All).
    pub fn add_word(&mut self, word: &str) {
        if word.chars().count() <= MAX_WORD {
            self.extra.insert(word.to_string());
        }
    }

    fn has(flags: &[Flag], f: Option<Flag>) -> bool {
        f.is_some_and(|f| flags.contains(&f))
    }

    /// The flag sets of a stem that may stand: not forbidden.
    fn stems(&self, stem: &str) -> impl Iterator<Item = &Vec<Flag>> {
        self.words.get(stem).into_iter().flatten().filter(move |f| !Self::has(f, self.forbidden))
    }

    /// Whether `w` (exact case) is a dictionary form.
    fn check_exact(&self, w: &str) -> bool {
        if self.words.get(w).is_some_and(|v| v.iter().any(|f| Self::has(f, self.forbidden))) {
            return false;
        }
        // A plain stem.
        if self.stems(w).any(|f| !Self::has(f, self.needaffix) && !Self::has(f, self.onlyincompound)) {
            return true;
        }
        // Suffixes, with an optional prefix (cross product) and an optional second suffix.
        if self.suffixed(w, None) {
            return true;
        }
        // Prefixes, with an optional suffix.
        for (flag, class) in &self.prefixes {
            for r in &class.rules {
                let Some(rest) = w.strip_prefix(r.add.as_str()) else { continue };
                let stem = format!("{}{rest}", r.strip);
                if stem.is_empty() || !cond_ok(&r.cond, &stem, false) {
                    continue;
                }
                let needs_more = r.cont.iter().any(|c| Some(*c) == self.needaffix);
                if !needs_more && self.stems(&stem).any(|f| f.contains(flag) && !Self::has(f, self.onlyincompound)) {
                    return true;
                }
                if class.cross && self.suffixed(&stem, Some(*flag)) {
                    return true;
                }
            }
        }
        false
    }

    /// Whether `w` is a stem plus a suffix, or a suffix on a suffix. `prefix` is a prefix
    /// flag the stem must also carry (a cross product; the suffix class must allow it too).
    fn suffixed(&self, w: &str, prefix: Option<Flag>) -> bool {
        for (flag, class) in &self.suffixes {
            for r in &class.rules {
                let Some(base) = w.strip_suffix(r.add.as_str()) else { continue };
                let stem = format!("{base}{}", r.strip);
                // A suffix that itself needs another affix doesn't make a word on its own.
                if stem.is_empty() || !cond_ok(&r.cond, &stem, true) || r.cont.iter().any(|c| Some(*c) == self.needaffix) {
                    continue;
                }
                if self
                    .stems(&stem)
                    .any(|f| f.contains(flag) && prefix.is_none_or(|p| class.cross && f.contains(&p)) && !Self::has(f, self.onlyincompound))
                {
                    return true;
                }
            }
        }
        self.twofold(w, prefix)
    }

    /// A suffix whose rule carries the continuation class of a second, outer suffix.
    fn twofold(&self, w: &str, prefix: Option<Flag>) -> bool {
        for (outer_flag, outer) in &self.suffixes {
            for r2 in &outer.rules {
                let Some(base) = w.strip_suffix(r2.add.as_str()) else { continue };
                let mid = format!("{base}{}", r2.strip);
                if mid.is_empty() || !cond_ok(&r2.cond, &mid, true) {
                    continue;
                }
                for (flag, class) in &self.suffixes {
                    for r in &class.rules {
                        if !r.cont.contains(outer_flag) {
                            continue;
                        }
                        let Some(b) = mid.strip_suffix(r.add.as_str()) else { continue };
                        let stem = format!("{b}{}", r.strip);
                        if !stem.is_empty()
                            && cond_ok(&r.cond, &stem, true)
                            && self.stems(&stem).any(|f| f.contains(flag) && prefix.is_none_or(|p| class.cross && f.contains(&p)))
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Whether `word` is spelled right. Numbers, single letters, words too long to check and
    /// words with no letters are accepted. Capitalized and all-caps forms of a lowercase entry
    /// are accepted unless the entry keeps its case.
    pub fn check(&self, word: &str) -> bool {
        let word = word.trim_matches(|c: char| c == '\'' || c == '’');
        let n = word.chars().count();
        if n <= 1 || n > MAX_WORD || !word.chars().any(char::is_alphabetic) || word.chars().any(|c| c.is_ascii_digit()) {
            return true;
        }
        let word = &word.replace('’', "'");
        if self.extra.contains(word.as_str()) || self.check_exact(word) {
            return true;
        }
        let lower = word.to_lowercase();
        if self.extra.contains(&lower) {
            return true;
        }
        let first_upper = word.chars().next().is_some_and(char::is_uppercase);
        let all_upper = word.chars().filter(|c| c.is_alphabetic()).all(char::is_uppercase);
        if !(first_upper || all_upper) {
            return false;
        }
        let keeps = |w: &str| self.words.get(w).is_some_and(|v| v.iter().any(|f| Self::has(f, self.keepcase)));
        let mut cands = vec![lower.clone()];
        if all_upper {
            let mut c = lower.chars();
            if let Some(f) = c.next() {
                cands.push(f.to_uppercase().chain(c).collect());
            }
        }
        cands.into_iter().any(|c| self.check_exact(&c) && !keeps(&c))
    }

    /// Corrections for a misspelled `word`, best first: replacement table entries, then
    /// single edits (a letter dropped, swapped with its neighbour, replaced or added, from the
    /// `TRY` letters), then the word split in two. Suggestions keep the word's capitalization.
    pub fn suggest(&self, word: &str) -> Vec<String> {
        let chars: Vec<char> = word.chars().collect();
        if chars.is_empty() || chars.len() > MAX_WORD {
            return Vec::new();
        }
        let cap = chars.first().is_some_and(|c| c.is_uppercase());
        let lower: String = word.to_lowercase();
        let lc: Vec<char> = lower.chars().collect();
        let mut out: Vec<String> = Vec::new();
        let ok = |s: &str| !s.is_empty() && self.check(s) && !self.words.get(s).is_some_and(|v| v.iter().all(|f| Self::has(f, self.nosuggest)));
        let push = |out: &mut Vec<String>, s: String| {
            if !out.contains(&s) && out.len() < MAX_SUGGESTIONS {
                out.push(s);
            }
        };
        for (from, to) in &self.rep {
            if from.is_empty() {
                continue;
            }
            let mut start = 0;
            while let Some(i) = lower.get(start..).and_then(|s| s.find(from.as_str())).map(|i| i + start) {
                let cand = format!("{}{to}{}", &lower[..i], &lower[i + from.len()..]);
                if ok(&cand) {
                    push(&mut out, cand);
                }
                start = i + from.len().max(1);
                if start >= lower.len() {
                    break;
                }
            }
        }
        let letters: Vec<char> = if self.try_chars.is_empty() { "esianrtolcdugmphbyfvkwz'".chars().collect() } else { self.try_chars.clone() };
        let s = |v: &[char]| v.iter().collect::<String>();
        for i in 0..lc.len() {
            let mut v = lc.clone();
            v.remove(i);
            let c = s(&v);
            if ok(&c) {
                push(&mut out, c);
            }
        }
        for i in 0..lc.len().saturating_sub(1) {
            let mut v = lc.clone();
            v.swap(i, i + 1);
            let c = s(&v);
            if ok(&c) {
                push(&mut out, c);
            }
        }
        for i in 0..lc.len() {
            for &l in &letters {
                if lc.get(i) == Some(&l) {
                    continue;
                }
                let mut v = lc.clone();
                if let Some(x) = v.get_mut(i) {
                    *x = l;
                }
                let c = s(&v);
                if ok(&c) {
                    push(&mut out, c);
                }
            }
        }
        for i in 0..=lc.len() {
            for &l in &letters {
                let mut v = lc.clone();
                v.insert(i, l);
                let c = s(&v);
                if ok(&c) {
                    push(&mut out, c);
                }
            }
        }
        for i in 1..lc.len() {
            let (a, b) = (s(&lc[..i]), s(&lc[i..]));
            if a.chars().count() > 1 && b.chars().count() > 1 && ok(&a) && ok(&b) {
                push(&mut out, format!("{a} {b}"));
            }
        }
        if cap {
            out = out
                .into_iter()
                .map(|w| {
                    let mut c = w.chars();
                    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
                })
                .collect();
        }
        out
    }
}

/// Split a `.dic` entry into its word and flags at the first unescaped slash (a slash in the
/// first column belongs to the word; `\/` is a slash in the word).
fn split_entry(entry: &str) -> (String, Option<&str>) {
    let bytes = entry.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        match bytes.get(i) {
            Some(b'\\') => i += 2,
            Some(b'/') => return (entry[..i].replace("\\/", "/"), Some(&entry[i + 1..])),
            _ => i += 1,
        }
    }
    (entry.replace("\\/", "/"), None)
}

/// The words of `text` to check, with their byte ranges: runs of letters, digits and inner
/// apostrophes or hyphens.
pub fn words(text: &str) -> Vec<(std::ops::Range<usize>, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (k, &(i, c)) in chars.iter().enumerate() {
        let inner = (c == '\'' || c == '’' || c == '-') && start.is_some() && chars.get(k + 1).is_some_and(|(_, n)| n.is_alphanumeric());
        if c.is_alphanumeric() || inner {
            start.get_or_insert(i);
        } else if let Some(s) = start.take() {
            out.push((s..i, &text[s..i]));
        }
    }
    if let Some(s) = start {
        out.push((s..text.len(), &text[s..]));
    }
    out
}

#[cfg(test)]
mod tests;
