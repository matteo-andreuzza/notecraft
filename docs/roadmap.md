# NoteCraft roadmap: milestones and what's next

> **Last reviewed:** 2026-10-15 · **Last updated:** 2026-10-15 · **Change:** major (M1.5 inserted, decision D1) · **Target:** Microsoft OneNote for Microsoft 365, desktop for Windows (2024 UI)

Forward-looking plan; details and rationale are in [`plan.md`](plan.md).

## Current focus

1. M1.5: the 2024 look (decision D1 in [`plan.md`](plan.md)): theme tokens and UI font, title bar
   with Search, the ribbon card in Simplified (default) and Classic modes, left navigation with the
   notebook tree and the page column, panes as cards, icons; specified in [`ui-parity.md`](ui-parity.md).
2. Then M2: Insert and Draw (tables, pictures, files, ink), the rich clipboard, the canvas details
   and the missing dialogs (see [`gaps.md`](gaps.md)), born in the new style.
3. Keep the two parity numbers honest: rows move to `wired` only with a test, to `verified` only
   with a `// parity:` test that pins the documented behaviour.

## Milestones

| # | Milestone | State |
|---|---|---|
| M1 | Shell and core editing (layout, hierarchy, canvas + outlines, Home tab, tags, search, persistence) | done 2026-10-15 (tag `m1-core`): wiring 28.1%, behaviour 8.4% |
| M1.5 | Modern UI: the 2024 look (decision D1) | in progress |
| M2 | Insert and Draw tabs | not started |
| M3 | Interop (`.one`/`.onetoc2`/`.onepkg`, exports, imports) | not started |
| M4 | History, Review, View, Help and power features | not started |
| M5 | Release: packaging, other platforms, performance budgets, 50 real workflows, user guide | not started |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-15 | major | Decision D1 (the 2024 UI): M1.5 inserted before M2; M5 added |
| 2026-10-15 | major | M1 done; focus moves to M2 |
| 2026-10-14 | major | First version |
