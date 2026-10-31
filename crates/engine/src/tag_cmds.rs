//! Note tags (Home › Tags): applying, checking and removing tags, Find Tags (the Tags
//! Summary), summary pages and custom tags. Tags go on paragraphs and on ink (decision D5,
//! `docs/plan.md` §4.11): a tagged drawing carries the tag on one of its strokes.

use std::sync::Arc;

use notecraft_doc::tags::MAX_OBJECT_TAGS;
use notecraft_doc::{Id, Outline, Page, PageObject, Paragraph, Run, TagDef, TagSymbol, TagUse};
use serde_json::{Value, json};

use crate::draw_cmds::selected_ink;
use crate::ops::{self, need_page, need_text, no_page};
use crate::selection::{Block, PageSel, Pos};
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
    let ink = selected_ink(s);
    if any || ink.iter().any(|id| page.ink(*id).is_some_and(|k| !k.tags.is_empty())) {
        None
    } else if ink.is_empty() {
        Some("The paragraph has no tag")
    } else {
        Some("The ink has no tag")
    }
}

/// Tags go on text and ink: a selection of pictures or attachments alone can't be tagged.
fn need_taggable(s: &Session) -> Option<&'static str> {
    if let Some(w) = need_text(s) {
        return Some(w);
    }
    if matches!(s.sel.on_page, PageSel::Objects { .. }) && ops::touched(s).is_empty() && selected_ink(s).is_empty() {
        return Some("Select text or ink to tag");
    }
    None
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        // R.Home.Tags.TagsGallery — apply (or remove, when every paragraph has it) one tag.
        CommandSpec::new("tag.toggle", "R.Home.Tags.TagsGallery", "Tags", "Home/Tags", toggle).params(r#"{"key": tag key}"#).when(need_taggable),
        // R.Home.Tags.ToDoTag — Ctrl+1 cycles: none → unchecked → checked → none
        // (KB.Tag.ToDo "apply, select, or clear"; exact cycle unverified).
        CommandSpec::new("tag.todo", "R.Home.Tags.ToDoTag", "To Do Tag", "Home/Tags/To Do Tag", |s, _| shortcut(s, "todo")).key("Mod+1").when(need_taggable),
        CommandSpec::new("tag.important", "R.Home.Tags.Important", "Important", "Home/Tags/Important", |s, _| shortcut(s, "important")).key("Mod+2").when(need_taggable),
        CommandSpec::new("tag.question", "R.Home.Tags.Question", "Question", "Home/Tags/Question", |s, _| shortcut(s, "question")).key("Mod+3").when(need_taggable),
        CommandSpec::new("tag.rememberForLater", "R.Home.Tags.RememberForLater", "Remember for later", "Home/Tags/Remember for later", |s, _| shortcut(s, "remember-for-later"))
            .key("Mod+4")
            .when(need_taggable),
        CommandSpec::new("tag.definition", "R.Home.Tags.Definition", "Definition", "Home/Tags/Definition", |s, _| shortcut(s, "definition")).key("Mod+5").when(need_taggable),
        CommandSpec::new("tag.highlight", "R.Home.Tags.Highlight", "Highlight", "Home/Tags/Highlight", |s, _| shortcut(s, "highlight")).key("Mod+6").when(need_taggable),
        CommandSpec::new("tag.contact", "R.Home.Tags.Contact", "Contact", "Home/Tags/Contact", |s, _| shortcut(s, "contact")).key("Mod+7").when(need_taggable),
        CommandSpec::new("tag.address", "R.Home.Tags.Address", "Address", "Home/Tags/Address", |s, _| shortcut(s, "address")).key("Mod+8").when(need_taggable),
        CommandSpec::new("tag.phoneNumber", "R.Home.Tags.PhoneNumber", "Phone number", "Home/Tags/Phone number", |s, _| shortcut(s, "phone-number")).key("Mod+9").when(need_taggable),
        // R.Home.Tags.TagsRemoveAll — Ctrl+0.
        CommandSpec::new("tag.removeAll", "R.Home.Tags.TagsRemoveAll", "Remove Tag", "Home/Tags/Remove Tag", remove_all).key("Mod+0").when(need_tagged),
        CommandSpec::new("tag.check", "CM.Tag.NoteTagCheck", "Check Tag", "Context/Tag/Check Tag", |s, v| set_checked(s, v, true)).params(r#"{"key"?: tag key}"#).when(need_tagged),
        CommandSpec::new("tag.uncheck", "CM.Tag.NoteTagUncheck", "Uncheck Tag", "Context/Tag/Uncheck Tag", |s, v| set_checked(s, v, false)).params(r#"{"key"?: tag key}"#).when(need_tagged),
        CommandSpec::new("tag.remove", "CM.Tag.TagsRemove", "Remove Tag", "Context/Tag/Remove Tag", remove_one).params(r#"{"key"?: tag key (default: the tag the menu was opened on)}"#).when(need_tagged),
        // CM.InkDrawing.TagsMarkAll / TagsClearAll — check or clear every to-do on the selection.
        CommandSpec::new("tag.checkAll", "CM.InkDrawing.TagsMarkAll", "Check All Tags", "Context/Ink Drawing/Check All Tags", |s, _| check_all(s, true)).when(need_tagged),
        CommandSpec::new("tag.clearAll", "CM.InkDrawing.TagsClearAll", "Clear All Checkmarks", "Context/Ink Drawing/Clear All Checkmarks", |s, _| check_all(s, false)).when(need_tagged),
        // NC.Tag.Click — clicking a checkbox toggles it (BEH.Tags.CheckboxClick), on a paragraph
        // or on a stroke.
        CommandSpec::new("tag.click", "BEH.Tags.CheckboxClick", "Toggle Checkbox", "Behaviour/Tags", click)
            .params(r#"{"block": outline or ink id, "para"?: int (required for outlines), "tag"?: index}"#)
            .when(need_page),
        // R.Home.Tags.FindTaggedNotes — the Tags Summary: every tagged paragraph in a scope,
        // grouped (BEH.Tags.Summary).
        CommandSpec::new("tag.findTags", "R.Home.Tags.FindTaggedNotes", "Find Tags", "Home/Tags/Find Tags", find_tags)
            .params(r#"{"scope"?: "page"|"section"|"sectionGroup"|"notebook"|"all", "group"?: "tag"|"section"|"title"|"date"|"text", "unchecked"?: bool}"#)
            .pure(),
        // BEH.Tags.SummaryPage — Create Summary Page.
        CommandSpec::new("tag.summaryPage", "BEH.Tags.SummaryPage", "Create Summary Page", "Behaviour/Tags", summary_page)
            .params(r#"{"scope"?: ..., "group"?: ..., "unchecked"?: bool}"#)
            .when(ops::need_section),
        // R.Home.Tags.TagsCustomize — add, modify or remove a custom tag (stored in preferences).
        CommandSpec::new("tag.customize", "R.Home.Tags.TagsCustomize", "Customize Tags", "Home/Tags/Customize Tags", customize)
            .params(r##"{"action": "add"|"modify"|"remove"|"moveUp"|"moveDown"|"reset", "key"?: string (any tag, built-in or custom), "name"?: string, "symbol"?: string, "fontColor"?: "#rrggbb"|null, "highlight"?: "#rrggbb"|null}"##)
            .pure(),
    ]
}

fn def_for(s: &Session, key: &str) -> Result<TagDef, CmdError> {
    s.tag_defs().into_iter().find(|d| d.key == key).ok_or_else(|| CmdError::Params(format!("unknown tag `{key}`")))
}

/// What a tag command acts on: the paragraphs the selection touches (or those of selected
/// containers) and the selected ink strokes.
struct Targets {
    paras: Vec<(Block, usize)>,
    ink: Vec<Id>,
}

/// The selection's targets (materialising a pending container when nothing is selected).
fn targets(s: &mut Session) -> Result<Targets, CmdError> {
    let ink = selected_ink(s);
    if ink.is_empty() && ops::touched(s).is_empty() && s.pending_outline.is_some() {
        ops::ensure_caret(s)?;
    }
    let paras: Vec<(Block, usize)> =
        ops::touched(s).into_iter().filter(|(b, _)| *b != Block::Title).flat_map(|(b, r)| r.map(move |i| (b, i))).collect();
    if paras.is_empty() && ink.is_empty() {
        return Err(CmdError::Disabled("click in a note first (the page title can't be tagged)".into()));
    }
    Ok(Targets { paras, ink })
}

/// Whether a selected stroke carries tag `key`, and if so whether it is checked.
fn ink_tag(page: &Page, ink: &[Id], key: &str) -> Option<bool> {
    ink.iter().filter_map(|id| page.ink(*id)).find_map(|k| k.tags.iter().find(|t| t.key == key).map(|t| t.checked))
}

/// The stroke a new tag goes on: the selection's leftmost (then topmost) stroke, so the icon
/// sits at the drawing's left edge (decision D5).
fn anchor(page: &Page, ink: &[Id]) -> Option<Id> {
    ink.iter()
        .filter_map(|id| page.ink(*id))
        .min_by(|a, b| {
            let (a, b) = (a.bounds().min, b.bounds().min);
            a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y))
        })
        .map(|k| k.id)
}

/// Put `def` on the selection's anchor stroke, unless one of its strokes already has it.
fn tag_ink(page: &mut Page, ink: &[Id], def: &TagDef) {
    if ink_tag(page, ink, &def.key).is_some() {
        return;
    }
    if let Some(k) = anchor(page, ink).and_then(|a| page.ink_mut(a))
        && k.tags.len() < MAX_OBJECT_TAGS
    {
        k.tags.push(TagUse::of(def));
    }
}

/// Apply `f` to the tags of every selected stroke.
fn each_ink(page: &mut Page, ink: &[Id], mut f: impl FnMut(&mut Vec<TagUse>)) {
    for id in ink {
        if let Some(k) = page.ink_mut(*id) {
            f(&mut k.tags);
        }
    }
}

fn toggle(s: &mut Session, v: &Value) -> CmdResult {
    let key = p::req_str(v, "key")?;
    let def = def_for(s, key)?;
    let t = targets(s)?;
    let page = s.page().ok_or_else(no_page)?;
    let all_have = t
        .paras
        .iter()
        .all(|(b, i)| Session::block_paras(page, *b).and_then(|ps| ps.get(*i)).is_some_and(|p| p.tags.iter().any(|t| t.key == def.key)))
        && (t.ink.is_empty() || ink_tag(page, &t.ink, &def.key).is_some());
    let page = s.page_mut().ok_or_else(no_page)?;
    for (b, i) in t.paras {
        if let Some(p) = Session::para_mut(page, b, i) {
            if all_have {
                p.tags.retain(|t| t.key != def.key);
            } else if !p.tags.iter().any(|t| t.key == def.key) {
                p.tags.push(TagUse::of(&def));
            }
        }
    }
    if all_have {
        each_ink(page, &t.ink, |tags| tags.retain(|t| t.key != def.key));
    } else {
        tag_ink(page, &t.ink, &def);
    }
    Ok(Value::Null)
}

/// The Ctrl+1…9 behaviour: to-do tags cycle unchecked → checked → removed; other tags toggle.
fn shortcut(s: &mut Session, key: &str) -> CmdResult {
    let def = def_for(s, key)?;
    if !def.is_todo() {
        return toggle(s, &json!({"key": key}));
    }
    let t = targets(s)?;
    let page = s.page().ok_or_else(no_page)?;
    let state = |b: Block, i: usize| {
        Session::block_paras(page, b).and_then(|ps| ps.get(i)).and_then(|p| p.tags.iter().find(|t| t.key == def.key).map(|t| t.checked))
    };
    // Decide by the first paragraph (or, with ink alone, by the ink), apply to all.
    let first = match t.paras.first() {
        Some((b, i)) => state(*b, *i),
        None => ink_tag(page, &t.ink, &def.key),
    };
    let page = s.page_mut().ok_or_else(no_page)?;
    match first {
        None => tag_ink(page, &t.ink, &def),
        Some(false) => each_ink(page, &t.ink, |tags| tags.iter_mut().filter(|t| t.key == def.key).for_each(|t| t.checked = true)),
        Some(true) => each_ink(page, &t.ink, |tags| tags.retain(|t| t.key != def.key)),
    }
    for (b, i) in t.paras {
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

/// Apply `f` to the tags of every target: each paragraph's and each selected stroke's.
fn each_target(s: &mut Session, mut f: impl FnMut(&mut Vec<TagUse>)) -> CmdResult {
    let t = targets(s)?;
    let page = s.page_mut().ok_or_else(no_page)?;
    for (b, i) in t.paras {
        if let Some(p) = Session::para_mut(page, b, i) {
            f(&mut p.tags);
        }
    }
    each_ink(page, &t.ink, f);
    Ok(Value::Null)
}

fn remove_all(s: &mut Session, _: &Value) -> CmdResult {
    each_target(s, Vec::clear)
}

fn remove_one(s: &mut Session, v: &Value) -> CmdResult {
    // CM.Tag.TagsRemove: the tag the menu was opened on when no key is given.
    let key = match p::str(v, "key") {
        Some(k) => k.to_string(),
        None => s.view.context_tag.clone().ok_or_else(|| CmdError::Params("`key` (the tag to remove) is required".into()))?,
    };
    each_target(s, |tags| tags.retain(|t| t.key != key))
}

fn set_checked(s: &mut Session, v: &Value, on: bool) -> CmdResult {
    // CM.Tag.NoteTagCheck / Uncheck: the tag the menu was opened on when no key is given.
    let key = p::str(v, "key").map(str::to_string).or_else(|| s.view.context_tag.clone());
    check_keyed(s, key, on)
}

fn check_keyed(s: &mut Session, key: Option<String>, on: bool) -> CmdResult {
    each_target(s, |tags| {
        for t in tags.iter_mut().filter(|t| t.symbol.is_checkbox() && key.as_ref().is_none_or(|k| *k == t.key)) {
            t.checked = on;
        }
    })
}

fn check_all(s: &mut Session, on: bool) -> CmdResult {
    check_keyed(s, None, on)
}

fn click(s: &mut Session, v: &Value) -> CmdResult {
    let Some(id) = v.get("block").and_then(Value::as_str).and_then(Id::parse) else {
        return Err(CmdError::Params("`block` (outline or ink id) is required".into()));
    };
    let which = p::usize(v, "tag").unwrap_or(0);
    let is_ink = s.page().ok_or_else(no_page)?.ink(id).is_some();
    let page = s.page_mut().ok_or_else(no_page)?;
    let tags = if is_ink {
        page.ink_mut(id).map(|k| &mut k.tags).ok_or_else(|| CmdError::Params("no such stroke".into()))?
    } else {
        let para = p::req_usize(v, "para")?;
        Session::para_mut(page, Block::Outline(id), para).map(|p| &mut p.tags).ok_or_else(|| CmdError::Params("no such paragraph".into()))?
    };
    let t = tags.iter_mut().filter(|t| t.symbol.is_checkbox()).nth(which).ok_or_else(|| CmdError::Params("that note has no checkbox".into()))?;
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
    /// Whether the tag is on a stroke (`block` is then the stroke's id).
    ink: bool,
}

/// What Find Tags and summary pages show for a tagged stroke (unverified wording).
const INK_TEXT: &str = "Ink drawing";

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
                if let Some(k) = o.as_ink() {
                    for t in k.tags.iter().filter(|t| !(unchecked_only && t.checked)) {
                        out.push(Found {
                            tag: t.clone(),
                            text: INK_TEXT.into(),
                            page: page.id,
                            page_title: page.display_title(),
                            section: section.clone(),
                            date: page.modified,
                            block: k.id,
                            para: 0,
                            ink: true,
                        });
                    }
                    continue;
                }
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
                            ink: false,
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
    let kind = crate::search_cmds::parse_scope(v)?.unwrap_or(ScopeKind::AllNotebooks);
    let group = p::str(v, "group").unwrap_or("tag");
    if !matches!(group, "tag" | "section" | "title" | "date" | "text") {
        return Err(CmdError::Params(format!("unknown grouping `{group}`")));
    }
    let found = collect(s, kind, p::bool(v, "unchecked").unwrap_or(false));
    let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
    for f in &found {
        let k = group_key(f, group, s.clock.utc_offset_min);
        let item = json!({"tag": f.tag.name, "key": f.tag.key, "checked": f.tag.checked, "text": f.text, "page": f.page.to_string(), "pageTitle": f.page_title, "section": f.section, "block": f.block.to_string(), "para": f.para, "ink": f.ink});
        match groups.iter_mut().find(|(g, _)| *g == k) {
            Some((_, items)) => items.push(item),
            None => groups.push((k, vec![item])),
        }
    }
    s.view.tags_pane = true;
    Ok(json!({"count": found.len(), "groups": groups.into_iter().map(|(g, items)| json!({"group": g, "items": items})).collect::<Vec<_>>()}))
}

fn summary_page(s: &mut Session, v: &Value) -> CmdResult {
    let kind = crate::search_cmds::parse_scope(v)?.unwrap_or(ScopeKind::AllNotebooks);
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

/// Keep `list` as the tag list (Customize Tags), and the custom ones also as `custom_tags`.
fn store_tags(s: &mut Session, list: Vec<TagDef>) -> CmdResult {
    s.prefs.custom_tags = list.iter().filter(|d| d.key.starts_with("custom-")).cloned().collect();
    let keys: Vec<String> = list.iter().map(|d| d.key.clone()).collect();
    s.prefs.tag_list = Some(list);
    Ok(json!({"tags": keys}))
}

fn customize(s: &mut Session, v: &Value) -> CmdResult {
    let action = p::req_str(v, "action")?;
    // A colour param: absent = keep, null = none, "#rrggbb" = that colour.
    let color = |k: &str| -> Result<Option<Option<notecraft_doc::color::Rgba>>, CmdError> {
        match v.get(k) {
            None => Ok(None),
            Some(Value::Null) => Ok(Some(None)),
            Some(Value::String(c)) => notecraft_doc::color::Rgba::from_hex(c)
                .map(|c| Some(Some(c)))
                .ok_or_else(|| CmdError::Params(format!("`{k}` must be #rrggbb or null"))),
            Some(_) => Err(CmdError::Params(format!("`{k}` must be #rrggbb or null"))),
        }
    };
    let symbol = || -> Result<Option<TagSymbol>, CmdError> {
        match p::str(v, "symbol") {
            Some(n) => parse_symbol(n).map(Some).ok_or_else(|| CmdError::Params(format!("unknown symbol `{n}`"))),
            None => Ok(None),
        }
    };
    let mut list = s.tag_defs();
    let find = |list: &[TagDef]| -> Result<usize, CmdError> {
        let key = p::req_str(v, "key")?;
        list.iter().position(|d| d.key == key).ok_or_else(|| CmdError::Params(format!("no tag `{key}`")))
    };
    match action {
        "add" => {
            if list.iter().filter(|d| d.key.starts_with("custom-")).count() >= notecraft_doc::tags::MAX_CUSTOM_TAGS {
                return Err(CmdError::Failed("too many custom tags".into()));
            }
            let name = notecraft_doc::clean_name(p::req_str(v, "name")?);
            if name.is_empty() {
                return Err(CmdError::Params("`name` can't be empty".into()));
            }
            let key = format!("custom-{}", s.new_id());
            let def = TagDef {
                key: key.clone(),
                name,
                symbol: symbol()?.unwrap_or(TagSymbol::Star),
                font_color: color("fontColor")?.flatten(),
                highlight: color("highlight")?.flatten(),
            };
            // New tags go first in the list, as the reference puts them (unverified).
            list.insert(0, def);
            let mut r = store_tags(s, list)?;
            r["key"] = json!(key);
            Ok(r)
        }
        "modify" => {
            let i = find(&list)?;
            let (sym, fc, hl) = (symbol()?, color("fontColor")?, color("highlight")?);
            let def = list.get_mut(i).ok_or_else(|| CmdError::Params("no such tag".into()))?;
            if let Some(n) = p::str(v, "name") {
                let n = notecraft_doc::clean_name(n);
                if n.is_empty() {
                    return Err(CmdError::Params("`name` can't be empty".into()));
                }
                def.name = n;
            }
            if let Some(sym) = sym {
                def.symbol = sym;
            }
            if let Some(c) = fc {
                def.font_color = c;
            }
            if let Some(c) = hl {
                def.highlight = c;
            }
            store_tags(s, list)
        }
        "remove" => {
            // Notes keep the tags they carry (a tag on a paragraph has its own name and symbol).
            let i = find(&list)?;
            list.remove(i);
            store_tags(s, list)
        }
        "moveUp" | "moveDown" => {
            let i = find(&list)?;
            let j = if action == "moveUp" { i.checked_sub(1) } else { Some(i + 1).filter(|j| *j < list.len()) };
            if let Some(j) = j {
                list.swap(i, j);
            }
            store_tags(s, list)
        }
        "reset" => {
            s.prefs.tag_list = None;
            Ok(json!({"tags": s.tag_defs().iter().map(|d| d.key.clone()).collect::<Vec<_>>()}))
        }
        o => Err(CmdError::Params(format!("unknown action `{o}`"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{run, sample};

    fn keys(s: &Session) -> Vec<String> {
        s.tag_defs().into_iter().map(|d| d.key).collect()
    }

    #[test]
    fn customize_tags_modifies_reorders_and_removes_any_tag() {
        // parity: UI.Panes.CustomizeTags
        let mut s = sample();
        let builtins = keys(&s);
        // A built-in renamed, recoloured and given another symbol.
        run(
            &mut s,
            "tag.customize",
            json!({"action": "modify", "key": "important", "name": "Urgent", "symbol": "exclamation", "highlight": "#ffcccc"}),
        );
        let d = s.tag_defs().into_iter().find(|d| d.key == "important").unwrap();
        assert_eq!((d.name.as_str(), d.symbol), ("Urgent", TagSymbol::Exclamation));
        assert!(d.highlight.is_some());
        run(&mut s, "tag.customize", json!({"action": "modify", "key": "important", "highlight": null}));
        assert!(s.tag_defs().into_iter().find(|d| d.key == "important").unwrap().highlight.is_none(), "null clears a colour");
        // Reorder: Question up above Important.
        run(&mut s, "tag.customize", json!({"action": "moveUp", "key": "question"}));
        assert_eq!(&keys(&s)[..3], &["todo", "question", "important"]);
        run(&mut s, "tag.customize", json!({"action": "moveUp", "key": "todo"}));
        assert_eq!(keys(&s)[0], "todo", "the first stays first");
        // A new tag goes first; removing a built-in leaves the notes that carry it alone.
        let k = run(&mut s, "tag.customize", json!({"action": "add", "name": "Recipe"}))["key"].as_str().unwrap().to_string();
        assert_eq!(keys(&s)[0], k);
        assert_eq!(s.prefs.custom_tags.len(), 1, "custom tags stay listed on their own too");
        run(&mut s, "text.insert", json!({"text": "x"}));
        run(&mut s, "tag.toggle", json!({"key": "idea"}));
        run(&mut s, "tag.customize", json!({"action": "remove", "key": "idea"}));
        assert!(!keys(&s).contains(&"idea".to_string()));
        assert!(
            s.page()
                .unwrap()
                .objects
                .iter()
                .filter_map(PageObject::as_outline)
                .flat_map(|o| o.paragraphs.iter())
                .any(|p| p.tags.iter().any(|t| t.key == "idea"))
        );
        // Reset brings the built-ins back in order (custom tags after them).
        run(&mut s, "tag.customize", json!({"action": "reset"}));
        assert_eq!(&keys(&s)[..builtins.len()], &builtins[..]);
        for bad in [
            json!({"action": "moveUp", "key": "nope"}),
            json!({"action": "modify", "key": "todo", "name": "  "}),
            json!({"action": "modify", "key": "todo", "fontColor": 3}),
            json!({"action": "spin"}),
        ] {
            assert!(matches!(s.run("tag.customize", bad.clone()), Err(CmdError::Params(_))), "{bad}");
        }
    }

    #[test]
    fn every_tag_of_the_gallery_applies() {
        // The Tags gallery's items, in the ribbon's order, and the tags they apply.
        let rows = [
            ("R.Home.Tags.ToDo", "todo"),
            ("R.Home.Tags.Important", "important"),
            ("R.Home.Tags.Question", "question"),
            ("R.Home.Tags.RememberForLater", "remember-for-later"),
            ("R.Home.Tags.Definition", "definition"),
            ("R.Home.Tags.Highlight", "highlight"),
            ("R.Home.Tags.Contact", "contact"),
            ("R.Home.Tags.Address", "address"),
            ("R.Home.Tags.PhoneNumber", "phone-number"),
            ("R.Home.Tags.WebSiteToVisit", "web-site-to-visit"),
            ("R.Home.Tags.Idea", "idea"),
            ("R.Home.Tags.Password", "password"),
            ("R.Home.Tags.Critical", "critical"),
            ("R.Home.Tags.ProjectA", "project-a"),
            ("R.Home.Tags.ProjectB", "project-b"),
            ("R.Home.Tags.MovieToSee", "movie-to-see"),
            ("R.Home.Tags.BookToRead", "book-to-read"),
            ("R.Home.Tags.MusicToListenTo", "music-to-listen-to"),
            ("R.Home.Tags.SourceForArticle", "source-for-article"),
            ("R.Home.Tags.RememberForBlog", "remember-for-blog"),
            ("R.Home.Tags.DiscussWithPersonA", "discuss-with-person-a"),
            ("R.Home.Tags.DiscussWithPersonB", "discuss-with-person-b"),
            ("R.Home.Tags.DiscussWithManager", "discuss-with-manager"),
            ("R.Home.Tags.SendInEmail", "send-in-email"),
            ("R.Home.Tags.ScheduleMeeting", "schedule-meeting"),
            ("R.Home.Tags.CallBack", "call-back"),
            ("R.Home.Tags.ToDoPriority1", "todo-priority-1"),
            ("R.Home.Tags.ToDoPriority2", "todo-priority-2"),
            ("R.Home.Tags.ClientRequest", "client-request"),
        ];
        let builtins = notecraft_doc::builtin_tags();
        assert_eq!(rows.len(), builtins.len());
        for ((row, key), def) in rows.iter().zip(&builtins) {
            assert_eq!(def.key, *key, "{row} is the built-in in the same place");
            let mut s = sample();
            run(&mut s, "text.insert", json!({"text": "x"}));
            run(&mut s, "tag.toggle", json!({"key": key}));
            let tagged = s
                .page()
                .unwrap()
                .objects
                .iter()
                .filter_map(PageObject::as_outline)
                .flat_map(|o| o.paragraphs.iter())
                .any(|p| p.tags.iter().any(|t| t.key == *key && t.name == def.name));
            assert!(tagged, "{row}");
        }
    }
}
