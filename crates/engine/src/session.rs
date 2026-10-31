//! The editing session: open notebooks, selection, undo, clipboard, search index, view state,
//! preferences and autosave.
//!
//! [`Session::run`] is the single entry point for every command. It:
//!
//! 1. looks the command up and checks its `enabled` predicate;
//! 2. snapshots the workspace and selection (cheap: structural sharing, see
//!    `notecraft_doc::notebook`);
//! 3. runs the command inside `catch_unwind` — the last-resort guard of the never-crash rule: an
//!    escaped panic restores the snapshot and becomes an error;
//! 4. on success, if the workspace changed: records **one** undo entry (consecutive typing in
//!    one paragraph coalesces into word-sized steps), marks the changed sections dirty for
//!    autosave, and bumps the revision the UI caches layout by; on failure, restores the
//!    snapshot so a half-done command never leaves a trace.

use std::collections::{HashMap, HashSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use notecraft_doc::{CharFormat, Id, IdGen, NavNode, Notebook, Page, Paragraph, RuleLines, TagDef, Timestamp, Workspace, builtin_tags};
use notecraft_search::{Index, PageMeta, Scope};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::selection::{Block, PageSel, Pos, Selection, TextRange};
use crate::{CmdError, CmdResult, commands};

/// Most undo steps kept.
pub const UNDO_MAX: usize = 300;

/// The host's clock. The engine never reads the system clock (wasm traps on it; tests must be
/// deterministic): the host sets it before running commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Clock {
    /// Milliseconds since the Unix epoch (UTC).
    pub now_ms: i64,
    /// Local offset east of UTC, in minutes.
    pub utc_offset_min: i32,
}

/// User preferences the engine needs (the UI persists them with craft-settings).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// Author name stamped on edits (Options › General › User name).
    pub author: String,
    /// The author's initials (`OPT.General.Initials`).
    pub initials: String,
    /// Show the Mini Toolbar on a selection (`OPT.General.ShowMiniToolbar`; the UI reads it).
    pub show_mini_toolbar: bool,
    /// Show note container frames on pages (`OPT.Display.ShowNoteContainers`; the UI reads it).
    pub show_note_containers: bool,
    /// Default font of new text (`OPT.Advanced.DefaultFont`).
    pub default_font: String,
    /// Default size in points.
    pub default_size: f32,
    /// Default colour of new text (`None`: automatic).
    pub default_color: Option<notecraft_doc::color::Rgba>,
    /// Show the Paste Options button after a paste (`OPT.Advanced.ShowPasteOptions`; the UI
    /// reads it).
    pub show_paste_options: bool,
    /// Tab after text makes a table (`OPT.Advanced.CreateTableOnTab`, decision D6).
    pub create_table_on_tab: bool,
    /// Auto-bulleting (catalog `OPT.Advanced.AutoBulleting`).
    pub auto_bullets: bool,
    /// Auto-numbering (`OPT.Advanced.AutoNumbering`).
    pub auto_numbering: bool,
    /// Wiki links `[[Page]]` (`OPT.Advanced.WikiLinks`).
    pub wiki_links: bool,
    /// Work out `2+3=` when Space or Enter follows (`OPT.Advanced.CalculateMath`).
    pub calculate_math: bool,
    /// Create new pages with rule lines (View › Rule Lines › Always Create Pages with Rule Lines).
    pub new_pages_rule_lines: Option<RuleLines>,
    /// Custom tags (Customize Tags).
    pub custom_tags: Vec<TagDef>,
    /// Default search scope (Set This Scope as Default).
    pub default_scope: ScopeKind,
    /// Backups to keep per notebook.
    pub backup_copies: usize,
    /// Where backups go (`OPT.SaveBackup.BackupFolder`; empty: the app's own folder).
    pub backup_folder: String,
    /// Back up automatically this often, in minutes (`OPT.SaveBackup.AutoBackup`; 0: never).
    pub auto_backup_minutes: u32,
    /// When the last backup ran (Unix ms; 0: never).
    pub last_backup_ms: i64,
    /// Lock protected sections after this many idle minutes (`OPT.Advanced.LockAfterIdle`;
    /// 0: never).
    pub lock_after_minutes: u32,
    /// Lock a protected section as soon as another section is opened
    /// (`OPT.Advanced.LockOnLeave`).
    pub lock_on_leave: bool,
    /// Where new notebooks go (`OPT.SaveBackup.DefaultNotebookLocation`; empty: ask).
    pub default_notebook_location: String,
    /// The notebook whose Quick Notes section takes quick notes
    /// (`OPT.SaveBackup.QuickNotesSection`; empty: the first open notebook).
    pub quick_notes_notebook: String,
    /// Page templates of the user's own (decision D10).
    pub templates: Vec<crate::template_cmds::SavedTemplate>,
    /// Recently used templates, latest first.
    pub recent_templates: Vec<String>,
}

