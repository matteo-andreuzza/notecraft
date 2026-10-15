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
