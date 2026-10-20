# NoteCraft plan

> **Last reviewed:** 2026-10-16 · **Last updated:** 2026-10-16 · **Change:** minor (M2: Dialogs done; `KB.Table.DeleteEmptyRow`) · **Target:** Microsoft OneNote for Microsoft 365, desktop for Windows (the 2024 UI)

How NoteCraft gets from nothing to a OneNote replacement a typical user could switch to. The
inventory of what "OneNote" means is [`parity-catalog.md`](parity-catalog.md) (1,291 rows); the
numbers are in [`parity.md`](parity.md); the sibling conventions we follow are in
[`structure-audit.md`](structure-audit.md).

> **Decision D1 (2026-10-15):** the visual model is the **current OneNote for Microsoft 365 (2024
> UI)**, not OneNote 2016 / Windows 10. It overrides the window layout fixed in the original brief
> (section tabs on top, page list on the right); the brief stays verbatim and §6 records the
> override. Commands, shortcuts and behaviour are unaffected. The per-surface specification is
> [`ui-parity.md`](ui-parity.md).

## 1. Principles

1. **Parity value first.** Work is ordered by how much of a OneNote user's day it unlocks: the
   P0 rows of the catalog (navigation, typing and formatting on the free-form canvas, tags,
   search, never losing a note) before anything else; depth over breadth.
2. **Catalog-first increments.** Each increment takes catalog rows, implements them to
   behavioural parity, tests them (pinning documented behaviour with `// parity: <ID>`), flips
   their status and commits. A control that exists without behaviour is drawn disabled.
3. **One registry.** Every action is an engine command; the ribbon, context menus, keyboard, CLI,
   control channel and MCP are views over the same registry.
4. **Never crash, never lose a note.** Atomic saves, crash-safe autosave, backups and a
   `kill -9` recovery test come in M1, not later.

## 2. Milestones

| # | Milestone | Catalog rows it targets | Exit criteria |
|---|---|---|---|
| **M1** | **Shell and core editing** | `UI.*` (window, ribbon with every tab/group present, navigation pane, section tabs, page list, canvas, search box, Find Tags pane); notebook/section group/section/page `CM.*` and `QAT.NR.*` rows; `R.Home.*` (clipboard, basic text, styles, tags); `KB.*` for editing, formatting, selection, tags, outlines, pages, notebooks, search; `BEH.Canvas/Outline/Text/Tags/Pages/Sections/Notebooks/Recycle/Save/Search.*`; `FMT.Native.Ncraft` | Every P0 row of those areas wired; the core workflows (meeting notes, task list with To Do tags, find a note by search, reorganise pages) pass headlessly; kill -9 recovery test green; screenshots of every tab reviewed |
| **M1.5** | **Modern UI (the 2024 look, decision D1)** | `UI.*` surfaces (title bar with Search, tab row, ribbon card in Simplified and Classic modes, left navigation with the notebook tree, page column, Quick notes), `OPT.General.OfficeTheme`, `R.View.Views.NavigationPanes`; every surface in [`ui-parity.md`](ui-parity.md) | Every surface of `ui-parity.md` built in the 2024 style in light and dark; the Simplified ribbon reaches every Classic control (inline or in the **…** overflow); themes Colorful, White, Dark Gray, Black and system; screenshots of every tab in both ribbon modes and every theme reviewed beside the references; no colour literal outside `theme.rs`/`icons.rs` |
| M2 | Insert and Draw | `R.Insert.*`, `R.Draw.*`, `CT.TableLayout.*`, `CT.Equation.*`, `CT.AudioVideo.*`, picture/attachment/ink `CM.*`, `BEH.Ink/Table/Paste.*` | Tables with all Table Layout tools, pictures, attachments, printouts, date/time, templates, rule lines/grid, symbols, ink with pressure, lasso, eraser modes |
| M3 | Interop | `FMT.*`, `BS.Export.*`, `BS.Open.OpenSection` | `.one`/`.onetoc2`/`.onepkg` import with measured fidelity per construct; `.one` export where the specs allow; PDF/DOCX/HTML/Markdown exports; Markdown/HTML/text/ENEX/PDF/image imports |
| M4 | History, Review, View, Help, power features | `R.History.*`, `R.Review.*`, `R.View.*`, `R.Help.*`, `OPT.*`, `UI.Dialogs.Customize*`, `UI.Ribbon.Keytips`, `BEH.Password/Versions/History.*` | Page versions with diff, spelling, password-protected sections (XChaCha20-Poly1305 + Argon2id), full Options, customizable ribbon and QAT, keytips, AccessKit, en + it localization, optional folder/WebDAV sync, WASM plug-ins, MCP tools; drawn window buttons where the compositor draws none |
| M5 | Release | Performance targets, packaging | Flatpak/AppImage/deb and `craft-launcher` integration; Windows/macOS builds; the performance budgets (10,000 pages open < 2 s, typing < 16 ms, search over 100k pages < 50 ms, 500+ ink strokes); 50 real workflows with a measured success rate in `parity.md`; user guide; tag `v1.0` |

