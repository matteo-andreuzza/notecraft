# NoteCraft dev log

Terse entries, newest last: what landed, numbers, what's still open. Sessions can end abruptly;
this log plus a green tree is how the next agent picks up.

## 2026-10-14 — bootstrap

- Audited the sibling Crafting Apps (cloned read-only into `.siblings/`, git-ignored):
  `docs/structure-audit.md` records what NoteCraft adopts (workspace layout, lints, profiles,
  `xtask`, packaging, CI, craft-libs pinning) and every conflict with the brief.
- Scaffold: workspace manifest, `xtask` (`layers`, `wasm`, `ci`, `assets`, `parity`, `deny`,
  `version`, `ico`), `notecraft-geom` (L0), AGENTS/CLAUDE/README, licences, NOTICE, ATTRIBUTION,
  omastore, deny.toml, CI and release workflows (the shared craft-common-actions pipeline),
  Linux and web packaging, an original placeholder app icon.
- Toolchain on the dev machine: Rust 1.99 stable (rustup, `~/.cargo`), wasm32 target.
- Still open: the parity catalog and plan (next commit), then M1.

## 2026-10-14 — parity catalog and plan

- `docs/parity-catalog.md`: 1,291 rows. Bootstrapped from Microsoft's public Office Fluent UI
  control-identifier list for OneNote 2019 (every ribbon tab/group/control, contextual tabs,
  backstage, 30 context menus, commands not in the ribbon, docked ribbon), the "OneNote for
  Windows" keyboard-shortcut table (178 rows, conflicts in the source noted), Microsoft 365
  additions (Help tab, newer Draw/View controls, style and tag gallery items), 59 UI surfaces,
  75 options, 70 behaviours, 31 format constructs. Planned command ids filled in for P0–P2.
- `cargo xtask parity` reads it: wiring 0/1291, behaviour 0/1291 (nothing implemented yet).
- `docs/plan.md`: milestones by parity value, M1 increments, crate graph, the key design
  decisions (flat paragraph lists with levels, structural-sharing undo, `.ncraft` notebook
  folders with atomic per-section saves, ribbon built from the catalog), risks, assumptions.
- `cargo deny` passes (cargo-deny installed locally).

## 2026-10-15 — M1: shell and core editing (tag `m1-core`)

- Crates: `doc` (L1 model: workspace → notebooks → section groups → sections → pages → outlines of
  flat paragraphs with levels), `text` and `search` (L2; 100k pages searched in < 50 ms),
  `format` (L3 `.ncraft` notebook folders: TOC JSON + one zip per section, atomic saves through
  craft-atomic, recycle folder, backups; the kill -9 test reopens a notebook killed mid-save six
  times), `engine` (L5 session: run = enabled check → snapshot → catch_unwind → one undo step,
  coalescing, dirty tracking, autosave; 227 commands), `ui-egui` (L6 OneNote-style shell: title
  row + QAT, ribbon laid out from the catalog, notebook pane, section tabs, page list, canvas,
  panes, find bar, backstage, three themes), `automation` (L6 loopback control channel with a
  per-start 0600 key, MCP server over stdio, read/write path roots).
- Apps: `notecraft-cli` (`commands`, `run`, `batch`, `mcp [--connect]`), `notecraft` (desktop:
  eframe/wgpu, settings.json, reopens notebooks and the last page, first-run "My Notebook",
  `--control PORT` with key input, clicks and screenshots), `notecraft-web` (wasm, sample
  notebook in memory; checked for wasm32, not yet served with trunk here).
- Acceptance: `crates/automation/tests/m1_workflows.rs` runs meeting notes, a To Do task list
  with Find Tags, find-by-search and page reorganisation through MCP on a folder notebook and
  reads it back from disk. The desktop app was driven end to end under Xvfb through
  `notecraft-cli mcp --connect` (typing, Ctrl+1, screenshots, quit; settings and notebooks
  reopened on the next start).
- Bugs found that way and fixed: a new page didn't take keyboard focus after Ctrl+N; right-click
  on a page tab or notebook-pane row opened the empty-area menu; highlighted text was white on
  yellow in the dark theme; the "+" section tab could be covered by the search box.
- Numbers: wiring 366/1302 (28.1%), behaviour 109/1302 (8.4%), 227 commands; 37 implemented rows
  carry `unverified` notes (listed in `docs/parity.md`). `parity:` markers that claimed more
  than their tests check were removed.
