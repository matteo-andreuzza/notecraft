//! Password-protected sections (decision D15; catalog `UI.Panes.Password`,
//! `R.Review.Section.PasswordProtectSection`, `KB.Protect.LockAll`, `BEH.Password.*`).
//!
//! A protected section is sealed on disk with a key derived from its password
//! (`notecraft_format::protect`). While it is unlocked its pages are in memory and the key is
//! kept in [`Session::keys`] so saves can seal it again; locking it seals the pages, drops
//! them and the key, and clears the undo history (earlier steps hold the notes in the clear).
//! Locked sections have no pages in memory, so search, Find Tags and exports never see them
//! (`BEH.Search.LockedExcluded`). A forgotten password can't be recovered
//! (`BEH.Password.NoRecovery`).
//!
//! Choices the reference doesn't document (unverified): pages deleted from a protected section
//! skip the Notebook Recycle Bin (its file is not sealed); a protected section can't be deleted
//! or copied until its password is removed; unlocking or changing a password asks for the
//! password again; a section is locked after the idle time of the Options.

use std::sync::Arc;

use notecraft_doc::{Id, Protection, Section};
use notecraft_format::protect::{self, KdfParams, Key};
use serde_json::{Value, json};

use crate::selection::PageSel;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// Longest password accepted (characters).
pub const MAX_PASSWORD: usize = 1024;

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // R.Review.Section.PasswordProtectSection — the Password Protection pane.
        CommandSpec::new("review.password", "R.Review.Section.PasswordProtectSection", "Password", "Review/Section/Password", |s, _| {
            s.view.password_pane = !s.view.password_pane;
            Ok(json!({"pane": s.view.password_pane}))
        })
        .pure()
        .when(need_section),
        CommandSpec::new(
            "section.setPassword",
            "UI.Panes.Password.SetPassword",
            "Set Password",
            "Window/Panes/Password Protection/Set Password",
            set_password,
        )
        .params(r#"{"password": string, "section"?: id}"#)
        .when(need_section),
        CommandSpec::new(
            "section.changePassword",
            "UI.Panes.Password.ChangePassword",
            "Change Password",
            "Window/Panes/Password Protection/Change Password",
            change_password,
        )
        .params(r#"{"old": string, "new": string, "section"?: id}"#)
        .when(need_protected),
        CommandSpec::new(
            "section.removePassword",
            "UI.Panes.Password.RemovePassword",
            "Remove Password",
            "Window/Panes/Password Protection/Remove Password",
            remove_password,
        )
        .params(r#"{"password": string, "section"?: id}"#)
        .when(need_protected),
        // BEH.Password.Lock — typing the password on a locked section opens it.
        CommandSpec::new("section.unlock", "NC.Section.Unlock", "Unlock Section", "Window/Canvas/Unlock Section", unlock)
            .params(r#"{"password": string, "section"?: id}"#)
            .when(need_section),
        CommandSpec::new("section.lock", "NC.Section.Lock", "Lock Section", "Window/Panes/Password Protection/Lock Section", |s, v| {
            let sid = target(s, v)?;
            if !s.ws.section(sid).is_some_and(|x| x.protection.is_some() && !x.is_locked()) {
                return Err(CmdError::Disabled("that section is not an unlocked password-protected section".into()));
            }
            lock(s, sid)?;
            Ok(json!({"locked": [sid.to_string()]}))
        })
        .params(r#"{"section"?: id}"#)
        .when(need_section),
        // KB.Protect.LockAll — Ctrl+Alt+L.
        CommandSpec::new("protect.lockAll", "KB.Protect.LockAll", "Lock All", "Keyboard/Protect/Lock all password-protected sections", |s, _| {
            let ids = s.unlocked_protected();
            for id in &ids {
                lock(s, *id)?;
            }
            Ok(json!({"locked": ids.iter().map(Id::to_string).collect::<Vec<_>>()}))
        })
        .key("Mod+Alt+L"),
    ]
}

fn need_section(s: &Session) -> Option<&'static str> {
    if s.section().is_some() { None } else { Some("Open a section first") }
}

fn need_protected(s: &Session) -> Option<&'static str> {
    match s.section() {
        Some(x) if x.protection.is_some() => None,
        Some(_) => Some("This section has no password"),
        None => Some("Open a section first"),
    }
}

fn target(s: &Session, v: &Value) -> Result<Id, CmdError> {
    match p::id(v, "section")? {
        Some(id) if s.ws.section(id).is_some() => Ok(id),
        Some(_) => Err(CmdError::Params("no such section".into())),
        None => s.sel.section.ok_or_else(|| CmdError::Disabled("open a section first".into())),
    }
}

fn password<'a>(v: &'a Value, key: &str) -> Result<&'a str, CmdError> {
    let pw = p::req_str(v, key)?;
    if pw.is_empty() {
        return Err(CmdError::Params("the password can't be empty".into()));
    }
    if pw.chars().count() > MAX_PASSWORD {
        return Err(CmdError::Params(format!("a password has at most {MAX_PASSWORD} characters")));
    }
    Ok(pw)
}

