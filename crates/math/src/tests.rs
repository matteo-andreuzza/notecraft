use proptest::prelude::*;

use crate::{Item, MathBox, Measure, Node, layout, parse};

/// Monospace-ish measure: 0.5 em a character, 0.75/0.25 em above/below the baseline.
struct Mono;

impl Measure for Mono {
    fn measure(&self, text: &str, size: f32, _italic: bool) -> (f32, f32, f32) {
        (text.chars().count() as f32 * size * 0.5, size * 0.75, size * 0.25)
    }
}

fn id(s: &str) -> Node {
    Node::Ident(s.into())
}

fn op(s: &str) -> Node {
    Node::Op(s.into())
}

fn num(s: &str) -> Node {
    Node::Number(s.into())
}

#[test]
fn fractions_scripts_and_operator_precedence() {
    assert_eq!(parse("a/b"), Node::Frac(Box::new(id("a")), Box::new(id("b"))));
    // Parentheses around an operand go; `+` binds looser than `/`.
    assert_eq!(parse("(a+b)/c"), Node::Frac(Box::new(Node::Row(vec![id("a"), op("+"), id("b")])), Box::new(id("c"))));
    assert_eq!(parse("a+b/c"), Node::Row(vec![id("a"), op("+"), Node::Frac(Box::new(id("b")), Box::new(id("c")))]));
    assert_eq!(parse("x_i^2"), Node::Script { base: Box::new(id("x")), sub: Some(Box::new(id("i"))), sup: Some(Box::new(num("2"))) });
    assert_eq!(parse("e^(-x)"), Node::Script { base: Box::new(id("e")), sub: None, sup: Some(Box::new(Node::Row(vec![op("−"), id("x")]))) });
    assert_eq!(parse("x^-1"), Node::Script { base: Box::new(id("x")), sub: None, sup: Some(Box::new(Node::Row(vec![op("−"), num("1")]))) });
}

#[test]
fn radicals_large_operators_brackets_matrices_and_words() {
    assert_eq!(parse("√(3&x)"), Node::Radical { degree: Some(Box::new(num("3"))), body: Box::new(id("x")) });
    assert_eq!(parse("∛x"), Node::Radical { degree: Some(Box::new(num("3"))), body: Box::new(id("x")) });
    let Node::NAry { op: '∑', sub: Some(sub), sup: Some(sup), body } = parse("∑_(i=1)^n x_i") else { panic!("{:?}", parse("∑_(i=1)^n x_i")) };
    assert_eq!((*sub, *sup), (Node::Row(vec![id("i"), op("="), num("1")]), id("n")));
    assert!(matches!(*body, Node::Script { .. }));
    assert_eq!(parse("[x]"), Node::Delim { open: '[', close: ']', body: Box::new(id("x")) });
    assert_eq!(parse("(x"), Node::Delim { open: '(', close: '\0', body: Box::new(id("x")) }, "an unclosed bracket");
    assert_eq!(parse("■(a&b@c&d)"), Node::Matrix(vec![vec![id("a"), id("b")], vec![id("c"), id("d")]]));
    // Function names are upright words; other letters are one variable each.
    assert_eq!(parse("sinx"), Node::Row(vec![Node::Text("sin".into()), id("x")]));
    // Terms written together are one operand: `2a` is the whole denominator, `dx` the numerator.
    assert_eq!(parse("1/2a"), Node::Frac(Box::new(num("1")), Box::new(Node::Row(vec![num("2"), id("a")]))));
    assert_eq!(parse("dy/dx"), Node::Frac(Box::new(Node::Row(vec![id("d"), id("y")])), Box::new(Node::Row(vec![id("d"), id("x")]))));
    assert_eq!(parse("a b/c"), Node::Row(vec![id("a"), Node::Frac(Box::new(id("b")), Box::new(id("c")))]), "a space ends a term");
    assert_eq!(parse("ab"), Node::Row(vec![id("a"), id("b")]));
    assert_eq!(parse("\\alpha+\\beta"), Node::Row(vec![id("α"), op("+"), id("β")]));
    assert_eq!(parse("\\nope"), Node::Row(vec![op("\\"), id("n"), id("o"), id("p"), id("e")]), "unknown control words stay as typed");
    assert_eq!(parse("x\u{302}"), Node::Accent { accent: '\u{302}', base: Box::new(id("x")) });
    assert_eq!(parse("⬚/⬚"), Node::Frac(Box::new(Node::Placeholder), Box::new(Node::Placeholder)));
    assert_eq!(parse("\"if \"x"), Node::Row(vec![Node::Text("if ".into()), id("x")]));
}

fn texts(b: &MathBox) -> Vec<(String, f32, f32)> {
    b.items.iter().filter_map(|i| if let Item::Text { text, x, y, .. } = i { Some((text.clone(), *x, *y)) } else { None }).collect()
}

#[test]
fn a_fraction_stacks_and_a_superscript_rises() {
    let b = layout(&parse("a/b"), 10.0, &Mono);
    let t = texts(&b);
    let (a, bb) = (&t[0], &t[1]);
    assert!(a.2 < 0.0 && bb.2 > 0.0, "numerator above the baseline, denominator below: {t:?}");
    assert!((a.1 - bb.1).abs() < 0.01, "both centred");
    assert!(b.items.iter().any(|i| matches!(i, Item::Path { .. })), "a fraction bar");
    let s = layout(&parse("x^2"), 10.0, &Mono);
    let t = texts(&s);
    assert!(t[1].2 < 0.0 && t[1].1 >= 5.0, "the 2 is raised, right of the x: {t:?}");
    // The quadratic formula lays out with the radical inside the fraction's numerator.
    let q = layout(&parse("x=(−b±√(b^2−4ac))/2a"), 12.0, &Mono);
    assert!(q.width > 0.0 && q.ascent > 12.0 && q.descent > 6.0);
    // Large operators and matrices have height.
    assert!(layout(&parse("∑_(k=0)^n k"), 12.0, &Mono).height() > 24.0);
    assert!(layout(&parse("(■(1&0@0&1))"), 12.0, &Mono).height() > 24.0);
}

#[test]
fn hostile_text_is_bounded() {
    let deep = "(".repeat(10_000) + "x";
    let b = layout(&parse(&deep), 12.0, &Mono);
    assert!(b.width.is_finite() && b.items.len() <= 4 * crate::MAX_LEN, "linear in the capped input");
    let long = "a/".repeat(50_000);
    let _ = layout(&parse(&long), 12.0, &Mono);
    let _ = layout(&parse("x"), f32::NAN, &Mono);
    let _ = layout(&parse("■(■(■("), 12.0, &Mono);
    let _ = layout(&parse("√(&)∑^_∫_^"), 12.0, &Mono);
}

proptest! {
    #[test]
    fn any_text_parses_and_lays_out_finitely(s in "[a-z0-9 /^_()\\[\\]{}|&@■√∑∫⬚+=\\-−\u{302}\\\\\"]{0,60}") {
        let b = layout(&parse(&s), 11.0, &Mono);
        prop_assert!(b.width.is_finite() && b.ascent.is_finite() && b.descent.is_finite());
        for it in &b.items {
            match it {
                Item::Text { x, y, .. } | Item::Slot { x, y, .. } => prop_assert!(x.is_finite() && y.is_finite()),
                Item::Path { points, .. } => prop_assert!(points.iter().all(|(x, y)| x.is_finite() && y.is_finite())),
            }
        }
    }
}
