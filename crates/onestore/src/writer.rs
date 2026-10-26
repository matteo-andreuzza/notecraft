//! The minimal revision-store writer that makes **synthetic** test files (clean room: the
//! tests never need a file written by Microsoft software). Built only for tests and with the
//! `fixtures` feature; not a user-facing `.one` export (decision D12: read-only in M3).
//!
//! The builder appends blocks to a buffer that starts with a 1,024-byte header, records one
//! transaction-log entry per list it writes, and fills the header in [`FileBuilder::finish`].
#![allow(clippy::indexing_slicing, missing_docs)]

use crate::filenode::NodeHeader;
use crate::header::{FFV_ONE, FFV_ONETOC2, FileKind, GUID_FILE_FORMAT, GUID_FILE_TYPE_ONE, GUID_FILE_TYPE_ONETOC2, HEADER_SIZE};
use crate::nodelist::{FOOTER_MAGIC, HEADER_MAGIC};
use crate::types::{ChunkRef, Guid};

/// Builds one synthetic file.
pub struct FileBuilder {
    kind: FileKind,
    buf: Vec<u8>,
    transactions: Vec<Vec<(u32, u32)>>,
    pending: Vec<(u32, u32)>,
    next_list: u32,
}

impl FileBuilder {
    /// An empty file of `kind`.
    pub fn new(kind: FileKind) -> Self {
        Self { kind, buf: vec![0; HEADER_SIZE], transactions: Vec::new(), pending: Vec::new(), next_list: 0x10 }
    }

    /// A transaction with explicit `(FileNodeListID, node count)` entries, written before the
    /// one that commits the lists of this builder.
    pub fn transaction(&mut self, entries: &[(u32, u32)]) {
        self.transactions.push(entries.to_vec());
    }

    /// Appends `bytes` at an 8-byte boundary (compressed references count in units of 8).
    pub fn blob(&mut self, bytes: &[u8]) -> ChunkRef {
        while !self.buf.len().is_multiple_of(8) {
            self.buf.push(0);
        }
        let stp = self.buf.len() as u64;
        self.buf.extend_from_slice(bytes);
        ChunkRef::new(stp, bytes.len() as u64)
    }

    /// A file node list in one fragment, all nodes committed.
    pub fn list(&mut self, nodes: &[Vec<u8>]) -> ChunkRef {
        self.list_in_fragments(nodes, nodes.len().max(1))
    }

    /// A file node list split into fragments of `per` nodes, all committed.
    pub fn list_in_fragments(&mut self, nodes: &[Vec<u8>], per: usize) -> ChunkRef {
        self.write_list(nodes, per.max(1), nodes.len())
    }

    /// A file node list in one fragment of which only the first `committed` nodes are committed.
    pub fn list_uncommitted(&mut self, nodes: &[Vec<u8>], committed: usize) -> ChunkRef {
        self.write_list(nodes, nodes.len().max(1), committed)
    }

    fn write_list(&mut self, nodes: &[Vec<u8>], per: usize, committed: usize) -> ChunkRef {
        let id = self.next_list;
        self.next_list += 1;
        if committed > 0 {
            self.pending.push((id, committed as u32));
        }
        let chunks: Vec<&[Vec<u8>]> = if nodes.is_empty() { vec![&[]] } else { nodes.chunks(per).collect() };
        // Fragments are written last to first so each knows its successor.
        let mut next = ChunkRef::NIL;
        for (seq, chunk) in chunks.iter().enumerate().rev() {
            let mut f = Vec::new();
            f.extend_from_slice(&HEADER_MAGIC.to_le_bytes());
            f.extend_from_slice(&id.to_le_bytes());
            f.extend_from_slice(&(seq as u32).to_le_bytes());
            for n in chunk.iter() {
                f.extend_from_slice(n);
            }
            if !next.nil {
                // §2.4.1: a fragment followed by another ends with a ChunkTerminatorFND.
                f.extend_from_slice(&node::encode(crate::filenode::id::CHUNK_TERMINATOR, 0, None, &[]));
            }
            put_ref64x32(&mut f, next);
            f.extend_from_slice(&FOOTER_MAGIC.to_le_bytes());
            next = self.blob(&f);
        }
        next
    }

    /// Writes the root file node list from `root` and finishes the file.
    pub fn finish(mut self, root: &[Vec<u8>]) -> Vec<u8> {
        let root = self.list(root);
        self.finish_with_root(root)
    }

