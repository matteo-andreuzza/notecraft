# Contributing

> **Last reviewed:** 2026-10-14 · **Last updated:** 2026-10-14 · **Change:** major (first version)

- **Language:** Rust only. No JavaScript or TypeScript. On the web, `wasm-bindgen` generates a
  small loader; never hand-write JS.
- **Licence:** contributions are MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`).
  Dependencies must be permissive too (`deny.toml`, checked by `cargo xtask deny`).
- **Clean room:** do not copy code, strings, icons, templates or any other asset from Microsoft
  OneNote or any proprietary software. Match behaviour and layout from public documentation
  (Microsoft support pages, the Open Specifications, the Fluent UI control-identifier lists) and
  user-visible behaviour only. Never decompile or inspect Microsoft binaries. Only use assets with
  permissive licences, keep their licence next to them, and list every asset in `ATTRIBUTION.md`.
  Fonts live in [storytold/craft-fonts](https://github.com/storytold/craft-fonts), never here.
- **Never crash:** non-test code must not panic (no `unwrap`/`expect`/`panic!`/`unreachable!`/
  `todo!`/`unimplemented!`, no `unsafe`). Errors go through `Result` and `?`; input-derived indices
  and sizes are checked; every crash fix ships with a regression test. See `AGENTS.md`.
- **Commands, not handlers:** new features are engine commands with tests; the UI calls them.
- **Catalog first:** every command and UI control points at a row of `docs/parity-catalog.md`;
  when you implement one, flip the row's status in the same commit and run `cargo xtask parity`.
- **Layering:** `cargo xtask layers` must pass. Register new crates in `xtask/src/layers.rs`.
- **Tests:** required for every change. Parsers need round-trip, malformed-input and fuzz/proptest
  coverage. Behaviour that matches documented OneNote behaviour gets a test carrying
  `// parity: <catalog ID>` and the row becomes `verified`.
- **Style:** `cargo fmt`; `cargo clippy -- -D warnings`. Match surrounding code. Comments explain
  *why*. Every public item has a `///` doc comment; every crate a `//!` doc saying its layer.
- **UI:** use `theme::Tokens` and the shared widgets. Verify visually with the offscreen
  screenshot example before submitting.
- **Commits:** small, focused, green. Subject `Area: what changed` (≤72 chars), with the milestone
  id where one applies (`M1 Engine: command registry and undo`); a body explaining why.

## Adding a command

1. **Find the catalog row.** Search `docs/parity-catalog.md` for the OneNote control, menu item or
   shortcut. Its `ID` is the command's `catalog` id, its `Path` the command's `menu`, its
   `Shortcut` the command's shortcut, and its `Command` column the command id to use. A command
   with no OneNote counterpart needs a catalog row first (prefix `NC.`).
2. **Algorithm** goes in the lowest crate that fits (`text` for editing, `search`, `doc` for
   structure edits), with unit tests. It is deterministic and never panics on hostile input.
3. **Command** goes in an engine module (`crates/engine/src/<area>_cmds.rs`) exposing `specs()`,
   registered in `commands.rs`. Fill in:
   - `id`, `catalog`, `label`, `menu` and OneNote's default `shortcut` (`Mod+B`; `Mod` is Ctrl,
     or ⌘ on macOS; several keys separated by ` / `),
   - a params doc string such as `{"level":1-9}` (agents read it through `commands`),
   - an `enabled` predicate returning the reason it is disabled (greys the control out),
   - `run`, which returns a JSON result and records exactly **one** undo step (typing and drags
     coalesce).
4. **Tests** in the module: behaviour, undo/redo, disabled states, bad params (`Err`, never a
   panic). Pin documented OneNote behaviour with `// parity: <ID>`.
5. **UI:** the ribbon control, context-menu item or shortcut dispatches the command id; nothing
   else.
6. Run `cargo xtask parity` and commit the updated `docs/parity-checklist.md` and `docs/parity.md`.
