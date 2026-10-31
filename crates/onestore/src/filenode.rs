//! [MS-ONESTORE] §2.4.3 FileNode and the §2.5 node types the reader needs.
//!
//! A file node is a 32-bit header (`FileNodeID` 10 bits, `Size` 13 bits, `StpFormat` 2 bits,
//! `CbFormat` 2 bits, `BaseType` 4 bits, `Reserved` 1 bit) and a body of `Size - 4` bytes. Nodes
//! of base type 1 and 2 start their body with a FileNodeChunkReference (§2.2.4.2) to data or to
//! another file node list. Node types the reader does not use are kept as [`NodeData::Other`]
//! and skipped by their `Size`, so new node types in newer files never stop a read.

use crate::bytes::Cursor;
use crate::error::{OneError, Result};
use crate::types::{ChunkRef, CompactId, ExtendedGuid, Guid, read_string};

/// §2.4.3 FileNodeID values (the subset the reader decodes; table in §2.4.3).
pub mod id {
    #![allow(missing_docs)]
    pub const OBJECT_SPACE_MANIFEST_ROOT: u16 = 0x004;
    pub const OBJECT_SPACE_MANIFEST_LIST_REFERENCE: u16 = 0x008;
    pub const OBJECT_SPACE_MANIFEST_LIST_START: u16 = 0x00C;
    pub const REVISION_MANIFEST_LIST_REFERENCE: u16 = 0x010;
    pub const REVISION_MANIFEST_LIST_START: u16 = 0x014;
    pub const REVISION_MANIFEST_START4: u16 = 0x01B;
    pub const REVISION_MANIFEST_END: u16 = 0x01C;
    pub const REVISION_MANIFEST_START6: u16 = 0x01E;
    pub const REVISION_MANIFEST_START7: u16 = 0x01F;
    pub const GLOBAL_ID_TABLE_START: u16 = 0x021;
    pub const GLOBAL_ID_TABLE_START2: u16 = 0x022;
    pub const GLOBAL_ID_TABLE_ENTRY: u16 = 0x024;
    pub const GLOBAL_ID_TABLE_ENTRY2: u16 = 0x025;
    pub const GLOBAL_ID_TABLE_ENTRY3: u16 = 0x026;
    pub const GLOBAL_ID_TABLE_END: u16 = 0x028;
    pub const OBJECT_DECLARATION_WITH_REF_COUNT: u16 = 0x02D;
    pub const OBJECT_DECLARATION_WITH_REF_COUNT2: u16 = 0x02E;
    pub const OBJECT_REVISION_WITH_REF_COUNT: u16 = 0x041;
    pub const OBJECT_REVISION_WITH_REF_COUNT2: u16 = 0x042;
    pub const ROOT_OBJECT_REFERENCE2: u16 = 0x059;
    pub const ROOT_OBJECT_REFERENCE3: u16 = 0x05A;
    pub const REVISION_ROLE_DECLARATION: u16 = 0x05C;
    pub const REVISION_ROLE_AND_CONTEXT_DECLARATION: u16 = 0x05D;
    pub const OBJECT_DECLARATION_FILE_DATA3_REF_COUNT: u16 = 0x072;
    pub const OBJECT_DECLARATION_FILE_DATA3_LARGE_REF_COUNT: u16 = 0x073;
    pub const OBJECT_DATA_ENCRYPTION_KEY_V2: u16 = 0x07C;
    pub const FILE_DATA_STORE_LIST_REFERENCE: u16 = 0x090;
    pub const FILE_DATA_STORE_OBJECT_REFERENCE: u16 = 0x094;
    pub const OBJECT_DECLARATION2_REF_COUNT: u16 = 0x0A4;
    pub const OBJECT_DECLARATION2_LARGE_REF_COUNT: u16 = 0x0A5;
    pub const OBJECT_GROUP_LIST_REFERENCE: u16 = 0x0B0;
    pub const OBJECT_GROUP_START: u16 = 0x0B4;
    pub const OBJECT_GROUP_END: u16 = 0x0B8;
    pub const READ_ONLY_OBJECT_DECLARATION2_REF_COUNT: u16 = 0x0C4;
    pub const READ_ONLY_OBJECT_DECLARATION2_LARGE_REF_COUNT: u16 = 0x0C5;
    pub const CHUNK_TERMINATOR: u16 = 0x0FF;
}