/// Random bytes for salts and nonces.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn random<const N: usize>() -> Result<[u8; N], CmdError> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|e| CmdError::Failed(format!("no secure random numbers available: {e}")))?;
    Ok(b)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn random<const N: usize>() -> Result<[u8; N], CmdError> {
    Err(CmdError::Disabled("password protection needs the desktop app".into()))
}

fn fail(e: notecraft_format::FormatError) -> CmdError {
    match e {
        notecraft_format::FormatError::WrongPassword => CmdError::Failed("The password is not correct.".into()),
        other => CmdError::Failed(other.to_string()),
    }
}

/// The key `pw` gives for `prot`'s salt and costs.
fn key_for(pw: &str, prot: &Protection) -> Result<Key, CmdError> {
    protect::derive_key(pw, &prot.salt, KdfParams::from_array(prot.kdf)).map_err(fail)
}

/// Check `pw` against an unlocked section's key, or open a locked one with it. Returns the
/// section as it is inside (unlocked) and its key.
fn verify(s: &Session, sid: Id, pw: &str) -> Result<(Section, Key), CmdError> {
    let sec = s.ws.section(sid).ok_or_else(|| CmdError::Params("no such section".into()))?;
    let prot = sec.protection.as_ref().ok_or_else(|| CmdError::Disabled("this section has no password".into()))?;
    let key = key_for(pw, prot)?;
    match &prot.locked {
        Some(bytes) => Ok((protect::open(bytes, &key).map_err(fail)?, key)),
        None => {
            if s.keys.get(&sid).map(|k| k.as_slice()) != Some(key.as_slice()) {
                return Err(CmdError::Failed("The password is not correct.".into()));
            }
            Ok((sec.as_ref().clone(), key))
        }
    }
}

fn replace(s: &mut Session, sid: Id, sec: Section) -> Result<(), CmdError> {
    *s.ws.section_mut(sid).ok_or_else(|| CmdError::Params("no such section".into()))? = sec;
    Ok(())
}

fn set_password(s: &mut Session, v: &Value) -> CmdResult {
    let sid = target(s, v)?;
    let pw = password(v, "password")?;
    let sec = s.ws.section(sid).ok_or_else(|| CmdError::Params("no such section".into()))?;
    if sec.protection.is_some() {
        return Err(CmdError::Disabled("this section already has a password (use Change Password)".into()));
    }
    let salt = random::<16>()?.to_vec();
    let prot = Protection { salt, kdf: s.kdf.to_array(), locked: None };
    let key = key_for(pw, &prot)?;
    s.ws.section_mut(sid).ok_or_else(|| CmdError::Params("no such section".into()))?.protection = Some(prot);
    s.keys.insert(sid, key);
    // Earlier steps would bring the section back without its password.
    s.forget_history = true;
    Ok(json!({"section": sid.to_string(), "protected": true}))
}

