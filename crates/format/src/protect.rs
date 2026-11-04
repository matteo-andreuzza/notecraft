//! Password-protected sections (decision D14): a section file sealed with a key derived from
//! the password.
//!
//! **Threat model.** The protection keeps a section's notes unreadable to someone who gets
//! its file (a copied notebook folder, a backup, a stolen disk) without the password. It does
//! not protect a section while it is unlocked in a running NoteCraft (its notes and the key
//! are in memory), nor against someone who can change the program. A forgotten password can't
//! be recovered (`BEH.Password.NoRecovery`): nothing else can open the file.
//!
//! **File layout** (`*.ncraft` like any section, told apart by its first bytes):
//!
//! ```text
//! "NCRAFT-SEALED\n"            14 bytes, MAGIC
//! header length                u32, little endian (at most MAX_HEADER)
//! header                       JSON: SealedHeader (id, name, colour and creation date shown
//!                              while locked; key derivation parameters, salt and nonce)
//! ciphertext                   XChaCha20-Poly1305 of the section's ordinary file (the zip of
//!                              section.json and its media), the header bytes as associated
//!                              data so neither part can be changed or swapped
//! ```
//!
//! The key is Argon2id (RFC 9106) of the password with a random 16-byte salt; the default cost
//! is 64 MiB, 3 passes, 1 lane (RFC 9106 §4's second recommended option, with fewer passes
//! for the memory). Costs read from a file are bounded so a crafted file can't make opening
//! it take gigabytes or minutes.

use std::sync::Arc;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use notecraft_doc::{Id, Protection, Section, SectionColor, Timestamp};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{FormatError, Result, corrupt};

/// The first bytes of a sealed section file.
pub const MAGIC: &[u8] = b"NCRAFT-SEALED\n";
/// The header's `format` value.
const HEADER_FORMAT: &str = "notecraft-sealed";
/// The sealed-file version this build writes and reads.
pub const SEALED_VERSION: u32 = 1;
/// Largest header read.
const MAX_HEADER: usize = 64 << 10;
/// Highest Argon2 memory cost accepted from a file (KiB): 1 GiB.
const MAX_MEMORY_KIB: u32 = 1 << 20;
/// Most Argon2 passes accepted from a file.
const MAX_PASSES: u32 = 16;
/// Most Argon2 lanes accepted from a file.
const MAX_LANES: u32 = 8;

/// A derived key (zeroed when dropped).
pub type Key = Zeroizing<[u8; 32]>;

/// Argon2id costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory in KiB.
    pub m_kib: u32,
    /// Passes.
    pub t: u32,
    /// Lanes.
    pub p: u32,
}

impl KdfParams {
    /// What new passwords use.
    pub const DEFAULT: KdfParams = KdfParams { m_kib: 64 << 10, t: 3, p: 1 };
    /// Cheap costs for tests (never for real notes).
    pub const TEST: KdfParams = KdfParams { m_kib: 64, t: 1, p: 1 };

    /// As stored in [`Protection::kdf`].
    pub fn to_array(self) -> [u32; 3] {
        [self.m_kib, self.t, self.p]
    }

    /// From [`Protection::kdf`].
    pub fn from_array(a: [u32; 3]) -> KdfParams {
        KdfParams { m_kib: a[0], t: a[1], p: a[2] }
    }

    fn checked(self) -> Result<KdfParams> {
        let ok = (8..=MAX_MEMORY_KIB).contains(&self.m_kib)
            && (1..=MAX_PASSES).contains(&self.t)
            && (1..=MAX_LANES).contains(&self.p)
            && self.m_kib >= 8 * self.p;
        if ok { Ok(self) } else { Err(corrupt("the section's key parameters are out of range")) }
    }
}

/// What a sealed file says in the clear.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SealedHeader {
    /// `notecraft-sealed`.
    pub format: String,
    /// [`SEALED_VERSION`].
    pub version: u32,
    /// The section's id.
    pub id: Id,
    /// Its name (the tab shows it while locked).
    pub name: String,
    /// Its tab colour.
    #[serde(default)]
    pub color: SectionColor,
    /// When it was created.
    #[serde(default)]
    pub created: Timestamp,
    /// Key derivation costs (Argon2id).
    pub kdf: KdfParams,
    /// The salt, base64.
    pub salt: String,
    /// The nonce, base64.
    pub nonce: String,
}

