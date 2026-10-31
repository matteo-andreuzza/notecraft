//! View › New Quick Note (Ctrl+Shift+M; catalog `R.View.Window.NewSideNote`,
//! `KB.General.QuickNote`, `UI.Window.QuickNote`, `BEH.Notebooks.QuickNotes`).
//!
//! A quick note is a new page at the end of the Quick Notes section of the default notebook
//! (`nav.quickNotes` finds or makes it; Options › Save & Backup names the notebook). The window
//! turns into a small note window showing only that page (`ViewState::quick_note`); Normal
//! View, or the note window's expand button, brings the full window back. unverified: the
//! reference opens a separate small window; NoteCraft reuses its one window.

use serde_json::{Value, json};

use crate::selection::{Block, Pos};
use crate::{CmdResult, CommandSpec, Session};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("window.quickNote", "R.View.Window.NewSideNote", "New Quick Note", "View/Window/New Quick Note", quick_note)
            .key("Mod+Shift+M"),
    ]
}

fn quick_note(s: &mut Session, _: &Value) -> CmdResult {
    // The Quick Notes section, made when missing (with a page of its own, which then is the note).
    let r = crate::section_cmds::quick_notes(s, &json!({}))?;
    let page = match r.get("page").and_then(Value::as_str) {
        Some(p) => p.to_string(),
        None => {
            let sid = s.sel.section.ok_or_else(crate::ops::no_page)?;
            let page = crate::page_cmds::make_page(s, "");
            let pid = page.id;
            let sec = s.ws.section_mut(sid).ok_or_else(crate::ops::no_page)?;
            sec.pages.push(std::sync::Arc::new(page));
            s.goto_page(pid)?;
            pid.to_string()
        }
    };
    s.view.quick_note = true;
    s.view.full_page = false;
    // Typing starts the note's text at once (a container appears where the page's text begins).
    s.set_caret(Pos::new(Block::Title, 0, 0));
    Ok(json!({"page": page}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{run, sample};

    #[test]
    fn a_quick_note_is_a_new_page_in_quick_notes_shown_alone() {
        // parity: R.View.Window.NewSideNote, KB.General.QuickNote, BEH.Notebooks.QuickNotes
        let mut s = sample();
        let before = s.notebook().unwrap().sections().iter().find(|x| x.name == "Quick Notes").unwrap().pages.len();
        let r = run(&mut s, "window.quickNote", json!({}));
        let sec = s.section().unwrap();
        assert_eq!(sec.name, "Quick Notes");
        assert_eq!(sec.pages.len(), before + 1, "a new page at the end");
        assert_eq!(sec.pages.last().unwrap().id.to_string(), r["page"].as_str().unwrap());
        assert!(s.view.quick_note);
        assert_eq!(crate::commands::by_shortcut("Mod+Shift+M").map(|c| c.id), Some("window.quickNote"));
        // Normal View brings the full window back; one undo step removes the note.
        run(&mut s, "view.normal", json!({}));
        assert!(!s.view.quick_note);
        run(&mut s, "edit.undo", json!({}));
        let sec = s.notebook().unwrap().sections().into_iter().find(|x| x.name == "Quick Notes").unwrap().clone();
        assert_eq!(sec.pages.len(), before);
        // Without a Quick Notes section, one is made and its first page is the note.
        let mut s = sample();
        let qid = s.notebook().unwrap().sections().iter().find(|x| x.name == "Quick Notes").unwrap().id;
        run(&mut s, "section.delete", json!({"section": qid.to_string()}));
        run(&mut s, "window.quickNote", json!({}));
        assert_eq!((s.section().unwrap().name.as_str(), s.section().unwrap().pages.len()), ("Quick Notes", 1));
    }
}