fn change_password(s: &mut Session, v: &Value) -> CmdResult {
    let sid = target(s, v)?;
    let (old, new) = (password(v, "old")?, password(v, "new")?);
    let was_locked = s.ws.section(sid).is_some_and(|x| x.is_locked());
    let (mut sec, _) = verify(s, sid, old)?;
    let prot = Protection { salt: random::<16>()?.to_vec(), kdf: s.kdf.to_array(), locked: None };
    let key = key_for(new, &prot)?;
    sec.protection = Some(prot);
    replace(s, sid, sec)?;
    s.keys.insert(sid, key);
    if was_locked {
        lock(s, sid)?;
    }
    s.forget_history = true;
    Ok(json!({"section": sid.to_string(), "changed": true}))
}

fn remove_password(s: &mut Session, v: &Value) -> CmdResult {
    let sid = target(s, v)?;
    let pw = password(v, "password")?;
    let (mut sec, _) = verify(s, sid, pw)?;
    sec.protection = None;
    replace(s, sid, sec)?;
    s.keys.remove(&sid);
    s.forget_history = true;
    Ok(json!({"section": sid.to_string(), "protected": false}))
}

fn unlock(s: &mut Session, v: &Value) -> CmdResult {
    let sid = target(s, v)?;
    let pw = password(v, "password")?;
    if !s.ws.section(sid).is_some_and(|x| x.is_locked()) {
        return Err(CmdError::Disabled("that section is not locked".into()));
    }
    let (sec, key) = verify(s, sid, pw)?;
    let pages = sec.pages.len();
    replace(s, sid, sec)?;
    s.keys.insert(sid, key);
    if s.sel.section == Some(sid) {
        s.goto_section(sid)?;
    }
    Ok(json!({"section": sid.to_string(), "pages": pages}))
}

/// Seal an unlocked protected section with its key and keep only the sealed file in memory.
fn lock(s: &mut Session, sid: Id) -> Result<(), CmdError> {
    let sec = s.ws.section(sid).ok_or_else(|| CmdError::Params("no such section".into()))?;
    let key = s.keys.get(&sid).ok_or_else(|| CmdError::Failed(format!("{}: its key is not available", sec.name)))?;
    let bytes = protect::seal(sec, key, random::<24>()?).map_err(fail)?;
    let mut locked = protect::locked_section(&bytes).map_err(fail)?;
    // The header carries what the tab shows; keep the section's place in the notebook.
    locked.default_template = sec.default_template.clone();
    replace(s, sid, locked)?;
    s.keys.remove(&sid);
    if s.sel.section == Some(sid) {
        s.sel.page = None;
        s.sel.on_page = PageSel::None;
    }
    s.forget_history = true;
    Ok(())
}

impl Session {
    /// Unlocked password-protected sections, in every open notebook.
    pub fn unlocked_protected(&self) -> Vec<Id> {
        self.ws.notebooks.iter().flat_map(|nb| nb.sections().into_iter().filter(|x| x.protection.is_some() && !x.is_locked()).map(|x| x.id)).collect()
    }

    /// The host calls this from its loop: lock every protected section after `idle_ms` without
    /// input when Options › Advanced asks for it (`OPT.Advanced.LockAfterIdle`), and the ones
    /// left behind when another section is open (`OPT.Advanced.LockOnLeave`). Returns whether
    /// anything was locked.
    pub fn auto_lock(&mut self, idle_ms: i64) -> bool {
        let open = self.unlocked_protected();
        if open.is_empty() {
            return false;
        }
        let limit = i64::from(self.prefs.lock_after_minutes).saturating_mul(60_000);
        let mut targets: Vec<Id> = Vec::new();
        if limit > 0 && idle_ms >= limit {
            targets = open.clone();
        } else if self.prefs.lock_on_leave {
            targets = open.into_iter().filter(|id| self.sel.section != Some(*id)).collect();
        }
        let mut any = false;
        for id in targets {
            any |= self.run("section.lock", json!({"section": id.to_string()})).is_ok();
        }
        any
    }

