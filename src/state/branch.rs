use std::collections::BTreeMap;

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
}

impl Default for BranchState {
    fn default() -> Self {
        Self::new()
    }
}
