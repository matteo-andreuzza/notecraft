//! `kill -9` recovery test (brief §6 M1, catalog `BEH.Save.CrashRecovery`).
//!
//! The test re-runs its own binary as a child process that saves the notebook in a tight loop
//! (each save rewrites a large section through the atomic writer), kills it with SIGKILL at a
//! random-ish moment, then opens the notebook: it must open without error, every section must
//! decode, and the page must hold one complete generation written by the child — never a mix
//! and never a truncated file. Repeated several times to hit different points of the write.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use notecraft_doc::{Id, IdGen, NavNode, Notebook, Page, Paragraph, Section, SectionColor, Timestamp};
use notecraft_format::store::{Dirty, NotebookStore};

const CHILD_ENV: &str = "NOTECRAFT_KILL9_CHILD";

fn notebook(generation: u64) -> Notebook {
    let mut nb = Notebook::new(Id(1), "Crash test", SectionColor::default());
    let mut s = Section::new(Id(10), "Notes", SectionColor::index(1), Timestamp(0));
    let mut p = Page::new(Id(20), Id(21), Timestamp(0), "");
    p.title = Paragraph::with_text(Id(21), &format!("gen {generation}"));
    // About 1 MB of text so a save takes long enough to be interrupted mid-write.
    let body = format!("generation {generation} ").repeat(50_000);
    let mut o = notecraft_doc::Outline::new(Id(30), notecraft_doc::geom::Point::new(36.0, 80.0), Id(31));
    o.paragraphs[0] = Paragraph::with_text(Id(31), &body);
    p.objects.push(notecraft_doc::PageObject::Outline(o));
    s.pages.push(Arc::new(p));
    nb.children.push(NavNode::Section(Arc::new(s)));
    nb
}

fn child(root: &Path) -> ! {
    let mut ids = IdGen::new(77);
    let mut store = match NotebookStore::open(root, &mut ids) {
        Ok(o) => o.store,
        Err(e) => {
            eprintln!("child: {e}");
            std::process::exit(2);
        }
    };
    let mut generation = 1u64;
    loop {
        generation += 1;
        let nb = notebook(generation);
        let mut dirty = Dirty::default();
        dirty.sections.insert(Id(10));
        if store.save(&nb, &dirty).is_err() {
            std::process::exit(3);
        }
    }
}

fn tmp() -> PathBuf {
    let d = std::env::temp_dir().join(format!("notecraft-kill9-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn kill9_never_corrupts_a_notebook() {
    if let Ok(root) = std::env::var(CHILD_ENV) {
        child(Path::new(&root));
    }
    let root = tmp();
    NotebookStore::create(&root, &notebook(1)).unwrap();
    let exe = std::env::current_exe().unwrap();
    for round in 0..6u64 {
        let mut c = std::process::Command::new(&exe)
            .args(["kill9_never_corrupts_a_notebook", "--exact", "--nocapture"])
            .env(CHILD_ENV, &root)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(150 + round * 97));
        c.kill().unwrap(); // SIGKILL on Unix: no destructors, no flushing
        let _ = c.wait();
        let o = NotebookStore::open(&root, &mut IdGen::new(5)).unwrap();
        let sections = o.notebook.sections();
        assert_eq!(sections.len(), 1, "{}: round {round}: {:?}", "BEH.Save.CrashRecovery", o.warnings);
        assert!(o.warnings.iter().all(|w| !w.contains("skipped")), "round {round}: a section was unreadable: {:?}", o.warnings);
        let page = &sections[0].pages[0];
        let title = page.display_title();
        let generation: u64 = title.strip_prefix("gen ").and_then(|g| g.parse().ok()).unwrap_or_else(|| panic!("round {round}: bad title {title:?}"));
        // Title and body come from the same generation: the file was written whole.
        assert!(page.plain_text().contains(&format!("generation {generation} ")), "round {round}: title and body disagree");
    }
    let _ = std::fs::remove_dir_all(&root);
}
