<h1 align="center">NoteCraft</h1>

<p align="center">
  <b>Notebooks, notes and ink; an open-source, clean-room reimplementation of Microsoft OneNote, rebuilt in pure Rust.</b>
</p>

<p align="center">
  A fast, local-first, open-source take on the OneNote workflow in its current Microsoft 365 look:
  the same ribbon (simplified or classic), the same notebook → section → page structure, the same
  free-form canvas and the same keyboard shortcuts.
  It runs natively on Linux (Wayland and X11), macOS and Windows, and in the browser via
  WebAssembly.<br>
  <i>The 14th Crafting App, by the ArtCraft team and community.</i>
</p>

<p align="center">
  <img alt="Written in Rust" src="https://img.shields.io/badge/written%20in-Rust-5e2f96?style=flat-square&logo=rust&logoColor=white">
  <img alt="Runs on Linux, macOS, Windows and the web" src="https://img.shields.io/badge/runs%20on-Linux%20%C2%B7%20macOS%20%C2%B7%20Windows%20%C2%B7%20Web-8a4fc8?style=flat-square">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-5e2f96?style=flat-square">
  <img alt="Agent-drivable over MCP" src="https://img.shields.io/badge/agents-MCP-8a4fc8?style=flat-square">
</p>

> [!WARNING]
> **Early alpha.** NoteCraft is being built in the open, milestone by milestone. Parity with
> OneNote is *measured*, never assumed: the two headline numbers (wiring and behaviour) are in
> [`docs/parity.md`](docs/parity.md), and every catalogued OneNote control, shortcut and behaviour
> with its status is in [`docs/parity-catalog.md`](docs/parity-catalog.md).

<p align="center">
  <a href="#why-notecraft">Why NoteCraft</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#for-agents-mcp-cli-and-the-control-channel">For agents</a> ·
  <a href="#architecture">Architecture</a> ·
  <a href="#roadmap">Roadmap</a> ·
  <a href="#the-crafting-apps">The Crafting Apps</a> ·
  <a href="#license-and-credits">License and credits</a>
</p>

## Why NoteCraft

- **A 1:1 replacement, not a look-alike.** The ribbon tabs (Home, Insert, Draw, History, Review,
  View, Help), their groups and controls in the one-row simplified ribbon or the classic one, the
  navigation on the left (every notebook, its sections, Quick notes, the page column), the
  free-form canvas with note containers, tags and the Find Tags pane, and OneNote's documented
  keyboard shortcuts — catalogued from public documentation and implemented to match.
- **Your notes stay yours.** Local-first, no telemetry, no account. Notebooks are folders of
  section files you can back up, sync over a shared folder, or read with any tool.
- **OneNote files.** Reading (and, where the public specifications allow, writing) `.one`,
  `.onetoc2` and `.onepkg` is a milestone of its own (M3), implemented from Microsoft's published
  Open Specifications.
- **Every action is a command.** The UI, the CLI, the JSON control channel and the MCP server all
  call the same command registry, so agents can drive everything a person can.

## Quick start

```sh
cargo run --release -p notecraft                               # desktop app
cargo run --release -p notecraft -- --sample                   # open the sample notebook
cargo run --release -p notecraft -- ~/Notebooks/Work           # open a notebook folder
cargo run --release -p notecraft -- --sample --control 7990    # + JSON control channel
cargo xtask ci                                                 # fmt, clippy, tests, assets, layers, wasm, parity, deny
```

### Web

```sh
cd apps/notecraft-web && trunk build --release     # → dist/web (serve it with any static server)
cd apps/notecraft-web && trunk serve --release     # http://127.0.0.1:8785
```

You need [trunk](https://trunkrs.dev) and the `wasm32-unknown-unknown` target.

### Linux

Wayland and X11 are both first-class (winit picks Wayland when `WAYLAND_DISPLAY` is set). File
dialogs go through the XDG desktop portal. The build needs no C libraries beyond what winit
loads at run time (`libxkbcommon`, `libwayland-client` or `libX11`, Vulkan or GL drivers).

## For agents: MCP, CLI and the control channel

```sh
notecraft-cli commands                      # every command: id, catalog id, menu path, shortcut, params
notecraft-cli run --notebook ~/Notebooks/Work --cmd 'page.new={"title":"Agenda"}' --print
notecraft-cli batch script.jsonl --sample   # one {"command": id, "params": {...}} per line
notecraft-cli mcp --notebook ~/Notebooks/Work   # MCP server over stdio (headless)
claude mcp add notecraft -- notecraft-cli mcp   # Claude Code
```

The MCP server ([`docs/mcp.md`](docs/mcp.md)) can be confined to folders with
`--automation-read-root` / `--automation-write-root`, or bridged to the running app with
`--connect 127.0.0.1:<port>`. The desktop app's control channel (`--control <port>`) accepts
JSON lines on `127.0.0.1`, guarded by a per-start key, that run commands, press keys and take
screenshots; see [`docs/control-protocol.md`](docs/control-protocol.md).

## Architecture

A layered Cargo workspace (the same scheme as PhotoCraft), enforced by `cargo xtask layers`:

| Layer | Crates | Job |
|---|---|---|
| L0 | `geom`, `color`, `util`, `ink-geom`, `codecs` | page geometry in points, colours, ink geometry |
| L1 | `doc` | notebook → section group → section → page → outlines; rich text, tags, links, versions, recycle bin |
| L2 | `text`, `ink`, `search`, `math` | rich-text editing, ink, full-text search, math |
| L3 | `render`, `gpu`, `format` | CPU reference renderer (oracle), wgpu renderer, native `.ncraft` format |
| L4 | `onestore`, `import-export`, `ocr`, `plugins`, `sync` | OneNote files, other formats, OCR hook, WASM plug-ins, sync |
| L5 | `engine` | session, undo, **command registry** |
| L6 | `ui-egui`, `automation` | the OneNote-style egui shell; control channel and MCP |

Details: [`docs/architecture.md`](docs/architecture.md) and [`docs/plan.md`](docs/plan.md).

## Roadmap

M1 shell and core editing → M2 Insert and Draw → M3 OneNote file interop → M4 History, Review,
View, Help and power features. Status, numbers and the current focus:
[`ROADMAP.md`](ROADMAP.md) and [`docs/roadmap.md`](docs/roadmap.md).

## The Crafting Apps

NoteCraft is one of the ArtCraft Crafting Apps — PhotoCraft, LightCraft, FilmCraft, DesignCraft,
VectorCraft, PdfCraft, EffectCraft, WordCraft, DeckCraft, GridCraft, CADCraft, SoundCraft,
FigCraft — all native Rust, all open source, at [getartcraft.com](https://getartcraft.com/apps).

## License and credits

NoteCraft is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
Copyright (c) 2026 ArtCraft Team and the NoteCraft contributors. Required notices are in [NOTICE](NOTICE).

Bundled icons, images and other assets keep their own open licenses; each one is listed with its
author, source and license in [ATTRIBUTION.md](ATTRIBUTION.md). All UI icons are drawn in code and
are original. NoteCraft bundles no fonts: it uses the fonts installed on your system, or
[craft-fonts](https://github.com/storytold/craft-fonts) when built with `CRAFT_FONTS_DIR`.

<sub>Microsoft, OneNote and Microsoft 365 are trademarks or registered trademarks of Microsoft Corporation in the United States and/or other countries. NoteCraft is an independent, open-source project and is not affiliated with, sponsored by or endorsed by Microsoft Corporation; these names are used only to describe the workflows and file formats it is compatible with.</sub>
