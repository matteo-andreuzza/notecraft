//! NoteCraft full-text search (layer **L2**: an algorithm over L1 data, no session or UI).
//!
//! An inverted index from tokens to pages, kept up to date page by page as the engine sees
//! page revisions change. Queries follow the reference's search box (catalog
//! `BEH.Search.Instant`, `BEH.Search.PrefixMatch`, `BEH.Search.Quoted`, `BEH.Search.Scope`):
//!
//! - case-insensitive; every word is a **prefix** ("meet" finds "meeting");
//! - all words must match (AND);
//! - `"quoted phrases"` must appear verbatim (case-insensitively) in the page text;
//! - results can be limited to a page, section, section group or notebook.
//!
//! **Performance.** Pages get dense `u32` document numbers; a query word ORs the postings of
//! every token in its prefix range into a bitset, words are ANDed, phrases verified last. A
//! one-letter prefix over 100,000 pages stays a few thousand word operations, which keeps the
//! 100k-page budget (brief §7: < 50 ms) with room to spare; `tests::budget_100k_pages` pins it.
//!
//! Locked password-protected sections must never be handed to [`Index::upsert`] (catalog
//! `BEH.Search.LockedExcluded`); that rule is enforced by the engine.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::collections::{BTreeMap, HashMap};

use notecraft_doc::Id;

/// Longest token indexed, in bytes (longer runs of letters are cut: hostile text cannot blow up
/// the index with megabyte-long "words").
pub const MAX_TOKEN: usize = 64;
/// Most query terms considered (the rest are ignored).
pub const MAX_TERMS: usize = 32;
/// Results that get a snippet computed eagerly.
pub const SNIPPET_LIMIT: usize = 200;

/// Where a page sits, for scoping and display.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageMeta {
    /// The page.
    pub page: Id,
    /// Its section.
    pub section: Id,
    /// Its notebook.
    pub notebook: Id,
    /// The section groups between the notebook and the section, outermost first.
    pub groups: Vec<Id>,
}

/// A search scope (the scope drop-down of the search box).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    /// This page.
    Page(Id),
    /// This section.
    Section(Id),
    /// This section group (and everything inside it).
    SectionGroup(Id),
    /// This notebook.
    Notebook(Id),
    /// All open notebooks.
    #[default]
    All,
}

impl Scope {
    fn admits(&self, m: &PageMeta) -> bool {
        match *self {
            Scope::Page(id) => m.page == id,
            Scope::Section(id) => m.section == id,
            Scope::SectionGroup(id) => m.groups.contains(&id),
            Scope::Notebook(id) => m.notebook == id,
            Scope::All => true,
        }
    }
}

/// One search result.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// The page.
    pub page: Id,
    /// Its location.
    pub meta: PageMeta,
    /// Whether the title matched (title hits are listed first).
    pub in_title: bool,
    /// A short excerpt around the first match.
    pub snippet: String,
}

#[derive(Debug, Clone)]
struct Doc {
    meta: PageMeta,
    title_lower: String,
    title_tokens: Vec<String>,
    text_lower: String,
    text: String,
    tokens: Vec<String>,
}

/// The index.
#[derive(Debug, Default, Clone)]
pub struct Index {
    docs: Vec<Option<Doc>>,
    by_page: HashMap<Id, u32>,
    free: Vec<u32>,
    postings: BTreeMap<String, Vec<u32>>,
}

/// Split text into lowercase tokens (runs of alphanumeric characters), capped at
/// [`MAX_TOKEN`] bytes each.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            for l in c.to_lowercase() {
                if cur.len() + l.len_utf8() <= MAX_TOKEN {
                    cur.push(l);
                }
            }
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A parsed query: prefix words and verbatim phrases (both lowercase).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Query {
    /// Words that must each prefix-match a token.
    pub words: Vec<String>,
    /// Phrases that must appear verbatim.
    pub phrases: Vec<String>,
}

