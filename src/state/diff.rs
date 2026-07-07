use crate::models::*;
use crate::ui;

/// Diff panel state — commit metadata, diff lines, file listing, scroll position, and scrollbar.
pub struct DiffState {
    pub commit_info: Option<CommitInfo>,
    pub diff_lines: Vec<String>,
    pub file_entries: Vec<FileEntry>,
    pub selected_file_index: usize,
    pub diff_scroll: usize,
    pub last_selected_hash: Option<String>,
    pub diff_pending: bool,
    /// Scrollbar for the diff panel.
    pub scrollbar: ui::scrollbar_view::ScrollbarView,
}

impl DiffState {
    pub fn new() -> Self {
        DiffState {
            commit_info: None,
            diff_lines: Vec::new(),
            file_entries: Vec::new(),
            selected_file_index: 0,
            diff_scroll: 0,
            last_selected_hash: None,
            diff_pending: false,
            scrollbar: ui::scrollbar_view::ScrollbarView::new(),
        }
    }
}

impl Default for DiffState {
    fn default() -> Self {
        Self::new()
    }
}
