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
                vec![Effect::SetDirty]
            }
            Command::MoveUp => {
                self.selected_file_index =
                    cycle_backward(self.selected_file_index, self.file_entries.len());
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
