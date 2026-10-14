//! Search: the search box across scopes (catalog `BEH.Search.*`) and Find on page (Ctrl+F).

use notecraft_search::find_all;
use serde_json::{Value, json};

use crate::ops::need_page;
use crate::selection::{Block, Pos};
use crate::session::ScopeKind;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn need_find(s: &Session) -> Option<&'static str> {
    if s.view.find.matches.is_empty() { Some("No matches on this page") } else { None }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // QAT.NR.NotebookSearch — Ctrl+E: search in the current scope; results are pages.
        CommandSpec::new(
            "search.all",
            "QAT.NR.NotebookSearch",
            "Search Notebooks",
            "Quick Access Toolbar/Commands Not in the Ribbon/Search Notebooks",
            search_all,
        )
        .key("Mod+E")
        .params(r#"{"query": string, "scope"?: "page"|"section"|"sectionGroup"|"notebook"|"all", "limit"?: int}"#)
        .pure(),
        // KB.Search.Page — Ctrl+F: find on the current page.
        CommandSpec::new("search.page", "KB.Search.Page", "Find on Page", "Keyboard/Search/Search the current page", find_page)
            .key("Mod+F")
            .params(r#"{"query"?: string}"#)
            .pure()
            .when(need_page),
        CommandSpec::new("search.next", "KB.Search.Next", "Next Match", "Keyboard/Search/Next result on the page", |s, _| step(s, 1))
            .key("F3")
            .pure()
            .when(need_find),
        CommandSpec::new("search.prev", "KB.Search.Prev", "Previous Match", "Keyboard/Search/Previous result on the page", |s, _| step(s, -1))
            .key("Shift+F3")
            .pure()
            .when(need_find),
        CommandSpec::new(
            "search.dismiss",
            "KB.Search.Dismiss",
            "Close Search",
            "Keyboard/Search/Dismiss the search and return to the page (Esc)",
            dismiss,
        )
        .pure(),
        CommandSpec::new(
            "search.resultsPane",
            "KB.Search.ResultsPane",
            "Search Results Pane",
            "Keyboard/Search/Open the Search Results pane (after searching)",
            results_pane,
        )
        .key("Alt+O")
        .pure(),
        CommandSpec::new(
            "search.openResult",
            "KB.Search.GoToResult",
            "Go to Result",
            "Keyboard/Search/Go to the selected result and dismiss search (Enter)",
            open_result,
        )
        .params(r#"{"page": id, "query"?: string}"#)
        .pure(),
        CommandSpec::new("search.scopePage", "CM.SearchScope.SearchScopePage", "This Page", "Context/Search Scope/This Page", |s, _| {
            set_scope(s, ScopeKind::Page)
        })
        .pure(),
        CommandSpec::new("search.scopeSection", "CM.SearchScope.SearchScopeSection", "This Section", "Context/Search Scope/This Section", |s, _| {
            set_scope(s, ScopeKind::Section)
        })
        .pure(),
        CommandSpec::new(
            "search.scopeSectionGroup",
            "CM.SearchScope.SearchScopeSectionGroup",
            "This Section Group",
            "Context/Search Scope/This Section Group",
            |s, _| set_scope(s, ScopeKind::SectionGroup),
        )
        .pure(),
        CommandSpec::new(
            "search.scopeNotebook",
            "CM.SearchScope.SearchScopeNotebook",
            "This Notebook",
            "Context/Search Scope/This Notebook",
            |s, _| set_scope(s, ScopeKind::Notebook),
        )
        .pure(),
        CommandSpec::new(
            "search.scopeAll",
            "CM.SearchScope.SearchScopeAllNotebooks",
            "All Notebooks",
            "Context/Search Scope/All Notebooks",
            |s, _| set_scope(s, ScopeKind::AllNotebooks),
        )
        .pure(),
        CommandSpec::new(
            "search.setDefaultScope",
            "CM.SearchScope.SearchScopeSetDefault",
            "Set This Scope as Default",
            "Context/Search Scope/Set This Scope as Default",
            |s, _| {
                s.prefs.default_scope = s.view.scope;
                Ok(Value::Null)
            },
        )
        .pure(),
    ]
}

pub(crate) fn parse_scope(v: &Value) -> Result<Option<ScopeKind>, CmdError> {
    Ok(match p::str(v, "scope") {
        None => None,
        Some("page") => Some(ScopeKind::Page),
        Some("section") => Some(ScopeKind::Section),
        Some("sectionGroup") => Some(ScopeKind::SectionGroup),
        Some("notebook") => Some(ScopeKind::Notebook),
        Some("all") => Some(ScopeKind::AllNotebooks),
        Some(o) => return Err(CmdError::Params(format!("unknown scope `{o}`"))),
    })
}

fn search_all(s: &mut Session, v: &Value) -> CmdResult {
    let q = p::req_str(v, "query")?;
    if q.len() > 4096 {
        return Err(CmdError::Params("query is too long".into()));
    }
    let kind = parse_scope(v)?.unwrap_or(s.view.scope);
    let scope = s.scope(kind);
    let limit = p::usize(v, "limit").unwrap_or(500).clamp(1, 100_000);
    let hits = s.index().search(q, scope);
    let total = hits.len();
    let items: Vec<Value> = hits
        .into_iter()
        .take(limit)
        .map(|h| {
            let section = s.ws.section(h.meta.section).map(|x| x.name.clone()).unwrap_or_default();
            let title = s.ws.page(h.page).map(|p| p.display_title()).unwrap_or_default();
            json!({"page": h.page.to_string(), "title": title, "section": section, "inTitle": h.in_title, "snippet": h.snippet})
        })
        .collect();
    Ok(json!({"total": total, "results": items}))
}

/// Find `query` on the current page (title then containers, in z-order) and select the first
/// match.
pub(crate) fn find_on_page(s: &mut Session, query: &str) -> usize {
    let mut matches = Vec::new();
    if let Some(page) = s.page()
        && !query.is_empty()
    {
        for r in find_all(&page.title.text(), query) {
            matches.push((Block::Title, 0, r.start, r.end));
        }
        for o in &page.objects {
            if let Some(ol) = o.as_outline() {
                for (i, p) in ol.paragraphs.iter().enumerate() {
                    for r in find_all(&p.text(), query) {
                        matches.push((Block::Outline(ol.id), i, r.start, r.end));
                    }
                }
            }
        }
    }
    let n = matches.len();
    s.view.find = crate::FindState { active: true, query: query.to_string(), matches, current: 0 };
    select_current(s);
    n
}

fn select_current(s: &mut Session) {
    if let Some(&(block, para, a, b)) = s.view.find.matches.get(s.view.find.current) {
        s.set_range(Pos::new(block, para, a), Pos::new(block, para, b));
        s.scroll_request = Some(crate::ScrollRequest::Caret);
    }
}

fn find_page(s: &mut Session, v: &Value) -> CmdResult {
    let q = p::str(v, "query").unwrap_or("").to_string();
    if q.len() > 4096 {
        return Err(CmdError::Params("query is too long".into()));
    }
    let n = find_on_page(s, &q);
    Ok(json!({"matches": n}))
}

fn step(s: &mut Session, d: i64) -> CmdResult {
    let n = s.view.find.matches.len();
    if n == 0 {
        return Ok(Value::Null);
    }
    s.view.find.current = (s.view.find.current as i64 + d).rem_euclid(n as i64) as usize;
    select_current(s);
    Ok(json!({"current": s.view.find.current + 1, "of": n}))
}

fn dismiss(s: &mut Session, _: &Value) -> CmdResult {
    s.view.find = crate::FindState::default();
    Ok(Value::Null)
}

fn results_pane(s: &mut Session, _: &Value) -> CmdResult {
    s.view.find.active = false;
    Ok(json!({"pane": "search"}))
}

fn open_result(s: &mut Session, v: &Value) -> CmdResult {
    let page = p::req_id(v, "page")?;
    s.goto_page(page)?;
    if let Some(q) = p::str(v, "query") {
        // Highlight the query's words on the page, as the reference does after a search.
        let first_word = q.trim_matches('"').split_whitespace().next().unwrap_or("").to_string();
        find_on_page(s, &first_word);
    }
    Ok(Value::Null)
}

fn set_scope(s: &mut Session, k: ScopeKind) -> CmdResult {
    s.view.scope = k;
    Ok(Value::Null)
}