/// Whether `bytes` are a sealed section file.
pub fn is_sealed(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

/// Derive the key for `password` with `salt` and `params` (slow on purpose).
pub fn derive_key(password: &str, salt: &[u8], params: KdfParams) -> Result<Key> {
    let params = params.checked()?;
    let p = argon2::Params::new(params.m_kib, params.t, params.p, Some(32)).map_err(|e| corrupt(format!("key parameters: {e}")))?;
    let a = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, p);
    let mut key = Zeroizing::new([0u8; 32]);
    a.hash_password_into(password.as_bytes(), salt, key.as_mut()).map_err(|e| corrupt(format!("key derivation: {e}")))?;
    Ok(key)
}

/// Read the header of a sealed file.
pub fn read_header(bytes: &[u8]) -> Result<(SealedHeader, usize)> {
    let rest = bytes.strip_prefix(MAGIC).ok_or_else(|| corrupt("not a sealed section"))?;
    let len_bytes: [u8; 4] = rest.get(..4).and_then(|b| b.try_into().ok()).ok_or_else(|| corrupt("sealed section: truncated"))?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    if len > MAX_HEADER {
        return Err(corrupt("sealed section: header too large"));
    }
    let start = MAGIC.len() + 4;
    let raw = bytes.get(start..start + len).ok_or_else(|| corrupt("sealed section: truncated header"))?;
    let header: SealedHeader = serde_json::from_slice(raw).map_err(|e| corrupt(format!("sealed section header: {e}")))?;
    if header.format != HEADER_FORMAT {
        return Err(corrupt("not a NoteCraft sealed section"));
    }
    if header.version > SEALED_VERSION {
        return Err(FormatError::Corrupt(format!("sealed section version {} is newer than this NoteCraft reads ({SEALED_VERSION})", header.version)));
    }
    header.kdf.checked()?;
    Ok((header, start + len))
}

/// The locked section a sealed file shows: id, name, colour and date, no pages, and its
/// [`Protection`] holding the file.
pub fn locked_section(bytes: &[u8]) -> Result<Section> {
    let (header, _) = read_header(bytes)?;
    let salt = salt_of(&header)?;
    let mut s = Section::new(header.id, &header.name, header.color, header.created);
    s.protection = Some(Protection { salt, kdf: header.kdf.to_array(), locked: Some(Arc::from(bytes)) });
    Ok(s)
}

/// The salt of a sealed file, decoded.
pub fn salt_of(header: &SealedHeader) -> Result<Vec<u8>> {
    notecraft_codecs::b64::decode(&header.salt).filter(|s| s.len() >= 8).ok_or_else(|| corrupt("sealed section: bad salt"))
}

/// Open a sealed file with `key`: the section inside. A wrong key (or a changed file) is
/// [`FormatError::WrongPassword`].
pub fn open(bytes: &[u8], key: &Key) -> Result<Section> {
    let (header, at) = read_header(bytes)?;
    let nonce = notecraft_codecs::b64::decode(&header.nonce).filter(|n| n.len() == 24).ok_or_else(|| corrupt("sealed section: bad nonce"))?;
    let aad = bytes.get(MAGIC.len() + 4..at).unwrap_or_default();
    let ct = bytes.get(at..).unwrap_or_default();
    let cipher = XChaCha20Poly1305::new(key.as_ref().into());
    let plain = Zeroizing::new(cipher.decrypt(XNonce::from_slice(&nonce), Payload { msg: ct, aad }).map_err(|_| FormatError::WrongPassword)?);
    let mut section = crate::decode_section(&plain)?;
    // The clear header is what the tab showed; the sealed copy is the truth, except that an
    // id must match (a header swapped onto another file fails the tag check anyway).
    if section.id != header.id {
        return Err(corrupt("sealed section: id mismatch"));
    }
    section.normalize_pages();
    let salt = salt_of(&header)?;
    section.protection = Some(Protection { salt, kdf: header.kdf.to_array(), locked: None });
    Ok(section)
}

