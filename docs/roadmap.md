# NoteCraft roadmap: milestones and what's next

> **Last reviewed:** 2026-10-24 · **Last updated:** 2026-10-24 · **Change:** major (M2 done; decisions D5–D11) · **Target:** Microsoft OneNote for Microsoft 365, desktop for Windows (2024 UI)

Forward-looking plan; details and rationale are in [`plan.md`](plan.md).

## Current focus

1. M3: Interop — read `.one`, `.onetoc2` and `.onepkg` with measured fidelity per construct,
   then the exports and imports ([`plan.md`](plan.md) §2). Audio, Ink to Text/Math and the M2
   leftovers in [`gaps.md`](gaps.md) wait for later increments.
2. Keep the two parity numbers honest: rows move to `wired` only with a test, to `verified` only
   with a `// parity:` test that pins the documented behaviour.

## Milestones

| # | Milestone | State |
|---|---|---|
| M1 | Shell and core editing (layout, hierarchy, canvas + outlines, Home tab, tags, search, persistence) | done 2026-10-15 (tag `m1-core`): wiring 28.1%, behaviour 8.4% |
| M1.5 | Modern UI: the 2024 look (decision D1) | done 2026-10-15 (tag `m1.5-modern-ui`): wiring 28.7%, behaviour 8.3% |
| M2 | Insert and Draw tabs | done 2026-10-24 (tag `m2-insert-draw`): wiring 49.5%, behaviour 13.1% — tables with merged cells and Sort…, pictures, attachments, printouts, screen clipping, the rich clipboard, drag and drop, ink with the Draw tab (pens, erasers, lasso, shapes, ruler, Snap to Grid), tags on ink, page templates, equations, the mini toolbar, context menus, Paste Options and the dialogs; `.ncraft` format 5; audio and video left for later |
| M3 | Interop (`.one`/`.onetoc2`/`.onepkg`, exports, imports) | not started |
| M4 | History, Review, View, Help and power features | not started |
| M5 | Release: packaging, other platforms, performance budgets, 50 real workflows, user guide | not started |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-24 | major | M2 done (tag `m2-insert-draw`): merged cells (D9), page templates (D10), equations and crate `math` (D11), Snap to Grid and Draw with Touch (D8), the ruler (D7); focus moves to M3 |
| 2026-10-20 | minor | M2 in progress: ink and the Draw tab (decision D4, format 4) |
| 2026-10-20 | minor | M2 in progress: pictures, attachments, rich clipboard, drag and drop (decision D3) |
| 2026-10-16 | minor | M2 in progress: Dialogs (Insert Table, Link search, Move/Copy, Recent Notebooks, notebook list, Symbol, Options), `KB.Table.DeleteEmptyRow` |
| 2026-10-16 | minor | M2 in progress: mini toolbar, context menus, Paste Options |
| 2026-10-15 | minor | M2 in progress: canvas structure and tables |
| 2026-10-15 | major | M1.5 done; focus moves to M2 |
| 2026-10-15 | major | Decision D1 (the 2024 UI): M1.5 inserted before M2; M5 added |
| 2026-10-15 | major | M1 done; focus moves to M2 |
| 2026-10-14 | major | First version |
