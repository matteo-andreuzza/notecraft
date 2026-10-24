//! NoteCraft equations (layer **L2**, no dependencies; decision D11, `docs/plan.md` §4.16).
//!
//! An equation is stored as its *linear format*: UnicodeMath text such as `x=(−b±√(b^2−4ac))/2a`,
//! the notation the reference also accepts when typing an equation. This crate reads that text
//! ([`parse`]) into a tree of [`Node`]s and lays the tree out as built-up ("Professional") math
//! ([`layout`]): fractions stacked, scripts raised and lowered, radicals, large operators with
//! limits, stretchy brackets and matrices. Text is measured by the caller through [`Measure`],
//! so the crate knows nothing of fonts; the large signs (∑ ∏ ∫ √, brackets, fraction bars) are
//! returned as line paths so they don't depend on a font having the glyphs.
//!
//! The supported subset (what [`parse`] understands; anything else is shown as typed):
//! - letters (italic variables; `sin`, `cos`, `log`, `lim`… upright), numbers, operators and
//!   relations (`+ − ± × ÷ · = ≠ < > ≤ ≥ ≈ → ∈ …`), `"quoted text"`;
//! - `a/b` fractions (parentheses around an operand are removed), `x^2`, `x_i`, `x_i^2`;
//! - `√x`, `√(n&x)` (the n-th root); `∑ ∏ ∫ ∬ ∭ ∮ ⋃ ⋂` with `_` and `^` limits, then their body;
//! - `( )`, `[ ]`, `{ }`, `| |` stretched around what they hold; `■(a&b@c&d)` matrices;
//! - combining accents after a letter (`x̂ x̃ x̄ ẋ x⃗`); `⬚` an empty placeholder;
//! - `\alpha`-style control words for Greek letters and common symbols ([`control_word`]).
//!
//! Input is hostile (files, agents): text is capped at [`MAX_LEN`] characters and nesting at
//! [`MAX_DEPTH`]; nothing here panics.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod layout;
mod parse;

pub use layout::{Item, MathBox, Measure, layout};
pub use parse::{MAX_DEPTH, MAX_LEN, Node, control_word, parse};

/// The placeholder for an empty slot of a structure (`⬚`, U+2B1A).
pub const PLACEHOLDER: char = '⬚';

#[cfg(test)]
mod tests;
