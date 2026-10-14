//! The ribbon's layout: which catalog control sits where, and at what size.
//!
//! Labels, commands and kinds come from `docs/parity-catalog.md` (the `R.*`/`CT.*` rows); this
//! table only adds what the catalog does not record — placement and size (large button, stack
//! of small buttons, row of icon buttons, in-ribbon gallery, drop-down children). The order of
//! tabs, groups and controls follows the reference's control inventory (catalog source
//! FLUENT19), and `tests::layout_covers_every_ribbon_row` fails if a catalog ribbon row is
//! missing here or an id here is not in the catalog, so the ribbon's structure can never drift
//! from the reference's.

/// A control placement.
#[derive(Debug, Clone, Copy)]
pub enum Item {
    /// Large button: 32 px icon with the label underneath.
    Large(&'static str),
    /// Large button with a drop-down of child controls (split buttons and menus).
    LargeMenu(&'static str, &'static [&'static str]),
    /// A column of up to three small buttons (16 px icon + label).
    Stack(&'static [&'static str]),
    /// A row of small icon-only buttons (Bold, Italic, …) — children of menus may follow the id.
    Row(&'static [&'static str]),
    /// Small icon-only button with a drop-down of children.
    SmallMenu(&'static str, &'static [&'static str]),
    /// Combo box with a width in points.
    Combo(&'static str, f32),
    /// In-ribbon gallery: the gallery row, its items, and extra commands in its drop-down.
    Gallery(&'static str, &'static [&'static str], &'static [&'static str]),
    /// Two rows of small controls (the Basic Text group).
    TwoRows(&'static [Item], &'static [Item]),
    /// A column of mixed small controls (buttons and labelled menus).
    Column(&'static [Item]),
    /// A small button with its label.
    Small(&'static str),
    /// A small labelled button with a drop-down of children.
    LabeledMenu(&'static str, &'static [&'static str]),
}

/// A ribbon group.
#[derive(Debug, Clone, Copy)]
pub struct Group {
    /// Label under the group.
    pub name: &'static str,
    /// Controls, left to right.
    pub items: &'static [Item],
}

/// A ribbon tab.
#[derive(Debug, Clone, Copy)]
pub struct Tab {
    /// Tab label.
    pub name: &'static str,
    /// Groups, left to right.
    pub groups: &'static [Group],
}

const STYLE_ITEMS: &[&str] = &[
    "R.Home.Styles.Heading1",
    "R.Home.Styles.Heading2",
    "R.Home.Styles.Heading3",
    "R.Home.Styles.Heading4",
    "R.Home.Styles.Heading5",
    "R.Home.Styles.Heading6",
    "R.Home.Styles.PageTitle",
    "R.Home.Styles.Citation",
    "R.Home.Styles.Quote",
    "R.Home.Styles.Code",
    "R.Home.Styles.Normal",
];

const TAG_ITEMS: &[&str] = &[
    "R.Home.Tags.ToDo",
    "R.Home.Tags.Important",
    "R.Home.Tags.Question",
    "R.Home.Tags.RememberForLater",
    "R.Home.Tags.Definition",
    "R.Home.Tags.Highlight",
    "R.Home.Tags.Contact",
    "R.Home.Tags.Address",
    "R.Home.Tags.PhoneNumber",
    "R.Home.Tags.WebSiteToVisit",
    "R.Home.Tags.Idea",
    "R.Home.Tags.Password",
    "R.Home.Tags.Critical",
    "R.Home.Tags.ProjectA",
    "R.Home.Tags.ProjectB",
    "R.Home.Tags.MovieToSee",
    "R.Home.Tags.BookToRead",
    "R.Home.Tags.MusicToListenTo",
    "R.Home.Tags.SourceForArticle",
    "R.Home.Tags.RememberForBlog",
    "R.Home.Tags.DiscussWithPersonA",
    "R.Home.Tags.DiscussWithPersonB",
    "R.Home.Tags.DiscussWithManager",
    "R.Home.Tags.SendInEmail",
    "R.Home.Tags.ScheduleMeeting",
    "R.Home.Tags.CallBack",
    "R.Home.Tags.ToDoPriority1",
    "R.Home.Tags.ToDoPriority2",
    "R.Home.Tags.ClientRequest",
];

/// The always-visible tabs, in order (File opens the backstage and is drawn separately).
pub const TABS: &[Tab] = &[
    Tab {
        name: "Home",
        groups: &[
            Group {
                name: "Clipboard",
                items: &[
                    Item::LargeMenu(
                        "R.Home.Clipboard.PasteMenu",
                        &[
                            "R.Home.Clipboard.Paste",
                            "R.Home.Clipboard.PasteKeepSource",
                            "R.Home.Clipboard.PasteMerge",
                            "R.Home.Clipboard.PasteText",
                            "R.Home.Clipboard.PastePicture",
                            "R.Home.Clipboard.PasteGallery",
                        ],
                    ),
                    Item::Stack(&["R.Home.Clipboard.Cut", "R.Home.Clipboard.CopyOneNote", "R.Home.Clipboard.FormatPainter"]),
                ],
            },
            Group {
                name: "Basic Text",
                items: &[Item::TwoRows(
                    &[
                        Item::Combo("R.Home.BasicText.Font", 120.0),
                        Item::Combo("R.Home.BasicText.FontSize", 44.0),
                        Item::SmallMenu("R.Home.BasicText.BulletsGallery", &[]),
                        Item::SmallMenu("R.Home.BasicText.NumberingGallery", &["R.Home.BasicText.NumberingCustomize"]),
                        Item::Row(&["R.Home.BasicText.OutdentClassic", "R.Home.BasicText.IndentClassic", "R.Home.BasicText.ClearFormatting"]),
                    ],
                    &[
                        Item::Row(&[
                            "R.Home.BasicText.Bold",
                            "R.Home.BasicText.Italic",
                            "R.Home.BasicText.Underline",
                            "R.Home.BasicText.Strikethrough",
                        ]),
                        Item::SmallMenu(
                            "R.Home.BasicText.ExtendedTextFormattingMenu",
                            &[
                                "R.Home.BasicText.ExtendedTextFormattingMostRecenltyUsed",
                                "R.Home.BasicText.Subscript",
                                "R.Home.BasicText.Superscript",
                            ],
                        ),
                        Item::SmallMenu("R.Home.BasicText.TextHighlightColorPicker", &["R.Home.BasicText.HighlightColorNone"]),
                        Item::SmallMenu("R.Home.BasicText.FontColorPicker", &[]),
                        Item::SmallMenu(
                            "R.Home.BasicText.ParagraphAlignmentMenu",
                            &[
                                "R.Home.BasicText.AlignLeft",
                                "R.Home.BasicText.AlignCenter",
                                "R.Home.BasicText.AlignRight",
                                "R.Home.BasicText.ParagraphSpacingOptions",
                                "R.Home.BasicText.TextDirectionLeftToRight",
                                "R.Home.BasicText.TextDirectionRightToLeft",
                            ],
                        ),
                        Item::Row(&["R.Home.BasicText.Delete"]),
                    ],
                )],
            },
            Group { name: "Styles", items: &[Item::Gallery("R.Home.Styles.QuickStylesGallery", STYLE_ITEMS, &["R.Home.Styles.ClearFormatting"])] },
            Group {
                name: "Tags",
                items: &[
                    Item::Gallery("R.Home.Tags.TagsGallery", TAG_ITEMS, &["R.Home.Tags.TagsCustomize", "R.Home.Tags.TagsRemoveAll"]),
                    Item::Column(&[
                        Item::Small("R.Home.Tags.ToDoTag"),
                        Item::Small("R.Home.Tags.FindTaggedNotes"),
                        Item::LabeledMenu(
                            "R.Home.Tags.OutlookTasksMenu",
                            &[
                                "R.Home.Tags.OutlookTaskTodayCtxItem",
                                "R.Home.Tags.OutlookTaskTomorrowCtxItem",
                                "R.Home.Tags.OutlookTaskThisWeekCtxItem",
                                "R.Home.Tags.OutlookTaskNextWeekCtxItem",
                                "R.Home.Tags.OutlookTaskLaterCtxItem",
                                "R.Home.Tags.OutlookTaskCustomDateCtxItem",
                                "R.Home.Tags.OutlookTaskMarkComplete",
                                "R.Home.Tags.OutlookTaskRemove",
                                "R.Home.Tags.OutlookTaskOpen",
                            ],
                        ),
                    ]),
                ],
            },
            Group { name: "Email", items: &[Item::Large("R.Home.Email.SendNotesViaEmail")] },
            Group {
                name: "Meetings",
                items: &[Item::LargeMenu(
                    "R.Home.Meetings.MeetingDetailsInsertGallery",
                    &["R.Home.Meetings.MeetingDetailsOtherDayInsert", "R.Home.Meetings.RefreshMeetingDetails"],
                )],
            },
            Group { name: "Voice", items: &[Item::Large("R.Home.Voice.Dictate")] },
        ],
    },
    Tab {
        name: "Insert",
        groups: &[
            Group { name: "Insert", items: &[Item::Large("R.Insert.Insert.PenInsertSpace")] },
            Group {
                name: "Tables",
                items: &[Item::LargeMenu(
                    "R.Insert.Tables.TableInsertGallery",
                    &["R.Insert.Tables.TableInsert", "R.Insert.Tables.InsertNewExcelSpreadsheetWithPreview"],
                )],
            },
            Group {
                name: "Files",
                items: &[
                    Item::Large("R.Insert.Files.PrintoutFromFileInsert"),
                    Item::Large("R.Insert.Files.FileAttachmentInsert"),
                    Item::LargeMenu(
                        "R.Insert.Files.InsertExcelSpreadsheetMenu",
                        &["R.Insert.Files.InsertExistingExcelSpreadsheet", "R.Insert.Files.InsertNewExcelSpreadsheetWithPreview"],
                    ),
                    Item::LargeMenu(
                        "R.Insert.Files.InsertVisioDrawingMenu",
                        &["R.Insert.Files.InsertExistingVisioDrawing", "R.Insert.Files.InsertNewVisioDrawingWithPreview"],
                    ),
                ],
            },
            Group {
                name: "Images",
                items: &[
                    Item::Large("R.Insert.Images.ScreenCaptureInsert"),
                    Item::Large("R.Insert.Images.PictureFromFileInsert"),
                    Item::Large("R.Insert.Images.ClipArtInsertDialog"),
                ],
            },
            Group {
                name: "Add-ins",
                items: &[
                    Item::Large("R.Insert.Addins.OfficeExtensionsAppStore"),
                    Item::LargeMenu(
                        "R.Insert.Addins.OfficeExtensionsGallery2",
                        &[
                            "R.Insert.Addins.OfficeExtensionsDialog",
                            "R.Insert.Addins.OfficeExtensionsFeaturedApp1",
                            "R.Insert.Addins.OfficeExtensionsFeaturedApp2",
                            "R.Insert.Addins.OfficeExtensionsFeaturedApp3",
                            "R.Insert.Addins.OfficeExtensionsFeaturedApp4",
                            "R.Insert.Addins.OfficeExtensionsFeaturedApp5",
                            "R.Insert.Addins.OfficeExtensionsFeaturedApp6",
                        ],
                    ),
                ],
            },
            Group { name: "Media", items: &[Item::Large("R.Insert.Media.InsertOnlineVideo")] },
            Group { name: "Links", items: &[Item::Large("R.Insert.Links.HyperlinkCreateHere")] },
            Group {
                name: "Recording",
                items: &[Item::Large("R.Insert.Recording.AudioRecordingInsert"), Item::Large("R.Insert.Recording.VideoRecordingInsert")],
            },
            Group {
                name: "Time Stamp",
                items: &[Item::Stack(&[
                    "R.Insert.TimeStamp.DateInsert",
                    "R.Insert.TimeStamp.TimeInsert",
                    "R.Insert.TimeStamp.DateAndTimeInsertOneNote",
                ])],
            },
            Group {
                name: "Pages",
                items: &[Item::LargeMenu(
                    "R.Insert.Pages.PageTemplatesSplitButton",
                    &[
                        "R.Insert.Pages.PageTemplatesTaskpane",
                        "R.Insert.Pages.StationeryMostRecentlyUsedList",
                        "R.Insert.Pages.NewPageFromStationery",
                    ],
                )],
            },
            Group {
                name: "Symbols",
                items: &[
                    Item::LargeMenu("R.Insert.Symbols.InsertBuildingBlocksEquationsGallery", &["R.Insert.Symbols.EquationInsertNew"]),
                    Item::LargeMenu("R.Insert.Symbols.SymbolInsertGallery", &["R.Insert.Symbols.SymbolsDialog"]),
                ],
            },
            Group { name: "Stickers", items: &[Item::Large("R.Insert.Stickers.Sticker")] },
        ],
    },
    Tab {
        name: "Draw",
        groups: &[
            Group {
                name: "Tools",
                items: &[
                    Item::Large("R.Draw.Tools.SelectAndTypeMode"),
                    Item::Large("R.Draw.Tools.LassoSelect"),
                    Item::Large("R.Draw.Tools.PanningHand"),
                    Item::LargeMenu(
                        "R.Draw.Tools.EraserMenu",
                        &[
                            "R.Draw.Tools.EraserLastUsed",
                            "R.Draw.Tools.SmallEraser",
                            "R.Draw.Tools.MediumEraser",
                            "R.Draw.Tools.LargeEraser",
                            "R.Draw.Tools.StrokeEraserOneNote",
                        ],
                    ),
                    Item::Gallery(
                        "R.Draw.Tools.InkStylesGallery",
                        &[],
                        &[
                            "R.Draw.Tools.InkBiasMenu",
                            "R.Draw.Tools.InkingBiasAuto",
                            "R.Draw.Tools.InkingBiasDraw",
                            "R.Draw.Tools.InkingBiasWrite",
                            "R.Draw.Tools.PenAsPointerMode",
                        ],
                    ),
                    Item::Large("R.Draw.Tools.AddPen"),
                    Item::Large("R.Draw.Tools.MoreColorAndThicknessOptions"),
                    Item::Large("R.Draw.Tools.FingerPaintingMode"),
                    Item::Large("R.Draw.Tools.Ruler"),
                ],
            },
            Group {
                name: "Shapes",
                items: &[Item::LargeMenu("R.Draw.Shapes.InkShapeGallery", &["R.Draw.Shapes.ShapesDuplicate", "R.Draw.Shapes.GridSnapTo"])],
            },
            Group {
                name: "Edit",
                items: &[
                    Item::Large("R.Draw.Edit.PenInsertSpace"),
                    Item::Large("R.Draw.Edit.Delete"),
                    Item::LargeMenu(
                        "R.Draw.Edit.ObjectsArrangeMenu",
                        &[
                            "R.Draw.Edit.ObjectBringForward",
                            "R.Draw.Edit.ObjectBringToFront",
                            "R.Draw.Edit.ObjectSendBackward",
                            "R.Draw.Edit.ObjectSendToBack",
                        ],
                    ),
                    Item::LargeMenu(
                        "R.Draw.Edit.RotateMenuOneNote",
                        &[
                            "R.Draw.Edit.RotateRight45OneNote",
                            "R.Draw.Edit.RotateLeft45OneNote",
                            "R.Draw.Edit.ObjectRotateRight90",
                            "R.Draw.Edit.ObjectRotateLeft90",
                            "R.Draw.Edit.ObjectFlipHorizontal",
                            "R.Draw.Edit.ObjectFlipVertical",
                        ],
                    ),
                ],
            },
            Group {
                name: "Convert",
                items: &[
                    Item::Large("R.Draw.Convert.InkToShape"),
                    Item::Large("R.Draw.Convert.SelectionToTextConvert"),
                    Item::Large("R.Draw.Convert.SelectionToMathConvert"),
                    Item::Large("R.Draw.Math.Math"),
                ],
            },
            Group { name: "Replay", items: &[Item::Large("R.Draw.Replay.InkReplay")] },
        ],
    },
    Tab {
        name: "History",
        groups: &[
            Group {
                name: "Unread",
                items: &[
                    Item::Large("R.History.Unread.GoToNextUnreadPage"),
                    Item::LargeMenu(
                        "R.History.Unread.UnreadMenu",
                        &[
                            "R.History.Unread.MarkPageAsReadContextual",
                            "R.History.Unread.MarkPageAsUnreadContextual",
                            "R.History.Unread.MarkAllPagesAsRead",
                            "R.History.Unread.UnreadNotebookChangesShow",
                        ],
                    ),
                ],
            },
            Group {
                name: "Authors",
                items: &[
                    Item::LargeMenu(
                        "R.History.Authors.FindByDateMenu",
                        &[
                            "R.History.Authors.FindToday",
                            "R.History.Authors.FindSinceYesterday",
                            "R.History.Authors.FindSinceSevenDays",
                            "R.History.Authors.FindLastTwoWeeks",
                            "R.History.Authors.FindLastMonth",
                            "R.History.Authors.FindLastThreeMonths",
                            "R.History.Authors.FindLasteSixMonths",
                            "R.History.Authors.FindAllChangesByDate",
                        ],
                    ),
                    Item::Large("R.History.Authors.FindUnread"),
                    Item::Large("R.History.Authors.AuthorHighlightingHide"),
                ],
            },
            Group {
                name: "History",
                items: &[
                    Item::LargeMenu(
                        "R.History.History.PageVersionSplitButton",
                        &[
                            "R.History.History.PageVersions",
                            "R.History.History.DeleteAllPageVersionsInSection",
                            "R.History.History.DeleteAllPageVersionsInFolder",
                            "R.History.History.DeleteAllPageVersionsInNotebook",
                            "R.History.History.DisableVersionHistory",
                        ],
                    ),
                    Item::LargeMenu(
                        "R.History.History.RecycleBinMenu",
                        &["R.History.History.RecycleBin", "R.History.History.EmptyRecycleBin", "R.History.History.DisableVersionHistory2"],
                    ),
                ],
            },
        ],
    },
    Tab {
        name: "Review",
        groups: &[
            Group {
                name: "Proofing",
                items: &[
                    Item::Large("R.Review.Proofing.SpellCheck"),
                    Item::Large("R.Review.Proofing.ResearchPane"),
                    Item::Large("R.Review.Proofing.Thesaurus"),
                ],
            },
            Group { name: "Accessibility", items: &[Item::Large("R.Review.Accessibility.AccessibilityChecker")] },
            Group {
                name: "Language",
                items: &[
                    Item::LargeMenu(
                        "R.Review.Language.TranslateMenu",
                        &["R.Review.Language.TranslateSelected", "R.Review.Language.MiniTranslator", "R.Review.Language.TranslationLanguageOptions"],
                    ),
                    Item::LargeMenu(
                        "R.Review.Language.LanguageCommands",
                        &["R.Review.Language.SetLanguage", "R.Review.Language.LanguagePreferences"],
                    ),
                ],
            },
            Group { name: "Section", items: &[Item::Large("R.Review.Section.PasswordProtectSection")] },
            Group { name: "Notes", items: &[Item::Large("R.Review.Notes.LinkedNotes")] },
        ],
    },
    Tab {
        name: "View",
        groups: &[
            Group {
                name: "Views",
                items: &[
                    Item::Large("R.View.Views.NormalViewOneNote"),
                    Item::Large("R.View.Views.FullPageView"),
                    Item::Large("R.View.Views.CurrentWindowDock"),
                    Item::Large("R.View.Views.SwitchBackground"),
                    Item::Large("R.View.Views.NavigationPanes"),
                ],
            },
            Group { name: "Immersive", items: &[Item::Large("R.View.Immersive.ImmersiveReader")] },
            Group {
                name: "Page Setup",
                items: &[
                    Item::LargeMenu("R.View.PageSetup.PageColorGallery", &[]),
                    Item::LargeMenu(
                        "R.View.PageSetup.RuleLinesMenu",
                        &["R.View.PageSetup.RuleLineColorMenu", "R.View.PageSetup.RuleLineColorGallery", "R.View.PageSetup.NewPagesWithRuleLines"],
                    ),
                    Item::Large("R.View.PageSetup.PageTitleHide"),
                    Item::Large("R.View.PageSetup.PaperSize"),
                ],
            },
            Group {
                name: "Zoom",
                items: &[
                    Item::Large("R.View.Zoom.ZoomOut"),
                    Item::Large("R.View.Zoom.ZoomIn"),
                    Item::Combo("R.View.Zoom.ZoomClassic", 64.0),
                    Item::Stack(&["R.View.Zoom.ZoomCurrent100", "R.View.Zoom.ZoomPageWidth"]),
                ],
            },
            Group {
                name: "Window",
                items: &[
                    Item::Large("R.View.Window.WindowNew"),
                    Item::Large("R.View.Window.NewDockedWindow"),
                    Item::Large("R.View.Window.NewSideNote"),
                    Item::Large("R.View.Window.WindowKeepOnTop"),
                ],
            },
        ],
    },
    Tab {
        name: "Help",
        groups: &[Group {
            name: "Help",
            items: &[
                Item::Large("R.Help.Help.Help"),
                Item::Large("R.Help.Help.ContactSupport"),
                Item::Large("R.Help.Help.Feedback"),
                Item::Large("R.Help.Help.ShowTraining"),
                Item::Large("R.Help.Help.WhatsNew"),
            ],
        }],
    },
];

/// Contextual tabs (shown only for the matching selection; M2 brings their content).
pub const CONTEXTUAL: &[Tab] = &[
    Tab {
        name: "Table Layout",
        groups: &[
            Group {
                name: "Select",
                items: &[
                    Item::Large("CT.TableLayout.Select.SelectTableOneNote"),
                    Item::Stack(&["CT.TableLayout.Select.SelectColumns", "CT.TableLayout.Select.SelectRows", "CT.TableLayout.Select.SelectCell"]),
                ],
            },
            Group {
                name: "Delete",
                items: &[
                    Item::Large("CT.TableLayout.Delete.DeleteTableOneNote"),
                    Item::Stack(&["CT.TableLayout.Delete.DeleteColumnsOneNote", "CT.TableLayout.Delete.DeleteRowsOneNote"]),
                ],
            },
            Group {
                name: "Insert",
                items: &[
                    Item::Large("CT.TableLayout.Insert.TableInsertRowsAbove"),
                    Item::Large("CT.TableLayout.Insert.TableInsertRowsBelow"),
                    Item::Large("CT.TableLayout.Insert.TableInsertColumnsLeft"),
                    Item::Large("CT.TableLayout.Insert.TableInsertColumnsRight"),
                ],
            },
            Group { name: "Merge", items: &[Item::Large("CT.TableLayout.Merge.MergeCells"), Item::Large("CT.TableLayout.Merge.UnmergeCells")] },
            Group {
                name: "Format",
                items: &[Item::Large("CT.TableLayout.Format.TableBordersHide"), Item::LargeMenu("CT.TableLayout.Format.ShadingColorPicker", &[])],
            },
            Group {
                name: "Alignment",
                items: &[Item::Stack(&[
                    "CT.TableLayout.Alignment.AlignLeft",
                    "CT.TableLayout.Alignment.AlignCenter",
                    "CT.TableLayout.Alignment.AlignRight",
                ])],
            },
            Group {
                name: "Data",
                items: &[Item::LargeMenu(
                    "CT.TableLayout.Data.SortMenu",
                    &[
                        "CT.TableLayout.Data.SortUp",
                        "CT.TableLayout.Data.SortDown",
                        "CT.TableLayout.Data.ShowSortTableRowsDialog",
                        "CT.TableLayout.Data.HeaderRowNoFormatting",
                    ],
                )],
            },
            Group { name: "Convert", items: &[Item::Large("CT.TableLayout.Convert.ConvertToExcelSpreadsheet")] },
        ],
    },
    Tab {
        name: "Equation",
        groups: &[
            Group {
                name: "Tools",
                items: &[
                    Item::LargeMenu("CT.Equation.Tools.EquationInsertGallery", &["CT.Equation.Tools.InkEquation"]),
                    Item::Stack(&[
                        "CT.Equation.Tools.EquationProfessional",
                        "CT.Equation.Tools.EquationLinearFormat",
                        "CT.Equation.Tools.EquationNormalText",
                    ]),
                    Item::Row(&["CT.Equation.Tools.EquationOptions"]),
                ],
            },
            Group { name: "Symbols", items: &[Item::Gallery("CT.Equation.Symbols.EquationSymbolsInsertGallery", &[], &[])] },
            Group {
                name: "Structures",
                items: &[
                    Item::LargeMenu("CT.Equation.Structures.EquationFractionGallery", &[]),
                    Item::LargeMenu("CT.Equation.Structures.EquationScriptGallery", &[]),
                    Item::LargeMenu("CT.Equation.Structures.EquationRadicalGallery", &[]),
                    Item::LargeMenu("CT.Equation.Structures.EquationIntegralGallery", &[]),
                    Item::LargeMenu("CT.Equation.Structures.EquationLargeOperatorGallery", &[]),
                    Item::Stack(&[
                        "CT.Equation.Structures.EquationDelimiterGallery",
                        "CT.Equation.Structures.EquationFunctionGallery",
                        "CT.Equation.Structures.EquationAccentGallery",
                    ]),
                    Item::Stack(&[
                        "CT.Equation.Structures.EquationLimitGallery",
                        "CT.Equation.Structures.EquationOperatorGallery",
                        "CT.Equation.Structures.EquationMatrixGallery",
                    ]),
                ],
            },
        ],
    },
    Tab {
        name: "Audio & Video",
        groups: &[
            Group {
                name: "Recording",
                items: &[Item::Large("CT.AudioVideo.Recording.AudioRecordingInsert"), Item::Large("CT.AudioVideo.Recording.VideoRecordingInsert")],
            },
            Group {
                name: "Playback",
                items: &[
                    Item::Large("CT.AudioVideo.Playback.RecordingPlay"),
                    Item::Large("CT.AudioVideo.Playback.RecordingPause"),
                    Item::Large("CT.AudioVideo.Playback.RecordingStop"),
                    Item::Stack(&["CT.AudioVideo.Playback.RewindLong", "CT.AudioVideo.Playback.RewindShort", "CT.AudioVideo.Playback.TotalTime"]),
                    Item::Stack(&["CT.AudioVideo.Playback.FastForwardShort", "CT.AudioVideo.Playback.FastForwardLong"]),
                ],
            },
            Group {
                name: "Options",
                items: &[Item::Large("CT.AudioVideo.Options.AudioNotePlayback"), Item::Large("CT.AudioVideo.Options.AudioVideoSettings")],
            },
        ],
    },
];

/// Every catalog id an item refers to (the control and its drop-down children).
pub fn ids(item: &Item) -> Vec<&'static str> {
    match *item {
        Item::Large(id) | Item::Combo(id, _) => vec![id],
        Item::LargeMenu(id, kids) | Item::SmallMenu(id, kids) => std::iter::once(id).chain(kids.iter().copied()).collect(),
        Item::Stack(ids) | Item::Row(ids) => ids.to_vec(),
        Item::Gallery(id, items, extra) => std::iter::once(id).chain(items.iter().copied()).chain(extra.iter().copied()).collect(),
        Item::TwoRows(a, b) => a.iter().chain(b.iter()).flat_map(ids).collect(),
        Item::Column(items) => items.iter().flat_map(ids).collect(),
        Item::Small(id) => vec![id],
        Item::LabeledMenu(id, kids) => std::iter::once(id).chain(kids.iter().copied()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_covers_every_ribbon_row() {
        // parity: UI.Ribbon.Tabs
        let rows = notecraft_engine::catalog::rows();
        let mut placed: Vec<&str> = TABS.iter().chain(CONTEXTUAL).flat_map(|t| t.groups.iter()).flat_map(|g| g.items.iter()).flat_map(ids).collect();
        let n = placed.len();
        placed.sort_unstable();
        placed.dedup();
        assert_eq!(placed.len(), n, "a control is placed twice");
        for id in &placed {
            assert!(rows.iter().any(|r| r.id == *id), "{id} is not a catalog row");
        }
        // Every ribbon row of the catalog is placed, except the touch-mode duplicates of the
        // Draw › Tools group (the same commands, laid out for touch).
        for r in rows.iter().filter(|r| (r.id.starts_with("R.") || r.id.starts_with("CT.")) && !r.id.starts_with("R.Draw.Toolstouch.")) {
            assert!(placed.contains(&r.id.as_str()), "catalog ribbon row {} is not placed in the ribbon", r.id);
        }
        let names: Vec<&str> = TABS.iter().map(|t| t.name).collect();
        assert_eq!(names, vec!["Home", "Insert", "Draw", "History", "Review", "View", "Help"], "the reference's tab order");
    }
}
