use ratatui::widgets::TableState;

use crate::app::cache::CommitCache;
use crate::app::commands::{Command, Effect};
use crate::ui;
use crate::view::CommitRow as Commit;

/// Commit table state — all commit data, filtering, selection, graph mode, lazy loading, and scrollbar.
pub struct CommitTableState {
    pub all_commits: Vec<Commit>,
    pub filtered_commits: Option<Vec<Commit>>,
    pub selected_index: usize,
    pub visible_to_commit: Vec<usize>,
    pub table_state: TableState,
    pub simplified_graph: bool,
    pub cache: CommitCache,
    pub commits_loaded: bool,
    pub commit_limit: usize,
    pub all_commits_loaded: bool,
    pub loading_more: bool,
    /// Whether to derive hash column colors from commit hash hex digits.
    pub hash_color_enabled: bool,
    /// Scrollbar for the commit table.
    pub scrollbar: ui::scrollbar_view::ScrollbarView,
}

impl CommitTableState {
    pub fn new(simplified_graph: bool, commit_limit: usize, hash_color_enabled: bool) -> Self {
        CommitTableState {
            all_commits: Vec::new(),
            filtered_commits: None,
            selected_index: 0,
            visible_to_commit: Vec::new(),
            table_state: TableState::default(),
            simplified_graph,
            cache: CommitCache::new(),
            commits_loaded: false,
            commit_limit,
            all_commits_loaded: false,
            loading_more: false,
            hash_color_enabled,
            scrollbar: ui::scrollbar_view::ScrollbarView::new(),
        }
    }

    /// Handle commit-table commands that require only local state.
    /// Navigation commands are handled by the dispatcher due to cross-state `visible_count`.
    pub(crate) fn handle_command(&mut self, cmd: &Command) -> Vec<Effect> {
        match cmd {
            Command::CopyHashShort => vec![Effect::CopySelectedHash { short: true }],
            Command::CopyHashFull => vec![Effect::CopySelectedHash { short: false }],
            Command::SelectCommitIndex(idx) => {
                self.selected_index = *idx;
                vec![Effect::SetDirty]
            }
            Command::ShowCommitDiff => vec![Effect::SetDirty],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> CommitTableState {
        CommitTableState::new(false, 500, false)
    }

    #[test]
    fn test_copy_hash_short_returns_effect() {
        let mut state = make_state();
        let effects = state.handle_command(&Command::CopyHashShort);
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            Effect::CopySelectedHash { short: true }
        ));
    }

    #[test]
    fn test_copy_hash_full_returns_effect() {
        let mut state = make_state();
        let effects = state.handle_command(&Command::CopyHashFull);
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            Effect::CopySelectedHash { short: false }
        ));
    }

    #[test]
    fn test_select_commit_index() {
        let mut state = make_state();
        state.selected_index = 0;
        let effects = state.handle_command(&Command::SelectCommitIndex(5));
        assert_eq!(state.selected_index, 5);
        assert!(effects.iter().any(|e| matches!(e, Effect::SetDirty)));
    }

    #[test]
    fn test_show_commit_diff_returns_set_dirty() {
        let mut state = make_state();
        let effects = state.handle_command(&Command::ShowCommitDiff);
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::SetDirty));
    }

    #[test]
    fn test_unknown_command_returns_empty_effects() {
        let mut state = make_state();
        let effects = state.handle_command(&Command::MoveUp);
        assert!(effects.is_empty());
    }
}