    /// `nb` as it must be written: every unlocked protected section sealed with its key. A
    /// section whose key is missing is an error, never written in the clear.
    pub(crate) fn sealed_copy(&self, nb: &Arc<notecraft_doc::Notebook>) -> Result<Arc<notecraft_doc::Notebook>, String> {
        let open: Vec<Id> = nb.sections().iter().filter(|x| x.protection.is_some() && !x.is_locked()).map(|x| x.id).collect();
        if open.is_empty() {
            return Ok(nb.clone());
        }
        let mut copy = nb.as_ref().clone();
        for id in open {
            let Some(sec) = copy.section_mut(id) else { continue };
            let key = self.keys.get(&id).ok_or_else(|| format!("{} is password protected and its key is not available", sec.name))?;
            let nonce = random::<24>().map_err(|e| e.to_string())?;
            let bytes = protect::seal(sec, key, nonce).map_err(|e| e.to_string())?;
            if let Some(prot) = sec.protection.as_mut() {
                prot.locked = Some(Arc::from(bytes));
            }
        }
        Ok(Arc::new(copy))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;
    use crate::tests::{clock, run};

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("notecraft-protect-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// A notebook on disk with a page "Bank codes" holding "PIN 1234" in a protected section.
    fn protected(name: &str) -> (Session, std::path::PathBuf, notecraft_doc::Id) {
        let root = dir(name);
        let mut s = Session::new(clock());
        s.kdf = notecraft_format::protect::KdfParams::TEST;
        run(&mut s, "file.newNotebook", json!({"name": "Private", "path": root.to_str().unwrap()}));
        run(&mut s, "page.rename", json!({"title": "Bank codes"}));
        run(&mut s, "text.insert", json!({"text": "PIN 1234"}));
        let sid = s.section().unwrap().id;
        run(&mut s, "section.setPassword", json!({"password": "s3cret"}));
        assert!(s.save_dirty().is_empty());
        (s, root, sid)
    }

    fn files_contain(root: &std::path::Path, needle: &[u8]) -> bool {
        fn walk(p: &std::path::Path, needle: &[u8]) -> bool {
            std::fs::read_dir(p).into_iter().flatten().flatten().any(|e| {
                let p = e.path();
                if p.is_dir() { walk(&p, needle) } else { std::fs::read(&p).is_ok_and(|b| b.windows(needle.len()).any(|w| w == needle)) }
            })
        }
        walk(root, needle)
    }

    #[test]
    fn a_protected_section_is_sealed_on_disk_and_opens_locked() {
        // parity: UI.Panes.Password.SetPassword
        let (mut s, root, sid) = protected("disk");
        assert!(!files_contain(&root, b"PIN 1234") && !files_contain(&root, b"Bank codes"), "BEH.Password.NoRecovery");
        let file = std::fs::read(root.join("New Section 1.ncraft")).unwrap();
        assert!(notecraft_format::protect::is_sealed(&file), "the section file is sealed");
        // Reopened, the section is locked: its name shows, its pages don't.
        let mut fresh = Session::new(clock());
        run(&mut fresh, "file.openNotebook", json!({"path": root.to_str().unwrap()}));
        let sec = fresh.ws.section(sid).unwrap().clone();
        assert!(sec.is_locked() && sec.pages.is_empty(), "BEH.Password.Lock");
        let found = run(&mut fresh, "search.all", json!({"query": "1234"}));
        assert_eq!(found["total"], 0, "BEH.Search.LockedExcluded");
        assert!(fresh.run("section.unlock", json!({"section": sid.to_string(), "password": "wrong"})).is_err());
        run(&mut fresh, "section.unlock", json!({"section": sid.to_string(), "password": "s3cret"}));
        let sec = fresh.ws.section(sid).unwrap();
        assert_eq!(sec.pages[0].title.text(), "Bank codes", "NC.Section.Unlock");
        // Edits while unlocked are saved sealed too.
        fresh.sel.section = Some(sid);
        run(&mut fresh, "page.rename", json!({"section": sid.to_string(), "title": "Bank codes 2"}));
        assert!(fresh.save_dirty().is_empty());
        assert!(!files_contain(&root, b"Bank codes 2"));
        // Lock All: pages leave memory and undo can't bring them back.
        run(&mut fresh, "protect.lockAll", json!({}));
        assert!(fresh.ws.section(sid).unwrap().is_locked(), "KB.Protect.LockAll");
        assert!(fresh.undo_labels().is_empty(), "no undo step holds the notes");
        assert!(fresh.save_dirty().is_empty());
        let _ = &mut s;
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn passwords_change_and_go_and_protected_sections_are_handled_with_care() {
        let (mut s, root, sid) = protected("change");
        assert!(s.run("section.changePassword", json!({"old": "nope", "new": "x"})).is_err());
        run(&mut s, "section.changePassword", json!({"old": "s3cret", "new": "n3w"}));
        run(&mut s, "section.lock", json!({}));
        assert!(s.run("section.unlock", json!({"password": "s3cret"})).is_err(), "UI.Panes.Password.ChangePassword");
        run(&mut s, "section.unlock", json!({"password": "n3w"}));
        // Deleting the section, or copying it, needs the password removed first.
        assert!(s.run("section.delete", json!({})).is_err());
        assert!(s.run("section.moveOrCopy", json!({"copy": true})).is_err());
        // A deleted page doesn't go to the (unsealed) Recycle Bin.
        run(&mut s, "page.new", json!({"title": "scratch"}));
        run(&mut s, "page.delete", json!({}));
        assert!(s.notebook().unwrap().recycle.pages.is_empty());
        // Exporting the notebook while the section is locked would lose it.
        run(&mut s, "section.lock", json!({}));
        assert!(s.run("file.exportNotebook", json!({"path": root.join("x.ncraft-archive").to_str().unwrap()})).is_err());
        // Remove Password: the section is written in the clear again.
        run(&mut s, "section.removePassword", json!({"password": "n3w"}));
        assert!(s.section().unwrap().protection.is_none(), "UI.Panes.Password.RemovePassword");
        assert!(s.save_dirty().is_empty());
        let mut fresh = Session::new(clock());
        run(&mut fresh, "file.openNotebook", json!({"path": root.to_str().unwrap()}));
        let sec = fresh.ws.section(sid).unwrap();
        assert!(sec.protection.is_none() && sec.pages.iter().any(|p| p.title.text() == "Bank codes"), "opens without a password");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sections_lock_after_the_idle_time_or_on_leaving() {
        let (mut s, root, sid) = protected("idle");
        assert!(!s.auto_lock(60_000), "one idle minute is not ten");
        assert!(s.auto_lock(11 * 60_000), "OPT.Advanced.LockAfterIdle");
        assert!(s.ws.section(sid).unwrap().is_locked());
        run(&mut s, "section.unlock", json!({"section": sid.to_string(), "password": "s3cret"}));
        run(&mut s, "file.options", json!({"set": {"lock_on_leave": true}}));
        assert!(!s.auto_lock(0), "still in the section");
        run(&mut s, "section.new", json!({}));
        assert!(s.auto_lock(0), "OPT.Advanced.LockOnLeave");
        assert!(s.ws.section(sid).unwrap().is_locked());
        // The Password pane toggles.
        assert_eq!(run(&mut s, "review.password", json!({}))["pane"], true, "R.Review.Section.PasswordProtectSection");
        let _ = std::fs::remove_dir_all(&root);
    }
}