/// Seal `section` with `key` (derived with the salt and costs of its [`Protection`]), using
/// `nonce` (24 random bytes, fresh for every write). The section's own JSON inside carries no
/// protection; its pages are sealed with everything else.
pub fn seal(section: &Section, key: &Key, nonce: [u8; 24]) -> Result<Vec<u8>> {
    let prot = section.protection.as_ref().ok_or_else(|| corrupt("the section has no password"))?;
    let (salt, kdf) = (prot.salt.as_slice(), KdfParams::from_array(prot.kdf));
    let header = SealedHeader {
        format: HEADER_FORMAT.into(),
        version: SEALED_VERSION,
        id: section.id,
        name: section.name.clone(),
        color: section.color,
        created: section.created,
        kdf,
        salt: notecraft_codecs::b64::encode(salt),
        nonce: notecraft_codecs::b64::encode(&nonce),
    };
    let head = serde_json::to_vec(&header).map_err(|e| corrupt(e.to_string()))?;
    let plain = Zeroizing::new(crate::encode_section_plain(section)?);
    let cipher = XChaCha20Poly1305::new(key.as_ref().into());
    let ct = cipher
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: plain.as_slice(), aad: &head })
        .map_err(|_| corrupt("could not seal the section"))?;
    let len = u32::try_from(head.len()).map_err(|_| corrupt("sealed header too large"))?;
    let mut out = Vec::with_capacity(MAGIC.len() + 4 + head.len() + ct.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&head);
    out.extend_from_slice(&ct);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section() -> Section {
        let mut s = Section::new(Id(7), "Diary", SectionColor::index(2), Timestamp(1_700_000_000_000));
        let mut page = notecraft_doc::Page::new(Id(8), Id(9), Timestamp(1_700_000_000_000), "me");
        page.title = notecraft_doc::Paragraph::with_text(Id(9), "Secret plans");
        s.pages.push(Arc::new(page));
        s
    }

    fn protect(s: &mut Section) -> Key {
        let salt = vec![3u8; 16];
        let key = derive_key("correct horse", &salt, KdfParams::TEST).unwrap();
        s.protection = Some(Protection { salt, kdf: KdfParams::TEST.to_array(), locked: None });
        key
    }

    #[test]
    fn a_sealed_section_opens_only_with_its_password() {
        let mut s = section();
        let key = protect(&mut s);
        let bytes = seal(&s, &key, [5; 24]).unwrap();
        assert!(is_sealed(&bytes));
        assert!(!bytes.windows(12).any(|w| w == b"Secret plans"), "no page text in the clear");
        // Opened from disk it is locked: name and colour only.
        let locked = crate::decode_section(&bytes).unwrap();
        assert!(locked.is_locked() && locked.pages.is_empty());
        assert_eq!((locked.id, locked.name.as_str(), locked.color), (Id(7), "Diary", SectionColor::index(2)));
        // Written back unchanged while locked.
        assert_eq!(crate::encode_section(&locked).unwrap(), bytes);
        // The right password opens it, a wrong one doesn't.
        let prot = locked.protection.clone().unwrap();
        let again = derive_key("correct horse", &prot.salt, KdfParams::from_array(prot.kdf)).unwrap();
        let opened = open(&bytes, &again).unwrap();
        assert_eq!(opened.pages[0].title.text(), "Secret plans");
        assert!(opened.protection.as_ref().is_some_and(|p| p.locked.is_none()));
        let wrong = derive_key("Correct horse", &prot.salt, KdfParams::from_array(prot.kdf)).unwrap();
        assert!(matches!(open(&bytes, &wrong), Err(FormatError::WrongPassword)));
    }

    #[test]
    fn a_protected_section_is_never_written_in_the_clear() {
        let mut s = section();
        let _ = protect(&mut s);
        assert!(matches!(crate::encode_section(&s), Err(FormatError::NeedsKey(_))));
    }

    #[test]
    fn changed_or_hostile_sealed_files_are_refused() {
        let mut s = section();
        let key = protect(&mut s);
        let bytes = seal(&s, &key, [1; 24]).unwrap();
        // A renamed header no longer matches the sealed content.
        let renamed = String::from_utf8_lossy(&bytes).replace("Diary", "Dairy");
        let renamed = renamed.as_bytes();
        if renamed.len() == bytes.len() {
            assert!(open(renamed, &key).is_err());
        }
        // A flipped ciphertext byte.
        let mut flipped = bytes.clone();
        if let Some(b) = flipped.last_mut() {
            *b ^= 1;
        }
        assert!(matches!(open(&flipped, &key), Err(FormatError::WrongPassword)));
        // Truncations, absurd header sizes and costs: errors, never panics.
        for n in [0, 5, MAGIC.len(), MAGIC.len() + 3, MAGIC.len() + 10, bytes.len() / 2] {
            let _ = crate::decode_section(&bytes[..n.min(bytes.len())]);
            let _ = open(&bytes[..n.min(bytes.len())], &key);
        }
        let mut huge = MAGIC.to_vec();
        huge.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(crate::decode_section(&huge).is_err());
        let costly = String::from_utf8_lossy(&bytes).replace("\"m_kib\":64", "\"m_kib\":99999999");
        assert!(crate::decode_section(costly.as_bytes()).is_err(), "a crafted memory cost is refused");
        assert!(derive_key("x", &[0; 16], KdfParams { m_kib: u32::MAX, t: 1, p: 1 }).is_err());
    }
}