impl Default for Prefs {
    fn default() -> Prefs {
        Prefs {
            author: String::new(),
            initials: String::new(),
            show_mini_toolbar: true,
            show_note_containers: true,
            default_font: notecraft_doc::text::DEFAULT_FONT.to_string(),
            default_size: notecraft_doc::text::DEFAULT_SIZE,
            default_color: None,
            show_paste_options: true,
            create_table_on_tab: true,
            auto_bullets: true,
            auto_numbering: true,
            wiki_links: true,
            calculate_math: true,
            new_pages_rule_lines: None,
            custom_tags: Vec::new(),
            default_scope: ScopeKind::AllNotebooks,
            backup_copies: 2,
            backup_folder: String::new(),
            // The reference backs up once a day by default (unverified).
            auto_backup_minutes: 24 * 60,
            last_backup_ms: 0,
            // The reference locks after 10 idle minutes by default (unverified).
            lock_after_minutes: 10,
            lock_on_leave: false,
            default_notebook_location: String::new(),
            quick_notes_notebook: String::new(),
            templates: Vec::new(),
            recent_templates: Vec::new(),
        }
    }
}

/// The search scope choices (the search box drop-down).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ScopeKind {
    /// This page.
    Page,
    /// This section.
    Section,
    /// This section group.
    SectionGroup,
    /// This notebook.
    Notebook,
    /// All notebooks.
    #[default]
    AllNotebooks,
}

/// The ribbon layout (catalog `UI.Ribbon.ModeSwitch`): the one-row simplified ribbon of the 2024
/// UI (the default) or the classic ribbon with labelled groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RibbonMode {
    /// One row of controls; what doesn't fit goes into the … overflow menu.
    #[default]
    Simplified,
    /// Groups with labels, large and small buttons.
    Classic,
}

/// Find on page (Ctrl+F) state.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FindState {
    /// Whether the find bar is open.
    pub active: bool,
    /// The query.
    pub query: String,
    /// Matches on the current page, in reading order.
    pub matches: Vec<(Block, usize, usize, usize)>,
    /// Index of the current match.
    pub current: usize,
}

/// View state (not part of the document, not undoable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewState {
    /// Zoom factor (1.0 = 100 %).
    pub zoom: f32,
    /// Full Page View.
    pub full_page: bool,
    /// Docked to the desktop.
    pub docked: bool,
    /// Ribbon collapsed.
    pub ribbon_collapsed: bool,
    /// Simplified or classic ribbon.
    pub ribbon_mode: RibbonMode,
    /// Notebook pane collapsed.
    pub notebook_pane_collapsed: bool,
    /// Scope of the search box.
    pub scope: ScopeKind,
    /// Find on page.
    pub find: FindState,
    /// Page list width in points (Ctrl+Shift+[ / ]).
    pub page_list_width: f32,
    /// Find Tags pane open.
    pub tags_pane: bool,
    /// Templates pane open (Insert › Page Templates).
    pub templates_pane: bool,
    /// Paper Size pane open (View › Paper Size).
    pub paper_pane: bool,
    /// Password Protection pane open (Review › Password).
    #[serde(default)]
    pub password_pane: bool,
    /// The recycle bin of this notebook is being viewed.
    pub recycle_view: Option<Id>,
}

