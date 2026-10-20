# NoteCraft roadmap: milestones and what's next

> **Last reviewed:** 2026-10-20 · **Last updated:** 2026-10-20 · **Change:** minor (M2: ink; decision D4) · **Target:** Microsoft OneNote for Microsoft 365, desktop for Windows (2024 UI)

Forward-looking plan; details and rationale are in [`plan.md`](plan.md).

## Current focus

1. M2: Insert and Draw (tables, pictures, files, ink), the rich clipboard, the canvas details
   and the missing dialogs (see [`gaps.md`](gaps.md)), born in the 2024 style of M1.5
   ([`ui-parity.md`](ui-parity.md)); tables get their contextual tab in both ribbon layouts.
2. Keep the two parity numbers honest: rows move to `wired` only with a test, to `verified` only
   with a `// parity:` test that pins the documented behaviour.

## Milestones

| # | Milestone | State |
|---|---|---|
| M1 | Shell and core editing (layout, hierarchy, canvas + outlines, Home tab, tags, search, persistence) | done 2026-10-15 (tag `m1-core`): wiring 28.1%, behaviour 8.4% |
| M1.5 | Modern UI: the 2024 look (decision D1) | done 2026-10-15 (tag `m1.5-modern-ui`): wiring 28.7%, behaviour 8.3% |
| M2 | Insert and Draw tabs | in progress on `m2/insert-draw`: paragraph handle, growing containers and page, tables with the Table Layout tab, the mini toolbar, context menus, Paste Options, Dialogs (Insert Table, Link search, Move/Copy, Recent Notebooks, notebook list, Symbol, Options with Office Theme), pictures and file attachments (decision D3, `.ncraft` format 3), the rich clipboard (HTML and pictures) and drag and drop of files, ink (decision D4, `.ncraft` format 4: pens, highlighters, erasers, lasso, the Draw tab), file printouts (PDF through `hayro`) done; screen clipping, audio to go |
| M3 | Interop (`.one`/`.onetoc2`/`.onepkg`, exports, imports) | not started |
| M4 | History, Review, View, Help and power features | not started |
| M5 | Release: packaging, other platforms, performance budgets, 50 real workflows, user guide | not started |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-20 | minor | M2 in progress: ink and the Draw tab (decision D4, format 4) |
| 2026-10-20 | minor | M2 in progress: pictures, attachments, rich clipboard, drag and drop (decision D3) |
| 2026-10-16 | minor | M2 in progress: Dialogs (Insert Table, Link search, Move/Copy, Recent Notebooks, notebook list, Symbol, Options), `KB.Table.DeleteEmptyRow` |
| 2026-10-16 | minor | M2 in progress: mini toolbar, context menus, Paste Options |
| 2026-10-15 | minor | M2 in progress: canvas structure and tables |
| 2026-10-15 | major | M1.5 done; focus moves to M2 |
| 2026-10-15 | major | Decision D1 (the 2024 UI): M1.5 inserted before M2; M5 added |
| 2026-10-15 | major | M1 done; focus moves to M2 |
| 2026-10-14 | major | First version |
