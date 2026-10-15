# Gaps: what NoteCraft lacks, ranked

> **Last reviewed:** 2026-10-15 · **Last updated:** 2026-10-15 · **Change:** major (after M1)

The ranked work list. The exhaustive per-control list is the generated
[`parity-checklist.md`](parity-checklist.md) (missing P0/P1 rows); this page ranks the big pieces.

| # | Gap | Why it matters | Milestone |
|---|---|---|---|
| 1 | `.one`/`.onetoc2`/`.onepkg` import (then export) | Switching needs the user's existing notebooks | M3 |
| 2 | Insert and Draw: tables, pictures, file attachments, ink, equations, and their context menus | Second most used surface; rich pages can't be written yet | M2 |
| 3 | System clipboard beyond plain text: HTML in and out, pictures, files; drag and drop of files and images onto the page | Copying between NoteCraft and a browser or office suite loses formatting | M2 |
| 4 | Canvas details: paragraph handle (drag/select), containers that grow with their text, the page growing right and down, mini toolbar, paste options, right-click menus on text and containers | Day-to-day editing feel (catalog `UI.Canvas.*`, `BEH.Canvas.*`, `CM.Text/NoteContainer/ParagraphHandle.*`) | M2 |
| 5 | Dialogs still missing or partial: Move or Copy Pages/Section (a tree, not just a command), Link (search pages), the full Options dialog, Recent Notebooks, notebook drop-down (Ctrl+G) | P0 rows in `UI.Dialogs.*`, `BS.Open.RecentNotebooks` | M2 |
| 6 | The web build keeps nothing between visits (no browser storage yet), can't download exports, and has no bold (no font files) | The web app is a demo until notebooks persist | M2 |
| 7 | File dialogs block the window while open (`rfd` blocking); the window size and position are not remembered | Desktop polish | M2 |
| 8 | Section tabs that don't fit are hidden instead of listed in an overflow menu (they stay reachable from the notebook pane) | Many-section notebooks | M2 |
| 9 | Themes: Colorful, White and Dark only (no Dark Gray, Black); IME preedit is not drawn in place; not yet tried with a screen reader | Accessibility and comfort | M4 |
| 10 | History, Review, View depth: page versions, authors, passwords, research, translation, printing, sync between devices | Power users and teams | M4 |

Smaller known shortfalls:

- Paste from other applications is plain text only (formatting survives copy and paste inside
  NoteCraft).
- The Tell Me box and the ribbon's ScreenTips work but have no UI test yet, so their rows stay
  `missing`.
- `--automation-read-root`/`--automation-write-root` confine the headless MCP server only; a
  running desktop app reached through `--control` is not confined.
- Ctrl+Shift+Minus for subscript (the M365 shortcut table) collides with another row in the same
  table; NoteCraft uses Ctrl+= (unverified).

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-15 | major | Re-ranked after M1 |
| 2026-10-14 | major | First version |