### M1 increments (in order)

1. `notecraft-doc` (L1): the object model below, stable ids, invariants, hostile-input-safe
   constructors; `notecraft-color` (L0): section and highlight palettes.
2. `notecraft-text` (L2): paragraph editing (insert, delete, split, merge, apply/toggle formats
   over byte ranges at grapheme boundaries), word boundaries, auto-bullet/number detection.
3. `notecraft-format` (L3): `.ncraft` notebook folders, atomic writes (`craft-atomic`),
   versioned schema, recovery of temp files, backups.
4. `notecraft-search` (L2): inverted index, prefix/phrase queries, scopes.
5. `notecraft-engine` (L5): `Session`, selection/caret model, navigation history, undo by
   structural-sharing snapshots, the registry and the M1 command modules (`notebook_cmds`,
   `section_cmds`, `page_cmds`, `text_cmds`, `format_cmds`, `para_cmds`, `style_cmds`,
   `tag_cmds`, `outline_cmds`, `nav_cmds`, `search_cmds`, `view_cmds`, `file_cmds`), autosave,
   `panic_hunt`, the catalog floor test.
6. `apps/notecraft-cli`: `commands`, `run`, `batch`, `mcp` (stdio); `cargo xtask parity` reads
   the registry through it.
7. `notecraft-ui-egui` (L6): theme tokens, icons drawn in code, the ribbon built from the
   catalog's ribbon rows (so its structure is the reference's by construction), notebook pane,
   section tabs, page list, canvas (containers, paragraphs, caret, selection, handles), search,
   Find Tags pane, dialogs; offscreen screenshot example.
8. `apps/notecraft` (desktop, control channel) and `apps/notecraft-web` (trunk).
9. Gates, parity numbers, devlog, tag `m1-core`.

### M1.5 increments (in order)

Each step is its own commit with `cargo xtask ci` green and its screenshots reviewed beside the
reference screenshots (kept locally, never committed: clean room, we only observe and measure).

1. **A0** Mandate and documents: decision D1, the catalog's `M365-2024` source, new and
   reclassified rows, [`ui-parity.md`](ui-parity.md).
2. **A1** Theme foundations: tokens (colours, radii, spacing, heights, type ramp) for Colorful,
   White, Dark Gray, Black and the system theme; a separate UI font family (Inter from
   `CRAFT_FONTS_DIR` or the system, else Noto Sans); no colour literals outside `theme.rs`/`icons.rs`.
3. **A2** Title bar (neutral, centred Search for commands and notes) and the tab row (semibold
   active tab with an accent underline; Share disabled).
4. **A3** The ribbon in a floating rounded card with two modes: Simplified (one row, overflow
   **…**, default) and Classic (labelled groups), switched by the ▾ chevron and remembered.
5. **A4** Navigation: ☰ and Search notebooks under the ribbon; the tree of every notebook with
   + New section and Quick notes; the page column with Add page and sort; section tabs removed.
6. **A5** Canvas (expand button), panes as cards, Find bar, menus, dialogs, backstage, toasts.
7. **A6** Icons in the WordCraft style (20×20 grid, 1.25 stroke, line + accent), drawn in code.
8. **A7** Tests (ribbon modes, Simplified coverage, contrast, tree, Quick notes, sort, settings)
   and screenshots (every tab in both modes, every theme, narrow window, 2×).
9. **A8** Parity, docs, devlog; tag `m1.5-modern-ui`.

### M2 increments (in order)

Everything new is built in the 2024 look of M1.5 (tokens, cards, the simplified and classic
ribbon; [`ui-parity.md`](ui-parity.md)).