    /// Finishes the file with an already written root list: transaction log, then header.
    pub fn finish_with_root(mut self, root: ChunkRef) -> Vec<u8> {
        let mut txs = std::mem::take(&mut self.transactions);
        let pending = std::mem::take(&mut self.pending);
        if !pending.is_empty() {
            txs.push(pending);
        }
        let mut log = Vec::new();
        for tx in &txs {
            for &(id, count) in tx {
                log.extend_from_slice(&id.to_le_bytes());
                log.extend_from_slice(&count.to_le_bytes());
            }
            // Sentinel; the CRC is a SHOULD (§2.3.3.2) that the reader does not check.
            log.extend_from_slice(&1u32.to_le_bytes());
            log.extend_from_slice(&0u32.to_le_bytes());
        }
        put_ref64x32(&mut log, ChunkRef::NIL);
        let log_ref = self.blob(&log);
        let len = self.buf.len() as u64;
        let (file_type, ffv) = match self.kind {
            FileKind::Section => (GUID_FILE_TYPE_ONE, FFV_ONE),
            FileKind::Toc => (GUID_FILE_TYPE_ONETOC2, FFV_ONETOC2),
        };
        let h = &mut self.buf[..HEADER_SIZE];
        h[0..16].copy_from_slice(&file_type.0);
        h[16..32].copy_from_slice(&Guid::new(0x5EC7_1001, 1, 2, [9; 8]).0);
        h[48..64].copy_from_slice(&GUID_FILE_FORMAT.0);
        for i in 0..4 {
            h[64 + 4 * i..68 + 4 * i].copy_from_slice(&ffv.to_le_bytes());
        }
        // fcrLegacyTransactionLog and fcrLegacyFileNodeListRoot MUST be fcrNil (§2.3.1).
        h[88..92].fill(0xFF);
        h[96..100].copy_from_slice(&(txs.len() as u32).to_le_bytes());
        h[112..116].fill(0xFF);
        let mut refs = Vec::new();
        put_ref64x32(&mut refs, log_ref);
        put_ref64x32(&mut refs, root);
        h[160..184].copy_from_slice(&refs);
        h[196..204].copy_from_slice(&len.to_le_bytes());
        self.buf
    }
}

