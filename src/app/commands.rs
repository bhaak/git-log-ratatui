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
    /// Focus the search panel and clear the query (bound to `/`).
    FocusSearchClear,
    /// Go back to the previous panel via LIFO focus stack (bound to `Esc`).
    GoBack,
    /// Preview diff for selected commit without moving focus (bound to `Space` in commits).
    PreviewDiff,
    /// Branch panel: jump to next (1) or previous (-1) sibling at the same tree depth.
    JumpToSibling(i32),
    /// Toggle the help modal overlay (bound to `?`).
    ToggleHelp,
    /// Quit the application.
    Quit,
}

/// Side effects produced by command handlers, processed by the app dispatcher.
/// These encapsulate operations that require App-level access (workers, cross-state).
#[derive(Debug)]
pub(crate) enum Effect {
    /// Request the branch worker to fetch branches.
    RequestBranches,
    /// Request the commit worker to fetch commits for an optional branch.
    RequestCommits(Option<String>),
    /// Toggle between full and simplified graph mode.
    ToggleGraph,
    /// Copy the hash of the currently selected commit (short or full).
    CopySelectedHash { short: bool },
    /// Rebuild the branch tree from current branch data.
    RebuildBranchTree,
    /// Request diff for a specific commit/stash OID.
    RequestDiff(String),
    /// Apply the current search query as a filter on commit data.
    ApplySearchFilter,
    /// Mark the UI as dirty (needs re-render).
    SetDirty,
}
