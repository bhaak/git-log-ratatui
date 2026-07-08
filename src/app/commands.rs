use crate::view::Panel;

/// Commands produced by panel event handlers (keyboard and mouse).
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
    /// Mouse: initiate a vertical resize drag.
    InitiateDragVertical,
    /// Mouse: initiate a horizontal resize drag.
    InitiateDragHorizontal,
    /// Mouse: end any drag operation.
    EndDrag,
    /// Mouse: start scrollbar dragging on a panel.
    InitiateScrollbarDrag(Panel),
    /// Mouse: end scrollbar dragging.
    EndScrollbarDrag,
    /// Set branch column width percentage.
    SetBranchWidthPct(u16),
    /// Set diff panel height percentage.
    SetDiffHeightPct(u16),
    /// Select a commit by its absolute index.
    SelectCommitIndex(usize),
    /// Mouse: select a branch item by index and trigger the appropriate action.
    MouseClickBranch {
        index: usize,
        full_path: String,
        is_branch: bool,
        is_expandable: bool,
        is_expanded: bool,
        key: String,
    },
    /// Set absolute diff scroll position (for scrollbar dragging).
    ScrollToAbsolute(usize),
    /// Quit the application.
    Quit,
}