/// Appends a FileChunkReference64x32 (§2.2.4.4).
pub fn put_ref64x32(out: &mut Vec<u8>, r: ChunkRef) {
    if r.nil {
        out.extend_from_slice(&u64::MAX.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
    } else {
        out.extend_from_slice(&r.stp.to_le_bytes());
        out.extend_from_slice(&(r.cb as u32).to_le_bytes());
    }
}

/// File node encoders (§2.4.3 header + the §2.5 bodies).
pub mod node {
    use super::*;
    use crate::filenode::id;
    use crate::types::{CompactId, ExtendedGuid};

    /// A node with `StpFormat` 0 (8-byte offset) and `CbFormat` 0 (4-byte size).
    pub fn encode(id: u16, base_type: u8, chunk: Option<ChunkRef>, body: &[u8]) -> Vec<u8> {
        encode_with(id, base_type, chunk, 0, 0, body)
    }

    /// A node with the given reference formats (§2.2.4.2).
    pub fn encode_with(id: u16, base_type: u8, chunk: Option<ChunkRef>, stp_format: u8, cb_format: u8, body: &[u8]) -> Vec<u8> {
        let mut fnd = Vec::new();
        if let Some(r) = chunk {
            let (stp, cb) = if r.nil { (u64::MAX, 0) } else { (r.stp, r.cb) };
            let all_ones = r.nil;
            match stp_format {
                0 => fnd.extend_from_slice(&stp.to_le_bytes()),
                1 => fnd.extend_from_slice(&(if all_ones { u32::MAX } else { stp as u32 }).to_le_bytes()),
                2 => fnd.extend_from_slice(&(if all_ones { u16::MAX } else { (stp / 8) as u16 }).to_le_bytes()),
                _ => fnd.extend_from_slice(&(if all_ones { u32::MAX } else { (stp / 8) as u32 }).to_le_bytes()),
            }
            match cb_format {
                0 => fnd.extend_from_slice(&(cb as u32).to_le_bytes()),
                1 => fnd.extend_from_slice(&cb.to_le_bytes()),
                2 => fnd.push(cb.div_ceil(8) as u8),
                _ => fnd.extend_from_slice(&(cb.div_ceil(8) as u16).to_le_bytes()),
            }
        }
        fnd.extend_from_slice(body);
        let h = NodeHeader { id, size: (fnd.len() + 4) as u16, stp_format, cb_format, base_type };
        let mut out = h.to_u32().to_le_bytes().to_vec();
        out.extend_from_slice(&fnd);
        out
    }

    pub fn xguid(out: &mut Vec<u8>, g: ExtendedGuid) {
        out.extend_from_slice(&g.guid.0);
        out.extend_from_slice(&g.n.to_le_bytes());
    }

    pub fn compact(out: &mut Vec<u8>, c: CompactId) {
        out.extend_from_slice(&c.to_u32().to_le_bytes());
    }

    fn with_guid(id: u16, g: ExtendedGuid) -> Vec<u8> {
        let mut b = Vec::new();
        xguid(&mut b, g);
        encode(id, 0, None, &b)
    }

    pub fn object_space_manifest_root(root: ExtendedGuid) -> Vec<u8> {
        with_guid(id::OBJECT_SPACE_MANIFEST_ROOT, root)
    }

    pub fn object_space_manifest_list_reference(list: ChunkRef, gosid: ExtendedGuid) -> Vec<u8> {
        let mut b = Vec::new();
        xguid(&mut b, gosid);
        encode(id::OBJECT_SPACE_MANIFEST_LIST_REFERENCE, 2, Some(list), &b)
    }

    pub fn object_space_manifest_list_start(gosid: ExtendedGuid) -> Vec<u8> {
        with_guid(id::OBJECT_SPACE_MANIFEST_LIST_START, gosid)
    }

    pub fn revision_manifest_list_reference(list: ChunkRef) -> Vec<u8> {
        encode(id::REVISION_MANIFEST_LIST_REFERENCE, 2, Some(list), &[])
    }

    pub fn revision_manifest_list_start(gosid: ExtendedGuid) -> Vec<u8> {
        let mut b = Vec::new();
        xguid(&mut b, gosid);
        b.extend_from_slice(&0u32.to_le_bytes()); // nInstance
        encode(id::REVISION_MANIFEST_LIST_START, 0, None, &b)
    }

    /// RevisionManifestStart7FND with a context, Start6FND without (§2.5.7, §2.5.8).
    pub fn revision_manifest_start(rid: ExtendedGuid, dependent: ExtendedGuid, role: u32, context: Option<ExtendedGuid>) -> Vec<u8> {
        let mut b = Vec::new();
        xguid(&mut b, rid);
        xguid(&mut b, dependent);
        b.extend_from_slice(&role.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes()); // odcsDefault: not encrypted
        match context {
            Some(c) => {
                xguid(&mut b, c);
                encode(id::REVISION_MANIFEST_START7, 0, None, &b)
            }
            None => encode(id::REVISION_MANIFEST_START6, 0, None, &b),
        }
    }

    pub fn revision_manifest_end() -> Vec<u8> {
        encode(id::REVISION_MANIFEST_END, 0, None, &[])
    }

    pub fn global_id_table_start() -> Vec<u8> {
        encode(id::GLOBAL_ID_TABLE_START2, 0, None, &[])
    }

    pub fn global_id_table_entry(index: u32, guid: Guid) -> Vec<u8> {
        let mut b = index.to_le_bytes().to_vec();
        b.extend_from_slice(&guid.0);
        encode(id::GLOBAL_ID_TABLE_ENTRY, 0, None, &b)
    }

    pub fn global_id_table_end() -> Vec<u8> {
        encode(id::GLOBAL_ID_TABLE_END, 0, None, &[])
    }

    /// ObjectDeclaration2RefCountFND (§2.5.25) with an ObjectDeclaration2Body (§2.6.16).
    pub fn object_declaration(data: ChunkRef, oid: CompactId, jcid: u32, has_oids: bool, has_osids: bool) -> Vec<u8> {
        let mut b = Vec::new();
        compact(&mut b, oid);
        b.extend_from_slice(&jcid.to_le_bytes());
        b.push(u8::from(has_oids) | (u8::from(has_osids) << 1));
        b.push(1); // cRef
        encode(id::OBJECT_DECLARATION2_REF_COUNT, 1, Some(data), &b)
    }

    /// ObjectDeclarationFileData3RefCountFND (§2.5.27).
    pub fn file_data_object(oid: CompactId, jcid: u32, reference: &str, extension: &str) -> Vec<u8> {
        let mut b = Vec::new();
        compact(&mut b, oid);
        b.extend_from_slice(&jcid.to_le_bytes());
        b.push(1); // cRef
        for s in [reference, extension] {
            let units: Vec<u16> = s.encode_utf16().collect();
            b.extend_from_slice(&(units.len() as u32).to_le_bytes());
            for u in units {
                b.extend_from_slice(&u.to_le_bytes());
            }
        }
        encode(id::OBJECT_DECLARATION_FILE_DATA3_REF_COUNT, 0, None, &b)
    }

    /// RootObjectReference3FND (§2.5.16).
    pub fn root_object(root: ExtendedGuid, role: u32) -> Vec<u8> {
        let mut b = Vec::new();
        xguid(&mut b, root);
        b.extend_from_slice(&role.to_le_bytes());
        encode(id::ROOT_OBJECT_REFERENCE3, 0, None, &b)
    }

    /// RevisionRoleDeclarationFND (§2.5.17).
    pub fn revision_role(rid: ExtendedGuid, role: u32) -> Vec<u8> {
        let mut b = Vec::new();
        xguid(&mut b, rid);
        b.extend_from_slice(&role.to_le_bytes());
        encode(id::REVISION_ROLE_DECLARATION, 0, None, &b)
    }

    pub fn object_group_list_reference(list: ChunkRef, group: ExtendedGuid) -> Vec<u8> {
        let mut b = Vec::new();
        xguid(&mut b, group);
        encode(id::OBJECT_GROUP_LIST_REFERENCE, 2, Some(list), &b)
    }

    pub fn object_group_start(group: ExtendedGuid) -> Vec<u8> {
        with_guid(id::OBJECT_GROUP_START, group)
    }

    pub fn object_group_end() -> Vec<u8> {
        encode(id::OBJECT_GROUP_END, 0, None, &[])
    }

    pub fn file_data_store_list_reference(list: ChunkRef) -> Vec<u8> {
        encode(id::FILE_DATA_STORE_LIST_REFERENCE, 2, Some(list), &[])
    }

    pub fn file_data_store_object_reference(data: ChunkRef, guid: Guid) -> Vec<u8> {
        encode(id::FILE_DATA_STORE_OBJECT_REFERENCE, 1, Some(data), &guid.0)
    }

    pub fn encryption_key(data: ChunkRef) -> Vec<u8> {
        encode(id::OBJECT_DATA_ENCRYPTION_KEY_V2, 1, Some(data), &[])
    }
}

/// Property set encoders (§2.6.1 ObjectSpaceObjectPropSet, §2.6.7 PropertySet).
pub mod props {
    use crate::types::CompactId;

    /// A value to encode; its §2.6.6 `type` comes from the variant.
    #[derive(Debug, Clone)]
    pub enum Val {
        NoData,
        Bool(bool),
        U8(u8),
        U16(u16),
        U32(u32),
        U64(u64),
        Bytes(Vec<u8>),
        Object(CompactId),
        Objects(Vec<CompactId>),
        Space(CompactId),
        Spaces(Vec<CompactId>),
        Array(Vec<PropSetBuilder>),
        Set(PropSetBuilder),
    }

    /// A property set under construction. Property ids are given with their [MS-ONE] type
    /// bits; the encoder replaces the type with the variant's and sets `boolValue`.
    #[derive(Debug, Clone, Default)]
    pub struct PropSetBuilder {
        props: Vec<(u32, Val)>,
    }

    impl PropSetBuilder {
        pub fn add(&mut self, id: u32, v: Val) -> &mut Self {
            self.props.push((id, v));
            self
        }

        /// The ObjectSpaceObjectPropSet: OIDs, OSIDs (when any), body, padding to 8 bytes.
        pub fn encode(&self) -> Vec<u8> {
            let (mut oids, mut osids) = (Vec::new(), Vec::new());
            let mut body = Vec::new();
            self.write_set(&mut body, &mut oids, &mut osids);
            let mut out = Vec::new();
            let no_osids = osids.is_empty();
            out.extend_from_slice(&((oids.len() as u32) | (u32::from(no_osids) << 31)).to_le_bytes());
            for id in &oids {
                out.extend_from_slice(&id.to_u32().to_le_bytes());
            }
            if !no_osids {
                out.extend_from_slice(&(osids.len() as u32).to_le_bytes());
                for id in &osids {
                    out.extend_from_slice(&id.to_u32().to_le_bytes());
                }
            }
            out.extend_from_slice(&body);
            while !out.len().is_multiple_of(8) {
                out.push(0);
            }
            out
        }

        fn write_set(&self, out: &mut Vec<u8>, oids: &mut Vec<CompactId>, osids: &mut Vec<CompactId>) {
            out.extend_from_slice(&(self.props.len() as u16).to_le_bytes());
            for (id, v) in &self.props {
                let kind: u32 = match v {
                    Val::NoData => 0x1,
                    Val::Bool(_) => 0x2,
                    Val::U8(_) => 0x3,
                    Val::U16(_) => 0x4,
                    Val::U32(_) => 0x5,
                    Val::U64(_) => 0x6,
                    Val::Bytes(_) => 0x7,
                    Val::Object(_) => 0x8,
                    Val::Objects(_) => 0x9,
                    Val::Space(_) => 0xA,
                    Val::Spaces(_) => 0xB,
                    Val::Array(_) => 0x10,
                    Val::Set(_) => 0x11,
                };
                let b = matches!(v, Val::Bool(true));
                let prid = (id & 0x03FF_FFFF) | (kind << 26) | (u32::from(b) << 31);
                out.extend_from_slice(&prid.to_le_bytes());
            }
            for (_, v) in &self.props {
                match v {
                    Val::NoData | Val::Bool(_) => {}
                    Val::U8(x) => out.push(*x),
                    Val::U16(x) => out.extend_from_slice(&x.to_le_bytes()),
                    Val::U32(x) => out.extend_from_slice(&x.to_le_bytes()),
                    Val::U64(x) => out.extend_from_slice(&x.to_le_bytes()),
                    Val::Bytes(x) => {
                        out.extend_from_slice(&(x.len() as u32).to_le_bytes());
                        out.extend_from_slice(x);
                    }
                    Val::Object(id) => oids.push(*id),
                    Val::Objects(ids) => {
                        out.extend_from_slice(&(ids.len() as u32).to_le_bytes());
                        oids.extend_from_slice(ids);
                    }
                    Val::Space(id) => osids.push(*id),
                    Val::Spaces(ids) => {
                        out.extend_from_slice(&(ids.len() as u32).to_le_bytes());
                        osids.extend_from_slice(ids);
                    }
                    Val::Array(sets) => {
                        out.extend_from_slice(&(sets.len() as u32).to_le_bytes());
                        if !sets.is_empty() {
                            out.extend_from_slice(&(0x11u32 << 26).to_le_bytes());
                        }
                        for s in sets {
                            s.write_set(out, oids, osids);
                        }
                    }
                    Val::Set(s) => s.write_set(out, oids, osids),
                }
            }
        }
    }
}

/// Whole revision stores: object spaces, revisions and objects (§2.1).
pub mod store {
    use super::props::PropSetBuilder;
    use super::{FileBuilder, node};
    use crate::header::FileKind;
    use crate::types::{CompactId, ExtendedGuid, Guid};

    /// One revision of an object space: its objects in one object group.
    pub struct RevBuilder {
        rid: ExtendedGuid,
        dependent: ExtendedGuid,
        role: u32,
        table: Vec<Guid>,
        objects: Vec<Obj>,
        roots: Vec<(ExtendedGuid, u32)>,
        encrypted: bool,
    }

    enum Obj {
        Props { oid: CompactId, jcid: u32, data: Vec<u8> },
        File { oid: CompactId, jcid: u32, reference: String, extension: String },
    }

    impl RevBuilder {
        /// The compact id of `x` in this revision's object group table (`x.n` ≤ 255).
        pub fn cid(&mut self, x: ExtendedGuid) -> CompactId {
            let i = match self.table.iter().position(|g| *g == x.guid) {
                Some(i) => i,
                None => {
                    self.table.push(x.guid);
                    self.table.len() - 1
                }
            };
            CompactId { n: x.n as u8, guid_index: i as u32 + 1 }
        }

        /// Declares a property-set object.
        pub fn object(&mut self, oid: ExtendedGuid, jcid: u32, props: &PropSetBuilder) -> &mut Self {
            let oid = self.cid(oid);
            self.objects.push(Obj::Props { oid, jcid, data: props.encode() });
            self
        }

        /// Declares a property-set object from raw ObjectSpaceObjectPropSet bytes.
        pub fn raw_object(&mut self, oid: ExtendedGuid, jcid: u32, data: Vec<u8>) -> &mut Self {
            let oid = self.cid(oid);
            self.objects.push(Obj::Props { oid, jcid, data });
            self
        }

        /// Declares a file data object.
        pub fn file_object(&mut self, oid: ExtendedGuid, jcid: u32, reference: &str, extension: &str) -> &mut Self {
            let oid = self.cid(oid);
            self.objects.push(Obj::File { oid, jcid, reference: reference.into(), extension: extension.into() });
            self
        }

        /// Names a root object.
        pub fn root(&mut self, oid: ExtendedGuid, role: u32) -> &mut Self {
            self.roots.push((oid, role));
            self
        }

        /// Marks the revision encrypted (an ObjectDataEncryptionKeyV2FNDX after its start).
        pub fn encrypted(&mut self) -> &mut Self {
            self.encrypted = true;
            self
        }
    }

    /// One object space.
    pub struct SpaceBuilder {
        id: ExtendedGuid,
        revisions: Vec<RevBuilder>,
        extra: Vec<Vec<u8>>,
    }

    impl SpaceBuilder {
        /// A revision with role 0x1 in the default context, depending on `dependent`.
        pub fn revision(&mut self, rid: ExtendedGuid, dependent: ExtendedGuid) -> &mut RevBuilder {
            self.revision_with_role(rid, dependent, 1)
        }

        /// A revision with `role`.
        pub fn revision_with_role(&mut self, rid: ExtendedGuid, dependent: ExtendedGuid, role: u32) -> &mut RevBuilder {
            self.revisions.push(RevBuilder { rid, dependent, role, table: Vec::new(), objects: Vec::new(), roots: Vec::new(), encrypted: false });
            let last = self.revisions.len() - 1;
            &mut self.revisions[last]
        }

        /// A raw node appended to the revision manifest list after the revisions.
        pub fn extra(&mut self, node: Vec<u8>) -> &mut Self {
            self.extra.push(node);
            self
        }
    }

    /// A whole file.
    pub struct StoreBuilder {
        kind: FileKind,
        root: ExtendedGuid,
        spaces: Vec<SpaceBuilder>,
        file_data: Vec<(Guid, Vec<u8>)>,
    }

    impl StoreBuilder {
        /// A file whose root object space is `root`.
        pub fn new(kind: FileKind, root: ExtendedGuid) -> Self {
            Self { kind, root, spaces: Vec::new(), file_data: Vec::new() }
        }

        /// Adds object space `id`.
        pub fn space(&mut self, id: ExtendedGuid) -> &mut SpaceBuilder {
            self.spaces.push(SpaceBuilder { id, revisions: Vec::new(), extra: Vec::new() });
            let last = self.spaces.len() - 1;
            &mut self.spaces[last]
        }

        /// Adds a file data store object; objects name it `<ifndf>{guid}</ifndf>`.
        pub fn file_data(&mut self, guid: Guid, bytes: &[u8]) -> String {
            self.file_data.push((guid, bytes.to_vec()));
            format!("<ifndf>{guid}</ifndf>")
        }

        /// Writes the file.
        pub fn finish(self) -> Vec<u8> {
            let mut b = FileBuilder::new(self.kind);
            let mut root_nodes = Vec::new();
            for space in &self.spaces {
                let mut rml = vec![node::revision_manifest_list_start(space.id)];
                for rev in &space.revisions {
                    let mut group = vec![node::object_group_start(rev.rid), node::global_id_table_start()];
                    for (i, g) in rev.table.iter().enumerate() {
                        group.push(node::global_id_table_entry(i as u32 + 1, *g));
                    }
                    group.push(node::global_id_table_end());
                    for o in &rev.objects {
                        match o {
                            Obj::Props { oid, jcid, data } => {
                                let r = b.blob(data);
                                group.push(node::object_declaration(r, *oid, *jcid, true, true));
                            }
                            Obj::File { oid, jcid, reference, extension } => group.push(node::file_data_object(*oid, *jcid, reference, extension)),
                        }
                    }
                    group.push(node::object_group_end());
                    let group_ref = b.list(&group);
                    let context = Some(ExtendedGuid::NIL);
                    rml.push(node::revision_manifest_start(rev.rid, rev.dependent, rev.role, context));
                    if rev.encrypted {
                        let key = b.blob(&[0u8; 16]);
                        rml.push(node::encryption_key(key));
                    }
                    rml.push(node::object_group_list_reference(group_ref, rev.rid));
                    for (oid, role) in &rev.roots {
                        rml.push(node::root_object(*oid, *role));
                    }
                    rml.push(node::revision_manifest_end());
                }
                rml.extend(space.extra.iter().cloned());
                let rml_ref = b.list(&rml);
                let osml = b.list(&[node::object_space_manifest_list_start(space.id), node::revision_manifest_list_reference(rml_ref)]);
                root_nodes.push(node::object_space_manifest_list_reference(osml, space.id));
            }
            root_nodes.push(node::object_space_manifest_root(self.root));
            if !self.file_data.is_empty() {
                let mut refs = Vec::new();
                for (guid, bytes) in &self.file_data {
                    let r = b.blob(&file_data_store_object(bytes));
                    refs.push(node::file_data_store_object_reference(r, *guid));
                }
                let list = b.list(&refs);
                root_nodes.push(node::file_data_store_list_reference(list));
            }
            b.finish(&root_nodes)
        }
    }

    /// §2.6.13 FileDataStoreObject around `bytes`.
    pub fn file_data_store_object(bytes: &[u8]) -> Vec<u8> {
        let mut o = Guid::new(0xBDE316E7, 0x2665, 0x4511, [0xA4, 0xC4, 0x8D, 0x4D, 0x0B, 0x7A, 0x9E, 0xAC]).0.to_vec();
        o.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        o.extend_from_slice(&[0u8; 12]);
        o.extend_from_slice(bytes);
        while !(o.len() + 16).is_multiple_of(8) {
            o.push(0);
        }
        o.extend_from_slice(&Guid::new(0x71FBA722, 0x0F79, 0x4A0B, [0xBB, 0x13, 0x89, 0x92, 0x56, 0x42, 0x6B, 0x24]).0);
        o
    }
}

/// Synthetic `.one` sections with [MS-ONE] pages, titles and outlines (the fixtures of the
/// import tests here and in the engine).
pub mod section {
    use super::props::{PropSetBuilder, Val};
    use super::store::{RevBuilder, StoreBuilder};
    use crate::header::FileKind;
    use crate::one::{jcid, prop};
    use crate::revision::{ROOT_CONTENT, ROOT_METADATA};
    use crate::types::{ExtendedGuid, Guid};

    /// A page to write.
    #[derive(Debug, Clone, Default)]
    pub struct FixturePage {
        pub title: String,
        /// [MS-ONE] PageLevel (1–3); 0 leaves it out.
        pub level: i32,
        /// Seconds since 1980 ([MS-ONE] §2.3.1 Time32) for LastModifiedTime; 0 leaves it out.
        pub modified: u32,
        pub outlines: Vec<FixtureOutline>,
    }

    /// An outline to write.
    #[derive(Debug, Clone, Default)]
    pub struct FixtureOutline {
        /// OffsetFromParentHoriz/Vert, in half inches.
        pub x: f32,
        pub y: f32,
        /// LayoutMaxWidth in half inches with IsLayoutSizeSetByUser.
        pub width: Option<f32>,
        /// `(level from 0, text)`: a paragraph's children are the following ones one level deeper.
        pub paragraphs: Vec<(u8, String)>,
        /// Store the text as TextExtendedAscii (Windows-1252) instead of RichEditTextUnicode.
        pub ascii: bool,
    }

    /// Writes a section of `pages`.
    pub fn write(pages: &[FixturePage]) -> Vec<u8> {
        let mut next = 0u32;
        let mut new_id = || {
            next += 1;
            ExtendedGuid::new(Guid::new(0x0B1E_0000 + next, 0x5EC7, 0, [7; 8]), 1)
        };
        let section_space = new_id();
        let mut b = StoreBuilder::new(FileKind::Section, section_space);
        let page_spaces: Vec<ExtendedGuid> = pages.iter().map(|_| new_id()).collect();
        {
            let (section, series) = (new_id(), new_id());
            let rid = new_id();
            let rev = b.space(section_space).revision(rid, ExtendedGuid::NIL);
            let spaces = page_spaces.iter().map(|s| rev.cid(*s)).collect();
            let mut p = PropSetBuilder::default();
            p.add(prop::CHILD_GRAPH_SPACE_ELEMENT_NODES, Val::Spaces(spaces));
            rev.object(series, jcid::PAGE_SERIES, &p);
            let mut s = PropSetBuilder::default();
            let series_c = rev.cid(series);
            s.add(prop::ELEMENT_CHILD_NODES, Val::Objects(vec![series_c]));
            rev.object(section, jcid::SECTION, &s).root(section, ROOT_CONTENT);
        }
        for (page, space) in pages.iter().zip(&page_spaces) {
            let ids: Vec<ExtendedGuid> = (0..6).map(|_| new_id()).collect();
            let (rid, manifest, page_node, title_node, title_outline, meta) = (ids[0], ids[1], ids[2], ids[3], ids[4], ids[5]);
            let mut outline_ids = Vec::new();
            let mut pending = Vec::new();
            for o in &page.outlines {
                let id = new_id();
                outline_ids.push(id);
                pending.push((id, o.clone(), (0..o.paragraphs.len() * 2).map(|_| new_id()).collect::<Vec<_>>()));
            }
            let title_paras = (0..2).map(|_| new_id()).collect::<Vec<_>>();
            let rev = b.space(*space).revision(rid, ExtendedGuid::NIL);
            // The title: a title node holding one outline flagged IsTitleText.
            let title = FixtureOutline { paragraphs: vec![(0, page.title.clone())], ..FixtureOutline::default() };
            write_outline(rev, title_outline, &title, &title_paras, true);
            let mut t = PropSetBuilder::default();
            let c = rev.cid(title_outline);
            t.add(prop::ELEMENT_CHILD_NODES, Val::Objects(vec![c]));
            rev.object(title_node, jcid::TITLE, &t);
            for (id, o, para_ids) in &pending {
                write_outline(rev, *id, o, para_ids, false);
            }
            let mut p = PropSetBuilder::default();
            let tc = rev.cid(title_node);
            p.add(prop::STRUCTURE_ELEMENT_CHILD_NODES, Val::Objects(vec![tc]));
            let oc = outline_ids.iter().map(|o| rev.cid(*o)).collect();
            p.add(prop::ELEMENT_CHILD_NODES, Val::Objects(oc));
            if page.modified != 0 {
                p.add(prop::LAST_MODIFIED_TIME, Val::U32(page.modified));
            }
            rev.object(page_node, jcid::PAGE, &p);
            let mut m = PropSetBuilder::default();
            let pc = rev.cid(page_node);
            m.add(prop::CONTENT_CHILD_NODES, Val::Objects(vec![pc]));
            rev.object(manifest, jcid::PAGE_MANIFEST, &m).root(manifest, ROOT_CONTENT);
            let mut md = PropSetBuilder::default();
            md.add(prop::CACHED_TITLE_STRING, Val::Bytes(wz(&page.title)));
            if page.level != 0 {
                md.add(prop::PAGE_LEVEL, Val::U32(page.level as u32));
            }
            rev.object(meta, jcid::PAGE_METADATA, &md).root(meta, ROOT_METADATA);
        }
        b.finish()
    }

    /// A null-terminated UTF-16LE string ([MS-ONE] §2.2.9 WzInAtom).
    pub fn wz(s: &str) -> Vec<u8> {
        s.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
    }

    /// `ids` holds two ids per paragraph (outline element, rich text node).
    fn write_outline(rev: &mut RevBuilder, id: ExtendedGuid, o: &FixtureOutline, ids: &[ExtendedGuid], title: bool) {
        let n = o.paragraphs.len();
        // children[i]: the paragraphs directly under paragraph i (None: under the outline).
        let mut parents: Vec<Option<usize>> = Vec::with_capacity(n);
        for (i, (level, _)) in o.paragraphs.iter().enumerate() {
            let parent = (0..i).rev().find(|&j| o.paragraphs[j].0 < *level);
            parents.push(parent);
        }
        for (i, (_, text)) in o.paragraphs.iter().enumerate() {
            let (oe, rt) = (ids[2 * i], ids[2 * i + 1]);
            let mut r = PropSetBuilder::default();
            if o.ascii {
                let bytes = text.chars().map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')).collect();
                r.add(prop::TEXT_EXTENDED_ASCII, Val::Bytes(bytes));
            } else {
                r.add(prop::RICH_EDIT_TEXT_UNICODE, Val::Bytes(wz(text)));
            }
            rev.object(rt, jcid::RICH_TEXT_OE, &r);
            let mut e = PropSetBuilder::default();
            let rc = rev.cid(rt);
            e.add(prop::CONTENT_CHILD_NODES, Val::Objects(vec![rc]));
            let kids: Vec<_> = (0..n).filter(|&k| parents[k] == Some(i)).map(|k| rev.cid(ids[2 * k])).collect();
            if !kids.is_empty() {
                e.add(prop::ELEMENT_CHILD_NODES, Val::Objects(kids));
            }
            rev.object(oe, jcid::OUTLINE_ELEMENT, &e);
        }
        let mut p = PropSetBuilder::default();
        let top: Vec<_> = (0..n).filter(|&k| parents[k].is_none()).map(|k| rev.cid(ids[2 * k])).collect();
        p.add(prop::ELEMENT_CHILD_NODES, Val::Objects(top));
        if title {
            p.add(prop::IS_TITLE_TEXT, Val::Bool(true));
        } else {
            p.add(prop::OFFSET_FROM_PARENT_HORIZ, Val::U32(o.x.to_bits()));
            p.add(prop::OFFSET_FROM_PARENT_VERT, Val::U32(o.y.to_bits()));
        }
        if let Some(w) = o.width {
            p.add(prop::LAYOUT_MAX_WIDTH, Val::U32(w.to_bits()));
            p.add(prop::IS_LAYOUT_SIZE_SET_BY_USER, Val::Bool(true));
        }
        rev.object(id, jcid::OUTLINE, &p);
    }
}
