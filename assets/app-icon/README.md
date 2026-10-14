# NoteCraft app icon (PLACEHOLDER)

**This icon is temporary.** Every Crafting App's icon is an engraved creature portrait that the
product owner makes (craftrules `standards/icon-design.md`); NoteCraft's hasn't been made yet.
Until it is, this placeholder stands in: a ring-bound notebook page with a title line, ruled
lines and a ticked to-do box, and three section tabs on its right edge, on a full-bleed
NoteCraft-violet tile. It is original geometry written by hand for this repository,
deliberately not modelled on any vendor's note-taking logo (no letters, no "N", no overlapping
panels).

When the owner's art arrives, replace `notecraft.svg`, run `packaging/icons.sh`, update this
README and the rows in `ATTRIBUTION.md`, and drop the word placeholder.

## Palette

| Colour | Hex | Used for |
|---|---|---|
| Ink | `#0b0b0c` | Line work and contour |
| Paper | `#efe9dc` | The page and two tabs |
| NoteCraft violet (app colour, **proposed**) | `#8a4fc8` | The field and the middle tab |

The app colour is a proposal pending the owner's approval; its darker ink variant is `#5e2f96`.

## Files

| File | Use |
|---|---|
| `notecraft.svg` | Canonical artwork; also `hicolor/scalable` |
| `notecraft-1024.png` | 1024 px render of the full tile |
| `notecraft-macos-512.png` | Runtime Dock icon on macOS |
| `notecraft.ico` | Windows icon (9 sizes) |
| `hicolor/` | Linux icon theme (16–512 px and scalable) |

`notecraft.icns` is generated on macOS (needs `iconutil`) before the first macOS release.
