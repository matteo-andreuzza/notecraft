//! Stable object ids.
//!
//! Every notebook, section group, section, page, note container and paragraph has an [`Id`]
//! that never changes for the object's lifetime: links ("Copy Link to Page/Paragraph"), the
//! selection, search results and undo all refer to objects by id, so renaming or reordering
//! never breaks them (catalog `BEH.Links.PageRenameTracking`).
//!
//! An id is 128 bits: the high 64 bits name the *replica* that created it (one per session,
//! random), the low 64 bits are that replica's counter. Two replicas never need to coordinate
//! to mint unique ids, which is what a CRDT-based sync (M4) needs; the shape also mirrors the
//! reference format's ExtendedGUID (a GUID plus a counter, [MS-ONESTORE] §2.2.1), so ids from
//! `.one` files map onto it (M3).

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A stable, globally unique object id. Serialized as 32 lowercase hex digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Id(pub u128);

impl Id {
    /// The nil id (never minted by [`IdGen`]); used for "no object".
    pub const NIL: Id = Id(0);

    /// Whether this is [`Id::NIL`].
    pub fn is_nil(self) -> bool {
        self.0 == 0
    }

    /// Parse 32 hex digits (case-insensitive). Anything else is `None`.
    pub fn parse(s: &str) -> Option<Id> {
        let s = s.trim();
        if s.len() != 32 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u128::from_str_radix(s, 16).ok().map(Id)
    }

    /// The replica half.
    pub fn replica(self) -> u64 {
        (self.0 >> 64) as u64
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Id, D::Error> {
        let s = String::deserialize(d)?;
        Id::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("bad id `{s}`")))
    }
}

/// Mints ids for one replica.
///
/// Invariant: the replica half is never 0 (so no minted id equals [`Id::NIL`]) and the counter
/// only grows, so ids from one generator are unique.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdGen {
    replica: u64,
    next: u64,
}

impl IdGen {
    /// A generator for `replica` (0 is replaced by 1 to keep ids non-nil).
    pub fn new(replica: u64) -> IdGen {
        IdGen { replica: replica.max(1), next: 1 }
    }

    /// A generator whose replica half comes from the process's hash seed: random on native
    /// targets (the standard library seeds `RandomState` from the OS), fixed on
    /// `wasm32-unknown-unknown`, where the web host should call [`IdGen::new`] with randomness
    /// from the browser instead.
    pub fn from_entropy() -> IdGen {
        use std::hash::{BuildHasher, Hasher};
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(0x4e6f_7465_4372_6166); // "NoteCraf"
        IdGen::new(h.finish())
    }

    /// The next id.
    pub fn next_id(&mut self) -> Id {
        let n = self.next;
        self.next = self.next.wrapping_add(1).max(1);
        Id(((self.replica as u128) << 64) | n as u128)
    }

    /// The replica this generator mints for.
    pub fn replica(&self) -> u64 {
        self.replica
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_round_trip() {
        let mut g = IdGen::new(7);
        let a = g.next_id();
        let b = g.next_id();
        assert_ne!(a, b);
        assert!(!a.is_nil());
        assert_eq!(a.replica(), 7);
        assert_eq!(Id::parse(&a.to_string()), Some(a));
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(serde_json::from_str::<Id>(&json).unwrap(), a);
    }

    #[test]
    fn hostile_id_strings_rejected() {
        for bad in ["", "xyz", "0123", &"g".repeat(32), &"0".repeat(33), "+0000000000000000000000000000001"] {
            assert_eq!(Id::parse(bad), None, "{bad}");
        }
        assert!(serde_json::from_str::<Id>("\"nope\"").is_err());
    }

    #[test]
    fn replica_zero_never_mints_nil() {
        let mut g = IdGen::new(0);
        assert!(!g.next_id().is_nil());
    }
}
