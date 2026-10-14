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
