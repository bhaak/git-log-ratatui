use std::collections::BTreeMap;

use crate::app::commands::{Command, Effect};
use crate::app::PAGE_SIZE;
use crate::domain::{BranchData, BranchScope};
use crate::ui;
use crate::view::TreeItem;

/// Branch panel state — tree, selection, scope, expanded node tracking, and scrollbar.
pub struct BranchState {
    pub all_branches: BranchData,
    pub branch_tree: Vec<TreeItem>,
    pub expanded_nodes: BTreeMap<String, bool>,
    pub branch_index: usize,
    pub branch_scope: BranchScope,
    pub selected_branch: Option<String>,
    pub branch_list_offset: usize,
    pub branches_loaded: bool,
    /// Whether to show staleness greyscale on branch and tag names.
    pub branch_staleness_enabled: bool,
    /// Scrollbar for the branch tree list.
    pub scrollbar: ui::scrollbar_view::ScrollbarView,
}

impl BranchState {
    pub fn new() -> Self {
        BranchState {
            all_branches: BranchData {
                default_branch: None,
                entries: Vec::new(),
                tags: Vec::new(),
                tags_dates: Vec::new(),
            },
            branch_tree: Vec::new(),
            expanded_nodes: BTreeMap::new(),
            branch_index: 0,
            branch_scope: BranchScope::All,
            selected_branch: None,
            branch_list_offset: 0,
            branches_loaded: false,
            branch_staleness_enabled: true,
            scrollbar: ui::scrollbar_view::ScrollbarView::new(),
        }
    }

    /// Handle branch-panel commands that require only local state.
    /// Multi-state commands (CycleScope, SelectBranch, MouseClickBranch) are handled
    /// by the dispatcher.
    pub(crate) fn handle_command(&mut self, cmd: &Command) -> Vec<Effect> {
        match cmd {
            Command::MoveUp => {
                self.branch_index =
                    branch_cycle_backward(self.branch_index, self.branch_tree.len());
                vec![Effect::SetDirty]
            }
            Command::MoveDown => {
                self.branch_index = branch_cycle_forward(self.branch_index, self.branch_tree.len());
                vec![Effect::SetDirty]
            }
            Command::PageUp => {
                self.branch_index = self.branch_index.saturating_sub(PAGE_SIZE);
                vec![Effect::SetDirty]
            }
            Command::PageDown => {
                self.branch_index =
                    (self.branch_index + PAGE_SIZE).min(self.branch_tree.len().saturating_sub(1));
                vec![Effect::SetDirty]
            }
            Command::JumpToTop => {
                self.branch_index = 0;
                vec![Effect::SetDirty]
            }
            Command::JumpToBottom => {
                self.branch_index = self.branch_tree.len().saturating_sub(1);
                vec![Effect::SetDirty]
            }
            Command::ToggleBranchNode { key, expanded } => {
                self.expanded_nodes.insert(key.clone(), *expanded);
                vec![Effect::RebuildBranchTree, Effect::SetDirty]
            }
            _ => vec![],
        }
    }
}

impl Default for BranchState {
    fn default() -> Self {
        Self::new()
    }
}

fn branch_cycle_forward(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (current + 1) % len
    }
}

