//! Base64 (RFC 4648, standard alphabet) for binary command parameters: a picture or file
//! passed by value (`insert.picture {"data"}`, a pasted screenshot) instead of by path, which
//! the web build and agents without file access need. Small and dependency-free; decoding is
//! tolerant of whitespace and missing padding, and bounded by the caller's length check.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode bytes.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk.first().copied().unwrap_or(0), chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn value(c: u8) -> Option<u32> {
    Some(match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' | b'-' => 62,
        b'/' | b'_' => 63,
        _ => return None,
    } as u32)
}

/// Decode base64 (standard or URL-safe alphabet, whitespace and a `data:…;base64,` prefix
/// ignored). `None` on any other character.
pub fn decode(s: &str) -> Option<Vec<u8>> {
    let s = match s.find(";base64,") {
        Some(i) if s.starts_with("data:") => s.get(i + 8..)?,
        _ => s,
    };
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for c in s.bytes() {
        if c.is_ascii_whitespace() {
            continue;
        }
        if c == b'=' {
            break;
        }
        acc = (acc << 6) | value(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_noise() {
        for n in 0..40usize {
            let bytes: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(decode(&encode(&bytes)).unwrap(), bytes, "{n}");
        }
        assert_eq!(encode(b"Man"), "TWFu");
        assert_eq!(encode(b"Ma"), "TWE=");
        assert_eq!(decode("TW\nE").unwrap(), b"Ma");
        assert_eq!(decode("data:image/png;base64,TWFu").unwrap(), b"Man");
        assert_eq!(decode("TWF*"), None);
        // RFC 4648 §10 test vectors.
        for (input, want) in
            [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")]
        {
            assert_eq!(encode(input.as_bytes()), want);
        }
        assert_eq!(encode(&[0xff, 0xfe, 0xfd]), "//79");
    }
}
