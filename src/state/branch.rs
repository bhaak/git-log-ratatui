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
            },
            branch_tree: Vec::new(),
            expanded_nodes: BTreeMap::new(),
            branch_index: 0,
            branch_scope: BranchScope::All,
            selected_branch: None,
            branch_list_offset: 0,
            branches_loaded: false,
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