impl Default for ViewState {
    fn default() -> ViewState {
        ViewState {
            zoom: 1.0,
            full_page: false,
            docked: false,
            ribbon_collapsed: false,
            ribbon_mode: RibbonMode::Simplified,
            notebook_pane_collapsed: false,
            scope: ScopeKind::AllNotebooks,
            find: FindState::default(),
            page_list_width: 200.0,
            tags_pane: false,
            templates_pane: false,
            paper_pane: false,
            password_pane: false,
            recycle_view: None,
        }
    }
}

/// A scroll the UI should perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollRequest {
    /// One screen up.
    PageUp,
    /// One screen down.
    PageDown,
    /// To the top of the page.
    Top,
    /// To the bottom of the page.
    Bottom,
    /// Bring the caret into view.
    Caret,
}

/// The engine's clipboard: rich paragraphs plus plain text (the UI mirrors the text to the
/// system clipboard; when the system clipboard still holds the same text on paste, the rich
/// version is used, so formatting survives copy and paste inside NoteCraft).
#[derive(Debug, Clone, Default)]
pub struct Clipboard {
    /// Copied paragraphs (one partial paragraph for a selection inside a paragraph).
    pub paragraphs: Vec<Paragraph>,
    /// Plain text of the same.
    pub text: String,
    /// Copied or cut pages (page list Cut/Copy).
    pub pages: Vec<Arc<Page>>,
    /// Copied or cut pictures, attachments and whole tables (bytes shared, not copied).
    pub objects: Vec<notecraft_doc::PageObject>,
    /// The copied notes as HTML, for other applications (empty when there is none).
    pub html: String,
}

/// What changed in one notebook since its last save.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NbDirty {
    /// Sections with changed content.
    pub sections: HashSet<Id>,
    /// The tree changed.
    pub structure: bool,
    /// The recycle bin changed.
    pub recycle: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct HistoryEntry {
    label: String,
    ws: Workspace,
    sel: Selection,
}

/// The session.
pub struct Session {
    /// Open notebooks.
    pub ws: Workspace,
    /// Selection and focus.
    pub sel: Selection,
    /// Id generator for this replica.
    pub ids: IdGen,
    /// Clipboard.
    pub clipboard: Clipboard,
    /// View state.
    pub view: ViewState,
    /// Preferences.
    pub prefs: Prefs,
    /// The Draw tab's tool and pens (decision D4; not notebook data, never an undo step).
    pub draw: crate::DrawState,
    /// The host's clock.
    pub clock: Clock,
    /// Formatting to apply to the next typed text at the caret (set by toggling Bold with no
    /// selection); cleared when the caret moves.
    pub pending_fmt: Option<CharFormat>,
    /// Format Painter: the copied format waiting to be applied.
    pub format_painter: Option<CharFormat>,
    /// Notices for the user (recovered files, failed saves).
    pub messages: Vec<String>,
    /// A click on empty canvas: where a note container will be created when the user starts
    /// typing (catalog `BEH.Canvas.ClickToType`). Nothing is created until then, so a stray
    /// click leaves no empty container and no undo step.
    pub pending_outline: Option<notecraft_doc::geom::Point>,
    /// A scroll request for the UI (Page Up/Down, Ctrl+Home/End), consumed by the canvas.
    pub scroll_request: Option<ScrollRequest>,
    /// File › Exit ran and everything saved: the desktop shell closes the window (headless
    /// hosts ignore it).
    pub quit_request: bool,
    /// What needs saving, per notebook.
    pub dirty: HashMap<Id, NbDirty>,
    /// Increments whenever the workspace changes (UI caches layout by it).
    pub revision: u64,
    /// When the workspace last changed (host clock), for autosave debouncing.
    pub last_change_ms: i64,
    pub(crate) undo: Vec<HistoryEntry>,
    pub(crate) redo: Vec<HistoryEntry>,
    pub(crate) nav_back: Vec<Id>,
    pub(crate) nav_forward: Vec<Id>,
    pub(crate) coalesce: Option<String>,
    last_coalesce: Option<String>,
    index: Index,
    indexed: HashMap<Id, Arc<Page>>,
    pub(crate) last_sel: HashMap<Id, PageSel>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) stores: HashMap<Id, notecraft_format::store::NotebookStore>,
    /// The keys of unlocked password-protected sections (decision D15), only in memory and
    /// zeroed when dropped.
    pub(crate) keys: HashMap<Id, notecraft_format::protect::Key>,
    /// Set by a command whose undo history must not survive it (locking a section: earlier
    /// steps hold its notes in the clear).
    pub(crate) forget_history: bool,
    /// Key derivation costs for new passwords (tests use cheap ones).
    pub(crate) kdf: notecraft_format::protect::KdfParams,
}

