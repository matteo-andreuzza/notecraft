//! Synthetic dictionaries written for these tests from the format description.

use super::*;

const AFF: &str = "SET UTF-8
TRY esianrtolcdugmphbyfvkwz'
KEEPCASE K
NEEDAFFIX N
FORBIDDENWORD F
NOSUGGEST S

REP 1
REP f ph

PFX A Y 1
PFX A 0 re .

SFX B Y 2
SFX B 0 ed [^y]
SFX B y ied y

SFX X Y 1
SFX X 0 able/Y .

SFX Y Y 1
SFX Y 0 s .

SFX P N 1
SFX P 0 ness .
";

const DIC: &str = "9
hello
try/B
work/AB
drink/X
NASA/K
happy/P
cherr/NY
phone/Y
foo/F
";

fn dict() -> Dictionary {
    Dictionary::load(AFF.as_bytes(), DIC.as_bytes()).unwrap()
}

#[test]
fn stems_affixes_and_their_rules() {
    let d = dict();
    for w in ["hello", "try", "tried", "work", "worked", "rework", "reworked", "drink", "drinkable", "drinkables"] {
        assert!(d.check(w), "{w} is a word");
    }
    // "tryed" breaks the [^y] condition, "try" has no re- prefix, "drink" takes -s only
    // after -able (a twofold suffix).
    for w in ["tryed", "workeded", "retried", "helloed", "drinks"] {
        assert!(!d.check(w), "{w} is not a word");
    }
}

#[test]
fn flags_change_what_a_word_may_do() {
    let d = dict();
    assert!(d.check("NASA") && !d.check("Nasa") && !d.check("nasa"), "KEEPCASE");
    assert!(!d.check("cherr") && d.check("cherrs"), "NEEDAFFIX: only with an affix");
    assert!(!d.check("foo"), "FORBIDDENWORD");
    assert!(d.check("happyness"), "a plain suffix");
    assert!(!d.check("rehappyness"), "P has no cross product and happy no A");
}

#[test]
fn case_numbers_and_apostrophes() {
    let d = dict();
    assert!(d.check("Hello") && d.check("HELLO") && d.check("Worked"));
    assert!(!d.check("hELLO"), "mixed case isn't a capitalization");
    assert!(d.check("2024") && d.check("x") && d.check("a1b2"), "numbers and single letters pass");
    assert!(d.check("'hello'"));
}

#[test]
fn suggestions_come_from_rep_and_single_edits() {
    let d = dict();
    assert!(d.suggest("fone").contains(&"phone".to_string()), "REP f ph");
    assert!(d.suggest("helo").contains(&"hello".to_string()), "a letter added");
    assert!(d.suggest("hlelo").contains(&"hello".to_string()), "two letters swapped");
    assert!(d.suggest("wrok").contains(&"work".to_string()));
    assert_eq!(d.suggest("Helo").first().map(String::as_str), Some("Hello"), "capitalization kept");
    assert!(d.suggest("hellowork").contains(&"hello work".to_string()), "split in two");
    assert!(d.suggest("hello").len() <= MAX_SUGGESTIONS);
}

#[test]
fn user_words_and_other_flag_types() {
    let mut d = dict();
    assert!(!d.check("NoteCraft"));
    d.add_word("NoteCraft");
    assert!(d.check("NoteCraft"));
    // Long and numeric flags, aliases.
    let aff = "FLAG long\nSFX Aa Y 1\nSFX Aa 0 s .\n";
    let d = Dictionary::load(aff.as_bytes(), b"1\ncat/Aa\n").unwrap();
    assert!(d.check("cats"));
    let aff = "FLAG num\nSFX 101 Y 1\nSFX 101 0 s .\n";
    let d = Dictionary::load(aff.as_bytes(), b"1\ndog/7,101\n").unwrap();
    assert!(d.check("dogs"));
    let aff = "AF 1\nAF B\nSFX B Y 1\nSFX B 0 s .\n";
    let d = Dictionary::load(aff.as_bytes(), b"1\nbird/1\n").unwrap();
    assert!(d.check("birds"), "AF aliases count from 1");
    // ISO 8859-1 files.
    let aff = b"SET ISO8859-1\n";
    let d = Dictionary::load(aff, b"1\ncaf\xe9\n").unwrap();
    assert!(d.check("café"));
}

#[test]
fn words_are_split_out_of_text() {
    let w: Vec<&str> = words("Don't re-check e-mail, 3 times!").into_iter().map(|(_, s)| s).collect();
    assert_eq!(w, ["Don't", "re-check", "e-mail", "3", "times"]);
    assert!(words("").is_empty());
}

#[test]
fn hostile_files_are_errors_or_bounded() {
    assert!(Dictionary::load(b"", b"").is_err(), "no words");
    let _ = Dictionary::load(b"SFX A Y 999999999\nSFX A 0 [[[[ [\nPFX", b"1\nx/A\n");
    let _ = Dictionary::load(b"FLAG long\nAF 5\nAF\n", b"1\n/\n\\/x/\n");
    let mut aff = String::from("SFX A Y 200001\n");
    for _ in 0..200_001 {
        aff.push_str("SFX A 0 s .\n");
    }
    // A class claiming more rules than MAX_RULES keeps only the first MAX_RULES.
    let d = Dictionary::load(aff.as_bytes(), b"1\nx/A\n").unwrap();
    assert!(d.suffixes.values().map(|c| c.rules.len()).sum::<usize>() <= MAX_RULES, "rules are capped");
    let d = dict();
    let long = "a".repeat(10_000);
    assert!(d.check(&long), "too long to check");
    assert!(d.suggest(&long).is_empty());
}
