# NoteCraft

A free, open-source note-taking app in the style of OneNote for desktop, written in Rust.
Notebooks are plain folders on your disk, everything is saved as you type, and the window follows
the modern OneNote layout: tabs and ribbon on top, notebook / section / page navigation on the
left, a free-form canvas where you click anywhere and start typing.

Status: **beta**. Most of the daily workflow works; see [Parity](#parity) for what does not yet.

## Features

- **Free-form pages** — note containers you drag and resize, nested paragraphs, Tab / Shift+Tab
  indentation, bullets and numbering, styles, tags (to-do, important, question…), find tags.
- **Notebooks, sections, section groups, pages and subpages**, page versions, a recycle bin,
  password-protected sections, search across notebooks, Quick Notes.
- **Insert** — tables (with the Table Layout tab), pictures, screen clipping, file attachments,
  file printouts (PDF), spreadsheets (`.xlsx` / `.csv`), links, equations, symbols, date and time,
  page templates, stickers, and OCR on pictures.
- **Draw** — pens, highlighters, eraser, lasso, shapes, ruler, ink to shape, insert space,
  ink replay.
- **Review** — spelling, thesaurus, accessibility checker, language preferences, section passwords.
- **View** — zoom, page color and rule lines, full page view, dock to desktop, new window.
- **Ribbon, Quick Access Toolbar, KeyTips, context menus, mini toolbar** and an Italian interface;
  Customize the Ribbon and most of Options.
- **Export** to PDF, Word, HTML, Markdown, MHT and SVG; **import** Word, Evernote (ENEX), HTML,
  Markdown and spreadsheets.
- A **control protocol** and an MCP server so scripts and AI agents can drive the app
  ([`docs/control-protocol.md`](docs/control-protocol.md), [`docs/mcp.md`](docs/mcp.md)).

## Run

Requires Rust 1.90 or newer.

```sh
cargo run --release -p notecraft                       # start the app
cargo run --release -p notecraft -- --sample           # open the sample notebook
cargo run --release -p notecraft -- ~/Notebooks/Work   # open a notebook folder
cargo run --release -p notecraft -- --help             # all options
```

On Linux it runs on Wayland and X11. Optional helpers are used when installed: `tesseract`
(OCR), `xdg-email` (Send), MyThes dictionaries (Thesaurus).

## Parity

NoteCraft tracks every OneNote desktop control, menu item, shortcut and option in
[`docs/parity-catalog.md`](docs/parity-catalog.md); [`docs/parity-checklist.md`](docs/parity-checklist.md)
is generated from it and shows what is wired. About 78% of the catalog is wired. Not yet:
audio / video recording and playback, writing `.one` / `.onepkg` files, conflict pages (sync),
linked notes, handwriting recognition (ink to text / math) and the equation structure menu.

## Develop

```sh
cargo test --workspace   # tests
cargo xtask ci           # all checks (format, lint, tests, layering, parity, deny, …)
cargo xtask parity       # regenerate docs/parity-checklist.md
```

Start with [`AGENTS.md`](AGENTS.md), then [`docs/plan.md`](docs/plan.md) and the latest entry of
[`log/devlog.md`](log/devlog.md). The icons are drawn in code (original artwork); the code is
clean-room: nothing is derived from Microsoft's software or assets.

## License

MIT or Apache-2.0, at your option ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).

<sub>Microsoft and OneNote are trademarks of Microsoft Corporation. NoteCraft is an independent project, not affiliated with or endorsed by Microsoft.</sub>
