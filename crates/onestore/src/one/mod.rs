//! [MS-ONE]: the OneNote content stored in a revision store — sections, page series, pages,
//! titles, outlines, outline elements and rich text — read into NoteCraft's document model.
//!
//! The structure walked (all object references through the property sets of [`crate::propset`]):
//!
//! ```text
//! root object space  ─ default content root: jcidSectionNode (§2.2.17)
//!   ElementChildNodes → jcidPageSeriesNode (§2.2.18)
//!     ChildGraphSpaceElementNodes → page object spaces (§2.1.10), in page order
//! page object space  ─ default content root: jcidPageManifestNode (§2.2.34)
//!                    ─ metadata root: jcidPageMetaData (§2.2.30): CachedTitleString, PageLevel
//!   ContentChildNodes → jcidPageNode (§2.2.19)
//!     StructureElementChildNodes → jcidTitleNode (§2.2.29) → outline with IsTitleText (§2.1.16)
//!     ElementChildNodes → jcidOutlineNode (§2.2.20) → jcidOutlineElementNode (§2.2.21)
//!       ContentChildNodes → jcidRichTextOENode (§2.2.23): RichEditTextUnicode / TextExtendedAscii
//!       ElementChildNodes → child outline elements (one level deeper)
//! ```

mod map;

pub use map::{Import, Report, read_section};

/// [MS-ONE] §2.1.13 JCIDs of the objects read.
pub mod jcid {
    #![allow(missing_docs)]
    pub const SECTION: u32 = 0x0006_0007;
    pub const PAGE_SERIES: u32 = 0x0006_0008;
    pub const PAGE: u32 = 0x0006_000B;
    pub const OUTLINE: u32 = 0x0006_000C;
    pub const OUTLINE_ELEMENT: u32 = 0x0006_000D;
    pub const RICH_TEXT_OE: u32 = 0x0006_000E;
    pub const IMAGE: u32 = 0x0006_0011;
    pub const NUMBER_LIST: u32 = 0x0006_0012;
    pub const OUTLINE_GROUP: u32 = 0x0006_0019;
    pub const TABLE: u32 = 0x0006_0022;
    pub const TITLE: u32 = 0x0006_002C;
    pub const PAGE_METADATA: u32 = 0x0002_0030;
    pub const EMBEDDED_FILE: u32 = 0x0006_0035;
    pub const PAGE_MANIFEST: u32 = 0x0006_0037;
}

/// [MS-ONE] §2.1.12 PropertyIDs of the properties read (with their type bits, as the spec
/// lists them; lookups compare the 26-bit id only).
pub mod prop {
    #![allow(missing_docs)]
    pub const OFFSET_FROM_PARENT_HORIZ: u32 = 0x1400_1C14;
    pub const OFFSET_FROM_PARENT_VERT: u32 = 0x1400_1C15;
    pub const LAYOUT_MAX_WIDTH: u32 = 0x1400_1C1B;
    pub const CONTENT_CHILD_NODES: u32 = 0x2400_1C1F;
    pub const ELEMENT_CHILD_NODES: u32 = 0x2400_1C20;
    pub const RICH_EDIT_TEXT_UNICODE: u32 = 0x1C00_1C22;
    pub const TOPOLOGY_CREATION_TIME_STAMP: u32 = 0x1800_1C65;
    pub const IS_TITLE_TEXT: u32 = 0x0800_1CB4;
    pub const IS_TITLE_DATE: u32 = 0x0800_1CB5;
    pub const IS_LAYOUT_SIZE_SET_BY_USER: u32 = 0x0800_1CBD;
    pub const CACHED_TITLE_STRING: u32 = 0x1C00_1CF3;
    pub const PAGE_MARGIN_ORIGIN_X: u32 = 0x1400_1D0F;
    pub const PAGE_MARGIN_ORIGIN_Y: u32 = 0x1400_1D10;
    pub const STRUCTURE_ELEMENT_CHILD_NODES: u32 = 0x2400_1D5F;
    pub const CHILD_GRAPH_SPACE_ELEMENT_NODES: u32 = 0x2C00_1D63;
    pub const AUTHOR: u32 = 0x1C00_1D75;
    pub const LAST_MODIFIED_TIME: u32 = 0x1400_1D7A;
    pub const PAGE_LEVEL: u32 = 0x1400_1DFF;
    pub const TEXT_EXTENDED_ASCII: u32 = 0x1C00_3498;
}

#[cfg(test)]
mod tests;
