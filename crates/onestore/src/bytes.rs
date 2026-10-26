//! A bounds-checked little-endian cursor over a byte slice.
//!
//! Every read returns [`OneError::Corrupt`] past the end instead of panicking, and every
//! position is reported as an absolute file offset (`origin + pos`) so errors point at the
//! byte a maintainer can look at. All integers in [MS-ONESTORE] are little-endian (§1.3.6).

use crate::error::{OneError, Result};

/// A cursor over `data`, which starts at absolute file offset `origin`.
#[derive(Debug, Clone)]
pub(crate) struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
    origin: u64,
}

impl<'a> Cursor<'a> {
    /// A cursor at the start of `data`, which lies at file offset `origin`.
    pub(crate) fn new(data: &'a [u8], origin: u64) -> Self {
        Self { data, pos: 0, origin }
    }

    /// The absolute file offset of the next byte.
    pub(crate) fn at(&self) -> u64 {
        self.origin.saturating_add(self.pos as u64)
    }

    /// Bytes left.
    pub(crate) fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// The next `n` bytes; stops a count or size field that points past the end.
    pub(crate) fn take(&mut self, n: usize, what: &str) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len());
        match end.and_then(|e| self.data.get(self.pos..e)) {
            Some(s) => {
                self.pos += n;
                Ok(s)
            }
            None => Err(OneError::corrupt(self.at(), format!("{what} runs past the end ({n} bytes wanted, {} left)", self.remaining()))),
        }
    }

    /// Skips `n` bytes.
    pub(crate) fn skip(&mut self, n: usize, what: &str) -> Result<()> {
        self.take(n, what).map(|_| ())
    }

    /// A fixed-size array.
    pub(crate) fn array<const N: usize>(&mut self, what: &str) -> Result<[u8; N]> {
        let s = self.take(N, what)?;
        let mut a = [0u8; N];
        a.copy_from_slice(s);
        Ok(a)
    }

    pub(crate) fn u8(&mut self, what: &str) -> Result<u8> {
        Ok(self.array::<1>(what)?[0])
    }

    pub(crate) fn u16(&mut self, what: &str) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array(what)?))
    }

    pub(crate) fn u32(&mut self, what: &str) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array(what)?))
    }

    pub(crate) fn u64(&mut self, what: &str) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array(what)?))
    }

    /// `n` UTF-16LE code units decoded lossily (unpaired surrogates become U+FFFD).
    pub(crate) fn utf16(&mut self, n: usize, what: &str) -> Result<String> {
        let bytes = n.checked_mul(2).ok_or_else(|| OneError::corrupt(self.at(), format!("{what}: length overflows")))?;
        Ok(utf16_lossy(self.take(bytes, what)?))
    }
}

/// UTF-16LE bytes decoded lossily; a trailing odd byte is dropped.
pub(crate) fn utf16_lossy(bytes: &[u8]) -> String {
    let units = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c));
    char::decode_utf16(units).map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER)).collect()
}

/// `data[start..start + len]` for absolute offsets into the whole file, or `Corrupt`.
pub(crate) fn slice<'a>(data: &'a [u8], start: u64, len: u64, what: &str) -> Result<&'a [u8]> {
    let range = usize::try_from(start).ok().zip(usize::try_from(len).ok()).and_then(|(s, l)| Some(s..s.checked_add(l)?));
    range
        .and_then(|r| data.get(r))
        .ok_or_else(|| OneError::corrupt(start, format!("{what} ({len} bytes at {start}) lies outside the file ({} bytes)", data.len())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_are_bounded() {
        let mut c = Cursor::new(&[1, 0, 2, 0, 0, 0], 100);
        assert_eq!(c.u16("a").unwrap(), 1);
        assert_eq!(c.at(), 102);
        assert_eq!(c.u32("b").unwrap(), 2);
        let e = c.u8("c").unwrap_err();
        assert!(matches!(e, OneError::Corrupt { at: 106, .. }), "{e:?}");
        assert!(c.take(usize::MAX, "huge").is_err());
    }

    #[test]
    fn slices_never_wrap() {
        let d = [0u8; 8];
        assert_eq!(slice(&d, 2, 4, "x").unwrap().len(), 4);
        assert!(slice(&d, 6, 4, "x").is_err());
        assert!(slice(&d, u64::MAX, 2, "x").is_err());
        assert!(slice(&d, 1, u64::MAX, "x").is_err());
    }

    #[test]
    fn utf16_is_lossy_not_fatal() {
        assert_eq!(utf16_lossy(&[b'h', 0, b'i', 0]), "hi");
        assert_eq!(utf16_lossy(&[0x00, 0xD8, b'x', 0]), "\u{FFFD}x");
        assert_eq!(utf16_lossy(&[b'a', 0, 7]), "a");
    }
}
