# NoteCraft plan

> **Last reviewed:** 2026-10-20 · **Last updated:** 2026-10-20 · **Change:** major (decision D4: ink) · **Target:** Microsoft OneNote for Microsoft 365, desktop for Windows (the 2024 UI)

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
   tables); dropping files on the page; file printouts (PDF pages through `hayro` 0.7.1 from
   crates.io, the user's choice of 2026-10-20). Screen clipping through the XDG Screenshot
   portal (Linux; a background thread, the result polled by the UI). Left: screen clipping on
   Windows/macOS, 45° rotation of pictures, cropping, copying text from printout pages.
4. **Ink (decision D4 / §4.10; done):** crates `ink-geom` and `ink`, `PageObject::Ink` (`.ncraft`
   format 4), the Draw tab's tools (Type, Lasso Select, Panning Hand, the four erasers, the pen
   gallery, Add Pen, Color & Thickness), strokes with pressure from egui touch force, the ink
   and lasso context menus, 45°/90° turns and flips of ink, the ruler (D7 / §4.12), Snap to
   Grid, Draw with Touch and two fingers on the ruler (D8 / §4.13). Left: recognition (Ink to
   Text/Math), Ink Replay, tablet pressure on Linux (PhotoCraft's XInput2 `tablet` crate), and
   crate `render` (CPU oracle for thumbnails and export).
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
- **Tab after text (decision D6, 2026-10-21).** Tab with the caret at the end of a paragraph's
  text in a container turns that paragraph into a one-row, two-column table: its text (and
  formatting and tags) in the first cell, the caret in the empty second one. Because a table
  is its own object (D2), the table takes the paragraph's place: the container keeps the
  paragraphs before it, those after it move to a new container below the table, and a container
  left with nothing goes. **No "creating a table" state:** the next Tabs add columns and Enter
  adds rows through the table's own keys (`KB.Table.NewColumnTab`, `KB.Table.NewRowEnter`),
  which already behave so on a one-row table. A caret inside the text still types a tab
  character, a list item still indents, and an empty paragraph still indents (unverified: the
  reference's behaviour for list items and for Tab in the middle of text). One undo step.

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
- **Stacking (2026-10-21):** a picture or attachment is drawn over the text of the containers
  and tables it overlaps, and takes clicks there, only when it comes after every one of them in
  the page's order (Order › Bring to Front, or moved onto older text); otherwise it stays under
  the text, which stays readable and clickable. Two layers approximate the full order (a
  picture between two overlapping containers is drawn over both; unverified).
- **Insert:** a picture comes in at its pixel size at 96 dpi (1 px = 0.75 pt), scaled down to at
  most 468 pt wide (unverified), at the same place a table goes (§4.8). Rotation and flips are
  view properties applied when drawing (the stored bytes never change); 90° turns swap the drawn
  width and height. Free 45° rotation and cropping stay disabled (P2, not in the M365 context
  menu's P0/P1).
- **Attachments** draw as an icon card with the file name. Open writes the bytes to a private
  temporary folder and asks the host to open the file with the system's handler; Save As writes
  them to a path the host's file dialog returns. The web build disables both until downloads
  exist.
- **File printouts (done 2026-10-20):** an attachment plus one picture per printed page, each
  linked to it (`Image::printout`), stacked below it at the page's print size. PDF pages are
  rendered by `hayro` 0.7 from crates.io (pure Rust, PdfCraft's renderer; `codecs::pdf`) at
  108 dpi, at most 200 pages, 3,000 px a side and 8 Mpx a page, under the codecs' panic guard;
  a picture file prints as itself; other formats are refused (no printer driver). Insert
  Printout on an attachment replaces its earlier pages; Delete Printout Pages and Open Original
  complete the menus. Rendering runs inside the command (a large PDF blocks for a moment; async
  later). **Later:** the `render` crate draws pictures from the same blobs for page export and
  printing; text from printout pages (Copy Text) waits for OCR or a PDF text layer.

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
- **Later:** Ink to Shape, Ink to Text/Math (recognition), Ink Replay,
  touch-only drawing, resizing ink by handles, the `render` crate drawing ink for export.

### 4.11 Tags on ink (decision D5, 2026-10-21)

- **A stroke carries tags** like a paragraph: `Ink.tags: Vec<TagUse>` (the same name, symbol
  and checked snapshot). The field is optional in `section.json` (absent when empty), so
  `.ncraft` stays at version 4 and older files open unchanged; at most 32 tags per stroke, one
  per tag key (hostile files are cut there).
- **A drawing is several strokes, but a tag is applied once.** With strokes one object each
  (D4), the tag commands (the gallery, Ctrl+1…9, Remove Tag, Check/Clear All, the checkbox
  click) act on the selected strokes as one drawing: the drawing *has* a tag when any of its
  strokes has it; applying puts the tag on the **leftmost** selected stroke (then the topmost)
  unless one already has it; removing, checking and clearing touch every selected stroke. So
  Find Tags lists a tagged drawing once, not once per stroke. A selection that mixes containers
  and ink tags both.
- **Placement:** the icons sit in a row just left of the tagged stroke's box, level with its
  top; a checkbox there toggles on click. Find Tags and summary pages show a tagged stroke as
  "Ink drawing", and clicking it in the pane selects the stroke (placement and wording
  unverified).
- Pictures and attachments take no tags (their reference menus have no Tags).
- **M3:** [MS-ONE] note tags hang off the ink node; import puts them on the node's leftmost
  stroke, export on the node that groups the page's strokes.

### 4.12 The ruler (decision D7, 2026-10-23)

- **Session state, not notebook data.** Draw › Ruler shows a straightedge lying over the page:
  `DrawState.ruler: Option<Ruler { center, angle }>` (`notecraft_ink::ruler`), in page points so
  it scrolls and zooms with the page. Showing, moving and turning it never add an undo step and
  nothing of it is saved, in the notebook or in the settings (a run starts without it).
- **Geometry:** 54 × 900 pt (longer than most windows are wide), centred where the page's view
  is when shown, turned `angle` degrees clockwise on screen (0–360); it reads its angle as a
  protractor does, 0–179° counterclockwise from horizontal. Millimetre ticks along the top edge,
  centimetres numbered (metric scale and look unverified).
- **Drawing along it:** a stroke whose first sample lies within 12 pt of a long edge (inside or
  outside) is laid along that edge: every sample moves onto the line half the pen's width outside
  it, within the ruler's length, keeping its pressure (`Ruler::snap`). The engine does it in
  `draw.stroke` (so headless clients get it; `"free": true` opts out) and the canvas shows the
  same while the stroke is drawn.
- **Moving and turning:** `draw.rulerPlace` (`NC.Draw.RulerPlace`) moves it to or by a point and
  turns it to or by an angle. On the canvas a drag that starts on the ruler moves it (with the
  pen, only away from its edges, where it draws instead); the mouse wheel over it turns it a
  degree a notch, up counterclockwise, and doesn't scroll the page (unverified).
- **Later:** snapping the angle to 0/45/90°, the erasers and the Shape tool against its edge,
  measuring. (Two-finger moving and turning: §4.13.)

### 4.13 Snap to Grid, Draw with Touch, two fingers on the ruler (decision D8, 2026-10-24)

- **Session settings, like the pens.** `DrawState.snap_to_grid` (off) and `DrawState.with_touch`
  (on) are switched by `draw.snapToGrid` and `draw.withTouch` (`{"on"?: bool}`), never undone,
  and remembered in the desktop settings with the pens (starting values unverified).
- **Snap to Grid** moves the two points a shape is drawn between onto the nearest grid
  crossing: the page's own grid when it shows one (View › Rule Lines › a grid), else 1/8 in
  (9 pt, the small grid's spacing), counted from the page's top-left corner where grid lines
  start. The engine does it in `draw.shape`; the canvas previews the same. Freehand strokes,
  moving and resizing don't snap (what the reference snaps beyond shapes is unverified).
- **Draw with Touch off:** a finger dragged with Pen, Eraser, Shape, Insert Space or Lasso
  moves the page instead (the Panning Hand's gesture); the mouse and pens keep drawing. egui
  doesn't say whether a touch is a pen or a finger, so a finger is a touch that reports no
  pressure (winit gives pressure for pens where the platform does; a finger with pressure would
  draw).
- **Two fingers on the ruler** (their midpoint on it) move it with them and turn it about that
  midpoint by the angle the fingers turn (egui's multi-touch rotation), without scrolling the
  page. The first finger's drag (egui-winit also sends it as the pointer) becomes a drag of the
  ruler, and a stroke, shape or lasso it had begun is dropped.

### 4.14 Merged cells (decision D9, 2026-10-24)

- **Model.** `TableCell` gains `row_span` and `col_span` (1 by default, omitted when 1). A
  merged cell covers the grid places to its right and below; the covered cells stay in their
  rows, empty and hidden, so every row still has one cell per column and every row/column
  command keeps its indices ([MS-ONE] keeps the full grid the same way). `Table::normalize`
  keeps spans ≥ 1, inside the table and apart (row by row, an earlier cell wins; a span that
  would reach a covered place stops short). `.ncraft` version 5; older files open unchanged.
- **Merge Cells** (`table.merge`) joins the selected rectangle — grown to whole merged cells it
  touches — into its top-left cell: the text of the non-empty cells in reading order, the
  others left empty. **Split Cells** (`table.split`) returns the merged cells in the selection
  to single cells, the text staying in the first (unverified: the reference may offer a rows ×
  columns split for an ordinary cell).
- **Everything else.** A caret in a merged cell selects all of its rows and columns, so Insert
  Below/Right go past it and Delete Rows/Columns take all of them; a row or column inserted
  inside a merged cell joins it; deleting a merged cell's own first row or column moves it, text
  included, to the first one left; Tab, Shift+Tab and the arrows count a merged cell once; Sort
  refuses a table with cells merged across rows (split them first). The canvas lays a merged
  cell over its columns' widths, a tall one stretches the last of its rows, and borders are
  drawn cell by cell. `notebook.inspect` gives `rowSpan`/`colSpan`, and `covered` for hidden
  places. HTML copied out keeps the grid but not the merge (a gap).

### 4.15 Page templates (decision D10, 2026-10-24)

- **NoteCraft's own templates.** No reference screenshots were available and the reference's
  templates are its content, so the built-in set is NoteCraft's: Academic (Lecture notes, Study
  plan), Blank (Blank, Lined, Small grid, Large grid), Business (Meeting notes, Project
  overview), Decorative (Sky, Sand, Mint page colours) and Planners (To-do list, Weekly
  planner), built in code from headings, To Do/Question tags, tables with a header row, rule
  lines and page colours (`template_cmds.rs`). Their look can be matched later from screenshots.
- **The user's own** ("My Templates"): Save current page as a template keeps the page's text,
  tables, ink and page setup under a name in the preferences (with the desktop settings);
  pictures and attachments are left out, since the settings file holds no media. A page from a
  template gets fresh ids (`ops::clone_page_fresh`); the template's title is kept when it has
  one, built-ins leave it empty.
- **Ids and defaults.** Templates are `builtin:<slug>` or `my:<name>`. A section can name a
  default template (`Section.default_template`, format 5); New Page, New Page Below and New
  Subpage in that section start from it (a deleted template falls back to a blank page). The
  last five used are remembered for Recently Used Templates.
- **Surfaces.** Insert › Page Templates opens the Templates pane (categories, Recently Used, My
  Templates with Delete, Always use a specific template, Save current page as a template with a
  Save As Template dialog). Commands: `insert.pageTemplates`, `page.newFromTemplate`,
  `insert.recentTemplate`, `template.list`, `template.setDefault`, `template.save`,
  `template.delete`. The Paper Size pane (View, M4) will offer the same Save link.

### 4.16 Equations (decision D11, 2026-10-24)

- **An equation is a paragraph** whose text is its linear format, UnicodeMath (`x=(−b±√(b^2−4ac))/2a`,
  the notation the reference also accepts when typing), marked `Paragraph::math` = Professional
  or Linear (`.ncraft` version 5). Every text command works on it unchanged: typing, the caret,
  selection, undo, copy and paste (a pasted equation stays one), search, tags. Enter never
  splits an equation: it starts a plain paragraph after it. Equations are not inline inside a
  line of text (the reference allows that): a whole-paragraph equation needs no inline objects
  in the text layout, which egui's galleys don't offer.
- **Crate `math` (L2, no dependencies)** parses the subset listed in its docs — fractions,
  scripts, radicals, large operators with limits, stretchy brackets, matrices, accents,
  function names, `\alpha`-style control words, `⬚` slots — and lays it out built up from a
  caller-supplied text measure; the large signs (∑ ∏ ∫ √, brackets, bars) are line paths, so no
  math font is needed. Input is capped (4,000 characters, nesting 48) and never panics.
- **Showing and editing.** Professional shows the equation built up; while the selection is in
  it, its linear text is edited in place in a light box (simplified: the reference edits built
  up). Linear always shows the text.
- **Surfaces.** Insert › Equation (Alt+=) with a gallery of built-in equations (NoteCraft's set
  of common results); the contextual Equation tab while the caret is in an equation: Equation,
  Professional, Linear, a Symbols grid and the eleven Structures galleries, each item a linear
  template with its first `⬚` selected so typing fills it; the equation's context menu (Cut,
  Copy, Paste Options, Professional, Linear; a right click selects the whole equation). Left
  for later: Ink Equation and Ink to Math, Normal Text, Equation Options, the structure-editing
  rows of the context menu, MathML in HTML.

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
| D5 | 2026-10-21 | Tags on ink: a stroke carries `tags` like a paragraph (optional field, format 4 unchanged); the tag commands treat the selected strokes as one drawing, putting a new tag on its leftmost stroke and removing or checking it on all (§4.11). | — | Strokes are separate objects (D4), so tagging each would list one drawing many times in Find Tags; one anchor stroke keeps one entry per tag and maps to [MS-ONE]'s tag on the ink node. |
| D6 | 2026-10-21 | Tab after a paragraph's text makes it a one-row, two-column table that takes the paragraph's place (the container splits around it); no session state, later Tabs and Enters use the table's own keys (§4.8). | — | Tables are separate objects (D2), so the paragraph must leave its container; the one-row table's existing Tab/Enter behaviour already covers "keep creating". |
| D11 | 2026-10-24 | Equations are paragraphs of UnicodeMath linear text (`Paragraph::math`), laid out built up by a new pure crate `math` with line-drawn large signs; edited as linear text while the caret is in them (§4.16). | The plan named a `math` crate; inline math was open | The user asked for equations in M2; whole-paragraph equations reuse every text command and need no inline objects in egui's text layout. |
| D10 | 2026-10-24 | Page templates: NoteCraft's own built-in set (five categories, built in code), the user's own saved in the preferences without pictures, a default template per section (`Section.default_template`), the Templates pane (§4.15). | The plan waited for reference screenshots | The user chose NoteCraft's own templates over waiting; the reference's templates are its content, so a clean-room set is needed anyway. |
| D9 | 2026-10-24 | Merged table cells: `row_span`/`col_span` on the top-left cell, covered cells kept in the grid empty and hidden; `table.merge`/`table.split`; `.ncraft` version 5 (§4.14). | Merging was left out of D2 ("needs a model change") | The full grid keeps every row/column command and the format's row/cell lists unchanged; [MS-ONE] stores tables the same way. The user approved the change. |
| D8 | 2026-10-24 | Snap to Grid puts shapes' end points on the page's grid (1/8 in without one); Draw with Touch off turns a pressureless touch into panning; two fingers on the ruler move and turn it (§4.13). | — | Settings only, no format change; snapping lives in the engine's `draw.shape`, so headless clients get it too. |
| D7 | 2026-10-23 | The Draw tab's ruler is session state (`DrawState.ruler`: centre and angle in page space), never saved or undone; a stroke that starts near one of its long edges is laid along it by the engine's `draw.stroke` (§4.12). | — | No document change and no format bump; the engine owns the snapping, so the canvas, the control channel and MCP draw the same lines. |

## 7. Assumptions

- The Microsoft 365 desktop app's ribbon equals OneNote 2019's control inventory (FLUENT19) plus
  the Help tab and a few newer Draw/View controls; differences are marked per row.
- "Sync" with Microsoft services is out of scope (brief §4); rows that need OneDrive, Outlook or
  the Office Store are `out-of-scope` but still drawn (disabled) where they have a placement.
- Local notebooks are folders; a notebook may also be opened read-only from a `.onepkg` (M3).
- Linux is the first-class platform; macOS/Windows/wasm must build and run the same UI.
- No Rust toolchain was preinstalled on the dev machine; Rust 1.99 stable was installed with
  rustup (`~/.cargo`), and builds use a single target directory.