impl Query {
    /// Parse the search box text.
    pub fn parse(q: &str) -> Query {
        let mut words = Vec::new();
        let mut phrases = Vec::new();
        let mut rest = q;
        while let Some(start) = rest.find('"') {
            words.extend(tokenize(rest.get(..start).unwrap_or("")));
            let after = rest.get(start + 1..).unwrap_or("");
            match after.find('"') {
                Some(end) => {
                    let phrase = after.get(..end).unwrap_or("").trim().to_lowercase();
                    if !phrase.is_empty() {
                        // A phrase's words also narrow the candidates through the index.
                        words.extend(tokenize(&phrase));
                        phrases.push(phrase);
                    }
                    rest = after.get(end + 1..).unwrap_or("");
                }
                None => {
                    rest = after;
                }
            }
        }
        words.extend(tokenize(rest));
        words.sort();
        words.dedup();
        words.truncate(MAX_TERMS);
        phrases.truncate(MAX_TERMS);
        Query { words, phrases }
    }

    /// Whether there is nothing to search for.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty() && self.phrases.is_empty()
    }
}

struct Bits(Vec<u64>);

impl Bits {
    fn new(n: usize) -> Bits {
        Bits(vec![0; n.div_ceil(64)])
    }
    fn set(&mut self, i: u32) {
        if let Some(w) = self.0.get_mut(i as usize / 64) {
            *w |= 1u64 << (i % 64);
        }
    }
    fn and(&mut self, o: &Bits) {
        for (a, b) in self.0.iter_mut().zip(&o.0) {
            *a &= *b;
        }
    }
    fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().enumerate().flat_map(|(wi, &w)| {
            let mut w = w;
            std::iter::from_fn(move || {
                if w == 0 {
                    return None;
                }
                let b = w.trailing_zeros();
                w &= w - 1;
                Some(wi as u32 * 64 + b)
            })
        })
    }
}

impl Index {
    /// An empty index.
    pub fn new() -> Index {
        Index::default()
    }

    /// Number of indexed pages.
    pub fn len(&self) -> usize {
        self.by_page.len()
    }

    /// Whether no page is indexed.
    pub fn is_empty(&self) -> bool {
        self.by_page.is_empty()
    }

    /// Whether `page` is indexed.
    pub fn contains(&self, page: Id) -> bool {
        self.by_page.contains_key(&page)
    }

    /// Index (or re-index) a page: its title and its full text (title, paragraphs, tag names,
    /// file names…, as the engine assembles it).
    pub fn upsert(&mut self, meta: PageMeta, title: &str, text: &str) {
        self.remove(meta.page);
        let mut tokens = tokenize(title);
        tokens.extend(tokenize(text));
        tokens.sort();
        tokens.dedup();
        let n = match self.free.pop() {
            Some(n) => n,
            None => {
                self.docs.push(None);
                (self.docs.len() - 1) as u32
            }
        };
        for t in &tokens {
            let list = self.postings.entry(t.clone()).or_default();
            if let Err(pos) = list.binary_search(&n) {
                list.insert(pos, n);
            }
        }
        self.by_page.insert(meta.page, n);
        if let Some(slot) = self.docs.get_mut(n as usize) {
            *slot = Some(Doc {
                title_lower: title.to_lowercase(),
                title_tokens: tokenize(title),
                text_lower: text.to_lowercase(),
                text: text.to_string(),
                meta,
                tokens,
            });
        }
    }

    /// Drop a page from the index.
    pub fn remove(&mut self, page: Id) {
        let Some(n) = self.by_page.remove(&page) else { return };
        if let Some(Some(doc)) = self.docs.get_mut(n as usize).map(Option::take) {
            for t in &doc.tokens {
                let empty = match self.postings.get_mut(t) {
                    Some(list) => {
                        if let Ok(pos) = list.binary_search(&n) {
                            list.remove(pos);
                        }
                        list.is_empty()
                    }
                    None => false,
                };
                if empty {
                    self.postings.remove(t);
                }
            }
        }
        self.free.push(n);
    }

