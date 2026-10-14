# NoteCraft plan

> **Last reviewed:** 2026-10-14 · **Last updated:** 2026-10-14 · **Change:** major (first version) · **Target:** Microsoft OneNote for Microsoft 365, desktop for Windows

How NoteCraft gets from nothing to a OneNote replacement a typical user could switch to. The
inventory of what "OneNote" means is [`parity-catalog.md`](parity-catalog.md) (1,291 rows); the
numbers are in [`parity.md`](parity.md); the sibling conventions we follow are in
[`structure-audit.md`](structure-audit.md).

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
| M2 | Insert and Draw | `R.Insert.*`, `R.Draw.*`, `CT.TableLayout.*`, `CT.Equation.*`, `CT.AudioVideo.*`, picture/attachment/ink `CM.*`, `BEH.Ink/Table/Paste.*` | Tables with all Table Layout tools, pictures, attachments, printouts, date/time, templates, rule lines/grid, symbols, ink with pressure, lasso, eraser modes |
| M3 | Interop | `FMT.*`, `BS.Export.*`, `BS.Open.OpenSection` | `.one`/`.onetoc2`/`.onepkg` import with measured fidelity per construct; `.one` export where the specs allow; PDF/DOCX/HTML/Markdown exports; Markdown/HTML/text/ENEX/PDF/image imports |
| M4 | History, Review, View, Help, power features | `R.History.*`, `R.Review.*`, `R.View.*`, `R.Help.*`, `OPT.*`, `UI.Dialogs.Customize*`, `UI.Ribbon.Keytips`, `BEH.Password/Versions/History.*` | Page versions with diff, spelling, password-protected sections (XChaCha20-Poly1305 + Argon2id), full Options, customizable ribbon and QAT, keytips, AccessKit, en + it localization, optional folder/WebDAV sync, WASM plug-ins, MCP tools |

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

## 6. Assumptions

- The Microsoft 365 desktop app's ribbon equals OneNote 2019's control inventory (FLUENT19) plus
  the Help tab and a few newer Draw/View controls; differences are marked per row.
- "Sync" with Microsoft services is out of scope (brief §4); rows that need OneDrive, Outlook or
  the Office Store are `out-of-scope` but still drawn (disabled) where they have a placement.
- Local notebooks are folders; a notebook may also be opened read-only from a `.onepkg` (M3).
- Linux is the first-class platform; macOS/Windows/wasm must build and run the same UI.
- No Rust toolchain was preinstalled on the dev machine; Rust 1.99 stable was installed with
  rustup (`~/.cargo`), and builds use a single target directory.
