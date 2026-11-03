//! History › Recent Edits: the pages changed in a period, newest first, in the Recent Edits
//! pane (catalog `R.History.Authors.FindByDateMenu` and its periods, `BEH.History.RecentEdits`,
//! `UI.Panes.RecentEdits`).
//!
//! A page's "edit time" is `Page::modified`, which every change to the page sets. Periods
//! that name days count from the local midnight (Today = since midnight, Since Yesterday =
//! since yesterday's midnight); the others are that many days back from now.

use notecraft_doc::{NavNode, Timestamp};
use serde_json::{Value, json};

use crate::search_cmds::parse_scope;
use crate::session::ScopeKind;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// Most pages one listing returns.
pub const MAX_RECENT: usize = 5_000;
const DAY_MS: i64 = 86_400_000;

/// The periods of History › Recent Edits, in the menu's order: id and label.
pub const RECENT_RANGES: &[(&str, &str)] = &[
    ("today", "Today"),
    ("yesterday", "Since Yesterday"),
    ("7d", "Last 7 Days"),
    ("14d", "Last 14 Days"),
    ("30d", "Last 30 Days"),
    ("3m", "Last 3 Months"),
    ("6m", "Last 6 Months"),
    ("all", "All Pages Sorted by Date"),
];

/// Most pages kept marked unread (oldest marks go first).
pub const MAX_UNREAD: usize = 10_000;

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new(
            "history.markUnread",
            "R.History.Unread.MarkPageAsUnreadContextual",
            "Mark as Unread",
            "History/Unread/Mark as Read/Mark as Unread",
            mark_unread,
        )
        .key("Mod+Q")
        .params(r#"{"page"?: id}"#)
        .pure()
        .when(crate::ops::need_page),
        CommandSpec::new("history.markRead", "R.History.Unread.MarkPageAsReadContextual", "Mark as Read", "History/Unread/Mark as Read", mark_read)
            .params(r#"{"page"?: id, "section"?: id, "group"?: id} (a page, every page of a section or section group)"#)
            .pure(),
        CommandSpec::new(
            "history.markNotebookRead",
            "R.History.Unread.MarkAllPagesAsRead",
            "Mark Notebook as Read",
            "History/Unread/Mark as Read/Mark Notebook as Read",
            |s, v| {
                let nb = match p::id(v, "notebook")? {
                    Some(id) => s.ws.notebook(id).ok_or_else(|| crate::CmdError::Params("no such notebook".into()))?,
                    None => s.notebook().ok_or_else(crate::ops::no_page)?,
                };
                let ids: Vec<notecraft_doc::Id> = nb.sections().iter().flat_map(|sec| sec.pages.iter().map(|p| p.id)).collect();
                s.prefs.unread_pages.retain(|p| !ids.contains(p));
                Ok(Value::Null)
            },
        )
        .params(r#"{"notebook"?: id}"#)
        .pure(),
        CommandSpec::new(
            "history.nextUnread",
            "R.History.Unread.GoToNextUnreadPage",
            "Next Unread",
            "History/Unread/Next Unread",
            next_unread,
        )
        .pure()
        .when(|s| if s.prefs.unread_pages.is_empty() { Some("No unread pages") } else { None }),
        // R.History.Authors.AuthorHighlightingHide: hide (or show again) the authors' initials
        // beside paragraphs other people edited.
        CommandSpec::new("history.hideAuthors", "R.History.Authors.AuthorHighlightingHide", "Hide Authors", "History/Authors/Hide Authors", |s, v| {
            s.prefs.show_authors = !p::bool(v, "on").unwrap_or(s.prefs.show_authors);
            Ok(json!({"hidden": !s.prefs.show_authors}))
        })
        .params(r#"{"on"?: bool (hide)}"#)
        .pure(),
        CommandSpec::new(
            "history.showUnread",
            "R.History.Unread.UnreadNotebookChangesShow",
            "Show Unread Changes in This Notebook",
            "History/Unread/Mark as Read/Show Unread Changes in This Notebook",
            |s, v| {
                s.prefs.show_unread = p::bool(v, "on").unwrap_or(!s.prefs.show_unread);
                Ok(json!({"on": s.prefs.show_unread}))
            },
        )
        .params(r#"{"on"?: bool}"#)
        .pure(),
        CommandSpec::new(
            "history.recentEdits",
            "R.History.Authors.FindByDateMenu",
            "Recent Edits",
            "History/Authors/Recent Edits",
            recent_edits,
        )
        .params(
            r#"{"range"?: "today"|"yesterday"|"7d"|"14d"|"30d"|"3m"|"6m"|"all" (default today), "scope"?: "section"|"sectionGroup"|"notebook"|"all" (default all), "limit"?: int, "open"?: bool (the pane; default true)}"#,
        )
        .pure(),
    ]
}

fn mark_unread(s: &mut Session, v: &Value) -> CmdResult {
    let page = match p::id(v, "page")? {
        Some(id) => id,
        None => s.sel.page.ok_or_else(crate::ops::no_page)?,
    };
    if s.ws.page(page).is_none() {
        return Err(CmdError::Params("no such page".into()));
    }
    if !s.prefs.unread_pages.contains(&page) {
        s.prefs.unread_pages.push(page);
        let extra = s.prefs.unread_pages.len().saturating_sub(MAX_UNREAD);
        s.prefs.unread_pages.drain(..extra);
    }
    Ok(Value::Null)
}

fn mark_read(s: &mut Session, v: &Value) -> CmdResult {
    let pages: Vec<notecraft_doc::Id> = if let Some(sid) = p::id(v, "section")? {
        s.ws.section(sid).ok_or_else(|| CmdError::Params("no such section".into()))?.pages.iter().map(|p| p.id).collect()
    } else if let Some(gid) = p::id(v, "group")? {
        let nb = s.ws.notebooks.iter().find(|nb| nb.find(gid).is_some()).ok_or_else(|| CmdError::Params("no such section group".into()))?;
        nb.sections().iter().filter(|sec| in_group(nb, sec.id, Some(gid))).flat_map(|sec| sec.pages.iter().map(|p| p.id)).collect()
    } else {
        vec![match p::id(v, "page")? {
            Some(id) => id,
            None => s.sel.page.ok_or_else(crate::ops::no_page)?,
        }]
    };
    s.prefs.unread_pages.retain(|p| !pages.contains(p));
    Ok(Value::Null)
}

/// The next unread page after the current one, in notebook order (then from the top).
fn next_unread(s: &mut Session, _: &Value) -> CmdResult {
    let order: Vec<notecraft_doc::Id> =
        s.ws.notebooks.iter().flat_map(|nb| nb.sections().into_iter().flat_map(|sec| sec.pages.iter().map(|p| p.id).collect::<Vec<_>>())).collect();
    let here = s.sel.page.and_then(|cur| order.iter().position(|p| *p == cur)).map_or(0, |i| i + 1);
    let next = order.iter().skip(here).chain(order.iter().take(here)).find(|p| s.prefs.unread_pages.contains(p)).copied();
    // Marks of pages that no longer exist are dropped on the way.
    s.prefs.unread_pages.retain(|p| order.contains(p));
    let next = next.ok_or_else(|| CmdError::Disabled("No unread pages".into()))?;
    s.goto_page(next)?;
    Ok(json!({"page": next.to_string()}))
}

/// The earliest edit time `range` includes, in ms since the epoch (`None` = every page).
pub fn range_start(range: &str, now_ms: i64, utc_offset_min: i32) -> Result<Option<i64>, CmdError> {
    let midnight = || {
        let c = Timestamp(now_ms).civil(utc_offset_min);
        Timestamp::from_civil(c.year, c.month, c.day, 0, 0, utc_offset_min).0
    };
    let days_back = |d: i64| now_ms.saturating_sub(d.saturating_mul(DAY_MS));
    Ok(match range {
        "today" => Some(midnight()),
        "yesterday" => Some(midnight().saturating_sub(DAY_MS)),
        "7d" => Some(days_back(7)),
        "14d" => Some(days_back(14)),
        "30d" => Some(days_back(30)),
        "3m" => Some(days_back(91)),
        "6m" => Some(days_back(182)),
        "all" => None,
        other => return Err(CmdError::Params(format!("unknown range `{other}`"))),
    })
}

fn recent_edits(s: &mut Session, v: &Value) -> CmdResult {
    let range = p::str(v, "range").unwrap_or("today").to_string();
    let since = range_start(&range, s.clock.now_ms, s.clock.utc_offset_min)?;
    let kind = parse_scope(v)?.unwrap_or(ScopeKind::AllNotebooks);
    let limit = p::usize(v, "limit").unwrap_or(MAX_RECENT).clamp(1, MAX_RECENT);
    let (cur_nb, cur_sec) = (s.sel.notebook, s.sel.section);
    let group = match s.scope(kind) {
        notecraft_search::Scope::SectionGroup(g) => Some(g),
        _ => None,
    };
    let off = s.clock.utc_offset_min;
    let mut found: Vec<(i64, Value)> = Vec::new();
    for nb in &s.ws.notebooks {
        if matches!(kind, ScopeKind::Notebook | ScopeKind::SectionGroup) && group.is_none() && Some(nb.id) != cur_nb {
            continue;
        }
        for sec in nb.sections() {
            let inside = match kind {
                ScopeKind::Page | ScopeKind::Section => Some(sec.id) == cur_sec,
                ScopeKind::SectionGroup if group.is_some() => in_group(nb, sec.id, group),
                _ => true,
            };
            if !inside {
                continue;
            }
            for page in &sec.pages {
                let at = page.modified.0;
                if since.is_some_and(|t| at < t) {
                    continue;
                }
                found.push((
                    at,
                    json!({
                        "page": page.id.to_string(),
                        "title": page.display_title(),
                        "section": sec.name,
                        "notebook": nb.name,
                        "modified": at,
                        "date": page.modified.short_date(off),
                        "time": page.modified.short_time(off),
                    }),
                ));
            }
        }
    }
    // Newest first; equal times keep the notebook order.
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    let total = found.len();
    let pages: Vec<Value> = found.into_iter().take(limit).map(|(_, v)| v).collect();
    if p::bool(v, "open").unwrap_or(true) {
        s.view.recent_edits = Some(range.clone());
    }
    Ok(json!({"range": range, "total": total, "pages": pages}))
}

/// Whether section `sid` sits (at any depth) inside section group `group`.
fn in_group(nb: &notecraft_doc::Notebook, sid: notecraft_doc::Id, group: Option<notecraft_doc::Id>) -> bool {
    let Some(g) = group else { return true };
    let Some(path) = nb.find(sid) else { return false };
    (1..path.len()).any(|k| path.get(..k).and_then(|pre| nb.node_at(pre)).map(NavNode::id) == Some(g))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{run, sample};

    #[test]
    fn recent_edits_lists_pages_changed_in_the_period_newest_first() {
        // parity: R.History.Authors.FindByDateMenu, BEH.History.RecentEdits, R.History.Authors.FindToday, R.History.Authors.FindAllChangesByDate
        let mut s = sample();
        let all = run(&mut s, "history.recentEdits", json!({"range": "all"}));
        let n = all["total"].as_u64().unwrap();
        assert!(n >= 3, "every page: {all}");
        assert_eq!(s.view.recent_edits.as_deref(), Some("all"), "the pane opens");
        let times: Vec<i64> = all["pages"].as_array().unwrap().iter().map(|p| p["modified"].as_i64().unwrap()).collect();
        assert!(times.windows(2).all(|w| w[0] >= w[1]), "newest first: {times:?}");
        // Edit one page now: it leads every list and is the only one changed today
        // (the sample's pages are older than the test clock's day).
        let before = run(&mut s, "history.recentEdits", json!({"range": "today"}))["total"].as_u64().unwrap();
        s.clock.now_ms += 3 * DAY_MS;
        run(&mut s, "text.insert", json!({"text": "fresh"}));
        let pid = s.page().unwrap().id.to_string();
        let today = run(&mut s, "history.recentEdits", json!({"range": "today"}));
        assert_eq!(today["total"].as_u64(), Some(1), "only the page just edited: {today} (before: {before})");
        assert_eq!(today["pages"][0]["page"].as_str(), Some(pid.as_str()));
        let all = run(&mut s, "history.recentEdits", json!({"range": "all"}));
        assert_eq!(all["pages"][0]["page"].as_str(), Some(pid.as_str()));
        // Wider periods hold at least as many pages as narrower ones.
        let mut last = 0;
        for (r, _) in RECENT_RANGES {
            let t = run(&mut s, "history.recentEdits", json!({"range": r, "open": false}))["total"].as_u64().unwrap();
            assert!(t >= last, "{r}: {t} < {last}");
            last = t;
        }
        // Scopes: this section holds no more than the notebook, which holds no more than all.
        let count = |s: &mut Session, scope: &str| run(s, "history.recentEdits", json!({"range": "all", "scope": scope}))["total"].as_u64().unwrap();
        let (sec, grp, nbk, every) = (count(&mut s, "section"), count(&mut s, "sectionGroup"), count(&mut s, "notebook"), count(&mut s, "all"));
        assert!(sec >= 1 && sec <= grp && grp <= nbk && nbk <= every, "{sec} {grp} {nbk} {every}");
        assert!(matches!(s.run("history.recentEdits", json!({"range": "decade"})), Err(CmdError::Params(_))));
        assert!(matches!(s.run("history.recentEdits", json!({"scope": "galaxy"})), Err(CmdError::Params(_))));
        assert_eq!(run(&mut s, "history.recentEdits", json!({"range": "all", "limit": 1}))["pages"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn pages_marked_unread_until_opened_and_next_unread_finds_them() {
        // parity: R.History.Unread.MarkPageAsUnreadContextual, R.History.Unread.MarkPageAsReadContextual, R.History.Unread.MarkAllPagesAsRead, R.History.Unread.GoToNextUnreadPage, R.History.Unread.UnreadNotebookChangesShow, BEH.History.Unread
        let mut s = sample();
        let pages: Vec<notecraft_doc::Id> = s.notebook().unwrap().sections().iter().flat_map(|x| x.pages.iter().map(|p| p.id)).collect();
        let here = s.sel.page.unwrap();
        assert!(s.run("history.nextUnread", json!({})).is_err(), "nothing unread");
        run(&mut s, "history.markUnread", json!({"page": pages[3].to_string()}));
        run(&mut s, "history.markUnread", json!({"page": pages[1].to_string()}));
        assert_eq!(crate::commands::by_shortcut("Mod+Q").map(|c| c.id), Some("history.markUnread"));
        // Next Unread goes forward from here, opening reads the page.
        assert_eq!(run(&mut s, "history.nextUnread", json!({}))["page"], pages[1].to_string());
        assert_eq!(s.prefs.unread_pages, vec![pages[3]]);
        run(&mut s, "history.nextUnread", json!({}));
        assert!(s.prefs.unread_pages.is_empty());
        // A whole section, then the notebook.
        let sec = s.notebook().unwrap().sections()[1].clone();
        for p in &sec.pages {
            run(&mut s, "history.markUnread", json!({"page": p.id.to_string()}));
        }
        run(&mut s, "history.markRead", json!({"section": sec.id.to_string()}));
        assert!(s.prefs.unread_pages.is_empty());
        run(&mut s, "history.markUnread", json!({"page": here.to_string()}));
        run(&mut s, "history.markNotebookRead", json!({}));
        assert!(s.prefs.unread_pages.is_empty());
        assert_eq!(run(&mut s, "history.showUnread", json!({}))["on"], false);
        // Reading is not an edit: no undo step.
        run(&mut s, "history.markUnread", json!({}));
        assert!(s.undo_labels().is_empty());
        assert!(s.run("history.markUnread", json!({"page": notecraft_doc::Id::NIL.to_string()})).is_err());
    }

    #[test]
    fn periods_count_from_local_midnight() {
        // parity: R.History.Authors.FindSinceYesterday, R.History.Authors.FindSinceSevenDays
        // 2026-10-31 15:00 at UTC+2.
        let now = Timestamp::from_civil(2026, 10, 31, 15, 0, 120).0;
        let today = range_start("today", now, 120).unwrap().unwrap();
        assert_eq!(today, Timestamp::from_civil(2026, 10, 31, 0, 0, 120).0);
        assert_eq!(range_start("yesterday", now, 120).unwrap(), Some(today - DAY_MS));
        assert_eq!(range_start("7d", now, 120).unwrap(), Some(now - 7 * DAY_MS));
        assert_eq!(range_start("all", now, 120).unwrap(), None);
        // Hostile clocks never overflow.
        assert!(range_start("6m", i64::MIN, 0).unwrap().is_some());
        let _ = range_start("today", i64::MAX, 0);
    }
}
