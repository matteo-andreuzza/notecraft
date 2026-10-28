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
    /// jcidParagraphStyleObject and jcidParagraphStyleObjectForText (§2.2.43, §2.2.44).
    pub const PARAGRAPH_STYLE: u32 = 0x0012_004D;
    pub const TABLE_ROW: u32 = 0x0006_0023;
    pub const TABLE_CELL: u32 = 0x0006_0024;
    /// File data objects (§2.2.33, §2.2.36).
    pub const EMBEDDED_FILE_CONTAINER: u32 = 0x0008_0036;
    pub const PICTURE_CONTAINER: u32 = 0x0008_0039;
    pub const NOTE_TAG_DEFINITION: u32 = 0x0012_0043;
}

/// [MS-ONE] §2.1.12 PropertyIDs of the properties read (with their type bits, as the spec
/// lists them; lookups compare the 26-bit id only).
pub mod prop {
    #![allow(missing_docs)]
    pub const BOLD: u32 = 0x0800_1C04;
    pub const ITALIC: u32 = 0x0800_1C05;
    pub const UNDERLINE: u32 = 0x0800_1C06;
    pub const STRIKETHROUGH: u32 = 0x0800_1C07;
    pub const SUPERSCRIPT: u32 = 0x0800_1C08;
    pub const SUBSCRIPT: u32 = 0x0800_1C09;
    pub const FONT: u32 = 0x1C00_1C0A;
    pub const FONT_SIZE: u32 = 0x1000_1C0B;
    pub const FONT_COLOR: u32 = 0x1400_1C0C;
    pub const HIGHLIGHT: u32 = 0x1400_1C0D;
    pub const TEXT_RUN_INDEX: u32 = 0x1C00_1E12;
    pub const TEXT_RUN_FORMATTING: u32 = 0x2400_1E13;
    pub const HYPERLINK: u32 = 0x0800_1E14;
    pub const HIDDEN: u32 = 0x0800_1E16;
    pub const PARAGRAPH_STYLE: u32 = 0x2000_342C;
    pub const PARAGRAPH_STYLE_ID: u32 = 0x1C00_345A;
    pub const PARAGRAPH_ALIGNMENT: u32 = 0x0C00_3477;
    pub const NUMBER_LIST_FORMAT: u32 = 0x1C00_1C1A;
    pub const LIST_NODES: u32 = 0x2400_1C26;
    pub const LIST_RESTART: u32 = 0x1400_1CB7;
    pub const NOTE_TAG_SHAPE: u32 = 0x1000_3464;
    pub const NOTE_TAG_LABEL: u32 = 0x1C00_3468;
    pub const NOTE_TAG_DEFINITION_OID: u32 = 0x2000_3488;
    pub const NOTE_TAG_STATES: u32 = 0x0400_3489;
    pub const ACTION_ITEM_STATUS: u32 = 0x1000_3470;
    pub const LAYOUT_MAX_HEIGHT: u32 = 0x1400_1C1C;
    pub const PICTURE_CONTAINER: u32 = 0x2000_1C3F;
    pub const ROW_COUNT: u32 = 0x1400_1D57;
    pub const COLUMN_COUNT: u32 = 0x1400_1D58;
    pub const TABLE_BORDERS_VISIBLE: u32 = 0x0800_1D5E;
    pub const TABLE_COLUMN_WIDTHS: u32 = 0x1C00_1D66;
    pub const CELL_SHADING_COLOR: u32 = 0x1400_1E26;
    pub const EMBEDDED_FILE_CONTAINER: u32 = 0x2000_1D9B;
    pub const EMBEDDED_FILE_NAME: u32 = 0x1C00_1D9C;
    pub const IMAGE_FILENAME: u32 = 0x1C00_1DD7;
    pub const WZ_HYPERLINK_URL: u32 = 0x1C00_1E20;
    pub const IMAGE_ALT_TEXT: u32 = 0x1C00_1E58;
    pub const PICTURE_WIDTH: u32 = 0x1400_34CD;
    pub const PICTURE_HEIGHT: u32 = 0x1400_34CE;
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