impl Default for Session {
    fn default() -> Session {
        Session::new(Clock::default())
    }
}

impl Session {
    /// An empty session (no notebooks).
    pub fn new(clock: Clock) -> Session {
        Session {
            ws: Workspace::default(),
            sel: Selection::default(),
            ids: IdGen::from_entropy(),
            clipboard: Clipboard::default(),
            view: ViewState::default(),
            draw: crate::DrawState::default(),
            prefs: Prefs::default(),
            clock,
            pending_fmt: None,
            format_painter: None,
            messages: Vec::new(),
            pending_outline: None,
            scroll_request: None,
            quit_request: false,
            dirty: HashMap::new(),
            revision: 0,
            last_change_ms: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            nav_back: Vec::new(),
            nav_forward: Vec::new(),
            coalesce: None,
            last_coalesce: None,
            index: Index::new(),
            indexed: HashMap::new(),
            last_sel: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            stores: HashMap::new(),
            keys: HashMap::new(),
            forget_history: false,
            kdf: notecraft_format::protect::KdfParams::DEFAULT,
        }
    }

    /// A session with the built-in sample notebook open (in memory).
    pub fn with_sample(clock: Clock) -> Session {
        let mut s = Session::new(clock);
        let nb = crate::sample::sample_notebook(&mut s.ids, Timestamp(clock.now_ms));
        s.add_notebook(nb);
        s
    }

