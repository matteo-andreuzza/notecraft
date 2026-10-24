//! The UnicodeMath linear format, read into a tree (the subset listed in the crate docs).

/// Longest linear text read, in characters (the rest is ignored).
pub const MAX_LEN: usize = 4_000;
/// Deepest nesting read; deeper structures are shown as typed.
pub const MAX_DEPTH: usize = 48;

/// One piece of an equation.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// Pieces side by side.
    Row(Vec<Node>),
    /// A variable: one letter, shown italic.
    Ident(String),
    /// A number.
    Number(String),
    /// An operator, relation or punctuation mark.
    Op(String),
    /// Upright text: a function name (`sin`, `lim`) or `"quoted text"`.
    Text(String),
    /// A stacked fraction.
    Frac(Box<Node>, Box<Node>),
    /// A base with a subscript, a superscript or both (limits under `lim`-like functions).
    Script {
        /// The base.
        base: Box<Node>,
        /// Below right (or under).
        sub: Option<Box<Node>>,
        /// Above right (or over).
        sup: Option<Box<Node>>,
    },
    /// A square root, or an n-th root with a degree.
    Radical {
        /// The degree (`√(3&x)`).
        degree: Option<Box<Node>>,
        /// What is under the sign.
        body: Box<Node>,
    },
    /// A large operator (∑ ∏ ∫ …) with its limits and what it applies to.
    NAry {
        /// The operator.
        op: char,
        /// Lower limit.
        sub: Option<Box<Node>>,
        /// Upper limit.
        sup: Option<Box<Node>>,
        /// Its argument.
        body: Box<Node>,
    },
    /// Brackets stretched around what they hold (`close` is `'\0'` when missing).
    Delim {
        /// Opening bracket.
        open: char,
        /// Closing bracket.
        close: char,
        /// Inside.
        body: Box<Node>,
    },
    /// A matrix: rows of cells.
    Matrix(Vec<Vec<Node>>),
    /// A combining accent over a base (`x̂`).
    Accent {
        /// The combining mark.
        accent: char,
        /// What it sits on.
        base: Box<Node>,
    },
    /// An empty slot (`⬚`).
    Placeholder,
}

/// Large operators.
pub(crate) const NARY: &[char] = &['∑', '∏', '∐', '∫', '∬', '∭', '∮', '⋃', '⋂', '⋁', '⋀'];
/// Combining accents understood after a base.
pub(crate) const ACCENTS: &[char] = &['\u{302}', '\u{303}', '\u{304}', '\u{305}', '\u{307}', '\u{308}', '\u{20D7}'];
/// Function names shown upright, longest first so `sinh` wins over `sin`.
const FUNCS: &[&str] = &[
    "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh", "sin", "cos", "tan", "cot", "sec", "csc", "log", "exp", "lim", "max", "min", "sup", "inf",
    "det", "dim", "gcd", "deg", "arg", "ln", "lg",
];
/// Whether `t` is a function name (a thin space follows it).
pub(crate) fn is_function(t: &str) -> bool {
    FUNCS.contains(&t)
}

/// Functions whose scripts go under and over them in a displayed equation.
pub(crate) const LIMIT_FUNCS: &[&str] = &["lim", "max", "min", "sup", "inf"];
/// Relations: a wide space on each side.
pub(crate) const RELATIONS: &str = "=≠<>≤≥≈≡≅∼∝→←↔⇒⇐⇔∈∉⊂⊃⊆⊇≔≜≪≫∣";
/// Binary operators: a medium space on each side after an operand.
pub(crate) const BINARY: &str = "+−±∓×÷·∘∗∪∩∧∨⊕⊗";