- Dropped: `craft-settings` for the desktop's preferences (it links MPL-2.0 `option-ext`; the
  brief allows permissive licences only).
- Still open (ranked in `docs/gaps.md`): `.one` import (M3); tables, pictures, files, ink (M2);
  rich clipboard; canvas details (paragraph handle, auto-width containers, mini toolbar,
  text context menus); Move/Copy, Link and Options dialogs; web persistence; async file dialogs;
  section-tab overflow; Tell Me and ScreenTips tests.

## 2026-10-15 — M1.5: the 2024 look (tag `m1.5-modern-ui`)

- Decision D1 (user directive): the visual model is the current OneNote for Microsoft 365 (2024
  UI), not OneNote 2016; it overrides the brief's layout (recorded in `docs/plan.md` §6, the brief
  untouched). New `docs/ui-parity.md` (structure, tokens, type, 24 surfaces with status); catalog
  source `M365-2024`, 12 new rows, 2016-only surfaces out of scope naming their replacement.
- A1 theme tokens after WordCraft (window background, ribbon card, page column, selection, text
  ramp, our violet accent #7A3DB8 / lilac in the dark themes, radii, sizes, type ramp); themes
  Colorful, White, Dark Gray, Black and "Use system setting" (XDG portal watcher ported from
  WordCraft; zbus was already in the tree via rfd); UI font Inter (CRAFT_FONTS_DIR or system) →
  Noto Sans, separate from the page's Carlito stack; contrast and no-colour-literal tests.
- A2 `chrome.rs`: neutral title bar (own drawn mark, Back, Undo, ▾ menu, title, centred Search
  for commands that hands text to Search notebooks, Alt+Q; author initials), tab row with the
  3 pt accent underline, Share disabled.
- A3 ribbon as a floating rounded card; `ui.ribbonMode` + `ViewState.ribbon_mode` (Simplified
  default), the ▾ chevron menu (layout, Always show / Show tabs only), remembered;
  `ribbon_simplified.rs` (own design: placements by priority, … overflow with every unplaced
  classic control, contextual tabs derived; coverage test).
- A4 navigation: ☰ + Search notebooks row, tree of every notebook (folds, colour bars, + New
  section, Quick notes), page column with Add page and sort; section tabs removed; new commands
  `view.navigationPanes`, `nav.quickNotes`, `page.sort`; the dragged pane width is now saved
  (bug), folds and the page column width too.
- A5 expand/restore button on the page, panes as cards, floating find card, toasts instead of the
  status bar, accent primary buttons, backstage rail restyled. A6 WordCraft's line icon set
  (`icons_set.rs`, ATTRIBUTION.md) mapped onto ~60 controls; the rest recoloured from the theme.
  A7 `ui.inspect` reports `ribbonMode`/`navigation`; snapshots of every tab in both layouts, all
  themes, narrow, nav hidden; e2e of the real app under Xvfb 8/8 (driver in `plan/research`).
- Numbers: wiring 377/1314 (28.7%), behaviour 109/1314 (8.3%), 231 commands (was 366/1302,
  227); in-scope 30.5% / 8.8%.
- Still open: see `docs/gaps.md` item 8 (older icons on ~135 controls, default combo boxes,
  auto-hide, QAT customization, Options dialog, drawn window buttons). Next: M2 Insert and Draw.

## 2026-10-15 — M2 (part 1): canvas structure and tables (branch `m2/insert-draw`)

- Canvas: the paragraph handle drags the paragraph with its children (pointer x sets the level,
  a 2 pt accent drop line; dropped on empty canvas it starts a new container:
  `outline.moveParagraph` takes x/y); click selects, double-click collapses. Drags hit-test the
  press origin, so a quick drag of a handle, bar or edge no longer misses. Kittest tests drive the
  real canvas with pointer events (handle, container bar and edge, growing containers and page).
- Tables (decision D2, `docs/plan.md` §4.8): `PageObject::Table` (rows, cells with paragraphs and
  shading, column widths, borders, header row; limits 5,000 rows / 64 columns / 50,000 cells),
  `Block::Cell` (cells are ordinary text blocks), `PageSel::Cells`; `.ncraft` format 2 with a
  byte-exact format-1 file test; loaded pages are now normalised in full (`normalize_pages`: the
  old loader only fixed page levels). `table_cmds.rs`, 25 commands: Insert › Table (size grid),
  select table/columns/rows/cell/range, delete, insert above/below/left/right, hide borders,
  shading, sort, header row, column width, convert text ↔ table, Tab/Shift+Tab/Enter/Alt+Enter;
  arrows cross cells; copy, delete, find and search see cells. Canvas layout per cell, borders,
  shading, selected cells, column-border drag, drag across cells; `chrome::contextual_tabs`
  shows Table Layout in both ribbon layouts with icons drawn on a 3 × 3 grid.
- Fixed: a Tab used by the page (indent, next cell) let egui move focus to the search box, so the
  next keystrokes went there.
- `ui.inspect` reports contextual tabs, tables and handles; control method `ui.pointer` for drags;
  e2e under Xvfb 16/16 (8 new); snapshots add a table page, Table Layout simplified and classic,
  selected cells, a hovered handle. `panic_hunt` gains two table states.
- Numbers: wiring 416/1320 (31.5%), behaviour 134/1320 (10.2%), 256 commands (was 377/1314,
  109, 231).
- Not done here (next, in order, `plan/handoff.md` §7): mini toolbar, text/container context
  menus, Paste Options, dialogs (Insert Table included), Insert misc, gaps item 8; then pictures
  and the rich clipboard, then ink. M2 exit criteria not met: no merge, no tag.

## 2026-10-16 — M2 (part 2): mini toolbar, context menus, Paste Options (branch `m2/insert-draw`)

- Mini toolbar (`UI.Canvas.MiniToolbar`, `mini_toolbar.rs`): a card above a text selection with
  font, size, bold, italic, underline, highlight, font colour, outdent, indent, bullets,
  numbering, styles and tags — reuses the Home ribbon's own `pub(crate)` helpers (`combo`,
  `small`, `gallery_menu`), so it's the same commands, `enabled` and tooltips as the ribbon, not
  a parallel implementation. Works in table cells too (read-only off `CanvasState.paras`; no
  change to canvas layout or hit-test).
