//! Napkin math (`BEH.Text.MathCalc`, option `OPT.Advanced.CalculateMath`): an expression typed
//! before `=` is worked out when Space or Enter follows, and the answer is typed after the sign.
//!
//! What it understands follows Microsoft's public pages ("Insert and calculate simple math
//! equations in OneNote" and the OneNote team's "Napkin Math in OneNote" post):
//! - `+`, `-` (also negation), `*` or `x`/`X`, `/`, `^`, `%` (a hundredth: `35*20%` is 7), `!`
//!   (factorial), `mod`, parentheses;
//! - `ABS ACOS ASIN ATAN COS DEG LN LOG LOG2 LOG10 RAD SIN SQRT TAN` with their argument in
//!   parentheses, `PMT(rate;payments;present value)`, the constants `PI`, `PHI`, `π`, `φ`
//!   (any case). The angle functions take and give degrees (`sin(30)` is 0.5 in the post);
//! - numbers with thousands separators (`70,000`) and a leading `$`; a `$` anywhere gives the
//!   answer as money (`$522,000.00`), separators in the input give them in the answer.
//!
//! The expression is the longest end of the line before the `=` that reads as one: "#38 mod 12"
//! works out "12", as the post describes. Answers have at most 15 significant digits
//! (unverified: the reference's rounding isn't documented and varies with the input).
//! Input is hostile: at most [`MAX_EXPRESSION`] characters, nesting at most 64 deep.

/// The longest expression looked at (characters before the `=`).
pub const MAX_EXPRESSION: usize = 256;
/// Deepest nesting of parentheses, signs and powers.
const MAX_NESTING: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tok {
    Num(f64),
    Ident(usize, usize),
    Op(char),
    Open,
    Close,
    Semi,
}

struct Lexed {
    toks: Vec<Tok>,
    money: bool,
    grouped: bool,
}

/// Split `s` into tokens; `None` on a character that can't be part of an expression.
fn lex(s: &str) -> Option<Lexed> {
    let chars: Vec<char> = s.chars().collect();
    let (mut toks, mut money, mut grouped) = (Vec::new(), false, false);
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        match c {
            ' ' | '\t' | '\u{a0}' => i += 1,
            '$' => {
                money = true;
                i += 1;
            }
            '0'..='9' | '.' => {
                let mut text = String::new();
                while let Some(&d) = chars.get(i) {
                    if d.is_ascii_digit() || d == '.' {
                        text.push(d);
                    } else if d == ',' && chars.get(i + 1..i + 4).is_some_and(|g| g.iter().all(char::is_ascii_digit)) {
                        // A thousands separator: exactly three digits follow.
                        grouped = true;
                    } else {
                        break;
                    }
                    i += 1;
                }
                toks.push(Tok::Num(text.parse().ok()?));
            }
            '+' | '-' | '−' | '*' | '×' | '/' | '÷' | '^' | '%' | '!' => {
                let op = match c {
                    '−' => '-',
                    '×' => '*',
                    '÷' => '/',
                    other => other,
                };
                toks.push(Tok::Op(op));
                i += 1;
            }
            '(' => {
                toks.push(Tok::Open);
                i += 1;
            }
            ')' => {
                toks.push(Tok::Close);
                i += 1;
            }
            ';' => {
                toks.push(Tok::Semi);
                i += 1;
            }
            'π' | 'Π' => {
                toks.push(Tok::Num(std::f64::consts::PI));
                i += 1;
            }
            'φ' | 'Φ' => {
                toks.push(Tok::Num(PHI));
                i += 1;
            }
            c if c.is_ascii_alphabetic() => {
                let start = i;
                while chars.get(i).is_some_and(|d| d.is_ascii_alphanumeric()) {
                    i += 1;
                }
                // `x` between numbers multiplies (`2x3`); `log10`, `log2` keep their digits.
                let word: String = chars.get(start..i)?.iter().collect::<String>().to_ascii_lowercase();
                if word == "x" {
                    toks.push(Tok::Op('*'));
                } else if word == "mod" {
                    toks.push(Tok::Op('m'));
                } else if let Some(split) = word.strip_prefix('x').filter(|r| r.chars().all(|d| d.is_ascii_digit())) {
                    // `x3` after a number: multiply by 3.
                    toks.push(Tok::Op('*'));
                    toks.push(Tok::Num(split.parse().ok()?));
                } else {
                    toks.push(Tok::Ident(start, i));
                }
            }
            _ => return None,
        }
    }
    Some(Lexed { toks, money, grouped })
}