/// The symbol a control word stands for (`\alpha` → `α`), if it is one NoteCraft knows.
pub fn control_word(name: &str) -> Option<&'static str> {
    Some(match name {
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" => "ϵ",
        "varepsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" => "θ",
        "vartheta" => "ϑ",
        "iota" => "ι",
        "kappa" => "κ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "pi" => "π",
        "rho" => "ρ",
        "sigma" => "σ",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" => "ϕ",
        "varphi" => "φ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "Omega" => "Ω",
        "infty" => "∞",
        "pm" => "±",
        "mp" => "∓",
        "times" => "×",
        "div" => "÷",
        "cdot" => "·",
        "le" | "leq" => "≤",
        "ge" | "geq" => "≥",
        "ne" | "neq" => "≠",
        "approx" => "≈",
        "equiv" => "≡",
        "sim" => "∼",
        "propto" => "∝",
        "to" | "rightarrow" => "→",
        "leftarrow" => "←",
        "Rightarrow" => "⇒",
        "Leftrightarrow" => "⇔",
        "in" => "∈",
        "notin" => "∉",
        "subset" => "⊂",
        "subseteq" => "⊆",
        "cup" => "∪",
        "cap" => "∩",
        "forall" => "∀",
        "exists" => "∃",
        "partial" => "∂",
        "nabla" => "∇",
        "sum" => "∑",
        "prod" => "∏",
        "int" => "∫",
        "iint" => "∬",
        "iiint" => "∭",
        "oint" => "∮",
        "bigcup" => "⋃",
        "bigcap" => "⋂",
        "sqrt" => "√",
        "cbrt" => "∛",
        "matrix" => "■",
        "degree" => "°",
        "cdots" => "⋯",
        "ldots" => "…",
        "angle" => "∠",
        "perp" => "⊥",
        "parallel" => "∥",
        "hat" => "\u{302}",
        "tilde" => "\u{303}",
        "bar" => "\u{305}",
        "dot" => "\u{307}",
        "ddot" => "\u{308}",
        "vec" => "\u{20D7}",
        "box" => "⬚",
        _ => return None,
    })
}