fn branch_cycle_backward(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else if current > 0 {
        current - 1
    } else {
        len - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> BranchState {
        BranchState::new()
    }

    #[test]
    fn test_move_up_wraps_around() {
        let mut state = make_state();
        state.branch_tree = vec![
            TreeItem {
                name: "a".into(),
                depth: 0,
                expandable: false,
                expanded: false,
                is_branch: true,
                full_path: "a".into(),
                tree_prefix: "".into(),
                last_commit_date: None,
                key: "a".into(),
            },
            TreeItem {
                name: "b".into(),
                depth: 0,
                expandable: false,
                expanded: false,
                is_branch: true,
                full_path: "b".into(),
                tree_prefix: "".into(),
                last_commit_date: None,
                key: "b".into(),
            },
        ];
        state.branch_index = 0;
        state.handle_command(&Command::MoveUp);
        assert_eq!(state.branch_index, 1);
    }

    #[test]
    fn test_move_down_wraps_to_start() {
        let mut state = make_state();
        state.branch_tree = vec![
            TreeItem {
                name: "a".into(),
                depth: 0,
                expandable: false,
                expanded: false,
                is_branch: true,
                full_path: "a".into(),
                tree_prefix: "".into(),
                last_commit_date: None,
                key: "a".into(),
            },
            TreeItem {
                name: "b".into(),
                depth: 0,
                expandable: false,
                expanded: false,
                is_branch: true,
                full_path: "b".into(),
                tree_prefix: "".into(),
                last_commit_date: None,
                key: "b".into(),
            },
        ];
        state.branch_index = 1;
        state.handle_command(&Command::MoveDown);
        assert_eq!(state.branch_index, 0);
    }

    #[test]
    fn test_move_on_empty_tree_sets_zero() {
        let mut state = make_state();
        state.branch_index = 5;
        state.handle_command(&Command::MoveUp);
        assert_eq!(state.branch_index, 0);
    }

    #[test]
    fn test_page_up() {
        let mut state = make_state();
        state.branch_index = 20;
        state.handle_command(&Command::PageUp);
        assert_eq!(state.branch_index, 10);
    }

    #[test]
    fn test_page_down_clamped() {
        let mut state = make_state();
        state.branch_tree = vec![TreeItem {
            last_commit_date: None,
            name: "a".into(),
            depth: 0,
            expandable: false,
            expanded: false,
            is_branch: true,
            full_path: "a".into(),
            tree_prefix: "".into(),
            key: "a".into(),
        }];
        state.branch_index = 0;
        state.handle_command(&Command::PageDown);
        assert_eq!(state.branch_index, 0);
    }

    #[test]
    fn test_jump_to_top() {
        let mut state = make_state();
        state.branch_index = 42;
        state.handle_command(&Command::JumpToTop);
        assert_eq!(state.branch_index, 0);
    }

    #[test]
    fn test_jump_to_bottom() {
        let mut state = make_state();
        state.branch_tree = vec![
            TreeItem {
                name: "a".into(),
                depth: 0,
                expandable: false,
                expanded: false,
                is_branch: true,
                full_path: "a".into(),
                tree_prefix: "".into(),
                last_commit_date: None,
                key: "a".into(),
            },
            TreeItem {
                name: "b".into(),
                depth: 0,
                expandable: false,
                expanded: false,
                is_branch: true,
                full_path: "b".into(),
                tree_prefix: "".into(),
                last_commit_date: None,
                key: "b".into(),
            },
            TreeItem {
                name: "c".into(),
                depth: 0,
                expandable: false,
                expanded: false,
                is_branch: true,
                full_path: "c".into(),
                tree_prefix: "".into(),
                last_commit_date: None,
                key: "c".into(),
            },
        ];
        state.handle_command(&Command::JumpToBottom);
        assert_eq!(state.branch_index, 2);
    }

    #[test]
    fn test_toggle_branch_node_stores_key_and_returns_rebuild_effect() {
        let mut state = make_state();
        let effects = state.handle_command(&Command::ToggleBranchNode {
            key: "feature/".into(),
            expanded: true,
        });
        assert_eq!(state.expanded_nodes.get("feature/"), Some(&true));
        assert!(effects
            .iter()
            .any(|e| matches!(e, Effect::RebuildBranchTree)));
        assert!(effects.iter().any(|e| matches!(e, Effect::SetDirty)));
    }

    #[test]
    fn test_move_returns_set_dirty_effect() {
        let mut state = make_state();
        state.branch_tree = vec![TreeItem {
            last_commit_date: None,
            name: "a".into(),
            depth: 0,
            expandable: false,
            expanded: false,
            is_branch: true,
            full_path: "a".into(),
            tree_prefix: "".into(),
            key: "a".into(),
        }];
        let effects = state.handle_command(&Command::MoveDown);
        assert!(effects.iter().any(|e| matches!(e, Effect::SetDirty)));
    }

    #[test]
    fn test_unknown_command_returns_empty_effects() {
        let mut state = make_state();
        let effects = state.handle_command(&Command::CopyHashShort);
        assert!(effects.is_empty());
    }
}