    /// Run a command by id. See the module docs for the guarantees.
    pub fn run(&mut self, id: &str, params: Value) -> CmdResult {
        let spec = commands::find(id).ok_or_else(|| CmdError::Unknown(id.to_string()))?;
        if !(params.is_object() || params.is_null()) {
            return Err(CmdError::Params("params must be a JSON object".into()));
        }
        if let Some(why) = (spec.enabled)(self) {
            return Err(CmdError::Disabled(why.to_string()));
        }
        let before_ws = self.ws.clone();
        let before_sel = self.sel.clone();
        self.coalesce = None;
        let run = spec.run;
        let result = catch_unwind(AssertUnwindSafe(|| run(self, &params)));
        let result = match result {
            Ok(r) => r,
            Err(_) => {
                self.ws = before_ws;
                self.sel = before_sel;
                return Err(CmdError::Failed(format!("internal error in `{id}`; the notebook was kept as it was")));
            }
        };
        if result.is_err() {
            self.ws = before_ws;
            self.sel = before_sel;
            self.last_coalesce = None;
            return result;
        }
        let history_cmd = matches!(id, "edit.undo" | "edit.redo" | "edit.undoTo" | "edit.redoTo");
        let changed_by_cmd = workspace_changed(&before_ws, &self.ws);
        // Tidy up containers emptied and then left behind; when the command itself changed
        // nothing (a caret move), the removal joins the previous undo step instead of adding one.
        let pruned = !history_cmd && crate::ops::prune_empty_outlines(self);
        if changed_by_cmd || pruned {
            self.mark_dirty(&before_ws);
            self.revision = self.revision.wrapping_add(1);
            self.last_change_ms = self.clock.now_ms;
        }
        if changed_by_cmd && !history_cmd {
            // A coalesce key ending in "|end" (typing a space) joins the current step but closes
            // it, so a word and its trailing space undo together.
            let (key, closes) = match self.coalesce.take() {
                Some(c) => match c.strip_suffix("|end") {
                    Some(k) => (Some(k.to_string()), true),
                    None => (Some(c), false),
                },
                None => (None, false),
            };
            let merge = key.is_some() && key == self.last_coalesce && !self.undo.is_empty();
            if !merge {
                self.undo.push(HistoryEntry { label: spec.label.to_string(), ws: before_ws, sel: before_sel });
                if self.undo.len() > UNDO_MAX {
                    self.undo.remove(0);
                }
            }
            self.last_coalesce = if closes { None } else { key };
            self.redo.clear();
        } else if !history_cmd && self.coalesce.is_none() {
            // A caret move or other non-editing command ends a typing burst.
            self.last_coalesce = None;
        }
        // Locking a section: no undo or redo step may bring its notes back.
        if std::mem::take(&mut self.forget_history) {
            self.undo.clear();
            self.redo.clear();
            self.last_coalesce = None;
        }
        result
    }

    /// Labels of the undo stack, newest first (the Undo drop-down).
    pub fn undo_labels(&self) -> Vec<String> {
        self.undo.iter().rev().map(|e| e.label.clone()).collect()
    }

    pub(crate) fn undo_step(&mut self) -> bool {
        let Some(e) = self.undo.pop() else { return false };
        let cur = HistoryEntry { label: e.label.clone(), ws: std::mem::replace(&mut self.ws, e.ws), sel: std::mem::replace(&mut self.sel, e.sel) };
        self.redo.push(cur);
        self.last_coalesce = None;
        true
    }

    pub(crate) fn redo_step(&mut self) -> bool {
        let Some(e) = self.redo.pop() else { return false };
        let cur = HistoryEntry { label: e.label.clone(), ws: std::mem::replace(&mut self.ws, e.ws), sel: std::mem::replace(&mut self.sel, e.sel) };
        self.undo.push(cur);
        self.last_coalesce = None;
        true
    }

    /// Whether there is anything to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is anything to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Forget the undo and redo history (a host's start-up work, like creating the first
    /// notebook, is not an edit to undo).
    pub fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Add a notebook to the workspace (in memory) and select its first page.
    pub fn add_notebook(&mut self, nb: Notebook) {
        let first_section = nb.sections().first().map(|s| (s.id, s.pages.first().map(|p| p.id)));
        let nb_id = nb.id;
        self.ws.notebooks.push(Arc::new(nb));
        self.sel.notebook = Some(nb_id);
        if let Some((sid, page)) = first_section {
            self.sel.section = Some(sid);
            self.sel.page = page;
        }
        self.sel.on_page = PageSel::None;
        self.revision = self.revision.wrapping_add(1);
    }

    /// The host's "now".
    pub fn now(&self) -> Timestamp {
        Timestamp(self.clock.now_ms)
    }

    /// The author name for new edits.
    pub fn author(&self) -> String {
        self.prefs.author.clone()
    }

    /// All tag definitions: built-ins, then custom tags.
    pub fn tag_defs(&self) -> Vec<TagDef> {
        let mut v = builtin_tags();
        v.extend(self.prefs.custom_tags.iter().cloned());
        v
    }

    /// The current page.
    pub fn page(&self) -> Option<&Arc<Page>> {
        self.ws.page(self.sel.page?)
    }

