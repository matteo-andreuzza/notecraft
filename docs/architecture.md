# Architecture

> **Last reviewed:** 2026-10-14 · **Last updated:** 2026-10-14 · **Change:** major (first version)

## Layers

NoteCraft is a layered Cargo workspace; `cargo xtask layers` enforces that a crate depends only on
strictly lower layers (plus the intra-layer chains in `xtask/src/layers.rs`) and that nothing below
L6 uses a UI toolkit (egui, eframe, winit, rfd, arboard).

| Layer | Crates | Role | Why here |
|---|---|---|---|
| L0 | `util`, `geom`, `color`, `ink-geom`, `codecs` (standalone) | Foundation types: page-space geometry in points, colours, ink geometry, codecs | No knowledge of notes; reusable and trivially wasm-safe |
| L1 | `doc` | The document model: notebook → section group → section → page → page objects (outlines with outline elements, ink, images, tables, files…), rich-text runs, paragraph styles, tags, links, versions, recycle bin. Pure data with stable ids | Everything else reads or edits this; it must not know how it is drawn, stored or edited |
| L2 | `text`, `ink`, `search`, `math` | Algorithms over the model: rich-text editing and layout rules, ink processing, full-text indexing and queries, math | Pure functions on L1 data, testable without a session |
| L3 | `render` (CPU oracle), `gpu` (wgpu), `format` (`.ncraft`) | Drawing pages to pixels; the native on-disk format | Consume the model; the GPU path is tested against the CPU oracle |
| L4 | `onestore`, `import-export`, `ocr`, `plugins`, `sync` | OneNote files ([MS-ONESTORE]/[MS-ONE]), other formats, OCR hook, sandboxed plug-ins, sync | Interop sits above the model and the native format it converts to |
| L5 | `engine` | The editing session, undo history, and the **command registry** | The single seam every front end uses |
| L6 | `ui-egui`, `automation` | The OneNote-style egui shell (ribbon, panes, canvas, dialogs, options); the JSON control channel and MCP server | Front ends: thin, all actions go through engine commands |
| apps | `notecraft`, `notecraft-cli`, `notecraft-web` | Desktop binary, headless CLI, browser build | Exempt from layering |

## Everything is a command

Every user-visible action is a `CommandSpec` in the engine: id, parity-catalog id, label, menu
path (OneNote's ribbon/menu path), shortcut, params doc, `enabled` predicate and `run`. The
ribbon, context menus, keyboard, CLI, control channel and MCP dispatch by id, so behaviour is
identical whichever way it is invoked, and one command records exactly one undo step.

The details of the document model, persistence and the UI shell are written up in
[`plan.md`](plan.md) and grow here as the crates land.
