# Attribution

Every non-code asset in this repository, with its author, source and licence. `cargo xtask assets`
(part of `cargo xtask ci`) fails if an asset file is missing from this table. Add the row in the
same commit as the file. NoteCraft bundles **no** Microsoft assets of any kind (AGENTS.md › Clean
room), and **no fonts** (they come from storytold/craft-fonts at build time, when
`CRAFT_FONTS_DIR` is set).

## Original assets

| Path | Title | Author | Source | Licence |
|---|---|---|---|---|
| `assets/app-icon/notecraft.svg` | NoteCraft app icon (placeholder) | NoteCraft contributors | original, drawn by hand for this repository | MIT OR Apache-2.0 (`assets/app-icon/LICENSE.txt`) |
| `assets/app-icon/notecraft-1024.png` | render of `notecraft.svg` | NoteCraft contributors | `packaging/icons.sh` | MIT OR Apache-2.0 |
| `assets/app-icon/notecraft-macos-512.png` | render of `notecraft.svg` (macOS grid) | NoteCraft contributors | `packaging/icons.sh` | MIT OR Apache-2.0 |
| `assets/app-icon/notecraft.ico` | Windows icon rendered from `notecraft.svg` | NoteCraft contributors | `packaging/icons.sh` | MIT OR Apache-2.0 |
| `assets/app-icon/hicolor/16x16/apps/ai.storyteller.notecraft.png` | render of `notecraft.svg` | NoteCraft contributors | `packaging/icons.sh` | MIT OR Apache-2.0 |
| `assets/app-icon/hicolor/24x24/apps/ai.storyteller.notecraft.png` | render of `notecraft.svg` | NoteCraft contributors | `packaging/icons.sh` | MIT OR Apache-2.0 |
| `assets/app-icon/hicolor/32x32/apps/ai.storyteller.notecraft.png` | render of `notecraft.svg` | NoteCraft contributors | `packaging/icons.sh` | MIT OR Apache-2.0 |
| `assets/app-icon/hicolor/48x48/apps/ai.storyteller.notecraft.png` | render of `notecraft.svg` | NoteCraft contributors | `packaging/icons.sh` | MIT OR Apache-2.0 |
| `assets/app-icon/hicolor/64x64/apps/ai.storyteller.notecraft.png` | render of `notecraft.svg` | NoteCraft contributors | `packaging/icons.sh` | MIT OR Apache-2.0 |
| `assets/app-icon/hicolor/128x128/apps/ai.storyteller.notecraft.png` | render of `notecraft.svg` | NoteCraft contributors | `packaging/icons.sh` | MIT OR Apache-2.0 |
| `assets/app-icon/hicolor/256x256/apps/ai.storyteller.notecraft.png` | render of `notecraft.svg` | NoteCraft contributors | `packaging/icons.sh` | MIT OR Apache-2.0 |
| `assets/app-icon/hicolor/512x512/apps/ai.storyteller.notecraft.png` | render of `notecraft.svg` | NoteCraft contributors | `packaging/icons.sh` | MIT OR Apache-2.0 |
| `assets/app-icon/hicolor/scalable/apps/ai.storyteller.notecraft.svg` | copy of `notecraft.svg` | NoteCraft contributors | `packaging/icons.sh` | MIT OR Apache-2.0 |
| `assets/app-icon/LICENSE.txt` | licence of the app icon | NoteCraft contributors | original | MIT OR Apache-2.0 |
| `assets/app-icon/README.md` | provenance of the app icon | NoteCraft contributors | original | MIT OR Apache-2.0 |

## Reference material (not bundled)

These public documents define what NoteCraft must do. Nothing from them is copied into the
binaries; they are cited by section in code comments and in `docs/parity-catalog.md`.

| Reference | Publisher | Where | Licence / terms | Used for |
|---|---|---|---|---|
| Office Fluent User Interface Control Identifiers, OneNote 2016 and 2019 (`onenotecontrols.xlsx`) | Microsoft | github.com/OfficeDev/office-fluent-ui-command-identifiers | MIT | the inventory of ribbon tabs, groups, controls, backstage entries and context menus in `docs/parity-catalog.md` (control names only; labels are our functional descriptions) |
| Keyboard shortcuts in OneNote | Microsoft Support | support.microsoft.com (article 44b8b3f4-c274-4bcc-a089-e80fdcc87950) | public documentation | the `KB.*` rows of the catalog |
| [MS-ONESTORE], [MS-ONE], [MS-ONETOC2], [MS-FSSHTTPB], [MS-CAB] | Microsoft Open Specifications | learn.microsoft.com/openspecs | Open Specifications Promise | the `.one`/`.onetoc2`/`.onepkg` readers and writers (M3) |