/// Replace known `\name` control words by their symbols (a space after the name is eaten).
fn expand(text: &str) -> Vec<char> {
    let chars: Vec<char> = text.chars().take(MAX_LEN).collect();
    let mut out = Vec::with_capacity(chars.len());
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if c == '\\' {
            let name: String = chars.iter().skip(i + 1).take_while(|c| c.is_ascii_alphabetic()).collect();
            if let Some(sym) = control_word(&name) {
                out.extend(sym.chars());
                i += 1 + name.len();
                if chars.get(i) == Some(&' ') {
                    i += 1;
                }
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Read linear-format text into a tree. Never fails: what isn't understood is kept as text.
pub fn parse(text: &str) -> Node {
    let mut p = Parser { c: expand(text), i: 0, depth: 0 };
    let mut items = Vec::new();
    while p.i < p.c.len() {
        let before = p.i;
        let row = p.row(&|_| false);
        match row {
            Node::Row(v) => items.extend(v),
            n => items.push(n),
        }
        if p.i == before {
            // A stray closing bracket: keep it as typed.
            if let Some(c) = p.next() {
                items.push(Node::Op(c.to_string()));
            }
        }
    }
    row_of(items)
}

fn row_of(mut items: Vec<Node>) -> Node {
    if items.len() == 1 { items.pop().unwrap_or(Node::Row(Vec::new())) } else { Node::Row(items) }
}

/// An operand's own parentheses go: `(a+b)/c` puts `a+b` over `c`.
fn strip(n: Node) -> Node {
    match n {
        Node::Delim { open: '(', close: ')', body } => *body,
        n => n,
    }
}

struct Parser {
    c: Vec<char>,
    i: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += 1;
        Some(c)
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }

    /// Pieces until the end or a character `stop` accepts (not consumed).
    fn row(&mut self, stop: &dyn Fn(char) -> bool) -> Node {
        let mut items = Vec::new();
        loop {
            self.skip_spaces();
            match self.peek() {
                None => break,
                Some(c) if stop(c) => break,
                Some(c) if ")]}".contains(c) && self.depth > 0 => break,
                _ => {}
            }
            let before = self.i;
            // A term's pieces join the row (a fraction keeps its own).
            match self.fraction(stop) {
                Node::Row(v) => items.extend(v),
                f => items.push(f),
            }
            if self.i == before {
                // Never stall: take the character as typed.
                if let Some(c) = self.next() {
                    items.push(Node::Op(c.to_string()));
                }
            }
        }
        row_of(items)
    }

    /// `a/b/c` → `(a/b)/c`; each side is a whole term (`2a`, `dx`).
    fn fraction(&mut self, stop: &dyn Fn(char) -> bool) -> Node {
        let mut a = self.term(stop);
        loop {
            let save = self.i;
            self.skip_spaces();
            if self.peek() != Some('/') {
                self.i = save;
                return a;
            }
            self.i += 1;
            self.skip_spaces();
            let b = self.term(stop);
            a = Node::Frac(Box::new(strip(a)), Box::new(strip(b)));
        }
    }

    /// Pieces written together with no space or operator between them (`2a`, `dx`, `n(n+1)`):
    /// one operand, as UnicodeMath reads them.
    fn term(&mut self, stop: &dyn Fn(char) -> bool) -> Node {
        let first = self.factor(stop);
        // An operator stands alone: `+b` isn't one term.
        if matches!(first, Node::Op(_)) {
            return first;
        }
        let mut items = vec![first];
        while let Some(c) = self.peek() {
            let joins = c.is_alphanumeric() || "([{√∛∜⬚■\"".contains(c) || NARY.contains(&c);
            if !joins || stop(c) || items.len() > 1_000 {
                break;
            }
            let before = self.i;
            items.push(self.factor(stop));
            if self.i == before {
                break;
            }
        }
        row_of(items)
    }

    /// A piece and its scripts.
    fn factor(&mut self, stop: &dyn Fn(char) -> bool) -> Node {
        let base = self.primary(stop);
        self.scripts(base)
    }

    fn scripts(&mut self, base: Node) -> Node {
        let (mut sub, mut sup) = (None, None);
        loop {
            match self.peek() {
                Some('_') if sub.is_none() => {
                    self.i += 1;
                    sub = Some(Box::new(self.operand()));
                }
                Some('^') if sup.is_none() => {
                    self.i += 1;
                    sup = Some(Box::new(self.operand()));
                }
                _ => break,
            }
        }
        if sub.is_none() && sup.is_none() { base } else { Node::Script { base: Box::new(base), sub, sup } }
    }

    /// A script or limit: one piece (its parentheses removed), a sign and a piece (`x^-1`).
    fn operand(&mut self) -> Node {
        match self.peek() {
            Some(c @ ('-' | '−' | '+')) => {
                self.i += 1;
                let sign = Node::Op(if c == '-' { "−".into() } else { c.to_string() });
                let rest = strip(self.primary(&|_| false));
                Node::Row(vec![sign, rest])
            }
            _ => strip(self.primary(&|_| false)),
        }
    }

    fn primary(&mut self, stop: &dyn Fn(char) -> bool) -> Node {
        let Some(c) = self.peek() else { return Node::Row(Vec::new()) };
        if self.depth >= MAX_DEPTH {
            self.i += 1;
            return Node::Text(c.to_string());
        }
        let n = match c {
            '_' | '^' => return Node::Row(Vec::new()),
            '⬚' => {
                self.i += 1;
                Node::Placeholder
            }
            '"' => {
                self.i += 1;
                let mut s = String::new();
                while let Some(x) = self.next() {
                    if x == '"' {
                        break;
                    }
                    s.push(x);
                }
                Node::Text(s)
            }
            c if c.is_ascii_digit() || c == '.' && self.c.get(self.i + 1).is_some_and(char::is_ascii_digit) => {
                let mut s = String::new();
                while let Some(x) = self.peek().filter(|x| x.is_ascii_digit() || *x == '.') {
                    s.push(x);
                    self.i += 1;
                }
                Node::Number(s)
            }
            c if c.is_alphabetic() => {
                let word: String = self.c.iter().skip(self.i).take_while(|x| x.is_ascii_alphabetic()).collect();
                match FUNCS.iter().find(|f| word.starts_with(**f)) {
                    Some(f) => {
                        self.i += f.len();
                        Node::Text((*f).to_string())
                    }
                    None => {
                        self.i += 1;
                        Node::Ident(c.to_string())
                    }
                }
            }
            '(' | '[' | '{' | '|' | '‖' | '⟨' | '⌊' | '⌈' => {
                self.i += 1;
                let close = match c {
                    '(' => ')',
                    '[' => ']',
                    '{' => '}',
                    '⟨' => '⟩',
                    '⌊' => '⌋',
                    '⌈' => '⌉',
                    x => x,
                };
                self.depth += 1;
                let body = self.row(&|x| x == close);
                self.depth -= 1;
                let close = if self.peek() == Some(close) {
                    self.i += 1;
                    close
                } else {
                    '\0'
                };
                Node::Delim { open: c, close, body: Box::new(body) }
            }
            '√' | '∛' | '∜' => {
                self.i += 1;
                self.radical(c)
            }
            '■' => {
                self.i += 1;
                self.matrix()
            }
            c if NARY.contains(&c) => {
                self.i += 1;
                let (mut sub, mut sup) = (None, None);
                loop {
                    match self.peek() {
                        Some('_') if sub.is_none() => {
                            self.i += 1;
                            sub = Some(Box::new(self.operand()));
                        }
                        Some('^') if sup.is_none() => {
                            self.i += 1;
                            sup = Some(Box::new(self.operand()));
                        }
                        _ => break,
                    }
                }
                self.skip_spaces();
                self.depth += 1;
                let body = match self.peek() {
                    Some(x) if !stop(x) && !")]}&@".contains(x) && !RELATIONS.contains(x) => self.term(stop),
                    _ => Node::Row(Vec::new()),
                };
                self.depth -= 1;
                Node::NAry { op: c, sub, sup, body: Box::new(body) }
            }
            '-' => {
                self.i += 1;
                Node::Op("−".into())
            }
            c => {
                self.i += 1;
                Node::Op(c.to_string())
            }
        };
        // Combining accents sit on what they follow.
        let mut n = n;
        while let Some(a) = self.peek().filter(|a| ACCENTS.contains(a)) {
            self.i += 1;
            n = Node::Accent { accent: a, base: Box::new(n) };
        }
        n
    }

    /// After `√`: `√x`, `√(x+1)`, `√(n&x)`; `∛` and `∜` carry their degree.
    fn radical(&mut self, sign: char) -> Node {
        let fixed = match sign {
            '∛' => Some(Node::Number("3".into())),
            '∜' => Some(Node::Number("4".into())),
            _ => None,
        };
        self.skip_spaces();
        self.depth += 1;
        let (degree, body) = if self.peek() == Some('(') {
            self.i += 1;
            let first = self.row(&|x| x == '&' || x == ')');
            let out = if self.peek() == Some('&') {
                self.i += 1;
                let body = self.row(&|x| x == ')');
                (Some(first), body)
            } else {
                (None, first)
            };
            if self.peek() == Some(')') {
                self.i += 1;
            }
            out
        } else {
            (None, self.factor(&|_| false))
        };
        self.depth -= 1;
        Node::Radical { degree: degree.or(fixed).map(Box::new), body: Box::new(body) }
    }

    /// After `■`: `(a&b@c&d)`, rows split by `@`, cells by `&`.
    fn matrix(&mut self) -> Node {
        self.skip_spaces();
        if self.peek() != Some('(') {
            return Node::Op("■".into());
        }
        self.i += 1;
        self.depth += 1;
        let mut rows = vec![Vec::new()];
        loop {
            let cell = self.row(&|x| x == '&' || x == '@' || x == ')');
            if let Some(r) = rows.last_mut() {
                r.push(cell);
            }
            match self.next() {
                Some('&') => {}
                Some('@') => rows.push(Vec::new()),
                _ => break,
            }
            if rows.len() > 100 || rows.last().is_some_and(|r| r.len() > 100) {
                break;
            }
        }
        self.depth -= 1;
        Node::Matrix(rows)
    }
}
