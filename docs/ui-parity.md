# NoteCraft UI parity: the 2024 look, surface by surface

> **Last reviewed:** 2026-10-15 · **Last updated:** 2026-10-15 · **Change:** major (first version, decision D1) · **Target:** Microsoft OneNote for Microsoft 365, desktop for Windows, 2024 UI

Decision D1 ([`plan.md`](plan.md) §6) makes the current Microsoft 365 app the visual model. This
page specifies every surface of that look in NoteCraft's own terms: placement, measures, radii,
colours (light and dark), type, states, and how far NoteCraft has got. The commands behind the
surfaces are in [`parity-catalog.md`](parity-catalog.md); the numbers in [`parity.md`](parity.md).

**Clean room.** The reference was only observed and measured on screenshots the maintainers took
of the desktop app (three views: Home, Draw, a notebook with a table) and of OneNote on the web
(the simplified and the classic ribbon of each tab). The screenshots stay on the maintainers'
machines and never enter the repository. Nothing is copied or traced: icons are drawn in code
(`crates/ui-egui/src/icons.rs`), colours are NoteCraft's own (the accent is our violet, not the
reference's), and the font is Inter or Noto Sans, not Segoe UI.

**Units.** Sizes are egui points (1 pt = 1 px at 100 % scale). Web screenshots were measured at
100 %; the desktop ones are about 1.1× that and were scaled back. Values are rounded to the
nearest whole point; where the reference differs between desktop and web, the desktop wins.

## 1. Window structure (top to bottom, left to right)

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ ◆  ←  ↶ ▾  Notebook - Page          [ ⌕ Search            ]       (AB)       │ title bar   40
│ File  Home  Insert  Draw  History  Review  View  Help            [Share ▾]   │ tab row     32
│ ╭──────────────────────────────────────────────────────────────────────────╮ │
│ │ ⎘▾ 🖌 [Calibri ▾][11▾] B I U ab A▾ ✎▾ ☰▾ ⅰ▾ ⇤ ⇥ ≡▾ │ ☑▾ │ Styles▾ … │ │ ribbon card 38 (Classic 92)
│ ╰─────────────────────────────────────────────────────────────────────── ▾ ╯ │
│ ☰                                                     [⌕ Search notebooks ▾] │ nav row     28
│ ▾ ▮ Work Notebook   │ [✎ Add page]  ⇅ │                                   ⤢   │
│     ▌ Meetings      │ Spokes          │  Page title                           │
│     ▌ Inventory  ◀  │ Saddle  ◀       │  ─────────────                        │
│   + New section     │ …               │  Wednesday, August 7, 2024            │
│ ───────────────     │                 │                                       │
│ ▸ ▮ Travel Journal  │                 │                                       │
│                     │                 │                                       │
│ ⧉ Quick notes       │                 │                                       │
└──────────────────────────────────────────────────────────────────────────────┘
  navigation 220       page column 200    page (white, no section tabs above it)
```

There is no status bar and there are no section tabs above the page. Panes (Tags, Search
results) open as cards on the right of the page.

## 2. Tokens

All values live in `crates/ui-egui/src/theme.rs` (`Tokens`); no colour literal appears anywhere
else in the shell except `icons.rs` (enforced by a test).

### 2.1 Colours

| Token | Use | Colorful (light, default) | White | Dark Gray | Black |
|---|---|---|---|---|---|
| `window_back` | title bar, tab row, area around the card, navigation pane | `#F3F2F5` | `#FFFFFF` | `#292929` | `#0A0A0A` |
| `ribbon_card` | ribbon card, menus, dialogs, pane cards | `#FFFFFF` | `#FFFFFF` | `#3B3B3B` | `#1F1F1F` |
| `card_border` | card hairline | `#E3E1E6` | `#D6D6D6` | `#4A4A4A` | `#333333` |
| `card_shadow` | card shadow (alpha) | 10 % black | 12 % black | 40 % black | 60 % black |
| `page_list` | page column | `#FFFFFF` | `#FAFAFA` | `#333333` | `#141414` |
| `canvas` | page | `#FFFFFF` | `#FFFFFF` | `#1F1F1F` (dark page) | `#000000` |
| `nav_hover` / `nav_selected` | tree and page rows | `#EAE8EE` / `#E3E0E8` | `#F0F0F0` / `#E6E6E6` | `#383838` / `#454545` | `#262626` / `#333333` |
| `text` / `text_dim` / `text_disabled` | labels | `#242424` / `#616161` / `#A6A6A6` | same | `#F5F5F5` / `#C8C8C8` / `#6E6E6E` | `#FFFFFF` / `#C8C8C8` / `#6E6E6E` |
| `icon` | line of icons | `#424242` | `#424242` | `#E0E0E0` | `#E0E0E0` |
| `accent` | underline, primary buttons, links, focus | `#7A3DB8` | `#7A3DB8` | `#B794F0` | `#C3A6F5` |
| `on_accent` | text on accent | `#FFFFFF` | `#FFFFFF` | `#1A1A1A` | `#000000` |
| `input` / `input_border` | boxes (Search, font, size) | `#FFFFFF` / `#D1D1D1` | `#FFFFFF` / `#C7C7C7` | `#2E2E2E` / `#5C5C5C` | `#141414` / `#4D4D4D` |
| `hover` / `pressed` / `checked` | ribbon buttons | `#ECEAF0` / `#E0DDE5` / `#E6DDF3` | `#F0F0F0` / `#E0E0E0` / `#EBE3F5` | `#474747` / `#525252` / `#4A3D5C` | `#2E2E2E` / `#3B3B3B` / `#3A2E4D` |

