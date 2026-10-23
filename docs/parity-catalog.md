# Parity catalog: Microsoft OneNote → NoteCraft

> **Last reviewed:** 2026-10-15 · **Last updated:** 2026-10-15 · **Change:** major (decision D1: the 2024 UI; source `M365-2024`) · **Target:** Microsoft OneNote for Microsoft 365, desktop for Windows, in its 2024 look (command inventory from the OneNote 2016/2019 ribbon app)

This is the **source of truth for parity** (brief §0): 1314 rows, one per ribbon control, context-menu item, backstage entry, keyboard shortcut, option, documented behaviour, file-format construct and UI surface of the reference product. `cargo xtask parity` joins it with the command registry and the tests and computes every number in [`parity.md`](parity.md); the generated per-area table and work list are in [`parity-checklist.md`](parity-checklist.md).

## How to read it

| Column | Meaning |
|---|---|
| **ID** | Stable id: `UI.` surfaces, `QAT.` toolbar, `R.<Tab>.<Group>.<Control>` ribbon, `CT.` contextual tabs, `BS.` backstage, `CM.<Menu>.<Control>` context menus, `QAT.NR.` commands not in the ribbon, `DOCK.` docked ribbon, `KB.` shortcuts, `OPT.` options, `BEH.` behaviour, `FMT.` formats, `NC.` NoteCraft-only additions. Commands and UI controls cite it in a comment |
| **Label** | Functional label (feature names only; never copied help text) |
| **Path** | Where it lives, as `Tab/Group/Control` (menu paths of commands must match it for their primary row) |
| **Kind** | button, toggle, gallery, split, menu, combo, shortcut, key-behaviour, option, behaviour, format, surface… |
| **Shortcut** | The reference's key combination in portable form (`Mod` = Ctrl, ⌘ on macOS; alternatives separated by ` / `). The command must bind it |
| **Command** | The NoteCraft command id that implements the row (planned ids are filled in ahead of time) |
| **Pri** | P0 daily core workflow · P1 important · P2 secondary · P3 niche or legacy |
| **Status** | `missing` · `disabled` (shown for layout parity, does nothing yet) · `wired` (works; a test mentions the command or the ID) · `verified` (a test carrying `// parity: <ID>` pins the documented behaviour) · `out-of-scope` (needs a Microsoft cloud service, Outlook or the Office Store; still drawn, disabled, where it has a placement) |
| **Source** | `FLUENT19:<Control>` = Office Fluent UI control identifiers for OneNote 2019 (Microsoft, MIT, github.com/OfficeDev/office-fluent-ui-command-identifiers); `KBD365` / `KBD-W10` = the "OneNote for Windows" / "OneNote for Windows 10" tables of Microsoft's "Keyboard shortcuts in OneNote" support article; `[MS-…]` = Open Specifications; `M365-2024` = observed in the current Microsoft 365 app (2024 UI, desktop and web) on screenshots taken by the maintainers: only observed and measured, never copied or traced (no Microsoft asset enters the repo); `GK` = general knowledge of the user-visible product (always flagged `unverified` where it fixes a detail) |
| **Notes** | Variant that defined the row, conflicts in the sources, `unverified` assumptions |

**Variant policy.** The Microsoft 365 desktop app is the reference. Its ribbon descends from
OneNote 2016/2019, whose complete control inventory Microsoft publishes (FLUENT19); rows that exist
only in the Microsoft 365 app (Help tab, newer Draw controls, Sticker, Immersive Reader…) are added
from general knowledge and marked so. Where a detail is undocumented, NoteCraft implements the most
plausible behaviour and the row says `unverified`. **The look and layout follow the 2024 UI**
(decision D1 in [`plan.md`](plan.md); per-surface specification in [`ui-parity.md`](ui-parity.md)):
surfaces that exist only in the 2016 layout are `out-of-scope` and their Notes name the 2024
surface that replaces them; the commands behind them stay.

**Counting.** Wiring coverage = (`wired` + `verified`) / all rows; behavioural coverage =
`verified` / all rows. Out-of-scope rows stay in the denominator of the headline numbers (an
in-scope figure is reported separately).

## Window layout and UI surfaces

