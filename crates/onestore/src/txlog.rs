//! [MS-ONESTORE] §2.3.3 Transaction log: how many file nodes of each file node list are
//! committed. Nodes past that count were written by an interrupted save and are ignored
//! (§2.4.1 `rgFileNodes`).

use std::collections::{HashMap, HashSet};

use crate::Limits;
use crate::bytes::Cursor;
use crate::error::{OneError, Result};
use crate::header::Header;
use crate::types::ChunkRef;

/// §2.3.3.2 TransactionEntry `srcID` of the sentinel that ends a transaction.
const SENTINEL: u32 = 0x0000_0001;

/// Committed file node counts by `FileNodeListID`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransactionLog {
    counts: HashMap<u32, u32>,
}

impl TransactionLog {
    /// The number of committed nodes of list `id` (0 for a list no transaction touched).
    pub fn committed(&self, id: u32) -> u32 {
        self.counts.get(&id).copied().unwrap_or(0)
    }

    /// Reads the first `header.transactions` transactions of the log.
    pub fn read(file: &[u8], header: &Header, limits: &Limits) -> Result<Self> {
        let mut log = Self::default();
        let mut left = header.transactions;
        let mut next = header.transaction_log;
        let mut seen = HashSet::new();
        while left > 0 {
            if next.is_empty() {
                return Err(OneError::corrupt(next.stp, format!("the transaction log ends {left} transactions early")));
            }
            // A fragment chain that loops back would never end.
            if !seen.insert(next.stp) || seen.len() > limits.max_fragments {
                return Err(OneError::corrupt(next.stp, "the transaction log's fragments form a loop"));
            }
            let bytes = next.bytes(file, "TransactionLogFragment")?;
            // §2.3.3.1 TransactionLogFragment: sizeTable (8-byte entries), then nextFragment (12).
            let table_len = bytes
                .len()
                .checked_sub(12)
                .ok_or_else(|| OneError::corrupt(next.stp, "a transaction log fragment is shorter than its next-fragment reference"))?;
            let mut c = Cursor::new(bytes, next.stp);
            for _ in 0..table_len / 8 {
                let src = c.u32("TransactionEntry.srcID")?;
                let switch = c.u32("TransactionEntry.TransactionEntrySwitch")?;
                match src {
                    // Unused space at the end of the last fragment.
                    0 => {}
                    SENTINEL => {
                        left -= 1;
                        if left == 0 {
                            break;
                        }
                    }
                    // §2.3.3.2: the list's node count once this transaction is applied.
                    id => {
                        log.counts.insert(id, switch);
                    }
                }
            }
            let mut tail = Cursor::new(bytes, next.stp);
            tail.skip(table_len, "sizeTable")?;
            next = ChunkRef::read64x32(&mut tail, "TransactionLogFragment.nextFragment")?;
        }
        Ok(log)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::FileKind;
    use crate::writer::FileBuilder;

    #[test]
    fn committed_counts_follow_the_last_transaction() {
        let mut b = FileBuilder::new(FileKind::Section);
        b.transaction(&[(0x10, 3), (0x11, 1)]);
        b.transaction(&[(0x10, 5)]);
        let file = b.finish(&[]);
        let header = Header::read(&file).unwrap();
        let log = TransactionLog::read(&file, &header, &Limits::default()).unwrap();
        assert_eq!(log.committed(0x10), 5);
        assert_eq!(log.committed(0x11), 1);
        assert_eq!(log.committed(0x12), 0);
    }

    #[test]
    fn uncommitted_transactions_are_ignored() {
        let mut b = FileBuilder::new(FileKind::Section);
        b.transaction(&[(0x10, 3)]);
        b.transaction(&[(0x10, 9)]);
        let mut file = b.finish(&[]);
        file[96..100].copy_from_slice(&1u32.to_le_bytes()); // cTransactionsInLog = 1
        let header = Header::read(&file).unwrap();
        assert_eq!(TransactionLog::read(&file, &header, &Limits::default()).unwrap().committed(0x10), 3);
    }

    #[test]
    fn a_log_that_claims_more_transactions_than_it_has_is_corrupt() {
        let mut b = FileBuilder::new(FileKind::Section);
        b.transaction(&[(0x10, 3)]);
        let mut file = b.finish(&[]);
        file[96..100].copy_from_slice(&7u32.to_le_bytes());
        let header = Header::read(&file).unwrap();
        assert!(matches!(TransactionLog::read(&file, &header, &Limits::default()), Err(OneError::Corrupt { .. })));
    }
}
