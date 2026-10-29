//! File › Export of pages and sections (BS.Export.Page, BS.Export.Section,
//! BS.Export.FormatMht): formats, the files beside Markdown, the report, refusals.

use notecraft_codecs::{Limits, Rgba8, encode_png};
use serde_json::json;

use crate::tests::{blank_page, run, sample};

fn dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("notecraft-export-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A blank page titled "Trip" with a line of text, a picture and a stroke.
fn trip(s: &mut crate::Session) {
    blank_page(s);
    run(s, "page.rename", json!({"title": "Trip"}));
    run(s, "outline.new", json!({"x": 40.0, "y": 100.0}));
    run(s, "text.insert", json!({"text": "Pack *socks*"}));
    let png = encode_png(&Rgba8::new(2, 2, vec![200; 16]).unwrap(), &Limits::PICTURE).unwrap();
    run(s, "insert.picture", json!({"data": crate::b64::encode(&png), "x": 40.0, "y": 200.0}));
    run(s, "draw.stroke", json!({"points": [[10, 300, 0.5], [60, 320, 0.9]]}));
}

#[test]
fn a_page_exports_as_web_page_html_and_markdown() {
    let d = dir("page");
    let mut s = sample();
    trip(&mut s);
    // .mht by extension.
    let mht = d.join("Trip.mht");
    let r = run(&mut s, "file.exportPage", json!({"path": mht.to_str().unwrap()}));
    assert_eq!((r["pages"].as_u64(), r["pictures"].as_u64(), r["inkLost"].as_u64()), (Some(1), Some(1), Some(1)), "{r}");
    let text = std::fs::read_to_string(&mht).unwrap();
    assert!(text.contains("multipart/related") && text.contains("image001.png"), "BS.Export.Page");
    // HTML with the picture inline.
    let html = d.join("Trip.htm");
    run(&mut s, "file.exportPage", json!({"path": html.to_str().unwrap()}));
    let text = std::fs::read_to_string(&html).unwrap();
    assert!(text.contains("<h1 class=\"title\">Trip</h1>") && text.contains("data:image/png;base64,"), "FMT.Export.Html");
    // Markdown with the picture in a folder beside it.
    let md = d.join("trip notes.txt");
    let r = run(&mut s, "file.exportPage", json!({"path": md.to_str().unwrap(), "format": "md"}));
    let text = std::fs::read_to_string(&md).unwrap();
    assert!(text.starts_with("# Trip\n") && text.contains("Pack \\*socks\\*"), "FMT.Export.Markdown: {text}");
    assert!(text.contains("![](trip%20notes_files/image001.png)"), "{text}");
    assert!(d.join("trip notes_files/image001.png").is_file());
    assert_eq!(r["written"].as_array().map(Vec::len), Some(2));
    // A Word document (BS.Export.FormatDocx): a zip package holding the page.
    let docx = d.join("Trip.docx");
    let r = run(&mut s, "file.exportPage", json!({"path": docx.to_str().unwrap()}));
    assert_eq!(r["pictures"].as_u64(), Some(1), "{r}");
    let bytes = std::fs::read(&docx).unwrap();
    assert!(bytes.starts_with(b"PK\x03\x04"), "FMT.Export.Docx");
    // PDF (BS.Export.FormatPdf), when the machine has a page font.
    if notecraft_fonts::page_fonts().regular.is_some() {
        let pdf = d.join("Trip.pdf");
        let r = run(&mut s, "file.exportPage", json!({"path": pdf.to_str().unwrap()}));
        assert_eq!((r["sheets"].as_u64(), r["inkLost"].as_u64()), (Some(1), Some(0)), "{r}");
        assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"));
        let nb = d.join("Notebook.pdf");
        let r = run(&mut s, "file.exportNotebook", json!({"path": nb.to_str().unwrap()}));
        assert!(r["pages"].as_u64().unwrap() >= 3, "every page of the notebook: {r}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_section_exports_every_page_in_order() {
    let d = dir("section");
    let mut s = sample();
    let names: Vec<String> = s.section().unwrap().pages.iter().map(|p| p.display_title()).collect();
    let path = d.join("section.md");
    let r = run(&mut s, "file.exportSection", json!({"path": path.to_str().unwrap()}));
    assert_eq!(r["pages"].as_u64(), Some(names.len() as u64), "BS.Export.Section");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut at = 0;
    for n in &names {
        let i = text[at..].find(&format!("# {n}")).map(|i| i + at);
        assert!(i.is_some(), "{n} in order: {text}");
        at = i.unwrap_or(at);
    }
    // The Single File Web Page command, for the section (CM.Section.FileSaveAsContextualItem
    // runs file.exportSection too).
    let mht = d.join("section.mht");
    let r = run(&mut s, "file.exportMht", json!({"path": mht.to_str().unwrap(), "scope": "section"}));
    assert_eq!(r["pages"].as_u64(), Some(names.len() as u64), "BS.Export.FormatMht");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn unavailable_formats_and_bad_params_are_refused_cleanly() {
    let d = dir("refuse");
    let mut s = sample();
    let p = |n: &str| d.join(n).to_string_lossy().into_owned();
    for (v, want) in [
        (json!({"path": p("a.xps")}), "not available yet"),
        (json!({"path": p("a.x"), "format": "doc"}), "not available yet"),
        (json!({"path": p("a.x"), "format": "rtf"}), "unknown export format"),
        (json!({"path": p("a.md"), "page": "not-an-id"}), ""),
    ] {
        let e = s.run("file.exportPage", v.clone()).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(!e.is_empty() && e.contains(want), "{v}: {e}");
    }
    assert!(s.run("file.exportMht", json!({"path": p("a.mht"), "scope": "notebook"})).is_err());
    // A link where the files folder goes is not written through.
    #[cfg(unix)]
    {
        let outside = dir("refuse-outside");
        std::os::unix::fs::symlink(&outside, d.join("b_files")).unwrap();
        trip(&mut s);
        let e = s.run("file.exportPage", json!({"path": p("b.md")})).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(e.contains("link"), "{e}");
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&outside);
    }
    assert_eq!(std::fs::read_dir(&d).unwrap().filter(|e| e.as_ref().is_ok_and(|e| e.path().is_file())).count(), 0, "nothing half-written");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn print_writes_the_print_ready_pdf() {
    if notecraft_fonts::page_fonts().regular.is_none() {
        return;
    }
    let d = dir("print");
    let mut s = sample();
    let title = s.page().unwrap().display_title();
    let r = run(&mut s, "file.print", json!({"dir": d.to_str().unwrap()}));
    let path = std::path::PathBuf::from(r["path"].as_str().unwrap());
    assert_eq!(path, d.join(format!("{title}.pdf")), "BS.Print.Print");
    assert!(std::fs::read(&path).unwrap().starts_with(b"%PDF-"));
    let r = run(&mut s, "file.printPreview", json!({"dir": d.to_str().unwrap(), "scope": "section"}));
    assert_eq!(r["pages"].as_u64(), Some(s.section().unwrap().pages.len() as u64), "BS.Print.Preview");
    assert!(s.run("file.print", json!({"scope": "notebook"})).is_err());
    let _ = std::fs::remove_dir_all(&d);
}