/// [MS-ONESTORE] §2.6.14 JCID: the type of an object ([MS-ONE] §2.1.13 lists the values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Jcid(pub u32);

impl Jcid {
    /// The 16-bit `index` that names the type.
    pub fn index(self) -> u16 {
        (self.0 & 0xFFFF) as u16
    }

    /// `IsBinary`: the object's data is not a property set.
    pub fn is_binary(self) -> bool {
        self.0 & (1 << 16) != 0
    }

    /// `IsPropertySet`: the object's data is an ObjectSpaceObjectPropSet (§2.6.1).
    pub fn is_property_set(self) -> bool {
        self.0 & (1 << 17) != 0
    }

    /// `IsFileData`: the object is a file data object (§2.6.13).
    pub fn is_file_data(self) -> bool {
        self.0 & (1 << 19) != 0
    }
}

/// A root object: by compact id (§2.5.15 RootObjectReference2FNDX, `.onetoc2`) or by extended
/// GUID (§2.5.16 RootObjectReference3FND, `.one`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootRef {
    /// Resolved through the revision's global identification table.
    Compact(CompactId),
    /// Already an extended GUID.
    Extended(ExtendedGuid),
}

/// A decoded file node body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeData {
    /// §2.5.1 ObjectSpaceManifestRootFND: the root object space.
    ObjectSpaceManifestRoot {
        /// `gosidRoot`.
        root: ExtendedGuid,
    },
    /// §2.5.2 ObjectSpaceManifestListReferenceFND.
    ObjectSpaceManifestListReference {
        /// The object space manifest list.
        list: ChunkRef,
        /// `gosid`: the object space's id.
        gosid: ExtendedGuid,
    },
    /// §2.5.3 ObjectSpaceManifestListStartFND.
    ObjectSpaceManifestListStart {
        /// `gosid`.
        gosid: ExtendedGuid,
    },
    /// §2.5.4 RevisionManifestListReferenceFND.
    RevisionManifestListReference {
        /// The revision manifest list.
        list: ChunkRef,
    },
    /// §2.5.5 RevisionManifestListStartFND.
    RevisionManifestListStart {
        /// `gosid`.
        gosid: ExtendedGuid,
    },
    /// §2.5.6 RevisionManifestStart4FND, §2.5.7 Start6FND, §2.5.8 Start7FND.
    RevisionManifestStart {
        /// `rid`: this revision.
        rid: ExtendedGuid,
        /// `ridDependent`: the revision this one adds to (nil: none).
        dependent: ExtendedGuid,
        /// `RevisionRole`.
        role: u32,
        /// `gctxid` (Start7 only; nil otherwise): the context.
        context: ExtendedGuid,
    },
    /// RevisionManifestEndFND (§2.4.3, no data).
    RevisionManifestEnd,
    /// §2.5.9 GlobalIdTableStartFNDX / GlobalIdTableStart2FND (§2.4.3, no data).
    GlobalIdTableStart,
    /// §2.5.10 GlobalIdTableEntryFNDX.
    GlobalIdTableEntry {
        /// `index`.
        index: u32,
        /// `guid`.
        guid: Guid,
    },
    /// §2.5.11 GlobalIdTableEntry2FNDX: copies one entry of the dependency's table.
    GlobalIdTableEntry2 {
        /// `iIndexMapFrom`.
        from: u32,
        /// `iIndexMapTo`.
        to: u32,
    },
    /// §2.5.12 GlobalIdTableEntry3FNDX: copies a range of the dependency's table.
    GlobalIdTableEntry3 {
        /// `iIndexCopyFromStart`.
        from: u32,
        /// `cEntriesToCopy`.
        count: u32,
        /// `iIndexCopyToStart`.
        to: u32,
    },
    /// GlobalIdTableEndFNDX (§2.4.3, no data).
    GlobalIdTableEnd,
    /// An object declared or revised with its data: §2.5.23 ObjectDeclarationWithRefCountFNDX,
    /// §2.5.24 …2FNDX, §2.5.13 ObjectRevisionWithRefCountFNDX, §2.5.14 …2FNDX, §2.5.25
    /// ObjectDeclaration2RefCountFND, §2.5.26 …LargeRefCountFND, §2.5.29
    /// ReadOnlyObjectDeclaration2RefCountFND, §2.5.30 …LargeRefCountFND.
    Object {
        /// The object's data (an ObjectSpaceObjectPropSet for property-set objects).
        data: ChunkRef,
        /// `oid`.
        oid: CompactId,
        /// `jcid` (a revision node keeps the type of the original declaration: zero here).
        jcid: Jcid,
        /// True for an ObjectRevision node, which replaces an earlier declaration's data.
        revision: bool,
    },
    /// §2.5.27 ObjectDeclarationFileData3RefCountFND, §2.5.28 …LargeRefCountFND.
    FileDataObject {
        /// `oid`.
        oid: CompactId,
        /// `jcid`.
        jcid: Jcid,
        /// `FileDataReference`: `<ifndf>{GUID}</ifndf>` (a file data store object) or
        /// `<file>name` (a file beside the section).
        reference: String,
        /// `Extension`, with its dot.
        extension: String,
    },
    /// §2.5.15/§2.5.16 RootObjectReference2FNDX / 3FND.
    RootObject {
        /// The root object.
        root: RootRef,
        /// `RootRole`.
        role: u32,
    },
    /// §2.5.17 RevisionRoleDeclarationFND / §2.5.18 RevisionRoleAndContextDeclarationFND.
    RevisionRole {
        /// `rid`.
        rid: ExtendedGuid,
        /// `RevisionRole`.
        role: u32,
        /// `gctxid` (nil for the default context).
        context: ExtendedGuid,
    },
    /// §2.5.31 ObjectGroupListReferenceFND.
    ObjectGroupListReference {
        /// The object group's file node list.
        list: ChunkRef,
        /// `ObjectGroupID`.
        group: ExtendedGuid,
    },
    /// §2.5.32 ObjectGroupStartFND.
    ObjectGroupStart {
        /// `oid`: the group.
        group: ExtendedGuid,
    },
    /// ObjectGroupEndFND (§2.4.3, no data).
    ObjectGroupEnd,
    /// §2.5.21 FileDataStoreListReferenceFND.
    FileDataStoreListReference {
        /// The file data store list.
        list: ChunkRef,
    },
    /// §2.5.22 FileDataStoreObjectReferenceFND.
    FileDataStoreObjectReference {
        /// The FileDataStoreObject (§2.6.13).
        data: ChunkRef,
        /// `guidReference`, named by `<ifndf>` references.
        guid: Guid,
    },
    /// §2.5.19 ObjectDataEncryptionKeyV2FNDX: the object space is encrypted.
    EncryptionKey,
    /// §2.4.3 ChunkTerminatorFND: the rest of the fragment is unused.
    ChunkTerminator,
    /// Any other node, skipped by its size.
    Other,
}

