//! Note tags (Home › Tags): applying, checking and removing tags, Find Tags (the Tags
//! Summary), summary pages and custom tags.

use std::sync::Arc;

use notecraft_doc::{Outline, PageObject, Paragraph, Run, TagDef, TagSymbol, TagUse};
use serde_json::{Value, json};

use crate::ops::{self, need_page, need_text, no_page};
use crate::selection::{Block, Pos};
use crate::session::ScopeKind;
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

fn need_tagged(s: &Session) -> Option<&'static str> {
    if let Some(w) = need_text(s) {
        return Some(w);
    }
    let Some(page) = s.page() else { return Some("No page is open") };
    let any = ops::touched(s)
        .iter()
        .any(|(b, r)| Session::block_paras(page, *b).is_some_and(|ps| ps.get(r.clone()).is_some_and(|x| x.iter().any(|p| !p.tags.is_empty()))));
    if any { None } else { Some("The paragraph has no tag") }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // R.Home.Tags.TagsGallery — apply (or remove, when every paragraph has it) one tag.
        CommandSpec::new("tag.toggle", "R.Home.Tags.TagsGallery", "Tags", "Home/Tags", toggle).params(r#"{"key": tag key}"#).when(need_text),
        // R.Home.Tags.ToDoTag — Ctrl+1 cycles: none → unchecked → checked → none
        // (KB.Tag.ToDo "apply, select, or clear"; exact cycle unverified).
        CommandSpec::new("tag.todo", "R.Home.Tags.ToDoTag", "To Do Tag", "Home/Tags/To Do Tag", |s, _| shortcut(s, "todo")).key("Mod+1").when(need_text),
        CommandSpec::new("tag.important", "R.Home.Tags.Important", "Important", "Home/Tags/Important", |s, _| shortcut(s, "important")).key("Mod+2").when(need_text),
        CommandSpec::new("tag.question", "R.Home.Tags.Question", "Question", "Home/Tags/Question", |s, _| shortcut(s, "question")).key("Mod+3").when(need_text),
        CommandSpec::new("tag.rememberForLater", "R.Home.Tags.RememberForLater", "Remember for later", "Home/Tags/Remember for later", |s, _| shortcut(s, "remember-for-later"))
            .key("Mod+4")
            .when(need_text),
        CommandSpec::new("tag.definition", "R.Home.Tags.Definition", "Definition", "Home/Tags/Definition", |s, _| shortcut(s, "definition")).key("Mod+5").when(need_text),
        CommandSpec::new("tag.highlight", "R.Home.Tags.Highlight", "Highlight", "Home/Tags/Highlight", |s, _| shortcut(s, "highlight")).key("Mod+6").when(need_text),
        CommandSpec::new("tag.contact", "R.Home.Tags.Contact", "Contact", "Home/Tags/Contact", |s, _| shortcut(s, "contact")).key("Mod+7").when(need_text),
        CommandSpec::new("tag.address", "R.Home.Tags.Address", "Address", "Home/Tags/Address", |s, _| shortcut(s, "address")).key("Mod+8").when(need_text),
        CommandSpec::new("tag.phoneNumber", "R.Home.Tags.PhoneNumber", "Phone number", "Home/Tags/Phone number", |s, _| shortcut(s, "phone-number")).key("Mod+9").when(need_text),
        // R.Home.Tags.TagsRemoveAll — Ctrl+0.
        CommandSpec::new("tag.removeAll", "R.Home.Tags.TagsRemoveAll", "Remove Tag", "Home/Tags/Remove Tag", remove_all).key("Mod+0").when(need_tagged),
        CommandSpec::new("tag.check", "CM.Tag.NoteTagCheck", "Check Tag", "Context/Tag/Check Tag", |s, v| set_checked(s, v, true)).params(r#"{"key"?: tag key}"#).when(need_tagged),
        CommandSpec::new("tag.uncheck", "CM.Tag.NoteTagUncheck", "Uncheck Tag", "Context/Tag/Uncheck Tag", |s, v| set_checked(s, v, false)).params(r#"{"key"?: tag key}"#).when(need_tagged),
        CommandSpec::new("tag.remove", "CM.Tag.TagsRemove", "Remove Tag", "Context/Tag/Remove Tag", remove_one).params(r#"{"key": tag key}"#).when(need_tagged),
        // CM.InkDrawing.TagsMarkAll / TagsClearAll — check or clear every to-do on the selection.
        CommandSpec::new("tag.checkAll", "CM.InkDrawing.TagsMarkAll", "Check All Tags", "Context/Ink Drawing/Check All Tags", |s, _| check_all(s, true)).when(need_tagged),
        CommandSpec::new("tag.clearAll", "CM.InkDrawing.TagsClearAll", "Clear All Checkmarks", "Context/Ink Drawing/Clear All Checkmarks", |s, _| check_all(s, false)).when(need_tagged),
        // NC.Tag.Click — clicking a checkbox toggles it (BEH.Tags.CheckboxClick).
        CommandSpec::new("tag.click", "BEH.Tags.CheckboxClick", "Toggle Checkbox", "Behaviour/Tags", click).params(r#"{"block": outline id, "para": int, "tag"?: index}"#).when(need_page),
        // R.Home.Tags.FindTaggedNotes — the Tags Summary: every tagged paragraph in a scope,
        // grouped (BEH.Tags.Summary).
        CommandSpec::new("tag.findTags", "R.Home.Tags.FindTaggedNotes", "Find Tags", "Home/Tags/Find Tags", find_tags)
            .params(r#"{"scope"?: "page"|"section"|"sectionGroup"|"notebook"|"all", "group"?: "tag"|"section"|"title"|"date"|"text", "unchecked"?: bool}"#)
            .pure(),
        // BEH.Tags.SummaryPage — Create Summary Page.
        CommandSpec::new("tag.summaryPage", "BEH.Tags.SummaryPage", "Create Summary Page", "Behaviour/Tags", summary_page).params(r#"{"scope"?: ..., "group"?: ..., "unchecked"?: bool}"#).when(|s| {
            if s.sel.section.is_some() { None } else { Some("Open a section first") }
        }),
        // R.Home.Tags.TagsCustomize — add, modify or remove a custom tag (stored in preferences).
        CommandSpec::new("tag.customize", "R.Home.Tags.TagsCustomize", "Customize Tags", "Home/Tags/Customize Tags", customize)
            .params(r##"{"action": "add"|"modify"|"remove", "key"?: string, "name"?: string, "symbol"?: string, "fontColor"?: "#rrggbb", "highlight"?: "#rrggbb"}"##)
            .pure(),
    ]
}

fn def_for(s: &Session, key: &str) -> Result<TagDef, CmdError> {
    s.tag_defs().into_iter().find(|d| d.key == key).ok_or_else(|| CmdError::Params(format!("unknown tag `{key}`")))
}

/// Paragraph locations touched by the selection (materialising a pending container).
fn targets(s: &mut Session) -> Result<Vec<(Block, usize)>, CmdError> {
    if ops::touched(s).is_empty() && s.pending_outline.is_some() {
        ops::ensure_caret(s)?;
    }
    let t: Vec<(Block, usize)> = ops::touched(s).into_iter().filter(|(b, _)| *b != Block::Title).flat_map(|(b, r)| r.map(move |i| (b, i))).collect();
    if t.is_empty() {
        return Err(CmdError::Disabled("click in a note first (the page title can't be tagged)".into()));
    }
    Ok(t)
}

fn toggle(s: &mut Session, v: &Value) -> CmdResult {
    let key = p::req_str(v, "key")?;
    let def = def_for(s, key)?;
    let targets = targets(s)?;
    let page = s.page().ok_or_else(no_page)?;
    let all_have = targets
        .iter()
        .all(|(b, i)| Session::block_paras(page, *b).and_then(|ps| ps.get(*i)).is_some_and(|p| p.tags.iter().any(|t| t.key == def.key)));
    let page = s.page_mut().ok_or_else(no_page)?;
    for (b, i) in targets {
        if let Some(p) = Session::para_mut(page, b, i) {
            if all_have {
                p.tags.retain(|t| t.key != def.key);
            } else if !p.tags.iter().any(|t| t.key == def.key) {
                p.tags.push(TagUse::of(&def));
            }
        }
    }
    Ok(Value::Null)
}

/// The Ctrl+1…9 behaviour: to-do tags cycle unchecked → checked → removed; other tags toggle.
fn shortcut(s: &mut Session, key: &str) -> CmdResult {
    let def = def_for(s, key)?;
    if !def.is_todo() {
        return toggle(s, &json!({"key": key}));
    }
    let targets = targets(s)?;
    let page = s.page().ok_or_else(no_page)?;
    let state = |b: Block, i: usize| {
        Session::block_paras(page, b).and_then(|ps| ps.get(i)).and_then(|p| p.tags.iter().find(|t| t.key == def.key).map(|t| t.checked))
    };
    // Decide by the first paragraph, apply to all.
    let first = targets.first().and_then(|(b, i)| state(*b, *i));
    let page = s.page_mut().ok_or_else(no_page)?;
    for (b, i) in targets {
        if let Some(p) = Session::para_mut(page, b, i) {
            match first {
                None => {
                    if !p.tags.iter().any(|t| t.key == def.key) {
                        p.tags.push(TagUse::of(&def));
                    }
                }
                Some(false) => p.tags.iter_mut().filter(|t| t.key == def.key).for_each(|t| t.checked = true),
                Some(true) => p.tags.retain(|t| t.key != def.key),
            }
        }
    }
    Ok(Value::Null)
}

fn remove_all(s: &mut Session, _: &Value) -> CmdResult {
    for (b, i) in targets(s)? {
        if let Some(p) = s.page_mut().and_then(|pg| Session::para_mut(pg, b, i)) {
            p.tags.clear();
        }
    }
    Ok(Value::Null)
}

fn remove_one(s: &mut Session, v: &Value) -> CmdResult {
    let key = p::req_str(v, "key")?.to_string();
    for (b, i) in targets(s)? {
        if let Some(p) = s.page_mut().and_then(|pg| Session::para_mut(pg, b, i)) {
            p.tags.retain(|t| t.key != key);
        }
    }
    Ok(Value::Null)
}

fn set_checked(s: &mut Session, v: &Value, on: bool) -> CmdResult {
    let key = p::str(v, "key").map(str::to_string);
    for (b, i) in targets(s)? {
        if let Some(p) = s.page_mut().and_then(|pg| Session::para_mut(pg, b, i)) {
            for t in p.tags.iter_mut().filter(|t| t.symbol.is_checkbox() && key.as_ref().is_none_or(|k| *k == t.key)) {
                t.checked = on;
            }
        }
    }
    Ok(Value::Null)
}

fn check_all(s: &mut Session, on: bool) -> CmdResult {
    set_checked(s, &Value::Null, on)
}

fn click(s: &mut Session, v: &Value) -> CmdResult {
    let block = match v.get("block").and_then(Value::as_str).and_then(notecraft_doc::Id::parse) {
        Some(id) => Block::Outline(id),
        None => return Err(CmdError::Params("`block` (outline id) is required".into())),
    };
    let para = p::req_usize(v, "para")?;
    let which = p::usize(v, "tag").unwrap_or(0);
    let page = s.page_mut().ok_or_else(no_page)?;
    let p = Session::para_mut(page, block, para).ok_or_else(|| CmdError::Params("no such paragraph".into()))?;
    let t =
        p.tags.iter_mut().filter(|t| t.symbol.is_checkbox()).nth(which).ok_or_else(|| CmdError::Params("that paragraph has no checkbox".into()))?;
    t.checked = !t.checked;
    Ok(json!({"checked": t.checked}))
}

/// One tagged paragraph found by Find Tags.
struct Found {
    tag: TagUse,
    text: String,
    page: notecraft_doc::Id,
    page_title: String,
    section: String,
    date: notecraft_doc::Timestamp,
    block: notecraft_doc::Id,
    para: usize,
}

fn scope_kind(v: &Value) -> Result<Option<ScopeKind>, CmdError> {
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

fn collect(s: &Session, kind: ScopeKind, unchecked_only: bool) -> Vec<Found> {
    let scope = s.scope(kind);
    let mut out = Vec::new();
    for nb in &s.ws.notebooks {
        crate::session::walk_pages(nb, &mut |meta, page| {
            let admitted = match scope {
                notecraft_search::Scope::Page(id) => meta.page == id,
                notecraft_search::Scope::Section(id) => meta.section == id,
                notecraft_search::Scope::SectionGroup(id) => meta.groups.contains(&id),
                notecraft_search::Scope::Notebook(id) => meta.notebook == id,
                notecraft_search::Scope::All => true,
            };
            if !admitted {
                return;
            }
            let section = s.ws.section(meta.section).map(|x| x.name.clone()).unwrap_or_default();
            for o in &page.objects {
                let Some(ol) = o.as_outline() else { continue };
                for (i, para) in ol.paragraphs.iter().enumerate() {
                    for t in &para.tags {
                        if unchecked_only && t.checked {
                            continue;
                        }
                        out.push(Found {
                            tag: t.clone(),
                            text: para.text().replace('\n', " "),
                            page: page.id,
                            page_title: page.display_title(),
                            section: section.clone(),
                            date: page.modified,
                            block: ol.id,
                            para: i,
                        });
                    }
                }
            }
        });
    }
    out
}

fn group_key(f: &Found, group: &str, offset: i32) -> String {
    match group {
        "section" => f.section.clone(),
        "title" => f.page_title.clone(),
        "date" => f.date.short_date(offset),
        "text" => f.text.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
        _ => f.tag.name.clone(),
    }
}

fn find_tags(s: &mut Session, v: &Value) -> CmdResult {
    let kind = scope_kind(v)?.unwrap_or(ScopeKind::AllNotebooks);
    let group = p::str(v, "group").unwrap_or("tag");
    if !matches!(group, "tag" | "section" | "title" | "date" | "text") {
        return Err(CmdError::Params(format!("unknown grouping `{group}`")));
    }
    let found = collect(s, kind, p::bool(v, "unchecked").unwrap_or(false));
    let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
    for f in &found {
        let k = group_key(f, group, s.clock.utc_offset_min);
        let item = json!({"tag": f.tag.name, "key": f.tag.key, "checked": f.tag.checked, "text": f.text, "page": f.page.to_string(), "pageTitle": f.page_title, "section": f.section, "block": f.block.to_string(), "para": f.para});
        match groups.iter_mut().find(|(g, _)| *g == k) {
            Some((_, items)) => items.push(item),
            None => groups.push((k, vec![item])),
        }
    }
    s.view.tags_pane = true;
    Ok(json!({"count": found.len(), "groups": groups.into_iter().map(|(g, items)| json!({"group": g, "items": items})).collect::<Vec<_>>()}))
}

fn summary_page(s: &mut Session, v: &Value) -> CmdResult {
    let kind = scope_kind(v)?.unwrap_or(ScopeKind::AllNotebooks);
    let group = p::str(v, "group").unwrap_or("tag").to_string();
    let found = collect(s, kind, p::bool(v, "unchecked").unwrap_or(false));
    let sid = s.sel.section.ok_or_else(no_page)?;
    let now = s.now();
    let author = s.author();
    let page_id = s.new_id();
    let title_id = s.new_id();
    let mut page = notecraft_doc::Page::new(page_id, title_id, now, &author);
    page.title = Paragraph::with_text(title_id, "Tags Summary");
    page.title.style = notecraft_doc::ParaStyle::PageTitle;
    let oid = s.new_id();
    let first = s.new_id();
    let mut ol = Outline::new(oid, notecraft_doc::geom::Point::new(ops::DEFAULT_LEFT, ops::CONTENT_TOP), first);
    ol.paragraphs.clear();
    let off = s.clock.utc_offset_min;
    let mut groups: Vec<String> = Vec::new();
    for f in &found {
        let k = group_key(f, &group, off);
        if !groups.contains(&k) {
            groups.push(k);
        }
    }
    for g in groups {
        let mut h = Paragraph::with_text(s.new_id(), &g);
        h.style = notecraft_doc::ParaStyle::Heading2;
        ol.paragraphs.push(h);
        for f in found.iter().filter(|f| group_key(f, &group, off) == g) {
            let mut p = Paragraph::new(s.new_id());
            p.runs.push(Run::new(&f.text, notecraft_doc::CharFormat::default()));
            p.tags.push(f.tag.clone());
            p.level = 1;
            ol.paragraphs.push(p);
        }
    }
    if ol.paragraphs.is_empty() {
        ol.paragraphs.push(Paragraph::with_text(s.new_id(), "No tagged notes were found."));
    }
    page.objects.push(PageObject::Outline(ol));
    let section = s.ws.section_mut(sid).ok_or_else(no_page)?;
    section.pages.push(Arc::new(page));
    s.goto_page(page_id)?;
    s.set_caret(Pos::new(Block::Title, 0, "Tags Summary".len()));
    Ok(json!({"page": page_id.to_string(), "count": found.len()}))
}

fn parse_symbol(name: &str) -> Option<TagSymbol> {
    use TagSymbol as S;
    Some(match name {
        "none" => S::None,
        "checkbox" => S::Checkbox,
        "star" => S::Star,
        "question" => S::Question,
        "person" => S::Person,
        "house" => S::House,
        "phone" => S::Phone,
        "globe" => S::Globe,
        "bulb" => S::Bulb,
        "key" => S::Key,
        "exclamation" => S::Exclamation,
        "flag" => S::FlagA,
        "film" => S::Film,
        "book" => S::Book,
        "music" => S::Music,
        "document" => S::Document,
        "pen" => S::Pen,
        "envelope" => S::Envelope,
        _ => return None,
    })
}

fn customize(s: &mut Session, v: &Value) -> CmdResult {
    let action = p::req_str(v, "action")?;
    let color = |k: &str| -> Result<Option<notecraft_doc::color::Rgba>, CmdError> {
        match p::str(v, k) {
            None => Ok(None),
            Some(c) => notecraft_doc::color::Rgba::from_hex(c).map(Some).ok_or_else(|| CmdError::Params(format!("`{k}` must be #rrggbb"))),
        }
    };
    match action {
        "add" => {
            if s.prefs.custom_tags.len() >= notecraft_doc::tags::MAX_CUSTOM_TAGS {
                return Err(CmdError::Failed("too many custom tags".into()));
            }
            let name = notecraft_doc::clean_name(p::req_str(v, "name")?);
            if name.is_empty() {
                return Err(CmdError::Params("`name` can't be empty".into()));
            }
            let symbol = match p::str(v, "symbol") {
                Some(n) => parse_symbol(n).ok_or_else(|| CmdError::Params(format!("unknown symbol `{n}`")))?,
                None => TagSymbol::Star,
            };
            let key = format!("custom-{}", s.new_id());
            s.prefs.custom_tags.push(TagDef { key: key.clone(), name, symbol, font_color: color("fontColor")?, highlight: color("highlight")? });
            Ok(json!({"key": key}))
        }
        "modify" => {
            let key = p::req_str(v, "key")?;
            let fc = color("fontColor")?;
            let hl = color("highlight")?;
            let sym = match p::str(v, "symbol") {
                Some(n) => Some(parse_symbol(n).ok_or_else(|| CmdError::Params(format!("unknown symbol `{n}`")))?),
                None => None,
            };
            let def =
                s.prefs.custom_tags.iter_mut().find(|d| d.key == key).ok_or_else(|| CmdError::Params("only custom tags can be modified".into()))?;
            if let Some(n) = p::str(v, "name") {
                def.name = notecraft_doc::clean_name(n);
            }
            if let Some(sym) = sym {
                def.symbol = sym;
            }
            if fc.is_some() {
                def.font_color = fc;
            }
            if hl.is_some() {
                def.highlight = hl;
            }
            Ok(Value::Null)
        }
        "remove" => {
            let key = p::req_str(v, "key")?;
            let before = s.prefs.custom_tags.len();
            s.prefs.custom_tags.retain(|d| d.key != key);
            if s.prefs.custom_tags.len() == before {
                return Err(CmdError::Params("no such custom tag".into()));
            }
            Ok(Value::Null)
        }
        o => Err(CmdError::Params(format!("unknown action `{o}`"))),
    }
}
