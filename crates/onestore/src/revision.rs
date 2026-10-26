//! The revision store's object graph: object spaces (§2.1.4), their revision manifests
//! (§2.1.9, §2.1.10), object groups (§2.1.13) and global identification tables (§2.1.3),
//! resolved to the **current revision** of each object space: the one labelled with revision
//! role 0x1 and the default context (§2.1.11, §2.1.12; [MS-ONE] §2.1.8), with the objects and
//! roots of its dependency chain folded in (§2.1.9).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::Limits;
use crate::bytes;
use crate::error::{OneError, Result};
use crate::filenode::{FileNode, Jcid, NodeData, RootRef};
use crate::header::{FileKind, Header};
use crate::nodelist::ListReader;
use crate::propset::{self, GlobalIdTable, PropertySet};
use crate::txlog::TransactionLog;
use crate::types::{ChunkRef, ExtendedGuid, Guid};

/// §2.1.12: the revision role of the active view of an object space.
pub const ROLE_DEFAULT: u32 = 0x1;
/// [MS-ONE] §2.1.8 RootRole: default content root.
pub const ROOT_CONTENT: u32 = 0x1;
/// [MS-ONE] §2.1.8 RootRole: metadata root.
pub const ROOT_METADATA: u32 = 0x2;

/// §2.6.13 FileDataStoreObject `guidHeader`.
const FILE_DATA_HEADER: Guid = Guid::new(0xBDE316E7, 0x2665, 0x4511, [0xA4, 0xC4, 0x8D, 0x4D, 0x0B, 0x7A, 0x9E, 0xAC]);
/// §2.6.13 FileDataStoreObject `guidFooter`.
const FILE_DATA_FOOTER: Guid = Guid::new(0x71FBA722, 0x0F79, 0x4A0B, [0xBB, 0x13, 0x89, 0x92, 0x56, 0x42, 0x6B, 0x24]);

/// An object of a revision.
#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    /// Its type.
    pub jcid: Jcid,
    /// Its data.
    pub data: ObjectData,
}

/// What an object holds.
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectData {
    /// A property set (§2.6.1), references resolved.
    Props(PropertySet),
    /// A file data object (§2.5.27): the reference to its bytes.
    File {
        /// `FileDataReference`.
        reference: String,
        /// `Extension`.
        extension: String,
    },
    /// Data the reader could not decode (the reason is in [`Store::warnings`]).
    Damaged,
}

/// One object space at its current revision.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ObjectSpace {
    /// `gosid`.
    pub id: ExtendedGuid,
    /// Root objects by [MS-ONE] RootRole.
    pub roots: HashMap<u32, ExtendedGuid>,
    /// The objects of the revision.
    pub objects: HashMap<ExtendedGuid, Object>,
}

impl ObjectSpace {
    /// The root object with `role`.
    pub fn root(&self, role: u32) -> Option<&Object> {
        self.objects.get(self.roots.get(&role)?)
    }

    /// The property set of object `id` (`None`: missing or not a property set).
    pub fn props(&self, id: &ExtendedGuid) -> Option<&PropertySet> {
        match &self.objects.get(id)?.data {
            ObjectData::Props(p) => Some(p),
            _ => None,
        }
    }
}

/// A whole revision-store file, resolved.
#[derive(Debug, Clone)]
pub struct Store<'a> {
    file: &'a [u8],
    /// The header.
    pub header: Header,
    /// The root object space's id (§2.5.1).
    pub root: ExtendedGuid,
    /// Object spaces, in root-list order.
    pub spaces: Vec<ObjectSpace>,
    /// File data store objects by `guidReference` (§2.5.22).
    file_data: HashMap<Guid, ChunkRef>,
    /// Problems that cost content without stopping the read, for the fidelity report.
    pub warnings: Vec<String>,
}

/// A revision manifest as read, before dependencies are folded in.
#[derive(Debug, Default)]
struct Manifest {
    dependent: ExtendedGuid,
    roots: HashMap<u32, ExtendedGuid>,
    /// Declarations in order, each with the global id table in effect for it.
    objects: Vec<(ExtendedGuid, Decl)>,
    /// The manifest's own global id table (`.onetoc2` Entry2/Entry3 copy from the dependency's).
    table: GlobalIdTable,
}

