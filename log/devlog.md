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

## 2026-10-20 — M2 increment 3: pictures, attachments, rich clipboard, drag and drop

- **Decision D3** (`docs/plan.md` §4.9, §6): pictures and attachments are page objects beside
  containers and tables; bytes in `Blob { id, len, bytes: Arc<[u8]> }`, serialized as `{id, len}`
  with the bytes in uncompressed `media/<id>` zip entries (section, recycle bin, archive);
  `.ncraft` format 3 (format 1 and 2 files still open, tested); limits: picture ≤ 64 MiB and
  16,384 px / 64 Mpx, attachment ≤ 256 MiB, ≤ 1 GiB media per file, ≤ 10,000 objects per page.
  A missing or mismatched media entry loads as a placeholder, never an error.
- **`notecraft-codecs`** (standalone): PNG/JPEG/GIF/BMP/WebP through `image` 0.25 without default
  features (pure Rust, wasm); header checked before any allocation, EXIF orientation applied,
  decoder panics caught; tests with truncation at every length, byte flips, random bytes, a
  10^12-pixel header.
- **Engine** `picture_cmds.rs` (15 commands): Insert › Pictures / File Attachment (path or base64
  `data`), rotate 90°/flip, move/resize (handles, arrow keys), restore size, background, alt
  text, save as, attachment open/save as/rename; objects copy and paste sharing bytes; whole
  tables copy as table objects. `edit.paste` reads `html` (another app's formatting, lists,
  tables as table objects), `image` and `external`.
- **`notecraft-import-export`** (L4, first code): `html.rs`, a bounded tolerant HTML reader and a
  writer (Copy returns HTML; tables as `<table>`).
- **UI**: `pictures.rs` (LRU texture cache by blob id, turns/flips through UVs, handles,
  placeholders, host actions), picture/attachment context menus from the catalog (rows not yet
  acting on objects drawn disabled), Alt Text and Rename dialogs, `clipboard.rs` (system clipboard
  through `Host::read_clipboard/write_clipboard`, arboard on the desktop; Ctrl+V with only a
  picture caught on the key release because egui-winit drops the press), dropped files become
  pictures or attachments. Snapshots: picture page, selected, dark, context menu. E2E 26/26
  (pictures, handle resize, attachment save, real X11 clipboard round trips).
- Gates: the refreshed advisory DB flagged `zip` 3 (unused `extract()`): upgraded to `zip` 9; the
  zbus helper crates' "unmaintained" advisories are ignored in `deny.toml` with reasons.
- Printouts (not done): PdfCraft's renderer `hayro` 0.7.1 passes NoteCraft's `cargo deny` policy
  (licences, bans, sources, advisories) and `cargo check --target wasm32-unknown-unknown`.
- Numbers: wiring 525/1320 (39.8%), behaviour 158/1320 (12.0%), 279 commands (was 494, 153, 264).
- Still open: printouts, screen clipping, 45° rotation, crop, OCR, picture links, ink, audio;
  M2 exit not met (no merge, no tag).

## 2026-10-20 — M2 increment 4: ink and the Draw tab (decision D4)

- **Decision D4** (`docs/plan.md` §4.10, §6): one ink stroke is one page object
  (`PageObject::Ink`: points relative to the object's position with per-point pressure, colour,
  width, pen or highlighter), so selection, lasso, Delete, Cut/Copy/Paste, Order, Move, Rotate
  and Undo work on ink unchanged; [MS-ONE] ink nodes map stroke by stroke (only the grouping is
  lost). `.ncraft` **format 4** (formats 1–3 open unchanged, tested with a v3 file). Limits:
  20,000 points per stroke, widths 0.1–72 pt, pressure 0–1 (NaN → 0.5), strokes count against
  the 10,000-objects-per-page cap; empty strokes are dropped on load.
- **`notecraft-ink-geom`** (L0): distances, polygon test, iterative RDP, disc cuts, stroke
  outlines, rotation (proptests on hostile input). **`notecraft-ink`** (L2): strokes from
  samples (cleaned, simplified at 0.2 pt, capped), hit tests, the stroke eraser and partial
  erasers (pieces outside the disc become new strokes; the sweep is bounded to 2,000 positions
  after a proptest showed a hostile path costing a minute), the lasso rule (¾ of the points,
  unverified), turns, flips, scaling.
- **Engine** `draw_cmds.rs` (16 commands): Type, Lasso Select, Panning Hand, Eraser and its four
  sizes, the pen gallery, Add Pen, Color & Thickness — session state, never an undo step;
  `draw.stroke` (one undo step), `draw.erase` (coalesced per gesture), `draw.lassoSelect`,
  Rotate Right/Left 45°; Rotate 90°/Flip turn ink about the selection's centre and keep it on
  the page; copy/paste carry strokes.
- **UI** `ink.rs`: strokes as triangle strips with round caps/joins (width from pressure, black
  ink follows the page text colour in dark themes, translucent highlighters), the live stroke,
  lasso and eraser feedback, the dashed selection box (drag to move), pen tool samples from every
  `PointerMoved` of a frame, pressure from egui touch force (else 0.5), the panning hand; the
  pen gallery inline in both ribbon layouts (pen glyphs drawn in code; a click on the selected
  pen opens thickness and colour), the Color & Thickness dialog, the ink and lasso context menus
  (`CM.InkDrawing.*`, `CM.LassoSelection.*` from the catalog), arrow keys nudge ink.
  Snapshots: `ink-page`, `ink-page-classic`, `ink-page-dark`, `ink-lasso`, `ink-zoom-2x`.
- Learned: a touch held still for many frames is egui's long press (the touch-screen context
  menu), not a drag — the kittest pressure test sends one sample per frame, as a pen does.
- Numbers: wiring 574/1322 (43.4%), behaviour 159/1322 (12.0%), 295 commands (was 525/1320,
  158, 279).
