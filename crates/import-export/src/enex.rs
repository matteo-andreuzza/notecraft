//! Evernote export files (`.enex`, catalog `FMT.Import.Enex`), read from Evernote's public
//! format: `<en-export>` holds `<note>`s, each with a `<title>`, `<created>`/`<updated>`
//! (`YYYYMMDDThhmmssZ`), its `<content>` in ENML (XHTML inside CDATA, root `<en-note>`) and
//! `<resource>`s (base64 `<data>`, `<mime>`, `<resource-attributes><file-name>`). ENML names a
//! resource by the MD5 of its bytes: `<en-media hash="…" type="…"/>`.
//!
//! **What is kept:** each note's title, dates and content (read as a rich paste through
//! [`crate::html`]); `<en-media>` pictures in place; other resources (and resources the
//! content never names) as attached files; `<en-todo checked="…"/>` as ☐/☑ markers the engine
//! turns into To Do tags. Encrypted text (`<en-crypt>`) is left out and counted.
//!
//! The XML reader is small and for this format only: elements, attributes, text, CDATA and
//! the five XML entities plus numeric references; no DTD is read (Evernote's is public but
//! informative only). Bounded: [`MAX_NOTES`] notes, [`MAX_RESOURCES`] resources per note;
//! malformed input ends the read with what was found so far.

use notecraft_codecs::b64;
use notecraft_doc::Timestamp;

use crate::html::{self, Block, REF};
use crate::import::{Imported, TODO_MARKS};

/// Most notes read.
pub const MAX_NOTES: usize = 10_000;
/// Most resources read per note.
pub const MAX_RESOURCES: usize = 1_000;

/// One Evernote note.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Note {
    /// Its title (empty when it had none).
    pub title: String,
    /// When it was created.
    pub created: Option<Timestamp>,
    /// When it was last changed.
    pub updated: Option<Timestamp>,
    /// Its content (pictures inline as `data:` URIs).
    pub content: Imported,
    /// Resources that aren't pictures in the content: (file name, bytes).
    pub files: Vec<(String, Vec<u8>)>,
    /// Encrypted passages left out.
    pub encrypted: usize,
}

/// MD5 (RFC 1321) of `data`, as Evernote names resources.
pub fn md5(data: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16,
        23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    // K[i] = floor(abs(sin(i + 1)) * 2^32) (RFC 1321 §3.4).
    let k: Vec<u32> = (0..64).map(|i| ((i as f64 + 1.0).sin().abs() * 4_294_967_296.0) as u32).collect();
    let (mut a0, mut b0, mut c0, mut d0): (u32, u32, u32, u32) = (0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476);
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_le_bytes());
    for chunk in msg.as_chunks::<64>().0 {
        let m: Vec<u32> = chunk.as_chunks::<4>().0.iter().map(|w| u32::from_le_bytes(*w)).collect();
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = [0u8; 16];
    for (i, w) in [a0, b0, c0, d0].iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    out
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// XML text with its entities decoded.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let c = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16).ok().and_then(char::from_u32),
            n if n.starts_with('#') => n[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// An attribute's value from a tag's attribute text.
fn attr(attrs: &str, name: &str) -> Option<String> {
    let mut rest = attrs;
    while let Some(i) = rest.find(name) {
        let before_ok = i == 0 || rest[..i].ends_with(|c: char| c.is_ascii_whitespace());
        let after = rest[i + name.len()..].trim_start();
        if before_ok && let Some(v) = after.strip_prefix('=') {
            let v = v.trim_start();
            let q = v.chars().next()?;
            if q == '"' || q == '\'' {
                let body = &v[1..];
                return body.find(q).map(|e| unescape(&body[..e]));
            }
        }
        rest = &rest[i + name.len()..];
    }
    None
}

/// One step of the reader.
enum Ev<'a> {
    Start(&'a str),
    End(&'a str),
    Text(String),
}

/// A pull reader over `xml`.
struct Xml<'a> {
    rest: &'a str,
    /// The end of a self-closing element, reported after its start.
    pending_end: Option<&'a str>,
}

impl<'a> Xml<'a> {
    fn next_ev(&mut self) -> Option<Ev<'a>> {
        if let Some(n) = self.pending_end.take() {
            return Some(Ev::End(n));
        }
        self.step()
    }

    fn step(&mut self) -> Option<Ev<'a>> {
        loop {
            if self.rest.is_empty() {
                return None;
            }
            let Some(lt) = self.rest.find('<') else {
                let t = unescape(self.rest);
                self.rest = "";
                return Some(Ev::Text(t));
            };
            if lt > 0 {
                let t = unescape(&self.rest[..lt]);
                self.rest = &self.rest[lt..];
                return Some(Ev::Text(t));
            }
            let tail = self.rest;
            if let Some(body) = tail.strip_prefix("<![CDATA[") {
                let end = body.find("]]>")?;
                self.rest = &body[end + 3..];
                return Some(Ev::Text(body[..end].to_string()));
            }
            if let Some(body) = tail.strip_prefix("<!--") {
                self.rest = body.find("-->").map_or("", |e| &body[e + 3..]);
                continue;
            }
            if tail.starts_with("<?") || tail.starts_with("<!") {
                self.rest = tail.find('>').map_or("", |e| &tail[e + 1..]);
                continue;
            }
            let gt = tail.find('>')?;
            let inner = &tail[1..gt];
            self.rest = &tail[gt + 1..];
            if let Some(name) = inner.strip_prefix('/') {
                return Some(Ev::End(name.trim()));
            }
            let self_closing = inner.ends_with('/');
            let inner = inner.trim_end_matches('/');
            let name = inner.split(|c: char| c.is_ascii_whitespace()).next().unwrap_or(inner);
            if self_closing {
                // Report a self-closing element as start + end: keep the end for the next call.
                self.pending_end = Some(name);
            }
            return Some(Ev::Start(name));
        }
    }
}

