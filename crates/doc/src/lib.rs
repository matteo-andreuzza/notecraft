//! NoteCraft document model (layer **L1**: depends only on the L0 `geom` and `color` crates).
//!
//! Pure data, mirroring the reference product's object model ([MS-ONE] §2.2) so that `.one`
//! interop (M3) maps one-to-one:
//!
//! ```text
//! Workspace ─ Notebook ─┬─ SectionGroup ─ (Sections and SectionGroups, nested)
//!                       ├─ Section ─ Page ─┬─ PageObject::Outline ─ Paragraph ─ Run
//!                       │                  └─ PageObject::Table ─ TableRow ─ TableCell ─ Paragraph
//!                       └─ RecycleBin
//! ```
//!
//! This crate knows nothing about how notes are drawn, stored, searched or edited: editing
//! algorithms live in `notecraft-text` (L2), storage in `notecraft-format` (L3), the session and
//! commands in `notecraft-engine` (L5). It sits at L1 because every other layer reads it.
//!
//! Invariants are restored by the `normalize` methods (`Paragraph`, `Outline`, `Page`,
//! `Section`, `Notebook`), which the storage layer runs on everything it loads: data from disk
//! is hostile.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod id;
pub mod notebook;
pub mod page;
pub mod table;
pub mod tags;
pub mod text;
pub mod time;

pub use id::{Id, IdGen};
pub use notebook::{DeletedPage, DeletedSection, NavNode, Notebook, RecycleBin, Section, SectionColor, SectionGroup, Workspace, clean_name};
pub use page::{Outline, OutlineWidth, Page, PageObject, PageSetup, PaperSize, RuleLines};
pub use table::{Table, TableCell, TableRow};
pub use tags::{TagDef, TagSymbol, TagUse, builtin_tags};
pub use text::{Align, Bullet, CharFormat, ListFormat, Numbering, ParaStyle, Paragraph, Run, Script, StyleDef, sanitize_text};
pub use time::{Civil, Timestamp};

pub use notecraft_color as color;
pub use notecraft_geom as geom;

/// Errors from model operations.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum DocError {
    /// No object with this id.
    #[error("no {kind} with id {id}")]
    NotFound {
        /// What was looked for.
        kind: &'static str,
        /// The id.
        id: Id,
    },
    /// The operation would break an invariant.
    #[error("{0}")]
    Invalid(String),
}