- Still open: ruler, shapes, Ink to Text/Math, Ink Replay, resizing ink by handles, tablet
  pressure on Linux (PhotoCraft's XInput2 `tablet` crate), `render`; printouts; audio; M2 exit
  not met (no merge, no tag).

## 2026-10-20 — M2: file printouts

- **Insert › File Printout** (`insert.printout`), and on an attachment **Insert Printout**,
  **Delete Printout Pages**; on a printed page **Open Original** (`printout_cmds.rs`, 4 commands).
  A printout is the attachment plus one picture per page linked to it (`Image::printout`, an
  optional field: format 4 unchanged), stacked below it at the page's print size, one undo step.
- **`codecs::pdf`**: PDF pages rendered by `hayro` 0.7.1 from crates.io (the user's choice; pure
  Rust, PdfCraft's renderer) on white at 108 dpi, at most 200 pages, 3,000 px a side, 8 Mpx a
  page, under the codecs' panic guard; encrypted files refused; `sample_pdf` writes the test PDFs
  (no fixtures). A picture file prints as itself; other formats are refused with a hint.
- `ATTRIBUTION.md` lists the BSD-3-Clause Foxit/PDFium substitute fonts and the CC0 ICC profile
  that `hayro` compiles into the binary.
- Snapshot `printout-page`; e2e check of a two-page PDF in the real app.
- Numbers: wiring 579/1322 (43.8%), behaviour 159/1322 (12.0%), 299 commands.
- Still open: text from printout pages, asynchronous rendering of large PDFs, the "attach or
  print" question when a file is dropped.

## 2026-10-20 — M2: links on pictures, attachments and ink; resizing ink

- Pictures, attachments and ink strokes carry a hyperlink of their own (`link`, optional,
  format 4 unchanged; cleaned on load). Link…, Edit Link…, Open Link, Copy Link and Remove Link
  act on the selected objects (`view_cmds.rs` `object_link`/`set_object_link`,
  `Session::current_link`); Edit Link… opens the Link dialog prefilled, and its OK edits an
  existing link instead of inserting linked text (this also fixes Edit Link… on text, which ran
  `link.edit` without a URL). Linked objects show their link in the tooltip.
- Ink selection box has the eight sizing handles; `object.resize` maps the box `from` → `to`
  (one small step per frame, coalesced into one undo step), or `dw`/`dh`.
- Menu header rows (Rotate, Order/Arrange) of the ink, lasso and Draw tab menus pinned by tests.
- Numbers: wiring 592/1322 (44.8%), behaviour 159/1322 (12.0%), 299 commands.
- Environment: the disk filled up during a gate (`target/debug/incremental` had grown to
  5.7 GB); deleted it and ran later builds with `CARGO_INCREMENTAL=0`.

## 2026-10-20 — M2: Insert Space

- **Insert Space** (`insert.space`, a toggle on Insert and Draw and in the ink menu) turns on a
  tool; dragging on the page runs `canvas.insertSpace` (`BEH.Canvas.InsertSpace`): every object
  that starts below (or right of, when the drag went sideways) the start line moves by the
  drag; removing space stops at the line; the tool returns to Type after one use (unverified).
  A band shows the space while dragging. One undo step; a drag that moves nothing records none.
- Numbers: wiring 596/1322 (45.1%), behaviour 159/1322 (12.0%), 301 commands.
- Environment: a gate failed with the linker killed by the OOM killer (15 GB RAM, ~4 GB
  available): `CARGO_BUILD_JOBS=4` fixed it.

## 2026-10-20 — M2: screen clipping

- **Insert › Screen Clipping** (`insert.screenClipping`): on Linux the desktop app asks the XDG
  Screenshot portal (interactive: the compositor draws the area selection, Wayland or X11) on a
  background thread (`apps/notecraft/src/screen_clip.rs`, zbus); the UI polls
  `Host::take_screen_clip` each frame and inserts the picture with a "Screen clipping taken:"
  line below it (wording unverified), one undo step. Elsewhere it explains why it's unavailable.
  Not exercised live (it would open the selection on the maintainer's screen); the engine path
  and the portal URI decoding are tested.
- Numbers: wiring 597/1322 (45.2%), behaviour 159/1322 (12.0%), 302 commands.

## 2026-10-20 — M2: shapes

- **Draw › Shapes** (`draw.shapes`, a gallery drawn in code: line, arrow, double arrow,
  rectangle, oval, triangle, right triangle, diamond, parallelogram; unverified list) and
  **Lock Drawing Mode** (`draw.lockDrawingMode`). A shape is dragged out (Shift: squares,
  circles, 45° lines) and stored as one ink stroke with the selected pen (`draw.shape`,
  `NC.Draw.Shape`; generator `notecraft_ink::shape_points`), one undo step; without the lock the
  tool returns to Type.
- Drawing: strokes are now split at sharp corners (an averaged normal pinched the line there),
  each run a triangle strip, the corners round joins.
- Numbers: wiring 600/1323 (45.4%), behaviour 160/1323 (12.1%), 305 commands.

## 2026-10-21 — M2: tags on ink (decision D5)

- **Decision D5** (`docs/plan.md` §4.11): `Ink.tags` (optional in `section.json`, format 4
  unchanged; one per key, at most 32). The tag commands treat the selected strokes as one
  drawing: a new tag goes on the leftmost stroke unless one has it; remove/check/clear touch
  every selected stroke, so Find Tags lists a drawing once ("Ink drawing", unverified wording).
  Icons left of the stroke (unverified placement), the checkbox toggles on click (`tag.click`
  takes an ink id), the ink menu's Tags submenu is the whole tag gallery, the Tags pane selects
  a tagged stroke. `ui.inspect` (`ink`) and `notebook.inspect` show ink tags; e2e check added.
- Fixed on the way: Tags pane rows weren't clickable on their text (a selectable label took
  the click from the row).
- Rows: `CM.InkDrawing.TagsGallery/TagsCustomize/TagsRemoveAll/TagsRemoveAll2` wired.

## 2026-10-21 — M2: picture leftovers

- **Paste: Picture** (`edit.pastePicture`): the clipboard's picture even when text came with
  it; NoteCraft's own copied pictures come back as pictures; text alone is refused until the
  renderer exists.
- **Inline pictures in pasted HTML**: `<img src="data:image/…;base64,…">` becomes
  `html::Block::Image` (at most 256; web pictures and pictures in table cells dropped); a paste
  places them in reading order, skipping ones that don't decode; alt text kept.
- **Stacking**: a picture or attachment in front of every container it overlaps is drawn over
  the text and takes clicks there (`pictures::stack`, `Layer`; plan §4.9).
- **Attachment menu headers**: file name and size (`CM.FileAttachment.FileAttachmentName/Size`).

## 2026-10-21 — M2: Tab after text makes a table (decision D6)

- **Decision D6** (`docs/plan.md` §4.8): Tab at the end of a paragraph's text turns it into a
  one-row, two-column table that takes the paragraph's place (`table_cmds::tab_to_table`, from
  `para.indent`'s Tab branch); no session state — later Tabs and Enter use the existing
  one-row-table keys. `BEH.Table.TabToCreate` verified, `KB.Table.CreateWithTab` wired.
- Numbers: wiring 610/1323 (46.1%), behaviour 161/1323 (12.2%), 306 commands; e2e 33/33.
- Environment: the linker was OOM-killed again with `CARGO_BUILD_JOBS=4`; gates ran with 2.
- Open: see `plan/handoff.md` §7 (ink leftovers: ruler, Ink to Shape, tablet pressure, pens in
  settings; picture leftovers: 45° rotation and crop, files pasted from a file manager; page
  templates need screenshots).

## 2026-10-23 — M2: Paste Options on objects, Insert File on drop, pens kept, the ruler (D7)

- **Paste Options in context menus** (`*.PasteGalleryMini`): a "Paste Options:" caption over
  the four paste choices as icon buttons (Keep Source Formatting, Merge Formatting, Keep Text
  Only, Picture; distinct icons now), each reading the system clipboard as Ctrl+V does — the
  old menu item pasted only NoteCraft's own clipboard. Now on the ink menu too
  (`CM.InkDrawing.PasteGalleryMini` wired).
- **Insert File on drop** (`BEH.Paste.Files`): a dropped PDF asks Attach File / Insert
  Printout / Cancel (several at once are asked about once); pictures still insert directly,
  other files are attached (NoteCraft prints only PDFs and pictures).
- **Pens kept**: the desktop settings remember the pen gallery, the selected pen, the eraser
  size and the shape (`settings.json` › `drawing`); `DrawState::normalize` repairs what a file
  gives back (≤ 32 pens, widths in range, the Type tool, no ruler).
- **The ruler (decision D7, `docs/plan.md` §4.12)**: `notecraft_ink::ruler` (geometry and
  `snap`), `DrawState.ruler` (session state, never saved or undone), `draw.ruler`
  (`R.Draw.Tools.Ruler`, shown in the middle of the page's view) and `draw.rulerPlace`
  (`NC.Draw.RulerPlace`, new row, verified); `draw.stroke` lays a stroke started within 12 pt of
  a long edge along it (`"free": true` opts out), and the canvas shows the same while drawing.
  The canvas draws it as a translucent card with millimetre ticks and its angle; a drag on it
  moves it, the wheel over it turns it a degree a notch. Snapshot `ink-ruler.png`.
- Numbers: wiring 613/1324 (46.3%), behaviour 162/1324 (12.2%), 308 commands.
- Open: two-finger turning of the ruler, Snap to Grid, Draw with Touch, Ink to Shape, tablet
  pressure on Linux; files copied in a file manager pasting as attachments; the M2 exit
  workflows (see `plan/handoff.md` §7).

## 2026-10-24 — M2: the exit workflows, Snap to Grid, Draw with Touch (D8)

- **Headless acceptance** (`crates/automation/tests/m2_workflows.rs`, helpers shared with M1 in
  `tests/common/mod.rs`): "a page with a table and pictures" (a table filled with Tab, rows
  added in the last cell, a header row, sorted, the header shaded; a picture from bytes resized
  with alt text, one from a file turned; a file attachment) and "handwritten notes" (pen
  strokes, a shape, a line along the ruler, a lasso selection tagged as a checked to-do and
  moved, the stroke eraser). Both drive MCP tools over a folder notebook, then a fresh session
  reads the page back from disk and must see exactly the same objects (blobs included).
- Seen on the way: a picture under 4 pt a side is raised to 4 pt on each side separately, so a
  tiny picture loses its proportions; kept (it keeps a hair-thin separator thin), the test uses
  a 16×8 picture.
- **P0/P1 rows of the M2 exit still `missing`** (`R.Draw.*` and `BEH.Ink/Table/Paste.*` have
  none left):
  - `R.Insert.Pages.PageTemplatesSplitButton`, `R.Insert.Pages.PageTemplatesTaskpane` (page
    templates: need the user's screenshots).
  - `CT.TableLayout.Data.SortMenu` (the Sort… dialog), `CT.TableLayout.Merge.MergeCells`,
    `CT.TableLayout.Merge.UnmergeCells` (cell spans: a model change to discuss first).
  - Object menus: `CM.NoteContainer.OrderObjectsMenu`, `CM.NoteContainer.ObjectMove`,
    `CM.NoteContainer.ObjectResize`; `CM.ParagraphHandle.AlternativeText`,
    `CM.ParagraphHandle.ImageSaveAs`, `CM.ParagraphHandle.TableMenu`,
    `CM.ParagraphHandle.TableInsert`; `CM.Equation.Cut`, `CM.Equation.CopyOneNote`,
    `CM.Equation.PasteGalleryMini` (equations, later).
  - Outside the M2 scope but P0/P1 and small: `CM.Tag.FindTaggedNotes`,
    `CM.OutlookTask.TagsRemove`, `CM.Bullet.ChangeToNumberList`, `CM.Number.ChangeToBullet`,
    the object rows of `CM.Text.*` (Rotate, flips, Alt Text, Set Picture as Background, Table).
- **Decision D8 (`docs/plan.md` §4.13)**: `draw.snapToGrid` (`R.Draw.Shapes.GridSnapTo`) puts a
  shape's two end points on the page's grid, or on 1/8 in without one, in the engine's
  `draw.shape` and in the canvas preview; `draw.withTouch` (`R.Draw.Tools.FingerPaintingMode`,
  on by default) off makes a finger pan with the drawing tools, a finger being a touch without
  pressure (egui can't tell pens from fingers); two fingers on the ruler move it and turn it
  about their midpoint, the first finger's drag becoming a drag of the ruler. Both settings are
  kept in the desktop settings with the pens.
- **Sort… for tables** (`CT.TableLayout.Data.ShowSortTableRowsDialog`, `table.sortDialog`): a
  Sort dialog (Sort by, Then by, each Ascending or Descending, Header row), columns named by the
  header row's text; the engine sorts by 1–3 columns in turn, stably, and can set the header
  row in the same undo step. The Table Layout › Sort menu is tested end to end
  (`CT.TableLayout.Data.SortMenu` verified).
- Numbers: wiring 617/1324 (46.6%), behaviour 163/1324 (12.3%), 311 commands; e2e 33/33.
- Open: the object menu rows above (NoteContainer Order/Move/Resize, ParagraphHandle Alt Text,
  Save As, Table), copied files from a file manager pasting as attachments, merge/split cells
  (to discuss), page templates (need screenshots).

## 2026-10-24 — M2 done: object menus, merged cells (D9), page templates (D10), equations (D11)

- **Object menus:** the note container's Order submenu, Move and Resize (they keep the container
  selected and say how; the arrow keys then nudge it, Shift+Left/Right change its width), the
  paragraph handle's Table submenu with Insert Table. The handle's picture rows (Alt Text, Save
  As, Copy Text from Picture) are `out-of-scope`: a paragraph never holds a picture (D3), the
  picture's own menu has them.
- **Merged cells (decision D9, `.ncraft` version 5):** `TableCell::row_span`/`col_span`, covered
  cells kept in the grid empty and hidden; `table.merge` joins the selected cells' text into the
  first, `table.split` undoes it; row/column inserts and deletes, Tab and the arrows respect
  merged cells; Sort refuses cells merged across rows; the canvas lays a merged cell over its
  columns and rows and draws borders cell by cell. Snapshot `table-merged.png`.
- **Page templates (decision D10):** NoteCraft's own built-in set (Academic, Blank, Business,
  Decorative, Planners) built in code, the user's own saved in the preferences (pictures left
  out), the last five used, and a default template per section (`Section.default_template`)
  that New Page starts from; Insert › Page Templates opens the Templates pane; Save As Template
  dialog; `template.list/setDefault/save/delete` for agents (new rows `NC.Templates.*`).
  Snapshot `templates-pane.png`.
- **Equations (decision D11):** new pure crate `notecraft-math` (L2) parses the UnicodeMath
  linear format (fractions with whole terms as operands, scripts, radicals with degrees, large
  operators with limits, stretchy brackets, matrices, accents, function names, control words,
  `⬚` slots) and lays it out built up, the large signs drawn as paths; an equation is a
  paragraph of linear text (`Paragraph::math`), built up on the canvas and edited as text in a
  light box while the caret is in it; Enter never splits one, a pasted one stays one.
  Insert › Equation (Alt+=) with built-in equations previewed built up, the contextual Equation
  tab (Professional, Linear, Symbols, eleven Structures galleries, own icons and a simplified
  layout), the equation context menu. Snapshots `equations.png`, `equation-tab.png`.
- **The M2 exit is met:** no P0/P1 row of `R.Insert.*`, `R.Draw.*`, `CT.TableLayout.*`,
  `CT.Equation.*`, the object `CM.*` or `BEH.Ink/Table/Paste.*` is `missing`; the headless
  workflows pass. Audio and video (P2) are left for after M3.
- Numbers: wiring 657/1328 (49.5%), behaviour 174/1328 (13.1%), 335 commands.

## 2026-10-25 — Code review: duplicated, dead and slow code; HTML spans; two dialogs

Branch `chore/code-review` (from `main` = `c03d1ed`). A review pass for duplicated, obsolete,
redundant and unoptimised code, found with a line-window duplicate scanner (exact and with
identifiers normalised), an unused-public-item scan and clippy's extra lints; only changes that
were certain landed, behaviour unchanged (every test green after each commit).

- **Dead code removed:** `icons_set.rs` kept all 306 icons ported from WordCraft but only the 57
  that `icons::set_name` maps to can be painted (1455 → 399 lines); `icons::draw_own` had 34
  arms (and parts of 19) for keys `set_name` always routes to the set. Unused items:
  `DocError` (and doc's `thiserror`), `AUTOMATIC_TEXT`, `PAGE_COLUMN_WIDTH`, `ICON_LARGE`,
  `has_bold`, `Dirty::is_clean`, `NotebookStore::section_path`, `Session::redo_labels`,
  `group_from_entry`; `proptest` dev-deps of four crates that never used it; `let _ = x;`
  lines that only hid dead variables and imports; redundant `too_many_arguments` allows.
- **Duplicates merged:** `automation::base64` (a copy of `engine::b64::encode`, now a
  re-export); `need_section`/`need_notebook` (six copies, now in `ops`), the link clipboard
  (four), search scope parsing, the search box frame, URL autolink on typing and Enter,
  "hidden under a collapsed ancestor" (pages and paragraphs), the math parser's scripts, the
  dialogs' section walk, attachment insertion (files and printouts), object drag start, the
  handle cursor, the theme picker, `execute_request`/`save_now` (headless and the desktop
  control channel), Back/Forward (`travel`), the UI tests' `harness()`.
- **Fixes found on the way:** going to an empty section skipped the 200-entry Back cap and
  kept a stale Forward stack (one `leave_page` now); stale M1 comments; an orphan doc comment.
- **Faster:** `text::snap` (twice per `Session::text_range`, which many enabled predicates call
  every frame) scanned the paragraph from its start: now a grapheme cursor around the offset,
  pinned to the old scan by a proptest; `Paragraph::text_cow` avoids a copy for single-run
  paragraphs; `ribbon::command` returns `&'static str`; `with_spans` collects merged cells once.
- **M2 leftovers:** merged cells copy to other applications as HTML `colspan`/`rowspan` (covered
  places left out) and pasted HTML spans make merged cells (`html::TableBlock`, the reader lays
  cells out as browsers do, hostile spans clamped); `UI.Dialogs.ParagraphSpacing` verified and
  `UI.Dialogs.NotebookProperties` wired (display name and colour; no location/format choice).
- Numbers: wiring 659/1328 (49.6%), behaviour 175/1328 (13.2%), 335 commands.
- Open: `link.open` still shows enabled off a link (its `url` param makes a `when` predicate
  wrong; needs a UI-side check); the two loggers of the binaries are near copies (kept: they
  differ in filtering); the canvas lays every page out each frame (a cache by revision would be
  a larger change). Next: M3 (`plan/next-session-prompt.md`), starting with the D12 proposal.

## 2026-10-27 — M3 starts: decision D12, crate `onestore`, Open Section

- **D12 approved** (the three questions answered: read-only in M3, real-file fidelity measured by
  the user locally, import as a copy). `docs/plan.md` §4.17 and the D12 row. Branch `m3/interop`
  from `chore/code-review` (the review branch is pushed to `origin`, not merged yet).
- **Crate `onestore` (L4, `doc` + `geom` only, wasm-clean)**, written from the public
  [MS-ONESTORE]/[MS-ONE] PDFs (rev. 13.3, local copies in `plan/specs/`, never committed), every
  structure cited: `bytes` (bounds-checked cursor), `types` (§2.2), `header` (§2.3.1; OneNote
  2007 and foreign files are `Unsupported`), `txlog` (§2.3.3, uncommitted nodes ignored),
  `nodelist` (§2.4: fragment chains, magic numbers, loops refused), `filenode` (§2.4.3, the §2.5
  nodes; unknown nodes skipped by size), `propset` (§2.6.1–2.6.9, generic by stored type,
  references resolved), `revision` (§2.1: object spaces, current revision of role 0x1/default
  context, dependency chains with loop guard, object groups, global id tables incl. `.onetoc2`
  Entry2/3, file data store §2.6.13, encrypted spaces → `Encrypted`), `one` ([MS-ONE] walk:
  section → page series → page spaces → title, outlines, nested outline elements, Unicode and
  Windows-1252 text, PageLevel, times) with a per-construct `Report`. `#![deny(clippy::indexing_slicing)]`.
- **Test writer** `onestore::writer` (tests + `fixtures` feature): raw nodes and lists, property
  sets, whole stores (`StoreBuilder`), synthetic sections (`writer::section`). 46 tests incl.
  every truncation, byte flips and a proptest of random tails (no panic).
- **`file.openSection`** (`BS.Open.OpenSection`, Ctrl+Alt+Shift+O, `onenote_cmds.rs`): path or
  base64 `data`, a unique section name, one undo step, the report in a notice and the result.
- Catalog: `FMT.One.ReadPages` verified; `FMT.One.Read`, `FMT.One.ReadOutlines`,
  `BS.Open.OpenSection` wired (outline position ignores PageMarginOrigin — unverified; the
  reference opens sections in place). Numbers: wiring 663/1328 (49.9%), behaviour 176/1328
  (13.3%), 336 commands.
- Open (D12 order of work, step 2): text formats (TextRunIndex/TextRunFormatting →
  jcidParagraphStyleObject), lists, tags, tables, pictures, attachments, ink; then `.onetoc2`,
  `.onepkg`; the UI file picker for Open Section; a CLI `import-one` report command.
- **Later the same day** (`84dc81a`, `4220d3f`, and the pictures commit): Open Section in the
  backstage and on Ctrl+Alt+Shift+O through the host picker (UI test with a fake picker,
  `backstage-open` snapshot); rich text (TextRunIndex/TextRunFormatting, character formats,
  style ids, alignment; hidden runs dropped; text hyperlink targets are not in the public spec),
  lists (NumberListFormat/MSONFC, ListRestart), note tags (built-in names keep NoteCraft's tag,
  others the nearest icon for NoteTagShape), pictures (PictureContainer → file data store,
  `codecs::probe` for pixels), attachments (EmbeddedFileContainer), tables (rows, cells, column
  widths, borders); content inside outlines becomes page objects below them (unverified
  placement). `onestore` now depends on `codecs` too. 50 tests. Rows `FMT.One.ReadRichText`,
  `ReadLists`, `ReadTags`, `ReadImages`, `ReadTables`, `ReadAttachments` wired. Numbers:
  wiring 669/1328 (50.4%), behaviour 176/1328 (13.3%), 336 commands.
- Open: ink (its [MS-ONE] structures), merged cells and shading in `.one` tables, `.onetoc2`,
  `.onepkg`, a CLI report command, the per-construct fidelity table once the user has measured
  real files.

## 2026-10-28 — M3: table shading, OneNote notebook folders; ink and TOC are not public

- **What the public spec leaves out** (checked in `plan/specs/ms-one.txt`, rev 13.3,
  2022-11-15): no ink JCID or property, no merged-cell property, and none of the `.onetoc2`
  PropertyIDs (FileIdentityGuid, FolderChildFilename, NotebookColor,
  NotebookElementOrderingID, TOCEntryIndex_OidIndex: the structures are described, their ids
  are not in the 144-entry table). Under the clean room they can't be read: ink stays reported
  lost (`FMT.One.ReadInk` missing, note says blocked), spans read as 1.
- `.one` tables: CellShadingColor (§2.2.97) → `TableCell::shading` (automatic = none); the
  fixture writer shades cells (`FixturePage::cell_shading`).
- **OneNote notebook folders** (user's choice): `file.openNotebook` on a folder with a
  `.onetoc2`/`.one` and no NoteCraft TOC (or on the `.onetoc2` itself) imports an in-memory
  copy (`onenote_cmds::import_onenote_folder`): each `.one` a section named by its file, each
  subfolder with sections a group, sorted by name; `OneNote_RecycleBin`, hidden entries and
  symlinks skipped; limits on files, total bytes and depth; damaged files are warnings, a
  folder with nothing readable fails cleanly; nothing is ever written to the folder (test
  compares the listing after `save_dirty`). Before this, such a folder opened as an empty
  NoteCraft notebook that would have written into it. `onestore::Report::absorb`/`lose_one`.
- Catalog: `FMT.Onetoc2.Read` wired (order from the folder — unverified). Numbers: wiring
  670/1328 (50.5%), behaviour 176/1328 (13.3%), 336 commands.
- Open: `.onepkg` ([MS-CAB]: download the spec to `plan/specs/`, unpack to the same folder
  import); a CLI report command; the fidelity table after the user's real-file measurements.
- **Later:** `notecraft-cli onenote PATH [--json]` prints the fidelity report of a `.one`, a
  OneNote folder or a `.onetoc2` (throwaway session, nothing saved): the tool for the user's
  real-file measurements. Both import results now list the damage messages (`warnings`, ≤ 50).
  `.onepkg` is on hold: the official [MS-CAB] PDF could not be found (the learn.microsoft.com
  and officeprotocoldoc URLs return 404; only secondary/GPL sources online, not usable under
  the clean room). The `cab` crate (MIT) was rejected: unchecked indexing and `unreachable!`
  on its read path. Plan: a bounded CAB reader in `onestore` (MSZIP via `miniz_oxide`,
  LZX/Quantum refused) once the spec is in `plan/specs/`.

## 2026-10-29 — M3: `.onepkg` packages

- Specs: the official [MS-CAB] (v20101026) and [MS-MCI] PDFs are gone from Microsoft's sites;
  the Wayback Machine copy of a mirror (`web.archive.org/web/2025…/rfc.nop.hu/winprot2/MS-CAB.pdf`,
  `MS-MCI.pdf`) is Microsoft's own document. Both in `plan/specs/` (text beside them).
- `onestore::cab` (`read_cabinet`, `CabLimits`): CFHEADER/CFFOLDER/CFFILE/CFDATA with every
  offset checked; stored and MSZIP folders (fresh decoder per block on a buffer that starts
  with the previous 32 KB, so back-references across blocks work — [MS-MCI] §2); Quantum, LZX,
  split cabinets refused; names UTF-8 or Windows-1252, `..`/empty parts dropped; a damaged
  folder costs only its files (warnings); caps on files and bytes unpacked; csum not verified
  (optional per §2.4.1). New dependency `miniz_oxide` 0.9 (already in the tree via flate2).
  The `cab` crate was rejected (panics on its read path). Tests: the spec's §3 example byte
  for byte, a hand-made fixed-Huffman block referring into the previous block, truncations,
  byte flips, a proptest. Fixture writer `writer::cab`.
- Engine: the notebook import walks a `Source` (disk folder or unpacked package);
  `file.openNotebook` on a `.onepkg` imports an in-memory copy named after the file, stepping
  into a single top folder (package layout unverified). CLI `onenote` takes `.onepkg` too.
- Catalog: `FMT.Onepkg.Read` wired. Numbers: wiring 671/1328 (50.5%), behaviour 176/1328
  (13.3%), 336 commands.
- Open: the fidelity table in `parity.md` after the user's real-file measurements
  (`notecraft-cli onenote PATH`); then the M3 exports/imports (PDF/DOCX/HTML/Markdown, ENEX…).

## 2026-10-29 — M3: Export page and section (web page, HTML, Markdown)

- `import-export::export`: `collect` turns pages into a format-neutral export (objects in
  reading order: top to bottom, then left to right; title and date line per page; pictures and
  attachments as resources, each blob once, names made safe and unique; ink counted as lost);
  `html_document` (inline `data:` URIs or relative links), `mht` (RFC 2557 multipart/related,
  base64 parts with `Content-Location`, RFC 2047 subject). `import-export::markdown`:
  CommonMark + GFM tables, strikethrough, task items for to-do tags; escapes what would start
  a block; fenced code; nested lists. `TableBlock::from_table` moved here from the engine;
  `b64` moved from the engine to `codecs` (re-exported as `notecraft_engine::b64`).
- Engine `export_cmds`: `file.exportPage`, `file.exportSection` (`format` mht/html/md or the
  path's extension), `file.exportMht` (`scope` page/section). Markdown writes pictures and
  files into `<name>_files` beside it, never through a link; `.one/.docx/.pdf/.xps` refused as
  "not available yet". Automation roots cover the three commands (write).
- UI: File › Export is the reference's two steps (1. Export Current: Page/Section/Notebook,
  2. Select Format, Export…); formats not written yet are listed disabled. The section menu's
  Export… opens it with that section chosen.
- Catalog: BS.Export.Page, BS.Export.Section, BS.Export.FormatMht,
  CM.Section.FileSaveAsContextualItem, FMT.Export.Html, FMT.Export.Markdown wired. Numbers:
  wiring 677/1328 (51.0%), behaviour 176/1328 (13.3%), 339 commands.
- Dev machine: the disk was full (target/ 27 GB); stale test binaries, incremental and
  examples removed; builds use `CARGO_INCREMENTAL=0` for now.
- Open: PDF (FMT.Export.Pdf, needs a page renderer to vectors), DOCX (WordCraft crates?),
  Markdown/text import, ENEX; the `.one` fidelity table still waits for real-file runs.

## 2026-10-29 — M3: Export to Word (.docx)

- `import-export::docx`: own WordprocessingML writer from ECMA-376 Part 1 (no WordCraft
  dependency needed): content types, relationships, core properties, styles built from
  `ParaStyle::def` (built-in ids/names for Heading1–6, Title, Quote; Citation and Code custom;
  Hyperlink character style), numbering (an abstract list per bullet glyph and per number
  format, one instance per run of numbered paragraphs, `restart` as `startOverride`), runs with
  every character format in schema order, hyperlinks as external relationships, tabs and soft
  breaks, to-do tags as ☐/☒, tables (fixed grid from the column widths, borders or `nil`,
  shading, `gridSpan`/`vMerge`), inline pictures scaled to the 468 pt text width (WebP/BMP
  re-encoded as PNG, a picture used twice stored once), page breaks between pages.
  Attachments become "Attached file: name" (reported). Export items now carry table widths,
  borders and shading.
- Engine: `format: "docx"` (or `.docx`) on `file.exportPage`/`file.exportSection`; UI: Word
  Document enabled for pages. Checked by hand: LibreOffice headless opens the sample section
  and a page with a link, a 3×3 table and a picture (text and PDF conversion look right).
- Catalog: BS.Export.FormatDocx, FMT.Export.Docx wired. Numbers: wiring 679/1328 (51.1%),
  behaviour 176/1328 (13.3%), 339 commands.

## 2026-10-29 — M3: Export to PDF (decision D13), crate `fonts`

- Decision D13 (plan §4.18, §6): flow formats share `export::collect`; PDF is drawn like the
  canvas with `krilla` 0.8 (MIT/Apache; features `raster-images` only — `simple-text` would
  pull the unmaintained `rustybuzz`/`ttf-parser`, and glyphs are measured here anyway).
- New L0 crate `notecraft-fonts`: the page font candidates (Carlito, Noto, Liberation,
  DejaVu; mono; symbol/math fallbacks), size-capped and validated with skrifa, looked up once
  per process. The canvas now takes its page faces from it (same bytes the PDF embeds).
- `import-export::pdf`: `text` lays paragraphs out with skrifa metrics (greedy breaks at
  spaces, hard breaks, tab stops every 36 pt, super/subscript rise, "at least" line spacing,
  glyph fallback across faces); the page becomes a display list (title + underline + date,
  containers with the canvas's indent/list/tag columns, bullets drawn as shapes, tables with
  merged cells, shading and borders, pictures with turns and flips, attachment cards, ink with
  round caps, highlighter opacity), then sheets: paper from the page setup else Letter with
  36 pt margins, scaled to the width, breaks between line and row bands. Real text with
  subsetted fonts, link annotations, metadata, attachments embedded as PDF files.
- Engine: `format: "pdf"` on `file.exportPage`/`file.exportSection`, and
  `file.exportNotebook` with `format: "pdf"` or a `.pdf` path. UI: PDF enabled for all three.
- Checked by hand: sample pages rendered with pdftoppm look like the canvas; `pdffonts`
  shows embedded, subsetted CID TrueType faces with Unicode maps.
- Catalog: BS.Export.FormatPdf, FMT.Export.Pdf wired. Numbers: 681/1328 (51.3%), behavioural coverage 176/1328 (13.3%), 339 commands.
- Disk: still tight (1.5 GB free after the gate); prune `target/debug/deps` between gates.

## 2026-10-30 — M3: Print (File › Print, Ctrl+P)

- Engine: `file.print` (BS.Print.Print, Ctrl+P) and `file.printPreview` (BS.Print.Preview)
  write the print-ready PDF of the current page or section (`scope`) into `dir` (default: a
  `notecraft-print` temp folder) through the PDF export, and return its path and title.
  Automation roots cover `dir`.
- UI: a File › Print backstage page (printer list from the host with the default chosen,
  copies, current page or whole section; Print and Print Preview); Ctrl+P opens it. Host gains
  `printers()` and `print_pdf()`; the desktop host uses `craft-print` (craft-libs, same pinned
  revision; CUPS `lp`/`lpstat`, native Windows printing), Print Preview opens the PDF in the
  system viewer. deny.toml allows storytold's winprint fork (craft-print's Windows backend).
- Catalog: BS.Print.Print, BS.Print.Preview, KB.Pages.Print wired (QAT.FilePrint stays until
  the QAT can be customised). Numbers: wiring 684/1328 (51.5%), behaviour 176/1328 (13.3%), 341 commands.
- Dev machine: `cargo clean` (25 GB of stale artifacts); builds keep `CARGO_INCREMENTAL=0`.

## 2026-10-30 — M3: Import Markdown, text and HTML files

- `import-export::import`: Markdown through `pulldown-cmark` 0.13 (MIT; tables,
  strikethrough, task lists) into HTML and then the clipboard's HTML reader, so a file keeps
  what a paste keeps; local pictures go through `nc-res:N` references (`html::parse_refs`,
  never accepted from plain HTML) so they skip the HTML size cap; task items become ☐/☑
  markers; plain text (a paragraph per line, tabs → levels; UTF-8 else Latin-1); HTML files
  as a paste. A leading first-level heading is the title.
- Engine `file.import` (new row NC.File.Import): a new page in the current section (its
  default template applies), content placed in fresh containers (`paste_blocks` gains
  `fresh`), ☐/☑ → To Do tags, pictures read only from the Markdown file's folder and below
  (canonicalised, size-capped), one undo step. Automation roots: read.
- UI: File › Open › Import File… (the stale ".onepkg coming later" line is gone).
- Catalog: NC.File.Import (new), FMT.Import.Markdown, FMT.Import.Text, FMT.Import.Html wired.
  Numbers: wiring 688/1329 (51.8%), behaviour 176/1329 (13.2%), 342 commands.
- Next: Evernote .enex (a section with a page per note) on the same path.

## 2026-10-30 — M3: Import Evernote exports (.enex)

- `import-export::enex`: Evernote's public export format with a small pull XML reader
  (elements, CDATA, the five entities and numeric references; no DTD), MD5 from RFC 1321
  (its test suite in the tests) to match `<en-media hash>` to resources, ENML → the HTML
  reader (`en-todo` → ☐/☑, `en-media` pictures through `nc-res:` references, `en-crypt`
  dropped and counted), other resources as files. Caps: 10,000 notes, 1,000 resources each;
  truncated or junk input ends the read, never a panic.
- Engine: `file.import` on a `.enex` makes a section named after the file (unique in the
  notebook) with a page per note (title, created/updated dates, content in fresh containers,
  To Do tags, attachments), opens the first, one undo step. UI picker offers `.enex`.
- Catalog: FMT.Import.Enex wired. Numbers: wiring 689/1329 (51.8%), behaviour 176/1329 (13.2%), 342 commands.

## 2026-10-30 — M3: Export a page as a PNG picture

- `pdf::PdfOptions::fit_content`: one sheet as large as the page's content (18 pt margin, no
  scaling). Engine `format: "png"` on `file.exportPage` renders that sheet with the codecs'
  PDF renderer at 144 dpi (capped at 16,000 px a side, 64 Mpx) and writes a PNG; a section is
  refused (one page per picture). UI: PNG Picture in File › Export for pages.
- Catalog: FMT.Export.Images wired (PNG; SVG not yet). Numbers: wiring 690/1329 (51.9%),
  behaviour 176/1329 (13.2%), 342 commands.

## 2026-10-31 — M3: import pictures and PDFs, reopen archives, Change Location

- `file.import` reads pictures (PNG/JPEG/GIF/BMP/WebP: a page titled with the file name
  holding the picture) and PDFs (a page holding its file printout, `printout_cmds::printout`
  now takes bytes). The file is checked before the page is made. The picker offers all of them.
- `file.openNotebook` on a `.ncraft-archive` opens a copy in memory; when the same notebook is
  already open the copy gets fresh notebook/group/section/page ids. Backstage Open gains
  "Open a Package or Archive…" (`.onepkg`, `.ncraft-archive`), which a folder picker could not reach.
- New `notebook.changeLocation` (row UI.Dialogs.NotebookProperties.ChangeLocation): writes
  the whole notebook into a new or empty folder and keeps saving there. This lets an imported
  OneNote notebook (folder, `.onepkg`) or an archive become a notebook on disk; until now an
  import could only be exported. The old folder is never deleted (the reference moves it:
  unverified). Notebook Properties shows the location and a Change Location… button.
- Catalog: FMT.Import.Images, FMT.Export.Archive, the new row wired. Numbers: wiring
  693/1330 (52.1%), behaviour 176/1330 (13.2%), 343 commands.
- A second session (notecraft-61) took the Paper Size pane and SVG export on `m3/paper-size`.

## 2026-10-31 — M2 leftovers: Open Link only on a link, pasting copied files

- `NoteApp::disabled_reason` draws `link.open` (KB.Format.OpenLink, "Open Link" in the text
  and handle menus) disabled unless the caret or the selected object has a link. The command
  itself still takes an explicit `url`, so clicks on links and agents are unchanged.
- BEH.Paste.Files: files copied in a file manager (`text/uri-list`, arboard `file_list`)
  paste like dropped files: pictures as pictures, PDFs through the Insert File dialog, the rest
  as attachments, at the caret. Their paths no longer get typed in as text. `RichClip.files`,
  `pictures::insert_files` shared with drops, `MAX_PASTED_FILES` = 50.

## 2026-10-31 — M2 leftover: Ink to Shape

- `notecraft_ink::recognize`: one stroke recognised as a line, triangle, rectangle/square,
  diamond, other four-sided shape, pentagon, hexagon, ellipse or circle. It compares the
  stroke's corner polygon (RDP at 8% of its size, weak corners merged) with the ellipse in its
  box and keeps the closer one within 6% of the size. Rectangles, squares, diamonds and circles
  are straightened; other polygons keep the corners drawn. Strokes under 8 pt, scribbles and
  writing stay as drawn; input over 20,000 samples is never examined.
- Engine: `draw.inkToShape` toggle (`DrawState.ink_to_shape`, no undo step); `draw.stroke`
  replaces a recognised stroke with the neat shape in the same pen and reports `shape`
  (`free: true` skips it, as for the ruler). The ribbon's Ink to Shape button is live and
  shows its state. The source is Microsoft's public "Convert ink to shapes in Office" page;
  the thresholds are unverified.
- Numbers: wiring 694/1330 (52.2%), behaviour 176/1330 (13.2%), 344 commands.
## 2026-10-31 — M3: the Paper Size pane

- Engine `paper_cmds.rs` (new): `view.paperSize` opens and closes the pane
  (`ViewState.paper_pane`); `page.paperSize` sets the page's `PageSetup.paper` — Auto (none),
  a named paper (`PAPER_SIZES`: Statement, Letter, Tabloid, Legal, A3–A6, B4–B6 JIS, Japanese
  Postcard, Index Card, Billfold), Custom width/height, Portrait/Landscape (the margins turn
  with the paper) and the four print margins; values clamped to the PDF export's limits; no
  params reports the paper. One undo step per change.
- UI: `panes::paper_pane` (Size, Orientation, Width, Height in inches, Print Margins, Save
  current page as a template); a drag or a typed number runs one command when it ends. The
  canvas dashes where a page's printed sheets end (`canvas::paper_marks`, unverified).
  Snapshot `paper-pane`.
- Catalog: R.View.PageSetup.PaperSize, UI.Panes.PaperSize wired; new rows
  UI.Panes.PaperSize.{Size, Orientation, Dimensions, Margins, SaveTemplate}. Numbers: wiring
  700/1335 (52.4%), behaviour 176/1335 (13.2%), 345 commands.
- Parallel sessions: a worktree must use its own `CARGO_TARGET_DIR` — two checkouts sharing
  one target dir get the same artifact hashes and run each other's binaries.
- Next here: SVG page export (FMT.Export.Images).

## 2026-10-31 — Behaviour rows pinned (BEH.*)

- New `engine/src/tests_behaviour.rs` pins 15 P0 behaviour rows whose code already existed but
  was never marked: copied page links navigate, a new notebook's default section and page,
  numbering 1./a./i. and renumbering, subpages (two levels, move with the parent, collapse),
  page date/time, dragging a page to another section, emptying the bin, nested section groups,
  checkbox clicks, several tags on one paragraph, tag toggling, the default font, styles
  replacing character formatting, and the zoom range and stops.
- `mark.py` also promoted rows whose tests already carried `// parity:` tokens (FMT import and
  export rows, the Paper Size pane). Numbers: wiring 716/1335 (53.6%), behaviour 190/1335
  (14.2%), 346 commands.
## 2026-10-31 — M3: Export a page as an SVG picture

- `import-export::pdf::svg::write`: the PDF's display list (same layout, fonts, pictures, ink,
  rule lines and page colour as the PDF and PNG exports) written as SVG 1.1, one picture the
  size of the content (18 pt margin). Text is glyph outlines from the page fonts (`skrifa`
  pen, one `<defs>` path per glyph, at most 20,000, `<use>` per character), so the file looks
  the same without the fonts; pictures are `data:` URIs (PNG/JPEG/GIF/WebP as they are, others
  re-encoded to PNG); links are `<a>` areas. `pdf::rule_segments` now feeds both writers.
- Engine: `format: "svg"` on `file.exportPage` (a section is refused, one page per picture).
  UI: SVG Picture in File › Export for pages. Checked by eye with rsvg-convert against the PNG.
- Catalog: FMT.Export.Images notes (PNG and SVG). Numbers unchanged (a format of an existing row).

## 2026-10-31 — M4: the Options dialog, napkin math

- File › Options is now the Options dialog (`ui-egui/src/options.rs`): twelve categories down
  the left as in the reference. Changes go to a draft of `Prefs`, OK applies them in one
  `file.options` call, Cancel forgets them. Categories NoteCraft can't offer yet (Proofing,
  Send to, Audio & Video, Language, Ribbon/QAT customization, Add-ins) say so instead of
  showing controls that do nothing.