    /// The current page, mutably (copy-on-write).
    pub fn page_mut(&mut self) -> Option<&mut Page> {
        let id = self.sel.page?;
        self.ws.page_mut(id)
    }

    /// The current notebook.
    pub fn notebook(&self) -> Option<&Arc<Notebook>> {
        self.ws.notebook(self.sel.notebook?)
    }

    /// The current section.
    pub fn section(&self) -> Option<&Arc<notecraft_doc::Section>> {
        self.ws.section(self.sel.section?)
    }

    /// The paragraphs of `block` on `page`.
    pub fn block_paras(page: &Page, block: Block) -> Option<&[Paragraph]> {
        match block {
            Block::Title => Some(std::slice::from_ref(&page.title)),
            Block::Outline(id) => page.outline(id).map(|o| o.paragraphs.as_slice()),
            Block::Cell(id) => page.cell(id).map(|c| c.paragraphs.as_slice()),
        }
    }

    /// The paragraphs of `block` on `page`, mutably.
    pub fn block_paras_mut(page: &mut Page, block: Block) -> Option<&mut Vec<Paragraph>> {
        match block {
            Block::Title => None,
            Block::Outline(id) => page.outline_mut(id).map(|o| &mut o.paragraphs),
            Block::Cell(id) => page.cell_mut(id).map(|c| &mut c.paragraphs),
        }
    }

    /// A paragraph of `block`, mutably (works for the title too).
    pub fn para_mut(page: &mut Page, block: Block, para: usize) -> Option<&mut Paragraph> {
        match block {
            Block::Title => (para == 0).then_some(&mut page.title),
            Block::Outline(id) => page.outline_mut(id)?.paragraphs.get_mut(para),
            Block::Cell(id) => page.cell_mut(id)?.paragraphs.get_mut(para),
        }
    }

    /// `pos` made valid for the current page: the block must exist; the paragraph index and
    /// offset are clamped and snapped to a grapheme boundary. `None` if the block is gone.
    pub fn clamp_pos(&self, pos: Pos) -> Option<Pos> {
        let page = self.page()?;
        let paras = Session::block_paras(page, pos.block)?;
        let last = paras.len().checked_sub(1)?;
        let para = pos.para.min(last);
        let text = paras.get(para)?.text_cow();
        Some(Pos { block: pos.block, para, offset: notecraft_text::snap(&text, pos.offset) })
    }

    /// The current text range, validated.
    pub fn text_range(&self) -> Option<TextRange> {
        let r = self.sel.text()?;
        let a = self.clamp_pos(r.anchor)?;
        let h = self.clamp_pos(r.head)?;
        if a.block != h.block {
            return Some(TextRange::caret(h));
        }
        Some(TextRange { anchor: a, head: h })
    }

    /// Place a caret (clears pending formatting).
    pub fn set_caret(&mut self, pos: Pos) {
        self.sel.on_page = PageSel::Text(TextRange::caret(pos));
        self.pending_fmt = None;
    }

    /// Set a text range.
    pub fn set_range(&mut self, anchor: Pos, head: Pos) {
        self.sel.on_page = PageSel::Text(TextRange { anchor, head });
        self.pending_fmt = None;
    }

    /// Navigate to a page (pushes the current page on the Back stack).
    pub fn goto_page(&mut self, page: Id) -> Result<(), CmdError> {
        let (sid, _) = self.ws.locate_page(page).ok_or_else(|| CmdError::Failed("that page no longer exists".into()))?;
        let nb = self.ws.notebook_of(sid).map(|n| n.id);
        if self.sel.page != Some(page) {
            self.leave_page();
        }
        self.show_page(nb, sid, page);
        Ok(())
    }