1. **Canvas details (done):** paragraph handle (`UI.Canvas.ParagraphHandle`), containers that
   grow with their text, a page that grows right and down, the mini toolbar as a card, text,
   container and paragraph-handle context menus (`CM.Text.*`, `CM.NoteContainer.*`,
   `CM.ParagraphHandle.*`, generated from the catalog's own `Path` column), Paste Options.
2. **Tables (done, decision D2 / §4.8):** a `PageObject::Table` in `notecraft-doc`, its `.ncraft`
   schema (format 2), commands, the Table Layout contextual tab in both ribbon layouts
   (`chrome::contextual_tabs`), sort, borders, convert to/from text, the Insert Table dialog,
   `KB.Table.DeleteEmptyRow`. Left for later: Tab-after-text table creation, merged cells.
3. **Pictures, attachments, printouts (decision D3 / §4.9; pictures, attachments, clipboard and
   drag and drop done):** crate `codecs` (PNG/JPEG/GIF/BMP/WebP through `image` without default
   features), `PageObject::Image`/`Attachment` with `Arc`-shared blobs in `media/` zip entries
   (`.ncraft` format 3), Insert › Pictures and File Attachment, the picture and attachment
   context menus (rotate 90°, flip, move, resize by handles and keys, restore size, background,
   alt text, save as, open, rename); the rich clipboard (crate `import-export`: a bounded HTML
   reader and writer; pictures and HTML through `arboard` on the desktop host; tables paste as
   tables); dropping files on the page. Left: file printouts (PdfCraft's `hayro` 0.7.1 passes
   our `cargo deny` policy and builds for wasm), screen clipping through the desktop portal,
   45° rotation, cropping.
4. **Ink (decision D4 / §4.10):** crates `ink-geom` and `ink` (pens, highlighters, eraser modes,
   lasso, pressure from winit; tablet-v2/XInput2 as a risk), the Draw tab's tools; later the
   ruler, shapes, recognition and crate `render` (CPU oracle for thumbnails and export).
5. **Dialogs (done):** Insert Table, Link with page search, Move/Copy as a flat searchable list
   (not a tree — simplest compatible choice, noted in the catalog), Options (with the Office
   Theme), Recent Notebooks, the notebook list (Ctrl+G), Symbol. Left for later: page templates,
   equations (`math`), asynchronous file dialogs, window geometry, web persistence (IndexedDB)
   and exports as downloads.
6. **Audio** recording in pure Rust where possible; video stays a documented gap.

Exit: the P0/P1 rows of `R.Insert.*`, `R.Draw.*`, `CT.TableLayout.*`, object `CM.*` and
`BEH.Ink/Table/Paste.*` wired; headless workflows "page with a table and pictures" and
"handwritten notes"; tag `m2-insert-draw`.

### M3–M5 increments

- **M3 Interop:** crate `onestore` ([MS-ONESTORE]/[MS-ONE], every structure cited, limits against
  hostile input, fuzzing): read `.one` (pages, outlines, rich text, lists, tags, pictures, tables,
  ink), `.onetoc2`, `.onepkg` ([MS-CAB]); `.one` export where the specs allow; crate
  `import-export`: PDF, HTML, Markdown, DOCX, PNG/SVG out; Markdown, HTML, text, ENEX, PDF, pictures
  in. A fidelity table per construct (kept, degraded, lost) in `parity.md`; tag `m3-interop`.
- **M4 History, Review, View, power:** page versions with diff, authors, unread; spelling (open
  dictionaries, licences checked); protected sections (XChaCha20-Poly1305 + Argon2id, with a
  threat model); full Options; ribbon and QAT customization; keytips (WordCraft's `keytips.rs`);
  AccessKit tried with a screen reader; en + it; optional sync (folder/WebDAV, CRDT); WASM
  plug-ins; more MCP tools; drawn window buttons where the compositor draws none; tag `m4-power`.
- **M5 Release:** Flatpak/AppImage/deb and `craft-launcher`; Windows/macOS builds; the
  performance budgets (10,000 pages < 2 s, typing < 16 ms, search over 100k pages < 50 ms, 500+
  ink strokes); 50 real workflows with a measured success rate in `parity.md`; user guide; tag
  `v1.0`.

## 3. Crate graph

```text
L0  util ─ geom ─ color          ink-geom (M2)      codecs (M2)
L1  doc ───────────────────────────────────────────────────────
L2  text      search      ink (M2)      math (M2)
L3  format    render (M2: CPU oracle for exports/thumbnails)    gpu (M2)
L4  onestore (M3)   import-export (M3)   ocr (M2 hook)   plugins (M4)   sync (M4)
L5  engine ─────────────────────────────────────────────────────
L6  ui-egui      automation (control channel + MCP)
apps notecraft  notecraft-cli  notecraft-web
```

## 4. Key design decisions

### 4.1 Object model (L1)

Mirrors the reference's object model ([MS-ONE] §2.2 node types) so `.one` interop maps
one-to-one:

- `Workspace` → open `Notebook`s (+ the Quick Notes destination).
- `Notebook { id, name, color, children: Vec<NavNode>, recycle_bin }`, `NavNode = Section |
  SectionGroup`, `SectionGroup { id, name, children }` (arbitrary nesting, depth-capped).
- `Section { id, name, color, pages: Vec<Arc<Page>>, protection }`.
- `Page { id, title: Paragraph, level: 0..=2, created, modified, author, objects: Vec<PageObject>,
  setup: PageSetup { color, rule_lines, hide_title, paper }, collapsed }`.
- `PageObject::Outline(Outline { id, pos, width: Auto | Fixed(pt), paragraphs })` in M1;
  images, ink, files, tables, printouts, media in M2.
- An outline holds a **flat list of paragraphs with an indent `level`** (invariant:
  `level[i+1] <= level[i] + 1`); a paragraph's children are the following paragraphs with a
  deeper level. This is equivalent to the reference's outline-element tree (conversion is
  lossless both ways) and makes caret movement, selection and deletion across paragraphs simple.
- `Paragraph { runs: Vec<Run>, style, align, list, tags, spacing, rtl }`, `Run { text, fmt }`;
  runs are normalised (no empty runs, adjacent equal formats merged).
- Ids: 128-bit (`replica:64 | counter:64`), unique per replica without coordination
  (CRDT-friendly for sync in M4), serialised as hex strings.

### 4.2 Undo

Structural sharing: sections hold `Arc<Page>`, notebooks `Arc<Section>`; a command edits through
`Arc::make_mut`, so a snapshot of the whole workspace is a cheap pointer copy and an edit copies
only the path to the page it touches. `Session::run` snapshots before running, pushes **one**
history entry if the workspace changed (typing coalesces into word-sized steps), and restores the
snapshot if the command fails or panics.

### 4.3 Persistence (`.ncraft`)

A notebook is a folder, as in the reference: `notebook.ncraft-toc` (JSON: name, colour, child
order) at its root and in every section-group subfolder; one `<Section name>.ncraft` file per
section (a zip holding `section.json` and, from M2, `media/`); deleted items in
`.ncraft-recycle/`. Saves are atomic per section (`craft-atomic`: temp file, fsync, rename), run
from autosave about a second after the last change, so a crash loses at most that second and
never corrupts a file. Backups copy section files into the platform data folder at the configured
interval, keeping N copies. The web build keeps notebooks in memory and downloads/uploads
archives.

### 4.4 Text layout

The canvas lays text out with egui's text engine (`LayoutJob`: per-run size, family, italic
skew, underline/strike, highlight background, baseline shift for sub/superscript). Bold needs a
bold face: fonts come from `CRAFT_FONTS_DIR` (embedded) or the system's font folders; without
either, bold is drawn as a faux-bold double stroke. Visual-line caret moves (Home/End/Up/Down
on wrapped lines) are computed by the UI from the laid-out galley and passed to the caret
commands as an explicit target; headless callers get paragraph-level fallbacks.

