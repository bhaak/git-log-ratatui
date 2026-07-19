use crate::app::commands::{Command, Effect};
use crate::domain::{CommitInfo, FileEntry};
use crate::ui;
use crate::ui::diff_panel;

/// Diff panel state — commit metadata, diff lines, file listing, scroll position, and scrollbar.
pub struct DiffState {
    pub commit_info: Option<CommitInfo>,
    pub diff_lines: Vec<String>,
    pub file_entries: Vec<FileEntry>,
    pub selected_file_index: usize,
    pub diff_scroll: usize,
    pub last_selected_hash: Option<String>,
    pub diff_pending: bool,
    /// Total display lines from the previous render, used to clamp scroll off-by-one.
    pub prev_total_lines: usize,
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
            prev_total_lines: 0,
            scrollbar: ui::scrollbar_view::ScrollbarView::new(),
        }
    }

    /// Handle diff-panel commands. Returns effects for cross-state operations.
    pub(crate) fn handle_command(&mut self, cmd: &Command) -> Vec<Effect> {
        match cmd {
            Command::ScrollDiff(delta) => {
                if *delta > 0 {
                    self.diff_scroll = self.diff_scroll.saturating_add(*delta as usize);
                } else {
                    self.diff_scroll = self.diff_scroll.saturating_sub((-delta) as usize);
                }
                vec![Effect::SetDirty]
            }
            Command::JumpToDiffFile(index) => {
                let offset =
                    diff_panel::diff_line_offset(self.commit_info.as_ref(), &self.file_entries);
                if let Some(entry) = self.file_entries.get(*index) {
                    self.diff_scroll = entry.diff_line + offset;
                }
                vec![Effect::SetDirty]
            }
            Command::SelectNextFile => {
                if !self.file_entries.is_empty() {
                    self.selected_file_index =
                        (self.selected_file_index + 1) % self.file_entries.len();
                    let offset =
                        diff_panel::diff_line_offset(self.commit_info.as_ref(), &self.file_entries);
                    if let Some(entry) = self.file_entries.get(self.selected_file_index) {
                        self.diff_scroll = entry.diff_line + offset;
                    }
                }
                vec![Effect::SetDirty]
            }
            Command::SelectPrevFile => {
                if !self.file_entries.is_empty() {
                    if self.selected_file_index > 0 {
                        self.selected_file_index -= 1;
                        let offset = diff_panel::diff_line_offset(
                            self.commit_info.as_ref(),
                            &self.file_entries,
                        );
                        if let Some(entry) = self.file_entries.get(self.selected_file_index) {
                            self.diff_scroll = entry.diff_line + offset;
                        }
                    } else {
                        self.diff_scroll = 0;
                        self.selected_file_index = 0;
                    }
                }
                vec![Effect::SetDirty]
            }
            Command::MoveDown => {
                self.selected_file_index =
                    cycle_forward(self.selected_file_index, self.file_entries.len());
                ensure_entry_visible(
                    self.selected_file_index,
                    &self.file_entries,
                    &mut self.diff_scroll,
                    self.scrollbar.viewport_length(),
                    self.commit_info.as_ref(),
                );
                vec![Effect::SetDirty]
            }
            Command::MoveUp => {
                self.selected_file_index =
                    cycle_backward(self.selected_file_index, self.file_entries.len());
                ensure_entry_visible(
                    self.selected_file_index,
                    &self.file_entries,
                    &mut self.diff_scroll,
                    self.scrollbar.viewport_length(),
                    self.commit_info.as_ref(),
                );
                vec![Effect::SetDirty]
            }
            Command::PageUp => {
                let page = self.scrollbar.viewport_length().max(1);
                self.diff_scroll = self.diff_scroll.saturating_sub(page);
                vec![Effect::SetDirty]
            }
            Command::PageDown => {
                let page = self.scrollbar.viewport_length().max(1);
                self.diff_scroll = self.diff_scroll.saturating_add(page);
                vec![Effect::SetDirty]
            }
            Command::JumpToTop => {
                self.diff_scroll = 0;
                vec![Effect::SetDirty]
            }
            Command::JumpToBottom => {
                self.diff_scroll = usize::MAX;
                vec![Effect::SetDirty]
            }
            Command::ScrollToAbsolute(pos) => {
                self.diff_scroll = *pos;
                vec![Effect::SetDirty]
            }
            _ => vec![],
        }
    }
}

impl Default for DiffState {
    fn default() -> Self {
        Self::new()
    }
}

fn cycle_forward(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (current + 1) % len
    }
}

fn cycle_backward(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else if current > 0 {
        current - 1
    } else {
        len - 1
    }
}

