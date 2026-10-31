use super::*;
use crate::writer::cab::{Compression, blocks_cab, write};

fn read(file: &[u8]) -> Cabinet {
    read_cabinet(file, &CabLimits::default()).unwrap()
}

fn names(cab: &Cabinet) -> Vec<String> {
    cab.files.iter().map(|f| f.path.join("/")).collect()
}

const HELLO: &[u8] = b"#include <stdio.h>\r\n\r\nvoid main(void)\r\n{\r\n    printf(\"Hello, world!\\n\");\r\n}\r\n";
const WELCOME: &[u8] = b"#include <stdio.h>\r\n\r\nvoid main(void)\r\n{\r\n    printf(\"Welcome!\\n\");\r\n}\r\n\r\n";

/// [MS-CAB] §3: the specification's own example, byte for byte.
#[test]
fn the_specification_example_reads() {
    let mut file = vec![
        0x4D, 0x53, 0x43, 0x46, 0x00, 0x00, 0x00, 0x00, 0xFD, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x2C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, //
        0x22, 0x06, 0x00, 0x00, 0x5E, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x4D, 0x00, 0x00, 0x00, //
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x6C, 0x22, 0xBA, 0x59, 0x20, 0x00, 0x68, 0x65, 0x6C, 0x6C, //
        0x6F, 0x2E, 0x63, 0x00, 0x4A, 0x00, 0x00, 0x00, 0x4D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x6C, 0x22, //
        0xE7, 0x59, 0x20, 0x00, 0x77, 0x65, 0x6C, 0x63, 0x6F, 0x6D, 0x65, 0x2E, 0x63, 0x00, 0xBD, 0x5A, //
        0xA6, 0x30, 0x97, 0x00, 0x97, 0x00,
    ];
    file.extend(HELLO);
    file.extend(WELCOME);
    assert_eq!(file.len(), 0xFD);
    let cab = read(&file);
    assert_eq!(names(&cab), vec!["hello.c", "welcome.c"], "FMT.Onepkg.Read");
    assert_eq!(cab.files[0].data, HELLO);
    assert_eq!(cab.files[1].data, WELCOME);
    assert!(cab.warnings.is_empty());
}

#[test]
fn mszip_folders_over_many_blocks_read_back() {
    let big: Vec<u8> = (0..100_000u32).map(|i| (i * 7 % 251) as u8).collect();
    let files: [(&str, &[u8]); 3] = [("Open Notebook.onetoc2", b"toc"), ("Work\\Caf\u{e9}.one", &big), ("empty.one", b"")];
    for compression in [Compression::None, Compression::MsZip] {
        let cab = read(&write(&files, compression));
        assert_eq!(names(&cab), vec!["Open Notebook.onetoc2", "Work/Caf\u{e9}.one", "empty.one"], "{compression:?}");
        assert_eq!(cab.files[1].data, big);
        assert!(cab.files[2].data.is_empty());
    }
}

/// Writes bits least significant first (RFC 1951 §3.1.1); Huffman codes go most significant
/// bit first.
#[derive(Default)]
struct Bits {
    out: Vec<u8>,
    n: usize,
}

impl Bits {
    fn bits(&mut self, value: u32, count: usize) {
        for i in 0..count {
            if self.n.is_multiple_of(8) {
                self.out.push(0);
            }
            *self.out.last_mut().unwrap() |= (((value >> i) & 1) as u8) << (self.n % 8);
            self.n += 1;
        }
    }
    fn code(&mut self, code: u32, count: usize) {
        for i in (0..count).rev() {
            self.bits((code >> i) & 1, 1);
        }
    }
}

/// [MS-MCI] §2: the history is kept from one MSZIP block to the next.
#[test]
fn an_mszip_block_refers_back_into_the_previous_one() {
    let first = b"abcdehello";
    let mut b1 = b"CK".to_vec();
    b1.extend(miniz_oxide::deflate::compress_to_vec(first, 6));
    // One fixed-Huffman block (RFC 1951 §3.2.6): final, type 01, length 10 (code 264), distance
    // 5 (code 4, one extra bit 0), end of block — "hello" twice, all from the first block.
    let mut bits = Bits::default();
    bits.bits(1, 1);
    bits.bits(1, 2);
    bits.code(0b000_1000, 7);
    bits.code(0b00100, 5);
    bits.bits(0, 1);
    bits.code(0, 7);
    let mut b2 = b"CK".to_vec();
    b2.extend(bits.out);
    let all = b"abcdehellohellohello";
    let file = blocks_cab(&[("a.txt", all)], &[(b1, 10), (b2, 10)], Compression::MsZip);
    let cab = read(&file);
    assert_eq!(cab.files[0].data, all);
}

#[test]
fn unsupported_and_damaged_cabinets_fail_cleanly() {
    let files: [(&str, &[u8]); 1] = [("a.one", b"data")];
    for n in [2u16, 3, 9] {
        let cab = read(&write(&files, Compression::Other(n)));
        assert!(cab.files.is_empty() && cab.warnings.len() == 1, "{n}: {cab:?}");
    }
    assert!(matches!(read_cabinet(b"PK\x03\x04 a zip", &CabLimits::default()), Err(OneError::Unsupported(_))));
    // A block without its CK signature damages only its folder.
    let file = blocks_cab(&files, &[(b"XX\x03\x00".to_vec(), 4)], Compression::MsZip);
    assert_eq!(read(&file).warnings.len(), 1);
    // A split cabinet (§2.1.10 cfhdrNEXT_CABINET).
    let mut split = write(&files, Compression::None);
    split[0x1E] = 0x02;
    assert!(matches!(read_cabinet(&split, &CabLimits::default()), Err(OneError::Unsupported(_))));
}

#[test]
fn forged_names_stay_inside_the_package() {
    let cab = read(&write(&[("..\\..\\.\\evil.one", b"x"), ("/abs/b.one", b"y")], Compression::None));
    assert_eq!(names(&cab), vec!["evil.one", "abs/b.one"]);
}

#[test]
fn limits_hold() {
    let data = vec![7u8; 5000];
    let file = write(&[("a", &data), ("b", &data)], Compression::MsZip);
    let tight = CabLimits { max_files: 1, ..CabLimits::default() };
    assert!(matches!(read_cabinet(&file, &tight), Err(OneError::TooLarge(_))));
    let small = CabLimits { max_total_bytes: 6000, ..CabLimits::default() };
    assert!(matches!(read_cabinet(&file, &small), Err(OneError::TooLarge(_))));
}

#[test]
fn truncated_and_flipped_cabinets_never_panic() {
    let data: Vec<u8> = (0..70_000u32).map(|i| (i % 13) as u8).collect();
    let file = write(&[("Work\\a.one", &data), ("b.one", b"bb")], Compression::MsZip);
    for n in 0..file.len().min(400) {
        let _ = read_cabinet(&file[..n], &CabLimits::default());
    }
    for n in (400..file.len()).step_by(97) {
        let _ = read_cabinet(&file[..n], &CabLimits::default());
    }
    for i in 0..file.len().min(300) {
        let mut f = file.clone();
        f[i] ^= 0xFF;
        let _ = read_cabinet(&f, &CabLimits::default());
    }
}

mod prop_tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn random_bytes_after_a_valid_header_never_panic(tail in proptest::collection::vec(any::<u8>(), 0..2048)) {
            let mut f = write(&[("a.one", b"abc")], Compression::MsZip)[..36].to_vec();
            f.extend(tail);
            let _ = read_cabinet(&f, &CabLimits::default());
        }
    }
}
