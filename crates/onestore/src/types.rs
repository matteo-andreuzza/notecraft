//! The common types of [MS-ONESTORE] §2.2: GUIDs, extended GUIDs, compact ids, chunk references
//! and strings in storage buffers.

use std::fmt;

use crate::bytes::{self, Cursor};
use crate::error::{OneError, Result};

/// A GUID in its on-disk layout ([MS-DTYP] §2.3.4.2: `Data1` u32, `Data2` u16, `Data3` u16 little
/// endian, then 8 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Guid(pub [u8; 16]);

impl Guid {
    /// The all-zero GUID.
    pub const NIL: Guid = Guid([0; 16]);

    /// The GUID written `{d1-d2-d3-d4[0..2]-d4[2..8]}` in registry form.
    pub const fn new(d1: u32, d2: u16, d3: u16, d4: [u8; 8]) -> Self {
        let a = d1.to_le_bytes();
        let b = d2.to_le_bytes();
        let c = d3.to_le_bytes();
        Guid([a[0], a[1], a[2], a[3], b[0], b[1], c[0], c[1], d4[0], d4[1], d4[2], d4[3], d4[4], d4[5], d4[6], d4[7]])
    }

    pub(crate) fn read(c: &mut Cursor<'_>, what: &str) -> Result<Self> {
        c.array(what).map(Guid)
    }

    /// True for the all-zero GUID.
    pub fn is_nil(&self) -> bool {
        *self == Self::NIL
    }

    /// Parses the registry form `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` (braces optional).
    pub fn parse(s: &str) -> Option<Self> {
        let hex: String = s.trim().trim_start_matches('{').trim_end_matches('}').chars().filter(|&c| c != '-').collect();
        if hex.len() != 32 || !hex.is_ascii() {
            return None;
        }
        let num = |r: std::ops::Range<usize>| hex.get(r).and_then(|h| u64::from_str_radix(h, 16).ok());
        let d1 = u32::try_from(num(0..8)?).ok()?;
        let d2 = u16::try_from(num(8..12)?).ok()?;
        let d3 = u16::try_from(num(12..16)?).ok()?;
        let mut d4 = [0u8; 8];
        for (i, b) in d4.iter_mut().enumerate() {
            *b = u8::try_from(num(16 + 2 * i..18 + 2 * i)?).ok()?;
        }
        Some(Guid::new(d1, d2, d3, d4))
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let g = &self.0;
        let d1 = u32::from_le_bytes([g[0], g[1], g[2], g[3]]);
        let d2 = u16::from_le_bytes([g[4], g[5]]);
        let d3 = u16::from_le_bytes([g[6], g[7]]);
        write!(f, "{{{d1:08X}-{d2:04X}-{d3:04X}-{:02X}{:02X}-", g[8], g[9])?;
        for b in &g[10..16] {
            write!(f, "{b:02X}")?;
        }
        f.write_str("}")
    }
}

impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// [MS-ONESTORE] §2.2.1 ExtendedGUID: a GUID and a 32-bit number. Object, object space,
/// revision and context ids are extended GUIDs.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ExtendedGuid {
    /// The GUID.
    pub guid: Guid,
    /// The number.
    pub n: u32,
}

impl ExtendedGuid {
    /// The nil extended GUID (zero GUID, `n = 0`), "no object" / "default context".
    pub const NIL: ExtendedGuid = ExtendedGuid { guid: Guid::NIL, n: 0 };

    /// An extended GUID.
    pub const fn new(guid: Guid, n: u32) -> Self {
        Self { guid, n }
    }

    pub(crate) fn read(c: &mut Cursor<'_>, what: &str) -> Result<Self> {
        let guid = Guid::read(c, what)?;
        let n = c.u32(what)?;
        Ok(Self { guid, n })
    }

    /// True for [`ExtendedGuid::NIL`].
    pub fn is_nil(&self) -> bool {
        *self == Self::NIL
    }
}

impl fmt::Debug for ExtendedGuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.guid, self.n)
    }
}

