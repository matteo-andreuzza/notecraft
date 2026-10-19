use super::*;
use proptest::prelude::*;

/// A small synthetic picture: a red/blue gradient with a transparent corner.
fn sample(w: u32, h: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(w, h, |x, y| {
        let a = if x == 0 && y == 0 { 0 } else { 255 };
        image::Rgba([(x * 255 / w.max(1)) as u8, 40, (y * 255 / h.max(1)) as u8, a])
    })
}

fn encoded(format: Format, w: u32, h: u32) -> Vec<u8> {
    let img = DynamicImage::ImageRgba8(sample(w, h));
    // JPEG has no alpha channel.
    let img = if format == Format::Jpeg { DynamicImage::ImageRgb8(img.into_rgb8()) } else { img };
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, format.image_format()).unwrap();
    out.into_inner()
}

#[test]
fn every_format_probes_and_decodes() {
    for f in Format::ALL {
        let bytes = encoded(f, 7, 5);
        assert_eq!(sniff(&bytes), Some(f), "{f:?}");
        let info = probe(&bytes, &Limits::PICTURE).unwrap();
        assert_eq!((info.format, info.width, info.height), (f, 7, 5));
        let img = decode(&bytes, &Limits::PICTURE).unwrap();
        assert_eq!((img.width, img.height, img.pixels.len()), (7, 5, 7 * 5 * 4), "{f:?}");
        assert!(Format::from_mime(f.mime()) == Some(f) && f.extensions().contains(&f.extension()));
    }
    assert_eq!(Format::from_mime("image/JPG; q=1"), Some(Format::Jpeg));
    assert_eq!(Format::from_mime("text/html"), None);
}

#[test]
fn png_round_trip_keeps_pixels() {
    let src = sample(9, 4);
    let img = Rgba8::new(9, 4, src.clone().into_raw()).unwrap();
    let png = encode_png(&img, &Limits::PICTURE).unwrap();
    assert_eq!(decode(&png, &Limits::PICTURE).unwrap(), img);
    assert!(Rgba8::new(9, 4, vec![0; 3]).is_err());
    assert!(Rgba8::new(0, 4, Vec::new()).is_err());
}

#[test]
fn decode_fit_shrinks_keeping_the_aspect_ratio() {
    let bytes = encoded(Format::Png, 400, 100);
    let small = decode_fit(&bytes, &Limits::PICTURE, 64).unwrap();
    assert_eq!((small.width, small.height), (64, 16));
    let same = decode_fit(&bytes, &Limits::PICTURE, 4096).unwrap();
    assert_eq!((same.width, same.height), (400, 100));
}

#[test]
fn unknown_and_empty_input_is_unsupported() {
    for bad in [&b""[..], b"hello", b"BM", b"RIFF\0\0\0\0WAVE", b"<svg/>"] {
        assert_eq!(probe(bad, &Limits::PICTURE), Err(CodecError::Unsupported), "{bad:?}");
        assert_eq!(decode(bad, &Limits::PICTURE), Err(CodecError::Unsupported));
    }
}

#[test]
fn limits_are_checked_before_decoding() {
    let bytes = encoded(Format::Png, 40, 30);
    let tight = Limits { max_bytes: 10, ..Limits::PICTURE };
    assert!(matches!(probe(&bytes, &tight), Err(CodecError::TooLarge(_))));
    let narrow = Limits { max_side: 32, ..Limits::PICTURE };
    assert!(matches!(decode(&bytes, &narrow), Err(CodecError::TooLarge(_))));
    let few = Limits { max_pixels: 1000, ..Limits::PICTURE };
    assert!(matches!(decode(&bytes, &few), Err(CodecError::TooLarge(_))));
    assert!(matches!(encode_png(&Rgba8::new(40, 30, vec![0; 4800]).unwrap(), &few), Err(CodecError::TooLarge(_))));
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c ^= u32::from(*b);
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

#[test]
fn a_header_claiming_a_huge_picture_is_refused_without_allocating() {
    // A valid 1×1 PNG whose IHDR (CRC fixed up) claims 1,000,000 × 1,000,000 pixels: a decoder
    // that trusted it would try to allocate 4 TB.
    let mut png = encoded(Format::Png, 1, 1);
    png[16..20].copy_from_slice(&1_000_000u32.to_be_bytes());
    png[20..24].copy_from_slice(&1_000_000u32.to_be_bytes());
    let crc = crc32(&png[12..29]);
    png[29..33].copy_from_slice(&crc.to_be_bytes());
    assert!(matches!(probe(&png, &Limits::PICTURE), Err(CodecError::TooLarge(_))));
    assert!(matches!(decode(&png, &Limits::PICTURE), Err(CodecError::TooLarge(_))));
}

#[test]
fn exif_orientation_is_applied() {
    // A JPEG with an EXIF APP1 segment saying "rotate 90° clockwise" (orientation 6), as phone
    // cameras write: a 6×2 sensor image is shown 2 wide and 6 tall.
    let jpeg = encoded(Format::Jpeg, 6, 2);
    let tiff: &[u8] = b"MM\0\x2a\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0\x06\0\0\0\0\0\0";
    let mut app1 = vec![0xFF, 0xE1];
    app1.extend_from_slice(&((2 + 6 + tiff.len()) as u16).to_be_bytes());
    app1.extend_from_slice(b"Exif\0\0");
    app1.extend_from_slice(tiff);
    let mut with_exif = jpeg[..2].to_vec();
    with_exif.extend_from_slice(&app1);
    with_exif.extend_from_slice(&jpeg[2..]);
    let info = probe(&with_exif, &Limits::PICTURE).unwrap();
    assert_eq!((info.width, info.height), (2, 6));
    let img = decode(&with_exif, &Limits::PICTURE).unwrap();
    assert_eq!((img.width, img.height), (2, 6));
}

#[test]
fn truncated_files_are_errors_not_panics() {
    for f in Format::ALL {
        let bytes = encoded(f, 16, 12);
        for n in 0..bytes.len() {
            let cut = &bytes[..n];
            // Some decoders fill a cut-off tail with grey; any result is fine but a panic.
            if let Ok(img) = decode(cut, &Limits::PICTURE) {
                assert_eq!(img.pixels.len() as u64, u64::from(img.width) * u64::from(img.height) * 4);
            }
            let _ = probe(cut, &Limits::PICTURE);
        }
    }
}

#[test]
fn corrupted_bytes_are_errors_not_panics() {
    // Deterministic byte flips across every format's whole file.
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    for f in Format::ALL {
        let bytes = encoded(f, 16, 12);
        for _ in 0..300 {
            let mut b = bytes.clone();
            for _ in 0..4 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let i = (seed as usize) % b.len();
                b[i] = (seed >> 32) as u8;
            }
            if let Ok(img) = decode(&b, &Limits::PICTURE) {
                assert!(img.width <= Limits::PICTURE.max_side && img.height <= Limits::PICTURE.max_side);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn random_bytes_behind_a_valid_magic_never_panic(
        magic in prop::sample::select(vec![&b"\x89PNG\r\n\x1a\n"[..], &[0xFF, 0xD8, 0xFF][..], b"GIF89a", b"BM", b"RIFF\0\0\0\0WEBPVP8 "]),
        tail in prop::collection::vec(any::<u8>(), 0..512),
    ) {
        let mut b = magic.to_vec();
        b.extend_from_slice(&tail);
        let _ = probe(&b, &Limits::PICTURE);
        let _ = decode_fit(&b, &Limits::PICTURE, 64);
    }
}
