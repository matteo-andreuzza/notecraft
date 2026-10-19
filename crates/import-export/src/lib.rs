//! NoteCraft interop with other formats (layer **L4**: reads and writes the L1 model, used by
//! the L5 engine).
//!
//! First code: [`html`], the rich clipboard's format (paste from browsers and office apps,
//! copy to them). Markdown, PDF, DOCX and page images join it in M3 (`docs/plan.md` Part C).
//! Every reader takes hostile input: sizes and nesting are bounded, and malformed input
//! degrades to plain text, never to an error that loses the user's paste or to a panic.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod html;