### 4.5 Search

`notecraft-search` keeps an inverted index (token → postings of page ids) updated per changed
page revision; queries are case-insensitive prefix matches, all words must match, quoted phrases
are verified against page text; scopes filter by notebook/section group/section/page. Locked
sections are never indexed.

### 4.6 Ribbon from the catalog

The UI embeds `docs/parity-catalog.md` and builds the ribbon from its `R.*` rows (tab → group →
control in catalog order, kind → widget), plus a small table of which controls are large. A row
whose command is not registered is drawn **disabled** with the reason in its tooltip. The ribbon
therefore always has the reference's complete structure, and wiring a command lights its control
up with no UI work.

### 4.7 Time

The engine never reads the clock itself (wasm traps on `SystemTime::now`): the host passes "now"
(milliseconds since the Unix epoch) and the local UTC offset. Dates are formatted by the engine
from civil-date arithmetic (no time-zone database needed beyond the host's offset).

### 4.8 Tables (decision D2, 2026-10-15)

- **A table is its own page object**, `PageObject::Table { id, pos, columns: [width pt], rows:
  [{ id, cells: [{ id, paragraphs, shading }] }], borders, header_row }`, positioned and moved
  like a note container. In the reference a table is an outline element inside a container
  ([MS-ONE] `jcidTableNode` as content of `jcidOutlineElementNode`); keeping it beside the
  containers leaves every text command untouched. **M3 import:** a container holding a table is
  split into container, table, container, stacked at the positions the layout gives them
  (containment is the only thing lost; rows, cells, column widths, borders and shading map 1:1
  to `RowCount`/`ColumnCount`, `TableColumnWidths`, `TableBordersVisible`, `CellShadingColor`).
- **Cells are text blocks.** `Block::Cell(cell id)` joins `Title` and `Outline(id)`; a cell holds
  a flat paragraph list with levels like a container, so typing, formatting, find and the caret
  commands work in cells unchanged. A drag across cells selects a rectangle
  (`PageSel::Cells { table, rows, cols }`); paragraph and character commands apply to every
  selected cell.
- **Columns have fixed widths** (default 96 pt, unverified), changed by dragging a border; rows
  are as tall as their tallest cell. No merged cells (Merge/Split stay disabled).
- **Limits** against hostile input: 5,000 rows, 64 columns, 50,000 cells, widths 18–2,000 pt;
  `Table::normalize` repairs ragged rows, empty cells and bad widths on load.
- **Keys:** Tab/Shift+Tab move between cells (the last cell adds a row, or a column in a one-row
  table), Enter adds a row (on an empty last row it ends the table), Alt+Enter a paragraph in the
  cell, Ctrl+Enter a row below; arrows cross cell edges.
- **Format:** `.ncraft` version 2 (version 1 files open unchanged; older builds refuse version 2).
  Loaded pages are now normalised in full (`Section::normalize_pages`), tables included.
- **Insert position:** below the caret's container (the UI passes the laid-out bottom; the
  engine alone estimates 20 pt per line), or where the user clicked, or below everything.

