//! OneNote's own files (decision D12): File › Open › Open Section imports a `.one` section file
//! into the current notebook as a new section. The original file is only read, never changed;
//! what the reader does not import yet is listed in the notice and the command's result.

use std::sync::Arc;

use notecraft_doc::{NavNode, SectionColor};
use serde_json::{Value, json};

use crate::ops::need_notebook;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// The largest `.one` file read (the reader's own limit is higher; a section this large is
/// already far beyond what the reference writes).
const MAX_ONE_BYTES: usize = 1 << 30;

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // BS.Open.OpenSection — Ctrl+Alt+Shift+O.
        CommandSpec::new("file.openSection", "BS.Open.OpenSection", "Open Section", "File/Open/Open Section", open_section)
            .key("Mod+Alt+Shift+O")
            .params(r#"{"path": .one file | "data": base64 with "name", "name"?: section name, "notebook"?: id} → {section, page?, imported, notImported, damaged}"#)
            .when(need_notebook),
    ]
}

fn open_section(s: &mut Session, v: &Value) -> CmdResult {
    let nb_id = match p::id(v, "notebook")? {
        Some(id) => id,
        None => s.sel.notebook.ok_or_else(|| CmdError::Disabled("open a notebook first".into()))?,
    };
    let nb = s.ws.notebook(nb_id).ok_or_else(|| CmdError::Params("no such notebook".into()))?.clone();
    let (bytes, file_name) = crate::picture_cmds::read_input(v, MAX_ONE_BYTES)?;
    let stem = p::str(v, "name").map(str::to_string).or_else(|| file_name.map(|n| n.strip_suffix(".one").unwrap_or(&n).to_string()));
    let base = stem.map(|n| notecraft_doc::clean_name(&n)).filter(|n| !n.is_empty()).unwrap_or_else(|| "Imported Section".into());
    let name = unique_name(&nb, &base);
    let now = s.now();
    let import = notecraft_onestore::read_section(&bytes, &name, &mut s.ids, now).map_err(|e| CmdError::Failed(e.to_string()))?;
    let mut section = import.section;
    section.color = SectionColor::rotation(nb.sections().len());
    let sid = section.id;
    let first = section.pages.first().map(|pg| pg.id);
    let report = import.report;
    let nbm = s.ws.notebook_mut(nb_id).ok_or_else(|| CmdError::Params("no such notebook".into()))?;
    nbm.children.push(NavNode::Section(Arc::new(section)));
    match first {
        Some(pid) => s.goto_page(pid)?,
        None => s.goto_section(sid)?,
    }
    s.notice(format!("{name}: {}", report.summary()));
    Ok(json!({
        "section": sid.to_string(),
        "page": first.map(|id| id.to_string()),
        "imported": report.kept,
        "notImported": report.lost,
        "damaged": report.warnings.len(),
    }))
}

/// `base`, or `base 2`, `base 3`… when the notebook already has a section of that name.
fn unique_name(nb: &notecraft_doc::Notebook, base: &str) -> String {
    let names: Vec<String> = nb.sections().iter().map(|s| s.name.to_lowercase()).collect();
    if !names.contains(&base.to_lowercase()) {
        return base.to_string();
    }
    (2..).map(|n| format!("{base} {n}")).find(|c| !names.contains(&c.to_lowercase())).unwrap_or_else(|| base.to_string())
}

#[cfg(test)]
mod tests {
    use notecraft_onestore::writer::section::{FixtureOutline, FixturePage, write};
    use serde_json::json;

    use crate::tests::{run, sample};

    fn fixture() -> Vec<u8> {
        write(&[
            FixturePage {
                title: "Trip".into(),
                level: 1,
                outlines: vec![FixtureOutline {
                    x: 2.0,
                    y: 4.0,
                    paragraphs: vec![(0, "Tickets".into()), (1, "Train".into())],
                    ..FixtureOutline::default()
                }],
                ..FixturePage::default()
            },
            FixturePage { title: "Hotel".into(), level: 2, ..FixturePage::default() },
        ])
    }

    #[test]
    fn open_section_imports_a_one_file_as_a_new_section() {
        let mut s = sample();
        let row = "BS.Open.OpenSection";
        let before = s.notebook().unwrap().sections().len();
        let r = run(&mut s, "file.openSection", json!({"data": crate::b64::encode(&fixture()), "name": "Travel"}));
        assert_eq!(r["imported"]["pages"], 2, "{row}");
        assert_eq!(r["damaged"], 0);
        let nb = s.notebook().unwrap();
        assert_eq!(nb.sections().len(), before + 1);
        let sec = std::sync::Arc::clone(nb.sections().last().unwrap());
        assert_eq!(sec.name, "Travel");
        assert_eq!(sec.pages.iter().map(|p| (p.title.text(), p.level)).collect::<Vec<_>>(), vec![("Trip".into(), 0), ("Hotel".into(), 1)]);
        // The first imported page is opened, with its outline text.
        let page = s.page().unwrap();
        assert_eq!(page.title.text(), "Trip");
        let text: Vec<_> = page.objects.iter().flat_map(|o| o.paragraphs()).map(|p| (p.level, p.text())).collect();
        assert_eq!(text, vec![(0, "Tickets".to_string()), (1, "Train".to_string())]);
        // One undo step removes the section again.
        run(&mut s, "edit.undo", json!({}));
        assert_eq!(s.notebook().unwrap().sections().len(), before);
    }

    #[test]
    fn a_second_import_gets_a_new_name() {
        let mut s = sample();
        let data = crate::b64::encode(&fixture());
        run(&mut s, "file.openSection", json!({"data": data, "name": "Travel"}));
        run(&mut s, "file.openSection", json!({"data": data, "name": "travel"}));
        let names: Vec<_> = s.notebook().unwrap().sections().iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&"travel 2".to_string()), "{names:?}");
    }

    #[test]
    fn foreign_or_damaged_files_fail_cleanly() {
        let mut s = sample();
        let before = s.notebook().unwrap().sections().len();
        for bytes in [b"%PDF-1.7 not a section".to_vec(), fixture()[..1500].to_vec()] {
            let e = s.run("file.openSection", json!({"data": crate::b64::encode(&bytes)})).unwrap_err();
            assert!(matches!(e, crate::CmdError::Failed(_)), "{e}");
        }
        assert!(s.run("file.openSection", json!({})).is_err());
        assert_eq!(s.notebook().unwrap().sections().len(), before);
    }
}