#[derive(Debug, Clone)]
enum Decl {
    Props { jcid: Jcid, data: ChunkRef, table: Arc<GlobalIdTable> },
    File { jcid: Jcid, reference: String, extension: String },
}

impl<'a> Store<'a> {
    /// Reads `file`: header, transaction log, root list, every object space at its current
    /// revision. Errors are structural (the file can't be walked); damage inside one object only
    /// marks that object [`ObjectData::Damaged`] and adds a warning.
    pub fn read(file: &'a [u8], limits: &Limits) -> Result<Self> {
        if file.len() as u64 > limits.max_file_bytes {
            return Err(OneError::TooLarge(format!("{} bytes (the limit is {})", file.len(), limits.max_file_bytes)));
        }
        let header = Header::read(file)?;
        let log = TransactionLog::read(file, &header, limits)?;
        let mut lists = ListReader::new(file, &log, limits);
        let root_list = lists.read(header.root_list)?;
        let mut store = Store { file, header, root: ExtendedGuid::NIL, spaces: Vec::new(), file_data: HashMap::new(), warnings: Vec::new() };
        let mut root = None;
        // §2.1.14 root file node list.
        for node in &root_list.nodes {
            match &node.data {
                NodeData::ObjectSpaceManifestListReference { list, gosid } => {
                    let space = store.read_space(&mut lists, *list, *gosid, limits)?;
                    store.spaces.push(space);
                }
                NodeData::ObjectSpaceManifestRoot { root: r } => root = Some(*r),
                NodeData::FileDataStoreListReference { list } => {
                    for n in lists.read(*list)?.nodes {
                        if let NodeData::FileDataStoreObjectReference { data, guid } = n.data {
                            store.file_data.insert(guid, data);
                        }
                    }
                }
                _ => {}
            }
        }
        store.root = root.ok_or_else(|| OneError::corrupt(header_root(&store), "the file names no root object space"))?;
        Ok(store)
    }

    /// The object space `id`.
    pub fn space(&self, id: &ExtendedGuid) -> Option<&ObjectSpace> {
        self.spaces.iter().find(|s| s.id == *id)
    }

    /// The root object space.
    pub fn root_space(&self) -> Option<&ObjectSpace> {
        self.space(&self.root)
    }

    /// Which file this is.
    pub fn kind(&self) -> FileKind {
        self.header.kind
    }

