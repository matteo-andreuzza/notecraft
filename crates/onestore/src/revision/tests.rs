use super::*;
use crate::writer::node;
use crate::writer::props::{PropSetBuilder, Val};
use crate::writer::store::StoreBuilder;

fn x(n: u32) -> ExtendedGuid {
    ExtendedGuid::new(Guid::new(0xC0DE_0000 + n, 7, 7, [1; 8]), n)
}

const JCID: u32 = 0x0006_000B; // [MS-ONE] jcidPageNode, any property-set type will do
const SIZE: u32 = 0x10001C0B; // [MS-ONE] FontSize

fn props(v: u16) -> PropSetBuilder {
    let mut p = PropSetBuilder::default();
    p.add(SIZE, Val::U16(v));
    p
}

fn read(file: &[u8]) -> Store<'_> {
    Store::read(file, &Limits::default()).unwrap()
}

#[test]
fn a_space_reads_its_objects_roots_and_references() {
    let mut b = StoreBuilder::new(FileKind::Section, x(1));
    let rev = b.space(x(1)).revision(x(10), ExtendedGuid::NIL);
    let child = rev.cid(x(21));
    let mut parent = props(1);
    parent.add(0x24001C20, Val::Objects(vec![child]));
    rev.object(x(20), JCID, &parent).object(x(21), JCID, &props(2)).root(x(20), ROOT_CONTENT);
    let file = b.finish();
    let s = read(&file);
    assert_eq!(s.root, x(1));
    let space = s.root_space().unwrap();
    let root = space.root(ROOT_CONTENT).unwrap();
    assert_eq!(root.jcid, Jcid(JCID));
    let ObjectData::Props(p) = &root.data else { panic!() };
    assert_eq!(p.objects(0x24001C20), vec![x(21)]);
    assert_eq!(space.props(&x(21)).unwrap().uint(SIZE), Some(2));
    assert!(s.warnings.is_empty(), "{:?}", s.warnings);
}

#[test]
fn a_dependent_revision_adds_to_and_overrides_its_dependency() {
    let mut b = StoreBuilder::new(FileKind::Section, x(1));
    let space = b.space(x(1));
    space.revision(x(10), ExtendedGuid::NIL).object(x(20), JCID, &props(1)).object(x(21), JCID, &props(2)).root(x(20), ROOT_CONTENT);
    space.revision(x(11), x(10)).object(x(21), JCID, &props(3)).object(x(22), JCID, &props(4)).root(x(22), ROOT_METADATA);
    let file = b.finish();
    let s = read(&file);
    let sp = s.root_space().unwrap();
    assert_eq!(sp.objects.len(), 3);
    assert_eq!(sp.props(&x(21)).unwrap().uint(SIZE), Some(3), "the later revision wins");
    assert_eq!(sp.roots.get(&ROOT_CONTENT), Some(&x(20)), "roots of the dependency are kept");
    assert_eq!(sp.roots.get(&ROOT_METADATA), Some(&x(22)));
}

#[test]
fn a_role_declaration_moves_the_current_revision() {
    let mut b = StoreBuilder::new(FileKind::Section, x(1));
    let space = b.space(x(1));
    space.revision(x(10), ExtendedGuid::NIL).object(x(20), JCID, &props(1)).root(x(20), ROOT_CONTENT);
    space.revision(x(11), ExtendedGuid::NIL).object(x(20), JCID, &props(9)).root(x(20), ROOT_CONTENT);
    // §2.5.17: label the first revision again as the default one.
    space.extra(node::revision_role(x(10), ROLE_DEFAULT));
    let file = b.finish();
    assert_eq!(read(&file).root_space().unwrap().props(&x(20)).unwrap().uint(SIZE), Some(1));
}