/// [MS-ONESTORE] §2.2.2 CompactID: an extended GUID written as an 8-bit `n` and a 24-bit index
/// into the global identification table in effect (§2.1.3).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Debug)]
pub struct CompactId {
    /// The `n` of the extended GUID.
    pub n: u8,
    /// The index of the GUID in the global identification table.
    pub guid_index: u32,
}

impl CompactId {
    pub(crate) fn from_u32(v: u32) -> Self {
        Self { n: (v & 0xFF) as u8, guid_index: v >> 8 }
    }

    pub(crate) fn read(c: &mut Cursor<'_>, what: &str) -> Result<Self> {
        c.u32(what).map(Self::from_u32)
    }

    /// The raw 32-bit value.
    pub fn to_u32(self) -> u32 {
        u32::from(self.n) | (self.guid_index << 8)
    }

    /// §2.2.2: `n = 0` and index 0 mean the nil extended GUID.
    pub fn is_nil(&self) -> bool {
        self.n == 0 && self.guid_index == 0
    }
}

/// A file chunk reference: where a block of the file starts and how long it is
/// ([MS-ONESTORE] §2.2.4: FileChunkReference32 §2.2.4.1, FileChunkReference64x32 §2.2.4.4,
/// FileNodeChunkReference §2.2.4.2). Values are decoded (compressed formats already multiplied
/// by 8); the `nil` flag remembers whether the stored `stp` had every bit set.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct ChunkRef {
    /// Absolute offset in the file.
    pub stp: u64,
    /// Size in bytes.
    pub cb: u64,
    /// §2.2.4: `fcrNil`, every bit of `stp` set and `cb` zero: "no data".
    pub nil: bool,
}

impl ChunkRef {
    /// `fcrNil`.
    pub const NIL: ChunkRef = ChunkRef { stp: u64::MAX, cb: 0, nil: true };

    /// A reference to `cb` bytes at `stp`.
    pub const fn new(stp: u64, cb: u64) -> Self {
        Self { stp, cb, nil: false }
    }

    /// §2.2.4: `fcrZero`, offset and size zero.
    pub fn is_zero(&self) -> bool {
        !self.nil && self.stp == 0 && self.cb == 0
    }

    /// Nil or zero: the reference points at nothing.
    pub fn is_empty(&self) -> bool {
        self.nil || self.is_zero()
    }

    /// §2.2.4.4 FileChunkReference64x32: `stp` u64, `cb` u32.
    pub(crate) fn read64x32(c: &mut Cursor<'_>, what: &str) -> Result<Self> {
        let stp = c.u64(what)?;
        let cb = c.u32(what)?;
        Ok(Self { stp, cb: u64::from(cb), nil: stp == u64::MAX && cb == 0 })
    }

    /// §2.2.4.2 FileNodeChunkReference with the `StpFormat` and `CbFormat` of the FileNode's
    /// header (§2.4.3): stp 0 = 8 bytes, 1 = 4 bytes, 2 = 2 bytes ×8, 3 = 4 bytes ×8; cb 0 = 4
    /// bytes, 1 = 8 bytes, 2 = 1 byte ×8, 3 = 2 bytes ×8.
    pub(crate) fn read_node(c: &mut Cursor<'_>, stp_format: u8, cb_format: u8, what: &str) -> Result<Self> {
        let (stp, stp_all_ones) = match stp_format {
            0 => {
                let v = c.u64(what)?;
                (v, v == u64::MAX)
            }
            1 => {
                let v = c.u32(what)?;
                (u64::from(v), v == u32::MAX)
            }
            2 => {
                let v = c.u16(what)?;
                (u64::from(v) * 8, v == u16::MAX)
            }
            _ => {
                let v = c.u32(what)?;
                (u64::from(v) * 8, v == u32::MAX)
            }
        };
        let cb = match cb_format {
            0 => u64::from(c.u32(what)?),
            1 => c.u64(what)?,
            2 => u64::from(c.u8(what)?) * 8,
            _ => u64::from(c.u16(what)?) * 8,
        };
        if stp_all_ones && cb == 0 {
            return Ok(Self::NIL);
        }
        Ok(Self { stp, cb, nil: false })
    }

