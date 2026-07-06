use crate::models::Panel;

/// Commands produced by panel event handlers.
/// The app-level dispatcher executes these on AppState.
#[derive(Debug, Clone)]
pub enum Command {
    /// Set search query and cursor position.
    SetSearch(String, usize),
    /// Clear the search query.
    ClearSearch,
    /// Navigation within current panel.
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    /// Focus management.
    SetFocus(Panel),
    FocusNext,
    FocusPrev,
    /// Branch panel: select a branch (triggers commit load).
    SelectBranch(String),
    /// Branch panel: expand/collapse a node.
    ToggleBranchNode {
        key: String,
        expanded: bool,
    },
    /// Cycle branch scope (All → Local → Remote).
    CycleScope,
    /// Toggle simplified graph mode.
    ToggleGraph,
    /// Copy short hash of selected commit.
    CopyHashShort,
    /// Copy full hash of selected commit.
    CopyHashFull,
    /// Paste clipboard text into search.
    PasteSearch(String),
    /// Diff: scroll up/down by delta.
    ScrollDiff(i32),
    /// Diff: jump to a specific file's diff section.
    JumpToDiffFile(usize),
    /// Diff: select next file.
    SelectNextFile,
    /// Diff: select previous file.
    SelectPrevFile,
    /// Diff: jump to top.
    JumpToTop,
    /// Diff: jump to bottom.
    JumpToBottom,
    /// Switch focus to diff panel for the selected commit.
    ShowCommitDiff,
    /// Quit the application.
    Quit,
}