/// Adjust `diff_scroll` so that the selected file entry is visible within the viewport.
fn ensure_entry_visible(
    selected_index: usize,
    file_entries: &[FileEntry],
    diff_scroll: &mut usize,
    viewport_length: usize,
    commit_info: Option<&CommitInfo>,
) {
    if file_entries.is_empty() || viewport_length == 0 {
        return;
    }

    let file_section_end = diff_panel::diff_line_offset(commit_info, file_entries);
    let metadata_lines = file_section_end.saturating_sub(2 + file_entries.len());
    let entry_line = metadata_lines + 1 + selected_index;

    if entry_line < *diff_scroll {
        // Scroll up to place the entry one line below the viewport top,
        // keeping the preceding entry visible as context.
        *diff_scroll = entry_line.saturating_sub(1);
    } else if entry_line >= diff_scroll.saturating_add(viewport_length) {
        // Scroll down to place the entry one line above the viewport bottom,
        // leaving room for the following entry.
        *diff_scroll = entry_line.saturating_sub(viewport_length.saturating_sub(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::FileEntry;

    fn fixture_file_entries() -> Vec<FileEntry> {
        vec![
            FileEntry {
                name: "a.rs".into(),
                diff_line: 5,
                status: '~',
                old_name: None,
                lines_added: 10,
                lines_removed: 3,
            },
            FileEntry {
                name: "b.rs".into(),
                diff_line: 20,
                status: '+',
                old_name: None,
                lines_added: 42,
                lines_removed: 0,
            },
            FileEntry {
                name: "c.rs".into(),
                diff_line: 40,
                status: '-',
                old_name: None,
                lines_added: 0,
                lines_removed: 7,
            },
        ]
    }

    #[test]
    fn test_scroll_diff_positive() {
        let mut state = DiffState::new();
        state.diff_scroll = 10;
        state.handle_command(&Command::ScrollDiff(5));
        assert_eq!(state.diff_scroll, 15);
    }

    #[test]
    fn test_scroll_diff_negative() {
        let mut state = DiffState::new();
        state.diff_scroll = 10;
        state.handle_command(&Command::ScrollDiff(-3));
        assert_eq!(state.diff_scroll, 7);
    }

    #[test]
    fn test_scroll_diff_no_underflow() {
        let mut state = DiffState::new();
        state.diff_scroll = 2;
        state.handle_command(&Command::ScrollDiff(-10));
        assert_eq!(state.diff_scroll, 0);
    }

    #[test]
    fn test_jump_to_diff_file() {
        let mut state = DiffState::new();
        state.file_entries = fixture_file_entries();
        state.handle_command(&Command::JumpToDiffFile(1));
        // b.rs starts at diff_line 20, offset from metadata adds some lines
        assert!(state.diff_scroll >= 20);
    }

    #[test]
    fn test_select_next_file_wraps() {
        let mut state = DiffState::new();
        state.file_entries = fixture_file_entries();
        state.selected_file_index = 2;
        state.handle_command(&Command::SelectNextFile);
        assert_eq!(state.selected_file_index, 0);
    }

    #[test]
    fn test_select_prev_file_from_zero_goes_to_zero_scroll() {
        let mut state = DiffState::new();
        state.file_entries = fixture_file_entries();
        state.selected_file_index = 0;
        state.diff_scroll = 50;
        state.handle_command(&Command::SelectPrevFile);
        assert_eq!(state.selected_file_index, 0);
        assert_eq!(state.diff_scroll, 0);
    }

    #[test]
    fn test_select_prev_file_from_nonzero() {
        let mut state = DiffState::new();
        state.file_entries = fixture_file_entries();
        state.selected_file_index = 1;
        state.handle_command(&Command::SelectPrevFile);
        assert_eq!(state.selected_file_index, 0);
    }

    #[test]
    fn test_move_down_in_diff_navigates_files() {
        let mut state = DiffState::new();
        state.file_entries = fixture_file_entries();
        state.selected_file_index = 0;
        state.handle_command(&Command::MoveDown);
        assert_eq!(state.selected_file_index, 1);
    }

    #[test]
    fn test_move_up_in_diff_wraps() {
        let mut state = DiffState::new();
        state.file_entries = fixture_file_entries();
        state.selected_file_index = 0;
        state.handle_command(&Command::MoveUp);
        assert_eq!(state.selected_file_index, 2);
    }

    #[test]
    fn test_page_up_scrolls_by_viewport() {
        let mut state = DiffState::new();
        state.diff_scroll = 50;
        state.handle_command(&Command::PageUp);
        assert!(state.diff_scroll < 50);
    }

    #[test]
    fn test_page_down_scrolls_forward() {
        let mut state = DiffState::new();
        state.diff_scroll = 10;
        state.handle_command(&Command::PageDown);
        assert!(state.diff_scroll > 10);
    }

    #[test]
    fn test_scroll_to_absolute() {
        let mut state = DiffState::new();
        state.handle_command(&Command::ScrollToAbsolute(42));
        assert_eq!(state.diff_scroll, 42);
    }

    #[test]
    fn test_jump_to_top_diff() {
        let mut state = DiffState::new();
        state.diff_scroll = 100;
        state.handle_command(&Command::JumpToTop);
        assert_eq!(state.diff_scroll, 0);
    }

    #[test]
    fn test_jump_to_bottom_diff_sets_max() {
        let mut state = DiffState::new();
        state.handle_command(&Command::JumpToBottom);
        assert_eq!(state.diff_scroll, usize::MAX);
    }

    #[test]
    fn test_select_next_file_empty_does_nothing() {
        let mut state = DiffState::new();
        state.selected_file_index = 5;
        state.handle_command(&Command::SelectNextFile);
        assert_eq!(state.selected_file_index, 5);
    }

    #[test]
    fn test_unknown_command_returns_empty_effects() {
        let mut state = DiffState::new();
        let effects = state.handle_command(&Command::CopyHashShort);
        assert!(effects.is_empty());
    }
}
