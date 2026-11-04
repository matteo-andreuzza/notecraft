//! [MS-ONESTORE] §2.6.1 ObjectSpaceObjectPropSet and §2.6.7 PropertySet: an object's
//! properties, with its object, object space and context references resolved to extended GUIDs.
//!
//! The decoding is generic: each value is read by the `type` stored in its PropertyID (§2.6.6),
//! so properties this reader does not know are still stepped over correctly. [MS-ONE] gives the
//! meaning of the ids ([`crate::one::props`]).

use std::collections::HashMap;

use crate::bytes::Cursor;
use crate::error::{OneError, Result};
use crate::types::{CompactId, ExtendedGuid, Guid};

/// [MS-ONESTORE] §2.6.6 PropertyID: `id` (26 bits), `type` (5 bits), `boolValue` (1 bit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PropertyId(pub u32);

impl PropertyId {
    /// The 26-bit identity ([MS-ONE] §2.1.12 lists them with their type bits, e.g. 0x1C001CF3).
    pub fn id(self) -> u32 {
        self.0 & 0x03FF_FFFF
    }

    /// The 5-bit type.
    pub fn kind(self) -> u8 {
        ((self.0 >> 26) & 0x1F) as u8
    }

    /// `boolValue`.
    pub fn bool_value(self) -> bool {
        self.0 >> 31 != 0
    }
}

/// A property value (§2.6.6 `type`).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// 0x1 NoData.
    NoData,
    /// 0x2 Bool.
    Bool(bool),
    /// 0x3 OneByteOfData.
    U8(u8),
    /// 0x4 TwoBytesOfData.
    U16(u16),
    /// 0x5 FourBytesOfData.
    U32(u32),
    /// 0x6 EightBytesOfData.
    U64(u64),
    /// 0x7 FourBytesOfLengthFollowedByData (§2.6.8).
    Bytes(Vec<u8>),
    /// 0x8 ObjectID (`None`: the nil id or one missing from the global id table).
    Object(Option<ExtendedGuid>),
    /// 0x9 ArrayOfObjectIDs.
    Objects(Vec<ExtendedGuid>),
    /// 0xA ObjectSpaceID.
    Space(Option<ExtendedGuid>),
    /// 0xB ArrayOfObjectSpaceIDs.
    Spaces(Vec<ExtendedGuid>),
    /// 0xC ContextID.
    Context(Option<ExtendedGuid>),
    /// 0xD ArrayOfContextIDs.
    Contexts(Vec<ExtendedGuid>),
    /// 0x10 ArrayOfPropertyValues (§2.6.9): property sets.
    Array(Vec<PropertySet>),
    /// 0x11 PropertySet.
    Set(PropertySet),
}

/// A decoded property set: properties in stored order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PropertySet {
    /// `(PropertyID, value)` pairs.
    pub props: Vec<(PropertyId, Value)>,
}

impl PropertySet {
    /// The value of the property whose 26-bit id equals `id`'s ([MS-ONE] values may be passed
    /// with their type bits).
    pub fn get(&self, id: u32) -> Option<&Value> {
        let want = PropertyId(id).id();
        self.props.iter().find(|(p, _)| p.id() == want).map(|(_, v)| v)
    }

