//! [MS-ONESTORE] §2.3.1 Header: the first 1,024 bytes of every revision-store file, validated
//! before anything else is read.

use crate::bytes::Cursor;
use crate::error::{OneError, Result};
use crate::types::{ChunkRef, Guid};

/// The header's size (§2.3.1).
pub const HEADER_SIZE: usize = 1024;

/// §2.3.1 `guidFileType` of a `.one` section file.
pub const GUID_FILE_TYPE_ONE: Guid = Guid::new(0x7B5C52E4, 0xD88C, 0x4DA7, [0xAE, 0xB1, 0x53, 0x78, 0xD0, 0x29, 0x96, 0xD3]);
/// §2.3.1 `guidFileType` of a `.onetoc2` table-of-contents file.
pub const GUID_FILE_TYPE_ONETOC2: Guid = Guid::new(0x43FF2FA1, 0xEFD9, 0x4C76, [0x9E, 0xE2, 0x10, 0xEA, 0x57, 0x22, 0x76, 0x5F]);
/// §2.3.1 `guidFileFormat`: the revision store file format.
pub const GUID_FILE_FORMAT: Guid = Guid::new(0x109ADD3F, 0x911B, 0x49F5, [0xA5, 0xD0, 0x17, 0x91, 0xED, 0xC8, 0xAE, 0xD8]);

/// §2.3.1 `ffvLastCodeThatWroteToThisFile` of a `.one` file.
pub const FFV_ONE: u32 = 0x2A;
/// §2.3.1 `ffvLastCodeThatWroteToThisFile` of a `.onetoc2` file.
pub const FFV_ONETOC2: u32 = 0x1B;

/// Which file the header describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// A `.one` section.
    Section,
    /// A `.onetoc2` notebook table of contents.
    Toc,
}

/// The header fields the reader uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// `guidFileType`.
    pub kind: FileKind,
    /// `guidFile`: this file's identity.
    pub guid_file: Guid,
    /// `ffvLastCodeThatWroteToThisFile`.
    pub ffv_last: u32,
    /// `cTransactionsInLog`: how many transactions of the log are committed.
    pub transactions: u32,
    /// `fcrTransactionLog`.
    pub transaction_log: ChunkRef,
    /// `fcrFileNodeListRoot`: the root file node list (§2.1.14).
    pub root_list: ChunkRef,
    /// `fcrHashedChunkList` (nil or zero when absent).
    pub hashed_chunk_list: ChunkRef,
    /// `cbExpectedFileLength`.
    pub expected_length: u64,
}

impl Header {
    /// Reads and validates the header at the start of `file`.
    pub fn read(file: &[u8]) -> Result<Self> {
        if file.len() < HEADER_SIZE {
            return Err(OneError::Unsupported(format!("{} bytes is too short for a OneNote file", file.len())));
        }
        let mut c = Cursor::new(file, 0);
        let file_type = Guid::read(&mut c, "guidFileType")?;
        let kind = match file_type {
            GUID_FILE_TYPE_ONE => FileKind::Section,
            GUID_FILE_TYPE_ONETOC2 => FileKind::Toc,
            _ => return Err(OneError::Unsupported("not a OneNote section or notebook file".into())),
        };
        let guid_file = Guid::read(&mut c, "guidFile")?;
        c.skip(16, "guidLegacyFileVersion")?;
        if Guid::read(&mut c, "guidFileFormat")? != GUID_FILE_FORMAT {
            return Err(OneError::Unsupported("not the OneNote 2010 or later file format".into()));
        }
        let ffv_last = c.u32("ffvLastCodeThatWroteToThisFile")?;
        // ffvOldestCodeThatHasWrittenToThisFile, ffvNewestCode…, ffvOldestCodeThatMayReadThisFile,
        // fcrLegacyFreeChunkList, fcrLegacyTransactionLog (§2.3.1: offsets 68–95).
        c.skip(12 + 8 + 8, "legacy header fields")?;
        let transactions = c.u32("cTransactionsInLog")?;
        // cbLegacyExpectedFileLength, rgbPlaceholder, fcrLegacyFileNodeListRoot,
        // cbLegacyFreeSpaceInFreeChunkList, the four flags, guidAncestor, crcName (100–147).
        c.skip(4 + 8 + 8 + 4 + 4 + 16 + 4, "legacy header fields")?;
        let hashed_chunk_list = ChunkRef::read64x32(&mut c, "fcrHashedChunkList")?;
        let transaction_log = ChunkRef::read64x32(&mut c, "fcrTransactionLog")?;
        let root_list = ChunkRef::read64x32(&mut c, "fcrFileNodeListRoot")?;
        c.skip(12, "fcrFreeChunkList")?;
        let expected_length = c.u64("cbExpectedFileLength")?;
        if transaction_log.is_empty() || root_list.is_empty() {
            // OneNote 2007 sections keep these in the legacy 32-bit fields; there is no public
            // specification of that format (catalog row FMT.One2007).
            return Err(OneError::Unsupported("OneNote 2007 sections can't be opened".into()));
        }
        Ok(Self { kind, guid_file, ffv_last, transactions, transaction_log, root_list, hashed_chunk_list, expected_length })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::FileBuilder;

    #[test]
    fn a_written_header_reads_back() {
        let file = FileBuilder::new(FileKind::Section).finish(&[]);
        let h = Header::read(&file).unwrap();
        assert_eq!(h.kind, FileKind::Section);
        assert_eq!(h.ffv_last, FFV_ONE);
        assert_eq!(h.expected_length, file.len() as u64);
        let toc = FileBuilder::new(FileKind::Toc).finish(&[]);
        assert_eq!(Header::read(&toc).unwrap().kind, FileKind::Toc);
    }

    #[test]
    fn foreign_files_are_unsupported_not_corrupt() {
        assert!(matches!(Header::read(b"%PDF-1.7"), Err(OneError::Unsupported(_))));
        assert!(matches!(Header::read(&[0u8; 2048]), Err(OneError::Unsupported(_))));
        let mut file = FileBuilder::new(FileKind::Section).finish(&[]);
        file[48] ^= 1; // guidFileFormat
        assert!(matches!(Header::read(&file), Err(OneError::Unsupported(_))));
    }

    #[test]
    fn a_header_without_the_modern_lists_is_onenote_2007() {
        let mut file = FileBuilder::new(FileKind::Section).finish(&[]);
        file[160..172].fill(0); // fcrTransactionLog = fcrZero
        assert!(matches!(Header::read(&file), Err(OneError::Unsupported(m)) if m.contains("2007")));
    }
}