    /// Leaving the current page for another: remember its selection and push it on the Back
    /// stack (capped at 200; a new visit clears Forward).
    fn leave_page(&mut self) {
        let Some(cur) = self.sel.page else { return };
        self.last_sel.insert(cur, self.sel.on_page.clone());
        self.nav_back.push(cur);
        if self.nav_back.len() > 200 {
            self.nav_back.remove(0);
        }
        self.nav_forward.clear();
    }

    /// Show a page without touching the Back/Forward stacks.
    pub(crate) fn show_page(&mut self, nb: Option<Id>, section: Id, page: Id) {
        self.sel.notebook = nb.or(self.sel.notebook);
        self.sel.section = Some(section);
        self.sel.page = Some(page);
        self.sel.pages.clear();
        self.sel.on_page = self.last_sel.get(&page).cloned().unwrap_or_default();
        if self.text_range().is_none() && matches!(self.sel.on_page, PageSel::Text(_)) {
            self.sel.on_page = PageSel::None;
        }
        self.pending_fmt = None;
        self.view.find.matches.clear();
        self.view.recycle_view = None;
    }

    /// Select a section (its first visible page, or none).
    pub fn goto_section(&mut self, section: Id) -> Result<(), CmdError> {
        let s = self.ws.section(section).ok_or_else(|| CmdError::Failed("that section no longer exists".into()))?;
        match s.pages.first().map(|p| p.id) {
            Some(p) => self.goto_page(p),
            None => {
                self.leave_page();
                self.sel.notebook = self.ws.notebook_of(section).map(|n| n.id);
                self.sel.section = Some(section);
                self.sel.page = None;
                self.sel.on_page = PageSel::None;
                Ok(())
            }
        }
    }

    /// Mark everything that differs between `old` and the current workspace dirty.
    fn mark_dirty(&mut self, old: &Workspace) {
        for nb in &self.ws.notebooks {
            let Some(o) = old.notebook(nb.id) else { continue };
            if Arc::ptr_eq(o, nb) {
                continue;
            }
            let d = self.dirty.entry(nb.id).or_default();
            if structure_key(o) != structure_key(nb) {
                d.structure = true;
            }
            if o.recycle != nb.recycle {
                d.recycle = true;
            }
            let old_sections: HashMap<Id, &Arc<notecraft_doc::Section>> = o.sections().into_iter().map(|s| (s.id, s)).collect();
            for s in nb.sections() {
                match old_sections.get(&s.id) {
                    Some(os) if Arc::ptr_eq(os, s) => {}
                    _ => {
                        d.sections.insert(s.id);
                    }
                }
            }
        }
    }

    /// Whether any open notebook has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty.values().any(|d| d.structure || d.recycle || !d.sections.is_empty())
    }

    /// The search index, refreshed for pages that changed since the last query. Pages of
    /// locked sections are never indexed (none exist yet: passwords come in M4).
    pub fn index(&mut self) -> &Index {
        let mut seen: HashSet<Id> = HashSet::new();
        let mut updates: Vec<(PageMeta, String, String)> = Vec::new();
        for nb in &self.ws.notebooks {
            walk_pages(nb, &mut |meta, page| {
                seen.insert(page.id);
                let fresh = self.indexed.get(&page.id).is_some_and(|p| Arc::ptr_eq(p, page));
                if !fresh {
                    updates.push((meta, page.display_title(), page_search_text(page)));
                }
            });
        }
        let stale: Vec<Id> = self.indexed.keys().filter(|id| !seen.contains(id)).copied().collect();
        for id in stale {
            self.index.remove(id);
            self.indexed.remove(&id);
        }
        for (meta, title, text) in updates {
            let pid = meta.page;
            self.index.upsert(meta, &title, &text);
            if let Some(p) = self.ws.page(pid) {
                self.indexed.insert(pid, p.clone());
            }
        }
        &self.index
    }

    /// The search scope for `kind` around the current selection.
    pub fn scope(&self, kind: ScopeKind) -> Scope {
        match kind {
            ScopeKind::Page => self.sel.page.map(Scope::Page).unwrap_or(Scope::All),
            ScopeKind::Section => self.sel.section.map(Scope::Section).unwrap_or(Scope::All),
            ScopeKind::SectionGroup => {
                // The innermost group around the current section; a top-level section has none,
                // so this falls back to the notebook (as the reference greys the option out).
                let group = self.sel.section.and_then(|sid| {
                    let nb = self.ws.notebook_of(sid)?;
                    let path = nb.find(sid)?;
                    let (_, parent) = path.split_last()?;
                    if parent.is_empty() { None } else { nb.node_at(parent).map(NavNode::id) }
                });
                match group {
                    Some(g) => Scope::SectionGroup(g),
                    None => self.sel.notebook.map(Scope::Notebook).unwrap_or(Scope::All),
                }
            }
            ScopeKind::Notebook => self.sel.notebook.map(Scope::Notebook).unwrap_or(Scope::All),
            ScopeKind::AllNotebooks => Scope::All,
        }
    }

    /// Record an informational notice for the UI.
    pub fn notice(&mut self, msg: impl Into<String>) {
        let m = msg.into();
        log::info!("{m}");
        self.messages.push(m);
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }

    /// Mint an id.
    pub fn new_id(&mut self) -> Id {
        self.ids.next_id()
    }
}