Contrast floors (tested): `text` on every background ≥ 7:1, `text_dim` ≥ 4.5:1, `accent` on
`ribbon_card` and `window_back` ≥ 4.5:1, `on_accent` on `accent` ≥ 4.5:1.
"Use system setting" picks Colorful or Dark Gray from the desktop's colour-scheme preference
(XDG portal on Linux; the egui-reported theme elsewhere).

### 2.2 Radii, spacing, heights

| Token | Value | Where |
|---|---|---|
| `radius.card` | 8 | ribbon card, pane cards, dialogs, toasts |
| `radius.control` | 4 | buttons, rows, inputs, the tab underline (1.5) |
| `radius.menu` | 6 | menus and drop-downs |
| `radius.search` | 6 | title-bar Search, Search notebooks |
| `space.card_margin` | 6 | gap between the card and the window edges |
| `space.row_pad` | 8 | horizontal padding inside tree and page rows |
| `height.title_bar` | 40 | |
| `height.tab_row` | 32 | |
| `height.ribbon_simplified` | 38 | card content height (16 px icons, 28 pt buttons) |
| `height.ribbon_classic` | 92 | 70 controls + 16 group label + padding |
| `height.nav_row` | 28 | the ☰ / Search notebooks row |
| `height.tree_row` / `height.page_row` | 28 | measured 26–29 |
| `width.nav` / `width.page_column` | 220 / 200 | defaults; dragged widths are remembered |
| shadow | offset (0, 2), blur 8 | cards; menus (0, 4) blur 16 |

### 2.3 Type

The UI family is separate from the page's: **Inter** when `CRAFT_FONTS_DIR` or the system has it,
else Noto Sans, Cantarell, DejaVu Sans. The page keeps its Calibri-compatible stack (Carlito).
No font file is in the repository.

| Rung | Size | Weight | Use |
|---|---|---|---|
| `caption` | 11 | regular | classic group labels, date under the title |
| `body` | 13 | regular | buttons, tree and page rows, menus |
| `tab` | 13.5 | regular / semibold (active) | tab row |
| `title` | 13 | regular | window title in the title bar |
| `heading` | 15 | semibold | pane and dialog titles |
| `page_title` | 20 (page font) | light | page title on the canvas |

## 3. Surfaces

Status: **done** (built in the 2024 style and tested), **partial**, **planned** (step of
[`plan.md`](plan.md) §2 M1.5), **out-of-scope** (Microsoft service; drawn disabled).

