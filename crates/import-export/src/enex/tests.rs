use super::*;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01";

fn enex(notes: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE en-export SYSTEM \"http://xml.evernote.com/pub/evernote-export4.dtd\">\n<en-export export-date=\"20261030T101010Z\" application=\"Evernote\">{notes}</en-export>"
    )
}

fn note_xml(title: &str, body: &str, resources: &str) -> String {
    format!(
        "<note><title>{title}</title><created>20240305T143000Z</created><updated>20240306T090000Z</updated>\
         <content><![CDATA[<?xml version=\"1.0\" encoding=\"UTF-8\"?><!DOCTYPE en-note SYSTEM \"http://xml.evernote.com/pub/enml2.dtd\"><en-note>{body}</en-note>]]></content>{resources}</note>"
    )
}

fn resource(bytes: &[u8], mime: &str, name: &str) -> String {
    format!(
        "<resource><data encoding=\"base64\">\n{}\n</data><mime>{mime}</mime><resource-attributes><file-name>{name}</file-name></resource-attributes></resource>",
        b64::encode(bytes)
    )
}

#[test]
fn md5_matches_rfc_1321() {
    for (input, want) in [
        ("", "d41d8cd98f00b204e9800998ecf8427e"),
        ("a", "0cc175b9c0f1b6a831c399e269772661"),
        ("abc", "900150983cd24fb0d6963f7d28e17f72"),
        ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
        ("12345678901234567890123456789012345678901234567890123456789012345678901234567890", "57edf4a22be3c955ac49da2e2107b67a"),
    ] {
        assert_eq!(hex(&md5(input.as_bytes())), want, "{input}");
    }
}

#[test]
fn notes_with_pictures_todos_and_files() {
    // parity: FMT.Import.Enex
    let hash = hex(&md5(PNG));
    let body = format!(
        "<div><en-todo checked=\"true\"/>Buy milk</div><div><en-todo/>Call Bob</div><p>See <b>this</b> &amp; that</p><en-media hash=\"{hash}\" type=\"image/png\"/><en-crypt>secret</en-crypt>"
    );
    let res = resource(PNG, "image/png", "photo.png") + &resource(b"%PDF-1.4", "application/pdf", "plan.pdf");
    let xml = enex(&(note_xml("Groceries &amp; more", &body, &res) + &note_xml("Second", "<div>two</div>", "")));
    let notes = read(&xml).unwrap();
    assert_eq!(notes.len(), 2);
    let n = &notes[0];
    assert_eq!(n.title, "Groceries & more");
    assert_eq!(n.created, Some(Timestamp::from_civil(2024, 3, 5, 14, 30, 0)));
    assert_eq!(n.encrypted, 1);
    let texts: Vec<String> = n
        .content
        .blocks
        .iter()
        .map(|b| match b {
            Block::Para(p) => p.text(),
            Block::Image { alt, src } => format!("image {alt} {}", &src[..21]),
            Block::Table(_) => "table".into(),
        })
        .collect();
    assert_eq!(texts, ["☑ Buy milk", "☐ Call Bob", "See this & that", "image photo.png data:image/png;base64"]);
    assert_eq!(n.files, vec![("plan.pdf".to_string(), b"%PDF-1.4".to_vec())], "the PDF isn't in the content: a file");
    assert_eq!(notes[1].title, "Second");
}

#[test]
fn not_enex_and_hostile_input() {
    assert!(read("<html></html>").is_err());
    // Truncated, unbalanced, junk: whatever was read, never a panic.
    let xml = enex(&note_xml("A", "<div>x</div>", ""));
    for cut in (0..xml.len()).step_by(7) {
        if xml.is_char_boundary(cut) {
            let _ = read(&xml[..cut]);
        }
    }
    let _ = read(&format!("<en-export>{}", "<note>".repeat(10_000)));
    let _ = read("<en-export><note><content>&#xFFFFFFF;&#99999999999;&bogus;&</content></note></en-export>");
    assert_eq!(unescape("a&lt;b&#65;&#x42;&zz;"), "a<bAB&zz;");
}