const PHI: f64 = 1.618_033_988_749_895;

struct Parser<'a> {
    toks: &'a [Tok],
    chars: Vec<char>,
    at: usize,
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<Tok> {
        self.toks.get(self.at).copied()
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.peek();
        self.at += 1;
        t
    }

    fn enter(&mut self) -> Option<()> {
        self.depth += 1;
        (self.depth <= MAX_NESTING).then_some(())
    }

    fn expr(&mut self) -> Option<f64> {
        self.enter()?;
        let mut v = self.term()?;
        while let Some(Tok::Op(op @ ('+' | '-'))) = self.peek() {
            self.at += 1;
            let r = self.term()?;
            v = if op == '+' { v + r } else { v - r };
        }
        self.depth -= 1;
        Some(v)
    }

    fn term(&mut self) -> Option<f64> {
        let mut v = self.power()?;
        while let Some(Tok::Op(op @ ('*' | '/' | 'm'))) = self.peek() {
            self.at += 1;
            let r = self.power()?;
            v = match op {
                '*' => v * r,
                '/' if r != 0.0 => v / r,
                'm' if r != 0.0 => v.rem_euclid(r),
                _ => return None,
            };
        }
        Some(v)
    }

    fn power(&mut self) -> Option<f64> {
        self.enter()?;
        let base = self.unary()?;
        let v = if let Some(Tok::Op('^')) = self.peek() {
            self.at += 1;
            base.powf(self.power()?)
        } else {
            base
        };
        self.depth -= 1;
        Some(v)
    }

    fn unary(&mut self) -> Option<f64> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.at += 1;
                self.enter()?;
                let v = -self.unary()?;
                self.depth -= 1;
                Some(v)
            }
            Some(Tok::Op('+')) => {
                self.at += 1;
                self.enter()?;
                let v = self.unary()?;
                self.depth -= 1;
                Some(v)
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> Option<f64> {
        let mut v = self.primary()?;
        loop {
            match self.peek() {
                Some(Tok::Op('%')) => v /= 100.0,
                Some(Tok::Op('!')) => v = factorial(v)?,
                _ => return Some(v),
            }
            self.at += 1;
        }
    }

    fn args(&mut self) -> Option<Vec<f64>> {
        if self.next()? != Tok::Open {
            return None;
        }
        let mut out = vec![self.expr()?];
        loop {
            match self.next()? {
                Tok::Semi if out.len() < 3 => out.push(self.expr()?),
                Tok::Close => return Some(out),
                _ => return None,
            }
        }
    }

    fn primary(&mut self) -> Option<f64> {
        match self.next()? {
            Tok::Num(v) => Some(v),
            Tok::Open => {
                let v = self.expr()?;
                (self.next()? == Tok::Close).then_some(v)
            }
            Tok::Ident(a, b) => {
                let name: String = self.chars.get(a..b)?.iter().collect::<String>().to_ascii_lowercase();
                match name.as_str() {
                    "pi" => return Some(std::f64::consts::PI),
                    "phi" => return Some(PHI),
                    _ => {}
                }
                let args = self.args()?;
                let one = || if args.len() == 1 { args.first().copied() } else { None };
                match name.as_str() {
                    "abs" => one().map(f64::abs),
                    "sqrt" => one().filter(|v| *v >= 0.0).map(f64::sqrt),
                    "sin" => one().map(|v| v.to_radians().sin()),
                    "cos" => one().map(|v| v.to_radians().cos()),
                    "tan" => one().map(|v| v.to_radians().tan()),
                    "asin" => one().filter(|v| v.abs() <= 1.0).map(|v| v.asin().to_degrees()),
                    "acos" => one().filter(|v| v.abs() <= 1.0).map(|v| v.acos().to_degrees()),
                    "atan" => one().map(|v| v.atan().to_degrees()),
                    "deg" => one().map(f64::to_degrees),
                    "rad" => one().map(f64::to_radians),
                    "ln" | "log" => one().filter(|v| *v > 0.0).map(f64::ln),
                    "log2" => one().filter(|v| *v > 0.0).map(f64::log2),
                    "log10" => one().filter(|v| *v > 0.0).map(f64::log10),
                    "pmt" => match args.as_slice() {
                        [rate, n, pv] if *n > 0.0 => Some(if *rate == 0.0 { pv / n } else { pv * rate / (1.0 - (1.0 + rate).powf(-n)) }),
                        _ => None,
                    },
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// n! for whole n from 0 to 170 (beyond, f64 overflows).
fn factorial(v: f64) -> Option<f64> {
    if v < 0.0 || v > 170.0 || v.fract() != 0.0 {
        return None;
    }
    Some((1..=v as u32).fold(1.0, |acc, k| acc * f64::from(k)))
}

/// The value of `expr`, and whether it used money (`$`) or thousands separators.
fn evaluate_full(expr: &str) -> Option<(f64, bool, bool)> {
    if expr.chars().count() > MAX_EXPRESSION {
        return None;
    }
    let lexed = lex(expr)?;
    let mut p = Parser { toks: &lexed.toks, chars: expr.chars().collect(), at: 0, depth: 0 };
    let v = p.expr()?;
    (p.at == lexed.toks.len() && v.is_finite()).then_some((v, lexed.money, lexed.grouped))
}

/// The value of `expr` (no `=`), or `None` when it isn't a complete expression.
pub fn evaluate(expr: &str) -> Option<f64> {
    evaluate_full(expr).map(|(v, _, _)| v)
}

/// `digits` (an integer part) with a comma every three digits.
fn group(digits: &str) -> String {
    let n = digits.len();
    let mut out = String::with_capacity(n + n / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (n - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `v` written for the page: at most 15 significant digits, no trailing zeros, scientific
/// notation from 10^15 up; money with two decimals and separators; separators when asked.
pub fn format(v: f64, money: bool, grouped: bool) -> String {
    let v = if v == 0.0 { 0.0 } else { v };
    if money {
        let s = format!("{:.2}", v.abs());
        let (int, frac) = s.split_once('.').unwrap_or((&s, "00"));
        return format!("{}${}.{frac}", if v < 0.0 { "-" } else { "" }, group(int));
    }
    if v.abs() >= 1e15 {
        let s = format!("{v:.14E}");
        let (mant, exp) = s.split_once('E').unwrap_or((&s, "0"));
        let mant = mant.trim_end_matches('0').trim_end_matches('.');
        let exp: i32 = exp.parse().unwrap_or(0);
        return format!("{mant}E+{exp}");
    }
    // Round to 15 significant digits, then print the shortest form of that.
    let rounded: f64 = format!("{v:.14e}").parse().unwrap_or(v);
    let s = format!("{rounded}");
    if !grouped {
        return s;
    }
    let (sign, rest) = s.strip_prefix('-').map_or(("", s.as_str()), |r| ("-", r));
    match rest.split_once('.') {
        Some((int, frac)) => format!("{sign}{}.{frac}", group(int)),
        None => format!("{sign}{}", group(rest)),
    }
}

/// The answer for a line ending in `=` (the text before the caret, `=` included), or `None`
/// when no expression ends there. The expression is the longest end of the line that reads as
/// one, starting at the beginning of a word.
pub fn answer(line: &str) -> Option<String> {
    let before = line.strip_suffix('=')?.trim_end();
    let chars: Vec<char> = before.chars().collect();
    let skip = chars.len().saturating_sub(MAX_EXPRESSION);
    let tail: Vec<char> = chars.get(skip..)?.to_vec();
    for start in 0..tail.len() {
        // Only at the start of a word: "#38 mod 12" is "12", since "#38" isn't a number.
        let boundary = start == 0 || tail.get(start - 1).is_some_and(|c| c.is_whitespace());
        if !boundary || tail.get(start).is_some_and(|c| c.is_whitespace()) {
            continue;
        }
        let candidate: String = tail.get(start..)?.iter().collect();
        if let Some((v, money, grouped)) = evaluate_full(&candidate) {
            return Some(format(v, money, grouped));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Option<String> {
        answer(s)
    }

    #[test]
    fn the_documented_examples() {
        // The examples of Microsoft's public pages.
        assert_eq!(a("95+83+416=").as_deref(), Some("594"));
        assert_eq!(a("sin(30) =").as_deref(), Some("0.5"));
        assert_eq!(a("(6 + 7) / (4 * sqrt(3)) =").as_deref(), Some("1.87638837486628"));
        assert_eq!(a("5 ^ 4 =").as_deref(), Some("625"));
        assert_eq!(a("35 * 20 % =").as_deref(), Some("7"));
        assert_eq!(a("2 x 3 X 5 *9 =").as_deref(), Some("270"));
        assert_eq!(a("1 * pi=").as_deref(), Some("3.14159265358979"));
        assert_eq!(a("1 * phi=").as_deref(), Some("1.61803398874989"));
        assert_eq!(a("π=").as_deref(), Some("3.14159265358979"));
        assert_eq!(a("5 ! =").as_deref(), Some("120"));
        assert_eq!(a("38 mod 12 =").as_deref(), Some("2"));
        assert_eq!(a("#38 mod 12 =").as_deref(), Some("12"), "the longest end that reads as an expression");
        assert_eq!(a("18 * $29,000 =").as_deref(), Some("$522,000.00"));
        assert_eq!(a("pmt(.019; 36; $50,000) =").as_deref(), Some("$1,930.28"));
        assert_eq!(a("70,000 / 12 =").as_deref(), Some("5,833.33333333333"));
        assert_eq!(a("SQRT(15)=").as_deref(), Some("3.87298334620742"));
        assert_eq!(a("Sqrt(9)=").as_deref(), Some("3"), "function names in any case");
    }

    #[test]
    fn prose_and_bad_expressions_give_nothing() {
        assert_eq!(a("Total ="), None);
        assert_eq!(a("x ="), None);
        assert_eq!(a("1/0="), None);
        assert_eq!(a("sqrt(-1)="), None);
        assert_eq!(a("(1+2="), None);
        assert_eq!(a("200!="), None);
        assert_eq!(a("no equals sign"), None);
        assert_eq!(a("="), None);
    }

    #[test]
    fn answers_are_tidy() {
        assert_eq!(a("0.1+0.2=").as_deref(), Some("0.3"));
        assert_eq!(a("-3*2=").as_deref(), Some("-6"));
        assert_eq!(a("2^60=").as_deref(), Some("1.15292150460685E+18"));
        assert_eq!(a("Costs: 3+4=").as_deref(), Some("7"), "text before the expression is left alone");
        assert_eq!(a("2^3^2=").as_deref(), Some("512"), "powers group to the right");
        assert_eq!(a("rad(180)=").as_deref(), Some("3.14159265358979"));
    }

    #[test]
    fn hostile_input_is_bounded() {
        let deep = format!("{}1{}=", "(".repeat(10_000), ")".repeat(10_000));
        assert_eq!(a(&deep), None);
        let long = format!("{}=", "1+".repeat(5_000) + "1");
        let _ = a(&long);
        let minus = format!("{}1=", "-".repeat(1_000));
        let _ = a(&minus);
        assert_eq!(evaluate("9^9^9^9"), None, "overflow is no answer");
    }
}