| # | Surface | Specification | Catalog | Status |
|---|---|---|---|---|
| 1 | Title bar | `window_back`; drawn app mark (ours, no logo), Back, Undo, ▾ quick-access menu (Redo, customize later), title `Notebook - Page` in `title`; drag moves the window, double-click maximizes; the native frame stays on Linux | UI.Window.TitleBar, UI.Window.QAT | planned (A2) |
| 2 | Search (title bar) | centred, 260–440 wide × 26, `input` with `radius.search`, magnifier; finds commands (Tell Me) and notes; Alt+Q focuses it | UI.Window.SearchBox, UI.Window.TellMe | planned (A2) |
| 3 | Account | initials of the author in a 28 circle at the right; disabled (Microsoft account) | UI.Window.Account | out-of-scope (A2) |
| 4 | Tab row | File Home Insert Draw History Review View Help (+ contextual tabs in accent text); active tab semibold with a 3 pt accent underline as wide as the label, radius 1.5; hover = `hover` background radius 4; double-click collapses the ribbon | UI.Ribbon.Tabs, UI.Ribbon.ContextualTabs | planned (A2) |
| 5 | Share | accent-filled button with `on_accent` text, radius 4, at the right of the tab row; disabled (OneDrive) | UI.Window.Share | out-of-scope (A2) |
| 6 | Ribbon card | `ribbon_card`, `radius.card`, hairline `card_border`, shadow; inset `space.card_margin` from the window edges; ▾/▴ chevron at the bottom right switches Simplified ↔ Classic | UI.Ribbon.ModeSwitch | planned (A3) |
| 7 | Simplified ribbon | one row, 28 pt buttons with 16 px icons; labels only where the reference has them (Styles, Email Page, Meeting Details); font and size combos; split ▾ buttons; separators between groups; what doesn't fit goes into the **…** overflow menu by priority; every Classic control reachable (tested) | UI.Ribbon.Simplified | planned (A3) |
| 8 | Classic ribbon | groups with a `caption` label at the bottom in `text_dim`, large buttons (32 px two-tone icons), small buttons in rows of three, Styles gallery framed with previews, group separators | UI.Ribbon.Tabs, UI.Ribbon.Galleries | planned (A3) |
| 9 | Ribbon collapse | Ctrl+F1 or double-click a tab hides the card; the tab row stays | UI.Ribbon.Collapse | planned (A3) |
| 10 | Nav row | under the card: ☰ at the left shows/hides the navigation; Search notebooks at the right (176 × 24, `input`, scope ▾, results drop-down) | UI.Nav.Toggle, UI.Nav.SearchBox | planned (A4) |
| 11 | Notebook tree | every open notebook: chevron, notebook icon in its colour, name; sections indented with a 4 × 14 colour bar radius 2; section groups with chevrons; thin separator between notebooks; selected row `nav_selected` radius 4; + New section in `accent` after each notebook's sections | UI.Nav.NotebookTree, UI.Nav.NewSectionLink, UI.Nav.NotebookPane | planned (A4) |
| 12 | Quick notes | pinned at the bottom of the navigation pane above a separator; opens the Quick Notes section | UI.Nav.QuickNotes | planned (A4) |
| 13 | Page column | `page_list`; Add page (outlined, radius 4, pencil icon, `accent` text) and sort (⇅) on top; rows 28 radius 4, subpages indented 16, stroke chevrons; drag to resize (remembered) | UI.Nav.PageList, UI.Nav.AddPage, UI.Nav.SortPages | planned (A4) |
| 14 | Page | `canvas`; title in `page_title` with a hairline under it, date and time in `caption` `text_dim`; expand ⤢ at the top right (full page view, restore ⤡) | UI.Canvas.Page, UI.Canvas.TitleArea, UI.Canvas.ExpandButton | planned (A5) |
| 15 | Panes | Tags summary and Search results as cards at the right of the page: `heading` title, ✕ close | UI.Panes.FindTags, UI.Nav.SearchResultsPane | planned (A5) |
| 16 | Find on page | floating card at the top right of the page: box, count, ↑ ↓, ✕ | UI.Nav.FindOnPage | planned (A5) |
| 17 | Menus | `ribbon_card`, `radius.menu`, shadow (0, 4) blur 16, rows 28, shortcuts in `text_dim`, separators | UI.Window.ContextMenus | planned (A5) |
| 18 | Dialogs | modal card `radius.card`, `heading` title, primary button `accent` / `on_accent`, secondary outlined | UI.Dialogs.* | planned (A5) |
| 19 | File (backstage) | left rail on `window_back` with ← and the entries, content on `ribbon_card` | BS.* | planned (A5) |
| 20 | Notifications | toasts at the bottom centre (card, 4 s) instead of a status bar | — | planned (A5) |
| 21 | Copilot | ribbon button, disabled | UI.Window.Copilot | out-of-scope (A3) |
| 22 | Window buttons | the compositor's; drawn buttons where none are drawn | — | gap (M4) |
| 23 | Themes | Colorful (default), White, Dark Gray, Black, Use system setting; remembered | UI.Window.DarkMode, OPT.General.OfficeTheme | done (A1): tokens and contrast tests, picked in File › Account, remembered; the Options dialog entry is still missing |
| 24 | Icons | 20 × 20 grid, 1.25 stroke, line in `icon` + accent and fill tones from the tokens, drawn in code | — | planned (A6) |

## 4. Screenshots

`cargo run -p notecraft-ui-egui --example snapshot` writes `target/screenshots/*.png`: every tab in
both ribbon modes, every theme, navigation hidden, a narrow window (overflow), full page view and
2×. Each step of M1.5 is reviewed beside the local reference screenshots before it is committed.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-15 | major | First version (decision D1): structure, tokens, type, 24 surfaces |