/// One file node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileNode {
    /// `FileNodeID`.
    pub id: u16,
    /// Absolute offset of the node's header.
    pub at: u64,
    /// The decoded body.
    pub data: NodeData,
}

/// The split 32-bit FileNode header (§2.4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NodeHeader {
    pub id: u16,
    pub size: u16,
    pub stp_format: u8,
    pub cb_format: u8,
    pub base_type: u8,
}

impl NodeHeader {
    pub(crate) fn from_u32(v: u32) -> Self {
        Self {
            id: (v & 0x3FF) as u16,
            size: ((v >> 10) & 0x1FFF) as u16,
            stp_format: ((v >> 23) & 3) as u8,
            cb_format: ((v >> 25) & 3) as u8,
            base_type: ((v >> 27) & 0xF) as u8,
        }
    }

    /// The header as stored, `Reserved` = 1 (§2.4.3).
    #[cfg(any(test, feature = "fixtures"))]
    pub(crate) fn to_u32(self) -> u32 {
        u32::from(self.id & 0x3FF)
            | (u32::from(self.size & 0x1FFF) << 10)
            | (u32::from(self.stp_format & 3) << 23)
            | (u32::from(self.cb_format & 3) << 25)
            | (u32::from(self.base_type & 0xF) << 27)
            | (1 << 31)
    }
}