    /// The bytes a file data object's `FileDataReference` names, when they are inside this
    /// file (`<ifndf>{GUID}</ifndf>`, §2.5.27); `None` for files stored beside the section
    /// (`<file>…`) or a damaged store object.
    pub fn file_bytes(&self, reference: &str) -> Option<&'a [u8]> {
        let guid = reference.strip_prefix("<ifndf>")?.strip_suffix("</ifndf>").and_then(Guid::parse)?;
        let chunk = self.file_data.get(&guid)?;
        file_data_object(self.file, *chunk).ok()
    }

    /// §2.1.6 object space manifest list → its last revision manifest list → current revision.
    fn read_space(&mut self, lists: &mut ListReader<'_>, list: ChunkRef, gosid: ExtendedGuid, limits: &Limits) -> Result<ObjectSpace> {
        let manifest_list = lists.read(list)?;
        // §2.1.6: if several revision manifest lists are listed, all but the last are ignored.
        let last = manifest_list.nodes.iter().rev().find_map(|n| match n.data {
            NodeData::RevisionManifestListReference { list } => Some(list),
            _ => None,
        });
        let Some(revisions) = last else {
            return Ok(ObjectSpace { id: gosid, ..ObjectSpace::default() });
        };
        let nodes = lists.read(revisions)?.nodes;
        let (manifests, labels) = self.read_manifests(lists, &nodes)?;
        // §2.1.11: the current revision of (role 0x1, default context).
        let Some(current) = labels.get(&(ROLE_DEFAULT, ExtendedGuid::NIL)).copied() else {
            return Ok(ObjectSpace { id: gosid, ..ObjectSpace::default() });
        };
        // The dependency chain, newest first; a forged loop or a missing revision stops it.
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut rid = current;
        while !rid.is_nil() && seen.insert(rid) {
            let Some(m) = manifests.get(&rid) else {
                self.warnings.push(format!("object space {gosid:?}: revision {rid:?} is missing"));
                break;
            };
            chain.push(m);
            rid = m.dependent;
        }
        let mut space = ObjectSpace { id: gosid, ..ObjectSpace::default() };
        let mut decls: HashMap<ExtendedGuid, Decl> = HashMap::new();
        for m in chain.iter().rev() {
            space.roots.extend(m.roots.iter().map(|(k, v)| (*k, *v)));
            for (oid, d) in &m.objects {
                // A revision (no JCID) keeps the type of the declaration it revises (§2.1.5).
                let d = match (d, decls.get(oid)) {
                    (Decl::Props { jcid, data, table }, Some(Decl::Props { jcid: old, .. })) if jcid.0 == 0 => {
                        Decl::Props { jcid: *old, data: *data, table: table.clone() }
                    }
                    _ => d.clone(),
                };
                decls.insert(*oid, d);
            }
        }
        for (oid, d) in decls {
            let object = self.decode_object(&d, limits.max_depth).unwrap_or_else(|e| {
                self.warnings.push(format!("object {oid:?} in object space {gosid:?}: {e}"));
                Object { jcid: jcid_of(&d), data: ObjectData::Damaged }
            });
            space.objects.insert(oid, object);
        }
        Ok(space)
    }

    /// §2.1.10 revision manifest list: the manifests by `rid` and the (role, context) labels.
    #[allow(clippy::type_complexity)]
    fn read_manifests(
        &mut self,
        lists: &mut ListReader<'_>,
        nodes: &[FileNode],
    ) -> Result<(HashMap<ExtendedGuid, Manifest>, HashMap<(u32, ExtendedGuid), ExtendedGuid>)> {
        let mut manifests: HashMap<ExtendedGuid, Manifest> = HashMap::new();
        let mut labels = HashMap::new();
        let mut open: Option<(ExtendedGuid, Manifest)> = None;
        let mut table: Arc<GlobalIdTable> = Arc::new(GlobalIdTable::new());
        let mut building: Option<GlobalIdTable> = None;
        for node in nodes {
            match &node.data {
                NodeData::RevisionManifestStart { rid, dependent, role, context } => {
                    // §2.1.11: a later association of the same label wins.
                    labels.insert((*role, *context), *rid);
                    table = Arc::new(GlobalIdTable::new());
                    open = Some((*rid, Manifest { dependent: *dependent, ..Manifest::default() }));
                }
                NodeData::RevisionRole { rid, role, context } => {
                    labels.insert((*role, *context), *rid);
                }
                // §2.1.9: the second node of an encrypted object space's revision manifest.
                NodeData::EncryptionKey if open.is_some() => return Err(OneError::Encrypted),
                NodeData::ObjectGroupListReference { list, .. } => {
                    let Some((_, m)) = open.as_mut() else { continue };
                    let group = lists.read(*list)?;
                    // §2.1.13: an object group carries its own global id table.
                    let mut gtable = Arc::new(GlobalIdTable::new());
                    let mut gbuilding: Option<GlobalIdTable> = None;
                    for n in &group.nodes {
                        Self::table_node(&n.data, &mut gtable, &mut gbuilding, None);
                        push_decl(m, &n.data, &gtable);
                    }
                }
                NodeData::RevisionManifestEnd => {
                    if let Some((rid, mut m)) = open.take() {
                        m.table = (*table).clone();
                        manifests.insert(rid, m);
                    }
                }
                data => {
                    let Some((_, m)) = open.as_mut() else { continue };
                    let dependency = manifests.get(&m.dependent).map(|d| &d.table);
                    if Self::table_node(data, &mut table, &mut building, dependency) {
                        continue;
                    }
                    if let NodeData::RootObject { root, role } = data {
                        let id = match root {
                            RootRef::Extended(x) => Some(*x),
                            RootRef::Compact(c) => propset::resolve(&table, *c),
                        };
                        if let Some(id) = id {
                            m.roots.insert(*role, id);
                        }
                    }
                    push_decl(m, data, &table);
                }
            }
        }
        Ok((manifests, labels))
    }

    /// Applies a global identification table node (§2.1.3); true if `data` was one. The table
    /// takes effect at its end node.
    fn table_node(data: &NodeData, table: &mut Arc<GlobalIdTable>, building: &mut Option<GlobalIdTable>, dependency: Option<&GlobalIdTable>) -> bool {
        match data {
            NodeData::GlobalIdTableStart => *building = Some(GlobalIdTable::new()),
            NodeData::GlobalIdTableEntry { index, guid } => {
                if let Some(t) = building.as_mut() {
                    t.insert(*index, *guid);
                }
            }
            // §2.5.11, §2.5.12 (.onetoc2): entries copied from the dependency revision's table.
            NodeData::GlobalIdTableEntry2 { from, to } => {
                if let (Some(t), Some(g)) = (building.as_mut(), dependency.and_then(|d| d.get(from))) {
                    t.insert(*to, *g);
                }
            }
            NodeData::GlobalIdTableEntry3 { from, count, to } => {
                if let (Some(t), Some(d)) = (building.as_mut(), dependency) {
                    // Bounded by the dependency's size: a forged count can't loop for long.
                    for k in 0..(*count).min(d.len() as u32) {
                        if let (Some(g), Some(dst)) = (from.checked_add(k).and_then(|f| d.get(&f)), to.checked_add(k)) {
                            t.insert(dst, *g);
                        }
                    }
                }
            }
            NodeData::GlobalIdTableEnd => {
                if let Some(t) = building.take() {
                    *table = Arc::new(t);
                }
            }
            _ => return false,
        }
        true
    }

    fn decode_object(&self, d: &Decl, max_depth: usize) -> Result<Object> {
        match d {
            Decl::Props { jcid, data, table } => {
                let bytes = data.bytes(self.file, "ObjectSpaceObjectPropSet")?;
                let props = propset::decode(bytes, data.stp, table, max_depth)?;
                Ok(Object { jcid: *jcid, data: ObjectData::Props(props) })
            }
            Decl::File { jcid, reference, extension } => {
                Ok(Object { jcid: *jcid, data: ObjectData::File { reference: reference.clone(), extension: extension.clone() } })
            }
        }
    }
}