/// `YYYYMMDDThhmmssZ` (UTC).
fn time(s: &str) -> Option<Timestamp> {
    let s = s.trim();
    let n = |a: usize, b: usize| s.get(a..b).and_then(|x| x.parse::<u32>().ok());
    if s.len() < 15 || s.as_bytes().get(8) != Some(&b'T') {
        return None;
    }
    Some(Timestamp::from_civil(i64::from(n(0, 4)?), n(4, 6)?, n(6, 8)?, n(9, 11)?, n(11, 13)?, 0))
}

/// Replace each `<name …>`/`<name …/>` element start in `s` by `f(attributes)`; its end tag
/// (if any) is dropped.
fn replace_elements(s: &str, name: &str, f: &mut dyn FnMut(&str) -> String) -> String {
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        if !after.starts_with(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>') {
            out.push_str(&rest[..i + open.len()]);
            rest = after;
            continue;
        }
        out.push_str(&rest[..i]);
        let Some(gt) = after.find('>') else {
            rest = "";
            break;
        };
        out.push_str(&f(after[..gt].trim_end_matches('/')));
        rest = &after[gt + 1..];
        if let Some(r) = rest.strip_prefix(close.as_str()) {
            rest = r;
        }
    }
    out.push_str(rest);
    out.replace(&close, "")
}

/// Drop every `<en-crypt>…</en-crypt>`; returns the text and how many were dropped.
fn drop_crypt(s: &str) -> (String, usize) {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    let mut n = 0;
    while let Some(i) = rest.find("<en-crypt") {
        out.push_str(&rest[..i]);
        n += 1;
        rest = rest[i..].find("</en-crypt>").map_or("", |e| &rest[i + e + "</en-crypt>".len()..]);
    }
    out.push_str(rest);
    (out, n)
}

#[derive(Default)]
struct Resource {
    data: Vec<u8>,
    mime: String,
    name: String,
    used: bool,
}