The window structure, panes and dialogs a OneNote user expects to find. A UI row is wired when the surface exists and works; its test is a UI test that mentions the row ID.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| UI.Window.TitleBar | Title bar with the page and notebook name | Window/Title Bar | surface |  |  | P0 | wired | GK |  |
| UI.Window.QAT | Quick Access Toolbar in the title bar (customizable, Back/Undo by default) | Window/Quick Access Toolbar | surface |  |  | P0 | wired | FLUENT19:Quick Access Toolbar | Back, Undo, Redo; customizing the toolbar is not implemented yet. |
| UI.Window.TellMe | Tell Me / Search box for commands in the title bar | Window/Tell Me | surface |  |  | P2 | wired | GK (Office 2016+) | In the 2024 UI this is the title-bar Search box (UI.Window.SearchBox). |
| UI.Window.SearchBox | Search box centred in the title bar: finds commands (Tell Me) and notes | Window/Title Bar/Search | surface |  |  | P1 | wired | M365-2024 | Alt+Q focuses it; it lists matching commands (Enter runs the first) and hands the text to Search notebooks for notes. |
| UI.Window.Share | Share button at the right of the tab row (share the notebook through OneDrive) | Window/Title Bar/Share | button |  |  | P3 | out-of-scope | M365-2024 | Needs a Microsoft cloud service; drawn disabled. |
| UI.Window.Copilot | Copilot button (ribbon and title bar) | Window/Title Bar/Copilot | button |  |  | P3 | out-of-scope | M365-2024 | Needs a Microsoft cloud service; drawn disabled. |
| UI.Window.Account | Account button with the user's picture or initials (Microsoft account sign-in) | Window/Title Bar/Account | button |  |  | P3 | out-of-scope | M365-2024 | Sign-in needs a Microsoft account; NoteCraft shows the author's initials, disabled. |
| UI.Ribbon.Tabs | Ribbon tab strip: File, Home, Insert, Draw, History, Review, View, Help (+ contextual tabs) | Window/Ribbon/Tabs | surface |  |  | P0 | verified | FLUENT19 (tab list); GK (Help tab, M365) |  |
| UI.Ribbon.Collapse | Collapse/expand the ribbon (double-click a tab, Ctrl+F1, pin) | Window/Ribbon/Collapse | surface |  |  | P0 | wired | FLUENT19:MinimizeRibbon | Arrow button and Ctrl+F1; the pin and double-click a tab also work. |
| UI.Ribbon.DisplayOptions | Ribbon display options (auto-hide, tabs only, tabs and commands) | Window/Ribbon/Display Options | surface |  |  | P2 | wired | GK (Office 2016+) | The ▾ chevron at the ribbon's bottom right: layout (single-line or classic) and Always show / Show tabs only; Automatically hide is drawn disabled (not implemented). |
| UI.Ribbon.Simplified | Simplified ribbon: one row of 16 px icons with labels only where needed, font and size boxes, split ▾ buttons, separators and an overflow … menu with what doesn't fit | Window/Ribbon/Simplified | surface |  |  | P0 | wired | M365-2024 | The default ribbon in the 2024 UI. Placements follow the reference screenshots; controls not placed are listed in the … menu under their classic group. |
| UI.Ribbon.ModeSwitch | Switch between the simplified and the classic ribbon (▾ chevron at the ribbon's bottom right); the choice is remembered | Window/Ribbon/Mode Switch | surface |  | ui.ribbonMode | P0 | wired | M365-2024 | Remembered in the desktop settings. |
| UI.Ribbon.ContextualTabs | Contextual tabs appear with the selection (Table, Equation, Audio & Video) | Window/Ribbon/Contextual Tabs | surface |  |  | P1 | wired | FLUENT19:TabSet* | Table Layout while the selection is in a table; Equation and Audio & Video come with their objects. |
| UI.Ribbon.Keytips | Access keys (Alt / F10 show keytips; Alt+N, F etc.) | Window/Ribbon/Keytips | surface |  |  | P2 | missing | KBD365 (Alt+N sequences) |  |
| UI.Ribbon.Galleries | In-ribbon galleries with live preview (Styles, Tags, Pens) | Window/Ribbon/Galleries | surface |  |  | P1 | missing | FLUENT19 (gallery controls) |  |
| UI.Ribbon.Tooltips | ScreenTips with the command name, shortcut and description | Window/Ribbon/ScreenTips | surface |  |  | P1 | missing | GK |  |
| UI.Nav.NotebookPane | Notebook pane on the left: notebooks, their section groups and sections in a tree | Window/Navigation/Notebook Pane | surface |  |  | P0 | wired | GK; FLUENT19:ContextMenuNotebook |  |
| UI.Nav.NotebookPaneCollapse | Collapse/expand (pin) the notebook pane; notebook drop-down when collapsed | Window/Navigation/Notebook Pane Collapse | surface |  |  | P0 | wired | GK | In the 2024 UI the ☰ button (UI.Nav.Toggle) shows and hides the navigation. |
| UI.Nav.Toggle | ☰ button under the ribbon that shows and hides the navigation (notebook tree and page column) | Window/Navigation/Toggle | surface |  |  | P0 | wired | M365-2024 | Runs `view.navigationPanes` (R.View.Views.NavigationPanes). |
| UI.Nav.NotebookTree | Tree of every open notebook: chevrons, notebook icon in the notebook's colour, sections with a coloured bar, section groups with chevrons, separators between notebooks, selection as a rounded grey background | Window/Navigation/Notebook Tree | surface |  |  | P0 | wired | M365-2024 | Folds are remembered in the desktop settings. |
| UI.Nav.NewSectionLink | "+ New section" link at the end of each notebook's sections | Window/Navigation/New Section | surface |  |  | P0 | wired | M365-2024 |  |
| UI.Nav.QuickNotes | Quick notes, pinned at the bottom of the navigation pane | Window/Navigation/Quick Notes | surface |  | nav.quickNotes | P1 | wired | M365-2024 | Opens (creating it if needed) the Quick Notes section of the first open notebook; unverified where the reference keeps it (its Options choose the location). |
| UI.Nav.SortPages | Sort pages button beside Add page (by title, date created, date modified) | Window/Navigation/Sort Pages | surface |  | page.sort | P1 | wired | M365-2024 | Title A→Z, date created or modified newest first; a page keeps its subpages under it. Sort orders unverified. |
| UI.Nav.NotebookList | Notebook drop-down list (switch notebook, Ctrl+G) | Window/Navigation/Notebook List | surface |  |  | P0 | verified | KBD365 (Ctrl+G) |  |
| UI.Nav.SectionTabs | Section tabs across the top of the page area, coloured by section colour, with section group tabs and the + (Create a New Section) tab | Window/Navigation/Section Tabs | surface |  |  | P0 | out-of-scope | GK; FLUENT19:ContextMenuSectionArea | Not present in the 2024 UI (decision D1); replaced by the sections in the notebook tree (UI.Nav.NotebookTree) and + New section (UI.Nav.NewSectionLink). |
| UI.Nav.PageList | Page list: titles, subpage indentation, collapse arrows, selected page highlight, resizable width (2024 UI: a column between the notebook tree and the page) | Window/Navigation/Page List | surface |  |  | P0 | wired | GK; KBD365 (Ctrl+Shift+[ / ]); M365-2024 (placement) |  |
| UI.Nav.AddPage | "Add page" button above the page list (2024 UI: outlined, with the sort button beside it); hover insertion line between pages | Window/Navigation/Add Page | surface |  |  | P0 | wired | GK; M365-2024 (look) | The hover insertion line between pages is not implemented yet. |
| UI.Nav.PageListLeft | Option: page tabs appear on the left | Window/Navigation/Page List Position | surface |  |  | P2 | out-of-scope | GK (Options › Display) | Not present in the 2024 UI (decision D1): the page column is always beside the notebook tree (unverified). |
| UI.Nav.SearchBox | Search notebooks box (2024 UI: under the ribbon, at the right) with scope drop-down and as-you-type results | Window/Navigation/Search Box | surface |  |  | P0 | wired | KBD365 (Ctrl+E); FLUENT19:ContextMenuSearchScope; M365-2024 (placement) | Under the ribbon at the right in the 2024 UI; Ctrl+E focuses it. |
| UI.Nav.SearchResultsPane | Search Results pane (Alt+O) docked on the right, grouped by section | Window/Navigation/Search Results Pane | surface |  |  | P1 | wired | KBD365 (Alt+O) | Opened with Pin Search Results (Alt+O) from the search drop-down. |
| UI.Nav.FindOnPage | Find on page bar (Ctrl+F): match count, next/previous, highlights on the page | Window/Navigation/Find on Page | surface |  |  | P0 | wired | KBD365 (Ctrl+F, F3, Shift+F3) | Match highlighting on the page is not pinned by a test (unverified). |
| UI.Canvas.Page | Page canvas: free-form, white page that grows right and down with content; page title area with date and time | Window/Canvas/Page | surface |  |  | P0 | wired | GK; [MS-ONE] page object |  |
| UI.Canvas.TitleArea | Page title line with the creation date and time beneath it | Window/Canvas/Title Area | surface |  |  | P0 | wired | GK; [MS-ONE] CachedTitleString, TopologyCreationTimeStamp |  |
| UI.Canvas.Container | Note container frame on hover/focus: move bar on top, resize handle on the right edge | Window/Canvas/Note Container | surface |  |  | P0 | verified | GK; [MS-ONE] jcidOutlineNode |  |
| UI.Canvas.ParagraphHandle | Paragraph handle (four-way arrow) left of each paragraph on hover: drag to move, click to select, double-click to collapse | Window/Canvas/Paragraph Handle | surface |  |  | P0 | verified | GK; FLUENT19:ContextMenuOutlineElementHandle |  |
| UI.Canvas.Scrollbars | Vertical and horizontal scroll bars; option to put the vertical bar on the left | Window/Canvas/Scroll Bars | surface |  |  | P1 | missing | GK (Options › Display) |  |
| UI.Canvas.MiniToolbar | Mini toolbar on text selection (font, size, bold, italic, underline, highlight, colour, indent, bullets, styles, tags) | Window/Canvas/Mini Toolbar | surface |  |  | P1 | verified | GK (Options › General) |  |
| UI.Canvas.PasteOptions | Paste Options button after pasting | Window/Canvas/Paste Options Button | surface |  |  | P1 | verified | GK (Options › Advanced) |  |
| UI.Canvas.InfoBar | Information bar at the top of the page (Ctrl+Shift+W performs its action) | Window/Canvas/Information Bar | surface |  |  | P2 | missing | KBD365 (Ctrl+Shift+W) |  |
| UI.Canvas.ExpandButton | Expand / restore button at the top right of the page (full page view) | Window/Canvas/Expand | surface |  |  | P1 | wired | M365-2024 | Runs `view.fullPage`. |
| UI.Panes.FindTags | Tags Summary pane: group by tag/section/title/date/note text, search scope, Show only unchecked items, Create Summary Page | Window/Panes/Tags Summary | surface |  |  | P0 | wired | FLUENT19:FindTaggedNotes; GK |  |
| UI.Panes.CustomizeTags | Customize Tags dialog: new tag, modify (name, symbol, font colour, highlight colour), reorder, remove | Window/Panes/Customize Tags | surface |  |  | P1 | missing | FLUENT19:TagsCustomize; GK |  |
| UI.Panes.PageVersions | Page versions listed under the page in the page list, by date and author | Window/Panes/Page Versions | surface |  |  | P1 | missing | FLUENT19:PageVersions |  |
| UI.Panes.RecycleBin | Notebook Recycle Bin as a special section with Deleted Pages / Deleted Sections | Window/Panes/Recycle Bin | surface |  |  | P0 | missing | FLUENT19:RecycleBin; GK |  |
| UI.Panes.Templates | Templates pane (categories, preview, Choose default template) | Window/Panes/Templates | surface |  |  | P1 | missing | FLUENT19:PageTemplatesTaskpane |  |
| UI.Panes.PaperSize | Paper Size pane (size, orientation, width, height, print margins, Save current page as a template) | Window/Panes/Paper Size | surface |  |  | P1 | missing | FLUENT19:PaperSize |  |
| UI.Panes.Password | Password Protection pane (Set Password, Remove Password, Lock All, options) | Window/Panes/Password Protection | surface |  |  | P1 | missing | FLUENT19:PasswordProtectSection |  |
| UI.Panes.Research | Research / Thesaurus pane | Window/Panes/Research | surface |  |  | P3 | missing | FLUENT19:ResearchPane |  |
| UI.Panes.RecentEdits | Search results pane listing recently edited pages by date | Window/Panes/Recent Edits | surface |  |  | P1 | missing | FLUENT19:FindByDateMenu |  |
| UI.Panes.ColorThickness | Color & Thickness dialog (pen/highlighter, thickness, colour) | Window/Panes/Color & Thickness | surface |  |  | P1 | wired | FLUENT19:MoreColorAndThicknessOptions |  |
| UI.Dialogs.MoveCopyPage | Move or Copy Pages dialog (notebook tree, Move/Copy buttons, search box) | Window/Dialogs/Move or Copy Pages | surface |  |  | P0 | verified | FLUENT19:MoveOrCopyPage | A flat, searchable list of every section with its full notebook path, not a collapsible tree. |
| UI.Dialogs.MoveCopySection | Move or Copy Section dialog | Window/Dialogs/Move or Copy Section | surface |  |  | P0 | verified | FLUENT19:SectionMove | A flat, searchable list of notebooks, not a tree. |
| UI.Dialogs.Link | Link dialog (text to display, address, search pages, pick a location in OneNote) | Window/Dialogs/Link | surface |  |  | P0 | verified | FLUENT19:HyperlinkCreateHere |  |
| UI.Dialogs.NotebookProperties | Notebook Properties dialog (display name, colour, location, default format) | Window/Dialogs/Notebook Properties | surface |  |  | P1 | missing | FLUENT19:NotebookProperties |  |
| UI.Dialogs.ParagraphSpacing | Paragraph Spacing dialog (before, after, line spacing at least) | Window/Dialogs/Paragraph Spacing | surface |  |  | P1 | missing | FLUENT19:ParagraphSpacingOptions |  |
| UI.Dialogs.InsertTable | Insert Table dialog (columns, rows) | Window/Dialogs/Insert Table | surface |  |  | P1 | verified | FLUENT19:TableInsert |  |
| UI.Dialogs.Symbol | Symbol dialog | Window/Dialogs/Symbol | surface |  |  | P2 | verified | FLUENT19:SymbolsDialog |  |
| UI.Dialogs.Options | OneNote Options dialog with its categories | Window/Dialogs/Options | surface |  |  | P0 | verified | FLUENT19:ApplicationOptionsDialog |  |
| UI.Dialogs.CustomizeRibbon | Customize the Ribbon page (tabs, groups, commands, reset, import/export) | Window/Dialogs/Customize Ribbon | surface |  |  | P2 | missing | GK (Options) |  |
| UI.Dialogs.CustomizeQAT | Quick Access Toolbar customization page (and the QAT drop-down) | Window/Dialogs/Customize QAT | surface |  |  | P1 | missing | GK (Options) |  |
| UI.Window.FullPage | Full Page View: ribbon and navigation hidden, a single button to return | Window/Full Page View | surface |  |  | P0 | wired | FLUENT19:FullPageView |  |
| UI.Window.Docked | Docked window: OneNote docked to the side of the desktop with the compact ribbon | Window/Docked | surface |  |  | P2 | missing | FLUENT19:CurrentWindowDock; TabHomeNarrow… |  |
| UI.Window.QuickNote | Quick Note window: a small borderless page saved to Quick Notes | Window/Quick Note | surface |  |  | P1 | missing | FLUENT19:NewSideNote; KBD365 |  |
| UI.Window.ContextMenus | Right-click context menus for each object (see CM rows) | Window/Context Menus | surface |  |  | P0 | wired | FLUENT19 (context menus) | Notebook, section group, section, page and pane-area menus; object menus on the page are not implemented yet. |
| UI.Window.DarkMode | Office theme (Colorful, Dark Gray, Black, White, Use system setting) and dark page canvas | Window/Theme | surface |  |  | P1 | wired | GK; M365-2024 | Picked in File › Account; the system setting follows the XDG portal on Linux. The Dark Gray and Black page is dark by default (the reference's page-colour switch is not implemented). |
| UI.Window.HiDPI | HiDPI and fractional scaling (Linux Wayland/X11) | Window/HiDPI | surface |  |  | P0 | wired | brief §4 | Rendered at 1.25×, 1.5× and 2× in tests. |
| UI.Window.IME | IME composition for CJK and other input methods | Window/IME | surface |  |  | P0 | wired | brief §4 | Commit inserts the composed text; the preedit is not drawn in place yet (unverified). |
| UI.Window.Clipboard | System clipboard: text, rich text (HTML), images | Window/Clipboard | surface |  |  | P0 | wired | brief §4 | desktop: copy puts text with HTML (tables as tables) or the copied picture on the system clipboard; paste reads HTML and pictures (arboard through the host); the web build stays plain text |
| UI.Window.DragDrop | Drag & drop of files, images and text onto the page; pages/sections in the navigation | Window/Drag and Drop | surface |  |  | P1 | missing | brief §4; GK |  |
| UI.Window.FileDialogs | Native file dialogs through the XDG desktop portal | Window/File Dialogs | surface |  |  | P0 | missing | brief §4 |  |
| UI.Window.Accessibility | Screen reader support (AccessKit) for panes, page list and canvas | Window/Accessibility | surface |  |  | P1 | wired | brief §6 M4 | Controls expose labels through AccessKit (the UI tests query them); not yet tried with a screen reader (unverified). |

## Quick Access Toolbar

Controls that can sit in the Quick Access Toolbar by default or from its customization list (FLUENT19 "Quick Access Toolbar").

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| QAT.ViewGoBack | Back | Quick Access Toolbar/Back | button | `Alt+Left` | nav.back | P0 | wired | FLUENT19:ViewGoBack |  |
| QAT.ViewGoForward | Forward | Quick Access Toolbar/Forward | button | `Alt+Right` | nav.forward | P1 | wired | FLUENT19:ViewGoForward |  |
| QAT.UndoButton | Undo | Quick Access Toolbar/Undo | button | `Mod+Z` | edit.undo | P0 | wired | FLUENT19:UndoButton |  |
| QAT.RedoButton | Redo | Quick Access Toolbar/Redo | button | `Mod+Y` | edit.redo | P0 | wired | FLUENT19:RedoButton |  |
| QAT.FilePrint | Print | Quick Access Toolbar/Print | button | `Mod+P` | file.print | P1 | missing | FLUENT19:FilePrint |  |
| QAT.FilePrintPreview | Print Preview | Quick Access Toolbar/Print Preview | button |  | file.printPreview | P2 | missing | FLUENT19:FilePrintPreview |  |
| QAT.FavoritePen1 | Favorite Pen 1 | Quick Access Toolbar/Favorite Pen 1 | toggle |  | draw.favoritePen1 | P2 | missing | FLUENT19:FavoritePen1 |  |
| QAT.FavoritePen2 | Favorite Pen 2 | Quick Access Toolbar/Favorite Pen 2 | toggle |  | draw.favoritePen2 | P2 | missing | FLUENT19:FavoritePen2 |  |
| QAT.FavoritePen3 | Favorite Pen 3 | Quick Access Toolbar/Favorite Pen 3 | toggle |  | draw.favoritePen3 | P2 | missing | FLUENT19:FavoritePen3 |  |
| QAT.FavoritePen4 | Favorite Pen 4 | Quick Access Toolbar/Favorite Pen 4 | toggle |  | draw.favoritePen4 | P2 | missing | FLUENT19:FavoritePen4 |  |
| QAT.FavoriteHighlighter1 | Favorite Highlighter 1 | Quick Access Toolbar/Favorite Highlighter 1 | toggle |  | draw.favoriteHighlighter1 | P2 | missing | FLUENT19:FavoriteHighlighter1 |  |
| QAT.PointerModeOptions | Pen and Pointer Modes | Quick Access Toolbar/Pen and Pointer Modes | gallery |  | draw.pointerMode | P2 | missing | FLUENT19:PointerModeOptions |  |
| QAT.ShapeSmileyFace | Shapes | Quick Access Toolbar/Shapes | menu |  | draw.shapes | P2 | missing | FLUENT19:ShapeSmileyFace |  |

## Ribbon › Home

The Home tab, group by group, in ribbon order. Rows from the Microsoft 365 app that the 2019 control list lacks are marked in Source.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| R.Home.Clipboard.PasteMenu | Paste | Home/Clipboard/Paste | split | `Mod+V` | edit.paste | P0 | wired | FLUENT19:PasteMenu |  |
| R.Home.Clipboard.Paste | Paste | Home/Clipboard/Paste | button | `Mod+V` | edit.paste | P0 | wired | FLUENT19:Paste |  |
| R.Home.Clipboard.PasteGallery | Paste Options | Home/Clipboard/Paste/Paste Options | gallery |  | edit.pasteOptions | P0 | wired | FLUENT19:PasteGallery |  |
| R.Home.Clipboard.Cut | Cut | Home/Clipboard/Cut | button | `Mod+X` | edit.cut | P0 | verified | FLUENT19:Cut |  |
| R.Home.Clipboard.CopyOneNote | Copy | Home/Clipboard/Copy | button | `Mod+C` | edit.copy | P0 | wired | FLUENT19:CopyOneNote |  |
| R.Home.Clipboard.FormatPainter | Format Painter | Home/Clipboard/Format Painter | control |  | edit.formatPainter | P1 | wired | FLUENT19:FormatPainter |  |
| R.Home.BasicText.Font | Font | Home/Basic Text/Font | combo |  | format.font | P0 | wired | FLUENT19:Font |  |
| R.Home.BasicText.FontSize | Font Size | Home/Basic Text/Font Size | combo |  | format.size | P0 | wired | FLUENT19:FontSize |  |
| R.Home.BasicText.BulletsGallery | Bullets | Home/Basic Text/Bullets | gallery | `Mod+.` | para.bullets | P0 | verified | FLUENT19:BulletsGallery |  |
| R.Home.BasicText.NumberingGallery | Numbering | Home/Basic Text/Numbering | gallery | `Mod+/` | para.numbering | P0 | verified | FLUENT19:NumberingGallery |  |
| R.Home.BasicText.NumberingCustomize | Numbering Customization | Home/Basic Text/Numbering/Numbering Customization | toggle |  | para.numberingCustomize | P2 | missing | FLUENT19:NumberingCustomize |  |
| R.Home.BasicText.OutdentClassic | Decrease Indent Position | Home/Basic Text/Decrease Indent Position | button | `Alt+Shift+Left / Shift+Tab` | para.outdent | P0 | verified | FLUENT19:OutdentClassic |  |
| R.Home.BasicText.IndentClassic | Increase Indent Position | Home/Basic Text/Increase Indent Position | button | `Alt+Shift+Right / Tab` | para.indent | P0 | wired | FLUENT19:IndentClassic |  |
| R.Home.BasicText.ClearFormatting | Clear All Formatting | Home/Basic Text/Clear All Formatting | button | `Mod+Shift+N` | format.clear | P0 | wired | FLUENT19:ClearFormatting |  |
| R.Home.BasicText.Bold | Bold | Home/Basic Text/Bold | toggle | `Mod+B` | format.bold | P0 | verified | FLUENT19:Bold |  |
| R.Home.BasicText.Italic | Italic | Home/Basic Text/Italic | toggle | `Mod+I` | format.italic | P0 | verified | FLUENT19:Italic |  |
| R.Home.BasicText.Underline | Underline | Home/Basic Text/Underline | toggle | `Mod+U` | format.underline | P0 | verified | FLUENT19:Underline |  |
| R.Home.BasicText.Strikethrough | Strikethrough | Home/Basic Text/Strikethrough | toggle | `Alt+-` | format.strikethrough | P0 | verified | FLUENT19:Strikethrough |  |
| R.Home.BasicText.ExtendedTextFormattingMenu | Subscript / Superscript | Home/Basic Text/Subscript / Superscript | split |  | format.subscript | P1 | wired | FLUENT19:ExtendedTextFormattingMenu |  |
| R.Home.BasicText.ExtendedTextFormattingMostRecenltyUsed | Most Recently Used (Subscript/Superscript) | Home/Basic Text/Subscript / Superscript/Most Recently Used (Subscript/Superscript) | toggle |  |  | P2 | missing | FLUENT19:ExtendedTextFormattingMostRecenltyUsed |  |
| R.Home.BasicText.Subscript | Subscript | Home/Basic Text/Subscript / Superscript/Subscript | toggle | `Mod+=` | format.subscript | P1 | verified | FLUENT19:Subscript |  |
| R.Home.BasicText.Superscript | Superscript | Home/Basic Text/Subscript / Superscript/Superscript | toggle | `Mod+Shift+Plus` | format.superscript | P1 | verified | FLUENT19:Superscript |  |
| R.Home.BasicText.TextHighlightColorPicker | Text Highlight Color | Home/Basic Text/Text Highlight Color | gallery | `Mod+Alt+H` | format.highlight | P0 | wired | FLUENT19:TextHighlightColorPicker |  |
| R.Home.BasicText.HighlightColorNone | No Color (highlight) | Home/Basic Text/Text Highlight Color/No Color (highlight) | button |  | format.highlightNone | P0 | wired | FLUENT19:HighlightColorNone |  |
| R.Home.BasicText.FontColorPicker | Font Color | Home/Basic Text/Font Color | gallery |  | format.color | P0 | wired | FLUENT19:FontColorPicker |  |
| R.Home.BasicText.ParagraphAlignmentMenu | Paragraph Alignment | Home/Basic Text/Paragraph Alignment | menu |  |  | P0 | missing | FLUENT19:ParagraphAlignmentMenu |  |
| R.Home.BasicText.AlignLeft | Align Left | Home/Basic Text/Paragraph Alignment/Align Left | toggle | `Mod+L` | para.alignLeft | P0 | verified | FLUENT19:AlignLeft |  |
| R.Home.BasicText.AlignCenter | Center | Home/Basic Text/Paragraph Alignment/Center | toggle |  | para.alignCenter | P0 | verified | FLUENT19:AlignCenter |  |
| R.Home.BasicText.AlignRight | Align Right | Home/Basic Text/Paragraph Alignment/Align Right | toggle | `Mod+R` | para.alignRight | P0 | verified | FLUENT19:AlignRight |  |
| R.Home.BasicText.ParagraphSpacingOptions | Paragraph Spacing Options | Home/Basic Text/Paragraph Alignment/Paragraph Spacing Options | button |  | para.spacingDialog | P1 | wired | FLUENT19:ParagraphSpacingOptions |  |
| R.Home.BasicText.TextDirectionLeftToRight | Left-to-Right | Home/Basic Text/Paragraph Alignment/Left-to-Right | toggle |  | para.ltr | P2 | wired | FLUENT19:TextDirectionLeftToRight |  |
| R.Home.BasicText.TextDirectionRightToLeft | Right-to-Left | Home/Basic Text/Paragraph Alignment/Right-to-Left | toggle |  | para.rtl | P2 | wired | FLUENT19:TextDirectionRightToLeft |  |
| R.Home.BasicText.Delete | Delete | Home/Basic Text/Delete | button | `Delete` | edit.delete | P0 | wired | FLUENT19:Delete |  |
| R.Home.Styles.QuickStylesGallery | Styles | Home/Styles | gallery |  | style.apply | P0 | wired | FLUENT19:QuickStylesGallery |  |
| R.Home.Styles.ClearFormatting | Clear All Formatting | Home/Styles/Clear All Formatting | button | `Mod+Shift+N` | format.clear | P0 | wired | FLUENT19:ClearFormatting |  |
| R.Home.Tags.TagsGallery | Tags | Home/Tags | gallery |  | tag.toggle | P0 | wired | FLUENT19:TagsGallery |  |
| R.Home.Tags.TagsCustomize | Customize Tags | Home/Tags/Customize Tags | button |  | tag.customize | P1 | wired | FLUENT19:TagsCustomize |  |
| R.Home.Tags.TagsRemoveAll | Remove Tag | Home/Tags/Remove Tag | button | `Mod+0` | tag.removeAll | P0 | wired | FLUENT19:TagsRemoveAll |  |
| R.Home.Tags.ToDoTag | To Do Tag | Home/Tags/To Do Tag | button | `Mod+1` | tag.todo | P0 | wired | FLUENT19:ToDoTag |  |
| R.Home.Tags.FindTaggedNotes | Find Tags | Home/Tags/Find Tags | toggle |  | tag.findTags | P0 | verified | FLUENT19:FindTaggedNotes |  |
| R.Home.Tags.OutlookTasksMenu | Outlook Tasks | Home/Tags/Outlook Tasks | menu |  |  | P3 | out-of-scope | FLUENT19:OutlookTasksMenu |  |
| R.Home.Tags.OutlookTaskTodayCtxItem | Outlook Task: Today | Home/Tags/Outlook Tasks/Outlook Task: Today | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskTodayCtxItem |  |
| R.Home.Tags.OutlookTaskTomorrowCtxItem | Outlook Task: Tomorrow | Home/Tags/Outlook Tasks/Outlook Task: Tomorrow | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskTomorrowCtxItem |  |
| R.Home.Tags.OutlookTaskThisWeekCtxItem | Outlook Task: This Week | Home/Tags/Outlook Tasks/Outlook Task: This Week | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskThisWeekCtxItem |  |
| R.Home.Tags.OutlookTaskNextWeekCtxItem | Outlook Task: Next Week | Home/Tags/Outlook Tasks/Outlook Task: Next Week | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskNextWeekCtxItem |  |
| R.Home.Tags.OutlookTaskLaterCtxItem | Outlook Task: No Date | Home/Tags/Outlook Tasks/Outlook Task: No Date | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskLaterCtxItem |  |
| R.Home.Tags.OutlookTaskCustomDateCtxItem | Outlook Task: Custom | Home/Tags/Outlook Tasks/Outlook Task: Custom | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskCustomDateCtxItem |  |
| R.Home.Tags.OutlookTaskMarkComplete | Mark Complete (Outlook task) | Home/Tags/Outlook Tasks/Mark Complete (Outlook task) | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskMarkComplete |  |
| R.Home.Tags.OutlookTaskRemove | Delete Outlook Task | Home/Tags/Outlook Tasks/Delete Outlook Task | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskRemove |  |
| R.Home.Tags.OutlookTaskOpen | Open Task in Outlook | Home/Tags/Outlook Tasks/Open Task in Outlook | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskOpen |  |
| R.Home.Email.SendNotesViaEmail | Email Page | Home/Email/Email Page | button | `Mod+Shift+E` | share.emailPage | P2 | missing | FLUENT19:SendNotesViaEmail |  |
| R.Home.Meetings.MeetingDetailsInsertGallery | Meeting Details | Home/Meetings/Meeting Details | gallery |  |  | P3 | out-of-scope | FLUENT19:MeetingDetailsInsertGallery |  |
| R.Home.Meetings.MeetingDetailsOtherDayInsert | Choose a Meeting from Another Day | Home/Meetings/Meeting Details/Choose a Meeting from Another Day | button |  |  | P3 | out-of-scope | FLUENT19:MeetingDetailsOtherDayInsert |  |
| R.Home.Meetings.RefreshMeetingDetails | Refresh Meeting Details | Home/Meetings/Meeting Details/Refresh Meeting Details | button |  |  | P3 | out-of-scope | FLUENT19:RefreshMeetingDetails |  |
| R.Home.Voice.Dictate | Dictate | Home/Voice/Dictate | split |  | home.dictate | P3 | out-of-scope | GK (Microsoft 365 OneNote) | needs a speech engine; unverified placement |
| R.Home.Clipboard.PasteKeepSource | Paste: Keep Source Formatting | Home/Clipboard/Paste/Keep Source Formatting | button |  | edit.pasteKeepSource | P0 | wired | FLUENT19:PasteSourceFormatting (in the Paste Options gallery) |  |
| R.Home.Clipboard.PasteMerge | Paste: Merge Formatting | Home/Clipboard/Paste/Merge Formatting | button |  | edit.pasteMerge | P1 | wired | FLUENT19:PasteMergeFormatting |  |
| R.Home.Clipboard.PasteText | Paste: Keep Text Only | Home/Clipboard/Paste/Keep Text Only | button | `Mod+Shift+V` | edit.pasteTextOnly | P0 | wired | FLUENT19:PasteTextOnly; KBD365 |  |
| R.Home.Clipboard.PastePicture | Paste: Picture | Home/Clipboard/Paste/Picture | button |  | edit.pastePicture | P2 | wired | FLUENT19:PasteAsPicture | The clipboard's picture, even when text or HTML came with it; pictures NoteCraft copied come back as pictures; text alone is refused (pasting text as a picture of itself waits for the renderer; unverified) |
| R.Home.Styles.Heading1 | Style: Heading 1 | Home/Styles/Heading 1 | gallery-item | `Mod+Alt+1` | style.heading1 | P0 | wired | GK (styles gallery contents); KBD365 for the keys | built-in style list from user-visible UI; unverified order |
| R.Home.Styles.Heading2 | Style: Heading 2 | Home/Styles/Heading 2 | gallery-item | `Mod+Alt+2` | style.heading2 | P0 | wired | GK (styles gallery contents); KBD365 for the keys | built-in style list from user-visible UI; unverified order |
| R.Home.Styles.Heading3 | Style: Heading 3 | Home/Styles/Heading 3 | gallery-item | `Mod+Alt+3` | style.heading3 | P0 | wired | GK (styles gallery contents); KBD365 for the keys | built-in style list from user-visible UI; unverified order |
| R.Home.Styles.Heading4 | Style: Heading 4 | Home/Styles/Heading 4 | gallery-item | `Mod+Alt+4` | style.heading4 | P0 | wired | GK (styles gallery contents); KBD365 for the keys | built-in style list from user-visible UI; unverified order |
| R.Home.Styles.Heading5 | Style: Heading 5 | Home/Styles/Heading 5 | gallery-item | `Mod+Alt+5` | style.heading5 | P0 | wired | GK (styles gallery contents); KBD365 for the keys | built-in style list from user-visible UI; unverified order |
| R.Home.Styles.Heading6 | Style: Heading 6 | Home/Styles/Heading 6 | gallery-item | `Mod+Alt+6` | style.heading6 | P0 | wired | GK (styles gallery contents); KBD365 for the keys | built-in style list from user-visible UI; unverified order |
| R.Home.Styles.PageTitle | Style: Page Title | Home/Styles/Page Title | gallery-item |  | style.pageTitle | P0 | wired | GK (styles gallery contents); KBD365 for the keys | built-in style list from user-visible UI; unverified order |
| R.Home.Styles.Citation | Style: Citation | Home/Styles/Citation | gallery-item |  | style.citation | P0 | wired | GK (styles gallery contents); KBD365 for the keys | built-in style list from user-visible UI; unverified order |
| R.Home.Styles.Quote | Style: Quote | Home/Styles/Quote | gallery-item |  | style.quote | P0 | wired | GK (styles gallery contents); KBD365 for the keys | built-in style list from user-visible UI; unverified order |
| R.Home.Styles.Code | Style: Code | Home/Styles/Code | gallery-item |  | style.code | P0 | wired | GK (styles gallery contents); KBD365 for the keys | built-in style list from user-visible UI; unverified order |
| R.Home.Styles.Normal | Style: Normal | Home/Styles/Normal | gallery-item |  | style.normal | P0 | wired | GK (styles gallery contents); KBD365 for the keys | built-in style list from user-visible UI; unverified order |
| R.Home.Tags.ToDo | Tag: To Do | Home/Tags/To Do | gallery-item | `Mod+1` | tag.todo | P0 | wired | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.Important | Tag: Important | Home/Tags/Important | gallery-item | `Mod+2` | tag.important | P0 | wired | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.Question | Tag: Question | Home/Tags/Question | gallery-item | `Mod+3` | tag.question | P0 | wired | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.RememberForLater | Tag: Remember for later | Home/Tags/Remember for later | gallery-item | `Mod+4` | tag.rememberForLater | P0 | wired | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.Definition | Tag: Definition | Home/Tags/Definition | gallery-item | `Mod+5` | tag.definition | P0 | wired | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.Highlight | Tag: Highlight | Home/Tags/Highlight | gallery-item | `Mod+6` | tag.highlight | P0 | wired | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.Contact | Tag: Contact | Home/Tags/Contact | gallery-item | `Mod+7` | tag.contact | P0 | wired | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.Address | Tag: Address | Home/Tags/Address | gallery-item | `Mod+8` | tag.address | P0 | wired | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.PhoneNumber | Tag: Phone number | Home/Tags/Phone number | gallery-item | `Mod+9` | tag.phoneNumber | P0 | wired | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.WebSiteToVisit | Tag: Web site to visit | Home/Tags/Web site to visit | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.Idea | Tag: Idea | Home/Tags/Idea | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.Password | Tag: Password | Home/Tags/Password | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.Critical | Tag: Critical | Home/Tags/Critical | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.ProjectA | Tag: Project A | Home/Tags/Project A | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.ProjectB | Tag: Project B | Home/Tags/Project B | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.MovieToSee | Tag: Movie to see | Home/Tags/Movie to see | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.BookToRead | Tag: Book to read | Home/Tags/Book to read | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.MusicToListenTo | Tag: Music to listen to | Home/Tags/Music to listen to | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.SourceForArticle | Tag: Source for article | Home/Tags/Source for article | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.RememberForBlog | Tag: Remember for blog | Home/Tags/Remember for blog | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.DiscussWithPersonA | Tag: Discuss with <Person A> | Home/Tags/Discuss with <Person A> | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.DiscussWithPersonB | Tag: Discuss with <Person B> | Home/Tags/Discuss with <Person B> | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.DiscussWithManager | Tag: Discuss with manager | Home/Tags/Discuss with manager | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.SendInEmail | Tag: Send in email | Home/Tags/Send in email | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.ScheduleMeeting | Tag: Schedule meeting | Home/Tags/Schedule meeting | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.CallBack | Tag: Call back | Home/Tags/Call back | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.ToDoPriority1 | Tag: To Do priority 1 | Home/Tags/To Do priority 1 | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.ToDoPriority2 | Tag: To Do priority 2 | Home/Tags/To Do priority 2 | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |
| R.Home.Tags.ClientRequest | Tag: Client request | Home/Tags/Client request | gallery-item |  |  | P1 | missing | GK (built-in tag list, user-visible); KBD365 for Ctrl+1–9 | built-in tag list and order from user-visible UI; unverified |

## Ribbon › Insert

The Insert tab, group by group, in ribbon order. Rows from the Microsoft 365 app that the 2019 control list lacks are marked in Source.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| R.Insert.Insert.PenInsertSpace | Insert Space | Insert/Insert Space | toggle |  | insert.space | P1 | wired | FLUENT19:PenInsertSpace |  |
| R.Insert.Tables.TableInsertGallery | Table | Insert/Tables/Table | gallery |  | insert.table | P1 | verified | FLUENT19:TableInsertGallery |  |
| R.Insert.Tables.TableInsert | Insert Table | Insert/Tables/Table/Insert Table | button |  | insert.tableDialog | P1 | verified | FLUENT19:TableInsert |  |
| R.Insert.Tables.InsertNewExcelSpreadsheetWithPreview | New Excel Spreadsheet | Insert/Tables/Table/New Excel Spreadsheet | button |  | insert.newSpreadsheet | P3 | missing | FLUENT19:InsertNewExcelSpreadsheetWithPreview |  |
| R.Insert.Files.PrintoutFromFileInsert | File Printout | Insert/Files/File Printout | button |  | insert.printout | P1 | wired | FLUENT19:PrintoutFromFileInsert | the file is attached and each page follows as a picture at its print size, rendered by hayro at 108 dpi (≤ 200 pages, ≤ 8 Mpx a page); PDF files and pictures only (unverified: the reference prints any file through a printer driver) |
| R.Insert.Files.FileAttachmentInsert | File Attachment | Insert/Files/File Attachment | button |  | insert.file | P1 | wired | FLUENT19:FileAttachmentInsert | attaches the file directly as an icon with its name; decision D3 (dropping a PDF asks first, `BEH.Paste.Files`) |
| R.Insert.Files.InsertExcelSpreadsheetMenu | Spreadsheet | Insert/Files/Spreadsheet | menu |  |  | P2 | missing | FLUENT19:InsertExcelSpreadsheetMenu |  |
| R.Insert.Files.InsertExistingExcelSpreadsheet | Existing Excel Spreadsheet | Insert/Files/Spreadsheet/Existing Excel Spreadsheet | button |  | insert.existingSpreadsheet | P2 | missing | FLUENT19:InsertExistingExcelSpreadsheet |  |
| R.Insert.Files.InsertNewExcelSpreadsheetWithPreview | New Excel Spreadsheet | Insert/Files/Spreadsheet/New Excel Spreadsheet | button |  | insert.newSpreadsheet | P3 | missing | FLUENT19:InsertNewExcelSpreadsheetWithPreview |  |
| R.Insert.Files.InsertVisioDrawingMenu | Diagram | Insert/Files/Diagram | menu |  |  | P3 | missing | FLUENT19:InsertVisioDrawingMenu |  |
| R.Insert.Files.InsertExistingVisioDrawing | Existing Visio Diagram | Insert/Files/Diagram/Existing Visio Diagram | button |  | insert.existingDiagram | P3 | missing | FLUENT19:InsertExistingVisioDrawing |  |
| R.Insert.Files.InsertNewVisioDrawingWithPreview | New Visio Diagram | Insert/Files/Diagram/New Visio Diagram | button |  |  | P3 | out-of-scope | FLUENT19:InsertNewVisioDrawingWithPreview |  |
| R.Insert.Images.ScreenCaptureInsert | Screen Clipping | Insert/Images/Screen Clipping | button |  | insert.screenClipping | P1 | wired | FLUENT19:ScreenCaptureInsert | Linux: the XDG Screenshot portal asks for the area (the desktop draws the selection; NoteCraft stays open, the reference hides itself); a "Screen clipping taken:" line with the date and time goes below the picture (wording unverified); other systems: disabled with a reason |
| R.Insert.Images.PictureFromFileInsert | Pictures | Insert/Images/Pictures | button |  | insert.picture | P1 | wired | FLUENT19:PictureFromFileInsert | PNG, JPEG, GIF (first frame), BMP, WebP; inserted at its 96-dpi pixel size, at most 468 pt wide, selected (unverified size rule); decision D3 |
| R.Insert.Images.ClipArtInsertDialog | Online Pictures | Insert/Images/Online Pictures | button |  | insert.onlinePicture | P3 | missing | FLUENT19:ClipArtInsertDialog |  |
| R.Insert.Addins.OfficeExtensionsAppStore | Get Add-ins (Store) | Insert/Add-ins/Get Add-ins (Store) | button |  |  | P3 | out-of-scope | FLUENT19:OfficeExtensionsAppStore |  |
| R.Insert.Addins.OfficeExtensionsGallery2 | My Add-ins | Insert/Add-ins/My Add-ins | gallery |  | plugins.manage | P3 | missing | FLUENT19:OfficeExtensionsGallery2 |  |
| R.Insert.Addins.OfficeExtensionsDialog | See All (add-ins) | Insert/Add-ins/My Add-ins/See All (add-ins) | button |  | plugins.manage | P3 | missing | FLUENT19:OfficeExtensionsDialog |  |
| R.Insert.Addins.OfficeExtensionsFeaturedApp1 | Featured Add-in 1 | Insert/Add-ins/Featured Add-in 1 | button |  |  | P3 | out-of-scope | FLUENT19:OfficeExtensionsFeaturedApp1 |  |
| R.Insert.Addins.OfficeExtensionsFeaturedApp2 | Featured Add-in 2 | Insert/Add-ins/Featured Add-in 2 | button |  |  | P3 | out-of-scope | FLUENT19:OfficeExtensionsFeaturedApp2 |  |
| R.Insert.Addins.OfficeExtensionsFeaturedApp3 | Featured Add-in 3 | Insert/Add-ins/Featured Add-in 3 | button |  |  | P3 | out-of-scope | FLUENT19:OfficeExtensionsFeaturedApp3 |  |
| R.Insert.Addins.OfficeExtensionsFeaturedApp4 | Featured Add-in 4 | Insert/Add-ins/Featured Add-in 4 | button |  |  | P3 | out-of-scope | FLUENT19:OfficeExtensionsFeaturedApp4 |  |
| R.Insert.Addins.OfficeExtensionsFeaturedApp5 | Featured Add-in 5 | Insert/Add-ins/Featured Add-in 5 | button |  |  | P3 | out-of-scope | FLUENT19:OfficeExtensionsFeaturedApp5 |  |
| R.Insert.Addins.OfficeExtensionsFeaturedApp6 | Featured Add-in 6 | Insert/Add-ins/Featured Add-in 6 | button |  |  | P3 | out-of-scope | FLUENT19:OfficeExtensionsFeaturedApp6 |  |
| R.Insert.Media.InsertOnlineVideo | Online Video | Insert/Media/Online Video | button |  | insert.onlineVideo | P3 | missing | FLUENT19:InsertOnlineVideo |  |
| R.Insert.Links.HyperlinkCreateHere | Link | Insert/Links/Link | button | `Mod+K` | insert.link | P0 | verified | FLUENT19:HyperlinkCreateHere |  |
| R.Insert.Recording.AudioRecordingInsert | Record Audio | Insert/Recording/Record Audio | toggle |  | insert.audio | P2 | missing | FLUENT19:AudioRecordingInsert |  |
| R.Insert.Recording.VideoRecordingInsert | Record Video | Insert/Recording/Record Video | toggle |  | insert.video | P2 | missing | FLUENT19:VideoRecordingInsert |  |
| R.Insert.TimeStamp.DateInsert | Date | Insert/Time Stamp/Date | button | `Alt+Shift+D` | insert.date | P1 | verified | FLUENT19:DateInsert |  |
| R.Insert.TimeStamp.TimeInsert | Time | Insert/Time Stamp/Time | button | `Alt+Shift+T` | insert.time | P1 | verified | FLUENT19:TimeInsert |  |
| R.Insert.TimeStamp.DateAndTimeInsertOneNote | Date & Time | Insert/Time Stamp/Date & Time | button | `Alt+Shift+F` | insert.dateTime | P1 | verified | FLUENT19:DateAndTimeInsertOneNote |  |
| R.Insert.Pages.PageTemplatesSplitButton | Page Templates | Insert/Pages/Page Templates | split |  | insert.pageTemplates | P1 | missing | FLUENT19:PageTemplatesSplitButton |  |
| R.Insert.Pages.PageTemplatesTaskpane | Page Templates… | Insert/Pages/Page Templates/Page Templates… | toggle |  | insert.pageTemplates | P1 | missing | FLUENT19:PageTemplatesTaskpane |  |
| R.Insert.Pages.StationeryMostRecentlyUsedList | Recently Used Templates | Insert/Pages/Page Templates/Recently Used Templates | gallery |  | insert.recentTemplate | P2 | missing | FLUENT19:StationeryMostRecentlyUsedList |  |
| R.Insert.Pages.NewPageFromStationery | Page from Template | Insert/Pages/Page Templates/Page from Template | button |  | page.newFromTemplate | P2 | missing | FLUENT19:NewPageFromStationery |  |
| R.Insert.Symbols.InsertBuildingBlocksEquationsGallery | Equation | Insert/Symbols/Equation | gallery | `Alt+=` | insert.equation | P2 | missing | FLUENT19:InsertBuildingBlocksEquationsGallery |  |
| R.Insert.Symbols.EquationInsertNew | Insert New Equation | Insert/Symbols/Equation/Insert New Equation | button | `Alt+=` | insert.equation | P2 | missing | FLUENT19:EquationInsertNew |  |
| R.Insert.Symbols.SymbolInsertGallery | Symbol | Insert/Symbols/Symbol | gallery |  | insert.symbol | P2 | wired | FLUENT19:SymbolInsertGallery |  |
| R.Insert.Symbols.SymbolsDialog | More Symbols… | Insert/Symbols/Symbol/More Symbols… | button |  | insert.symbolDialog | P2 | verified | FLUENT19:SymbolsDialog |  |
| R.Insert.Stickers.Sticker | Sticker | Insert/Stickers/Sticker | gallery |  | insert.sticker | P3 | missing | KBD365 ("Insert a sticker": Alt+N, S) | unverified group name |

## Ribbon › Draw

The Draw tab, group by group, in ribbon order. Rows from the Microsoft 365 app that the 2019 control list lacks are marked in Source.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| R.Draw.Tools.SelectAndTypeMode | Type | Draw/Tools/Type | toggle |  | draw.type | P1 | wired | FLUENT19:SelectAndTypeMode |  |
| R.Draw.Tools.LassoSelect | Lasso Select | Draw/Tools/Lasso Select | toggle |  | draw.lasso | P1 | wired | FLUENT19:LassoSelect |  |
| R.Draw.Tools.PanningHand | Panning Hand | Draw/Tools/Panning Hand | toggle |  | draw.pan | P2 | wired | FLUENT19:PanningHand |  |
| R.Draw.Tools.EraserMenu | Eraser | Draw/Tools/Eraser | split |  | draw.eraser | P1 | wired | FLUENT19:EraserMenu | the Stroke Eraser is the default, unverified |
| R.Draw.Tools.EraserLastUsed | Eraser (last used) | Draw/Tools/Eraser/Eraser (last used) | toggle |  | draw.eraser | P1 | wired | FLUENT19:EraserLastUsed |  |
| R.Draw.Tools.SmallEraser | Small Eraser | Draw/Tools/Eraser/Small Eraser | toggle |  | draw.eraserSmall | P1 | wired | FLUENT19:SmallEraser | radius 2.5 pt, unverified |
| R.Draw.Tools.MediumEraser | Medium Eraser | Draw/Tools/Eraser/Medium Eraser | toggle |  | draw.eraserMedium | P1 | wired | FLUENT19:MediumEraser | radius 6 pt, unverified |
| R.Draw.Tools.LargeEraser | Large Eraser | Draw/Tools/Eraser/Large Eraser | toggle |  | draw.eraserLarge | P1 | wired | FLUENT19:LargeEraser | radius 12 pt, unverified |
| R.Draw.Tools.StrokeEraserOneNote | Stroke Eraser | Draw/Tools/Eraser/Stroke Eraser | toggle |  | draw.eraserStroke | P1 | wired | FLUENT19:StrokeEraserOneNote |  |
| R.Draw.Tools.InkStylesGallery | Pens | Draw/Tools/Pens | gallery |  | draw.pen | P1 | wired | FLUENT19:InkStylesGallery | default pens (black, red, blue 0.5 mm; yellow, green highlighters 5 mm) unverified; the desktop app remembers the gallery, the selected pen, the eraser size and the shape between runs (the tool starts as Type) |
| R.Draw.Tools.InkBiasMenu | Ink Mode | Draw/Tools/Pens/Ink Mode | menu |  |  | P2 | missing | FLUENT19:InkBiasMenu |  |
| R.Draw.Tools.InkingBiasAuto | Create Both Handwriting and Drawings | Draw/Tools/Pens/Ink Mode/Create Both Handwriting and Drawings | toggle |  | draw.inkModeAuto | P2 | missing | FLUENT19:InkingBiasAuto |  |
| R.Draw.Tools.InkingBiasDraw | Create Drawings Only | Draw/Tools/Pens/Ink Mode/Create Drawings Only | toggle |  | draw.inkModeDraw | P2 | missing | FLUENT19:InkingBiasDraw |  |
| R.Draw.Tools.InkingBiasWrite | Create Handwriting Only | Draw/Tools/Pens/Ink Mode/Create Handwriting Only | toggle |  | draw.inkModeWrite | P2 | missing | FLUENT19:InkingBiasWrite |  |
| R.Draw.Tools.PenAsPointerMode | Use Pen as Pointer | Draw/Tools/Pens/Ink Mode/Use Pen as Pointer | toggle |  | draw.penAsPointer | P2 | missing | FLUENT19:PenAsPointerMode |  |
| R.Draw.Tools.MoreColorAndThicknessOptions | Color & Thickness | Draw/Tools/Color & Thickness | button |  | draw.colorThickness | P1 | wired | FLUENT19:MoreColorAndThicknessOptions |  |
| R.Draw.Tools.FingerPaintingMode | Draw with Touch | Draw/Tools/Draw with Touch | toggle |  | draw.withTouch | P2 | missing | FLUENT19:FingerPaintingMode |  |
| R.Draw.Toolstouch.SelectAndTypeMode | Type | Draw/Tools (touch)/Type | toggle |  | draw.type | P3 | missing | FLUENT19:SelectAndTypeMode | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.LassoSelect | Lasso Select | Draw/Tools (touch)/Lasso Select | toggle |  | draw.lasso | P3 | missing | FLUENT19:LassoSelect | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.EraserMenu | Eraser | Draw/Tools (touch)/Eraser | split |  | draw.eraser | P3 | missing | FLUENT19:EraserMenu | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.EraserLastUsed | Eraser (last used) | Draw/Tools (touch)/Eraser/Eraser (last used) | toggle |  | draw.eraser | P3 | missing | FLUENT19:EraserLastUsed | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.SmallEraser | Small Eraser | Draw/Tools (touch)/Eraser/Small Eraser | toggle |  | draw.eraserSmall | P3 | missing | FLUENT19:SmallEraser | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.MediumEraser | Medium Eraser | Draw/Tools (touch)/Eraser/Medium Eraser | toggle |  | draw.eraserMedium | P3 | missing | FLUENT19:MediumEraser | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.LargeEraser | Large Eraser | Draw/Tools (touch)/Eraser/Large Eraser | toggle |  | draw.eraserLarge | P3 | missing | FLUENT19:LargeEraser | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.StrokeEraserOneNote | Stroke Eraser | Draw/Tools (touch)/Eraser/Stroke Eraser | toggle |  | draw.eraserStroke | P3 | missing | FLUENT19:StrokeEraserOneNote | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.InkStylesGallery | Pens | Draw/Tools (touch)/Pens | gallery |  | draw.pen | P3 | missing | FLUENT19:InkStylesGallery | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.InkBiasMenu | Ink Mode | Draw/Tools (touch)/Pens/Ink Mode | menu |  |  | P3 | missing | FLUENT19:InkBiasMenu | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.InkingBiasAuto | Create Both Handwriting and Drawings | Draw/Tools (touch)/Pens/Ink Mode/Create Both Handwriting and Drawings | toggle |  | draw.inkModeAuto | P3 | missing | FLUENT19:InkingBiasAuto | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.InkingBiasDraw | Create Drawings Only | Draw/Tools (touch)/Pens/Ink Mode/Create Drawings Only | toggle |  | draw.inkModeDraw | P3 | missing | FLUENT19:InkingBiasDraw | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.InkingBiasWrite | Create Handwriting Only | Draw/Tools (touch)/Pens/Ink Mode/Create Handwriting Only | toggle |  | draw.inkModeWrite | P3 | missing | FLUENT19:InkingBiasWrite | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.PenAsPointerMode | Use Pen as Pointer | Draw/Tools (touch)/Pens/Ink Mode/Use Pen as Pointer | toggle |  | draw.penAsPointer | P3 | missing | FLUENT19:PenAsPointerMode | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.MoreColorAndThicknessOptions | Color & Thickness | Draw/Tools (touch)/Color & Thickness | button |  | draw.colorThickness | P3 | missing | FLUENT19:MoreColorAndThicknessOptions | touch-mode layout of the Tools group (same commands) |
| R.Draw.Toolstouch.FingerPaintingMode | Draw with Touch | Draw/Tools (touch)/Draw with Touch | toggle |  | draw.withTouch | P3 | missing | FLUENT19:FingerPaintingMode | touch-mode layout of the Tools group (same commands) |
| R.Draw.Shapes.InkShapeGallery | Shapes | Draw/Shapes | gallery |  | draw.shapes | P1 | wired | FLUENT19:InkShapeGallery | line, arrow, double arrow, rectangle, oval, triangle, right triangle, diamond, parallelogram, drawn as ink with the selected pen; graph axes and 3-D shapes not yet (unverified list) |
| R.Draw.Shapes.ShapesDuplicate | Lock Drawing Mode | Draw/Shapes/Lock Drawing Mode | toggle |  | draw.lockDrawingMode | P2 | wired | FLUENT19:ShapesDuplicate | off: the Shape tool returns to Type after one shape (unverified) |
| R.Draw.Shapes.GridSnapTo | Snap to Grid | Draw/Shapes/Snap to Grid | toggle |  | draw.snapToGrid | P2 | missing | FLUENT19:GridSnapTo |  |
| R.Draw.Edit.PenInsertSpace | Insert Space | Draw/Edit/Insert Space | toggle |  | insert.space | P1 | wired | FLUENT19:PenInsertSpace |  |
| R.Draw.Edit.Delete | Delete | Draw/Edit/Delete | button | `Delete` | edit.delete | P0 | wired | FLUENT19:Delete |  |
| R.Draw.Edit.ObjectsArrangeMenu | Arrange | Draw/Edit/Arrange | menu |  |  | P1 | wired | FLUENT19:ObjectsArrangeMenu |  |
| R.Draw.Edit.ObjectBringForward | Bring Forward | Draw/Edit/Arrange/Bring Forward | button |  | object.bringForward | P1 | wired | FLUENT19:ObjectBringForward |  |
| R.Draw.Edit.ObjectBringToFront | Bring to Front | Draw/Edit/Arrange/Bring to Front | button |  | object.bringToFront | P1 | wired | FLUENT19:ObjectBringToFront |  |
| R.Draw.Edit.ObjectSendBackward | Send Backward | Draw/Edit/Arrange/Send Backward | button |  | object.sendBackward | P1 | wired | FLUENT19:ObjectSendBackward |  |
| R.Draw.Edit.ObjectSendToBack | Send to Back | Draw/Edit/Arrange/Send to Back | button |  | object.sendToBack | P1 | wired | FLUENT19:ObjectSendToBack |  |
| R.Draw.Edit.RotateMenuOneNote | Rotate | Draw/Edit/Rotate | menu |  |  | P1 | wired | FLUENT19:RotateMenuOneNote |  |
| R.Draw.Edit.RotateRight45OneNote | Rotate Right 45° | Draw/Edit/Rotate/Rotate Right 45° | button |  | object.rotateRight45 | P2 | wired | FLUENT19:RotateRight45OneNote |  |
| R.Draw.Edit.RotateLeft45OneNote | Rotate Left 45° | Draw/Edit/Rotate/Rotate Left 45° | button |  | object.rotateLeft45 | P2 | wired | FLUENT19:RotateLeft45OneNote |  |
| R.Draw.Edit.ObjectRotateRight90 | Rotate Right 90° | Draw/Edit/Rotate/Rotate Right 90° | button |  | object.rotateRight90 | P1 | wired | FLUENT19:ObjectRotateRight90 |  |
| R.Draw.Edit.ObjectRotateLeft90 | Rotate Left 90° | Draw/Edit/Rotate/Rotate Left 90° | button |  | object.rotateLeft90 | P1 | wired | FLUENT19:ObjectRotateLeft90 |  |
| R.Draw.Edit.ObjectFlipHorizontal | Flip Horizontal | Draw/Edit/Rotate/Flip Horizontal | button |  | object.flipHorizontal | P1 | wired | FLUENT19:ObjectFlipHorizontal |  |
| R.Draw.Edit.ObjectFlipVertical | Flip Vertical | Draw/Edit/Rotate/Flip Vertical | button |  | object.flipVertical | P1 | wired | FLUENT19:ObjectFlipVertical |  |
| R.Draw.Convert.SelectionToTextConvert | Ink to Text | Draw/Convert/Ink to Text | button |  | draw.inkToText | P2 | missing | FLUENT19:SelectionToTextConvert |  |
| R.Draw.Convert.SelectionToMathConvert | Ink to Math | Draw/Convert/Ink to Math | button |  | draw.inkToMath | P2 | missing | FLUENT19:SelectionToMathConvert |  |
| R.Draw.Tools.AddPen | Add Pen | Draw/Tools/Add Pen | button |  | draw.addPen | P1 | wired | GK (Microsoft 365 OneNote Draw tab) | unverified |
| R.Draw.Tools.Ruler | Ruler | Draw/Tools/Ruler | toggle |  | draw.ruler | P2 | wired | GK (Microsoft 365 OneNote Draw tab) | Decision D7: a straightedge over the page (54 × 900 pt, millimetre ticks with centimetres numbered, its angle in the middle), shown in the middle of the page's view; a pen stroke started within 12 pt of a long edge runs along it; session state, never saved or undone (size, scale, snap distance, look and starting angle unverified) |
| R.Draw.Convert.InkToShape | Ink to Shape | Draw/Convert/Ink to Shape | toggle |  | draw.inkToShape | P2 | missing | GK (Microsoft 365 OneNote Draw tab) | unverified |
| R.Draw.Replay.InkReplay | Ink Replay | Draw/Replay/Ink Replay | button |  | draw.replay | P2 | missing | GK (Microsoft 365 OneNote Draw tab) | unverified |
| R.Draw.Math.Math | Math (pane) | Draw/Convert/Math | button |  | draw.mathPane | P3 | missing | GK (Microsoft 365 OneNote Draw tab) | unverified |

## Ribbon › History

The History tab, group by group, in ribbon order. Rows from the Microsoft 365 app that the 2019 control list lacks are marked in Source.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| R.History.Unread.GoToNextUnreadPage | Next Unread | History/Unread/Next Unread | button |  | history.nextUnread | P2 | missing | FLUENT19:GoToNextUnreadPage |  |
| R.History.Unread.UnreadMenu | Mark as Read | History/Unread/Mark as Read | menu |  |  | P2 | missing | FLUENT19:UnreadMenu |  |
| R.History.Unread.MarkPageAsReadContextual | Mark as Read | History/Unread/Mark as Read | button |  | history.markRead | P2 | missing | FLUENT19:MarkPageAsReadContextual |  |
| R.History.Unread.MarkPageAsUnreadContextual | Mark as Unread | History/Unread/Mark as Read/Mark as Unread | button | `Mod+Q` | history.markUnread | P2 | missing | FLUENT19:MarkPageAsUnreadContextual |  |
| R.History.Unread.MarkAllPagesAsRead | Mark Notebook as Read | History/Unread/Mark as Read/Mark Notebook as Read | button |  | history.markNotebookRead | P2 | missing | FLUENT19:MarkAllPagesAsRead |  |
| R.History.Unread.UnreadNotebookChangesShow | Show Unread Changes in This Notebook | History/Unread/Mark as Read/Show Unread Changes in This Notebook | toggle |  | history.showUnread | P2 | missing | FLUENT19:UnreadNotebookChangesShow |  |
| R.History.Authors.FindByDateMenu | Recent Edits | History/Authors/Recent Edits | menu |  |  | P1 | missing | FLUENT19:FindByDateMenu |  |
| R.History.Authors.FindToday | Recent Edits: Today | History/Authors/Recent Edits/Recent Edits: Today | button |  | history.recentEdits | P1 | missing | FLUENT19:FindToday |  |
| R.History.Authors.FindSinceYesterday | Recent Edits: Since Yesterday | History/Authors/Recent Edits/Recent Edits: Since Yesterday | button |  | history.recentEdits | P1 | missing | FLUENT19:FindSinceYesterday |  |
| R.History.Authors.FindSinceSevenDays | Recent Edits: Last 7 Days | History/Authors/Recent Edits/Recent Edits: Last 7 Days | button |  | history.recentEdits | P1 | missing | FLUENT19:FindSinceSevenDays |  |
| R.History.Authors.FindLastTwoWeeks | Recent Edits: Last 14 Days | History/Authors/Recent Edits/Recent Edits: Last 14 Days | button |  | history.recentEdits | P1 | missing | FLUENT19:FindLastTwoWeeks |  |
| R.History.Authors.FindLastMonth | Recent Edits: Last 30 Days | History/Authors/Recent Edits/Recent Edits: Last 30 Days | button |  | history.recentEdits | P1 | missing | FLUENT19:FindLastMonth |  |
| R.History.Authors.FindLastThreeMonths | Recent Edits: Last 3 Months | History/Authors/Recent Edits/Recent Edits: Last 3 Months | button |  | history.recentEdits | P1 | missing | FLUENT19:FindLastThreeMonths |  |
| R.History.Authors.FindLasteSixMonths | Recent Edits: Last 6 Months | History/Authors/Recent Edits/Recent Edits: Last 6 Months | button |  | history.recentEdits | P1 | missing | FLUENT19:FindLasteSixMonths |  |
| R.History.Authors.FindAllChangesByDate | Recent Edits: All Pages Sorted by Date | History/Authors/Recent Edits/Recent Edits: All Pages Sorted by Date | button |  | history.recentEdits | P1 | missing | FLUENT19:FindAllChangesByDate |  |
| R.History.Authors.FindUnread | Find by Author | History/Authors/Find by Author | button |  | history.findByAuthor | P2 | missing | FLUENT19:FindUnread |  |
| R.History.Authors.AuthorHighlightingHide | Hide Authors | History/Authors/Hide Authors | toggle |  | history.hideAuthors | P2 | missing | FLUENT19:AuthorHighlightingHide |  |
| R.History.History.PageVersionSplitButton | Page Versions | History/Page Versions | split |  | history.pageVersions | P1 | missing | FLUENT19:PageVersionSplitButton |  |
| R.History.History.PageVersions | Page Versions | History/Page Versions | toggle |  | history.pageVersions | P1 | missing | FLUENT19:PageVersions |  |
| R.History.History.DeleteAllPageVersionsInSection | Delete All Versions in Section | History/Page Versions/Delete All Versions in Section | button |  | history.deleteVersionsSection | P2 | missing | FLUENT19:DeleteAllPageVersionsInSection |  |
| R.History.History.DeleteAllPageVersionsInFolder | Delete All Versions in Section Group | History/Page Versions/Delete All Versions in Section Group | button |  | history.deleteVersionsGroup | P2 | missing | FLUENT19:DeleteAllPageVersionsInFolder |  |
| R.History.History.DeleteAllPageVersionsInNotebook | Delete All Versions in Notebook | History/Page Versions/Delete All Versions in Notebook | button |  | history.deleteVersionsNotebook | P2 | missing | FLUENT19:DeleteAllPageVersionsInNotebook |  |
| R.History.History.DisableVersionHistory | Disable History for This Notebook | History/Page Versions/Disable History for This Notebook | toggle |  | history.disable | P2 | missing | FLUENT19:DisableVersionHistory |  |
| R.History.History.RecycleBinMenu | Notebook Recycle Bin | History/Notebook Recycle Bin | split |  | history.recycleBin | P0 | verified | FLUENT19:RecycleBinMenu |  |
| R.History.History.RecycleBin | Notebook Recycle Bin | History/Notebook Recycle Bin | toggle |  | history.recycleBin | P0 | wired | FLUENT19:RecycleBin |  |
| R.History.History.EmptyRecycleBin | Empty Recycle Bin | History/Notebook Recycle Bin/Empty Recycle Bin | button |  | history.emptyRecycleBin | P0 | verified | FLUENT19:EmptyRecycleBin |  |
| R.History.History.DisableVersionHistory2 | Disable History for This Notebook | History/Notebook Recycle Bin/Disable History for This Notebook | toggle |  | history.disable | P2 | missing | FLUENT19:DisableVersionHistory |  |

## Ribbon › Review

The Review tab, group by group, in ribbon order. Rows from the Microsoft 365 app that the 2019 control list lacks are marked in Source.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| R.Review.Proofing.SpellCheck | Spelling | Review/Proofing/Spelling | button | `F7` | review.spelling | P1 | missing | FLUENT19:SpellCheck |  |
| R.Review.Proofing.ResearchPane | Research | Review/Proofing/Research | toggle |  | review.research | P3 | missing | FLUENT19:ResearchPane |  |
| R.Review.Proofing.Thesaurus | Thesaurus | Review/Proofing/Thesaurus | button | `Shift+F7` | review.thesaurus | P2 | missing | FLUENT19:Thesaurus |  |
| R.Review.Accessibility.AccessibilityChecker | Check Accessibility | Review/Accessibility/Check Accessibility | button |  | review.accessibility | P2 | missing | FLUENT19:AccessibilityChecker |  |
| R.Review.Language.TranslateMenu | Translate | Review/Language/Translate | menu |  |  | P3 | missing | FLUENT19:TranslateMenu |  |
| R.Review.Language.TranslateSelected | Translate Selected Text | Review/Language/Translate/Translate Selected Text | button |  | review.translateSelection | P3 | missing | FLUENT19:TranslateSelected |  |
| R.Review.Language.MiniTranslator | Mini Translator | Review/Language/Translate/Mini Translator | toggle |  | review.miniTranslator | P3 | missing | FLUENT19:MiniTranslator |  |
| R.Review.Language.TranslationLanguageOptions | Choose Translation Language | Review/Language/Translate/Choose Translation Language | button |  | review.translationLanguage | P3 | missing | FLUENT19:TranslationLanguageOptions |  |
| R.Review.Language.LanguageCommands | Language | Review/Language | menu |  |  | P2 | missing | FLUENT19:LanguageCommands |  |
| R.Review.Language.SetLanguage | Set Proofing Language | Review/Language/Set Proofing Language | toggle |  | review.setLanguage | P2 | missing | FLUENT19:SetLanguage |  |
| R.Review.Language.LanguagePreferences | Language Preferences | Review/Language/Language Preferences | button |  | review.languagePreferences | P2 | missing | FLUENT19:LanguagePreferences |  |
| R.Review.Section.PasswordProtectSection | Password | Review/Section/Password | button |  | review.password | P1 | missing | FLUENT19:PasswordProtectSection |  |
| R.Review.Notes.LinkedNotes | Linked Notes | Review/Notes/Linked Notes | button |  | review.linkedNotes | P3 | missing | FLUENT19:LinkedNotes |  |

## Ribbon › View

The View tab, group by group, in ribbon order. Rows from the Microsoft 365 app that the 2019 control list lacks are marked in Source.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| R.View.Views.NormalViewOneNote | Normal View | View/Views/Normal View | toggle |  | view.normal | P0 | wired | FLUENT19:NormalViewOneNote |  |
| R.View.Views.FullPageView | Full Page View | View/Views/Full Page View | toggle | `F11` | view.fullPage | P0 | verified | FLUENT19:FullPageView |  |
| R.View.Views.CurrentWindowDock | Dock to Desktop | View/Views/Dock to Desktop | toggle | `Mod+Alt+D` | view.dock | P1 | missing | FLUENT19:CurrentWindowDock |  |
| R.View.PageSetup.PageColorGallery | Page Color | View/Page Setup/Page Color | gallery |  | view.pageColor | P1 | wired | FLUENT19:PageColorGallery |  |
| R.View.PageSetup.RuleLinesMenu | Rule Lines | View/Page Setup/Rule Lines | gallery | `Mod+Shift+R` | view.ruleLines | P1 | verified | FLUENT19:RuleLinesMenu |  |
| R.View.PageSetup.RuleLineColorMenu | Rule Line Color | View/Page Setup/Rule Lines/Rule Line Color | menu |  |  | P2 | missing | FLUENT19:RuleLineColorMenu |  |
| R.View.PageSetup.RuleLineColorGallery | Rule Line Color | View/Page Setup/Rule Lines/Rule Line Color | gallery |  | view.ruleLineColor | P2 | wired | FLUENT19:RuleLineColorGallery |  |
| R.View.PageSetup.NewPagesWithRuleLines | Always Create Pages with Rule Lines | View/Page Setup/Rule Lines/Always Create Pages with Rule Lines | toggle |  | view.newPagesWithRuleLines | P2 | wired | FLUENT19:NewPagesWithRuleLines |  |
| R.View.PageSetup.PageTitleHide | Hide Page Title | View/Page Setup/Hide Page Title | toggle |  | view.hidePageTitle | P1 | verified | FLUENT19:PageTitleHide |  |
| R.View.PageSetup.PaperSize | Paper Size | View/Page Setup/Paper Size | toggle |  | view.paperSize | P1 | missing | FLUENT19:PaperSize |  |
| R.View.Zoom.ZoomOut | Zoom Out | View/Zoom/Zoom Out | button | `Mod+-` | view.zoomOut | P0 | verified | FLUENT19:ZoomOut |  |
| R.View.Zoom.ZoomIn | Zoom In | View/Zoom/Zoom In | button | `Mod+Plus` | view.zoomIn | P0 | verified | FLUENT19:ZoomIn |  |
| R.View.Zoom.ZoomClassic | Zoom | View/Zoom | combo |  | view.zoom | P0 | wired | FLUENT19:ZoomClassic |  |
| R.View.Zoom.ZoomCurrent100 | 100% | View/Zoom/100% | button |  | view.zoom100 | P0 | verified | FLUENT19:ZoomCurrent100 |  |
| R.View.Zoom.ZoomPageWidth | Page Width | View/Zoom/Page Width | button |  | view.pageWidth | P1 | wired | FLUENT19:ZoomPageWidth |  |
| R.View.Window.WindowNew | New Window | View/Window/New Window | button | `Mod+M` | window.new | P1 | missing | FLUENT19:WindowNew |  |
| R.View.Window.NewDockedWindow | New Docked Window | View/Window/New Docked Window | button |  | window.newDocked | P2 | missing | FLUENT19:NewDockedWindow |  |
| R.View.Window.NewSideNote | New Quick Note | View/Window/New Quick Note | button | `Mod+Shift+M` | window.quickNote | P1 | missing | FLUENT19:NewSideNote |  |
| R.View.Window.WindowKeepOnTop | Always on Top | View/Window/Always on Top | toggle |  | window.alwaysOnTop | P2 | missing | FLUENT19:WindowKeepOnTop |  |
| R.View.Immersive.ImmersiveReader | Immersive Reader | View/Immersive/Immersive Reader | button |  | view.immersiveReader | P3 | missing | GK (Microsoft 365 OneNote View tab) | unverified |
| R.View.Views.SwitchBackground | Switch Background (dark mode page) | View/Views/Switch Background | toggle |  | view.switchBackground | P2 | missing | GK (Microsoft 365 OneNote dark mode) | unverified |
| R.View.Views.NavigationPanes | Navigation Panes (show/hide) | View/Views/Navigation Panes | toggle |  | view.navigationPanes | P1 | wired | GK (Microsoft 365 OneNote, 2023 navigation update) | Hides the notebook tree and the page column together, as the ☰ button does (unverified). |

## Ribbon › Help

The Help tab of the Microsoft 365 app (not in the 2019 control list).

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| R.Help.Help.Help | Help | Help/Help/Help | button | `F1` | help.help | P1 | missing | GK (Microsoft 365 Help tab); KBD (F1) | unverified group names |
| R.Help.Help.ContactSupport | Contact Support | Help/Help/Contact Support | button |  |  | P3 | out-of-scope | GK (Microsoft 365 Help tab) |  |
| R.Help.Help.Feedback | Feedback | Help/Help/Feedback | button |  | help.feedback | P3 | missing | GK (Microsoft 365 Help tab) |  |
| R.Help.Help.ShowTraining | Show Training | Help/Help/Show Training | button |  | help.training | P3 | missing | GK (Microsoft 365 Help tab) |  |
| R.Help.Help.WhatsNew | What's New | Help/Help/What's New | button |  | help.whatsNew | P3 | missing | GK (Microsoft 365 Help tab) |  |

## Contextual tab › Table Layout

Shown only while the selection is in a table / an equation / on a recording.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| CT.TableLayout.Select.SelectTableOneNote | Select Table | Table Layout/Select/Select Table | button |  | table.selectTable | P1 | verified | FLUENT19:SelectTableOneNote |  |
| CT.TableLayout.Select.SelectColumns | Select Columns | Table Layout/Select/Select Columns | button |  | table.selectColumns | P1 | verified | FLUENT19:SelectColumns |  |
| CT.TableLayout.Select.SelectRows | Select Rows | Table Layout/Select/Select Rows | button |  | table.selectRows | P1 | verified | FLUENT19:SelectRows |  |
| CT.TableLayout.Select.SelectCell | Select Cell | Table Layout/Select/Select Cell | button |  | table.selectCell | P1 | verified | FLUENT19:SelectCell |  |
| CT.TableLayout.Delete.DeleteTableOneNote | Delete Table | Table Layout/Delete/Delete Table | button |  | table.delete | P1 | verified | FLUENT19:DeleteTableOneNote |  |
| CT.TableLayout.Delete.DeleteColumnsOneNote | Delete Columns | Table Layout/Delete/Delete Columns | button |  | table.deleteColumns | P1 | verified | FLUENT19:DeleteColumnsOneNote |  |
| CT.TableLayout.Delete.DeleteRowsOneNote | Delete Rows | Table Layout/Delete/Delete Rows | button |  | table.deleteRows | P1 | verified | FLUENT19:DeleteRowsOneNote |  |
| CT.TableLayout.Insert.TableInsertRowsAbove | Insert Above | Table Layout/Insert/Insert Above | button |  | table.insertAbove | P1 | verified | FLUENT19:TableInsertRowsAbove |  |
| CT.TableLayout.Insert.TableInsertRowsBelow | Insert Below | Table Layout/Insert/Insert Below | button | `Mod+Enter` | table.insertBelow | P1 | verified | FLUENT19:TableInsertRowsBelow |  |
| CT.TableLayout.Insert.TableInsertColumnsLeft | Insert Left | Table Layout/Insert/Insert Left | button | `Mod+Alt+E` | table.insertLeft | P1 | verified | FLUENT19:TableInsertColumnsLeft |  |
| CT.TableLayout.Insert.TableInsertColumnsRight | Insert Right | Table Layout/Insert/Insert Right | button | `Mod+Alt+R` | table.insertRight | P1 | verified | FLUENT19:TableInsertColumnsRight |  |
| CT.TableLayout.Merge.MergeCells | Merge Cells | Table Layout/Merge/Merge Cells | button |  | table.merge | P1 | missing | FLUENT19:MergeCells |  |
| CT.TableLayout.Merge.UnmergeCells | Split Cells | Table Layout/Merge/Split Cells | button |  | table.split | P1 | missing | FLUENT19:UnmergeCells |  |
| CT.TableLayout.Format.TableBordersHide | Hide Borders | Table Layout/Format/Hide Borders | toggle |  | table.hideBorders | P1 | verified | FLUENT19:TableBordersHide |  |
| CT.TableLayout.Format.ShadingColorPicker | Shading | Table Layout/Format/Shading | gallery |  | table.shading | P1 | verified | FLUENT19:ShadingColorPicker |  |
| CT.TableLayout.Alignment.AlignLeft | Align Left | Table Layout/Alignment/Align Left | toggle | `Mod+L` | para.alignLeft | P0 | wired | FLUENT19:AlignLeft |  |
| CT.TableLayout.Alignment.AlignCenter | Center | Table Layout/Alignment/Center | toggle |  | para.alignCenter | P0 | wired | FLUENT19:AlignCenter |  |
| CT.TableLayout.Alignment.AlignRight | Align Right | Table Layout/Alignment/Align Right | toggle | `Mod+R` | para.alignRight | P0 | wired | FLUENT19:AlignRight |  |
| CT.TableLayout.Data.SortMenu | Sort | Table Layout/Data/Sort | menu |  |  | P1 | missing | FLUENT19:SortMenu |  |
| CT.TableLayout.Data.SortUp | Sort Ascending | Table Layout/Data/Sort/Sort Ascending | button |  | table.sortAscending | P1 | wired | FLUENT19:SortUp | Sorts the whole table by the caret's column, the header row excluded; unverified collation (numbers first, text ignoring case, empty cells last). |
| CT.TableLayout.Data.SortDown | Sort Descending | Table Layout/Data/Sort/Sort Descending | button |  | table.sortDescending | P1 | wired | FLUENT19:SortDown | As Sort Ascending, reversed (empty cells stay last); unverified collation. |
| CT.TableLayout.Data.ShowSortTableRowsDialog | Sort… | Table Layout/Data/Sort/Sort… | button |  | table.sortDialog | P2 | missing | FLUENT19:ShowSortTableRowsDialog |  |
| CT.TableLayout.Data.HeaderRowNoFormatting | Header Row (excluded from sort) | Table Layout/Data/Sort/Header Row (excluded from sort) | check |  | table.headerRow | P2 | verified | FLUENT19:HeaderRowNoFormatting |  |
| CT.TableLayout.Convert.ConvertToExcelSpreadsheet | Convert to Excel Spreadsheet | Table Layout/Convert/Convert to Excel Spreadsheet | button |  | table.convertToSpreadsheet | P3 | missing | FLUENT19:ConvertToExcelSpreadsheet |  |

## Contextual tab › Equation

Shown only while the selection is in a table / an equation / on a recording.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| CT.Equation.Tools.EquationInsertGallery | Equation | Equation/Tools/Equation | gallery |  | insert.equation | P2 | missing | FLUENT19:EquationInsertGallery |  |
| CT.Equation.Tools.InkEquation | Ink Equation | Equation/Tools/Ink Equation | button |  | equation.ink | P3 | missing | FLUENT19:InkEquation |  |
| CT.Equation.Tools.EquationProfessional | Professional | Equation/Tools/Professional | button |  | equation.professional | P2 | missing | FLUENT19:EquationProfessional |  |
| CT.Equation.Tools.EquationLinearFormat | Linear | Equation/Tools/Linear | button |  | equation.linear | P2 | missing | FLUENT19:EquationLinearFormat |  |
| CT.Equation.Tools.EquationNormalText | Normal Text | Equation/Tools/Normal Text | toggle |  | equation.normalText | P2 | missing | FLUENT19:EquationNormalText |  |
| CT.Equation.Tools.EquationOptions | Equation Options | Equation/Tools/Equation Options | launcher |  | equation.options | P3 | missing | FLUENT19:EquationOptions |  |
| CT.Equation.Symbols.EquationSymbolsInsertGallery | Symbols | Equation/Symbols | gallery |  | equation.symbol | P2 | missing | FLUENT19:EquationSymbolsInsertGallery |  |
| CT.Equation.Structures.EquationFractionGallery | Fraction | Equation/Structures/Fraction | gallery |  | equation.structureFraction | P2 | missing | FLUENT19:EquationFractionGallery |  |
| CT.Equation.Structures.EquationScriptGallery | Script | Equation/Structures/Script | gallery |  | equation.structureScript | P2 | missing | FLUENT19:EquationScriptGallery |  |
| CT.Equation.Structures.EquationRadicalGallery | Radical | Equation/Structures/Radical | gallery |  | equation.structureRadical | P2 | missing | FLUENT19:EquationRadicalGallery |  |
| CT.Equation.Structures.EquationIntegralGallery | Integral | Equation/Structures/Integral | gallery |  | equation.structureIntegral | P2 | missing | FLUENT19:EquationIntegralGallery |  |
| CT.Equation.Structures.EquationLargeOperatorGallery | Large Operator | Equation/Structures/Large Operator | gallery |  | equation.structureLargeOperator | P2 | missing | FLUENT19:EquationLargeOperatorGallery |  |
| CT.Equation.Structures.EquationDelimiterGallery | Bracket | Equation/Structures/Bracket | gallery |  | equation.structureDelimiter | P2 | missing | FLUENT19:EquationDelimiterGallery |  |
| CT.Equation.Structures.EquationFunctionGallery | Function | Equation/Structures/Function | gallery |  | equation.structureFunction | P2 | missing | FLUENT19:EquationFunctionGallery |  |
| CT.Equation.Structures.EquationAccentGallery | Accent | Equation/Structures/Accent | gallery |  | equation.structureAccent | P2 | missing | FLUENT19:EquationAccentGallery |  |
| CT.Equation.Structures.EquationLimitGallery | Limit and Log | Equation/Structures/Limit and Log | gallery |  | equation.structureLimit | P2 | missing | FLUENT19:EquationLimitGallery |  |
| CT.Equation.Structures.EquationOperatorGallery | Operator | Equation/Structures/Operator | gallery |  | equation.structureOperator | P2 | missing | FLUENT19:EquationOperatorGallery |  |
| CT.Equation.Structures.EquationMatrixGallery | Matrix | Equation/Structures/Matrix | gallery |  | equation.structureMatrix | P2 | missing | FLUENT19:EquationMatrixGallery |  |

## Contextual tab › Audio & Video

Shown only while the selection is in a table / an equation / on a recording.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| CT.AudioVideo.Recording.AudioRecordingInsert | Record Audio | Audio & Video/Recording/Record Audio | toggle |  | insert.audio | P2 | missing | FLUENT19:AudioRecordingInsert |  |
| CT.AudioVideo.Recording.VideoRecordingInsert | Record Video | Audio & Video/Recording/Record Video | toggle |  | insert.video | P2 | missing | FLUENT19:VideoRecordingInsert |  |
| CT.AudioVideo.Playback.RecordingPlay | Play | Audio & Video/Playback/Play | button | `Mod+Alt+P` | media.play | P2 | missing | FLUENT19:RecordingPlay |  |
| CT.AudioVideo.Playback.RecordingPause | Pause | Audio & Video/Playback/Pause | toggle |  | media.pause | P2 | missing | FLUENT19:RecordingPause |  |
| CT.AudioVideo.Playback.RecordingStop | Stop | Audio & Video/Playback/Stop | button | `Mod+Alt+S` | media.stop | P2 | missing | FLUENT19:RecordingStop |  |
| CT.AudioVideo.Playback.RewindLong | Rewind 10 Minutes | Audio & Video/Playback/Rewind 10 Minutes | button |  | media.rewindLong | P3 | missing | FLUENT19:RewindLong |  |
| CT.AudioVideo.Playback.RewindShort | Rewind 10 Seconds | Audio & Video/Playback/Rewind 10 Seconds | button | `Mod+Alt+Y` | media.rewind | P2 | missing | FLUENT19:RewindShort |  |
| CT.AudioVideo.Playback.TotalTime | Elapsed / Total Time | Audio & Video/Playback/Elapsed / Total Time | control |  |  | P3 | missing | FLUENT19:TotalTime |  |
| CT.AudioVideo.Playback.FastForwardShort | Fast Forward 10 Seconds | Audio & Video/Playback/Fast Forward 10 Seconds | button | `Mod+Alt+U` | media.forward | P2 | missing | FLUENT19:FastForwardShort |  |
| CT.AudioVideo.Playback.FastForwardLong | Fast Forward 10 Minutes | Audio & Video/Playback/Fast Forward 10 Minutes | button |  | media.forwardLong | P3 | missing | FLUENT19:FastForwardLong |  |
| CT.AudioVideo.Options.AudioNotePlayback | See Playback | Audio & Video/Options/See Playback | toggle |  | media.seePlayback | P3 | missing | FLUENT19:AudioNotePlayback |  |
| CT.AudioVideo.Options.AudioVideoSettings | Audio & Video Settings | Audio & Video/Options/Audio & Video Settings | button |  | media.settings | P3 | missing | FLUENT19:AudioVideoSettings |  |

## Backstage (File)

The File menu. Layout-only groups of the control list (thumbnails, upsell, sign-in slabs) are folded into the functional rows below.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| BS.Info.NotebookInformation | Info: Notebook Information (settings for each open notebook) | File/Info/Notebook Information | page |  | notebook.properties | P1 | wired | FLUENT19:TabInfo |  |
| BS.Info.Settings | Info: Settings (Properties, Sync, Close, Open Backups) | File/Info/Settings | menu |  | notebook.properties | P1 | missing | FLUENT19:TabInfo |  |
| BS.Info.ViewSyncStatus | Info: View Sync Status | File/Info/View Sync Status | button |  | notebook.syncStatus | P2 | missing | FLUENT19:GroupSyncStatus |  |
| BS.Info.OpenBackups | Info: Open Backups | File/Info/Open Backups | button |  | file.openBackups | P1 | wired | FLUENT19:OpenBackup |  |
| BS.New.NewNotebook | New: New Notebook (location, name) | File/New/New Notebook | page |  | file.newNotebook | P0 | wired | FLUENT19:TabNew |  |
| BS.New.AddPlace | New: Add a Place (cloud locations) | File/New/Add a Place | button |  |  | P3 | out-of-scope | FLUENT19:GroupAddLocation |  |
| BS.Open.OpenNotebook | Open: Open Notebook (browse) | File/Open/Browse | page | `Mod+O` | file.openNotebook | P0 | wired | FLUENT19:ButtonOpenBrowse |  |
| BS.Open.RecentNotebooks | Open: Recent Notebooks | File/Open/Recent Notebooks | list |  | file.openRecent | P0 | verified | FLUENT19:GroupRecentDocuments |  |
| BS.Open.SharedWithMe | Open: Shared with Me | File/Open/Shared with Me | list |  |  | P3 | out-of-scope | FLUENT19:GroupSharedWithMeDocuments |  |
| BS.Open.OpenSection | Open: Open Section (.one file) | File/Open/Open Section | button | `Mod+Alt+Shift+O` | file.openSection | P1 | missing | FLUENT19:JotOpenFile; KBD365 |  |
| BS.Print.Print | Print: Print | File/Print/Print | button | `Mod+P` | file.print | P1 | missing | FLUENT19:FilePrint |  |
| BS.Print.Preview | Print: Print Preview (range, paper, header/footer) | File/Print/Print Preview | dialog |  | file.printPreview | P2 | missing | FLUENT19:FilePrintPreview |  |
| BS.Share.WithPeople | Share: Share with People | File/Share/Share with People | page |  |  | P3 | out-of-scope | FLUENT19:ShareWithPeople |  |
| BS.Share.GetLink | Share: Get a Sharing Link | File/Share/Get a Sharing Link | page |  |  | P3 | out-of-scope | FLUENT19:GetLinks |  |
| BS.Share.WithNetwork | Share: Share on Network (shared folder) | File/Share/Share on Network | page |  | notebook.moveToFolder | P2 | missing | FLUENT19:ShareWithNetwork |  |
| BS.Share.WithMeeting | Share: Share with Meeting | File/Share/Share with Meeting | page |  |  | P3 | out-of-scope | FLUENT19:ShareWithMeeting |  |
| BS.Share.MoveNotebook | Share: Move Notebook | File/Share/Move Notebook | page |  | notebook.moveToFolder | P2 | missing | FLUENT19:MoveNotebook |  |
| BS.Export.Page | Export: Page | File/Export/Page | choice |  | file.exportPage | P1 | missing | FLUENT19:PageSaveAs |  |
| BS.Export.Section | Export: Section | File/Export/Section | choice |  | file.exportSection | P1 | missing | FLUENT19:SectionSaveAs |  |
| BS.Export.Notebook | Export: Notebook | File/Export/Notebook | choice |  | file.exportNotebook | P1 | wired | FLUENT19:NotebookSaveAsBackstage |  |
| BS.Export.FormatOne | Export format: OneNote 2010-2016 Section (*.one) | File/Export/OneNote Section (.one) | choice |  | file.exportOne | P1 | missing | GK: Export page of the backstage | unverified |
| BS.Export.FormatOnepkg | Export format: OneNote Package (*.onepkg) | File/Export/OneNote Package (.onepkg) | choice |  | file.exportOnepkg | P1 | missing | GK | unverified |
| BS.Export.FormatDocx | Export format: Word Document (*.docx) | File/Export/Word Document (.docx) | choice |  | file.exportDocx | P1 | missing | GK | unverified |
| BS.Export.FormatDoc | Export format: Word 97-2003 Document (*.doc) | File/Export/Word 97-2003 Document (.doc) | choice |  | file.exportDoc | P3 | missing | GK | unverified |
| BS.Export.FormatPdf | Export format: PDF (*.pdf) | File/Export/PDF (.pdf) | choice |  | file.exportPdf | P1 | missing | GK | unverified |
| BS.Export.FormatXps | Export format: XPS (*.xps) | File/Export/XPS (.xps) | choice |  | file.exportXps | P3 | missing | GK | unverified |
| BS.Export.FormatMht | Export format: Single File Web Page (*.mht) | File/Export/Single File Web Page (.mht) | choice |  | file.exportMht | P2 | missing | GK | unverified |
| BS.Send.Email | Send: Email Page | File/Send/Email Page | button | `Mod+Shift+E` | share.emailPage | P2 | missing | FLUENT19:SendNotesViaEmail |  |
| BS.Send.Attachment | Send: Send as Attachment | File/Send/Send as Attachment | button |  | share.sendAttachment | P2 | missing | FLUENT19:SendNotesAsAttachment |  |
| BS.Send.Pdf | Send: Send as PDF | File/Send/Send as PDF | button |  | share.sendPdf | P2 | missing | FLUENT19:SendPdf |  |
| BS.Send.Word | Send: Send to Word | File/Send/Send to Word | button |  | share.sendToWord | P2 | missing | FLUENT19:SendNotesToWord |  |
| BS.Send.Blog | Send: Send to Blog | File/Send/Send to Blog | button |  |  | P3 | out-of-scope | FLUENT19:SendNotesToBlog |  |
| BS.Account.Identity | Account: User Information (name used as author) | File/Account/User Information | page |  | file.options | P2 | missing | FLUENT19:GroupBackstageIdentity |  |
| BS.Account.Theme | Account: Office Theme / Background | File/Account/Theme | combo |  | view.theme | P1 | missing | FLUENT19:Theming |  |
| BS.Account.SignIn | Account: Sign in / Connected Services | File/Account/Connected Services | page |  |  | P3 | out-of-scope | FLUENT19:GroupBackstageSignIn |  |
| BS.Account.About | Account: About (version) | File/Account/About | button |  | help.about | P1 | missing | FLUENT19:GroupAboutOfficeProducts |  |
| BS.Account.WhatsNew | Account: What's New | File/Account/What's New | button |  | help.whatsNew | P3 | missing | FLUENT19:GroupWhatsNewInOfficeProducts |  |
| BS.Account.Updates | Account: Update Options | File/Account/Update Options | menu |  |  | P3 | out-of-scope | FLUENT19:GroupClickToRunUpdateStatus | updates come from the Craft launcher |
| BS.Account.Insider | Account: Office Insider | File/Account/Insider | menu |  |  | P3 | out-of-scope | FLUENT19:GroupOfficeInsider |  |
| BS.Feedback | Feedback | File/Feedback | page |  | help.feedback | P3 | missing | FLUENT19:TabOfficeFeedback |  |
| BS.Options | Options | File/Options | dialog |  | file.options | P0 | wired | FLUENT19:ApplicationOptionsDialog |  |
| BS.Exit | Exit | File/Exit | button |  | file.exit | P0 | missing | FLUENT19:FileExit |  |

## Context menus

Every item of every right-click menu, in menu order (FLUENT19 context menus). Items repeat across menus on purpose: each placement is checked.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| CM.NoteContainer.Cut | Cut | Context/Note Container/Cut | button | `Mod+X` | edit.cut | P0 | wired | FLUENT19:ContextMenuOutlineHandle/Cut |  |
| CM.NoteContainer.CopyOneNote | Copy | Context/Note Container/Copy | button | `Mod+C` | edit.copy | P0 | wired | FLUENT19:ContextMenuOutlineHandle/CopyOneNote |  |
| CM.NoteContainer.PasteGalleryMini | Paste Options | Context/Note Container/Paste Options | gallery |  | edit.pasteOptions | P0 | wired | FLUENT19:ContextMenuOutlineHandle/PasteGalleryMini | A row of the four paste choices (Keep Source Formatting, Merge Formatting, Keep Text Only, Picture) under a "Paste Options:" caption; each reads the system clipboard as Ctrl+V does (order and icons unverified) |
| CM.NoteContainer.Delete | Delete | Context/Note Container/Delete | button | `Delete` | edit.delete | P0 | wired | FLUENT19:ContextMenuOutlineHandle/Delete |  |
| CM.NoteContainer.HyperlinkCreate | Link… | Context/Note Container/Link… | button | `Mod+K` | insert.link | P0 | wired | FLUENT19:ContextMenuOutlineHandle/HyperlinkCreate |  |
| CM.NoteContainer.SelectOutlineLevelsMenu | Select Outline Levels | Context/Note Container/Select Outline Levels | menu |  |  | P2 | missing | FLUENT19:ContextMenuOutlineHandle/SelectOutlineLevelsMenu |  |
| CM.NoteContainer.OutlineSelectLevel1 | Show Level 1 | Context/Note Container/Select Outline Levels/Show Level 1 | button |  | outline.level1 | P1 | wired | FLUENT19:ContextMenuOutlineHandle/OutlineSelectLevel1 |  |
| CM.NoteContainer.OutlineSelectLevel2 | Show Level 2 | Context/Note Container/Select Outline Levels/Show Level 2 | button |  | outline.level2 | P1 | wired | FLUENT19:ContextMenuOutlineHandle/OutlineSelectLevel2 |  |
| CM.NoteContainer.OutlineSelectLevel3 | Show Level 3 | Context/Note Container/Select Outline Levels/Show Level 3 | button |  | outline.level3 | P1 | wired | FLUENT19:ContextMenuOutlineHandle/OutlineSelectLevel3 |  |
| CM.NoteContainer.OutlineSelectLevel4 | Show Level 4 | Context/Note Container/Select Outline Levels/Show Level 4 | button |  | outline.level4 | P1 | wired | FLUENT19:ContextMenuOutlineHandle/OutlineSelectLevel4 |  |
| CM.NoteContainer.OutlineSelectLevel5 | Show Level 5 | Context/Note Container/Select Outline Levels/Show Level 5 | button |  | outline.level5 | P1 | wired | FLUENT19:ContextMenuOutlineHandle/OutlineSelectLevel5 |  |
| CM.NoteContainer.OutlineSelectLevelAll | Show All Levels | Context/Note Container/Select Outline Levels/Show All Levels | button | `Alt+Shift+0` | outline.expandAll | P1 | wired | FLUENT19:ContextMenuOutlineHandle/OutlineSelectLevelAll |  |
| CM.NoteContainer.OrderObjectsMenu | Order | Context/Note Container/Order | menu |  |  | P1 | missing | FLUENT19:ContextMenuOutlineHandle/OrderObjectsMenu |  |
| CM.NoteContainer.ObjectBringForward | Bring Forward | Context/Note Container/Order/Bring Forward | button |  | object.bringForward | P1 | verified | FLUENT19:ContextMenuOutlineHandle/ObjectBringForward |  |
| CM.NoteContainer.ObjectBringToFront | Bring to Front | Context/Note Container/Order/Bring to Front | button |  | object.bringToFront | P1 | verified | FLUENT19:ContextMenuOutlineHandle/ObjectBringToFront |  |
| CM.NoteContainer.ObjectSendBackward | Send Backward | Context/Note Container/Order/Send Backward | button |  | object.sendBackward | P1 | verified | FLUENT19:ContextMenuOutlineHandle/ObjectSendBackward |  |
| CM.NoteContainer.ObjectSendToBack | Send to Back | Context/Note Container/Order/Send to Back | button |  | object.sendToBack | P1 | verified | FLUENT19:ContextMenuOutlineHandle/ObjectSendToBack |  |
| CM.NoteContainer.ObjectMove | Move | Context/Note Container/Move | button |  | object.move | P1 | missing | FLUENT19:ContextMenuOutlineHandle/ObjectMove |  |
| CM.NoteContainer.ObjectResize | Resize | Context/Note Container/Resize | button |  | object.resize | P1 | missing | FLUENT19:ContextMenuOutlineHandle/ObjectResize |  |
| CM.NoteContainer.ConvertInkMenu | Convert Ink | Context/Note Container/Convert Ink | menu |  |  | P2 | missing | FLUENT19:ContextMenuOutlineHandle/ConvertInkMenu |  |
| CM.NoteContainer.SelectionToTextConvert | Ink to Text | Context/Note Container/Convert Ink/Ink to Text | button |  | draw.inkToText | P2 | missing | FLUENT19:ContextMenuOutlineHandle/SelectionToTextConvert |  |
| CM.NoteContainer.SelectionToMathConvert | Ink to Math | Context/Note Container/Convert Ink/Ink to Math | button |  | draw.inkToMath | P2 | missing | FLUENT19:ContextMenuOutlineHandle/SelectionToMathConvert |  |
| CM.ParagraphHandle.Cut | Cut | Context/Paragraph Handle/Cut | button | `Mod+X` | edit.cut | P0 | wired | FLUENT19:ContextMenuOutlineElementHandle/Cut |  |
| CM.ParagraphHandle.CopyOneNote | Copy | Context/Paragraph Handle/Copy | button | `Mod+C` | edit.copy | P0 | wired | FLUENT19:ContextMenuOutlineElementHandle/CopyOneNote |  |
| CM.ParagraphHandle.PasteGalleryMini | Paste Options | Context/Paragraph Handle/Paste Options | gallery |  | edit.pasteOptions | P0 | wired | FLUENT19:ContextMenuOutlineElementHandle/PasteGalleryMini | A row of the four paste choices (Keep Source Formatting, Merge Formatting, Keep Text Only, Picture) under a "Paste Options:" caption; each reads the system clipboard as Ctrl+V does (order and icons unverified) |
| CM.ParagraphHandle.Delete | Delete | Context/Paragraph Handle/Delete | button | `Delete` | edit.delete | P0 | wired | FLUENT19:ContextMenuOutlineElementHandle/Delete |  |
| CM.ParagraphHandle.HyperlinkCreate | Link… | Context/Paragraph Handle/Link… | button | `Mod+K` | insert.link | P0 | wired | FLUENT19:ContextMenuOutlineElementHandle/HyperlinkCreate |  |
| CM.ParagraphHandle.CopyTextFromPicture | Copy Text from Picture | Context/Paragraph Handle/Copy Text from Picture | button |  | object.copyTextFromPicture | P2 | missing | FLUENT19:ContextMenuOutlineElementHandle/CopyTextFromPicture |  |
| CM.ParagraphHandle.AlternativeText | Alt Text | Context/Paragraph Handle/Alt Text | button |  | object.altText | P1 | missing | FLUENT19:ContextMenuOutlineElementHandle/AlternativeText |  |
| CM.ParagraphHandle.ImageSaveAs | Save As (picture) | Context/Paragraph Handle/Save As (picture) | button |  | object.saveAs | P1 | missing | FLUENT19:ContextMenuOutlineElementHandle/ImageSaveAs |  |
| CM.ParagraphHandle.TableMenu | Table | Context/Paragraph Handle/Table | menu |  |  | P1 | missing | FLUENT19:ContextMenuOutlineElementHandle/TableMenu |  |
| CM.ParagraphHandle.TableInsert | Insert Table | Context/Paragraph Handle/Table/Insert Table | button |  | insert.tableDialog | P1 | missing | FLUENT19:ContextMenuOutlineElementHandle/TableInsert |  |
| CM.ParagraphHandle.TableInsertColumnsLeft | Insert Left | Context/Paragraph Handle/Table/Insert Left | button | `Mod+Alt+E` | table.insertLeft | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/TableInsertColumnsLeft |  |
| CM.ParagraphHandle.TableInsertColumnsRight | Insert Right | Context/Paragraph Handle/Table/Insert Right | button | `Mod+Alt+R` | table.insertRight | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/TableInsertColumnsRight |  |
| CM.ParagraphHandle.TableInsertRowsAbove | Insert Above | Context/Paragraph Handle/Table/Insert Above | button |  | table.insertAbove | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/TableInsertRowsAbove |  |
| CM.ParagraphHandle.TableInsertRowsBelow | Insert Below | Context/Paragraph Handle/Table/Insert Below | button | `Mod+Enter` | table.insertBelow | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/TableInsertRowsBelow |  |
| CM.ParagraphHandle.DeleteTableOneNote | Delete Table | Context/Paragraph Handle/Table/Delete Table | button |  | table.delete | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/DeleteTableOneNote |  |
| CM.ParagraphHandle.DeleteColumnsOneNote | Delete Columns | Context/Paragraph Handle/Table/Delete Columns | button |  | table.deleteColumns | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/DeleteColumnsOneNote |  |
| CM.ParagraphHandle.DeleteRowsOneNote | Delete Rows | Context/Paragraph Handle/Table/Delete Rows | button |  | table.deleteRows | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/DeleteRowsOneNote |  |
| CM.ParagraphHandle.SelectTableOneNote | Select Table | Context/Paragraph Handle/Table/Select Table | button |  | table.selectTable | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/SelectTableOneNote |  |
| CM.ParagraphHandle.SelectColumns | Select Columns | Context/Paragraph Handle/Table/Select Columns | button |  | table.selectColumns | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/SelectColumns |  |
| CM.ParagraphHandle.SelectRows | Select Rows | Context/Paragraph Handle/Table/Select Rows | button |  | table.selectRows | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/SelectRows |  |
| CM.ParagraphHandle.SelectCell | Select Cell | Context/Paragraph Handle/Table/Select Cell | button |  | table.selectCell | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/SelectCell |  |
| CM.ParagraphHandle.TableBordersHide | Hide Borders | Context/Paragraph Handle/Table/Hide Borders | toggle |  | table.hideBorders | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/TableBordersHide |  |
| CM.ParagraphHandle.ShadingColorPicker | Shading | Context/Paragraph Handle/Table/Shading | gallery |  | table.shading | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/ShadingColorPicker |  |
| CM.ParagraphHandle.SortUp | Sort Ascending | Context/Paragraph Handle/Table/Sort Ascending | button |  | table.sortAscending | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/SortUp |  |
| CM.ParagraphHandle.SortDown | Sort Descending | Context/Paragraph Handle/Table/Sort Descending | button |  | table.sortDescending | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/SortDown |  |
| CM.ParagraphHandle.ShowSortTableRowsDialog | Sort… | Context/Paragraph Handle/Table/Sort… | button |  | table.sortDialog | P2 | missing | FLUENT19:ContextMenuOutlineElementHandle/ShowSortTableRowsDialog |  |
| CM.ParagraphHandle.HeaderRowNoFormatting | Header Row (excluded from sort) | Context/Paragraph Handle/Table/Header Row (excluded from sort) | check |  | table.headerRow | P2 | wired | FLUENT19:ContextMenuOutlineElementHandle/HeaderRowNoFormatting |  |
| CM.ParagraphHandle.SelectOutlineLevelsMenu | Select Outline Levels | Context/Paragraph Handle/Select Outline Levels | menu |  |  | P2 | missing | FLUENT19:ContextMenuOutlineElementHandle/SelectOutlineLevelsMenu |  |
| CM.ParagraphHandle.OutlineSelectLevel1 | Show Level 1 | Context/Paragraph Handle/Select Outline Levels/Show Level 1 | button |  | outline.level1 | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/OutlineSelectLevel1 |  |
| CM.ParagraphHandle.OutlineSelectLevel2 | Show Level 2 | Context/Paragraph Handle/Select Outline Levels/Show Level 2 | button |  | outline.level2 | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/OutlineSelectLevel2 |  |
| CM.ParagraphHandle.OutlineSelectLevel3 | Show Level 3 | Context/Paragraph Handle/Select Outline Levels/Show Level 3 | button |  | outline.level3 | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/OutlineSelectLevel3 |  |
| CM.ParagraphHandle.OutlineSelectLevel4 | Show Level 4 | Context/Paragraph Handle/Select Outline Levels/Show Level 4 | button |  | outline.level4 | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/OutlineSelectLevel4 |  |
| CM.ParagraphHandle.OutlineSelectLevel5 | Show Level 5 | Context/Paragraph Handle/Select Outline Levels/Show Level 5 | button |  | outline.level5 | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/OutlineSelectLevel5 |  |
| CM.ParagraphHandle.OutlineSelectLevelAll | Show All Levels | Context/Paragraph Handle/Select Outline Levels/Show All Levels | button | `Alt+Shift+0` | outline.expandAll | P1 | wired | FLUENT19:ContextMenuOutlineElementHandle/OutlineSelectLevelAll |  |
| CM.ParagraphHandle.ConvertInkMenu | Convert Ink | Context/Paragraph Handle/Convert Ink | menu |  |  | P2 | missing | FLUENT19:ContextMenuOutlineElementHandle/ConvertInkMenu |  |
| CM.ParagraphHandle.SelectionToTextConvert | Ink to Text | Context/Paragraph Handle/Convert Ink/Ink to Text | button |  | draw.inkToText | P2 | missing | FLUENT19:ContextMenuOutlineElementHandle/SelectionToTextConvert |  |
| CM.ParagraphHandle.SelectionToMathConvert | Ink to Math | Context/Paragraph Handle/Convert Ink/Ink to Math | button |  | draw.inkToMath | P2 | missing | FLUENT19:ContextMenuOutlineElementHandle/SelectionToMathConvert |  |
| CM.ParagraphHandle.LinkedNotesMenu | Linked Notes | Context/Paragraph Handle/Linked Notes | menu |  |  | P3 | missing | FLUENT19:ContextMenuOutlineElementHandle/LinkedNotesMenu |  |
| CM.ParagraphHandle.LinkedFileOpen | Open Linked File | Context/Paragraph Handle/Linked Notes/Open Linked File | button |  | linked.open | P3 | missing | FLUENT19:ContextMenuOutlineElementHandle/LinkedFileOpen |  |
| CM.ParagraphHandle.LinkedNotesTargetUrlCopy | Copy Link to Linked File | Context/Paragraph Handle/Linked Notes/Copy Link to Linked File | button |  | linked.copyUrl | P3 | missing | FLUENT19:ContextMenuOutlineElementHandle/LinkedNotesTargetUrlCopy |  |
| CM.ParagraphHandle.NoteLinkRepairLink | Repair Link | Context/Paragraph Handle/Linked Notes/Repair Link | button |  | linked.repair | P3 | missing | FLUENT19:ContextMenuOutlineElementHandle/NoteLinkRepairLink |  |
| CM.ParagraphHandle.NoteLinkReLink | Re-link | Context/Paragraph Handle/Linked Notes/Re-link | button |  | linked.relink | P3 | missing | FLUENT19:ContextMenuOutlineElementHandle/NoteLinkReLink |  |
| CM.ParagraphHandle.NoteLinkDelete | Delete Link | Context/Paragraph Handle/Linked Notes/Delete Link | button |  | linked.delete | P3 | missing | FLUENT19:ContextMenuOutlineElementHandle/NoteLinkDelete |  |
| CM.ParagraphHandle.LinkedNotesOptions | Linked Notes Options | Context/Paragraph Handle/Linked Notes/Linked Notes Options | button |  | linked.options | P3 | missing | FLUENT19:ContextMenuOutlineElementHandle/LinkedNotesOptions |  |
| CM.ParagraphHandle.OCRLanguageListMenu | Make Text in Image Searchable | Context/Paragraph Handle/Make Text in Image Searchable | menu |  | object.ocrLanguage | P2 | missing | FLUENT19:ContextMenuOutlineElementHandle/OCRLanguageListMenu |  |
| CM.ParagraphHandle.LastModifiedBy | Last Modified By (author and date) | Context/Paragraph Handle/Last Modified By (author and date) | button |  | object.lastModified | P2 | missing | FLUENT19:ContextMenuOutlineElementHandle/LastModifiedBy |  |
| CM.Text.NoSpellingSuggestions | Spelling suggestions | Context/Text/Spelling suggestions | gallery |  | review.spellingSuggestion | P1 | missing | FLUENT19:ContextMenuText/NoSpellingSuggestions |  |
| CM.Text.InkAlternate1 | Ink Recognition Alternate 1 | Context/Text/Ink Recognition Alternate 1 | button |  | draw.inkAlternate | P3 | missing | FLUENT19:ContextMenuText/InkAlternate1 |  |
| CM.Text.InkAlternate2 | Ink Recognition Alternate 2 | Context/Text/Ink Recognition Alternate 2 | button |  | draw.inkAlternate | P3 | missing | FLUENT19:ContextMenuText/InkAlternate2 |  |
| CM.Text.InkAlternate3 | Ink Recognition Alternate 3 | Context/Text/Ink Recognition Alternate 3 | button |  | draw.inkAlternate | P3 | missing | FLUENT19:ContextMenuText/InkAlternate3 |  |
| CM.Text.InkAlternate4 | Ink Recognition Alternate 4 | Context/Text/Ink Recognition Alternate 4 | button |  | draw.inkAlternate | P3 | missing | FLUENT19:ContextMenuText/InkAlternate4 |  |
| CM.Text.InkAlternate5 | Ink Recognition Alternate 5 | Context/Text/Ink Recognition Alternate 5 | button |  | draw.inkAlternate | P3 | missing | FLUENT19:ContextMenuText/InkAlternate5 |  |
| CM.Text.MoreInkResults | More Ink Results | Context/Text/More Ink Results | button |  | draw.inkAlternate | P3 | missing | FLUENT19:ContextMenuText/MoreInkResults |  |
| CM.Text.Cut | Cut | Context/Text/Cut | button | `Mod+X` | edit.cut | P0 | verified | FLUENT19:ContextMenuText/Cut |  |
| CM.Text.CopyOneNote | Copy | Context/Text/Copy | button | `Mod+C` | edit.copy | P0 | wired | FLUENT19:ContextMenuText/CopyOneNote |  |
| CM.Text.PasteGalleryMini | Paste Options | Context/Text/Paste Options | gallery |  | edit.pasteOptions | P0 | wired | FLUENT19:ContextMenuText/PasteGalleryMini | A row of the four paste choices (Keep Source Formatting, Merge Formatting, Keep Text Only, Picture) under a "Paste Options:" caption; each reads the system clipboard as Ctrl+V does (order and icons unverified) |
| CM.Text.HyperlinkOpen | Open Link | Context/Text/Open Link | button | `Enter` | link.open | P0 | wired | FLUENT19:ContextMenuText/HyperlinkOpen |  |
| CM.Text.LinkEdit | Edit Link… | Context/Text/Edit Link… | button |  | link.edit | P0 | verified | FLUENT19:ContextMenuText/LinkEdit |  |
| CM.Text.HyperlinkCreate | Link… | Context/Text/Link… | button | `Mod+K` | insert.link | P0 | wired | FLUENT19:ContextMenuText/HyperlinkCreate |  |
| CM.Text.ToPageLink | Link to Page (wiki link) | Context/Text/Link to Page (wiki link) | button |  | link.toPage | P1 | missing | FLUENT19:ContextMenuText/ToPageLink |  |
| CM.Text.LinkCopyOneNote | Copy Link | Context/Text/Copy Link | button |  | link.copy | P0 | verified | FLUENT19:ContextMenuText/LinkCopyOneNote |  |
| CM.Text.LinkSelect | Select Link | Context/Text/Select Link | button |  | link.select | P1 | verified | FLUENT19:ContextMenuText/LinkSelect |  |
| CM.Text.LinkRemove | Remove Link | Context/Text/Remove Link | button |  | link.remove | P0 | verified | FLUENT19:ContextMenuText/LinkRemove |  |
| CM.Text.CopyTextFromPicture | Copy Text from Picture | Context/Text/Copy Text from Picture | button |  | object.copyTextFromPicture | P2 | missing | FLUENT19:ContextMenuText/CopyTextFromPicture |  |
| CM.Text.PrintoutPageTextCopy | Copy Text from This Page of the Printout | Context/Text/Copy Text from This Page of the Printout | button |  | printout.copyPageText | P2 | missing | FLUENT19:ContextMenuText/PrintoutPageTextCopy |  |
| CM.Text.CopyTextFromPrintoutPages | Copy Text from All Pages of the Printout | Context/Text/Copy Text from All Pages of the Printout | button |  | printout.copyAllText | P2 | missing | FLUENT19:ContextMenuText/CopyTextFromPrintoutPages |  |
| CM.Text.RotateInkMenu | Rotate | Context/Text/Rotate | menu |  |  | P1 | missing | FLUENT19:ContextMenuText/RotateInkMenu |  |
| CM.Text.RotateRight45OneNote | Rotate Right 45° | Context/Text/Rotate/Rotate Right 45° | button |  | object.rotateRight45 | P2 | missing | FLUENT19:ContextMenuText/RotateRight45OneNote |  |
| CM.Text.RotateLeft45OneNote | Rotate Left 45° | Context/Text/Rotate/Rotate Left 45° | button |  | object.rotateLeft45 | P2 | missing | FLUENT19:ContextMenuText/RotateLeft45OneNote |  |
| CM.Text.ObjectRotateRight90 | Rotate Right 90° | Context/Text/Rotate/Rotate Right 90° | button |  | object.rotateRight90 | P1 | missing | FLUENT19:ContextMenuText/ObjectRotateRight90 |  |
| CM.Text.ObjectRotateLeft90 | Rotate Left 90° | Context/Text/Rotate/Rotate Left 90° | button |  | object.rotateLeft90 | P1 | missing | FLUENT19:ContextMenuText/ObjectRotateLeft90 |  |
| CM.Text.ObjectFlipHorizontal | Flip Horizontal | Context/Text/Rotate/Flip Horizontal | button |  | object.flipHorizontal | P1 | missing | FLUENT19:ContextMenuText/ObjectFlipHorizontal |  |
| CM.Text.ObjectFlipVertical | Flip Vertical | Context/Text/Rotate/Flip Vertical | button |  | object.flipVertical | P1 | missing | FLUENT19:ContextMenuText/ObjectFlipVertical |  |
| CM.Text.TreatInkAsMenu | Treat Selected Ink As | Context/Text/Treat Selected Ink As | menu |  |  | P2 | missing | FLUENT19:ContextMenuText/TreatInkAsMenu |  |
| CM.Text.TreatInkAsHandwriting | Handwriting | Context/Text/Treat Selected Ink As/Handwriting | button |  | draw.treatAsHandwriting | P2 | missing | FLUENT19:ContextMenuText/TreatInkAsHandwriting |  |
| CM.Text.TreatInkAsDrawing | Drawing | Context/Text/Treat Selected Ink As/Drawing | button |  | draw.treatAsDrawing | P2 | missing | FLUENT19:ContextMenuText/TreatInkAsDrawing |  |
| CM.Text.ConvertInkMenu | Convert Ink | Context/Text/Convert Ink | menu |  |  | P2 | missing | FLUENT19:ContextMenuText/ConvertInkMenu |  |
| CM.Text.SelectionToTextConvert | Ink to Text | Context/Text/Convert Ink/Ink to Text | button |  | draw.inkToText | P2 | missing | FLUENT19:ContextMenuText/SelectionToTextConvert |  |
| CM.Text.SelectionToMathConvert | Ink to Math | Context/Text/Convert Ink/Ink to Math | button |  | draw.inkToMath | P2 | missing | FLUENT19:ContextMenuText/SelectionToMathConvert |  |
| CM.Text.LookUp | Smart Lookup | Context/Text/Smart Lookup | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuText/LookUp |  |
| CM.Text.Translate | Translate | Context/Text/Translate | button |  | review.translateSelection | P3 | missing | FLUENT19:ContextMenuText/Translate |  |
| CM.Text.PictureAsBackground | Set Picture as Background | Context/Text/Set Picture as Background | toggle |  | object.setAsBackground | P1 | missing | FLUENT19:ContextMenuText/PictureAsBackground |  |
| CM.Text.AlternativeText | Alt Text | Context/Text/Alt Text | button |  | object.altText | P1 | missing | FLUENT19:ContextMenuText/AlternativeText |  |
| CM.Text.OCRLanguageListMenu | Make Text in Image Searchable | Context/Text/Make Text in Image Searchable | menu |  | object.ocrLanguage | P2 | missing | FLUENT19:ContextMenuText/OCRLanguageListMenu |  |
| CM.Text.TableMenu | Table | Context/Text/Table | menu |  |  | P1 | missing | FLUENT19:ContextMenuText/TableMenu |  |
| CM.Text.TableInsert | Insert Table | Context/Text/Table/Insert Table | button |  | insert.tableDialog | P1 | missing | FLUENT19:ContextMenuText/TableInsert |  |
| CM.Text.TableInsertColumnsLeft | Insert Left | Context/Text/Table/Insert Left | button | `Mod+Alt+E` | table.insertLeft | P1 | wired | FLUENT19:ContextMenuText/TableInsertColumnsLeft |  |
| CM.Text.TableInsertColumnsRight | Insert Right | Context/Text/Table/Insert Right | button | `Mod+Alt+R` | table.insertRight | P1 | wired | FLUENT19:ContextMenuText/TableInsertColumnsRight |  |
| CM.Text.TableInsertRowsAbove | Insert Above | Context/Text/Table/Insert Above | button |  | table.insertAbove | P1 | wired | FLUENT19:ContextMenuText/TableInsertRowsAbove |  |
| CM.Text.TableInsertRowsBelow | Insert Below | Context/Text/Table/Insert Below | button | `Mod+Enter` | table.insertBelow | P1 | wired | FLUENT19:ContextMenuText/TableInsertRowsBelow |  |
| CM.Text.DeleteTableOneNote | Delete Table | Context/Text/Table/Delete Table | button |  | table.delete | P1 | wired | FLUENT19:ContextMenuText/DeleteTableOneNote |  |
| CM.Text.DeleteColumnsOneNote | Delete Columns | Context/Text/Table/Delete Columns | button |  | table.deleteColumns | P1 | wired | FLUENT19:ContextMenuText/DeleteColumnsOneNote |  |
| CM.Text.DeleteRowsOneNote | Delete Rows | Context/Text/Table/Delete Rows | button |  | table.deleteRows | P1 | wired | FLUENT19:ContextMenuText/DeleteRowsOneNote |  |
| CM.Text.SelectTableOneNote | Select Table | Context/Text/Table/Select Table | button |  | table.selectTable | P1 | verified | FLUENT19:ContextMenuText/SelectTableOneNote |  |
| CM.Text.SelectColumns | Select Columns | Context/Text/Table/Select Columns | button |  | table.selectColumns | P1 | wired | FLUENT19:ContextMenuText/SelectColumns |  |
| CM.Text.SelectRows | Select Rows | Context/Text/Table/Select Rows | button |  | table.selectRows | P1 | wired | FLUENT19:ContextMenuText/SelectRows |  |
| CM.Text.SelectCell | Select Cell | Context/Text/Table/Select Cell | button |  | table.selectCell | P1 | wired | FLUENT19:ContextMenuText/SelectCell |  |
| CM.Text.TableBordersHide | Hide Borders | Context/Text/Table/Hide Borders | toggle |  | table.hideBorders | P1 | wired | FLUENT19:ContextMenuText/TableBordersHide |  |
| CM.Text.ShadingColorPicker | Shading | Context/Text/Table/Shading | gallery |  | table.shading | P1 | wired | FLUENT19:ContextMenuText/ShadingColorPicker |  |
| CM.Text.SortUp | Sort Ascending | Context/Text/Table/Sort Ascending | button |  | table.sortAscending | P1 | wired | FLUENT19:ContextMenuText/SortUp |  |
| CM.Text.SortDown | Sort Descending | Context/Text/Table/Sort Descending | button |  | table.sortDescending | P1 | wired | FLUENT19:ContextMenuText/SortDown |  |
| CM.Text.ShowSortTableRowsDialog | Sort… | Context/Text/Table/Sort… | button |  | table.sortDialog | P2 | missing | FLUENT19:ContextMenuText/ShowSortTableRowsDialog |  |
| CM.Text.HeaderRowNoFormatting | Header Row (excluded from sort) | Context/Text/Table/Header Row (excluded from sort) | check |  | table.headerRow | P2 | wired | FLUENT19:ContextMenuText/HeaderRowNoFormatting |  |
| CM.Text.ConvertToExcelSpreadsheet | Convert to Excel Spreadsheet | Context/Text/Convert to Excel Spreadsheet | button |  | table.convertToSpreadsheet | P3 | missing | FLUENT19:ContextMenuText/ConvertToExcelSpreadsheet |  |
| CM.Text.LinkedNotesMenu | Linked Notes | Context/Text/Linked Notes | menu |  |  | P3 | missing | FLUENT19:ContextMenuText/LinkedNotesMenu |  |
| CM.Text.LinkedFileOpen | Open Linked File | Context/Text/Linked Notes/Open Linked File | button |  | linked.open | P3 | missing | FLUENT19:ContextMenuText/LinkedFileOpen |  |
| CM.Text.LinkedNotesTargetUrlCopy | Copy Link to Linked File | Context/Text/Linked Notes/Copy Link to Linked File | button |  | linked.copyUrl | P3 | missing | FLUENT19:ContextMenuText/LinkedNotesTargetUrlCopy |  |
| CM.Text.NoteLinkRepairLink | Repair Link | Context/Text/Linked Notes/Repair Link | button |  | linked.repair | P3 | missing | FLUENT19:ContextMenuText/NoteLinkRepairLink |  |
| CM.Text.NoteLinkReLink | Re-link | Context/Text/Linked Notes/Re-link | button |  | linked.relink | P3 | missing | FLUENT19:ContextMenuText/NoteLinkReLink |  |
| CM.Text.NoteLinkDelete | Delete Link | Context/Text/Linked Notes/Delete Link | button |  | linked.delete | P3 | missing | FLUENT19:ContextMenuText/NoteLinkDelete |  |
| CM.Text.LinkedNotesOptions | Linked Notes Options | Context/Text/Linked Notes/Linked Notes Options | button |  | linked.options | P3 | missing | FLUENT19:ContextMenuText/LinkedNotesOptions |  |
| CM.Text.LastModifiedBy | Last Modified By (author and date) | Context/Text/Last Modified By (author and date) | button |  | object.lastModified | P2 | missing | FLUENT19:ContextMenuText/LastModifiedBy |  |
| CM.MisspelledWord.NoSpellingSuggestions | Spelling suggestions | Context/Misspelled Word/Spelling suggestions | gallery |  | review.spellingSuggestion | P1 | missing | FLUENT19:ContextMenuSpell/NoSpellingSuggestions |  |
| CM.Bullet.BulletRemove | Remove Bullet | Context/Bullet/Remove Bullet | button |  | para.bulletRemove | P0 | wired | FLUENT19:ContextMenuBullets/BulletRemove |  |
| CM.Bullet.ChangeToNumberList | Change to Numbered List | Context/Bullet/Change to Numbered List | button |  | para.numbering | P1 | missing | FLUENT19:ContextMenuBullets/ChangeToNumberList |  |
| CM.Number.NumberListRestart | Restart Numbering | Context/Number/Restart Numbering | button |  | para.restartNumbering | P1 | verified | FLUENT19:ContextMenuNumbering/NumberListRestart |  |
| CM.Number.ContinueNumberList | Continue Numbering | Context/Number/Continue Numbering | button |  | para.continueNumbering | P1 | wired | FLUENT19:ContextMenuNumbering/ContinueNumberList |  |
| CM.Number.NumberListRemove | Remove Numbering | Context/Number/Remove Numbering | button |  | para.numberingRemove | P0 | wired | FLUENT19:ContextMenuNumbering/NumberListRemove |  |
| CM.Number.ChangeToBullet | Change to Bulleted List | Context/Number/Change to Bulleted List | button |  | para.bullets | P1 | missing | FLUENT19:ContextMenuNumbering/ChangeToBullet |  |
| CM.InkDrawing.Cut | Cut | Context/Ink Drawing/Cut | button | `Mod+X` | edit.cut | P0 | wired | FLUENT19:ContextMenuInkDrawing/Cut |  |
| CM.InkDrawing.CopyOneNote | Copy | Context/Ink Drawing/Copy | button | `Mod+C` | edit.copy | P0 | wired | FLUENT19:ContextMenuInkDrawing/CopyOneNote |  |
| CM.InkDrawing.PasteGalleryMini | Paste Options | Context/Ink Drawing/Paste Options | gallery |  | edit.pasteOptions | P0 | wired | FLUENT19:ContextMenuInkDrawing/PasteGalleryMini | A row of the four paste choices (Keep Source Formatting, Merge Formatting, Keep Text Only, Picture) under a "Paste Options:" caption; each reads the system clipboard as Ctrl+V does (order and icons unverified) |
| CM.InkDrawing.HyperlinkCreate | Link… | Context/Ink Drawing/Link… | button | `Mod+K` | insert.link | P0 | wired | FLUENT19:ContextMenuInkDrawing/HyperlinkCreate |  |
| CM.InkDrawing.OrderObjectsMenu | Order | Context/Ink Drawing/Order | menu |  |  | P1 | wired | FLUENT19:ContextMenuInkDrawing/OrderObjectsMenu |  |
| CM.InkDrawing.ObjectBringForward | Bring Forward | Context/Ink Drawing/Order/Bring Forward | button |  | object.bringForward | P1 | wired | FLUENT19:ContextMenuInkDrawing/ObjectBringForward |  |
| CM.InkDrawing.ObjectBringToFront | Bring to Front | Context/Ink Drawing/Order/Bring to Front | button |  | object.bringToFront | P1 | wired | FLUENT19:ContextMenuInkDrawing/ObjectBringToFront |  |
| CM.InkDrawing.ObjectSendBackward | Send Backward | Context/Ink Drawing/Order/Send Backward | button |  | object.sendBackward | P1 | wired | FLUENT19:ContextMenuInkDrawing/ObjectSendBackward |  |
| CM.InkDrawing.ObjectSendToBack | Send to Back | Context/Ink Drawing/Order/Send to Back | button |  | object.sendToBack | P1 | wired | FLUENT19:ContextMenuInkDrawing/ObjectSendToBack |  |
| CM.InkDrawing.RotateInkMenu | Rotate | Context/Ink Drawing/Rotate | menu |  |  | P1 | wired | FLUENT19:ContextMenuInkDrawing/RotateInkMenu |  |
| CM.InkDrawing.RotateRight45OneNote | Rotate Right 45° | Context/Ink Drawing/Rotate/Rotate Right 45° | button |  | object.rotateRight45 | P2 | wired | FLUENT19:ContextMenuInkDrawing/RotateRight45OneNote |  |
| CM.InkDrawing.RotateLeft45OneNote | Rotate Left 45° | Context/Ink Drawing/Rotate/Rotate Left 45° | button |  | object.rotateLeft45 | P2 | wired | FLUENT19:ContextMenuInkDrawing/RotateLeft45OneNote |  |
| CM.InkDrawing.ObjectRotateRight90 | Rotate Right 90° | Context/Ink Drawing/Rotate/Rotate Right 90° | button |  | object.rotateRight90 | P1 | wired | FLUENT19:ContextMenuInkDrawing/ObjectRotateRight90 |  |
| CM.InkDrawing.ObjectRotateLeft90 | Rotate Left 90° | Context/Ink Drawing/Rotate/Rotate Left 90° | button |  | object.rotateLeft90 | P1 | wired | FLUENT19:ContextMenuInkDrawing/ObjectRotateLeft90 |  |
| CM.InkDrawing.ObjectFlipHorizontal | Flip Horizontal | Context/Ink Drawing/Rotate/Flip Horizontal | button |  | object.flipHorizontal | P1 | wired | FLUENT19:ContextMenuInkDrawing/ObjectFlipHorizontal |  |
| CM.InkDrawing.ObjectFlipVertical | Flip Vertical | Context/Ink Drawing/Rotate/Flip Vertical | button |  | object.flipVertical | P1 | wired | FLUENT19:ContextMenuInkDrawing/ObjectFlipVertical |  |
| CM.InkDrawing.TreatInkAsMenu | Treat Selected Ink As | Context/Ink Drawing/Treat Selected Ink As | menu |  |  | P2 | missing | FLUENT19:ContextMenuInkDrawing/TreatInkAsMenu |  |
| CM.InkDrawing.TreatInkAsHandwriting | Handwriting | Context/Ink Drawing/Treat Selected Ink As/Handwriting | button |  | draw.treatAsHandwriting | P2 | missing | FLUENT19:ContextMenuInkDrawing/TreatInkAsHandwriting |  |
| CM.InkDrawing.TreatInkAsDrawing | Drawing | Context/Ink Drawing/Treat Selected Ink As/Drawing | button |  | draw.treatAsDrawing | P2 | missing | FLUENT19:ContextMenuInkDrawing/TreatInkAsDrawing |  |
| CM.InkDrawing.ObjectMove | Move | Context/Ink Drawing/Move | button |  | object.move | P1 | wired | FLUENT19:ContextMenuInkDrawing/ObjectMove |  |
| CM.InkDrawing.ObjectResize | Resize | Context/Ink Drawing/Resize | button |  | object.resize | P1 | wired | FLUENT19:ContextMenuInkDrawing/ObjectResize |  |
| CM.InkDrawing.SelectionToMathConvert | Ink to Math | Context/Ink Drawing/Ink to Math | button |  | draw.inkToMath | P2 | missing | FLUENT19:ContextMenuInkDrawing/SelectionToMathConvert |  |
| CM.InkDrawing.TagsGallery | Tags | Context/Ink Drawing/Tags | gallery |  | tag.toggle | P0 | wired | FLUENT19:ContextMenuInkDrawing/TagsGallery | Decision D5: a drawing carries the tag on its leftmost selected stroke; the icon sits left of that stroke, level with its top, and Find Tags lists it as "Ink drawing" (placement and wording unverified) |
| CM.InkDrawing.TagsCustomize | Customize Tags | Context/Ink Drawing/Tags/Customize Tags | button |  | tag.customize | P1 | wired | FLUENT19:ContextMenuInkDrawing/TagsCustomize | Opens the Customize Tags dialog, as on the Home tab |
| CM.InkDrawing.TagsRemoveAll | Remove Tag | Context/Ink Drawing/Tags/Remove Tag | button | `Mod+0` | tag.removeAll | P0 | wired | FLUENT19:ContextMenuInkDrawing/TagsRemoveAll | Clears the tags of every selected stroke (D5) |
| CM.InkDrawing.TagsMarkAll | Check All Tags | Context/Ink Drawing/Check All Tags | button |  | tag.checkAll | P2 | wired | FLUENT19:ContextMenuInkDrawing/TagsMarkAll |  |
| CM.InkDrawing.TagsClearAll | Clear All Checkmarks | Context/Ink Drawing/Clear All Checkmarks | button |  | tag.clearAll | P2 | wired | FLUENT19:ContextMenuInkDrawing/TagsClearAll |  |
| CM.InkDrawing.TagsRemoveAll2 | Remove Tag | Context/Ink Drawing/Remove Tag | button | `Mod+0` | tag.removeAll | P0 | wired | FLUENT19:ContextMenuInkDrawing/TagsRemoveAll | Clears the tags of every selected stroke (D5) |
| CM.InkDrawing.PenInsertSpace | Insert Space | Context/Ink Drawing/Insert Space | toggle |  | insert.space | P1 | wired | FLUENT19:ContextMenuInkDrawing/PenInsertSpace |  |
| CM.LassoSelection.Cut | Cut | Context/Lasso Selection/Cut | button | `Mod+X` | edit.cut | P0 | wired | FLUENT19:ContextMenuLassoSelection/Cut |  |
| CM.LassoSelection.CopyOneNote | Copy | Context/Lasso Selection/Copy | button | `Mod+C` | edit.copy | P0 | wired | FLUENT19:ContextMenuLassoSelection/CopyOneNote |  |
| CM.LassoSelection.Delete | Delete | Context/Lasso Selection/Delete | button | `Delete` | edit.delete | P0 | wired | FLUENT19:ContextMenuLassoSelection/Delete |  |
| CM.LassoSelection.RotateInkMenu | Rotate | Context/Lasso Selection/Rotate | menu |  |  | P1 | wired | FLUENT19:ContextMenuLassoSelection/RotateInkMenu |  |
| CM.LassoSelection.RotateRight45OneNote | Rotate Right 45° | Context/Lasso Selection/Rotate/Rotate Right 45° | button |  | object.rotateRight45 | P2 | wired | FLUENT19:ContextMenuLassoSelection/RotateRight45OneNote |  |
| CM.LassoSelection.RotateLeft45OneNote | Rotate Left 45° | Context/Lasso Selection/Rotate/Rotate Left 45° | button |  | object.rotateLeft45 | P2 | wired | FLUENT19:ContextMenuLassoSelection/RotateLeft45OneNote |  |
| CM.LassoSelection.ObjectRotateRight90 | Rotate Right 90° | Context/Lasso Selection/Rotate/Rotate Right 90° | button |  | object.rotateRight90 | P1 | wired | FLUENT19:ContextMenuLassoSelection/ObjectRotateRight90 |  |
| CM.LassoSelection.ObjectRotateLeft90 | Rotate Left 90° | Context/Lasso Selection/Rotate/Rotate Left 90° | button |  | object.rotateLeft90 | P1 | wired | FLUENT19:ContextMenuLassoSelection/ObjectRotateLeft90 |  |
| CM.LassoSelection.ObjectFlipHorizontal | Flip Horizontal | Context/Lasso Selection/Rotate/Flip Horizontal | button |  | object.flipHorizontal | P1 | wired | FLUENT19:ContextMenuLassoSelection/ObjectFlipHorizontal |  |
| CM.LassoSelection.ObjectFlipVertical | Flip Vertical | Context/Lasso Selection/Rotate/Flip Vertical | button |  | object.flipVertical | P1 | wired | FLUENT19:ContextMenuLassoSelection/ObjectFlipVertical |  |
| CM.LassoSelection.TreatInkAsMenu | Treat Selected Ink As | Context/Lasso Selection/Treat Selected Ink As | menu |  |  | P2 | missing | FLUENT19:ContextMenuLassoSelection/TreatInkAsMenu |  |
| CM.LassoSelection.TreatInkAsHandwriting | Handwriting | Context/Lasso Selection/Treat Selected Ink As/Handwriting | button |  | draw.treatAsHandwriting | P2 | missing | FLUENT19:ContextMenuLassoSelection/TreatInkAsHandwriting |  |
| CM.LassoSelection.TreatInkAsDrawing | Drawing | Context/Lasso Selection/Treat Selected Ink As/Drawing | button |  | draw.treatAsDrawing | P2 | missing | FLUENT19:ContextMenuLassoSelection/TreatInkAsDrawing |  |
| CM.LassoSelection.SelectionToTextConvert | Ink to Text | Context/Lasso Selection/Ink to Text | button |  | draw.inkToText | P2 | missing | FLUENT19:ContextMenuLassoSelection/SelectionToTextConvert |  |
| CM.LassoSelection.SelectionToMathConvert | Ink to Math | Context/Lasso Selection/Ink to Math | button |  | draw.inkToMath | P2 | missing | FLUENT19:ContextMenuLassoSelection/SelectionToMathConvert |  |
| CM.Picture.OleObjectEdit | Edit (embedded object) | Context/Picture/Edit (embedded object) | button |  | object.editOle | P3 | missing | FLUENT19:ContextMenuPicture/OleObjectEdit |  |
| CM.Picture.RefreshOleObjects | Refresh | Context/Picture/Refresh | button |  | printout.refresh | P2 | missing | FLUENT19:ContextMenuPicture/RefreshOleObjects |  |
| CM.Picture.SelectWhatToDisplay | Display As | Context/Picture/Display As | button |  | printout.displayAs | P2 | missing | FLUENT19:ContextMenuPicture/SelectWhatToDisplay |  |
| CM.Picture.Cut | Cut | Context/Picture/Cut | button | `Mod+X` | edit.cut | P0 | wired | FLUENT19:ContextMenuPicture/Cut |  |
| CM.Picture.CopyOneNote | Copy | Context/Picture/Copy | button | `Mod+C` | edit.copy | P0 | wired | FLUENT19:ContextMenuPicture/CopyOneNote | copies the picture as an object inside NoteCraft |
| CM.Picture.CopyTextFromPicture | Copy Text from Picture | Context/Picture/Copy Text from Picture | button |  | object.copyTextFromPicture | P2 | missing | FLUENT19:ContextMenuPicture/CopyTextFromPicture |  |
| CM.Picture.PrintoutPageTextCopy | Copy Text from This Page of the Printout | Context/Picture/Copy Text from This Page of the Printout | button |  | printout.copyPageText | P2 | missing | FLUENT19:ContextMenuPicture/PrintoutPageTextCopy |  |
| CM.Picture.CopyTextFromPrintoutPages | Copy Text from All Pages of the Printout | Context/Picture/Copy Text from All Pages of the Printout | button |  | printout.copyAllText | P2 | missing | FLUENT19:ContextMenuPicture/CopyTextFromPrintoutPages |  |
| CM.Picture.ZoomToDocument | Zoom to Document | Context/Picture/Zoom to Document | button |  | printout.zoomToDocument | P3 | missing | FLUENT19:ContextMenuPicture/ZoomToDocument |  |
| CM.Picture.ViewOriginalPrintout | Open Original | Context/Picture/Open Original | button |  | printout.openOriginal | P2 | wired | FLUENT19:ContextMenuPicture/ViewOriginalPrintout |  |
| CM.Picture.PasteGalleryMini | Paste Options | Context/Picture/Paste Options | gallery |  | edit.pasteOptions | P0 | wired | FLUENT19:ContextMenuPicture/PasteGalleryMini | A row of the four paste choices (Keep Source Formatting, Merge Formatting, Keep Text Only, Picture) under a "Paste Options:" caption; each reads the system clipboard as Ctrl+V does (order and icons unverified) |
| CM.Picture.ImageSaveAs | Save As (picture) | Context/Picture/Save As (picture) | button |  | object.saveAs | P1 | wired | FLUENT19:ContextMenuPicture/ImageSaveAs | writes the stored file unchanged (rotation and flips are view properties) |
| CM.Picture.HyperlinkOpen | Open Link | Context/Picture/Open Link | button | `Enter` | link.open | P0 | wired | FLUENT19:ContextMenuPicture/HyperlinkOpen |  |
| CM.Picture.LinkEdit | Edit Link… | Context/Picture/Edit Link… | button |  | link.edit | P0 | wired | FLUENT19:ContextMenuPicture/LinkEdit |  |
| CM.Picture.HyperlinkCreate | Link… | Context/Picture/Link… | button | `Mod+K` | insert.link | P0 | wired | FLUENT19:ContextMenuPicture/HyperlinkCreate |  |
| CM.Picture.LinkCopyOneNote | Copy Link | Context/Picture/Copy Link | button |  | link.copy | P0 | wired | FLUENT19:ContextMenuPicture/LinkCopyOneNote |  |
| CM.Picture.LinkRemove | Remove Link | Context/Picture/Remove Link | button |  | link.remove | P0 | wired | FLUENT19:ContextMenuPicture/LinkRemove |  |
| CM.Picture.OrderObjectsMenu | Order | Context/Picture/Order | menu |  |  | P1 | wired | FLUENT19:ContextMenuPicture/OrderObjectsMenu |  |
| CM.Picture.ObjectBringForward | Bring Forward | Context/Picture/Order/Bring Forward | button |  | object.bringForward | P1 | wired | FLUENT19:ContextMenuPicture/ObjectBringForward | pictures are drawn under text; the order applies among pictures and attachments |
| CM.Picture.ObjectBringToFront | Bring to Front | Context/Picture/Order/Bring to Front | button |  | object.bringToFront | P1 | wired | FLUENT19:ContextMenuPicture/ObjectBringToFront | pictures are drawn under text; the order applies among pictures and attachments |
| CM.Picture.ObjectSendBackward | Send Backward | Context/Picture/Order/Send Backward | button |  | object.sendBackward | P1 | wired | FLUENT19:ContextMenuPicture/ObjectSendBackward | pictures are drawn under text; the order applies among pictures and attachments |
| CM.Picture.ObjectSendToBack | Send to Back | Context/Picture/Order/Send to Back | button |  | object.sendToBack | P1 | wired | FLUENT19:ContextMenuPicture/ObjectSendToBack | pictures are drawn under text; the order applies among pictures and attachments |
| CM.Picture.RotateInkMenu | Rotate | Context/Picture/Rotate | menu |  |  | P1 | wired | FLUENT19:ContextMenuPicture/RotateInkMenu |  |
| CM.Picture.RotateRight45OneNote | Rotate Right 45° | Context/Picture/Rotate/Rotate Right 45° | button |  | object.rotateRight45 | P2 | missing | FLUENT19:ContextMenuPicture/RotateRight45OneNote |  |
| CM.Picture.RotateLeft45OneNote | Rotate Left 45° | Context/Picture/Rotate/Rotate Left 45° | button |  | object.rotateLeft45 | P2 | missing | FLUENT19:ContextMenuPicture/RotateLeft45OneNote |  |
| CM.Picture.ObjectRotateRight90 | Rotate Right 90° | Context/Picture/Rotate/Rotate Right 90° | button |  | object.rotateRight90 | P1 | verified | FLUENT19:ContextMenuPicture/ObjectRotateRight90 | turns about the picture's centre |
| CM.Picture.ObjectRotateLeft90 | Rotate Left 90° | Context/Picture/Rotate/Rotate Left 90° | button |  | object.rotateLeft90 | P1 | verified | FLUENT19:ContextMenuPicture/ObjectRotateLeft90 | turns about the picture's centre |
| CM.Picture.ObjectFlipHorizontal | Flip Horizontal | Context/Picture/Rotate/Flip Horizontal | button |  | object.flipHorizontal | P1 | verified | FLUENT19:ContextMenuPicture/ObjectFlipHorizontal |  |
| CM.Picture.ObjectFlipVertical | Flip Vertical | Context/Picture/Rotate/Flip Vertical | button |  | object.flipVertical | P1 | verified | FLUENT19:ContextMenuPicture/ObjectFlipVertical |  |
| CM.Picture.ObjectMove | Move | Context/Picture/Move | button |  | object.move | P1 | wired | FLUENT19:ContextMenuPicture/ObjectMove | unverified: from the menu it explains the gesture; drag, or arrow keys (1 pt, Ctrl 10 pt) |
| CM.Picture.ObjectResize | Resize | Context/Picture/Resize | button |  | object.resize | P1 | wired | FLUENT19:ContextMenuPicture/ObjectResize | unverified: from the menu it explains the gesture; corner handles keep the proportions, edge handles change one side, Shift+arrow keys |
| CM.Picture.RestoreImageSize | Restore to Original Size | Context/Picture/Restore to Original Size | button |  | object.restoreSize | P1 | verified | FLUENT19:ContextMenuPicture/RestoreImageSize | the pixel size at 96 dpi |
| CM.Picture.PictureAsBackground | Set Picture as Background | Context/Picture/Set Picture as Background | toggle |  | object.setAsBackground | P1 | wired | FLUENT19:ContextMenuPicture/PictureAsBackground | unverified: a background picture goes behind everything and only a right click reaches it |
| CM.Picture.AlternativeText | Alt Text | Context/Picture/Alt Text | button |  | object.altText | P1 | wired | FLUENT19:ContextMenuPicture/AlternativeText |  |
| CM.Picture.OCRLanguageListMenu | Make Text in Image Searchable | Context/Picture/Make Text in Image Searchable | menu |  | object.ocrLanguage | P2 | missing | FLUENT19:ContextMenuPicture/OCRLanguageListMenu |  |
| CM.FileAttachment.FileAttachmentName | File name (header) | Context/File Attachment/File name (header) | labelControl |  |  | P2 | wired | FLUENT19:ContextMenuFileAttachment/FileAttachmentName | The attachment's file name, a non-clickable header |
| CM.FileAttachment.FileAttachmentSize | File size (header) | Context/File Attachment/File size (header) | labelControl |  |  | P2 | wired | FLUENT19:ContextMenuFileAttachment/FileAttachmentSize | The file size in bytes, KB or MB of 1,024 (wording unverified) |
| CM.FileAttachment.PlayFromPage | Play (attached media) | Context/File Attachment/Play (attached media) | button |  | media.play | P2 | missing | FLUENT19:ContextMenuFileAttachment/PlayFromPage |  |
| CM.FileAttachment.FileAttachmentOpen | Open | Context/File Attachment/Open | button |  | attachment.open | P1 | wired | FLUENT19:ContextMenuFileAttachment/FileAttachmentOpen | writes a copy to a private temporary folder and opens it with the system handler (desktop); double-click does the same |
| CM.FileAttachment.FileAttachmentSaveAs | Save As… | Context/File Attachment/Save As… | button |  | attachment.saveAs | P1 | wired | FLUENT19:ContextMenuFileAttachment/FileAttachmentSaveAs |  |
| CM.FileAttachment.RenameSelected | Rename | Context/File Attachment/Rename | button |  | attachment.rename | P1 | wired | FLUENT19:ContextMenuFileAttachment/RenameSelected |  |
| CM.FileAttachment.FilePrintoutInsert | Insert Printout | Context/File Attachment/Insert Printout | button |  | attachment.insertPrintout | P2 | wired | FLUENT19:ContextMenuFileAttachment/FilePrintoutInsert | replaces pages printed earlier from the same file |
| CM.FileAttachment.RefreshPrintoutPages | Refresh Printout | Context/File Attachment/Refresh Printout | button |  | printout.refresh | P2 | missing | FLUENT19:ContextMenuFileAttachment/RefreshPrintoutPages |  |
| CM.FileAttachment.JotDeletePrintoutPages | Delete Printout Pages | Context/File Attachment/Delete Printout Pages | button |  | printout.delete | P2 | wired | FLUENT19:ContextMenuFileAttachment/JotDeletePrintoutPages |  |
| CM.FileAttachment.SelectWhatToDisplay | Display As | Context/File Attachment/Display As | button |  | printout.displayAs | P2 | missing | FLUENT19:ContextMenuFileAttachment/SelectWhatToDisplay |  |
| CM.FileAttachment.RefreshOleObjects | Refresh | Context/File Attachment/Refresh | button |  | printout.refresh | P2 | missing | FLUENT19:ContextMenuFileAttachment/RefreshOleObjects |  |
| CM.FileAttachment.OriginalFileOpen | Open Original | Context/File Attachment/Open Original | button |  | attachment.openOriginal | P2 | missing | FLUENT19:ContextMenuFileAttachment/OriginalFileOpen |  |
| CM.FileAttachment.FileAttachmentShortcutToOriginalCopy | Copy Link to File | Context/File Attachment/Copy Link to File | button |  | attachment.copyLink | P2 | missing | FLUENT19:ContextMenuFileAttachment/FileAttachmentShortcutToOriginalCopy |  |
| CM.FileAttachment.Cut | Cut | Context/File Attachment/Cut | button | `Mod+X` | edit.cut | P0 | wired | FLUENT19:ContextMenuFileAttachment/Cut |  |
| CM.FileAttachment.CopyOneNote | Copy | Context/File Attachment/Copy | button | `Mod+C` | edit.copy | P0 | wired | FLUENT19:ContextMenuFileAttachment/CopyOneNote |  |
| CM.FileAttachment.PasteGalleryMini | Paste Options | Context/File Attachment/Paste Options | gallery |  | edit.pasteOptions | P0 | wired | FLUENT19:ContextMenuFileAttachment/PasteGalleryMini | A row of the four paste choices (Keep Source Formatting, Merge Formatting, Keep Text Only, Picture) under a "Paste Options:" caption; each reads the system clipboard as Ctrl+V does (order and icons unverified) |
| CM.FileAttachment.HyperlinkCreate | Link… | Context/File Attachment/Link… | button | `Mod+K` | insert.link | P0 | wired | FLUENT19:ContextMenuFileAttachment/HyperlinkCreate |  |
| CM.FileAttachment.AlternativeText | Alt Text | Context/File Attachment/Alt Text | button |  | object.altText | P1 | wired | FLUENT19:ContextMenuFileAttachment/AlternativeText |  |
| CM.Tag.NoteTagCheck | Check Tag | Context/Tag/Check Tag | button |  | tag.check | P0 | wired | FLUENT19:ContextMenuTag/NoteTagCheck |  |
| CM.Tag.NoteTagUncheck | Uncheck Tag | Context/Tag/Uncheck Tag | button |  | tag.uncheck | P0 | wired | FLUENT19:ContextMenuTag/NoteTagUncheck |  |
| CM.Tag.TagsRemove | Remove Tag | Context/Tag/Remove Tag | button |  | tag.remove | P0 | wired | FLUENT19:ContextMenuTag/TagsRemove |  |
| CM.Tag.FindTaggedNotes | Find Tags | Context/Tag/Find Tags | toggle |  | tag.findTags | P0 | missing | FLUENT19:ContextMenuTag/FindTaggedNotes |  |
| CM.Tag.TagsCustomize | Customize Tags | Context/Tag/Customize Tags | button |  | tag.customize | P1 | missing | FLUENT19:ContextMenuTag/TagsCustomize |  |
| CM.Tag.JotAddToCustomNoteFlag | Add to Custom Tags | Context/Tag/Add to Custom Tags | button |  | tag.addToCustom | P2 | missing | FLUENT19:ContextMenuTag/JotAddToCustomNoteFlag |  |
| CM.OutlookTask.OutlookTaskTodayContextual | Outlook Task: Today | Context/Outlook Task/Outlook Task: Today | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuOutlookTask/OutlookTaskTodayContextual |  |
| CM.OutlookTask.OutlookTaskTomorrowContextual | Outlook Task: Tomorrow | Context/Outlook Task/Outlook Task: Tomorrow | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuOutlookTask/OutlookTaskTomorrowContextual |  |
| CM.OutlookTask.OutlookTaskThisWeekContextual | Outlook Task: This Week | Context/Outlook Task/Outlook Task: This Week | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuOutlookTask/OutlookTaskThisWeekContextual |  |
| CM.OutlookTask.OutlookTaskNextWeekContextual | Outlook Task: Next Week | Context/Outlook Task/Outlook Task: Next Week | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuOutlookTask/OutlookTaskNextWeekContextual |  |
| CM.OutlookTask.OutlookTaskLaterContextual | Outlook Task: No Date | Context/Outlook Task/Outlook Task: No Date | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuOutlookTask/OutlookTaskLaterContextual |  |
| CM.OutlookTask.OutlookTaskCustomDateContextual | Outlook Task: Custom | Context/Outlook Task/Outlook Task: Custom | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuOutlookTask/OutlookTaskCustomDateContextual |  |
| CM.OutlookTask.OutlookTaskMarkCompleteContextual | Mark Complete (Outlook task) | Context/Outlook Task/Mark Complete (Outlook task) | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuOutlookTask/OutlookTaskMarkCompleteContextual |  |
| CM.OutlookTask.OutlookTaskRemoveContextual | Delete Outlook Task | Context/Outlook Task/Delete Outlook Task | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuOutlookTask/OutlookTaskRemoveContextual |  |
| CM.OutlookTask.TagsRemove | Remove Tag | Context/Outlook Task/Remove Tag | button |  | tag.remove | P0 | missing | FLUENT19:ContextMenuOutlookTask/TagsRemove |  |
| CM.OutlookTask.OutlookTaskOpenContextual | Open Task in Outlook | Context/Outlook Task/Open Task in Outlook | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuOutlookTask/OutlookTaskOpenContextual |  |
| CM.Notebook.CurrentNotebookSyncNow | Sync This Notebook Now | Context/Notebook/Sync This Notebook Now | button | `Shift+F9` | notebook.syncNow | P2 | missing | FLUENT19:ContextMenuNotebook/CurrentNotebookSyncNow |  |
| CM.Notebook.GroupSyncStatus | Notebook Sync Status | Context/Notebook/Notebook Sync Status | button |  | notebook.syncStatus | P2 | missing | FLUENT19:ContextMenuNotebook/GroupSyncStatus |  |
| CM.Notebook.ShareThisNotebook | Share This Notebook | Context/Notebook/Share This Notebook | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuNotebook/ShareThisNotebook |  |
| CM.Notebook.NotebookLinkCreate | Copy Link to Notebook | Context/Notebook/Copy Link to Notebook | button |  | notebook.copyLink | P1 | wired | FLUENT19:ContextMenuNotebook/NotebookLinkCreate |  |
| CM.Notebook.MarkAllPagesAsRead | Mark Notebook as Read | Context/Notebook/Mark Notebook as Read | button |  | history.markNotebookRead | P2 | missing | FLUENT19:ContextMenuNotebook/MarkAllPagesAsRead |  |
| CM.Notebook.NotebookMoveUp | Move Up | Context/Notebook/Move Up | button |  | notebook.moveUp | P1 | wired | FLUENT19:ContextMenuNotebook/NotebookMoveUp |  |
| CM.Notebook.NotebookMoveDown | Move Down | Context/Notebook/Move Down | button |  | notebook.moveDown | P1 | wired | FLUENT19:ContextMenuNotebook/NotebookMoveDown |  |
| CM.Notebook.RecycleBin | Notebook Recycle Bin | Context/Notebook/Notebook Recycle Bin | toggle |  | history.recycleBin | P0 | wired | FLUENT19:ContextMenuNotebook/RecycleBin |  |
| CM.Notebook.NotebookProperties | Properties… | Context/Notebook/Properties… | button |  | notebook.properties | P1 | wired | FLUENT19:ContextMenuNotebook/NotebookProperties |  |
| CM.Notebook.NotebookClose | Close This Notebook | Context/Notebook/Close This Notebook | button |  | notebook.close | P0 | wired | FLUENT19:ContextMenuNotebook/NotebookClose |  |
| CM.SectionGroup.FolderRename | Rename | Context/Section Group/Rename | button |  | sectionGroup.rename | P0 | wired | FLUENT19:ContextMenuSectionGroup/FolderRename |  |
| CM.SectionGroup.FolderMove | Move… | Context/Section Group/Move… | button |  | sectionGroup.move | P1 | wired | FLUENT19:ContextMenuSectionGroup/FolderMove |  |
| CM.SectionGroup.FolderDelete | Delete | Context/Section Group/Delete | button |  | sectionGroup.delete | P0 | wired | FLUENT19:ContextMenuSectionGroup/FolderDelete |  |
| CM.SectionGroup.SectionGroupLinkCreate | Copy Link to Section Group | Context/Section Group/Copy Link to Section Group | button |  | sectionGroup.copyLink | P2 | wired | FLUENT19:ContextMenuSectionGroup/SectionGroupLinkCreate |  |
| CM.SectionGroup.MarkPageAsRead | Mark as Read | Context/Section Group/Mark as Read | button |  | history.markRead | P2 | missing | FLUENT19:ContextMenuSectionGroup/MarkPageAsRead |  |
| CM.SectionGroup.SectionInsert | New Section | Context/Section Group/New Section | button | `Mod+T` | section.new | P0 | wired | FLUENT19:ContextMenuSectionGroup/SectionInsert |  |
| CM.SectionGroup.SectionGroupInsert | New Section Group | Context/Section Group/New Section Group | button |  | sectionGroup.new | P0 | wired | FLUENT19:ContextMenuSectionGroup/SectionGroupInsert |  |
| CM.Section.SectionRenameOneNote | Rename | Context/Section/Rename | button |  | section.rename | P0 | wired | FLUENT19:ContextMenuSection/SectionRenameOneNote |  |
| CM.Section.FileSaveAsContextualItem | Export… | Context/Section/Export… | button |  | file.exportSection | P1 | missing | FLUENT19:ContextMenuSection/FileSaveAsContextualItem |  |
| CM.Section.NotebookDelete | Delete | Context/Section/Delete | button |  | section.delete | P0 | wired | FLUENT19:ContextMenuSection/NotebookDelete |  |
| CM.Section.EmptyTrash | Empty Deleted Pages | Context/Section/Empty Deleted Pages | button |  | history.emptyRecycleBin | P2 | missing | FLUENT19:ContextMenuSection/EmptyTrash |  |
| CM.Section.SectionMove | Move or Copy… | Context/Section/Move or Copy… | button |  | section.moveOrCopy | P0 | verified | FLUENT19:ContextMenuSection/SectionMove |  |
| CM.Section.SectionsMerge | Merge into Another Section… | Context/Section/Merge into Another Section… | button |  | section.merge | P1 | verified | FLUENT19:ContextMenuSection/SectionsMerge |  |
| CM.Section.SectionClose | Close (section) | Context/Section/Close (section) | button |  | section.close | P2 | missing | FLUENT19:ContextMenuSection/SectionClose |  |
| CM.Section.ShortcutCopy | Copy (section) | Context/Section/Copy (section) | button |  | section.copy | P2 | missing | FLUENT19:ContextMenuSection/ShortcutCopy |  |
| CM.Section.SectionLinkCreate | Copy Link to Section | Context/Section/Copy Link to Section | button |  | section.copyLink | P1 | wired | FLUENT19:ContextMenuSection/SectionLinkCreate |  |
| CM.Section.SectionInsert | New Section | Context/Section/New Section | button | `Mod+T` | section.new | P0 | wired | FLUENT19:ContextMenuSection/SectionInsert |  |
| CM.Section.SectionGroupInsert | New Section Group | Context/Section/New Section Group | button |  | sectionGroup.new | P0 | wired | FLUENT19:ContextMenuSection/SectionGroupInsert |  |
| CM.Section.MarkPageAsRead | Mark as Read | Context/Section/Mark as Read | button |  | history.markRead | P2 | missing | FLUENT19:ContextMenuSection/MarkPageAsRead |  |
| CM.Section.PasswordProtectSectionContextual | Password Protect This Section… | Context/Section/Password Protect This Section… | button |  | review.password | P1 | missing | FLUENT19:ContextMenuSection/PasswordProtectSectionContextual |  |
| CM.Section.SectionColorMenu | Section Color | Context/Section/Section Color | menu |  | section.color | P0 | verified | FLUENT19:ContextMenuSection/SectionColorMenu |  |
| CM.Section.NotebookColor4 | Section Color 4 | Context/Section/Section Color/Section Color 4 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor4 |  |
| CM.Section.NotebookColor2 | Section Color 2 | Context/Section/Section Color/Section Color 2 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor2 |  |
| CM.Section.NotebookColor3 | Section Color 3 | Context/Section/Section Color/Section Color 3 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor3 |  |
| CM.Section.NotebookColor7 | Section Color 7 | Context/Section/Section Color/Section Color 7 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor7 |  |
| CM.Section.NotebookColor5 | Section Color 5 | Context/Section/Section Color/Section Color 5 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor5 |  |
| CM.Section.NotebookColor8 | Section Color 8 | Context/Section/Section Color/Section Color 8 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor8 |  |
| CM.Section.NotebookColor6 | Section Color 6 | Context/Section/Section Color/Section Color 6 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor6 |  |
| CM.Section.NotebookColor1 | Section Color 1 | Context/Section/Section Color/Section Color 1 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor1 |  |
| CM.Section.NotebookColor9 | Section Color 9 | Context/Section/Section Color/Section Color 9 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor9 |  |
| CM.Section.NotebookColor10 | Section Color 10 | Context/Section/Section Color/Section Color 10 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor10 |  |
| CM.Section.NotebookColor11 | Section Color 11 | Context/Section/Section Color/Section Color 11 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor11 |  |
| CM.Section.NotebookColor12 | Section Color 12 | Context/Section/Section Color/Section Color 12 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor12 |  |
| CM.Section.NotebookColor13 | Section Color 13 | Context/Section/Section Color/Section Color 13 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor13 |  |
| CM.Section.NotebookColor14 | Section Color 14 | Context/Section/Section Color/Section Color 14 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor14 |  |
| CM.Section.NotebookColor15 | Section Color 15 | Context/Section/Section Color/Section Color 15 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor15 |  |
| CM.Section.NotebookColor16 | Section Color 16 | Context/Section/Section Color/Section Color 16 | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColor16 |  |
| CM.Section.NotebookColorNone | None (section color) | Context/Section/Section Color/None (section color) | toggle |  | section.color | P0 | wired | FLUENT19:ContextMenuSection/NotebookColorNone |  |
| CM.Page.RenamePageOneNote | Rename | Context/Page/Rename | button |  | page.rename | P0 | wired | FLUENT19:ContextMenuPage/RenamePageOneNote |  |
| CM.Page.Delete | Delete | Context/Page/Delete | button | `Delete` | edit.delete | P0 | wired | FLUENT19:ContextMenuPage/Delete |  |
| CM.Page.Cut | Cut | Context/Page/Cut | button | `Mod+X` | edit.cut | P0 | wired | FLUENT19:ContextMenuPage/Cut |  |
| CM.Page.CopyOneNote | Copy | Context/Page/Copy | button | `Mod+C` | edit.copy | P0 | wired | FLUENT19:ContextMenuPage/CopyOneNote |  |
| CM.Page.Paste | Paste | Context/Page/Paste | button | `Mod+V` | edit.paste | P0 | wired | FLUENT19:ContextMenuPage/Paste |  |
| CM.Page.PageLinkCreate | Copy Link to Page | Context/Page/Copy Link to Page | button |  | page.copyLink | P0 | wired | FLUENT19:ContextMenuPage/PageLinkCreate |  |
| CM.Page.MoveOrCopyPageContextual | Move or Copy… | Context/Page/Move or Copy… | button | `Mod+Alt+M` | page.moveOrCopy | P0 | wired | FLUENT19:ContextMenuPage/MoveOrCopyPageContextual |  |
| CM.Page.CopyOrMoveToSection | Move or Copy to Section | Context/Page/Move or Copy to Section | button |  | page.moveOrCopy | P1 | missing | FLUENT19:ContextMenuPage/CopyOrMoveToSection |  |
| CM.Page.NewPageCtxItem | New Page | Context/Page/New Page | button |  | page.new | P0 | wired | FLUENT19:ContextMenuPage/NewPageCtxItem |  |
| CM.Page.PagesDemote | Make Subpage | Context/Page/Make Subpage | button | `Mod+Alt+]` | page.demote | P0 | wired | FLUENT19:ContextMenuPage/PagesDemote |  |
| CM.Page.PagesPromote | Promote Subpage | Context/Page/Promote Subpage | button | `Mod+Alt+[` | page.promote | P0 | wired | FLUENT19:ContextMenuPage/PagesPromote |  |
| CM.Page.IndentedPagesShow | Expand Subpages | Context/Page/Expand Subpages | button |  | page.expandSubpages | P0 | verified | FLUENT19:ContextMenuPage/IndentedPagesShow |  |
| CM.Page.IndentedPagesCollapse | Collapse Subpages | Context/Page/Collapse Subpages | button |  | page.collapseSubpages | P0 | verified | FLUENT19:ContextMenuPage/IndentedPagesCollapse |  |
| CM.Page.MarkPageAsUnreadContextual | Mark as Unread | Context/Page/Mark as Unread | button | `Mod+Q` | history.markUnread | P2 | missing | FLUENT19:ContextMenuPage/MarkPageAsUnreadContextual |  |
| CM.Page.MarkPageAsReadContextual | Mark as Read | Context/Page/Mark as Read | button |  | history.markRead | P2 | missing | FLUENT19:ContextMenuPage/MarkPageAsReadContextual |  |
| CM.Page.RefreshMeetingDetails | Refresh Meeting Details | Context/Page/Refresh Meeting Details | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuPage/RefreshMeetingDetails |  |
| CM.Page.DownloadMeetingFileIntoOnenoteMenu | Download Meeting Files | Context/Page/Download Meeting Files | toggle |  |  | P3 | out-of-scope | FLUENT19:ContextMenuPage/DownloadMeetingFileIntoOnenoteMenu |  |
| CM.Page.PageVersionsShow | Show Page Versions | Context/Page/Show Page Versions | button |  | history.pageVersions | P1 | missing | FLUENT19:ContextMenuPage/PageVersionsShow |  |
| CM.Page.PageVersionsHide | Hide Page Versions | Context/Page/Hide Page Versions | button |  | history.pageVersions | P1 | missing | FLUENT19:ContextMenuPage/PageVersionsHide |  |
| CM.Page.ConflictPagesShow | Show Conflict Pages | Context/Page/Show Conflict Pages | toggle |  | sync.showConflicts | P2 | missing | FLUENT19:ContextMenuPage/ConflictPagesShow |  |
| CM.ConflictPage.ConflictPageDelete | Delete Conflict Page | Context/Conflict Page/Delete Conflict Page | button |  | sync.deleteConflict | P2 | missing | FLUENT19:ContextMenuConflictPage/ConflictPageDelete |  |
| CM.ConflictPage.CopyOrMoveToSection | Move or Copy to Section | Context/Conflict Page/Move or Copy to Section | button |  | page.moveOrCopy | P1 | missing | FLUENT19:ContextMenuConflictPage/CopyOrMoveToSection |  |
| CM.ConflictPage.GoToPreviousConflict | Previous Conflict | Context/Conflict Page/Previous Conflict | button |  | sync.prevConflict | P2 | missing | FLUENT19:ContextMenuConflictPage/GoToPreviousConflict |  |
| CM.ConflictPage.GoToNextConflict | Next Conflict | Context/Conflict Page/Next Conflict | button |  | sync.nextConflict | P2 | missing | FLUENT19:ContextMenuConflictPage/GoToNextConflict |  |
| CM.ConflictPage.ConflictPageCollapse | Collapse Conflict Pages | Context/Conflict Page/Collapse Conflict Pages | button |  | sync.collapseConflicts | P2 | missing | FLUENT19:ContextMenuConflictPage/ConflictPageCollapse |  |
| CM.PageVersion.PageVersionRestore | Restore Version | Context/Page Version/Restore Version | button |  | history.restoreVersion | P1 | missing | FLUENT19:ContextMenuVersionPage/PageVersionRestore |  |
| CM.PageVersion.PageVersionDelete | Delete Version | Context/Page Version/Delete Version | button |  | history.deleteVersion | P1 | missing | FLUENT19:ContextMenuVersionPage/PageVersionDelete |  |
| CM.PageVersion.CopyOrMoveToSection | Move or Copy to Section | Context/Page Version/Move or Copy to Section | button |  | page.moveOrCopy | P1 | missing | FLUENT19:ContextMenuVersionPage/CopyOrMoveToSection |  |
| CM.PageVersion.DeleteAllPageVersionsInSection | Delete All Versions in Section | Context/Page Version/Delete All Versions in Section | button |  | history.deleteVersionsSection | P2 | missing | FLUENT19:ContextMenuVersionPage/DeleteAllPageVersionsInSection |  |
| CM.PageVersion.DeleteAllPageVersionsInFolder | Delete All Versions in Section Group | Context/Page Version/Delete All Versions in Section Group | button |  | history.deleteVersionsGroup | P2 | missing | FLUENT19:ContextMenuVersionPage/DeleteAllPageVersionsInFolder |  |
| CM.PageVersion.DeleteAllPageVersionsInNotebook | Delete All Versions in Notebook | Context/Page Version/Delete All Versions in Notebook | button |  | history.deleteVersionsNotebook | P2 | missing | FLUENT19:ContextMenuVersionPage/DeleteAllPageVersionsInNotebook |  |
| CM.PageVersion.DisableVersionHistory | Disable History for This Notebook | Context/Page Version/Disable History for This Notebook | toggle |  | history.disable | P2 | missing | FLUENT19:ContextMenuVersionPage/DisableVersionHistory |  |
| CM.PageVersion.PageVersionsHide | Hide Page Versions | Context/Page Version/Hide Page Versions | button |  | history.pageVersions | P1 | missing | FLUENT19:ContextMenuVersionPage/PageVersionsHide |  |
| CM.PageListArea.NewPageAtSectionEndContextual | New Page | Context/Page List Area/New Page | button | `Mod+N` | page.new | P0 | wired | FLUENT19:ContextMenuPageArea/NewPageAtSectionEndContextual |  |
| CM.PageListArea.PastePage | Paste Page | Context/Page List Area/Paste Page | button |  | page.paste | P1 | wired | FLUENT19:ContextMenuPageArea/PastePage |  |
| CM.SectionTabsArea.SectionInsert | New Section | Context/Section Tabs Area/New Section | button | `Mod+T` | section.new | P0 | out-of-scope | FLUENT19:ContextMenuSectionArea/SectionInsert | No section tabs area in the 2024 UI (decision D1); the same command is in CM.NotebookPaneArea.SectionInsert. |
| CM.SectionTabsArea.SectionGroupInsert | New Section Group | Context/Section Tabs Area/New Section Group | button |  | sectionGroup.new | P0 | out-of-scope | FLUENT19:ContextMenuSectionArea/SectionGroupInsert | No section tabs area in the 2024 UI (decision D1); the same command is in CM.NotebookPaneArea.SectionGroupInsert. |
| CM.NotebookPaneArea.SectionInsert | New Section | Context/Notebook Pane Area/New Section | button | `Mod+T` | section.new | P0 | wired | FLUENT19:ContextMenuNotebookArea/SectionInsert |  |
| CM.NotebookPaneArea.SectionGroupInsert | New Section Group | Context/Notebook Pane Area/New Section Group | button |  | sectionGroup.new | P0 | wired | FLUENT19:ContextMenuNotebookArea/SectionGroupInsert |  |
| CM.NotebookPaneArea.NotebookNew | New Notebook | Context/Notebook Pane Area/New Notebook | button |  | file.newNotebook | P0 | wired | FLUENT19:ContextMenuNotebookArea/NotebookNew |  |
| CM.NotebookPaneArea.NotebookOpen | Open Notebook | Context/Notebook Pane Area/Open Notebook | button | `Mod+O` | file.openNotebook | P0 | wired | FLUENT19:ContextMenuNotebookArea/NotebookOpen |  |
| CM.LegacyFormat.SectionConvertTo2010 | Convert Section to 2010-2016 Format | Context/Legacy Format/Convert Section to 2010-2016 Format | button |  | section.convertLegacy | P3 | missing | FLUENT19:ContextMenuConvertVersion/SectionConvertTo2010 |  |
| CM.LegacyFormat.NotebookConvertTo2010 | Convert Notebook to 2010-2016 Format | Context/Legacy Format/Convert Notebook to 2010-2016 Format | button |  | notebook.convertLegacy | P3 | missing | FLUENT19:ContextMenuConvertVersion/NotebookConvertTo2010 |  |
| CM.ShreddedStorage.ConvertShreddedSection | Convert Shredded Section | Context/Shredded Storage/Convert Shredded Section | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuConvertShredded/ConvertShreddedSection |  |
| CM.ShreddedStorage.ConvertShreddedNotebook | Convert Shredded Notebook | Context/Shredded Storage/Convert Shredded Notebook | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuConvertShredded/ConvertShreddedNotebook |  |
| CM.LegacySection.SectionUpgrade | Upgrade Legacy Section | Context/Legacy Section/Upgrade Legacy Section | button |  | section.convertLegacy | P3 | missing | FLUENT19:ContextMenuLegacySection/SectionUpgrade |  |
| CM.LegacySection.UpgradeAllLegacySectionsInNotebook | Upgrade All Legacy Sections | Context/Legacy Section/Upgrade All Legacy Sections | button |  | notebook.convertLegacy | P3 | missing | FLUENT19:ContextMenuLegacySection/UpgradeAllLegacySectionsInNotebook |  |
| CM.AuthenticationFailure.AuthenticationFailureGallery | Sign In (sync failure) | Context/Authentication Failure/Sign In (sync failure) | gallery |  |  | P3 | out-of-scope | FLUENT19:ContextMenuAuthenticationFailure/AuthenticationFailureGallery |  |
| CM.MeetingFileInfoBar.DownloadMeetingFileIntoOnenote | Download Meeting File | Context/Meeting File Info Bar/Download Meeting File | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuDownloadMeetingFileInfoBar/DownloadMeetingFileIntoOnenote |  |
| CM.MeetingFileInfoBar.NotDownloadMeetingFileIntoOnenote | Don't Download Meeting File | Context/Meeting File Info Bar/Don't Download Meeting File | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuDownloadMeetingFileInfoBar/NotDownloadMeetingFileIntoOnenote |  |
| CM.OutofSpace.HistoryDelete | Delete History (out of space) | Context/Out of Space/Delete History (out of space) | button |  | history.deleteVersionsNotebook | P3 | missing | FLUENT19:ContextMenuOutOfSpace/HistoryDelete |  |
| CM.Equation.Cut | Cut | Context/Equation/Cut | button | `Mod+X` | edit.cut | P0 | missing | FLUENT19:ContextMenuEquation/Cut |  |
| CM.Equation.CopyOneNote | Copy | Context/Equation/Copy | button | `Mod+C` | edit.copy | P0 | missing | FLUENT19:ContextMenuEquation/CopyOneNote |  |
| CM.Equation.PasteGalleryMini | Paste Options | Context/Equation/Paste Options | gallery |  | edit.pasteOptions | P0 | missing | FLUENT19:ContextMenuEquation/PasteGalleryMini |  |
| CM.Equation.EquationProfessional | Professional | Context/Equation/Professional | button |  | equation.professional | P2 | missing | FLUENT19:ContextMenuEquation/EquationProfessional |  |
| CM.Equation.EquationLinearFormat | Linear | Context/Equation/Linear | button |  | equation.linear | P2 | missing | FLUENT19:ContextMenuEquation/EquationLinearFormat |  |
| CM.Equation.EquationRemoveStructure | Remove Structure | Context/Equation/Remove Structure | button |  | equation.removeStructure | P3 | missing | FLUENT19:ContextMenuEquation/EquationRemoveStructure |  |
| CM.Equation.EquationIncreaseArgumentSize | Increase Argument Size | Context/Equation/Increase Argument Size | button |  | equation.increaseArgumentSize | P3 | missing | FLUENT19:ContextMenuEquation/EquationIncreaseArgumentSize |  |
| CM.Equation.EquationDecreaseArgumentSize | Decrease Argument Size | Context/Equation/Decrease Argument Size | button |  | equation.decreaseArgumentSize | P3 | missing | FLUENT19:ContextMenuEquation/EquationDecreaseArgumentSize |  |
| CM.Equation.EquationInsertArgumentBefore | Insert Argument Before | Context/Equation/Insert Argument Before | button |  | equation.insertArgumentBefore | P3 | missing | FLUENT19:ContextMenuEquation/EquationInsertArgumentBefore |  |
| CM.Equation.EquationInsertArgumentAfter | Insert Argument After | Context/Equation/Insert Argument After | button |  | equation.insertArgumentAfter | P3 | missing | FLUENT19:ContextMenuEquation/EquationInsertArgumentAfter |  |
| CM.Equation.EquationShowHideOpeningDelimiter | Show/Hide Opening Delimiter | Context/Equation/Show/Hide Opening Delimiter | button |  | equation.showHideOpeningDelimiter | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHideOpeningDelimiter |  |
| CM.Equation.EquationShowHideClosingDelimiter | Show/Hide Closing Delimiter | Context/Equation/Show/Hide Closing Delimiter | button |  | equation.showHideClosingDelimiter | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHideClosingDelimiter |  |
| CM.Equation.EquationStretchDelimiters | Stretch Delimiters | Context/Equation/Stretch Delimiters | toggle |  | equation.stretchDelimiters | P3 | missing | FLUENT19:ContextMenuEquation/EquationStretchDelimiters |  |
| CM.Equation.EquationMatchDelimiters | Match Delimiters | Context/Equation/Match Delimiters | button |  | equation.matchDelimiters | P3 | missing | FLUENT19:ContextMenuEquation/EquationMatchDelimiters |  |
| CM.Equation.EquationDeleteArgument | Delete Argument | Context/Equation/Delete Argument | button |  | equation.deleteArgument | P3 | missing | FLUENT19:ContextMenuEquation/EquationDeleteArgument |  |
| CM.Equation.EquationScriptLocation | Script Location | Context/Equation/Script Location | button |  | equation.scriptLocation | P3 | missing | FLUENT19:ContextMenuEquation/EquationScriptLocation |  |
| CM.Equation.EquationLimitLocation | Limit Location | Context/Equation/Limit Location | button |  | equation.limitLocation | P3 | missing | FLUENT19:ContextMenuEquation/EquationLimitLocation |  |
| CM.Equation.EquationShowHidePlaceholders | Show/Hide Placeholders | Context/Equation/Show/Hide Placeholders | button |  | equation.showHidePlaceholders | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHidePlaceholders |  |
| CM.Equation.EquationChangeLimitLocation | Change Limit Location | Context/Equation/Change Limit Location | button |  | equation.changeLimitLocation | P3 | missing | FLUENT19:ContextMenuEquation/EquationChangeLimitLocation |  |
| CM.Equation.EquationShowHideLowerLimit | Show/Hide Lower Limit | Context/Equation/Show/Hide Lower Limit | button |  | equation.showHideLowerLimit | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHideLowerLimit |  |
| CM.Equation.EquationShowHideUpperLimit | Show/Hide Upper Limit | Context/Equation/Show/Hide Upper Limit | button |  | equation.showHideUpperLimit | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHideUpperLimit |  |
| CM.Equation.EquationStretchNaryOperator | Stretch Nary Operator | Context/Equation/Stretch Nary Operator | toggle |  | equation.stretchNaryOperator | P3 | missing | FLUENT19:ContextMenuEquation/EquationStretchNaryOperator |  |
| CM.Equation.EquationBarLocation | Bar Location | Context/Equation/Bar Location | button |  | equation.barLocation | P3 | missing | FLUENT19:ContextMenuEquation/EquationBarLocation |  |
| CM.Equation.EquationShowHideRadicalDegree | Show/Hide Radical Degree | Context/Equation/Show/Hide Radical Degree | button |  | equation.showHideRadicalDegree | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHideRadicalDegree |  |
| CM.Equation.EquationGroupingCharacterLocation | Grouping Character Location | Context/Equation/Grouping Character Location | button |  | equation.groupingCharacterLocation | P3 | missing | FLUENT19:ContextMenuEquation/EquationGroupingCharacterLocation |  |
| CM.Equation.EquationRemoveSubscript | Remove Subscript | Context/Equation/Remove Subscript | button |  | equation.removeSubscript | P3 | missing | FLUENT19:ContextMenuEquation/EquationRemoveSubscript |  |
| CM.Equation.EquationRemoveSuperscript | Remove Superscript | Context/Equation/Remove Superscript | button |  | equation.removeSuperscript | P3 | missing | FLUENT19:ContextMenuEquation/EquationRemoveSuperscript |  |
| CM.Equation.EquationScriptAlignment | Script Alignment | Context/Equation/Script Alignment | button |  | equation.scriptAlignment | P3 | missing | FLUENT19:ContextMenuEquation/EquationScriptAlignment |  |
| CM.Equation.EquationStackedFraction | Stacked Fraction | Context/Equation/Stacked Fraction | button |  | equation.stackedFraction | P3 | missing | FLUENT19:ContextMenuEquation/EquationStackedFraction |  |
| CM.Equation.EquationSkewedFraction | Skewed Fraction | Context/Equation/Skewed Fraction | button |  | equation.skewedFraction | P3 | missing | FLUENT19:ContextMenuEquation/EquationSkewedFraction |  |
| CM.Equation.EquationLinearFraction | Linear Fraction | Context/Equation/Linear Fraction | button |  | equation.linearFraction | P3 | missing | FLUENT19:ContextMenuEquation/EquationLinearFraction |  |
| CM.Equation.EquationNoBarFraction | No Bar Fraction | Context/Equation/No Bar Fraction | button |  | equation.noBarFraction | P3 | missing | FLUENT19:ContextMenuEquation/EquationNoBarFraction |  |
| CM.Equation.EquationBorderPropertiesMenu | Border Properties Menu | Context/Equation/Border Properties Menu | menu |  |  | P3 | missing | FLUENT19:ContextMenuEquation/EquationBorderPropertiesMenu |  |
| CM.Equation.EquationShowHideBorderTop | Show/Hide Border Top | Context/Equation/Border Properties Menu/Show/Hide Border Top | button |  | equation.showHideBorderTop | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHideBorderTop |  |
| CM.Equation.EquationShowHideBorderLeft | Show/Hide Border Left | Context/Equation/Border Properties Menu/Show/Hide Border Left | button |  | equation.showHideBorderLeft | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHideBorderLeft |  |
| CM.Equation.EquationShowHideBorderBottom | Show/Hide Border Bottom | Context/Equation/Border Properties Menu/Show/Hide Border Bottom | button |  | equation.showHideBorderBottom | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHideBorderBottom |  |
| CM.Equation.EquationShowHideBorderRight | Show/Hide Border Right | Context/Equation/Border Properties Menu/Show/Hide Border Right | button |  | equation.showHideBorderRight | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHideBorderRight |  |
| CM.Equation.EquationShowHideBorderHorizontalStrike | Show/Hide Border Horizontal Strike | Context/Equation/Border Properties Menu/Show/Hide Border Horizontal Strike | button |  | equation.showHideBorderHorizontalStrike | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHideBorderHorizontalStrike |  |
| CM.Equation.EquationShowHideBorderVerticalStrike | Show/Hide Border Vertical Strike | Context/Equation/Border Properties Menu/Show/Hide Border Vertical Strike | button |  | equation.showHideBorderVerticalStrike | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHideBorderVerticalStrike |  |
| CM.Equation.EquationShowHideBorderTopLeftStrike | Show/Hide Border Top Left Strike | Context/Equation/Border Properties Menu/Show/Hide Border Top Left Strike | button |  | equation.showHideBorderTopLeftStrike | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHideBorderTopLeftStrike |  |
| CM.Equation.EquationShowHideBorderBottomLeftStrike | Show/Hide Border Bottom Left Strike | Context/Equation/Border Properties Menu/Show/Hide Border Bottom Left Strike | button |  | equation.showHideBorderBottomLeftStrike | P3 | missing | FLUENT19:ContextMenuEquation/EquationShowHideBorderBottomLeftStrike |  |
| CM.Equation.EquationVerticalAlignmentMenu | Vertical Alignment Menu | Context/Equation/Vertical Alignment Menu | menu |  |  | P3 | missing | FLUENT19:ContextMenuEquation/EquationVerticalAlignmentMenu |  |
| CM.Equation.EquationVerticalAlignmentTop | Vertical Alignment Top | Context/Equation/Vertical Alignment Menu/Vertical Alignment Top | button |  | equation.verticalAlignmentTop | P3 | missing | FLUENT19:ContextMenuEquation/EquationVerticalAlignmentTop |  |
| CM.Equation.EquationVerticalAlignmentCenter | Vertical Alignment Center | Context/Equation/Vertical Alignment Menu/Vertical Alignment Center | button |  | equation.verticalAlignmentCenter | P3 | missing | FLUENT19:ContextMenuEquation/EquationVerticalAlignmentCenter |  |
| CM.Equation.EquationVerticalAlignmentBottom | Vertical Alignment Bottom | Context/Equation/Vertical Alignment Menu/Vertical Alignment Bottom | button |  | equation.verticalAlignmentBottom | P3 | missing | FLUENT19:ContextMenuEquation/EquationVerticalAlignmentBottom |  |
| CM.Equation.EquationMatrixInsertMenu | Matrix Insert Menu | Context/Equation/Matrix Insert Menu | menu |  |  | P3 | missing | FLUENT19:ContextMenuEquation/EquationMatrixInsertMenu |  |
| CM.Equation.EquationMatrixInsertRowBefore | Matrix Insert Row Before | Context/Equation/Matrix Insert Menu/Matrix Insert Row Before | button |  | equation.matrixInsertRowBefore | P3 | missing | FLUENT19:ContextMenuEquation/EquationMatrixInsertRowBefore |  |
| CM.Equation.EquationMatrixInsertRowAfter | Matrix Insert Row After | Context/Equation/Matrix Insert Menu/Matrix Insert Row After | button |  | equation.matrixInsertRowAfter | P3 | missing | FLUENT19:ContextMenuEquation/EquationMatrixInsertRowAfter |  |
| CM.Equation.EquationMatrixInsertColumnBefore | Matrix Insert Column Before | Context/Equation/Matrix Insert Menu/Matrix Insert Column Before | button |  | equation.matrixInsertColumnBefore | P3 | missing | FLUENT19:ContextMenuEquation/EquationMatrixInsertColumnBefore |  |
| CM.Equation.EquationMatrixInsertColumnAfter | Matrix Insert Column After | Context/Equation/Matrix Insert Menu/Matrix Insert Column After | button |  | equation.matrixInsertColumnAfter | P3 | missing | FLUENT19:ContextMenuEquation/EquationMatrixInsertColumnAfter |  |
| CM.Equation.EquationMatrixDeleteMenu | Matrix Delete Menu | Context/Equation/Matrix Delete Menu | menu |  |  | P3 | missing | FLUENT19:ContextMenuEquation/EquationMatrixDeleteMenu |  |
| CM.Equation.EquationMatrixDeleteRow | Matrix Delete Row | Context/Equation/Matrix Delete Menu/Matrix Delete Row | button |  | equation.matrixDeleteRow | P3 | missing | FLUENT19:ContextMenuEquation/EquationMatrixDeleteRow |  |
| CM.Equation.EquationMatrixDeleteColumn | Matrix Delete Column | Context/Equation/Matrix Delete Menu/Matrix Delete Column | button |  | equation.matrixDeleteColumn | P3 | missing | FLUENT19:ContextMenuEquation/EquationMatrixDeleteColumn |  |
| CM.Equation.LastModifiedBy | Last Modified By (author and date) | Context/Equation/Last Modified By (author and date) | button |  | object.lastModified | P2 | missing | FLUENT19:ContextMenuEquation/LastModifiedBy |  |
| CM.PageIndicator.LinkedFileOpenGallery | Open Linked Files | Context/Page Indicator/Open Linked Files | gallery |  | linked.open | P3 | missing | FLUENT19:ContextMenuPageIndicator/LinkedFileOpenGallery |  |
| CM.PageIndicator.NoteLinkDeleteGallery | Delete Link(s) | Context/Page Indicator/Delete Link(s) | gallery |  | linked.delete | P3 | missing | FLUENT19:ContextMenuPageIndicator/NoteLinkDeleteGallery |  |
| CM.PageIndicator.NoteLinksDeleteAll | Delete All Links on This Page | Context/Page Indicator/Delete Link(s)/Delete All Links on This Page | button |  | linked.deleteAll | P3 | missing | FLUENT19:ContextMenuPageIndicator/NoteLinksDeleteAll |  |
| CM.PageIndicator.AutoLinkingStart | Start Taking Linked Notes | Context/Page Indicator/Start Taking Linked Notes | button |  | linked.start | P3 | missing | FLUENT19:ContextMenuPageIndicator/AutoLinkingStart |  |
| CM.PageIndicator.AutoLinkingStop | Stop Taking Linked Notes | Context/Page Indicator/Stop Taking Linked Notes | button |  | linked.stop | P3 | missing | FLUENT19:ContextMenuPageIndicator/AutoLinkingStop |  |
| CM.PageIndicator.LinkedNotesSettings | Linked Notes Options | Context/Page Indicator/Linked Notes Options | button |  | linked.options | P3 | missing | FLUENT19:ContextMenuPageIndicator/LinkedNotesSettings |  |
| CM.PageIndicator.RefreshMeetingDetails | Refresh Meeting Details | Context/Page Indicator/Refresh Meeting Details | button |  |  | P3 | out-of-scope | FLUENT19:ContextMenuPageIndicator/RefreshMeetingDetails |  |
| CM.PageIndicator.DownloadMeetingFileIntoOnenoteMenu | Download Meeting Files | Context/Page Indicator/Download Meeting Files | toggle |  |  | P3 | out-of-scope | FLUENT19:ContextMenuPageIndicator/DownloadMeetingFileIntoOnenoteMenu |  |
| CM.LinkedNotes.LinkedFileOpen | Open Linked File | Context/Linked Notes/Open Linked File | button |  | linked.open | P3 | missing | FLUENT19:ContextMenuLinkedNotes/LinkedFileOpen |  |
| CM.LinkedNotes.LinkedNotesTargetUrlCopy | Copy Link to Linked File | Context/Linked Notes/Copy Link to Linked File | button |  | linked.copyUrl | P3 | missing | FLUENT19:ContextMenuLinkedNotes/LinkedNotesTargetUrlCopy |  |
| CM.LinkedNotes.NoteLinkRepairLink | Repair Link | Context/Linked Notes/Repair Link | button |  | linked.repair | P3 | missing | FLUENT19:ContextMenuLinkedNotes/NoteLinkRepairLink |  |
| CM.LinkedNotes.NoteLinkReLink | Re-link | Context/Linked Notes/Re-link | button |  | linked.relink | P3 | missing | FLUENT19:ContextMenuLinkedNotes/NoteLinkReLink |  |
| CM.LinkedNotes.NoteLinkDelete | Delete Link | Context/Linked Notes/Delete Link | button |  | linked.delete | P3 | missing | FLUENT19:ContextMenuLinkedNotes/NoteLinkDelete |  |
| CM.LinkedNotes.LinkedNotesOptions | Linked Notes Options | Context/Linked Notes/Linked Notes Options | button |  | linked.options | P3 | missing | FLUENT19:ContextMenuLinkedNotes/LinkedNotesOptions |  |
| CM.SearchScope.SearchScopePage | This Page | Context/Search Scope/This Page | toggle |  | search.scopePage | P0 | wired | FLUENT19:ContextMenuSearchScope/SearchScopePage |  |
| CM.SearchScope.SearchScopeSection | This Section | Context/Search Scope/This Section | toggle |  | search.scopeSection | P0 | verified | FLUENT19:ContextMenuSearchScope/SearchScopeSection |  |
| CM.SearchScope.SearchScopeSectionGroup | This Section Group | Context/Search Scope/This Section Group | toggle |  | search.scopeSectionGroup | P0 | wired | FLUENT19:ContextMenuSearchScope/SearchScopeSectionGroup |  |
| CM.SearchScope.SearchScopeNotebook | This Notebook | Context/Search Scope/This Notebook | toggle |  | search.scopeNotebook | P0 | wired | FLUENT19:ContextMenuSearchScope/SearchScopeNotebook |  |
| CM.SearchScope.SearchScopeAllNotebooks | All Notebooks | Context/Search Scope/All Notebooks | toggle |  | search.scopeAll | P0 | wired | FLUENT19:ContextMenuSearchScope/SearchScopeAllNotebooks |  |
| CM.SearchScope.SearchScopeSetDefault | Set This Scope as Default | Context/Search Scope/Set This Scope as Default | button |  | search.setDefaultScope | P1 | wired | FLUENT19:ContextMenuSearchScope/SearchScopeSetDefault |  |

## Commands not in the ribbon

Commands available only through Customize Quick Access Toolbar ("Commands Not in the Ribbon").

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| QAT.NR.WebGoBack | Back (history list) | Quick Access Toolbar/Commands Not in the Ribbon/Back (history list) | gallery |  | nav.back | P2 | missing | FLUENT19:WebGoBack |  |
| QAT.NR.WebGoForward | Forward (history list) | Quick Access Toolbar/Commands Not in the Ribbon/Forward (history list) | gallery |  | nav.forward | P2 | missing | FLUENT19:WebGoForward |  |
| QAT.NR.NotebookSearch | Search Notebooks | Quick Access Toolbar/Commands Not in the Ribbon/Search Notebooks | button | `Mod+E` | search.all | P0 | wired | FLUENT19:NotebookSearch |  |
| QAT.NR.PageDeleteOneNote | Delete Page | Quick Access Toolbar/Commands Not in the Ribbon/Delete Page | button |  | page.delete | P0 | wired | FLUENT19:PageDeleteOneNote |  |
| QAT.NR.PreviousPageOneNote | Previous Page | Quick Access Toolbar/Commands Not in the Ribbon/Previous Page | button | `Mod+PageUp` | nav.prevPage | P0 | wired | FLUENT19:PreviousPageOneNote |  |
| QAT.NR.NextPageOneNote | Next Page | Quick Access Toolbar/Commands Not in the Ribbon/Next Page | button | `Mod+PageDown` | nav.nextPage | P0 | wired | FLUENT19:NextPageOneNote |  |
| QAT.NR.Undo | Undo (history list) | Quick Access Toolbar/Commands Not in the Ribbon/Undo (history list) | gallery |  | edit.undoTo | P1 | wired | FLUENT19:Undo |  |
| QAT.NR.Redo | Redo (history list) | Quick Access Toolbar/Commands Not in the Ribbon/Redo (history list) | gallery |  | edit.redoTo | P1 | wired | FLUENT19:Redo |  |
| QAT.NR.Help | Help | Quick Access Toolbar/Commands Not in the Ribbon/Help | button | `F1` | help.help | P1 | missing | FLUENT19:Help |  |
| QAT.NR.PasteSourceFormatting | Paste: Keep Source Formatting | Quick Access Toolbar/Commands Not in the Ribbon/Paste: Keep Source Formatting | button |  | edit.pasteKeepSource | P0 | missing | FLUENT19:PasteSourceFormatting |  |
| QAT.NR.PasteMergeFormatting | Paste: Merge Formatting | Quick Access Toolbar/Commands Not in the Ribbon/Paste: Merge Formatting | button |  | edit.pasteMerge | P1 | missing | FLUENT19:PasteMergeFormatting |  |
| QAT.NR.PasteTextOnly | Paste: Keep Text Only | Quick Access Toolbar/Commands Not in the Ribbon/Paste: Keep Text Only | button | `Mod+Shift+V` | edit.pasteTextOnly | P0 | missing | FLUENT19:PasteTextOnly |  |
| QAT.NR.PasteAsPicture | Paste: Picture | Quick Access Toolbar/Commands Not in the Ribbon/Paste: Picture | button |  | edit.pastePicture | P2 | wired | FLUENT19:PasteAsPicture | Same command as R.Home.Clipboard.PastePicture |
| QAT.NR.PasteInk | Paste Ink | Quick Access Toolbar/Commands Not in the Ribbon/Paste Ink | button |  | edit.pasteInk | P3 | missing | FLUENT19:PasteInk |  |
| QAT.NR.FavoritePen5 | Favorite Pen 5 | Quick Access Toolbar/Commands Not in the Ribbon/Favorite Pen 5 | toggle |  | draw.favoritePen5 | P2 | missing | FLUENT19:FavoritePen5 |  |
| QAT.NR.FavoritePen6 | Favorite Pen 6 | Quick Access Toolbar/Commands Not in the Ribbon/Favorite Pen 6 | toggle |  | draw.favoritePen6 | P2 | missing | FLUENT19:FavoritePen6 |  |
| QAT.NR.FavoritePen7 | Favorite Pen 7 | Quick Access Toolbar/Commands Not in the Ribbon/Favorite Pen 7 | toggle |  | draw.favoritePen7 | P2 | missing | FLUENT19:FavoritePen7 |  |
| QAT.NR.FavoritePen8 | Favorite Pen 8 | Quick Access Toolbar/Commands Not in the Ribbon/Favorite Pen 8 | toggle |  | draw.favoritePen8 | P2 | missing | FLUENT19:FavoritePen8 |  |
| QAT.NR.FavoriteHighlighter2 | Favorite Highlighter 2 | Quick Access Toolbar/Commands Not in the Ribbon/Favorite Highlighter 2 | toggle |  | draw.favoriteHighlighter2 | P2 | missing | FLUENT19:FavoriteHighlighter2 |  |
| QAT.NR.FavoriteHighlighter3 | Favorite Highlighter 3 | Quick Access Toolbar/Commands Not in the Ribbon/Favorite Highlighter 3 | toggle |  | draw.favoriteHighlighter3 | P2 | missing | FLUENT19:FavoriteHighlighter3 |  |
| QAT.NR.FavoriteHighlighter4 | Favorite Highlighter 4 | Quick Access Toolbar/Commands Not in the Ribbon/Favorite Highlighter 4 | toggle |  | draw.favoriteHighlighter4 | P2 | missing | FLUENT19:FavoriteHighlighter4 |  |
| QAT.NR.PlaybackTipPlay | Play (playback tip) | Quick Access Toolbar/Commands Not in the Ribbon/Play (playback tip) | button |  | media.play | P3 | missing | FLUENT19:PlaybackTipPlay |  |
| QAT.NR.About | About NoteCraft | Quick Access Toolbar/Commands Not in the Ribbon/About NoteCraft | button |  | help.about | P1 | missing | FLUENT19:About |  |
| QAT.NR.MarkPageAsUnread | Mark as Unread | Quick Access Toolbar/Commands Not in the Ribbon/Mark as Unread | button | `Mod+Q` | history.markUnread | P2 | missing | FLUENT19:MarkPageAsUnread |  |
| QAT.NR.MoveOrCopyPage | Move or Copy Page | Quick Access Toolbar/Commands Not in the Ribbon/Move or Copy Page | button | `Mod+Alt+M` | page.moveOrCopy | P0 | verified | FLUENT19:MoveOrCopyPage |  |
| QAT.NR.NewPageAfterCurrentGroup | New Page After Current Group | Quick Access Toolbar/Commands Not in the Ribbon/New Page After Current Group | button |  | page.newAfterGroup | P1 | wired | FLUENT19:NewPageAfterCurrentGroup |  |
| QAT.NR.FontSizeIncrease1Point | Increase Font Size | Quick Access Toolbar/Commands Not in the Ribbon/Increase Font Size | button | `Mod+Shift+>` | format.growFont | P0 | wired | FLUENT19:FontSizeIncrease1Point |  |
| QAT.NR.FontSizeDecrease1Point | Decrease Font Size | Quick Access Toolbar/Commands Not in the Ribbon/Decrease Font Size | button | `Mod+Shift+<` | format.shrinkFont | P0 | wired | FLUENT19:FontSizeDecrease1Point |  |
| QAT.NR.MinimizeRibbon | Collapse the Ribbon | Quick Access Toolbar/Commands Not in the Ribbon/Collapse the Ribbon | toggle | `Mod+F1` | ui.collapseRibbon | P0 | wired | FLUENT19:MinimizeRibbon |  |
| QAT.NR.HelpKeyboardShortcuts | Keyboard Shortcuts | Quick Access Toolbar/Commands Not in the Ribbon/Keyboard Shortcuts | button |  | help.keyboardShortcuts | P2 | missing | FLUENT19:HelpKeyboardShortcuts |  |
| QAT.NR.OutlookTaskToday | Outlook Task: Today | Quick Access Toolbar/Commands Not in the Ribbon/Outlook Task: Today | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskToday |  |
| QAT.NR.OutlookTaskTomorrow | Outlook Task: Tomorrow | Quick Access Toolbar/Commands Not in the Ribbon/Outlook Task: Tomorrow | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskTomorrow |  |
| QAT.NR.OutlookTaskThisWeek | Outlook Task: This Week | Quick Access Toolbar/Commands Not in the Ribbon/Outlook Task: This Week | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskThisWeek |  |
| QAT.NR.OutlookTaskNextWeek | Outlook Task: Next Week | Quick Access Toolbar/Commands Not in the Ribbon/Outlook Task: Next Week | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskNextWeek |  |
| QAT.NR.OutlookTaskLater | Outlook Task: No Date | Quick Access Toolbar/Commands Not in the Ribbon/Outlook Task: No Date | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskLater |  |
| QAT.NR.OutlookTaskCustomDate | Outlook Task: Custom | Quick Access Toolbar/Commands Not in the Ribbon/Outlook Task: Custom | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskCustomDate |  |
| QAT.NR.OpenBackup | Open Backups | Quick Access Toolbar/Commands Not in the Ribbon/Open Backups | button |  | file.openBackups | P1 | missing | FLUENT19:OpenBackup |  |
| QAT.NR.JotOpenFile | Open File (section) | Quick Access Toolbar/Commands Not in the Ribbon/Open File (section) | button | `Mod+Alt+Shift+O` | file.openSection | P1 | missing | FLUENT19:JotOpenFile |  |
| QAT.NR.NewPageInSeries | New Page in Series | Quick Access Toolbar/Commands Not in the Ribbon/New Page in Series | button |  | page.newInSeries | P2 | missing | FLUENT19:NewPageInSeries |  |
| QAT.NR.AttachNotesToMeeting | Attach Notes to Meeting | Quick Access Toolbar/Commands Not in the Ribbon/Attach Notes to Meeting | button |  |  | P3 | out-of-scope | FLUENT19:AttachNotesToMeeting |  |
| QAT.NR.PenMode | Pen Mode | Quick Access Toolbar/Commands Not in the Ribbon/Pen Mode | toggle |  | draw.penMode | P2 | missing | FLUENT19:PenMode |  |
| QAT.NR.HighlighterMode | Highlighter Mode | Quick Access Toolbar/Commands Not in the Ribbon/Highlighter Mode | toggle |  | draw.highlighterMode | P2 | missing | FLUENT19:HighlighterMode |  |
| QAT.NR.HideRibbon | Hide Ribbon | Quick Access Toolbar/Commands Not in the Ribbon/Hide Ribbon | toggle |  | ui.hideRibbon | P2 | missing | FLUENT19:HideRibbon |  |
| QAT.NR.OfficeFeedbackBackstage | Feedback | Quick Access Toolbar/Commands Not in the Ribbon/Feedback | button |  | help.feedback | P3 | missing | FLUENT19:OfficeFeedbackBackstage |  |
| QAT.NR.NewPageFromTemplate | New Page from Template | Quick Access Toolbar/Commands Not in the Ribbon/New Page from Template | button |  | page.newFromTemplate | P2 | missing | FLUENT19:NewPageFromTemplate |  |
| QAT.NR.PasteAsInkOneNote | Paste as Ink | Quick Access Toolbar/Commands Not in the Ribbon/Paste as Ink | button |  | edit.pasteInk | P3 | missing | FLUENT19:PasteAsInkOneNote |  |
| QAT.NR.ZoomCurrent75 | 75% | Quick Access Toolbar/Commands Not in the Ribbon/75% | button |  | view.zoom75 | P3 | wired | FLUENT19:ZoomCurrent75 |  |
| QAT.NR.Spelling | Spelling | Quick Access Toolbar/Commands Not in the Ribbon/Spelling | button | `F7` | review.spelling | P1 | missing | FLUENT19:Spelling |  |
| QAT.NR.OutlookTaskRemoveCtxItem | Delete Outlook Task | Quick Access Toolbar/Commands Not in the Ribbon/Delete Outlook Task | button |  |  | P3 | out-of-scope | FLUENT19:OutlookTaskRemoveCtxItem |  |
| QAT.NR.NotebookRename | Rename Notebook | Quick Access Toolbar/Commands Not in the Ribbon/Rename Notebook | button |  | notebook.rename | P1 | wired | FLUENT19:NotebookRename |  |
| QAT.NR.NotebookSaveAs | Save Notebook As | Quick Access Toolbar/Commands Not in the Ribbon/Save Notebook As | button |  | file.exportNotebook | P1 | missing | FLUENT19:NotebookSaveAs |  |
| QAT.NR.SendPdf | Send as PDF | Quick Access Toolbar/Commands Not in the Ribbon/Send as PDF | button |  | share.sendPdf | P2 | missing | FLUENT19:SendPdf |  |
| QAT.NR.SendNotesToBlog | Send to Blog | Quick Access Toolbar/Commands Not in the Ribbon/Send to Blog | button |  |  | P3 | out-of-scope | FLUENT19:SendNotesToBlog |  |
| QAT.NR.ApplicationOptionsDialog | Options | Quick Access Toolbar/Commands Not in the Ribbon/Options | button |  | file.options | P0 | missing | FLUENT19:ApplicationOptionsDialog |  |
| QAT.NR.SendNotesAsAttachment | Send as Attachment | Quick Access Toolbar/Commands Not in the Ribbon/Send as Attachment | button |  | share.sendAttachment | P2 | missing | FLUENT19:SendNotesAsAttachment |  |
| QAT.NR.FileExit | Exit | Quick Access Toolbar/Commands Not in the Ribbon/Exit | button |  | file.exit | P0 | missing | FLUENT19:FileExit |  |
| QAT.NR.OfficeFeedback | Feedback | Quick Access Toolbar/Commands Not in the Ribbon/Feedback | button |  | help.feedback | P3 | missing | FLUENT19:OfficeFeedback |  |
| QAT.NR.ShapeColorAndThickness | Shape Color & Thickness | Quick Access Toolbar/Commands Not in the Ribbon/Shape Color & Thickness | toggle |  | draw.shapeColorThickness | P2 | missing | FLUENT19:ShapeColorAndThickness |  |
| QAT.NR.NewSharedNotebookCreate | New Shared Notebook | Quick Access Toolbar/Commands Not in the Ribbon/New Shared Notebook | button |  |  | P3 | out-of-scope | FLUENT19:NewSharedNotebookCreate |  |

## Docked window ribbon

The compact ribbon of a docked OneNote window (one row per group).

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| DOCK.Home.GroupClipboard | Docked window ribbon: Home › Clipboard | Docked/Home/Clipboard | layout |  |  | P3 | missing | FLUENT19:TabHomeNarrow/GroupClipboard |  |
| DOCK.Home.GroupBasicTextSmall | Docked window ribbon: Home › Basic Text Small | Docked/Home/Basic Text Small | layout |  |  | P3 | missing | FLUENT19:TabHomeNarrow/GroupBasicTextSmall |  |
| DOCK.Home.GroupTaggingSmall | Docked window ribbon: Home › Tagging Small | Docked/Home/Tagging Small | layout |  |  | P3 | missing | FLUENT19:TabHomeNarrow/GroupTaggingSmall |  |
| DOCK.Insert.GroupInsertTables | Docked window ribbon: Insert › Tables | Docked/Insert/Tables | layout |  |  | P3 | missing | FLUENT19:TabInsertNarrow/GroupInsertTables |  |
| DOCK.Insert.GroupInsertFiles | Docked window ribbon: Insert › Files | Docked/Insert/Files | layout |  |  | P3 | missing | FLUENT19:TabInsertNarrow/GroupInsertFiles |  |
| DOCK.Insert.GroupImages | Docked window ribbon: Insert › Images | Docked/Insert/Images | layout |  |  | P3 | missing | FLUENT19:TabInsertNarrow/GroupImages |  |
| DOCK.Insert.GroupInsertLinks | Docked window ribbon: Insert › Links | Docked/Insert/Links | layout |  |  | P3 | missing | FLUENT19:TabInsertNarrow/GroupInsertLinks |  |
| DOCK.Insert.GroupOutlook | Docked window ribbon: Insert › Meetings | Docked/Insert/Meetings | layout |  |  | P3 | missing | FLUENT19:TabInsertNarrow/GroupOutlook |  |
| DOCK.Insert.GroupRecordAndLink | Docked window ribbon: Insert › Recording | Docked/Insert/Recording | layout |  |  | P3 | missing | FLUENT19:TabInsertNarrow/GroupRecordAndLink |  |
| DOCK.Draw.GroupWrite | Docked window ribbon: Draw › Tools | Docked/Draw/Tools | layout |  |  | P3 | missing | FLUENT19:TabDrawNarrow/GroupWrite |  |
| DOCK.Draw.GroupWriteOnTouch | Docked window ribbon: Draw › Tools (touch) | Docked/Draw/Tools (touch) | layout |  |  | P3 | missing | FLUENT19:TabDrawNarrow/GroupWriteOnTouch |  |
| DOCK.Draw.GroupInkEditNarrow | Docked window ribbon: Draw › Ink Edit Narrow | Docked/Draw/Ink Edit Narrow | layout |  |  | P3 | missing | FLUENT19:TabDrawNarrow/GroupInkEditNarrow |  |
| DOCK.View.GroupView | Docked window ribbon: View › Views | Docked/View/Views | layout |  |  | P3 | missing | FLUENT19:TabViewNarrow/GroupView |  |
| DOCK.View.GroupViewDocked | Docked window ribbon: View › View Docked | Docked/View/View Docked | layout |  |  | P3 | missing | FLUENT19:TabViewNarrow/GroupViewDocked |  |
| DOCK.View.GroupViewFullPage | Docked window ribbon: View › View Full Page | Docked/View/View Full Page | layout |  |  | P3 | missing | FLUENT19:TabViewNarrow/GroupViewFullPage |  |
| DOCK.View.GroupPageSetupNarrow | Docked window ribbon: View › Page Setup Narrow | Docked/View/Page Setup Narrow | layout |  |  | P3 | missing | FLUENT19:TabViewNarrow/GroupPageSetupNarrow |  |
| DOCK.View.GroupZoomDocked | Docked window ribbon: View › Zoom Docked | Docked/View/Zoom Docked | layout |  |  | P3 | missing | FLUENT19:TabViewNarrow/GroupZoomDocked |  |
| DOCK.View.GroupWindowNarrow | Docked window ribbon: View › Window Narrow | Docked/View/Window Narrow | layout |  |  | P3 | missing | FLUENT19:TabViewNarrow/GroupWindowNarrow |  |
| DOCK.Pages.GroupNavigate | Docked window ribbon: Pages › Navigate | Docked/Pages/Navigate | layout |  |  | P3 | missing | FLUENT19:TabPages/GroupNavigate |  |
| DOCK.Pages.GroupInsertPages | Docked window ribbon: Pages › Insert Pages | Docked/Pages/Insert Pages | layout |  |  | P3 | missing | FLUENT19:TabPages/GroupInsertPages |  |

## Keyboard shortcuts

The complete "OneNote for Windows" table of Microsoft's keyboard-shortcut article (KBD365), deduplicated; `Mod` is Ctrl (⌘ on macOS). Rows without a Shortcut are context keys or keytip sequences (see Notes).

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| KB.General.NewWindow | Open a new window | Keyboard/General/Open a new window | shortcut | `Mod+M` | window.new | P1 | missing | KBD365 |  |
| KB.General.QuickNote | Create a Quick Note | Keyboard/General/Create a Quick Note | shortcut | `Mod+Shift+M` | window.quickNote | P1 | missing | KBD365 | global Win+Alt+N is a desktop-wide hotkey; registered by the desktop app where the platform allows |
| KB.General.Dock | Dock the window | Keyboard/General/Dock the window | shortcut | `Mod+Alt+D` | view.dock | P2 | missing | KBD365 |  |
| KB.General.Undo | Undo the previous action | Keyboard/General/Undo the previous action | shortcut | `Mod+Z` | edit.undo | P0 | wired | KBD365 |  |
| KB.General.Redo | Redo the previous action | Keyboard/General/Redo the previous action | shortcut | `Mod+Y` | edit.redo | P0 | wired | KBD365 |  |
| KB.General.SelectAll | Select all items on the current page (repeat to expand) | Keyboard/General/Select all items on the current page (repeat to expand) | shortcut | `Mod+A` | edit.selectAll | P0 | wired | KBD365 | repeat expands: paragraph → container → page |
| KB.General.Cut | Cut | Keyboard/General/Cut | shortcut | `Mod+X` | edit.cut | P0 | wired | KBD365 |  |
| KB.General.Copy | Copy | Keyboard/General/Copy | shortcut | `Mod+C` | edit.copy | P0 | wired | KBD365 |  |
| KB.General.Paste | Paste | Keyboard/General/Paste | shortcut | `Mod+V` | edit.paste | P0 | wired | KBD365 |  |
| KB.General.PasteText | Paste text only | Keyboard/General/Paste text only | shortcut | `Mod+Shift+V` | edit.pasteTextOnly | P0 | wired | KBD365 |  |
| KB.General.LineStart | Move to the beginning of the line | Keyboard/General/Move to the beginning of the line | shortcut | `Home` | caret.lineStart | P0 | verified | KBD365 |  |
| KB.General.LineEnd | Move to the end of the line | Keyboard/General/Move to the end of the line | shortcut | `End` | caret.lineEnd | P0 | wired | KBD365 |  |
| KB.General.WordLeft | Move one word to the left | Keyboard/General/Move one word to the left | shortcut | `Mod+Left` | caret.wordLeft | P0 | verified | KBD365 |  |
| KB.General.WordRight | Move one word to the right | Keyboard/General/Move one word to the right | shortcut | `Mod+Right` | caret.wordRight | P0 | verified | KBD365 |  |
| KB.General.Backspace | Delete one character to the left | Keyboard/General/Delete one character to the left | shortcut | `Backspace` | text.backspace | P0 | wired | KBD365 |  |
| KB.General.DeleteChar | Delete one character to the right | Keyboard/General/Delete one character to the right | shortcut | `Delete` | text.deleteForward | P0 | verified | KBD365 |  |
| KB.General.DeleteWordLeft | Delete one word to the left | Keyboard/General/Delete one word to the left | shortcut | `Mod+Backspace` | text.deleteWordLeft | P0 | verified | KBD365 |  |
| KB.General.DeleteWordRight | Delete one word to the right | Keyboard/General/Delete one word to the right | shortcut | `Mod+Delete` | text.deleteWordRight | P0 | verified | KBD365 |  |
| KB.General.LineBreak | Insert a line break without starting a new paragraph | Keyboard/General/Insert a line break without starting a new paragraph | shortcut | `Shift+Enter` | text.lineBreak | P0 | verified | KBD365 |  |
| KB.General.Spelling | Check spelling | Keyboard/General/Check spelling | shortcut | `F7` | review.spelling | P1 | missing | KBD365 |  |
| KB.General.Thesaurus | Open the thesaurus for the selected word | Keyboard/General/Open the thesaurus for the selected word | shortcut | `Shift+F7` | review.thesaurus | P2 | missing | KBD365 |  |
| KB.General.ContextMenu | Display the context menu for the focused object | Keyboard/General/Display the context menu for the focused object | shortcut | `Shift+F10` | ui.contextMenu | P1 | missing | KBD365 | also the Menu key |
| KB.General.InfoBarAction | Perform the action suggested on the Information Bar | Keyboard/General/Perform the action suggested on the Information Bar | shortcut | `Mod+Shift+W` | ui.infoBarAction | P3 | missing | KBD365 |  |
| KB.General.PlayAudio | Play the selected audio recording | Keyboard/General/Play the selected audio recording | shortcut | `Mod+Alt+P` | media.play | P2 | missing | KBD365 |  |
| KB.General.StopAudio | Stop audio recording playback | Keyboard/General/Stop audio recording playback | shortcut | `Mod+Alt+S` | media.stop | P2 | missing | KBD365 |  |
| KB.General.Rewind10 | Skip backward 10 seconds | Keyboard/General/Skip backward 10 seconds | shortcut | `Mod+Alt+Y` | media.rewind | P2 | missing | KBD365 |  |
| KB.General.Forward10 | Skip forward 10 seconds | Keyboard/General/Skip forward 10 seconds | shortcut | `Mod+Alt+U` | media.forward | P2 | missing | KBD365 |  |
| KB.Format.Highlight | Highlight the selected text | Keyboard/Format notes/Highlight the selected text | shortcut | `Mod+Alt+H` | format.highlight | P0 | wired | KBD365 | the Windows 10 table also lists Ctrl+Shift+H |
| KB.Format.Link | Insert a hyperlink | Keyboard/Format notes/Insert a hyperlink | shortcut | `Mod+K` | insert.link | P0 | wired | KBD365 |  |
| KB.Format.CopyFormat | Copy the formatting of the selected text (Format Painter) | Keyboard/Format notes/Copy the formatting of the selected text (Format Painter) | shortcut | `Mod+Alt+C` | format.copyFormat | P1 | verified | KBD365 | older documentation: Ctrl+Shift+C |
| KB.Format.PasteFormat | Paste the formatting to the selected text (Format Painter) | Keyboard/Format notes/Paste the formatting to the selected text (Format Painter) | shortcut | `Mod+Alt+V` | format.pasteFormat | P1 | verified | KBD365 | older documentation: Ctrl+Shift+V (now Paste text only) |
| KB.Format.OpenLink | Open a hyperlink (Enter on the link text) | Keyboard/Format notes/Open a hyperlink (Enter on the link text) | key-behaviour |  | link.open | P0 | wired | KBD365 | context key: Enter while the caret is on a link |
| KB.Format.Bold | Bold | Keyboard/Format notes/Bold | shortcut | `Mod+B` | format.bold | P0 | verified | KBD365 |  |
| KB.Format.Italic | Italic | Keyboard/Format notes/Italic | shortcut | `Mod+I` | format.italic | P0 | wired | KBD365 |  |
| KB.Format.Underline | Underline | Keyboard/Format notes/Underline | shortcut | `Mod+U` | format.underline | P0 | wired | KBD365 |  |
| KB.Format.Strikethrough | Strikethrough | Keyboard/Format notes/Strikethrough | shortcut | `Alt+-` | format.strikethrough | P0 | wired | KBD365 | the Windows 10 table lists Ctrl+Hyphen |
| KB.Format.Superscript | Superscript | Keyboard/Format notes/Superscript | shortcut | `Mod+Shift+Plus` | format.superscript | P1 | wired | KBD365 | "Ctrl+Shift+Plus(+)"; the Windows 10 table: Ctrl+Shift+= (same keys on US layouts) |
| KB.Format.Subscript | Subscript | Keyboard/Format notes/Subscript | shortcut | `Mod+=` | format.subscript | P1 | wired | KBD365 | unverified: the M365 table says Ctrl+Shift+Minus, which collides with KB.Select.Subordinates in the same table; Ctrl+= is the Windows 10 table's key |
| KB.Format.Bullets | Bulleted list | Keyboard/Format notes/Bulleted list | shortcut | `Mod+.` | para.bullets | P0 | wired | KBD365 |  |
| KB.Format.Numbering | Numbered list | Keyboard/Format notes/Numbered list | shortcut | `Mod+/` | para.numbering | P0 | wired | KBD365 |  |
| KB.Format.Heading1 | Apply Heading 1 | Keyboard/Format notes/Apply Heading 1 | shortcut | `Mod+Alt+1` | style.heading1 | P0 | wired | KBD365 |  |
| KB.Format.Heading2 | Apply Heading 2 | Keyboard/Format notes/Apply Heading 2 | shortcut | `Mod+Alt+2` | style.heading2 | P0 | wired | KBD365 |  |
| KB.Format.Heading3 | Apply Heading 3 | Keyboard/Format notes/Apply Heading 3 | shortcut | `Mod+Alt+3` | style.heading3 | P0 | wired | KBD365 |  |
| KB.Format.Heading4 | Apply Heading 4 | Keyboard/Format notes/Apply Heading 4 | shortcut | `Mod+Alt+4` | style.heading4 | P1 | wired | KBD365 |  |
| KB.Format.Heading5 | Apply Heading 5 | Keyboard/Format notes/Apply Heading 5 | shortcut | `Mod+Alt+5` | style.heading5 | P1 | wired | KBD365 |  |
| KB.Format.Heading6 | Apply Heading 6 | Keyboard/Format notes/Apply Heading 6 | shortcut | `Mod+Alt+6` | style.heading6 | P1 | wired | KBD365 |  |
| KB.Format.ClearFormatting | Clear all formatting (apply Normal style) | Keyboard/Format notes/Clear all formatting (apply Normal style) | shortcut | `Mod+Shift+N` | format.clear | P0 | wired | KBD365 |  |
| KB.Format.Indent | Increase the paragraph indent | Keyboard/Format notes/Increase the paragraph indent | shortcut | `Alt+Shift+Right` | para.indent | P0 | wired | KBD365 | Tab at the beginning of a line too |
| KB.Format.Outdent | Decrease the paragraph indent | Keyboard/Format notes/Decrease the paragraph indent | shortcut | `Alt+Shift+Left` | para.outdent | P0 | wired | KBD365 | Shift+Tab at the beginning of a line too |
| KB.Format.AlignLeft | Align left | Keyboard/Format notes/Align left | shortcut | `Mod+L` | para.alignLeft | P0 | wired | KBD365 |  |
| KB.Format.AlignRight | Align right | Keyboard/Format notes/Align right | shortcut | `Mod+R` | para.alignRight | P0 | wired | KBD365 |  |
| KB.Format.GrowFont | Increase the font size | Keyboard/Format notes/Increase the font size | shortcut | `Mod+Shift+>` | format.growFont | P0 | wired | KBD365 |  |
| KB.Format.ShrinkFont | Decrease the font size | Keyboard/Format notes/Decrease the font size | shortcut | `Mod+Shift+<` | format.shrinkFont | P0 | wired | KBD365 |  |
| KB.Format.RuleLines | Show or hide rule lines on the current page | Keyboard/Format notes/Show or hide rule lines on the current page | shortcut | `Mod+Shift+R` | view.ruleLines | P1 | verified | KBD365 |  |
| KB.Insert.FileKeytip | Insert a document or file (Alt+N, F) | Keyboard/Insert items/Insert a document or file (Alt+N, F) | key-behaviour |  | insert.file | P2 | missing | KBD365 | keytip sequence Alt+N, F |
| KB.Insert.PrintoutKeytip | Insert a file as a printout (Alt+N, O) | Keyboard/Insert items/Insert a file as a printout (Alt+N, O) | key-behaviour |  | insert.printout | P2 | missing | KBD365 | keytip sequence Alt+N, O |
| KB.Insert.TogglePrintouts | Show or hide document printouts (high contrast) | Keyboard/Insert items/Show or hide document printouts (high contrast) | shortcut | `Alt+Shift+P` | view.togglePrintouts | P3 | missing | KBD365 |  |
| KB.Insert.PictureKeytip | Insert a picture from a file (Alt+N, P) | Keyboard/Insert items/Insert a picture from a file (Alt+N, P) | key-behaviour |  | insert.picture | P2 | missing | KBD365 | keytip sequence Alt+N, P |
| KB.Insert.StickerKeytip | Insert a sticker (Alt+N, S) | Keyboard/Insert items/Insert a sticker (Alt+N, S) | key-behaviour |  | insert.sticker | P3 | missing | KBD365 | keytip sequence Alt+N, S |
| KB.Insert.ScreenClipping | Insert a screen clipping (platform screenshot, then paste) | Keyboard/Insert items/Insert a screen clipping (platform screenshot, then paste) | key-behaviour |  | insert.screenClipping | P2 | missing | KBD365 | Win+Shift+S is the Windows snipping hotkey; on Linux the portal screenshot API |
| KB.Insert.Date | Insert the current date | Keyboard/Insert items/Insert the current date | shortcut | `Alt+Shift+D` | insert.date | P1 | wired | KBD365 |  |
| KB.Insert.DateTime | Insert the current date and time | Keyboard/Insert items/Insert the current date and time | shortcut | `Alt+Shift+F` | insert.dateTime | P1 | wired | KBD365 |  |
| KB.Insert.Time | Insert the current time | Keyboard/Insert items/Insert the current time | shortcut | `Alt+Shift+T` | insert.time | P1 | wired | KBD365 |  |
| KB.Insert.Equation | Start a math equation or convert the selection | Keyboard/Insert items/Start a math equation or convert the selection | shortcut | `Alt+=` | insert.equation | P2 | missing | KBD365 |  |
| KB.Table.CreateWithTab | Create a table: Tab after typing text on a new line | Keyboard/Tables/Create a table: Tab after typing text on a new line | key-behaviour |  |  | P1 | wired | KBD365 | Decision D6; see BEH.Table.TabToCreate |
| KB.Table.NewColumnTab | Create another column in a single-row table: Tab | Keyboard/Tables/Create another column in a single-row table: Tab | key-behaviour |  |  | P1 | verified | KBD365 | behaviour of Tab in a table |
| KB.Table.NewRowEnter | Create another row at the end cell: Enter (again to finish) | Keyboard/Tables/Create another row at the end cell: Enter (again to finish) | key-behaviour |  | table.enter | P1 | verified | KBD365 | Behaviour of Enter in a table. Enter in other cells also adds a row below (unverified); Enter on an empty last row removes it and leaves the table. |
| KB.Table.RowBelow | Insert a row below the current row | Keyboard/Tables/Insert a row below the current row | shortcut | `Mod+Enter` | table.insertBelow | P1 | wired | KBD365 | in a table cell |
| KB.Table.ParagraphInCell | Create another paragraph in the same cell | Keyboard/Tables/Create another paragraph in the same cell | shortcut | `Alt+Enter` | table.newParagraphInCell | P1 | verified | KBD365 |  |
| KB.Table.ColumnRight | Create a column to the right | Keyboard/Tables/Create a column to the right | shortcut | `Mod+Alt+R` | table.insertRight | P1 | wired | KBD365 |  |
| KB.Table.ColumnLeft | Create a column to the left | Keyboard/Tables/Create a column to the left | shortcut | `Mod+Alt+E` | table.insertLeft | P1 | wired | KBD365 |  |
| KB.Table.RowAboveEnter | Create a row above: Enter at the beginning of a row (not the first) | Keyboard/Tables/Create a row above: Enter at the beginning of a row (not the first) | key-behaviour |  |  | P1 | missing | KBD365 | behaviour of Enter in a table |
| KB.Table.TabInLastCell | Create a new row: Tab in the last cell | Keyboard/Tables/Create a new row: Tab in the last cell | key-behaviour |  |  | P1 | verified | KBD365 | behaviour of Tab in a table |
| KB.Table.DeleteEmptyRow | Delete the current empty row: Delete twice at its beginning | Keyboard/Tables/Delete the current empty row: Delete twice at its beginning | key-behaviour |  |  | P2 | verified | KBD365 | One Delete, not the reference's reported two; same end state (the row is gone) without a cross-call key-repeat counter. |
| KB.Select.ToLineEnd | Select to the end of the line | Keyboard/Select/Select to the end of the line | shortcut | `Shift+End` | caret.selectLineEnd | P0 | verified | KBD365 |  |
| KB.Select.Line | Select the whole line (Shift+Down at the line start) | Keyboard/Select/Select the whole line (Shift+Down at the line start) | shortcut | `Shift+Down` | caret.selectDown | P0 | wired | KBD365 |  |
| KB.Select.Title | Jump to the page title and select it | Keyboard/Select/Jump to the page title and select it | shortcut | `Mod+Shift+T` | nav.pageTitle | P0 | wired | KBD365 |  |
| KB.Select.Cancel | Cancel selecting the outline or page | Keyboard/Select/Cancel selecting the outline or page | shortcut | `Escape` | edit.cancelSelection | P0 | wired | KBD365 |  |
| KB.Select.MoveUp | Move the selected paragraphs up | Keyboard/Select/Move the selected paragraphs up | shortcut | `Alt+Shift+Up` | outline.moveUp | P0 | verified | KBD365 |  |
| KB.Select.MoveDown | Move the selected paragraphs down | Keyboard/Select/Move the selected paragraphs down | shortcut | `Alt+Shift+Down` | outline.moveDown | P0 | wired | KBD365 |  |
| KB.Select.Subordinates | Select the current paragraph and its subordinate paragraphs | Keyboard/Select/Select the current paragraph and its subordinate paragraphs | shortcut | `Mod+Shift+-` | outline.selectBranch | P1 | verified | KBD365 | see KB.Format.Subscript for the documented collision |
| KB.Select.Delete | Delete the selected note or object | Keyboard/Select/Delete the selected note or object | shortcut | `Delete` | edit.delete | P0 | wired | KBD365 |  |
| KB.Select.Back | Go back to the last page visited | Keyboard/Select/Go back to the last page visited | shortcut | `Alt+Left` | nav.back | P0 | verified | KBD365 |  |
| KB.Select.Forward | Go forward to the next page visited | Keyboard/Select/Go forward to the next page visited | shortcut | `Alt+Right` | nav.forward | P1 | verified | KBD365 |  |
| KB.Tag.ToDo | Apply, check or clear the To Do tag | Keyboard/Tags/Apply, check or clear the To Do tag | shortcut | `Mod+1` | tag.todo | P0 | verified | KBD365 |  |
| KB.Tag.Important | Important tag | Keyboard/Tags/Important tag | shortcut | `Mod+2` | tag.important | P0 | verified | KBD365 |  |
| KB.Tag.Question | Question tag | Keyboard/Tags/Question tag | shortcut | `Mod+3` | tag.question | P0 | verified | KBD365 |  |
| KB.Tag.RememberForLater | Remember for later tag | Keyboard/Tags/Remember for later tag | shortcut | `Mod+4` | tag.rememberForLater | P0 | verified | KBD365 |  |
| KB.Tag.Definition | Definition tag | Keyboard/Tags/Definition tag | shortcut | `Mod+5` | tag.definition | P0 | verified | KBD365 |  |
| KB.Tag.Highlight | Highlight tag | Keyboard/Tags/Highlight tag | shortcut | `Mod+6` | tag.highlight | P0 | verified | KBD365 |  |
| KB.Tag.Contact | Contact tag | Keyboard/Tags/Contact tag | shortcut | `Mod+7` | tag.contact | P0 | verified | KBD365 |  |
| KB.Tag.Address | Address tag | Keyboard/Tags/Address tag | shortcut | `Mod+8` | tag.address | P0 | verified | KBD365 |  |
| KB.Tag.PhoneNumber | Phone number tag | Keyboard/Tags/Phone number tag | shortcut | `Mod+9` | tag.phoneNumber | P0 | verified | KBD365 |  |
| KB.Tag.RemoveAll | Remove all tags from the selected notes | Keyboard/Tags/Remove all tags from the selected notes | shortcut | `Mod+0` | tag.removeAll | P0 | verified | KBD365 |  |
| KB.Outline.Level1 | Collapse the outline to level 1 | Keyboard/Outlines/Collapse the outline to level 1 | shortcut | `Alt+Shift+1` | outline.level1 | P1 | verified | KBD365 |  |
| KB.Outline.Level2 | Expand the outline to level 2 | Keyboard/Outlines/Expand the outline to level 2 | shortcut | `Alt+Shift+2` | outline.level2 | P1 | wired | KBD365 |  |
| KB.Outline.Level3 | Expand the outline to level 3 | Keyboard/Outlines/Expand the outline to level 3 | shortcut | `Alt+Shift+3` | outline.level3 | P1 | wired | KBD365 |  |
| KB.Outline.Level4 | Expand the outline to level 4 | Keyboard/Outlines/Expand the outline to level 4 | shortcut | `Alt+Shift+4` | outline.level4 | P1 | wired | KBD365 |  |
| KB.Outline.Level5 | Expand the outline to level 5 | Keyboard/Outlines/Expand the outline to level 5 | shortcut | `Alt+Shift+5` | outline.level5 | P1 | wired | KBD365 |  |
| KB.Outline.Level6 | Expand the outline to level 6 | Keyboard/Outlines/Expand the outline to level 6 | shortcut | `Alt+Shift+6` | outline.level6 | P1 | wired | KBD365 |  |
| KB.Outline.Level7 | Expand the outline to level 7 | Keyboard/Outlines/Expand the outline to level 7 | shortcut | `Alt+Shift+7` | outline.level7 | P1 | wired | KBD365 |  |
| KB.Outline.Level8 | Expand the outline to level 8 | Keyboard/Outlines/Expand the outline to level 8 | shortcut | `Alt+Shift+8` | outline.level8 | P1 | wired | KBD365 |  |
| KB.Outline.Level9 | Expand the outline to level 9 | Keyboard/Outlines/Expand the outline to level 9 | shortcut | `Alt+Shift+9` | outline.level9 | P1 | wired | KBD365 |  |
| KB.Outline.ExpandAll | Expand all levels | Keyboard/Outlines/Expand all levels | shortcut | `Alt+Shift+0` | outline.expandAll | P1 | verified | KBD365 |  |
| KB.Outline.ExpandHeading | Expand the selected heading | Keyboard/Outlines/Expand the selected heading | shortcut | `Alt+Shift+= / Alt+Shift+Plus` | outline.expand | P1 | verified | KBD365 | two documented spellings of the same keys |
| KB.Outline.CollapseHeading | Collapse the selected heading | Keyboard/Outlines/Collapse the selected heading | shortcut | `Alt+Shift+-` | outline.collapse | P1 | verified | KBD365 | "Alt+Shift+Hyphen" and "Alt+Shift+Minus" in the table: same keys |
| KB.Outline.IndentTab | Increase the indent by one level | Keyboard/Outlines/Increase the indent by one level | shortcut | `Tab` | para.indent | P0 | wired | KBD365 | Tab at the start of a paragraph |
| KB.Outline.OutdentTab | Decrease the indent by one level | Keyboard/Outlines/Decrease the indent by one level | shortcut | `Shift+Tab` | para.outdent | P0 | wired | KBD365 |  |
| KB.Language.LTR | Set writing direction left to right | Keyboard/Language/Set writing direction left to right | shortcut | `Mod+LeftShift` | para.ltr | P2 | missing | KBD365 | left Ctrl+Shift chord; needs side-specific modifiers |
| KB.Language.RTL | Set writing direction right to left | Keyboard/Language/Set writing direction right to left | shortcut | `Mod+RightShift` | para.rtl | P2 | missing | KBD365 | needs side-specific modifiers |
| KB.Pages.FullPage | Enable or disable full-page view | Keyboard/Pages/Enable or disable full-page view | shortcut | `F11` | view.fullPage | P0 | wired | KBD365 |  |
| KB.Pages.ToggleGroup | Expand or collapse a page group in the page list | Keyboard/Pages/Expand or collapse a page group in the page list | shortcut | `Mod+Shift+*` | page.toggleSubpages | P1 | verified | KBD365 |  |
| KB.Pages.Print | Print the current page | Keyboard/Pages/Print the current page | shortcut | `Mod+P` | file.print | P1 | missing | KBD365 |  |
| KB.Pages.NewPage | Add a new page at the end of the section | Keyboard/Pages/Add a new page at the end of the section | shortcut | `Mod+N` | page.new | P0 | verified | KBD365 |  |
| KB.Pages.WidenList | Increase the width of the page list | Keyboard/Pages/Increase the width of the page list | shortcut | `Mod+Shift+[` | ui.pageListWider | P2 | wired | KBD365 |  |
| KB.Pages.NarrowList | Decrease the width of the page list | Keyboard/Pages/Decrease the width of the page list | shortcut | `Mod+Shift+]` | ui.pageListNarrower | P2 | wired | KBD365 |  |
| KB.Pages.Promote | Decrease the indent level of the current page | Keyboard/Pages/Decrease the indent level of the current page | shortcut | `Mod+Alt+[` | page.promote | P0 | verified | KBD365 |  |
| KB.Pages.Demote | Increase the indent level of the current page | Keyboard/Pages/Increase the indent level of the current page | shortcut | `Mod+Alt+]` | page.demote | P0 | verified | KBD365 |  |
| KB.Pages.NewSubpage | Create a new subpage below the current page | Keyboard/Pages/Create a new subpage below the current page | shortcut | `Mod+Shift+Alt+N` | page.newSubpage | P0 | verified | KBD365 |  |
| KB.Pages.NewPageBelow | Create a new page below the current page tab at the same level | Keyboard/Pages/Create a new page below the current page tab at the same level | shortcut | `Mod+Alt+N` | page.newBelow | P1 | verified | KBD365 | from the Windows 10 table / OneNote 2016 documentation |
| KB.Pages.SelectPage | Select the current page | Keyboard/Pages/Select the current page | shortcut | `Mod+Shift+A` | page.select | P1 | verified | KBD365 | Ctrl+A then selects the whole page group |
| KB.Pages.MoveUp | Move the selected page up in the page list | Keyboard/Pages/Move the selected page up in the page list | key-behaviour |  | page.moveUp | P0 | wired | KBD365 | Alt+Shift+Up while the page list has focus (same keys as KB.Select.MoveUp on the canvas) |
| KB.Pages.MoveDown | Move the selected page down in the page list | Keyboard/Pages/Move the selected page down in the page list | key-behaviour |  | page.moveDown | P0 | wired | KBD365 | Alt+Shift+Down while the page list has focus |
| KB.Pages.FirstVisible | Go to the first page in the visible set | Keyboard/Pages/Go to the first page in the visible set | shortcut | `Alt+PageUp` | nav.firstVisiblePage | P2 | wired | KBD365 |  |
| KB.Pages.LastVisible | Go to the last page in the visible set | Keyboard/Pages/Go to the last page in the visible set | shortcut | `Alt+PageDown` | nav.lastVisiblePage | P2 | wired | KBD365 |  |
| KB.Pages.ScrollUp | Scroll up in the current page | Keyboard/Pages/Scroll up in the current page | shortcut | `PageUp` | view.scrollPageUp | P0 | wired | KBD365 |  |
| KB.Pages.ScrollDown | Scroll down in the current page | Keyboard/Pages/Scroll down in the current page | shortcut | `PageDown` | view.scrollPageDown | P0 | wired | KBD365 |  |
| KB.Pages.Top | Scroll to the top of the page | Keyboard/Pages/Scroll to the top of the page | shortcut | `Mod+Home` | view.scrollTop | P0 | wired | KBD365 |  |
| KB.Pages.Bottom | Scroll to the bottom of the page | Keyboard/Pages/Scroll to the bottom of the page | shortcut | `Mod+End` | view.scrollBottom | P0 | wired | KBD365 |  |
| KB.Pages.NextParagraph | Go to the next paragraph | Keyboard/Pages/Go to the next paragraph | shortcut | `Mod+Down` | caret.nextParagraph | P0 | verified | KBD365 |  |
| KB.Pages.PrevParagraph | Go to the previous paragraph | Keyboard/Pages/Go to the previous paragraph | shortcut | `Mod+Up` | caret.prevParagraph | P0 | verified | KBD365 |  |
| KB.Pages.ExpandUp | Move the cursor up on the page, or expand the page up | Keyboard/Pages/Move the cursor up on the page, or expand the page up | shortcut | `Mod+Alt+Up` | caret.pageUp | P2 | missing | KBD365 |  |
| KB.Pages.ExpandDown | Move the cursor down, or expand the page down | Keyboard/Pages/Move the cursor down, or expand the page down | shortcut | `Mod+Alt+Down` | caret.pageDown | P2 | missing | KBD365 |  |
| KB.Pages.ExpandLeft | Move the cursor left, or expand the page left | Keyboard/Pages/Move the cursor left, or expand the page left | shortcut | `Mod+Alt+Left` | caret.pageLeft | P2 | missing | KBD365 |  |
| KB.Pages.ExpandRight | Move the cursor right, or expand the page right | Keyboard/Pages/Move the cursor right, or expand the page right | shortcut | `Mod+Alt+Right` | caret.pageRight | P2 | missing | KBD365 |  |
| KB.Pages.NextContainer | Go to the next note container | Keyboard/Pages/Go to the next note container | shortcut | `Alt+Down` | nav.nextContainer | P1 | verified | KBD365 |  |
| KB.Pages.ZoomIn | Zoom in | Keyboard/Pages/Zoom in | shortcut | `Mod+Plus` | view.zoomIn | P0 | wired | KBD365 | the Windows 10 table: Ctrl+Alt+Plus (keypad) or Ctrl+Alt+Shift+Plus |
| KB.Pages.ZoomOut | Zoom out | Keyboard/Pages/Zoom out | shortcut | `Mod+-` | view.zoomOut | P0 | wired | KBD365 | the Windows 10 table: Ctrl+Alt+Minus (keypad) or Ctrl+Alt+Shift+Hyphen |
| KB.Pages.Sync | Synchronize the notebook (notes are saved automatically) | Keyboard/Pages/Synchronize the notebook (notes are saved automatically) | shortcut | `Mod+S` | file.save | P0 | wired | KBD365 | locally: flush pending saves now |
| KB.Notebooks.Open | Open a notebook | Keyboard/Notebooks and sections/Open a notebook | shortcut | `Mod+O` | file.openNotebook | P0 | wired | KBD365 |  |
| KB.Notebooks.OpenSection | Open a section | Keyboard/Notebooks and sections/Open a section | shortcut | `Mod+Alt+Shift+O` | file.openSection | P1 | missing | KBD365 |  |
| KB.Notebooks.NewSection | Create a new section | Keyboard/Notebooks and sections/Create a new section | shortcut | `Mod+T` | section.new | P0 | verified | KBD365 |  |
| KB.Notebooks.NextSection | Go to the next section | Keyboard/Notebooks and sections/Go to the next section | shortcut | `Mod+Tab` | nav.nextSection | P0 | verified | KBD365 |  |
| KB.Notebooks.PrevSection | Go to the previous section | Keyboard/Notebooks and sections/Go to the previous section | shortcut | `Mod+Shift+Tab` | nav.prevSection | P0 | verified | KBD365 |  |
| KB.Notebooks.NextPage | Go to the next page in the section | Keyboard/Notebooks and sections/Go to the next page in the section | shortcut | `Mod+PageDown` | nav.nextPage | P0 | wired | KBD365 |  |
| KB.Notebooks.PrevPage | Go to the previous page in the section | Keyboard/Notebooks and sections/Go to the previous page in the section | shortcut | `Mod+PageUp` | nav.prevPage | P0 | wired | KBD365 |  |
| KB.Notebooks.FirstPage | Go to the first page in the section | Keyboard/Notebooks and sections/Go to the first page in the section | shortcut | `Alt+Home` | nav.firstPage | P0 | verified | KBD365 |  |
| KB.Notebooks.LastPage | Go to the last page in the section | Keyboard/Notebooks and sections/Go to the last page in the section | shortcut | `Alt+End` | nav.lastPage | P0 | verified | KBD365 |  |
| KB.Notebooks.MoveCopyPage | Move or copy the current page | Keyboard/Notebooks and sections/Move or copy the current page | shortcut | `Mod+Alt+M` | page.moveOrCopy | P0 | wired | KBD365 |  |
| KB.Notebooks.FocusPageTab | Put focus on the current page tab | Keyboard/Notebooks and sections/Put focus on the current page tab | shortcut | `Mod+Alt+G` | nav.focusPageTab | P1 | wired | KBD365 |  |
| KB.Notebooks.FocusSectionTab | Put focus on the current section tab | Keyboard/Notebooks and sections/Put focus on the current section tab | shortcut | `Mod+Shift+G` | nav.focusSectionTab | P1 | wired | KBD365 | In the 2024 UI the focus goes to the current section in the notebook tree. |
| KB.Notebooks.MoveSection | Move the current section (Ctrl+Shift+G, arrows, Shift+F10) | Keyboard/Notebooks and sections/Move the current section (Ctrl+Shift+G, arrows, Shift+F10) | key-behaviour |  | section.moveOrCopy | P2 | missing | KBD365 | key sequence through the section tab context menu |
| KB.Notebooks.SwitchNotebook | Switch notebook on the navigation bar | Keyboard/Notebooks and sections/Switch notebook on the navigation bar | shortcut | `Mod+G` | nav.notebookList | P1 | wired | KBD365 | then arrows/Tab and Enter |
| KB.Search.All | Move to the Search box (all notebooks) | Keyboard/Search/Move to the Search box (all notebooks) | shortcut | `Mod+E` | search.all | P0 | wired | KBD365 |  |
| KB.Search.PreviewNext | While searching, preview the next result (Down) | Keyboard/Search/While searching, preview the next result (Down) | key-behaviour |  | search.previewNext | P1 | missing | KBD365 | Down in the search results |
| KB.Search.GoToResult | Go to the selected result and dismiss search (Enter) | Keyboard/Search/Go to the selected result and dismiss search (Enter) | key-behaviour |  | search.openResult | P0 | wired | KBD365 | Enter in the search results |
| KB.Search.ChangeScope | Change the search scope (Ctrl+E, Tab, Space) | Keyboard/Search/Change the search scope (Ctrl+E, Tab, Space) | key-behaviour |  | search.scope | P1 | missing | KBD365 | key sequence |
| KB.Search.ResultsPane | Open the Search Results pane (after searching) | Keyboard/Search/Open the Search Results pane (after searching) | shortcut | `Alt+O` | search.resultsPane | P1 | wired | KBD365 |  |
| KB.Search.Page | Search the current page | Keyboard/Search/Search the current page | shortcut | `Mod+F` | search.page | P0 | wired | KBD365 |  |
| KB.Search.Next | Next result on the page | Keyboard/Search/Next result on the page | shortcut | `F3` | search.next | P0 | verified | KBD365 | Enter too, in the find box |
| KB.Search.Prev | Previous result on the page | Keyboard/Search/Previous result on the page | shortcut | `Shift+F3` | search.prev | P0 | verified | KBD365 |  |
| KB.Search.Dismiss | Dismiss the search and return to the page (Esc) | Keyboard/Search/Dismiss the search and return to the page (Esc) | key-behaviour |  | search.dismiss | P0 | verified | KBD365 | Escape in the search box |
| KB.Share.Email | Send the selected pages in an email | Keyboard/Share/Send the selected pages in an email | shortcut | `Mod+Shift+E` | share.emailPage | P2 | missing | KBD365 |  |
| KB.Share.TaskToday | Create a Today Outlook task | Keyboard/Share/Create a Today Outlook task | shortcut | `Mod+Shift+1` |  | P3 | out-of-scope | KBD365 | Outlook |
| KB.Share.TaskTomorrow | Create a Tomorrow Outlook task | Keyboard/Share/Create a Tomorrow Outlook task | shortcut | `Mod+Shift+2` |  | P3 | out-of-scope | KBD365 | Outlook |
| KB.Share.TaskThisWeek | Create a This Week Outlook task | Keyboard/Share/Create a This Week Outlook task | shortcut | `Mod+Shift+3` |  | P3 | out-of-scope | KBD365 | Outlook |
| KB.Share.TaskNextWeek | Create a Next Week Outlook task | Keyboard/Share/Create a Next Week Outlook task | shortcut | `Mod+Shift+4` |  | P3 | out-of-scope | KBD365 | Outlook |
| KB.Share.TaskNoDate | Create a No Date Outlook task | Keyboard/Share/Create a No Date Outlook task | shortcut | `Mod+Shift+5` |  | P3 | out-of-scope | KBD365 | Outlook |
| KB.Share.TaskOpen | Open the selected Outlook task | Keyboard/Share/Open the selected Outlook task | shortcut | `Mod+Shift+K` |  | P3 | out-of-scope | KBD365 | Outlook |
| KB.Share.TaskComplete | Mark the selected Outlook task as complete | Keyboard/Share/Mark the selected Outlook task as complete | shortcut | `Mod+Shift+9` |  | P3 | out-of-scope | KBD365 | Outlook |
| KB.Share.TaskDelete | Delete the selected Outlook task | Keyboard/Share/Delete the selected Outlook task | shortcut | `Mod+Shift+0` |  | P3 | out-of-scope | KBD365 | Outlook |
| KB.Share.SyncCurrent | Synchronize changes in the current shared notebook | Keyboard/Share/Synchronize changes in the current shared notebook | shortcut | `Shift+F9` | notebook.syncNow | P2 | missing | KBD365 |  |
| KB.Share.SyncAll | Synchronize changes in all shared notebooks | Keyboard/Share/Synchronize changes in all shared notebooks | shortcut | `F9` | notebook.syncAll | P2 | missing | KBD365 |  |
| KB.Share.MarkUnread | Mark the current page as unread | Keyboard/Share/Mark the current page as unread | shortcut | `Mod+Q` | history.markUnread | P2 | missing | KBD365 |  |
| KB.Protect.LockAll | Lock all password-protected sections | Keyboard/Protect/Lock all password-protected sections | shortcut | `Mod+Alt+L` | protect.lockAll | P1 | missing | KBD365 |  |
| KB.App.Help | Help | Keyboard/Application/Help | shortcut | `F1` | help.help | P1 | missing | KBD365 | standard Office key |
| KB.App.Ribbon | Collapse or expand the ribbon | Keyboard/Application/Collapse or expand the ribbon | shortcut | `Mod+F1` | ui.collapseRibbon | P1 | wired | KBD365 | standard Office key; unverified for OneNote |
| KB.App.Keytips | Show keytips | Keyboard/Application/Show keytips | shortcut | `F10` | ui.keytips | P2 | missing | KBD365 | Alt alone too |

## Options

The OneNote Options dialog, category by category. Setting names are functional descriptions from the user-visible dialog (unverified wording).

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| OPT.General.ShowMiniToolbar | Show Mini Toolbar on selection | File/Options/General/Show Mini Toolbar on selection | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.General.LivePreview | Enable Live Preview | File/Options/General/Enable Live Preview | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.General.ScreenTipStyle | ScreenTip style | File/Options/General/ScreenTip style | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.General.UserName | User name (author of edits) | File/Options/General/User name (author of edits) | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.General.Initials | Initials | File/Options/General/Initials | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.General.OfficeTheme | Office Theme (Colorful, Dark Gray, Black, White, Use system setting) | File/Options/General/Office Theme (Colorful, Dark Gray, Black, White, Use system setting) | option |  |  | P1 | wired | GK (OneNote Options dialog, user-visible); M365-2024 | unverified wording |
| OPT.General.OfficeBackground | Office Background | File/Options/General/Office Background | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.General.Privacy | Privacy settings | File/Options/General/Privacy settings | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Display.TrayIcon | Place OneNote icon in the notification area of the taskbar | File/Options/Display/Place OneNote icon in the notification area of the taskbar | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Display.NewPagesRuleLines | Create all new pages with rule lines | File/Options/Display/Create all new pages with rule lines | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Display.DockNewWindows | Dock new OneNote windows to the side of the desktop | File/Options/Display/Dock new OneNote windows to the side of the desktop | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Display.PageTabsLeft | Page tabs appear on the left | File/Options/Display/Page tabs appear on the left | option |  |  | P2 | out-of-scope | GK (OneNote Options dialog, user-visible) | unverified wording; not present in the 2024 UI (decision D1): the page column is always beside the notebook tree (unverified). |
| OPT.Display.NavBarLeft | Navigation bar appears on the left | File/Options/Display/Navigation bar appears on the left | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Display.ScrollBarLeft | Vertical scroll bar appears on the left | File/Options/Display/Vertical scroll bar appears on the left | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Display.FloatingNewPage | Show the floating New Page button near page tabs | File/Options/Display/Show the floating New Page button near page tabs | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Display.ShowNoteContainers | Show note containers on pages | File/Options/Display/Show note containers on pages | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Display.ScreenClippingNotifications | Disable screen clipping notifications | File/Options/Display/Disable screen clipping notifications | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Proofing.AutoCorrect | AutoCorrect Options… | File/Options/Proofing/AutoCorrect Options… | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Proofing.IgnoreUppercase | Ignore words in UPPERCASE | File/Options/Proofing/Ignore words in UPPERCASE | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Proofing.IgnoreNumbers | Ignore words that contain numbers | File/Options/Proofing/Ignore words that contain numbers | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Proofing.IgnoreUrls | Ignore Internet and file addresses | File/Options/Proofing/Ignore Internet and file addresses | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Proofing.FlagRepeated | Flag repeated words | File/Options/Proofing/Flag repeated words | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Proofing.CustomDictionaries | Custom Dictionaries… | File/Options/Proofing/Custom Dictionaries… | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Proofing.CheckAsYouType | Check spelling as you type | File/Options/Proofing/Check spelling as you type | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Proofing.HideSpellingErrors | Hide spelling and grammar errors | File/Options/Proofing/Hide spelling and grammar errors | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Proofing.CheckGrammar | Check grammar with spelling | File/Options/Proofing/Check grammar with spelling | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SaveBackup.QuickNotesSection | Quick Notes section location | File/Options/Save & Backup/Quick Notes section location | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SaveBackup.BackupFolder | Backup folder | File/Options/Save & Backup/Backup folder | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SaveBackup.DefaultNotebookLocation | Default notebook location | File/Options/Save & Backup/Default notebook location | option |  |  | P0 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SaveBackup.AutoBackup | Automatically back up my notebook at the following time interval | File/Options/Save & Backup/Automatically back up my notebook at the following time interval | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SaveBackup.BackupCopies | Number of backup copies of each file | File/Options/Save & Backup/Number of backup copies of each file | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SaveBackup.BackupChangedNow | Back Up Changed Files Now | File/Options/Save & Backup/Back Up Changed Files Now | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SaveBackup.BackupAllNow | Back Up All Notebooks Now | File/Options/Save & Backup/Back Up All Notebooks Now | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SaveBackup.OptimizeThreshold | Percentage of unused space to allow in files without optimizing | File/Options/Save & Backup/Percentage of unused space to allow in files without optimizing | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SaveBackup.OptimizeInterval | Optimize section files after the following time interval | File/Options/Save & Backup/Optimize section files after the following time interval | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SaveBackup.OptimizeAllNow | Optimize All Files Now | File/Options/Save & Backup/Optimize All Files Now | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SaveBackup.CacheLocation | Cache file location | File/Options/Save & Backup/Cache file location | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SendtoOneNote.SendEmail | Email messages: destination | File/Options/Send to OneNote/Email messages: destination | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SendtoOneNote.SendMeetingNotes | Meeting notes: destination | File/Options/Send to OneNote/Meeting notes: destination | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SendtoOneNote.SendWebContent | Web content: destination | File/Options/Send to OneNote/Web content: destination | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SendtoOneNote.SendPrint | Print to OneNote: destination | File/Options/Send to OneNote/Print to OneNote: destination | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.SendtoOneNote.SendScreenClipping | Screen clippings: destination | File/Options/Send to OneNote/Screen clippings: destination | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.AudioVideo.AudioDevice | Audio recording device | File/Options/Audio & Video/Audio recording device | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.AudioVideo.AudioCodec | Audio codec and format | File/Options/Audio & Video/Audio codec and format | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.AudioVideo.VideoDevice | Video recording device | File/Options/Audio & Video/Video recording device | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.AudioVideo.VideoProfile | Video profile | File/Options/Audio & Video/Video profile | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.AudioVideo.AudioSearch | Enable searching audio and video recordings for words | File/Options/Audio & Video/Enable searching audio and video recordings for words | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.AudioVideo.RewindLinked | Rewind from start of paragraph when playing linked audio/video by N seconds | File/Options/Audio & Video/Rewind from start of paragraph when playing linked audio/video by N seconds | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Language.EditingLanguages | Choose editing languages | File/Options/Language/Choose editing languages | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Language.DisplayLanguage | Choose display language (interface) | File/Options/Language/Choose display language (interface) | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.CustomizeRibbon.CustomizeRibbon | Customize the Ribbon (tabs, groups, commands; reset; import/export) | File/Options/Customize Ribbon/Customize the Ribbon (tabs, groups, commands; reset; import/export) | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.QuickAccessToolbar.CustomizeQAT | Customize the Quick Access Toolbar (commands, order, below the ribbon) | File/Options/Quick Access Toolbar/Customize the Quick Access Toolbar (commands, order, below the ribbon) | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.PasteLinkToSource | Include link to source when pasting from the Internet | File/Options/Advanced/Include link to source when pasting from the Internet | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.ShowPasteOptions | Show Paste Options button when content is pasted | File/Options/Advanced/Show Paste Options button when content is pasted | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.AutoNumbering | Apply auto-numbering to lists | File/Options/Advanced/Apply auto-numbering to lists | option |  |  | P0 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.AutoBulleting | Apply auto-bulleting to lists | File/Options/Advanced/Apply auto-bulleting to lists | option |  |  | P0 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.CalculateMath | Calculate mathematical expressions automatically | File/Options/Advanced/Calculate mathematical expressions automatically | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.CreateTableOnTab | Create a table when Tab is pressed after text | File/Options/Advanced/Create a table when Tab is pressed after text | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.WikiLinks | Enable Wiki linking ([[Page]]) | File/Options/Advanced/Enable Wiki linking ([[Page]]) | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.LinkedNotesOptions | Linked Notes options | File/Options/Advanced/Linked Notes options | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.EmailAttachCopy | Email: attach a copy of the original notes as a OneNote file | File/Options/Advanced/Email: attach a copy of the original notes as a OneNote file | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.EmailSignature | Email: add a signature | File/Options/Advanced/Email: add a signature | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.PenPressure | Use pen pressure sensitivity | File/Options/Advanced/Use pen pressure sensitivity | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.AutoSwitchPen | Automatically switch between pen and selection tool | File/Options/Advanced/Automatically switch between pen and selection tool | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.CompletedToDoSummary | Show completed To Do tags in the Tags Summary | File/Options/Advanced/Show completed To Do tags in the Tags Summary | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.MeasurementUnits | Measurement units | File/Options/Advanced/Measurement units | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.DefaultFont | Default font, size and colour | File/Options/Advanced/Default font, size and colour | option |  |  | P0 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.LockAfterIdle | Lock password-protected sections after N minutes idle | File/Options/Advanced/Lock password-protected sections after N minutes idle | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.LockOnLeave | Lock password-protected sections as soon as I navigate away from them | File/Options/Advanced/Lock password-protected sections as soon as I navigate away from them | option |  |  | P1 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.AddinsAccessLocked | Enable add-on programs to access password-protected sections when unlocked | File/Options/Advanced/Enable add-on programs to access password-protected sections when unlocked | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.BatteryOptions | Battery life optimization | File/Options/Advanced/Battery life optimization | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.ShowAuthors | Show authors | File/Options/Advanced/Show authors | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Advanced.SearchHighlight | Highlight search results on the page | File/Options/Advanced/Highlight search results on the page | option |  |  | P2 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.Add-ins.AddIns | Add-ins (COM add-ins list) | File/Options/Add-ins/Add-ins (COM add-ins list) | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |
| OPT.TrustCenter.TrustCenter | Trust Center (privacy options) | File/Options/Trust Center/Trust Center (privacy options) | option |  |  | P3 | missing | GK (OneNote Options dialog, user-visible) | unverified wording |

## Behaviour

Documented (or, where marked, plausible and unverified) behaviour. A behaviour row is wired when it works and a test mentions its ID; verified when the test pins the documented behaviour.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| BEH.Canvas.ClickToType | Click anywhere on a page and start typing: a note container is created at the click | Behaviour/Canvas | behaviour |  |  | P0 | verified | MS Support: "Take notes in OneNote" (basic tasks) |  |
| BEH.Canvas.ContainerAutoWidth | A new container grows in width with the text up to a maximum, then wraps; it grows in height with content | Behaviour/Canvas | behaviour |  |  | P0 | wired | GK; [MS-ONE] LayoutMaxWidth, IsLayoutSizeSetByUser | unverified maximum (we use 6.5in) |
| BEH.Canvas.ContainerResize | Dragging a container's right edge sets a fixed width (text rewraps) | Behaviour/Canvas | behaviour |  |  | P0 | verified | GK; [MS-ONE] IsLayoutSizeSetByUser |  |
| BEH.Canvas.ContainerMove | Dragging a container's top bar moves it anywhere on the page | Behaviour/Canvas | behaviour |  |  | P0 | verified | GK |  |
| BEH.Canvas.EmptyContainerRemoved | A container left empty disappears when the caret leaves it | Behaviour/Canvas | behaviour |  |  | P0 | wired | GK | unverified |
| BEH.Canvas.PageGrows | The page grows right and down as content is added; it never has a fixed size unless Paper Size is set | Behaviour/Canvas | behaviour |  |  | P0 | verified | GK; FLUENT19:PaperSize |  |
| BEH.Canvas.MergeContainers | Shift-dragging a container onto another merges them | Behaviour/Canvas | behaviour |  |  | P2 | missing | GK | unverified |
| BEH.Canvas.InsertSpace | Insert Space: drag on the page to push content down or right | Behaviour/Canvas | behaviour |  | canvas.insertSpace | P1 | wired | FLUENT19:PenInsertSpace | the longer way of the drag decides down or right; removing space stops at the line; the tool returns to Type after one use (unverified) |
| BEH.Outline.EnterNewParagraph | Enter splits/creates a paragraph at the same level; it keeps the list format | Behaviour/Outline | behaviour |  |  | P0 | verified | GK |  |
| BEH.Outline.TabIndent | Tab at the start of a paragraph indents it (and its children) one level; Shift+Tab outdents | Behaviour/Outline | behaviour |  |  | P0 | verified | KBD365 |  |
| BEH.Outline.EmptyBulletEnds | Enter on an empty bulleted or numbered paragraph ends the list (outdents, then removes the bullet) | Behaviour/Outline | behaviour |  |  | P0 | wired | GK | unverified exact sequence |
| BEH.Outline.AutoBullet | Typing "* " or "- " at the start of a paragraph starts a bulleted list (Options › Advanced) | Behaviour/Outline | behaviour |  |  | P0 | verified | GK (Options › Advanced: auto-bulleting) |  |
| BEH.Outline.AutoNumber | Typing "1. ", "a) ", "i. " at the start of a paragraph starts a numbered list | Behaviour/Outline | behaviour |  |  | P0 | verified | GK (Options › Advanced: auto-numbering) |  |
| BEH.Outline.NumberingSequence | Numbered lists renumber automatically; levels use 1., a., i. by default | Behaviour/Outline | behaviour |  |  | P0 | missing | GK | unverified default sequence |
| BEH.Outline.CollapseExpand | Double-clicking a paragraph handle collapses/expands its children; collapsed paragraphs show a + handle | Behaviour/Outline | behaviour |  |  | P1 | wired | GK |  |
| BEH.Outline.MoveParagraph | Alt+Shift+Up/Down moves the paragraph with its children past its siblings | Behaviour/Outline | behaviour |  |  | P0 | verified | KBD365 |  |
| BEH.Outline.DragParagraph | Dragging a paragraph handle moves the paragraph (with children) within or between containers | Behaviour/Outline | behaviour |  |  | P1 | verified | GK || The pointer's x sets the level; dropped away from every container it starts a new container (unverified). |
| BEH.Text.Undo | Undo/redo reverse whole user actions; typing coalesces into word-sized steps | Behaviour/Text | behaviour |  |  | P0 | wired | GK | unverified granularity |
| BEH.Text.FormatToggles | Bold/italic/underline toggle on the selection, or set the format for the next typed text at the caret | Behaviour/Text | behaviour |  |  | P0 | verified | GK |  |
| BEH.Text.FontSizeSteps | Increase/Decrease Font Size step through 8, 9, 10, 10.5, 11, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72 | Behaviour/Text | behaviour |  |  | P1 | wired | GK (Office font size list) | unverified list for OneNote |
| BEH.Text.DefaultFont | New text uses the default font (Calibri 11 by default, configurable) | Behaviour/Text | behaviour |  |  | P0 | missing | GK (Options › Advanced › Default font) | Calibri is proprietary: metric-compatible substitute (Carlito) when available |
| BEH.Text.Styles | Paragraph styles are paragraph-level: applying one replaces character formatting with the style's | Behaviour/Text | behaviour |  |  | P0 | missing | GK | unverified |
| BEH.Text.HeadingSizes | Built-in styles: Heading 1 16pt blue, Heading 2 14pt blue bold, Heading 3 12pt blue bold, Heading 4 12pt blue bold italic, Heading 5 11pt blue bold, Heading 6 11pt blue italic, Page Title 20pt, Citation 9pt grey, Quote 11pt italic grey, Code Consolas 10pt, Normal 11pt | Behaviour/Text | behaviour |  |  | P1 | missing | GK (user-visible defaults) | unverified exact values |
| BEH.Text.ClearFormatting | Clear All Formatting returns the selection to the Normal style | Behaviour/Text | behaviour |  |  | P0 | verified | KBD365 ("Apply the Normal style") |  |
| BEH.Text.HyperlinkAuto | Typing a URL followed by space or Enter turns it into a link | Behaviour/Text | behaviour |  |  | P1 | verified | GK |  |
| BEH.Text.MathCalc | Typing an expression followed by "=" and Space/Enter inserts the result (e.g. 12*3= → 36) | Behaviour/Text | behaviour |  |  | P1 | missing | GK (Options › Advanced: calculate mathematical expressions) |  |
| BEH.Links.Wiki | Typing [[Page Name]] creates a link to that page, creating the page in the section if it does not exist | Behaviour/Links | behaviour |  |  | P1 | verified | GK (wiki linking) |  |
| BEH.Links.CopyLink | Copy Link to Page/Section/Paragraph produces a link that navigates there (onenote: URI in the reference; notecraft: URI here) | Behaviour/Links | behaviour |  |  | P0 | missing | FLUENT19:PageLinkCreate |  |
| BEH.Links.PageRenameTracking | Links to pages keep working after the page is renamed or moved | Behaviour/Links | behaviour |  |  | P1 | missing | GK | id-based links |
| BEH.Tags.Toggle | Applying a tag already on a paragraph removes it; To Do toggles checked → unchecked → removed via Ctrl+1 sequence | Behaviour/Tags | behaviour |  |  | P0 | missing | KBD365 ("Apply, select, or clear the To Do tag") | unverified exact cycle |
| BEH.Tags.CheckboxClick | Clicking a To Do checkbox checks/unchecks it | Behaviour/Tags | behaviour |  |  | P0 | missing | GK |  |
| BEH.Tags.MultipleTags | A paragraph can carry several tags; their icons stack left of the paragraph | Behaviour/Tags | behaviour |  |  | P0 | missing | GK |  |
| BEH.Tags.Summary | Find Tags lists every tagged paragraph in the chosen scope, grouped, with click-to-navigate | Behaviour/Tags | behaviour |  |  | P0 | verified | FLUENT19:FindTaggedNotes |  |
| BEH.Tags.SummaryPage | Create Summary Page builds a new page with the tagged items | Behaviour/Tags | behaviour |  |  | P1 | verified | GK |  |
| BEH.Tags.Custom | Custom tags (name, symbol, font colour, highlight colour) are stored per user and apply to any notebook | Behaviour/Tags | behaviour |  |  | P1 | verified | GK | stored in settings, mirrored into the notebook for portability |
| BEH.Pages.Title | The page title is the page tab name; an untitled page shows "Untitled page" with the first line of content as fallback | Behaviour/Pages | behaviour |  |  | P0 | wired | GK | unverified fallback |
| BEH.Pages.DateTime | New pages show their creation date and time under the title; clicking it lets you change them | Behaviour/Pages | behaviour |  |  | P0 | missing | GK; [MS-ONE] TopologyCreationTimeStamp |  |
| BEH.Pages.Subpages | Pages can be indented up to two levels below a top-level page (3 levels); subpages move with their parent | Behaviour/Pages | behaviour |  |  | P0 | missing | GK; [MS-ONE] PageLevel | unverified max depth |
| BEH.Pages.Collapse | A page with subpages can be collapsed in the page list | Behaviour/Pages | behaviour |  |  | P0 | missing | FLUENT19:IndentedPagesCollapse |  |
| BEH.Pages.DragReorder | Pages, sections and notebooks reorder by drag & drop; dragging a page onto another section moves it | Behaviour/Pages | behaviour |  |  | P0 | missing | GK |  |
| BEH.Pages.NewPageInsertion | Hovering between page tabs shows an insertion point to add a page there | Behaviour/Pages | behaviour |  |  | P1 | missing | GK |  |
| BEH.Sections.Colors | Sections take one of 16 colours (or None); new sections get the next colour in a rotation | Behaviour/Sections | behaviour |  |  | P0 | wired | FLUENT19:NotebookColor1..16 | unverified rotation order |
| BEH.Sections.Groups | Section groups nest sections and other section groups; shown as tabs/folders | Behaviour/Sections | behaviour |  |  | P0 | missing | FLUENT19:SectionGroupInsert |  |
| BEH.Sections.NewSectionName | A new section is named "New Section 1", "New Section 2"… and starts in rename mode with one Untitled page | Behaviour/Sections | behaviour |  |  | P0 | wired | GK | unverified |
| BEH.Notebooks.Default | A new notebook starts with one section ("New Section 1") holding one Untitled page | Behaviour/Notebooks | behaviour |  |  | P0 | missing | GK | unverified |
| BEH.Notebooks.QuickNotes | Quick Notes are saved to the Quick Notes section of the default notebook | Behaviour/Notebooks | behaviour |  |  | P1 | missing | GK (Options › Save & Backup) |  |
| BEH.Recycle.DeletedPages | Deleted pages and sections go to the Notebook Recycle Bin and are kept for 60 days; they can be restored with Move or Copy | Behaviour/Recycle | behaviour |  |  | P0 | wired | GK (MS Support: notebook recycle bin) | unverified 60-day figure |
| BEH.Recycle.Empty | Empty Recycle Bin permanently deletes after confirmation | Behaviour/Recycle | behaviour |  |  | P0 | missing | FLUENT19:EmptyRecycleBin |  |
| BEH.Versions.Snapshots | Page versions are kept as edits happen (per author and time); a version can be restored, copied or deleted | Behaviour/Versions | behaviour |  |  | P1 | missing | FLUENT19:PageVersions |  |
| BEH.Save.Autosave | Notes are saved automatically; there is no Save command needed | Behaviour/Save | behaviour |  |  | P0 | verified | KBD365 note on Ctrl+S |  |
| BEH.Save.Backups | Automatic backups at the configured interval, N copies; Open Backups shows them | Behaviour/Save | behaviour |  |  | P1 | verified | GK (Options › Save & Backup) |  |
| BEH.Save.CrashRecovery | After a crash or kill -9 no saved note is lost and the notebook opens cleanly | Behaviour/Save | behaviour |  |  | P0 | wired | brief §6 M1 | The kill -9 test pins that the notebook reopens with every section whole; that the last completed save survives is not pinned (unverified). |
| BEH.Search.Instant | Search finds words as you type across the chosen scope; results show page title and section; matches are highlighted on the page | Behaviour/Search | behaviour |  |  | P0 | verified | KBD365 (search keys); GK |  |
| BEH.Search.Scope | Scopes: This Page, This Section, This Section Group, This Notebook, All Notebooks; default scope can be set | Behaviour/Search | behaviour |  |  | P0 | verified | FLUENT19:ContextMenuSearchScope |  |
| BEH.Search.PrefixMatch | Search matches word prefixes, case-insensitively, and finds titles, text, tags and file names | Behaviour/Search | behaviour |  |  | P0 | wired | GK | unverified matching rules |
| BEH.Search.Quoted | Quoted phrases match exactly; several words match pages that contain all of them | Behaviour/Search | behaviour |  |  | P1 | wired | GK | unverified |
| BEH.Search.LockedExcluded | Locked password-protected sections are not searched | Behaviour/Search | behaviour |  |  | P1 | missing | GK (MS Support: password-protect sections) |  |
| BEH.Password.Lock | A password-protected section shows a lock; its pages are hidden until unlocked; it relocks after the idle time | Behaviour/Password | behaviour |  |  | P1 | missing | FLUENT19:PasswordProtectSection; KBD365 (Ctrl+Alt+L) |  |
| BEH.Password.NoRecovery | A forgotten section password cannot be recovered | Behaviour/Password | behaviour |  |  | P1 | missing | GK (MS Support) |  |
| BEH.View.RuleLines | Rule lines: Narrow, College, Standard, Wide ruled; Small, Medium, Large, Very Large grid; None; colour; optional margin line | Behaviour/View | behaviour |  |  | P1 | missing | FLUENT19:RuleLinesMenu | unverified spacings |
| BEH.View.PageColor | Page colour: a palette of 16 light colours plus No Color | Behaviour/View | behaviour |  |  | P1 | missing | FLUENT19:PageColorGallery | unverified palette |
| BEH.View.Zoom | Zoom from 10% to 400%; Zoom In/Out step through fixed stops | Behaviour/View | behaviour |  |  | P0 | missing | FLUENT19:ZoomClassic | unverified stops |
| BEH.Paste.HTML | Pasting rich text keeps formatting (Keep Source Formatting is the default paste) | Behaviour/Paste | behaviour |  |  | P0 | wired | FLUENT19:PasteSourceFormatting | unverified default. HTML from other applications keeps paragraphs, headings, basic formatting, links, colour, lists (as levels) and tables (as table objects); a bounded, tolerant reader (notecraft-import-export); inline pictures (`data:image/…;base64` sources) become pictures between the paragraphs around them, pictures from the web are dropped (no network by default) |
| BEH.Paste.Images | Pasting an image creates a picture on the page at the caret | Behaviour/Paste | behaviour |  |  | P1 | wired | GK | a picture on the system clipboard (no text) is pasted as a picture where a new object goes; Ctrl+V with only a picture is caught on the key release (egui-winit drops the press) |
| BEH.Paste.Files | Dropping a file asks: attach file / insert printout | Behaviour/Paste | behaviour |  |  | P1 | wired | GK | a dropped picture file becomes a picture; a dropped PDF opens the Insert File dialog (Attach File, Insert Printout, Cancel; several PDFs dropped together are asked about once); other files are attached, since NoteCraft prints only PDFs and pictures (Insert Printout on the attachment's menu prints one afterwards). Dialog wording and default button unverified |
| BEH.Table.TabToCreate | Tab after text in a paragraph turns it into a two-column table | Behaviour/Table | behaviour |  |  | P1 | verified | KBD365 ("Create a table") | Decision D6: the table takes the paragraph's place (the container splits around it); a caret inside the text types a tab, a list item indents |
| BEH.Ink.PressureTilt | Pen pressure (and tilt where available) shapes ink stroke width | Behaviour/Ink | behaviour |  |  | P1 | wired | brief §6 M2 | Pens draw width × (0.4 + 1.2 × pressure), unverified curve; pressure from egui touch force where winit reports it, else 0.5; tilt not used |
| BEH.Ink.Lasso | Lasso select picks ink strokes and objects inside the loop | Behaviour/Ink | behaviour |  | draw.lassoSelect | P1 | wired | FLUENT19:LassoSelect | A stroke is picked when three quarters of its points are inside, another object when its centre is (unverified) |
| BEH.History.RecentEdits | Recent Edits lists pages changed in the chosen period, newest first | Behaviour/History | behaviour |  |  | P1 | missing | FLUENT19:FindByDateMenu |  |
| BEH.History.Unread | Pages changed by others since you last looked are bold in the page list; Next Unread goes there | Behaviour/History | behaviour |  |  | P2 | missing | FLUENT19:GoToNextUnreadPage |  |

## File formats

Interop constructs. Per-construct preserved/degraded/dropped numbers for `.one` live in docs/parity.md once M3 lands.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| FMT.One.Read | Read .one section files (OneNote 2010–2016 format, revision store) | Files/One | format |  |  | P0 | missing | [MS-ONESTORE] §2; [MS-ONE] §2 | M3 |
| FMT.One.ReadPages | Pages, titles, levels (subpages), creation times | Files/One | format |  |  | P0 | missing | [MS-ONE] jcidPageNode, PageLevel, CachedTitleString |  |
| FMT.One.ReadOutlines | Outlines, outline elements, indentation, position and width | Files/One | format |  |  | P0 | missing | [MS-ONE] jcidOutlineNode, jcidOutlineElementNode |  |
| FMT.One.ReadRichText | Rich text: runs, fonts, sizes, bold/italic/underline/strike, colours, highlight, hyperlinks | Files/One | format |  |  | P0 | missing | [MS-ONE] jcidRichTextOENode, TextRunFormatting, jcidParagraphStyleObject |  |
| FMT.One.ReadLists | Bulleted and numbered lists | Files/One | format |  |  | P0 | missing | [MS-ONE] jcidNumberListNode |  |
| FMT.One.ReadTags | Note tags (shape, label, checked state) | Files/One | format |  |  | P0 | missing | [MS-ONE] jcidNoteTagSharedDefinitionContainer, NoteTagState |  |
| FMT.One.ReadImages | Pictures (embedded image data, size, alt text) | Files/One | format |  |  | P1 | missing | [MS-ONE] jcidImageNode, PictureContainer |  |
| FMT.One.ReadTables | Tables (rows, cells, column widths, borders) | Files/One | format |  |  | P1 | missing | [MS-ONE] jcidTableNode, jcidTableRowNode, jcidTableCellNode |  |
| FMT.One.ReadAttachments | Embedded files (file data store) | Files/One | format |  |  | P1 | missing | [MS-ONE] jcidEmbeddedFileNode; [MS-ONESTORE] FileDataStoreObject |  |
| FMT.One.ReadInk | Ink strokes (paths, pressure, colour, width) | Files/One | format |  |  | P1 | missing | [MS-ONE] jcidInkDataNode, InkStrokeProperties |  |
| FMT.One.ReadVersions | Page history (revisions) | Files/One | format |  |  | P2 | missing | [MS-ONESTORE] revision manifests |  |
| FMT.One.ReadEncrypted | Password-protected sections (.one with encryption) | Files/One | format |  |  | P2 | missing | [MS-ONESTORE] §2.3.4.? encryption (public spec section to be cited at implementation) | unverified scope |
| FMT.One.Write | Write .one section files OneNote can open | Files/One | format |  |  | P1 | missing | [MS-ONESTORE]; [MS-ONE] | M3 |
| FMT.Onetoc2.Read | Read .onetoc2 notebook table of contents (section order, colours) | Files/Onetoc2 | format |  |  | P0 | missing | [MS-ONETOC2]? ([MS-ONE] §2.? jcidSectionNode / table of contents) | unverified spec name |
| FMT.Onetoc2.Write | Write .onetoc2 | Files/Onetoc2 | format |  |  | P2 | missing | [MS-ONE] |  |
| FMT.Onepkg.Read | Read .onepkg packages (CAB archive of a notebook) | Files/Onepkg | format |  |  | P0 | missing | [MS-CAB] |  |
| FMT.Onepkg.Write | Write .onepkg packages | Files/Onepkg | format |  |  | P2 | missing | [MS-CAB] |  |
| FMT.One2007 | Read OneNote 2007 sections | Files/One2007 | format |  |  | P3 | missing | no public specification | gap: undocumented format |
| FMT.Export.Pdf | Export page/section/notebook to PDF | Files/Export | format |  |  | P1 | missing | ISO 32000 |  |
| FMT.Export.Docx | Export to Word (.docx) | Files/Export | format |  |  | P1 | missing | ECMA-376 (reuse WordCraft crates where possible) |  |
| FMT.Export.Html | Export to HTML / single-file web page (.mht) | Files/Export | format |  |  | P2 | missing | HTML; RFC 2557 (MHTML) |  |
| FMT.Export.Markdown | Export to Markdown | Files/Export | format |  |  | P1 | missing | CommonMark |  |
| FMT.Export.Images | Export a page to PNG/SVG | Files/Export | format |  |  | P2 | missing | PNG; SVG |  |
| FMT.Export.Archive | Export a whole notebook archive | Files/Export | format |  |  | P1 | missing | NoteCraft (.ncraft folder in a zip) |  |
| FMT.Import.Markdown | Import Markdown | Files/Import | format |  |  | P1 | missing | CommonMark |  |
| FMT.Import.Html | Import HTML | Files/Import | format |  |  | P2 | missing | HTML |  |
| FMT.Import.Text | Import plain text | Files/Import | format |  |  | P1 | missing | — |  |
| FMT.Import.Enex | Import Evernote .enex | Files/Import | format |  |  | P2 | missing | ENEX (public DTD) |  |
| FMT.Import.Pdf | Import PDF as printout | Files/Import | format |  |  | P2 | wired | ISO 32000 | as a file printout (attachment plus one picture per page; no text layer yet) |
| FMT.Import.Images | Import images | Files/Import | format |  |  | P1 | missing | PNG/JPEG/… |  |
| FMT.Native.Ncraft | Native notebook: folder of .ncraft section files + notebook.ncraft-toc, atomic saves | Files/Native | format |  |  | P0 | verified | NoteCraft (docs/architecture.md) |  |

## NoteCraft additions

Actions every editor needs that the reference exposes only implicitly (typing, arrow keys, clicks): they are commands too, so agents and the CLI can drive them. Not counted as reference parity on their own, but kept in the catalog so every command has a row.

| ID | Label | Path | Kind | Shortcut | Command | Pri | Status | Source | Notes |
|---|---|---|---|---|---|---|---|---|---|
| NC.Text.Insert | Type text | Keyboard/Typing/Type text | key-behaviour |  | text.insert | P0 | verified | NoteCraft | typing; one undo step per word |
| NC.Text.Enter | New paragraph (Enter) | Keyboard/Typing/New paragraph | key-behaviour | `Enter` | text.newline | P0 | wired | NoteCraft |  |
| NC.Caret.Set | Place the caret (click in text) | Mouse/Click in text | mouse |  | caret.set | P0 | wired | NoteCraft |  |
| NC.Caret.Left | Move left | Keyboard/Navigation/Left | key-behaviour | `Left` | caret.left | P0 | wired | NoteCraft | Shift extends |
| NC.Caret.Right | Move right | Keyboard/Navigation/Right | key-behaviour | `Right` | caret.right | P0 | wired | NoteCraft | Shift extends |
| NC.Caret.Up | Move up | Keyboard/Navigation/Up | key-behaviour | `Up` | caret.up | P0 | wired | NoteCraft | visual lines come from the UI |
| NC.Caret.Down | Move down | Keyboard/Navigation/Down | key-behaviour | `Down` | caret.down | P0 | wired | NoteCraft | visual lines come from the UI |
| NC.Object.Select | Select a note container (click its top bar) | Mouse/Select container | mouse |  | object.select | P0 | wired | NoteCraft |  |
| NC.Page.Select | Open a page (click its tab) | Mouse/Page tab | mouse |  | nav.goto | P0 | wired | NoteCraft | Ctrl/Shift+click select several |
| NC.Section.Select | Open a section (click it in the notebook tree) | Mouse/Section tab | mouse |  | section.select | P0 | wired | NoteCraft |  |
| NC.Notebook.Select | Open a notebook (click it in the notebook pane) | Mouse/Notebook pane | mouse |  | notebook.select | P0 | wired | NoteCraft |  |
| NC.Table.NextCell | Next Cell | Keyboard/Tables/Move to the next cell | key-behaviour | `Tab` | table.nextCell | P1 | wired | NoteCraft; KBD365 (Tables) | Move to the next table cell (Tab); from the last cell a new row, or a new column in a one-row table. Lands at the end of the cell's text (unverified) |
| NC.Table.PrevCell | Previous Cell | Keyboard/Tables/Move to the previous cell | key-behaviour | `Shift+Tab` | table.prevCell | P1 | verified | NoteCraft; KBD365 (Tables) | Move to the previous table cell (Shift+Tab). |
| NC.Table.SelectRange | Select Cells | Mouse/Drag across table cells | mouse |  | table.selectRange | P1 | verified | NoteCraft | Select table cells by dragging across them. |
| NC.Table.ColumnWidth | Column Width | Mouse/Drag a table column border | mouse |  | table.columnWidth | P1 | verified | NoteCraft; [MS-ONE] TableColumnWidths | Drag a table column's border to change its width. |
| NC.Table.ConvertTextToTable | Convert Text to Table | Insert/Tables/Table/Convert Text to Table | button |  | insert.convertTextToTable | P1 | wired | NoteCraft | The selected paragraphs become a table: a row per paragraph, tabs split the cells. Unverified placement in the reference |
| NC.Table.ConvertToText | Convert to Text | Table Layout/Convert/Convert to Text | button |  | table.convertToText | P2 | wired | NoteCraft | A table becomes text: a paragraph per row, cells joined by tabs. Unverified placement in the reference |
| NC.Draw.Stroke | Ink | Mouse/Canvas/Draw | mouse |  | draw.stroke | P1 | verified | NoteCraft (decision D4) | Dragging with a pen or highlighter draws one stroke: one page object and one undo step; the finished stroke is simplified (0.2 pt). |
| NC.Draw.Erase | Erase | Mouse/Canvas/Erase | mouse |  | draw.erase | P1 | wired | NoteCraft (decision D4) | Dragging with an eraser erases along the path; one drag is one undo step. Eraser radii (2.5/6/12 pt) unverified |
| NC.Draw.Shape | Shape | Mouse/Canvas/Shape | mouse |  | draw.shape | P1 | verified | NoteCraft (decision D4) | Dragging with the Shape tool draws the chosen shape with the selected pen as one ink stroke (one undo step); Shift keeps the proportions. |
| NC.Draw.RulerPlace | Move or Turn the Ruler | Mouse/Canvas/Ruler | mouse |  | draw.rulerPlace | P2 | verified | NoteCraft (decision D7) | Moves the ruler to or by a point and turns it to or by an angle (never an undo step). On the canvas a drag that starts on the ruler, away from its edges with the pen, moves it; the mouse wheel over it turns it a degree a notch, up counterclockwise, instead of scrolling. |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-14 | major | First version: FLUENT19 inventory (ribbon, contextual tabs, backstage, context menus, commands not in the ribbon, docked ribbon), KBD365 shortcuts, Microsoft 365 additions, UI surfaces, options, behaviour and formats |
| 2026-10-15 | major | Decision D1 (the 2024 UI): source `M365-2024`; 12 new rows (title-bar Search, Share/Copilot/Account out-of-scope, Simplified ribbon and mode switch, ☰ toggle, notebook tree, + New section, Quick notes, sort pages, expand button); 2016-only surfaces (section tabs, their area menu, page-tabs-left option) out-of-scope with their replacement named |