fn header_root(store: &Store<'_>) -> u64 {
    store.header.root_list.stp
}

fn jcid_of(d: &Decl) -> Jcid {
    match d {
        Decl::Props { jcid, .. } | Decl::File { jcid, .. } => *jcid,
    }
}

/// Records an object declaration node into `m`, resolving its id with `table`.
fn push_decl(m: &mut Manifest, data: &NodeData, table: &Arc<GlobalIdTable>) {
    match data {
        NodeData::Object { data, oid, jcid, .. } => {
            if let Some(id) = propset::resolve(table, *oid) {
                m.objects.push((id, Decl::Props { jcid: *jcid, data: *data, table: table.clone() }));
            }
        }
        NodeData::FileDataObject { oid, jcid, reference, extension } => {
            if let Some(id) = propset::resolve(table, *oid) {
                m.objects.push((id, Decl::File { jcid: *jcid, reference: reference.clone(), extension: extension.clone() }));
            }
        }
        _ => {}
    }
}

/// §2.6.13 FileDataStoreObject: guidHeader, cbLength (u64), unused (u32), reserved (u64), the
/// data, padding (so the structure ends on an 8-byte boundary), guidFooter. The footer is read
/// at the end of the referenced chunk, so the padding rule's exact reading doesn't matter.
fn file_data_object(file: &[u8], chunk: ChunkRef) -> Result<&[u8]> {
    const HEAD: usize = 16 + 8 + 4 + 8;
    let bytes = chunk.bytes(file, "FileDataStoreObject")?;
    let mut c = bytes::Cursor::new(bytes, chunk.stp);
    if Guid::read(&mut c, "guidHeader")? != FILE_DATA_HEADER {
        return Err(OneError::corrupt(chunk.stp, "a file data store object has the wrong header"));
    }
    let len = usize::try_from(c.u64("cbLength")?).unwrap_or(usize::MAX);
    c.skip(12, "unused, reserved")?;
    let data = c.take(len, "FileData")?;
    let footer_at = bytes
        .len()
        .checked_sub(16)
        .filter(|&f| f >= HEAD.saturating_add(len))
        .ok_or_else(|| OneError::corrupt(chunk.stp, "a file data store object has no room for its footer"))?;
    let mut f = bytes::Cursor::new(bytes.get(footer_at..).unwrap_or_default(), chunk.stp + footer_at as u64);
    if Guid::read(&mut f, "guidFooter")? != FILE_DATA_FOOTER {
        return Err(OneError::corrupt(f.at(), "a file data store object has the wrong footer"));
    }
    Ok(data)
}

#[cfg(test)]
mod tests;