### 4.9 Pictures and attachments (decision D3, 2026-10-19)

- **Two new page objects**, beside containers and tables (the D2 precedent):
  `PageObject::Image(Image { id, pos, size, blob, pixels, quarter_turns, flip_h, flip_v,
  alt_text, background })` and `PageObject::Attachment(Attachment { id, pos, name, blob,
  alt_text })`. `size` is the drawn size in points (after rotation), `pixels` the decoded size
  of the stored picture. In the reference a picture or an embedded file is either directly on
  the page or the content of an outline element ([MS-ONE] `jcidImageNode`,
  `jcidEmbeddedFileNode`); **M3 import** splits a container around an inline one exactly as for
  tables (only containment is lost). The size, alt text, background flag, file name and file
  data map to the node's picture size, alt text, background, embedded-file name and file data
  store reference (property names to be pinned against [MS-ONE] in M3).
- **Bytes live in a `Blob { id, kind, bytes: Arc<[u8]> }`.** The id is a fresh 128-bit `Id`
  minted on insert: it plays the role of the reference's file-data-store GUID and keys the zip
  entry. Bytes are shared by `Arc`, so undo snapshots, copy/paste and duplicated pages never copy
  them. `section.json` stores only the blob's id, kind and length; the bytes are zip entries
  `media/<id>` beside it, **stored uncompressed** (pictures are already compressed and autosave
  rewrites the section), one entry per distinct id. Sections, the recycle bin (a deleted page
  keeps its pictures) and notebook archives all carry their media. On load a missing, oversize
  or mismatched entry leaves that object with empty bytes, drawn as a "can't show this picture"
  placeholder: one damaged picture never costs the section.
- **Format 3.** `.ncraft` version 3; files of versions 1 and 2 open unchanged (tests keep a
  byte-exact v1 file and a v2 table file); older builds refuse version 3 rather than drop media.
- **Limits against hostile input:** a picture file ≤ 64 MiB and ≤ 16,384 px per side and
  64 Mpx; an attachment ≤ 256 MiB; ≤ 1 GiB of media per section file; ≤ 10,000 objects per page;
  drawn sizes 4–16,000 pt; alt text ≤ 4,096 characters; attachment names are single safe file
  names ≤ 255 bytes. The `codecs` crate decodes with these caps (an error, never a panic or an
  unbounded allocation); the canvas downscales textures to ≤ 4,096 px per side and keeps a
  bounded, least-recently-used texture cache keyed by blob id.
