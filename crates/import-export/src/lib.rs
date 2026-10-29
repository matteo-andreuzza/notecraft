//! NoteCraft interop with other formats (layer **L4**: reads and writes the L1 model, used by
//! the L5 engine).
//!
//! [`html`] is the rich clipboard's format (paste from browsers and office apps, copy to
//! them); [`export`] gathers pages for File › Export and writes them as HTML and Single File
//! Web Page (`.mht`), [`markdown`] as Markdown. PDF, DOCX and page images join them in M3
//! (`docs/plan.md` Part C).
//! Every reader takes hostile input: sizes and nesting are bounded, and malformed input
//! degrades to plain text, never to an error that loses the user's paste or to a panic.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod export;
pub mod html;
pub mod markdown;
