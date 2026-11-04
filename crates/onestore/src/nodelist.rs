//! [MS-ONESTORE] §2.4 File node lists: chains of FileNodeListFragment structures (§2.4.1),
//! each a FileNodeListHeader (§2.4.2), file nodes (§2.4.3), padding, the next fragment's
//! reference and a footer.

use std::collections::HashSet;

use crate::Limits;
use crate::bytes::Cursor;
use crate::error::{OneError, Result};
use crate::filenode::{self, FileNode, NodeData, NodeHeader};
use crate::txlog::TransactionLog;
use crate::types::ChunkRef;

/// §2.4.2 `uintMagic`.
pub const HEADER_MAGIC: u64 = 0xA456_7AB1_F5F7_F4C4;
/// §2.4.1 `footer`.
pub const FOOTER_MAGIC: u64 = 0x8BC2_15C3_8233_BA4B;
/// Header (16) + nextFragment (12) + footer (8): the smallest fragment.
const FRAGMENT_OVERHEAD: usize = 16 + 12 + 8;

/// A file node list: its id and its committed nodes in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileNodeList {
    /// `FileNodeListID`.
    pub id: u32,
    /// The committed nodes (§2.4.1: never more than the transaction log's count).
    pub nodes: Vec<FileNode>,
}

/// Reads file node lists of one file, enforcing the limits across all of them.
pub(crate) struct ListReader<'a> {
    file: &'a [u8],
    log: &'a TransactionLog,
    limits: &'a Limits,
    /// Every fragment read so far: a fragment belongs to exactly one list, so seeing one twice
    /// means a forged chain (a loop, or two lists sharing a fragment).
    fragments: HashSet<u64>,
    nodes: usize,
}

impl<'a> ListReader<'a> {
    pub(crate) fn new(file: &'a [u8], log: &'a TransactionLog, limits: &'a Limits) -> Self {
        Self { file, log, limits, fragments: HashSet::new(), nodes: 0 }
    }

    /// Reads the list whose first fragment `first` references.
    pub(crate) fn read(&mut self, first: ChunkRef) -> Result<FileNodeList> {
        let mut list: Option<(u32, u32)> = None; // (id, committed count)
        let mut nodes = Vec::new();
        let mut next = first;
        let mut sequence = 0u32;
        loop {
            if next.is_empty() {
                let (id, want) = list.unwrap_or((0, 0));
                return Err(OneError::corrupt(first.stp, format!("file node list {id:#X} ends after {} of its {want} nodes", nodes.len())));
            }
            if !self.fragments.insert(next.stp) {
                return Err(OneError::corrupt(next.stp, "a file node list fragment is used twice (a loop in the file)"));
            }
            if self.fragments.len() > self.limits.max_fragments {
                return Err(OneError::TooLarge(format!("more than {} file node list fragments", self.limits.max_fragments)));
            }
            let bytes = next.bytes(self.file, "FileNodeListFragment")?;
            let body_len =
                bytes.len().checked_sub(FRAGMENT_OVERHEAD).ok_or_else(|| OneError::corrupt(next.stp, "a file node list fragment is too short"))?;
            let mut c = Cursor::new(bytes, next.stp);
            // §2.4.2 FileNodeListHeader.
            if c.u64("FileNodeListHeader.uintMagic")? != HEADER_MAGIC {
                return Err(OneError::corrupt(next.stp, "a file node list fragment has the wrong magic number"));
            }
            let id = c.u32("FileNodeListID")?;
            let seq = c.u32("nFragmentSequence")?;
            let (list_id, committed) = match list {
                None => {
                    // §2.4.2: FileNodeListID MUST be at least 0x10.
                    if id < 0x10 {
                        return Err(OneError::corrupt(next.stp, format!("file node list id {id:#X} is reserved")));
                    }
                    let l = (id, self.log.committed(id));
                    list = Some(l);
                    l
                }
                Some(l) => l,
            };
            if id != list_id || seq != sequence {
                return Err(OneError::corrupt(
                    next.stp,
                    format!("fragment {seq} of list {id:#X} found where fragment {sequence} of list {list_id:#X} belongs"),
                ));
            }
            let body = c.take(body_len, "rgFileNodes")?;
            let next_ref = ChunkRef::read64x32(&mut c, "FileNodeListFragment.nextFragment")?;
            if c.u64("FileNodeListFragment.footer")? != FOOTER_MAGIC {
                return Err(OneError::corrupt(c.at().saturating_sub(8), "a file node list fragment has the wrong footer"));
            }
            self.read_nodes(body, next.stp + 16, committed, &mut nodes)?;
            // §2.4.1: once the committed count is reached, nextFragment MUST be ignored.
            if nodes.len() as u64 >= u64::from(committed) {
                return Ok(FileNodeList { id: list_id, nodes });
            }
            next = next_ref;
            sequence = sequence.checked_add(1).ok_or_else(|| OneError::corrupt(next.stp, "too many fragments"))?;
        }
    }

