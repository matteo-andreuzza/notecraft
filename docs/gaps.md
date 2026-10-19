# Gaps: what NoteCraft lacks, ranked

> **Last reviewed:** 2026-10-20 · **Last updated:** 2026-10-20 · **Change:** minor (M2: pictures, attachments, rich clipboard and drag and drop narrow gaps 2 and 3)

The ranked work list. The exhaustive per-control list is the generated
[`parity-checklist.md`](parity-checklist.md) (missing P0/P1 rows); this page ranks the big pieces.

| # | Gap | Why it matters | Milestone |
|---|---|---|---|
| 1 | `.one`/`.onetoc2`/`.onepkg` import (then export) | Switching needs the user's existing notebooks | M3 |
| 2 | Insert and Draw: ink, equations, file printouts, screen clipping, audio; pictures lack free (45°) rotation, cropping, OCR and links, and draw under text whatever their order (tables, pictures and file attachments with their context menus are in) | Second most used surface; handwritten pages can't be written yet | M2 |
| 3 | Clipboard and drag and drop leftovers: copied files (a file manager's file list) don't paste as attachments; dropping on the web build; Paste: Picture (`edit.pastePicture`); HTML pictures (`<img>`) are dropped | Small frictions moving content in | M2 |
| 4 | Canvas details: the mini toolbar, Paste Options, and the right-click menus on text, containers and paragraph handles are all in now (including the handle, growing containers and the growing page); still missing are a few items those menus leave disabled for later gaps (ink, OCR, linked notes, picture rotation, spelling, translate) and `link.open`'s missing `enabled` predicate | Day-to-day editing feel (catalog `UI.Canvas.*`, `CM.Text/NoteContainer/ParagraphHandle.*`) | M2 |
| 5 | The web build keeps nothing between visits (no browser storage yet), can't download exports, and has no bold (no font files) | The web app is a demo until notebooks persist | M2 |
| 6 | File dialogs block the window while open (`rfd` blocking); the window size and position are not remembered | Desktop polish | M2 |
| 7 | Leftovers of the 2024 look: about 135 controls keep NoteCraft's older drawings (not yet in the line style), the font and size boxes are egui's default combo, the ribbon can't auto-hide, the Quick Access Toolbar can't be customized, window buttons are the compositor's ([`ui-parity.md`](ui-parity.md)) | Polish of the new look | M2–M4 |
| 8 | IME preedit is not drawn in place; not yet tried with a screen reader | Accessibility and comfort | M4 |
| 9 | History, Review, View depth: page versions, authors, passwords, research, translation, printing, sync between devices | Power users and teams | M4 |

Smaller known shortfalls:

- Autosave rewrites a whole section file, media included: a section with hundreds of megabytes
  of pictures writes them all again a second after each change (D3 keeps entries uncompressed so
  this is a copy, not a recompression; reusing unchanged entries is a later optimisation).
- The Tell Me box and the ribbon's ScreenTips work but have no UI test yet, so their rows stay
  `missing`.
- `--automation-read-root`/`--automation-write-root` confine the headless MCP server only; a
  running desktop app reached through `--control` is not confined.
- Tables: no Tab-after-text table creation (`BEH.Table.TabToCreate`), no merged cells, no Sort…
  dialog; a table is a page object beside containers rather than inside one (decision D2). Cell
  ranges copy as tab-separated text (whole tables copy as tables, and as HTML tables for other
  applications).
- Ctrl+Shift+Minus for subscript (the M365 shortcut table) collides with another row in the same
  table; NoteCraft uses Ctrl+= (unverified).

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-16 | minor | M2: Dialogs done — closes gap 5 (Insert Table, Link search, Move/Copy as a flat searchable list, Recent Notebooks, notebook list, Symbol, Office Theme); `KB.Table.DeleteEmptyRow` |
| 2026-10-16 | minor | M2: mini toolbar, context menus, Paste Options done (gap 4 nearly closed) |
| 2026-10-15 | minor | M2: tables and the paragraph handle done |
| 2026-10-15 | major | Re-ranked after M1 |
| 2026-10-14 | major | First version |