/// A note's ENML and resources as content and files.
fn note(title: String, created: Option<Timestamp>, updated: Option<Timestamp>, enml: &str, mut res: Vec<Resource>) -> Note {
    let hashes: Vec<String> = res.iter().map(|r| hex(&md5(&r.data))).collect();
    let (enml, encrypted) = drop_crypt(enml);
    let enml = replace_elements(&enml, "en-todo", &mut |a| {
        let done = attr(a, "checked").is_some_and(|v| v.eq_ignore_ascii_case("true"));
        TODO_MARKS[usize::from(done)].to_string()
    });
    let mut pictures: Vec<usize> = Vec::new();
    let enml = replace_elements(&enml, "en-media", &mut |a| {
        let hash = attr(a, "hash").unwrap_or_default().to_ascii_lowercase();
        match hashes.iter().position(|h| *h == hash) {
            Some(i) if res[i].mime.starts_with("image/") && notecraft_codecs::sniff(&res[i].data).is_some() => {
                res[i].used = true;
                pictures.push(i);
                format!("<img src=\"{REF}{}\" alt=\"{}\">", pictures.len() - 1, res[i].name.replace(['"', '<', '>', '&'], ""))
            }
            _ => String::new(),
        }
    });
    let blocks = html::parse_refs(&enml)
        .into_iter()
        .filter_map(|b| match b {
            Block::Image { src, alt } if src.starts_with(REF) => {
                let r = src.strip_prefix(REF).and_then(|n| n.parse::<usize>().ok()).and_then(|n| pictures.get(n)).and_then(|&i| res.get(i))?;
                let mime = notecraft_codecs::sniff(&r.data)?.mime();
                Some(Block::Image { src: format!("data:{mime};base64,{}", b64::encode(&r.data)), alt })
            }
            b => Some(b),
        })
        .collect();
    let files = res
        .into_iter()
        .filter(|r| !r.used && !r.data.is_empty())
        .enumerate()
        .map(|(k, r)| {
            let name = if r.name.trim().is_empty() { format!("attachment {}", k + 1) } else { r.name };
            (name, r.data)
        })
        .collect();
    Note { title, created, updated, content: Imported { title: None, blocks, pictures_lost: 0 }, files, encrypted }
}

/// The notes of an `.enex` file. `Err` when it isn't one.
pub fn read(xml: &str) -> Result<Vec<Note>, String> {
    if !xml.contains("<en-export") {
        return Err("not an Evernote export (.enex): no <en-export>".into());
    }
    let mut x = Xml { rest: xml, pending_end: None };
    let mut notes = Vec::new();
    // The element path and the text gathered for the innermost element.
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();
    let (mut title, mut created, mut updated, mut content) = (String::new(), None, None, String::new());
    let mut res: Vec<Resource> = Vec::new();
    let mut cur: Option<Resource> = None;
    while let Some(ev) = x.next_ev() {
        match ev {
            Ev::Start(name) => {
                if path.len() > 64 {
                    break;
                }
                path.push(name.to_string());
                text.clear();
                match name {
                    "note" => {
                        (title, created, updated, content) = (String::new(), None, None, String::new());
                        res.clear();
                    }
                    "resource" => cur = Some(Resource::default()),
                    _ => {}
                }
            }
            Ev::Text(t) => {
                if text.len() + t.len() <= 1 << 30 {
                    text.push_str(&t);
                }
            }
            Ev::End(name) => {
                let in_res = path.iter().any(|p| p == "resource");
                match name {
                    "title" if !in_res => title = text.trim().to_string(),
                    "created" if !in_res => created = time(&text),
                    "updated" if !in_res => updated = time(&text),
                    "content" => content = std::mem::take(&mut text),
                    "data" => {
                        if let Some(r) = cur.as_mut() {
                            r.data = b64::decode(&text).unwrap_or_default();
                        }
                    }
                    "mime" => {
                        if let Some(r) = cur.as_mut() {
                            r.mime = text.trim().to_ascii_lowercase();
                        }
                    }
                    "file-name" => {
                        if let Some(r) = cur.as_mut() {
                            r.name = notecraft_doc::media::safe_file_name(text.trim());
                        }
                    }
                    "resource" => {
                        if let Some(r) = cur.take()
                            && res.len() < MAX_RESOURCES
                        {
                            res.push(r);
                        }
                    }
                    "note" => {
                        notes.push(note(std::mem::take(&mut title), created, updated, &content, std::mem::take(&mut res)));
                        if notes.len() >= MAX_NOTES {
                            break;
                        }
                    }
                    _ => {}
                }
                // Close up to the matching element (forgiving stray end tags).
                if let Some(at) = path.iter().rposition(|p| p == name) {
                    path.truncate(at);
                }
                text.clear();
            }
        }
    }
    Ok(notes)
}

#[cfg(test)]
#[path = "enex/tests.rs"]
mod tests;