- New `Prefs` fields, each one changing behaviour: initials, show_mini_toolbar,
  show_note_containers (canvas frames), default_color (with default_font/size: an empty Normal
  paragraph types in them, `ops::typing_fmt`), show_paste_options, create_table_on_tab,
  calculate_math, backup_folder, auto_backup_minutes and last_backup_ms (`Session::backup_due`;
  the UI loop runs `file.backupChanged`), default_notebook_location (used by
  `file.newNotebook` without a path unless `memory`), quick_notes_notebook.
- New command `file.backupChanged` (OPT.SaveBackup.BackupChangedNow). `file.backupNow` falls
  back to the Options folder.
- BEH.Text.MathCalc: `notecraft_math::calc` reads the operators, functions and examples of
  Microsoft's public pages ("Insert and calculate simple math equations", "Napkin Math in
  OneNote"). Space or Enter after `expr=` types the answer.
- Catalog: 18 OPT/BEH rows wired. Numbers: wiring 736/1335, behaviour 191/1335.
## 2026-10-31 — M3/P0: the Notebook Recycle Bin in the page column

- History › Notebook Recycle Bin (and the notebook's context menu) already ran
  `history.recycleBin`, but nothing showed the bin: deleted pages and sections could not be
  found again from the UI. `nav::recycle_bin` now takes over the page column while
  `ViewState.recycle_view` is set: Deleted Pages (title, from section, date) and Deleted
  Sections (page count, date), Restore on hover and on right click, Move to › a section for
  pages (`history.restore` with `section`), Empty Recycle Bin (asks first), ✕ to leave.
  Restoring opens the item, which leaves the bin. Snapshot `recycle-bin`; UI test.
- Catalog: UI.Panes.RecycleBin wired (P0). Not yet: a read-only preview of a deleted page, the
  60-day purge (BEH.Recycle.DeletedPages).

## 2026-10-31 — M3/P0: File › Exit is a command

- New `file.exit` (BS.Exit, QAT.NR.FileExit): saves every notebook, then sets
  `Session.quit_request`, which `NoteApp::logic` turns into the shell's close. When a notebook
  can't be saved it refuses ("not closing: …") and the window stays open; the backstage Exit
  used to save, ignore errors and close. Agents and the control channel can run it too.
- Catalog: BS.Exit, QAT.NR.FileExit wired.

## 2026-10-31 — Desktop: the window opens as it was left

- `settings::Window` (in `settings.json`): inner size, maximized, and the position where the
  windowing system reports one (not on Wayland). The shell observes the viewport every frame
  (`Window::observe`: maximized, full-screen or minimized windows keep the size to restore to)
  and opens the next run with it (`Window::viewport`); the settings are now read before the
  window opens. Hostile values are clamped or dropped (`Window::sane`). Closes part of gap 6.

## 2026-10-31 — M4: password-protected sections (decision D15)

- `notecraft_format::protect`: sealed section files (magic `NCRAFT-SEALED`, a JSON header in
  the clear, XChaCha20-Poly1305 over the ordinary zip with the header as associated data,
  Argon2id keys; costs read from a file are bounded). `decode_section` opens a sealed file
  locked; `encode_section` writes a locked section's bytes back unchanged and refuses an
  unlocked protected section (`FormatError::NeedsKey`), so nothing protected is ever written
  in the clear. Dependencies: chacha20poly1305 0.10, argon2 0.5, zeroize, getrandom 0.3
  (native only), all MIT OR Apache-2.0.
- doc `Section.protection` (`Protection { salt, kdf, locked }`, never in the JSON).
- Engine `protect_cmds.rs`: review.password (pane), section.setPassword / changePassword /
  removePassword / unlock / lock, protect.lockAll (Ctrl+Alt+L), `Session::auto_lock` (idle
  minutes and lock-on-leave from the Options), `sealed_copy` before every save and Change
  Location. Keys stay in `Session.keys` and are zeroed on drop. Locking clears the undo
  history (`Session.forget_history`). Deletes skip the Recycle Bin, and protected sections
  can't be deleted or copied. Archive export refuses locked sections.
- UI `password_ui.rs`: the Password Protection pane, the unlock prompt in place of the page,
  the lock on the section row, the lock options in Options › Advanced.
- Fixed: the first-run guard for automatic backups (meant for the Options commit) was never
  applied; now a fresh install waits one interval.
- Catalog: 16 rows wired. Numbers: wiring 751/1340 (56.0%).
## 2026-10-31 — M3/P1: History › Recent Edits

- New `history_cmds.rs`: `history.recentEdits` lists the pages changed in a period (Today and
  Since Yesterday from local midnight; 7, 14, 30 days; 3 and 6 months as 91/182 days,
  unverified; all), newest first by `Page::modified`, in a scope (section, section group,
  notebook, all notebooks), capped at 5,000; it opens the Recent Edits pane
  (`ViewState.recent_edits`).
- UI: `panes::recent_pane` (Show, Search, the list; a click opens the page); the ribbon's
  eight period rows pass their range (`ribbon::params_for`). Snapshot `recent-edits`.
- Catalog: R.History.Authors.FindByDateMenu and its 8 periods, UI.Panes.RecentEdits,
  BEH.History.RecentEdits wired. Numbers: wiring 748/1335 (56.0%), behaviour 191/1335, 349
  commands.