#[test]
fn revisions_of_other_roles_are_not_current() {
    let mut b = StoreBuilder::new(FileKind::Section, x(1));
    let space = b.space(x(1));
    space.revision(x(10), ExtendedGuid::NIL).object(x(20), JCID, &props(1)).root(x(20), ROOT_CONTENT);
    space.revision_with_role(x(11), ExtendedGuid::NIL, 4).object(x(20), JCID, &props(9)).root(x(20), ROOT_CONTENT);
    let file = b.finish();
    assert_eq!(read(&file).root_space().unwrap().props(&x(20)).unwrap().uint(SIZE), Some(1));
}

#[test]
fn a_dependency_loop_stops() {
    let mut b = StoreBuilder::new(FileKind::Section, x(1));
    let space = b.space(x(1));
    space.revision(x(10), x(11)).object(x(20), JCID, &props(1)).root(x(20), ROOT_CONTENT);
    space.revision(x(11), x(10)).object(x(21), JCID, &props(2));
    space.extra(node::revision_role(x(10), ROLE_DEFAULT));
    let file = b.finish();
    let s = read(&file);
    assert_eq!(s.root_space().unwrap().objects.len(), 2);
}

#[test]
fn a_damaged_object_is_reported_not_fatal() {
    let mut b = StoreBuilder::new(FileKind::Section, x(1));
    let rev = b.space(x(1)).revision(x(10), ExtendedGuid::NIL);
    rev.object(x(20), JCID, &props(1)).raw_object(x(21), JCID, vec![0xFF, 0xFF, 0xFF, 0x00]).root(x(20), ROOT_CONTENT);
    let file = b.finish();
    let s = read(&file);
    let sp = s.root_space().unwrap();
    assert_eq!(sp.objects.get(&x(21)).unwrap().data, ObjectData::Damaged);
    assert_eq!(s.warnings.len(), 1);
    assert!(sp.props(&x(20)).is_some());
}

#[test]
fn encrypted_spaces_are_reported() {
    let mut b = StoreBuilder::new(FileKind::Section, x(1));
    b.space(x(1)).revision(x(10), ExtendedGuid::NIL).encrypted().object(x(20), JCID, &props(1));
    let file = b.finish();
    assert!(matches!(Store::read(&file, &Limits::default()), Err(OneError::Encrypted)));
}

#[test]
fn file_data_objects_find_their_bytes() {
    let mut b = StoreBuilder::new(FileKind::Section, x(1));
    let picture = [0x89, b'P', b'N', b'G', 1, 2, 3];
    let r = b.file_data(Guid::new(0xF11E, 1, 2, [3; 8]), &picture);
    b.space(x(1)).revision(x(10), ExtendedGuid::NIL).file_object(x(30), 0x0008_0036, &r, ".png");
    let file = b.finish();
    let s = read(&file);
    let o = s.root_space().unwrap().objects.get(&x(30)).unwrap();
    let ObjectData::File { reference, extension } = &o.data else { panic!() };
    assert_eq!(extension, ".png");
    assert_eq!(s.file_bytes(reference), Some(&picture[..]));
    assert_eq!(s.file_bytes("<file>beside.bin"), None);
    assert_eq!(s.file_bytes("<ifndf>{00000000-0000-0000-0000-000000000000}</ifndf>"), None);
}

#[test]
fn too_large_files_are_refused_before_reading() {
    let file = StoreBuilder::new(FileKind::Section, x(1)).finish();
    let limits = Limits { max_file_bytes: 100, ..Limits::default() };
    assert!(matches!(Store::read(&file, &limits), Err(OneError::TooLarge(_))));
}

#[test]
fn a_file_without_a_root_space_is_corrupt() {
    let mut b = crate::writer::FileBuilder::new(FileKind::Section);
    let osml = b.list(&[node::object_space_manifest_list_start(x(1))]);
    let file = b.finish(&[node::object_space_manifest_list_reference(osml, x(1))]);
    assert!(matches!(Store::read(&file, &Limits::default()), Err(OneError::Corrupt { .. })));
}