/// Whether two workspaces differ (pointer comparison per notebook: copy-on-write makes any edit
/// visible here).
pub fn workspace_changed(a: &Workspace, b: &Workspace) -> bool {
    a.notebooks.len() != b.notebooks.len() || a.notebooks.iter().zip(&b.notebooks).any(|(x, y)| !Arc::ptr_eq(x, y))
}

/// A fingerprint of a notebook's tree (what the TOC files record).
fn structure_key(nb: &Notebook) -> Vec<(usize, u8, Id, String, Option<u8>)> {
    fn walk(nodes: &[NavNode], depth: usize, out: &mut Vec<(usize, u8, Id, String, Option<u8>)>) {
        if depth > notecraft_doc::notebook::MAX_GROUP_DEPTH {
            return;
        }
        for n in nodes {
            match n {
                NavNode::Section(s) => out.push((depth, 0, s.id, s.name.clone(), s.color.0)),
                NavNode::Group(g) => {
                    out.push((depth, 1, g.id, g.name.clone(), None));
                    walk(&g.children, depth + 1, out);
                }
            }
        }
    }
    let mut out = vec![(0, 2, nb.id, nb.name.clone(), nb.color.0)];
    walk(&nb.children, 1, &mut out);
    out
}

/// Visit every page of a notebook with its search metadata.
pub(crate) fn walk_pages(nb: &Notebook, f: &mut dyn FnMut(PageMeta, &Arc<Page>)) {
    fn walk(nodes: &[NavNode], nb: Id, groups: &mut Vec<Id>, depth: usize, f: &mut dyn FnMut(PageMeta, &Arc<Page>)) {
        if depth > notecraft_doc::notebook::MAX_GROUP_DEPTH {
            return;
        }
        for n in nodes {
            match n {
                NavNode::Section(s) => {
                    for p in &s.pages {
                        f(PageMeta { page: p.id, section: s.id, notebook: nb, groups: groups.clone() }, p);
                    }
                }
                NavNode::Group(g) => {
                    groups.push(g.id);
                    walk(&g.children, nb, groups, depth + 1, f);
                    groups.pop();
                }
            }
        }
    }
    walk(&nb.children, nb.id, &mut Vec::new(), 0, f);
}

/// The text a page is searched by: title, paragraphs, tag names and link targets.
pub(crate) fn page_search_text(p: &Page) -> String {
    let mut s = p.plain_text();
    for o in &p.objects {
        for para in o.paragraphs() {
            for t in &para.tags {
                s.push(' ');
                s.push_str(&t.name);
            }
        }
    }
    s
}
