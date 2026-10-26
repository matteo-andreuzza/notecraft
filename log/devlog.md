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