    /// Run a query in a scope. Results: title matches first, then the rest, each group in
    /// document-number order (insertion order, which the engine keeps as notebook order).
    pub fn search(&self, q: &str, scope: Scope) -> Vec<Hit> {
        let query = Query::parse(q);
        if query.is_empty() {
            return Vec::new();
        }
        let n = self.docs.len();
        let mut acc: Option<Bits> = None;
        for w in &query.words {
            let mut bits = Bits::new(n);
            for (_, list) in self.postings.range(w.clone()..).take_while(|(t, _)| t.starts_with(w.as_str())) {
                for &d in list {
                    bits.set(d);
                }
            }
            match acc.as_mut() {
                Some(a) => a.and(&bits),
                None => acc = Some(bits),
            }
        }
        let Some(acc) = acc else { return Vec::new() };
        let mut title_docs = Vec::new();
        let mut body_docs = Vec::new();
        for d in acc.iter() {
            let Some(Some(doc)) = self.docs.get(d as usize) else { continue };
            if !scope.admits(&doc.meta) {
                continue;
            }
            if !query.phrases.iter().all(|p| doc.text_lower.contains(p.as_str()) || doc.title_lower.contains(p.as_str())) {
                continue;
            }
            let in_title = query.words.iter().all(|w| doc.title_tokens.iter().any(|t| t.starts_with(w.as_str())));
            if in_title { title_docs.push(doc) } else { body_docs.push(doc) }
        }
        let n_title = title_docs.len();
        title_docs
            .into_iter()
            .chain(body_docs)
            .enumerate()
            .map(|(i, doc)| Hit {
                page: doc.meta.page,
                meta: doc.meta.clone(),
                in_title: i < n_title,
                // Snippets cost allocations; the results pane shows the first few hundred, and
                // [`Index::snippet`] computes the rest on demand.
                snippet: if i < SNIPPET_LIMIT { snippet(&doc.text, &doc.text_lower, &query) } else { String::new() },
            })
            .collect()
    }

    /// The snippet for `page` under query `q` (for results beyond [`SNIPPET_LIMIT`]).
    pub fn snippet(&self, page: Id, q: &str) -> String {
        let query = Query::parse(q);
        self.by_page
            .get(&page)
            .and_then(|&n| self.docs.get(n as usize))
            .and_then(Option::as_ref)
            .map(|d| snippet(&d.text, &d.text_lower, &query))
            .unwrap_or_default()
    }
}

/// About 80 characters of `text` around the first occurrence of any query term.
fn snippet(text: &str, lower: &str, q: &Query) -> String {
    let first = q.phrases.iter().chain(q.words.iter()).filter_map(|t| lower.find(t.as_str())).min().unwrap_or(0);
    // `lower` and `text` can differ in byte length (case folding); map by char count.
    let chars_before = lower.get(..first).map(|s| s.chars().count()).unwrap_or(0);
    let start = chars_before.saturating_sub(30);
    let s: String = text.chars().skip(start).take(80).collect::<String>().replace('\n', " ");
    let s = s.trim().to_string();
    if start > 0 { format!("…{s}") } else { s }
}