    /// The nodes of one fragment's `rgFileNodes` (at file offset `origin`), up to `committed`.
    fn read_nodes(&mut self, body: &[u8], origin: u64, committed: u32, nodes: &mut Vec<FileNode>) -> Result<()> {
        let mut c = Cursor::new(body, origin);
        // §2.4.1: the stream ends with fewer than 4 bytes left, a ChunkTerminatorFND or the count.
        while c.remaining() >= 4 && (nodes.len() as u64) < u64::from(committed) {
            let at = c.at();
            let raw = c.u32("FileNode header")?;
            if raw == 0 {
                // Zero padding where a node should be: nothing more in this fragment.
                break;
            }
            let h = NodeHeader::from_u32(raw);
            let body_len = usize::from(h.size)
                .checked_sub(4)
                .ok_or_else(|| OneError::corrupt(at, format!("file node {:#05X} claims a size of {} bytes", h.id, h.size)))?;
            let mut nc = Cursor::new(c.take(body_len, "FileNode")?, at + 4);
            let node = filenode::decode(h, at, &mut nc)?;
            if node.data == NodeData::ChunkTerminator {
                break;
            }
            self.nodes += 1;
            if self.nodes > self.limits.max_nodes {
                return Err(OneError::TooLarge(format!("more than {} file nodes", self.limits.max_nodes)));
            }
            nodes.push(node);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filenode::id;
    use crate::header::{FileKind, Header};
    use crate::types::ExtendedGuid;
    use crate::writer::{FileBuilder, node};

    fn gosid(n: u32) -> ExtendedGuid {
        ExtendedGuid::new(crate::types::Guid::new(n, 1, 2, [3; 8]), n)
    }

    fn start(n: u32) -> Vec<u8> {
        node::object_space_manifest_list_start(gosid(n))
    }

    fn read_root(file: &[u8]) -> Result<FileNodeList> {
        let header = Header::read(file)?;
        let limits = Limits::default();
        let log = TransactionLog::read(file, &header, &limits)?;
        ListReader::new(file, &log, &limits).read(header.root_list)
    }

    #[test]
    fn a_list_reads_back_across_fragments() {
        let mut b = FileBuilder::new(FileKind::Section);
        let nodes: Vec<_> = (1..=7).map(start).collect();
        let root = b.list_in_fragments(&nodes, 3);
        let file = b.finish_with_root(root);
        let list = read_root(&file).unwrap();
        assert_eq!(list.nodes.len(), 7);
        assert_eq!(list.nodes[6].data, NodeData::ObjectSpaceManifestListStart { gosid: gosid(7) });
        assert!(list.nodes.iter().all(|n| n.id == id::OBJECT_SPACE_MANIFEST_LIST_START));
    }

    #[test]
    fn nodes_past_the_committed_count_are_ignored() {
        let mut b = FileBuilder::new(FileKind::Section);
        let nodes: Vec<_> = (1..=4).map(start).collect();
        let root = b.list_uncommitted(&nodes, 2);
        let file = b.finish_with_root(root);
        assert_eq!(read_root(&file).unwrap().nodes.len(), 2);
    }

    #[test]
    fn a_fragment_loop_is_refused() {
        let mut b = FileBuilder::new(FileKind::Section);
        let nodes: Vec<_> = (1..=4).map(start).collect();
        let root = b.list_in_fragments(&nodes, 2);
        let mut file = b.finish_with_root(root);
        // Point the first fragment's nextFragment back at itself.
        let first = root.stp as usize;
        let len = root.cb as usize;
        file[first + len - 20..first + len - 12].copy_from_slice(&root.stp.to_le_bytes());
        file[first + len - 12..first + len - 8].copy_from_slice(&(root.cb as u32).to_le_bytes());
        let e = read_root(&file).unwrap_err();
        assert!(matches!(&e, OneError::Corrupt { what, .. } if what.contains("twice")), "{e}");
    }

    #[test]
    fn bad_magic_and_sizes_are_corrupt() {
        let mut b = FileBuilder::new(FileKind::Section);
        let root = b.list(&[start(1)]);
        let good = b.finish_with_root(root);
        let at = root.stp as usize;
        let mut file = good.clone();
        file[at] ^= 0xFF;
        assert!(matches!(read_root(&file), Err(OneError::Corrupt { .. })));
        let mut file = good.clone();
        let end = at + root.cb as usize;
        file[end - 1] ^= 0xFF;
        assert!(matches!(read_root(&file), Err(OneError::Corrupt { .. })));
        // A node whose Size runs past the fragment.
        let mut file = good;
        let h = NodeHeader { id: id::OBJECT_SPACE_MANIFEST_LIST_START, size: 0x1FFF, stp_format: 0, cb_format: 0, base_type: 0 };
        file[at + 16..at + 20].copy_from_slice(&h.to_u32().to_le_bytes());
        assert!(matches!(read_root(&file), Err(OneError::Corrupt { .. })));
    }

    #[test]
    fn the_node_budget_is_enforced() {
        let mut b = FileBuilder::new(FileKind::Section);
        let nodes: Vec<_> = (1..=5).map(start).collect();
        let root = b.list(&nodes);
        let file = b.finish_with_root(root);
        let header = Header::read(&file).unwrap();
        let limits = Limits { max_nodes: 3, ..Limits::default() };
        let log = TransactionLog::read(&file, &header, &limits).unwrap();
        assert!(matches!(ListReader::new(&file, &log, &limits).read(header.root_list), Err(OneError::TooLarge(_))));
    }
}
