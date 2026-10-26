//! NoteCraft's reader of OneNote's own files (layer **L4**, interop; decision D12,
//! `docs/plan.md` §4.17): `.one` sections and `.onetoc2` notebook tables of contents in the
//! revision store format of the public Open Specification **[MS-ONESTORE]**, whose objects carry
//! the page content described by **[MS-ONE]**.
//!
//! It sits at L4 because it turns foreign bytes into the document model (L1) and nothing above
//! the engine needs to know the format; it has no UI or platform dependencies and works on a
//! `&[u8]`, so it builds for wasm.
//!
//! **Clean room.** Written from the public specifications only: every structure, field and
//! validation rule cites its section (`[MS-ONESTORE] §2.4.1`). No Microsoft code, binaries or
//! files were read; the tests use synthetic files made by this crate's own test writer
//! ([`writer`], test builds and the `fixtures` feature only).
//!
//! **Every byte is hostile.** All reads are bounds-checked ([`bytes`]), every chunk reference is
//! checked to lie inside the file before use, fragment chains and object graphs are walked with
//! visited sets (they can be forged into loops), and the totals are capped by [`Limits`]. A
//! damaged, truncated or forged file gives a [`OneError`], never a panic or an unbounded
//! allocation. The reader is **read-only**: it never writes the user's file.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![deny(clippy::indexing_slicing)]

pub(crate) mod bytes;
pub mod error;
pub mod filenode;
pub mod header;
pub mod nodelist;
pub mod one;
pub mod propset;
pub mod revision;
pub mod txlog;
pub mod types;
#[cfg(any(test, feature = "fixtures"))]
pub mod writer;

pub use error::{OneError, Result};
pub use header::{FileKind, Header};
pub use one::{Import, Report, read_section};
pub use revision::{Object, ObjectData, ObjectSpace, Store};
pub use types::{ChunkRef, CompactId, ExtendedGuid, Guid};

/// The reader's caps. Every one bounds work or memory that a forged file could otherwise make
/// unbounded; going over one is [`OneError::TooLarge`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// The largest file read (2 GiB: the largest section the reference writes is far smaller).
    pub max_file_bytes: u64,
    /// File node list and transaction log fragments, in total.
    pub max_fragments: usize,
    /// File nodes, in total (about 64 bytes of memory each).
    pub max_nodes: usize,
    /// Depth of object references and nested property sets followed.
    pub max_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_file_bytes: 2 << 30, max_fragments: 1_000_000, max_nodes: 2_000_000, max_depth: 64 }
    }
}
