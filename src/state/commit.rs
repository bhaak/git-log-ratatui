use ratatui::widgets::TableState;

use crate::models::Commit;

/// Commit table state — all commit data, filtering, selection, graph mode, and lazy loading.
pub struct CommitTableState {
    pub all_commits: Vec<Commit>,
    pub filtered_commits: Option<Vec<Commit>>,
    pub selected_index: usize,
    pub visible_to_commit: Vec<usize>,
    pub table_state: TableState,
    pub simplified_graph: bool,
    pub full_commits_cache: Option<Vec<Commit>>,
    pub simplified_commits_cache: Option<Vec<Commit>>,
    pub commits_loaded: bool,
    pub commit_limit: usize,
    pub all_commits_loaded: bool,
    pub loading_more: bool,
}

impl CommitTableState {
    pub fn new(simplified_graph: bool, commit_limit: usize) -> Self {
        CommitTableState {
            all_commits: Vec::new(),
            filtered_commits: None,
            selected_index: 0,
            visible_to_commit: Vec::new(),
            table_state: TableState::default(),
            simplified_graph,
            full_commits_cache: None,
            simplified_commits_cache: None,
            commits_loaded: false,
            commit_limit,
            all_commits_loaded: false,
            loading_more: false,
        }
    }
}