- Context menus (`CM.Text.*`, `CM.NoteContainer.*`, `CM.ParagraphHandle.*`, `context_menu.rs`):
  built straight from the catalog's own `Path` column (a one-off Python script turned 134 rows
  into `Entry::Item`/`Submenu` trees), so the menus can't drift out of sync with the catalog;
  `menus_cover_every_context_row` checks placement both ways. Every entry is the ribbon's own
  catalog control (`menu_item`/`special`); unimplemented ones (ink, OCR, linked notes, picture
  rotation, spelling, translate, Excel/Visio) show disabled with a reason. A right click resolves
  to the paragraph handle, else a container's bar, else the text/cell under the pointer (`hit_para`
  already covers cells, so a cell's right click gets the Text menu with a working Table submenu);
  it places the caret/selection there as a left click would, unless the point is already inside
  an existing selection. New commands `object.bringForward/bringToFront/sendBackward/sendToBack`
  (the Note Container menu's Order submenu) reorder within `Page::objects`'s existing z-order, no
  model change; a boundary no-op is checked read-only before `page_mut` so it costs no undo step
  (`Arc::make_mut`'s copy-on-write would otherwise record one from the clone alone).
- Paste Options (`UI.Canvas.PasteOptions`, `paste_options.rs`): a button near the caret for 6 s
  after any paste, opening the ribbon's own three choices (Keep Source/Merge/Text Only).
- Found and fixed along the way: egui's `Response::context_menu`, reused for a *second* right
  click on the canvas's one shared response within a kittest session, closes the popup again
  within a frame or two (not a production bug — reproduced down to a minimal repro, a single
  right click stays open indefinitely, and real use always has real frames between two clicks);
  worked around with one right click per kittest harness. Found, not fixed: `link.open` has no
  `enabled` predicate, so "Open Link" shows enabled even off a link.
- Tests: 7 new kittest tests (mini toolbar, 4 context-menu targets, a table-cell right click,
  Paste Options) plus an engine test for the 4 z-order commands; e2e under Xvfb 18/18 (2 new:
  a table-cell right click, Paste Options after a title paste), new screenshots `mini-toolbar`,
  `context-menu-text`, `paste-options`.
- Numbers: wiring 481/1320 (36.4%), behaviour 142/1320 (10.8%), 260 commands (was 416/1320,
  134, 256).
- Not done here (next, in order, `plan/handoff.md` §7): the dialogs (Insert Table, Link with page
  search, Move/Copy as a tree, notebook list, Recent Notebooks, Symbol, Options), small table
  leftovers, gaps item 8, the `link.open` fix. Stopped after three complete, committed steps on a
  long session, before starting the larger dialogs arc. M2 exit criteria not met: no merge, no tag.

## 2026-10-16 — M2 (part 3): Dialogs, `KB.Table.DeleteEmptyRow` (branch `m2/insert-draw`)

- Dialogs, three commits (`e18c0c1`, `c2ba2db`, `f2c539d`), one `egui::Modal` driven by
  `app.ui.dialog: Option<Dialog>` (`dialogs.rs`) and `ribbon::dialog_for` intercepting the
  matching ribbon/context-menu click:
  - Insert Table (`R.Insert.Tables.TableInsert`): `insert.tableDialog` reuses `insert_table`
    (made `pub(crate)`), same pattern as `para.spacingDialog`.
  - Link (`UI.Dialogs.Link`) and Move/Copy Page/Section (`UI.Dialogs.MoveCopyPage/Section`):
    page/section/notebook search added. Shipped as a flat, searchable, full-breadcrumb-path list
    rather than a collapsible tree — the catalog's own "simplest compatible choice" per
    `AGENTS.md`, and the catalog Notes say so rather than overclaiming "tree" fidelity.
  - Notebook List (`Ctrl+G`, `UI.Nav.NotebookList`) and Recent Notebooks (`BS.Open.RecentNotebooks`):
    a new engine command `file.openRecent`, `UiState.recent_notebooks` (capped at 8, tracked in
    `NoteApp::try_run`'s single after-every-command hook), a Recent Notebooks section in the Open
    backstage page, and a desktop-settings round trip (`settings.rs`).
  - Symbol (`UI.Dialogs.Symbol`, `R.Insert.Symbols.*`): a 10-per-row button grid of Unicode
    symbols (clean-room: plain Unicode, nothing derived from Microsoft glyphs); both the gallery
    and full-dialog ribbon entries map to the same `Dialog::Symbol` so there's one implementation,
    not two.
  - Options (`UI.Dialogs.Options`, `OPT.General.OfficeTheme`): the theme picker also appears on
    the Options page (it already existed on Account); `OPT.General.OfficeTheme`'s own pre-existing
    "unverified wording" catalog note correctly keeps it at `wired`, not `verified`.
  - Recurring kittest snag: a dialog's row label (a breadcrumb, a plain notebook name) can exactly
    match an always-rendered background control (the nav tree), so an exact `query_by_label`
    panics on 2+ matches — fixed per-case with `_contains` substrings, `query_all_by_label(...)
    .count()`, or collapsing the competing pane first.
  - Investigated, not applied: `link.open`'s missing `enabled` predicate (noted in part 2). Its
    body already takes an optional `url` override (wiki-link follow, `para.copyLink`'s navigate-
    back) used from a caret position that is *not* on a link — both paths are tested
    (`wiki_links_create_and_follow_pages`, `links_insert_edit_remove_copy_select`). `Enabled` is
    `fn(&Session) -> Option<&'static str>`, blind to params, and `Session::run` checks it
    unconditionally; a caret-only predicate would disable those explicit-`url` calls too. No safe
    one-line fix exists without either breaking that navigation or widening `Enabled` to see
    params (too big for a P2 nicety) — left as is.
- `KB.Table.DeleteEmptyRow` (`03503fa`): Delete at the very start of a wholly empty table row
  removes it (`table_cmds::delete_empty_row`, called from `text_cmds::delete_forward`'s existing
  "nothing left to merge forward" branch). Simplified to one press, not the reference's reported
  two — the end state is the same and a cross-call key-repeat counter is a lot of state for a P2
  behaviour; the catalog Note says so.
- Not done, judged not worth it this session: `BEH.Table.TabToCreate` (Tab after text makes a
  table) needs a real design decision (first Tab must create the table and keep "creating" mode
  for further Tabs/columns, Enter must exit it) rather than a one-line change, so it stays a gap.
- Docs updated to match: `gaps.md` gap 5 (Dialogs) closed and renumbered; `roadmap.md`,
  `plan.md`'s M2 increments 2 and 5.
- Numbers: wiring 494/1320 (37.4%), behaviour 153/1320 (11.6%), 264 commands (was 481/1320, 142,
  260).
- Not done here (next, `plan/handoff.md` §7): gaps item 8 (leftovers of the look) if a future
  session has room; then pictures, the rich clipboard, then ink. M2 exit criteria not met: no
  merge, no tag.