    /// A Bool property (absent: `None`).
    pub fn bool(&self, id: u32) -> Option<bool> {
        match self.get(id)? {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// An integer property of any fixed size, widened.
    pub fn uint(&self, id: u32) -> Option<u64> {
        match self.get(id)? {
            Value::U8(v) => Some(u64::from(*v)),
            Value::U16(v) => Some(u64::from(*v)),
            Value::U32(v) => Some(u64::from(*v)),
            Value::U64(v) => Some(*v),
            _ => None,
        }
    }

    /// A FourBytesOfData property read as an IEEE float ([MS-ONE] layout values).
    pub fn f32(&self, id: u32) -> Option<f32> {
        match self.get(id)? {
            Value::U32(v) => Some(f32::from_bits(*v)).filter(|f| f.is_finite()),
            _ => None,
        }
    }

    /// The bytes of a FourBytesOfLengthFollowedByData property.
    pub fn bytes(&self, id: u32) -> Option<&[u8]> {
        match self.get(id)? {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// The objects of an ObjectID or ArrayOfObjectIDs property (empty when absent).
    pub fn objects(&self, id: u32) -> Vec<ExtendedGuid> {
        match self.get(id) {
            Some(Value::Objects(v)) => v.clone(),
            Some(Value::Object(Some(o))) => vec![*o],
            _ => Vec::new(),
        }
    }

    /// The object spaces of an ObjectSpaceID or ArrayOfObjectSpaceIDs property.
    pub fn spaces(&self, id: u32) -> Vec<ExtendedGuid> {
        match self.get(id) {
            Some(Value::Spaces(v)) => v.clone(),
            Some(Value::Space(Some(o))) => vec![*o],
            _ => Vec::new(),
        }
    }
}

/// A global identification table (§2.1.3): GUID by index.
pub type GlobalIdTable = HashMap<u32, Guid>;

/// Resolves a compact id with `table` (§2.2.2): `None` for the nil id or a missing index.
pub(crate) fn resolve(table: &GlobalIdTable, id: CompactId) -> Option<ExtendedGuid> {
    if id.is_nil() {
        return None;
    }
    table.get(&id.guid_index).map(|g| ExtendedGuid::new(*g, u32::from(id.n)))
}

/// The three reference streams of §2.6.1, consumed in property order.
struct Streams {
    oids: std::vec::IntoIter<CompactId>,
    osids: std::vec::IntoIter<CompactId>,
    contexts: std::vec::IntoIter<CompactId>,
}

/// §2.6.5 ObjectSpaceObjectStreamHeader: `Count` (24 bits), reserved (6),
/// `ExtendedStreamsPresent`, `OsidStreamNotPresent`; then `Count` compact ids.
fn read_stream(c: &mut Cursor<'_>, what: &str) -> Result<(Vec<CompactId>, bool, bool)> {
    let h = c.u32(what)?;
    let count = (h & 0x00FF_FFFF) as usize;
    let extended = h & (1 << 30) != 0;
    let no_osids = h & (1 << 31) != 0;
    // Each id is 4 bytes: a count larger than what is left is forged.
    if count > c.remaining() / 4 {
        return Err(OneError::corrupt(c.at(), format!("{what} claims {count} ids")));
    }
    let ids = (0..count).map(|_| CompactId::read(c, what)).collect::<Result<Vec<_>>>()?;
    Ok((ids, extended, no_osids))
}

/// Decodes an ObjectSpaceObjectPropSet (§2.6.1) at file offset `origin`.
pub fn decode(data: &[u8], origin: u64, table: &GlobalIdTable, max_depth: usize) -> Result<PropertySet> {
    let mut c = Cursor::new(data, origin);
    let (oids, _, no_osids) = read_stream(&mut c, "ObjectSpaceObjectStreamOfOIDs")?;
    let (mut osids, mut contexts) = (Vec::new(), Vec::new());
    if !no_osids {
        let (ids, extended, _) = read_stream(&mut c, "ObjectSpaceObjectStreamOfOSIDs")?;
        osids = ids;
        if extended {
            contexts = read_stream(&mut c, "ObjectSpaceObjectStreamOfContextIDs")?.0;
        }
    }
    let mut streams = Streams { oids: oids.into_iter(), osids: osids.into_iter(), contexts: contexts.into_iter() };
    // Trailing padding (≤ 7 zero bytes) is ignored.
    read_set(&mut c, &mut streams, table, max_depth)
}

/// §2.6.7 PropertySet: `cProperties`, `rgPrids`, then `rgData`.
fn read_set(c: &mut Cursor<'_>, s: &mut Streams, table: &GlobalIdTable, depth: usize) -> Result<PropertySet> {
    if depth == 0 {
        return Err(OneError::TooLarge("property sets nested too deeply".into()));
    }
    let n = usize::from(c.u16("PropertySet.cProperties")?);
    if n > c.remaining() / 4 {
        return Err(OneError::corrupt(c.at(), format!("a property set claims {n} properties")));
    }
    let prids = (0..n).map(|_| c.u32("PropertySet.rgPrids").map(PropertyId)).collect::<Result<Vec<_>>>()?;
    let mut props = Vec::with_capacity(n);
    for p in prids {
        let v = read_value(c, p, s, table, depth)?;
        props.push((p, v));
    }
    Ok(PropertySet { props })
}

fn read_value(c: &mut Cursor<'_>, p: PropertyId, s: &mut Streams, table: &GlobalIdTable, depth: usize) -> Result<Value> {
    let at = c.at();
    let next = |it: &mut std::vec::IntoIter<CompactId>, what: &str| {
        it.next().ok_or_else(|| OneError::corrupt(at, format!("property {:#010X} needs more {what} than the object lists", p.0)))
    };
    let many = |c: &mut Cursor<'_>, it: &mut std::vec::IntoIter<CompactId>, what: &str| -> Result<Vec<ExtendedGuid>> {
        let n = c.u32("array count")? as usize;
        if n > it.len() {
            return Err(OneError::corrupt(at, format!("property {:#010X} needs {n} {what}, the object lists {}", p.0, it.len())));
        }
        Ok(it.by_ref().take(n).filter_map(|id| resolve(table, id)).collect())
    };
    Ok(match p.kind() {
        0x1 => Value::NoData,
        0x2 => Value::Bool(p.bool_value()),
        0x3 => Value::U8(c.u8("property data")?),
        0x4 => Value::U16(c.u16("property data")?),
        0x5 => Value::U32(c.u32("property data")?),
        0x6 => Value::U64(c.u64("property data")?),
        0x7 => {
            // §2.6.8: cb MUST be less than 0x40000000; the cursor also bounds it by the data left.
            let cb = c.u32("prtFourBytesOfLengthFollowedByData.cb")?;
            Value::Bytes(c.take(cb as usize, "property data")?.to_vec())
        }
        0x8 => Value::Object(resolve(table, next(&mut s.oids, "object ids")?)),
        0x9 => Value::Objects(many(c, &mut s.oids, "object ids")?),
        0xA => Value::Space(resolve(table, next(&mut s.osids, "object space ids")?)),
        0xB => Value::Spaces(many(c, &mut s.osids, "object space ids")?),
        0xC => Value::Context(resolve(table, next(&mut s.contexts, "context ids")?)),
        0xD => Value::Contexts(many(c, &mut s.contexts, "context ids")?),
        0x10 => {
            // §2.6.9 prtArrayOfPropertyValues: cProperties, then (if any) prid and the sets.
            let n = c.u32("prtArrayOfPropertyValues.cProperties")? as usize;
            let mut sets = Vec::new();
            if n > 0 {
                let prid = PropertyId(c.u32("prtArrayOfPropertyValues.prid")?);
                if prid.kind() != 0x11 {
                    return Err(OneError::corrupt(at, "an array of property values must hold property sets"));
                }
                // Each set takes at least its 2-byte count.
                if n > c.remaining() / 2 {
                    return Err(OneError::corrupt(at, format!("an array claims {n} property sets")));
                }
                for _ in 0..n {
                    sets.push(read_set(c, s, table, depth - 1)?);
                }
            }
            Value::Array(sets)
        }
        0x11 => Value::Set(read_set(c, s, table, depth - 1)?),
        t => return Err(OneError::corrupt(at, format!("property {:#010X} has unknown type {t:#X}", p.0))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::props::{PropSetBuilder, Val};

    fn guid(n: u32) -> Guid {
        Guid::new(n, 0, 0, [0; 8])
    }

    #[test]
    fn every_value_type_round_trips() {
        let table: GlobalIdTable = [(1, guid(1)), (2, guid(2))].into_iter().collect();
        let a = CompactId { n: 3, guid_index: 1 };
        let b = CompactId { n: 4, guid_index: 2 };
        let mut child = PropSetBuilder::default();
        child.add(0x14001C0B, Val::U32(7));
        child.add(0x20001C3F, Val::Object(b));
        let mut set = PropSetBuilder::default();
        set.add(0x08001C04, Val::Bool(true));
        set.add(0x0C001C03, Val::U8(2));
        set.add(0x10001C0B, Val::U16(22));
        set.add(0x14001C14, Val::U32(1.5f32.to_bits()));
        set.add(0x18001C65, Val::U64(9));
        set.add(0x1C001CF3, Val::Bytes(b"t\0".to_vec()));
        set.add(0x20001C3F, Val::Object(a));
        set.add(0x24001C20, Val::Objects(vec![a, b]));
        set.add(0x2C001D63, Val::Spaces(vec![b]));
        set.add(0x40003499, Val::Array(vec![child.clone(), child]));
        set.add(0x04003489, Val::NoData);
        let bytes = set.encode();
        let p = decode(&bytes, 0, &table, 8).unwrap();
        let ea = ExtendedGuid::new(guid(1), 3);
        let eb = ExtendedGuid::new(guid(2), 4);
        assert_eq!(p.bool(0x08001C04), Some(true));
        assert_eq!(p.uint(0x0C001C03), Some(2));
        assert_eq!(p.uint(0x10001C0B), Some(22));
        assert_eq!(p.f32(0x14001C14), Some(1.5));
        assert_eq!(p.uint(0x18001C65), Some(9));
        assert_eq!(p.bytes(0x1C001CF3), Some(&b"t\0"[..]));
        assert_eq!(p.objects(0x20001C3F), vec![ea]);
        assert_eq!(p.objects(0x24001C20), vec![ea, eb]);
        assert_eq!(p.spaces(0x2C001D63), vec![eb]);
        match p.get(0x40003499) {
            Some(Value::Array(sets)) => {
                assert_eq!(sets.len(), 2);
                assert_eq!(sets[1].objects(0x20001C3F), vec![eb]);
                assert_eq!(sets[0].uint(0x14001C0B), Some(7));
            }
            v => panic!("{v:?}"),
        }
        assert_eq!(p.get(0x04003489), Some(&Value::NoData));
        assert_eq!(p.get(0x0BAD), None);
    }

    #[test]
    fn forged_counts_are_corrupt_not_huge() {
        let table = GlobalIdTable::new();
        // OIDs header claiming 2^24 - 1 ids, osids absent.
        let bytes = (0x80FF_FFFFu32).to_le_bytes();
        assert!(decode(&bytes, 0, &table, 8).is_err());
        // An ArrayOfObjectIDs property wanting more ids than the stream has.
        let mut set = PropSetBuilder::default();
        set.add(0x24001C20, Val::Objects(vec![]));
        let mut bytes = set.encode();
        let n = bytes.len();
        bytes[n - 4..].copy_from_slice(&5u32.to_le_bytes());
        assert!(decode(&bytes, 0, &table, 8).is_err());
    }

    #[test]
    fn nesting_is_bounded() {
        let mut set = PropSetBuilder::default();
        set.add(0x08001C04, Val::Bool(false));
        for _ in 0..10 {
            let mut outer = PropSetBuilder::default();
            outer.add(0x44000001, Val::Set(set));
            set = outer;
        }
        let bytes = set.encode();
        assert!(matches!(decode(&bytes, 0, &GlobalIdTable::new(), 4), Err(OneError::TooLarge(_))));
        assert!(decode(&bytes, 0, &GlobalIdTable::new(), 16).is_ok());
    }
}