/// Reads the node whose header was just read at `at`; `c` holds exactly its body.
pub(crate) fn decode(h: NodeHeader, at: u64, c: &mut Cursor<'_>) -> Result<FileNode> {
    use id::*;
    let chunk = |c: &mut Cursor<'_>| -> Result<ChunkRef> {
        if h.base_type == 1 || h.base_type == 2 {
            ChunkRef::read_node(c, h.stp_format, h.cb_format, "FileNodeChunkReference")
        } else {
            Err(OneError::corrupt(at, format!("file node {:#05X} should reference data (BaseType {})", h.id, h.base_type)))
        }
    };
    let xg = |c: &mut Cursor<'_>, what: &str| ExtendedGuid::read(c, what);
    let data = match h.id {
        OBJECT_SPACE_MANIFEST_ROOT => NodeData::ObjectSpaceManifestRoot { root: xg(c, "gosidRoot")? },
        OBJECT_SPACE_MANIFEST_LIST_REFERENCE => NodeData::ObjectSpaceManifestListReference { list: chunk(c)?, gosid: xg(c, "gosid")? },
        OBJECT_SPACE_MANIFEST_LIST_START => NodeData::ObjectSpaceManifestListStart { gosid: xg(c, "gosid")? },
        REVISION_MANIFEST_LIST_REFERENCE => NodeData::RevisionManifestListReference { list: chunk(c)? },
        REVISION_MANIFEST_LIST_START => NodeData::RevisionManifestListStart { gosid: xg(c, "gosid")? },
        REVISION_MANIFEST_START4 => {
            let rid = xg(c, "rid")?;
            let dependent = xg(c, "ridDependent")?;
            c.skip(8, "timeCreation")?;
            let role = c.u32("RevisionRole")?;
            NodeData::RevisionManifestStart { rid, dependent, role, context: ExtendedGuid::NIL }
        }
        REVISION_MANIFEST_START6 | REVISION_MANIFEST_START7 => {
            let rid = xg(c, "rid")?;
            let dependent = xg(c, "ridDependent")?;
            let role = c.u32("RevisionRole")?;
            c.skip(2, "odcsDefault")?;
            let context = if h.id == REVISION_MANIFEST_START7 { xg(c, "gctxid")? } else { ExtendedGuid::NIL };
            NodeData::RevisionManifestStart { rid, dependent, role, context }
        }
        REVISION_MANIFEST_END => NodeData::RevisionManifestEnd,
        GLOBAL_ID_TABLE_START | GLOBAL_ID_TABLE_START2 => NodeData::GlobalIdTableStart,
        GLOBAL_ID_TABLE_ENTRY => NodeData::GlobalIdTableEntry { index: c.u32("index")?, guid: Guid::read(c, "guid")? },
        GLOBAL_ID_TABLE_ENTRY2 => NodeData::GlobalIdTableEntry2 { from: c.u32("iIndexMapFrom")?, to: c.u32("iIndexMapTo")? },
        GLOBAL_ID_TABLE_ENTRY3 => {
            NodeData::GlobalIdTableEntry3 { from: c.u32("iIndexCopyFromStart")?, count: c.u32("cEntriesToCopy")?, to: c.u32("iIndexCopyToStart")? }
        }
        GLOBAL_ID_TABLE_END => NodeData::GlobalIdTableEnd,
        OBJECT_DECLARATION_WITH_REF_COUNT | OBJECT_DECLARATION_WITH_REF_COUNT2 => {
            let data = chunk(c)?;
            // §2.6.15 ObjectDeclarationWithRefCountBody: oid, then jci (10 bits), odc (4 bits),
            // two reserved fields. `jci` is the JCID's index (MUST be 0x01) and the object a property set.
            let oid = CompactId::read(c, "oid")?;
            let bits = c.u16("jci")?;
            NodeData::Object { data, oid, jcid: Jcid(0x0002_0000 | u32::from(bits & 0x3FF)), revision: false }
        }
        OBJECT_REVISION_WITH_REF_COUNT | OBJECT_REVISION_WITH_REF_COUNT2 => {
            NodeData::Object { data: chunk(c)?, oid: CompactId::read(c, "oid")?, jcid: Jcid(0), revision: true }
        }
        OBJECT_DECLARATION2_REF_COUNT
        | OBJECT_DECLARATION2_LARGE_REF_COUNT
        | READ_ONLY_OBJECT_DECLARATION2_REF_COUNT
        | READ_ONLY_OBJECT_DECLARATION2_LARGE_REF_COUNT => {
            // §2.6.16 ObjectDeclaration2Body: oid, jcid, then flags and the reference count.
            let data = chunk(c)?;
            NodeData::Object { data, oid: CompactId::read(c, "oid")?, jcid: Jcid(c.u32("jcid")?), revision: false }
        }
        OBJECT_DECLARATION_FILE_DATA3_REF_COUNT | OBJECT_DECLARATION_FILE_DATA3_LARGE_REF_COUNT => {
            let oid = CompactId::read(c, "oid")?;
            let jcid = Jcid(c.u32("jcid")?);
            c.skip(if h.id == OBJECT_DECLARATION_FILE_DATA3_REF_COUNT { 1 } else { 4 }, "cRef")?;
            let reference = read_string(c, "FileDataReference")?;
            let extension = read_string(c, "Extension")?;
            NodeData::FileDataObject { oid, jcid, reference, extension }
        }
        ROOT_OBJECT_REFERENCE2 => NodeData::RootObject { root: RootRef::Compact(CompactId::read(c, "oidRoot")?), role: c.u32("RootRole")? },
        ROOT_OBJECT_REFERENCE3 => NodeData::RootObject { root: RootRef::Extended(xg(c, "oidRoot")?), role: c.u32("RootRole")? },
        REVISION_ROLE_DECLARATION | REVISION_ROLE_AND_CONTEXT_DECLARATION => {
            let rid = xg(c, "rid")?;
            let role = c.u32("RevisionRole")?;
            let context = if h.id == REVISION_ROLE_AND_CONTEXT_DECLARATION { xg(c, "gctxid")? } else { ExtendedGuid::NIL };
            NodeData::RevisionRole { rid, role, context }
        }
        OBJECT_GROUP_LIST_REFERENCE => NodeData::ObjectGroupListReference { list: chunk(c)?, group: xg(c, "ObjectGroupID")? },
        OBJECT_GROUP_START => NodeData::ObjectGroupStart { group: xg(c, "oid")? },
        OBJECT_GROUP_END => NodeData::ObjectGroupEnd,
        FILE_DATA_STORE_LIST_REFERENCE => NodeData::FileDataStoreListReference { list: chunk(c)? },
        FILE_DATA_STORE_OBJECT_REFERENCE => NodeData::FileDataStoreObjectReference { data: chunk(c)?, guid: Guid::read(c, "guidReference")? },
        OBJECT_DATA_ENCRYPTION_KEY_V2 => NodeData::EncryptionKey,
        CHUNK_TERMINATOR => NodeData::ChunkTerminator,
        _ => NodeData::Other,
    };
    Ok(FileNode { id: h.id, at, data })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_bits_round_trip() {
        let h = NodeHeader { id: 0x0A4, size: 0x1234, stp_format: 2, cb_format: 3, base_type: 1 };
        let v = h.to_u32();
        assert_eq!(v >> 31, 1, "Reserved MUST be 1");
        assert_eq!(NodeHeader::from_u32(v), h);
    }

    #[test]
    fn a_node_that_needs_a_reference_refuses_base_type_zero() {
        let h = NodeHeader { id: id::REVISION_MANIFEST_LIST_REFERENCE, size: 16, stp_format: 0, cb_format: 0, base_type: 0 };
        let body = [0u8; 12];
        assert!(decode(h, 0, &mut Cursor::new(&body, 4)).is_err());
    }

    #[test]
    fn unknown_nodes_are_kept_as_other() {
        let h = NodeHeader { id: 0x3FE, size: 4, stp_format: 0, cb_format: 0, base_type: 0 };
        assert_eq!(decode(h, 8, &mut Cursor::new(&[], 12)).unwrap().data, NodeData::Other);
    }

    #[test]
    fn jcid_flags() {
        let page = Jcid(0x0006_000B); // [MS-ONE] jcidPageNode
        assert_eq!(page.index(), 0x0B);
        assert!(page.is_property_set() && !page.is_binary() && !page.is_file_data());
    }
}