    /// The referenced bytes of `file`, refusing a reference outside it (a forged offset or size).
    pub(crate) fn bytes<'a>(&self, file: &'a [u8], what: &str) -> Result<&'a [u8]> {
        if self.nil {
            return Err(OneError::corrupt(0, format!("{what}: the reference is nil")));
        }
        bytes::slice(file, self.stp, self.cb, what)
    }
}

/// [MS-ONESTORE] §2.2.3 StringInStorageBuffer: a 32-bit count of UTF-16 code units, then the units.
pub(crate) fn read_string(c: &mut Cursor<'_>, what: &str) -> Result<String> {
    let n = c.u32(what)?;
    let n = usize::try_from(n).map_err(|_| OneError::corrupt(c.at(), format!("{what}: length overflows")))?;
    c.utf16(n, what)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_round_trips_through_text() {
        let g = Guid::new(0x7B5C52E4, 0xD88C, 0x4DA7, [0xAE, 0xB1, 0x53, 0x78, 0xD0, 0x29, 0x96, 0xD3]);
        assert_eq!(g.0[..4], [0xE4, 0x52, 0x5C, 0x7B]);
        let s = g.to_string();
        assert_eq!(s, "{7B5C52E4-D88C-4DA7-AEB1-5378D02996D3}");
        assert_eq!(Guid::parse(&s), Some(g));
        assert_eq!(Guid::parse("7b5c52e4-d88c-4da7-aeb1-5378d02996d3"), Some(g));
        assert_eq!(Guid::parse("{nope}"), None);
        assert_eq!(Guid::parse("{7B5C52E4-D88C-4DA7-AEB1-5378D02996Dé}"), None);
    }

    #[test]
    fn compact_id_splits_n_and_index() {
        let id = CompactId::from_u32(0x0000_0305);
        assert_eq!((id.n, id.guid_index), (5, 3));
        assert_eq!(id.to_u32(), 0x305);
        assert!(CompactId::from_u32(0).is_nil());
    }

    #[test]
    fn node_chunk_references_decode_every_format() {
        let read = |bytes: &[u8], s, c| ChunkRef::read_node(&mut Cursor::new(bytes, 0), s, c, "r");
        assert_eq!(read(&[16, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0], 0, 0).unwrap(), ChunkRef::new(16, 4));
        assert_eq!(read(&[16, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0], 1, 1).unwrap(), ChunkRef::new(16, 4));
        assert_eq!(read(&[2, 0, 3], 2, 2).unwrap(), ChunkRef::new(16, 24));
        assert_eq!(read(&[2, 0, 0, 0, 3, 0], 3, 3).unwrap(), ChunkRef::new(16, 24));
        assert!(read(&[0xFF, 0xFF, 0], 2, 2).unwrap().nil);
        assert!(read(&[0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0], 1, 0).unwrap().nil);
        assert!(read(&[1], 0, 0).is_err());
    }

    #[test]
    fn references_outside_the_file_are_refused() {
        let file = [0u8; 32];
        assert!(ChunkRef::new(16, 16).bytes(&file, "x").is_ok());
        assert!(ChunkRef::new(16, 17).bytes(&file, "x").is_err());
        assert!(ChunkRef::new(u64::MAX - 1, 4).bytes(&file, "x").is_err());
        assert!(ChunkRef::NIL.bytes(&file, "x").is_err());
    }

    #[test]
    fn strings_in_storage_buffers() {
        let mut c = Cursor::new(&[2, 0, 0, 0, b'o', 0, b'k', 0], 0);
        assert_eq!(read_string(&mut c, "s").unwrap(), "ok");
        let mut c = Cursor::new(&[0xFF, 0xFF, 0xFF, 0x7F], 0);
        assert!(read_string(&mut c, "s").is_err());
    }
}