/// Case-insensitive occurrences of `needle` in `hay` as byte ranges into `hay` (Find on page,
/// catalog `UI.Nav.FindOnPage`). Matching compares lowercase chars one-to-one, so offsets stay
/// valid for `hay` even when case folding changes byte lengths.
pub fn find_all(hay: &str, needle: &str) -> Vec<std::ops::Range<usize>> {
    let needle: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let hay_chars: Vec<(usize, char)> = hay.char_indices().map(|(i, c)| (i, c.to_lowercase().next().unwrap_or(c))).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= hay_chars.len() {
        let matches = needle.iter().enumerate().all(|(k, c)| hay_chars.get(i + k).map(|h| h.1) == Some(*c));
        if matches {
            let start = hay_chars.get(i).map(|h| h.0).unwrap_or(0);
            let end = hay_chars.get(i + needle.len()).map(|h| h.0).unwrap_or(hay.len());
            out.push(start..end);
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(p: u128, s: u128, nb: u128, groups: &[u128]) -> PageMeta {
        PageMeta { page: Id(p), section: Id(s), notebook: Id(nb), groups: groups.iter().map(|g| Id(*g)).collect() }
    }

    fn index() -> Index {
        let mut ix = Index::new();
        ix.upsert(meta(1, 10, 100, &[]), "Weekly meeting", "Agenda: budget review. Action items for Anna.");
        ix.upsert(meta(2, 10, 100, &[]), "Groceries", "milk, eggs, bread — meet Anna at 5");
        ix.upsert(meta(3, 20, 100, &[50]), "Project plan", "Milestones and the budget for Q4");
        ix.upsert(meta(4, 30, 200, &[]), "Other notebook", "budget");
        ix
    }

    #[test]
    fn prefix_and_and_semantics() {
        // parity: BEH.Search.PrefixMatch
        let ix = index();
        let ids = |q: &str, s: Scope| ix.search(q, s).iter().map(|h| h.page.0).collect::<Vec<_>>();
        assert_eq!(ids("meet", Scope::All), vec![1, 2], "title hit first");
        assert_eq!(ids("BUDG", Scope::All), vec![1, 3, 4]);
        assert_eq!(ids("budget anna", Scope::All), vec![1]);
        assert!(ids("zzz", Scope::All).is_empty());
        assert!(ids("   ", Scope::All).is_empty());
    }

    #[test]
    fn scopes() {
        // parity: BEH.Search.Scope
        let ix = index();
        let ids = |s: Scope| ix.search("budget", s).iter().map(|h| h.page.0).collect::<Vec<_>>();
        assert_eq!(ids(Scope::Section(Id(10))), vec![1]);
        assert_eq!(ids(Scope::SectionGroup(Id(50))), vec![3]);
        assert_eq!(ids(Scope::Notebook(Id(100))), vec![1, 3]);
        assert_eq!(ids(Scope::Page(Id(4))), vec![4]);
    }

    #[test]
    fn quoted_phrases() {
        // parity: BEH.Search.Quoted
        let ix = index();
        assert_eq!(ix.search("\"budget review\"", Scope::All).len(), 1);
        assert_eq!(ix.search("\"review budget\"", Scope::All).len(), 0);
        let q = Query::parse("a \"b c\" d \"unterminated");
        assert_eq!(q.phrases, vec!["b c"]);
        assert!(q.words.contains(&"unterminated".to_string()));
    }

    #[test]
    fn reindex_and_remove() {
        let mut ix = index();
        ix.upsert(meta(2, 10, 100, &[]), "Groceries", "apples");
        assert!(ix.search("milk", Scope::All).is_empty());
        assert_eq!(ix.search("apples", Scope::All).len(), 1);
        ix.remove(Id(2));
        assert!(ix.search("apples", Scope::All).is_empty());
        assert_eq!(ix.len(), 3);
        // The freed document number is reused without leaking old postings.
        ix.upsert(meta(9, 10, 100, &[]), "New", "fresh words");
        assert_eq!(ix.search("fresh", Scope::All)[0].page, Id(9));
        assert!(ix.search("groceries", Scope::All).is_empty());
    }

    #[test]
    fn hostile_text_is_bounded() {
        let mut ix = Index::new();
        let long = "a".repeat(100_000);
        ix.upsert(meta(1, 1, 1, &[]), &long, &"\u{0}".repeat(1000));
        assert!(ix.postings.keys().all(|k| k.len() <= MAX_TOKEN));
        assert_eq!(ix.search(&long, Scope::All).len(), 1);
    }

    #[test]
    fn find_all_on_page() {
        // parity: UI.Nav.FindOnPage
        assert_eq!(find_all("Abc abc ABC", "abc"), vec![0..3, 4..7, 8..11]);
        // Case folding that changes byte length keeps ranges on char boundaries of the original.
        for r in find_all("İstanbul İSTANBUL", "stan") {
            assert!("İstanbul İSTANBUL".is_char_boundary(r.start));
        }
        assert!(find_all("x", "").is_empty());
        let s = "Ünïcode ünï";
        for r in find_all(s, "ÜNÏ") {
            assert!(s.is_char_boundary(r.start) && s.is_char_boundary(r.end));
        }
    }

    #[test]
    fn budget_100k_pages() {
        // Brief §7 budget: search over 100k pages < 50 ms (release). Debug builds get 10× slack.
        let mut ix = Index::new();
        let words = ["alpha", "beta", "gamma", "delta", "meeting", "budget", "anna", "notes", "plan", "review"];
        for i in 0..100_000u128 {
            let w = words[(i % 10) as usize];
            let v = words[((i / 10) % 10) as usize];
            ix.upsert(meta(i + 1, i % 500, 1, &[]), &format!("{w} page {i}"), &format!("{v} {w} body text number {i}"));
        }
        let budget = if cfg!(debug_assertions) { 500 } else { 50 };
        for q in ["a", "meet", "budget anna", "\"body text\""] {
            #[allow(clippy::disallowed_methods)]
            let t = std::time::Instant::now();
            let hits = ix.search(q, Scope::All);
            let ms = t.elapsed().as_millis();
            assert!(!hits.is_empty());
            assert!(ms < budget, "query {q:?} took {ms} ms (budget {budget} ms)");
        }
    }
}