- **Insert:** a picture comes in at its pixel size at 96 dpi (1 px = 0.75 pt), scaled down to at
  most 468 pt wide (unverified), at the same place a table goes (§4.8). Rotation and flips are
  view properties applied when drawing (the stored bytes never change); 90° turns swap the drawn
  width and height. Free 45° rotation and cropping stay disabled (P2, not in the M365 context
  menu's P0/P1).
- **Attachments** draw as an icon card with the file name. Open writes the bytes to a private
  temporary folder and asks the host to open the file with the system's handler; Save As writes
  them to a path the host's file dialog returns. The web build disables both until downloads
  exist.
- **Later:** a file printout (M2/M3) is an attachment plus one picture per printed page linked to
  it, as in the reference; the `render` crate draws pictures from the same blobs for page export
  and printing.

### 4.10 Ink (decision D4, 2026-10-20)

- **One stroke is one page object**, `PageObject::Ink(Ink { id, pos, points: [[x, y,
  pressure]], color, width, tool })`, beside containers, tables and pictures (the D2/D3
  precedent). Points are in page points relative to `pos` (the top-left of the points' box), so
  moving a stroke is one assignment; `tool` is `Pen` or `Highlighter`; `width` is the nominal tip
  width in points; pressure is 0–1 per point (0.5 when the device reports none). Strokes being
  objects means the lasso, Delete, Cut/Copy/Paste, the arrow keys, Order and the Rotate commands
  work on them through the existing object selection (`PageSel::Objects`), and Undo removes one
  stroke at a time. In the reference ink is `jcidInkNode` content holding `InkData` with one or
  more strokes ([MS-ONE]); **M3 import** makes one object per stroke (only the grouping is lost;
  path, pressure, colour, width and the highlighter flag map to the stroke properties, names to
  be pinned against [MS-ONE] in M3), export groups the strokes of a page into one ink node.
- **Width follows pressure** for pens: drawn width = `width × (0.4 + 1.2 × pressure)`, so the
  constant 0.5 of a mouse draws exactly `width` (curve unverified). Highlighters ignore pressure
  and draw translucent. Pressure comes from egui's touch force where winit reports it; mouse
  strokes get 0.5 (PhotoCraft's XInput2 `tablet` crate is the path to real tablets on X11).
- **Geometry** lives in `notecraft-ink-geom` (L0: simplification, distances, polygon tests,
  circle cuts, outlines for drawing) and the stroke operations in `notecraft-ink` (L2: building
  a stroke from samples, partial and whole-stroke erasing, lasso picks, 45°/90° turns, flips).
  A finished stroke is simplified (Ramer–Douglas–Peucker, 0.2 pt) before it is stored.
- **Erasers:** the Stroke Eraser removes every stroke it touches; Small, Medium and Large
  (radius 2.5, 6 and 12 pt, unverified) cut strokes and keep the pieces outside the eraser
  (each piece a new stroke). One drag is one undo step (the commands coalesce by gesture).
- **Lasso:** a stroke is picked when at least three quarters of its points are inside the loop;
  any other object when the centre of its box is (unverified).
- **Tools** (Type, Lasso Select, Panning Hand, Eraser, the pens) and the pen gallery are session
  state (`Session::draw`), not notebook data: choosing a tool never adds an undo step. The
  default pens are black, red and blue pens of 0.5 mm and yellow and green highlighters of
  5 mm (unverified); Add Pen appends one (at most 32).
- **Format 4.** `.ncraft` version 4; files of versions 1–3 open unchanged; older builds refuse
  version 4 rather than drop ink.
- **Limits against hostile input:** ≤ 20,000 points per stroke (longer input is simplified,
  then cut), coordinates through `geom::finite`, pressure clamped to 0–1 (NaN → 0.5), widths
  0.1–72 pt, a stroke without points is dropped on load, and strokes count against the
  10,000-objects-per-page cap of D3.
- **Later:** the ruler, shapes (Ink to Shape), Ink to Text/Math (recognition), Ink Replay,
  touch-only drawing, resizing ink by handles, the `render` crate drawing ink for export.

## 5. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| `.one` revision store complexity ([MS-ONESTORE] is large; object spaces, revision manifests, file data stores) | M3 slips | Read-only first, per-construct fidelity numbers, synthetic fixtures produced by our own writer, fuzzing |
| Fonts: Calibri (the reference's default) is proprietary | Layout differs | Metric-compatible Carlito when present; Calibri named in the model so files round-trip |
| egui text engine limits (no OpenType features, no bold synthesis) | Rich-text fidelity | Fonts from craft-fonts/system; own layout in `notecraft-text` if needed later |
| Ink on Linux: winit exposes no pressure/tilt on Wayland tablet-v2 / X11 XInput2 | M2 ink fidelity | Isolated tablet crate like PhotoCraft's; document gaps |
| Pure-Rust audio/video stack | M2 recording | Audio first (cpal is C-free on Linux only via ALSA bindings → check licences/build); video likely a documented gap |
| Undocumented behaviour (exact list sequences, rule-line spacings, default palettes) | Parity claims | Mark `unverified`, list in parity.md, never count them as verified |
| Memory on the dev machine (15 GB, often ~3 GB free) | Slow builds | One target dir, modest parallelism, no parallel agents building at once |

## 6. Decisions

| # | Date | Decision | Overrides | Why |
|---|---|---|---|---|
| D1 | 2026-10-15 | The visual model is the current OneNote for Microsoft 365 desktop app (the 2024 UI): neutral title bar with a centred Search box; a tab row whose active tab is semibold with an accent underline; the ribbon in a floating rounded card, **Simplified (one row) by default** with a ▾ chevron to the **Classic** ribbon (both in the modern style, the choice remembered); navigation on the left (a tree of every notebook with coloured section bars, + New section, Quick notes pinned at the bottom; the page column with Add page and sort); **no section tabs above the page**; panes as cards. Surfaces that only exist in the 2016 layout become `out-of-scope` rows that name their replacement. | The window layout of the original brief (OneNote 2016-style: section tabs across the top, page list on the right) | The user's directive: like the other ArtCraft apps follow the latest Office. The brief is kept verbatim; this table records the override. The command set, menu paths, shortcuts and behaviour mandate are unchanged. |
| D2 | 2026-10-15 | Tables are their own page object (`PageObject::Table`) with cells as text blocks (`Block::Cell`) and fixed column widths; `.ncraft` version 2 (§4.8). | — (the plan named `PageObject::Table`; the containment detail was open) | The simplest model compatible with M3: [MS-ONE] tables map 1:1 except that a table inside a container becomes a neighbour of it; no existing text command changes. |
| D3 | 2026-10-19 | Pictures and attachments are page objects (`PageObject::Image`, `PageObject::Attachment`) whose bytes are `Arc`-shared blobs keyed by a 128-bit id and stored as uncompressed `media/<id>` entries in the section zip; `.ncraft` version 3; explicit size, pixel, byte and object-count limits (§4.9). | — (the plan named `media/` in the section zip; the object shape and limits were open) | Undo and copy/paste stay pointer copies; blob ids map to [MS-ONE]'s file-data-store GUIDs; a damaged picture degrades to a placeholder instead of failing the section; printouts and export reuse the same blobs. |
| D4 | 2026-10-20 | Ink strokes are page objects, one stroke per object (`PageObject::Ink`, points relative to the object's position with per-point pressure, colour, width, pen or highlighter); `.ncraft` version 4; geometry in `ink-geom` (L0), stroke operations in `ink` (L2); tools are session state (§4.10). | — (the plan named the crates; the object shape was open) | Every object command (select, lasso, delete, clipboard, order, rotate, move, undo) works on ink unchanged; [MS-ONE] ink nodes map stroke by stroke, losing only their grouping. |

## 7. Assumptions

- The Microsoft 365 desktop app's ribbon equals OneNote 2019's control inventory (FLUENT19) plus
  the Help tab and a few newer Draw/View controls; differences are marked per row.
- "Sync" with Microsoft services is out of scope (brief §4); rows that need OneDrive, Outlook or
  the Office Store are `out-of-scope` but still drawn (disabled) where they have a placement.
- Local notebooks are folders; a notebook may also be opened read-only from a `.onepkg` (M3).
- Linux is the first-class platform; macOS/Windows/wasm must build and run the same UI.
- No Rust toolchain was preinstalled on the dev machine; Rust 1.99 stable was installed with
  rustup (`~/.cargo`), and builds use a single target directory.
