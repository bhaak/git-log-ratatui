use ratatui::style::Color;

/// Customizable color theme for the application.
#[derive(Clone)]
pub struct Theme {
    /// Focused panel border color (overall app accent).
    pub focused_border: Color,
    /// Unfocused panel border color.
    pub unfocused_border: Color,
    /// Selected item background (lists, tables).
    pub selected_bg: Color,
    /// Unselected item background (lists, tables).
    pub unselected_bg: Color,
    /// Commit graph lane colors (8 colors distributed across hue spectrum).
    pub graph_colors: [Color; 8],
    /// Merge commit marker color.
    pub commit_merge: Color,
    /// Default commit text color.
    pub commit_default: Color,
    /// Second commit text color.
    pub commit_secondary: Color,
    /// Decoration tag color (tags like v1.0).
    pub decoration_tag: Color,
    /// Decoration local branch color.
    pub decoration_local: Color,
    /// Decoration remote branch color.
    pub decoration_remote: Color,
    /// Decoration HEAD color.
    pub decoration_head: Color,
    /// Diff added lines color.
    pub diff_added: Color,
    /// Diff added word-highlight background.
    pub diff_added_bg: Color,
    /// Diff removed lines color.
    pub diff_removed: Color,
    /// Diff removed word-highlight background.
    pub diff_removed_bg: Color,
    /// Diff modified file indicator color.
    pub diff_modified: Color,
    /// Diff renamed file indicator color.
    pub diff_renamed: Color,
    /// Diff hunk header color.
    pub diff_hunk_header: Color,
    /// Diff file header color.
    pub diff_file_header: Color,
    /// Diff context line color (unchanged lines).
    pub diff_context: Color,
    /// Diff selected file background.
    pub diff_selected_file_bg: Color,
    /// Diff selected file text.
    pub diff_selected_file_fg: Color,
    /// Diff selected file border.
    pub diff_selected_file_border: Color,
    /// Search input cursor foreground color.
    pub search_cursor_fg: Color,
    /// Search input cursor background color.
    pub search_cursor_bg: Color,
    /// Help bar title color.
    pub help_title: Color,
    /// Scrollbar thumb (active indicator) color.
    pub scrollbar_thumb: Color,
    /// Scrollbar track (background) color.
    #[allow(dead_code)]
    pub scrollbar_track: Color,
    /// Scope indicator text color.
    pub scope_text: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            focused_border: Color::Rgb(180, 140, 255),
            unfocused_border: Color::Gray,
            selected_bg: Color::Rgb(80, 60, 120),
            unselected_bg: Color::DarkGray,
            graph_colors: [
                Color::Rgb(0xCC, 0x66, 0x4D),
                Color::Rgb(0xCC, 0x99, 0x4D),
                Color::Rgb(0xCC, 0xCC, 0x4D),
                Color::Rgb(0x4D, 0xCC, 0x66),
                Color::Rgb(0x4D, 0xCC, 0xCC),
                Color::Rgb(0x4D, 0x99, 0xCC),
                Color::Rgb(0x99, 0x4D, 0xCC),
                Color::Rgb(0xCC, 0x4D, 0x99),
            ],
            commit_merge: Color::Yellow,
            commit_default: Color::White,
            commit_secondary: Color::DarkGray,
            decoration_tag: Color::Yellow,
            decoration_local: Color::Green,
            decoration_remote: Color::Red,
            decoration_head: Color::LightGreen,
            diff_added: Color::Green,
            diff_added_bg: Color::Rgb(0, 50, 0),
            diff_removed: Color::Red,
            diff_removed_bg: Color::Rgb(50, 0, 0),
            diff_modified: Color::Yellow,
            diff_renamed: Color::Blue,
            diff_hunk_header: Color::Cyan,
            diff_file_header: Color::Yellow,
            diff_context: Color::Gray,
            diff_selected_file_bg: Color::White,
            diff_selected_file_fg: Color::Black,
            diff_selected_file_border: Color::Rgb(100, 150, 255),
            search_cursor_fg: Color::Black,
            search_cursor_bg: Color::Rgb(180, 140, 255),
            help_title: Color::Yellow,
            scrollbar_thumb: Color::Rgb(200, 150, 100),
            scrollbar_track: Color::DarkGray,
            scope_text: Color::Yellow,
        }
    }
}
