use std::collections::BTreeMap;

use ratatui::widgets::TableState;

use crate::models::*;
use crate::theme::Theme;
use crate::ui;

/// Pure application state -- all data fields without workers or rendering logic.
/// Lives inside App as `App.state`, accessible via Deref coercion.
pub struct AppState {
    pub repo_path: String,

    // Branch panel state
    pub all_branches: BranchData,
    pub branch_tree: Vec<TreeItem>,
    pub expanded_nodes: BTreeMap<String, bool>,
    pub branch_index: usize,
    pub branch_scope: BranchScope,
    pub selected_branch: Option<String>,

    // Search state
    pub search_query: String,
    pub cursor_pos: usize,

    // Commit table state
    pub all_commits: Vec<Commit>,
    /// None = show all commits (references all_commits directly).
    pub filtered_commits: Option<Vec<Commit>>,
    pub selected_index: usize,
    /// Maps visible row (skipping graph_only) to filtered_commits index.
    pub visible_to_commit: Vec<usize>,

    // Diff state
    pub commit_info: Option<CommitInfo>,
    pub diff_lines: Vec<String>,
    pub file_entries: Vec<FileEntry>,
    pub selected_file_index: usize,
    pub diff_scroll: usize,
    pub last_selected_hash: Option<String>,

    // UI state
    pub focus: Panel,
    pub branch_width_pct: u16,
    pub diff_height_pct: u16,
    pub dragging: Option<ui::layout::DragDirection>,
    pub scrollbar_drag: Option<Panel>,
    pub last_size: Option<(u16, u16)>,
    pub last_mouse_pos: Option<(u16, u16)>,
    pub table_state: TableState,
    pub branch_list_offset: usize,

    // Scrollbar widgets
    pub branch_scrollbar: ui::scrollbar_view::ScrollbarView,
    pub table_scrollbar: ui::scrollbar_view::ScrollbarView,
    pub diff_scrollbar: ui::scrollbar_view::ScrollbarView,

    // Status
    pub status_message: Option<String>,
    pub branches_loaded: bool,
    pub commits_loaded: bool,
    pub diff_pending: bool,
    pub poll_interval_ms: u8,

    // Lazy loading state
    pub commit_limit: usize,
    pub all_commits_loaded: bool,
    pub loading_more: bool,

    // Graph mode
    pub simplified_graph: bool,
    pub full_commits_cache: Option<Vec<Commit>>,
    pub simplified_commits_cache: Option<Vec<Commit>>,

    // Frame timing (debug mode)
    pub debug: bool,
    pub last_frame_time_ms: u64,

    /// Set true when state changes; cleared after each render.
    pub dirty: bool,

    /// Color theme (customizable via config).
    pub theme: Theme,
}

impl AppState {
    /// Create default application state.
    /// Does not spawn workers -- that happens in App::new().
    pub fn new(repo_path: String, simplified_graph: bool, debug: bool) -> Self {
        AppState {
            repo_path,
            all_branches: BranchData {
                default_branch: None,
                entries: Vec::new(),
            },
            branch_tree: Vec::new(),
            expanded_nodes: BTreeMap::new(),
            branch_index: 0,
            branch_scope: BranchScope::All,
            selected_branch: None,
            search_query: String::new(),
            cursor_pos: 0,
            all_commits: Vec::new(),
            filtered_commits: None,
            selected_index: 0,
            visible_to_commit: Vec::new(),
            commit_info: None,
            diff_lines: Vec::new(),
            file_entries: Vec::new(),
            selected_file_index: 0,
            diff_scroll: 0,
            last_selected_hash: None,
            focus: Panel::Commits,
            branch_width_pct: ui::layout::DEFAULT_BRANCH_PCT,
            diff_height_pct: ui::layout::DEFAULT_DIFF_PCT,
            dragging: None,
            scrollbar_drag: None,
            last_size: None,
            last_mouse_pos: None,
            table_state: TableState::default(),
            branch_list_offset: 0,
            branch_scrollbar: ui::scrollbar_view::ScrollbarView::new(),
            table_scrollbar: ui::scrollbar_view::ScrollbarView::new(),
            diff_scrollbar: ui::scrollbar_view::ScrollbarView::new(),
            status_message: None,
            branches_loaded: false,
            commits_loaded: false,
            diff_pending: false,
            poll_interval_ms: 10,
            commit_limit: 5000,
            all_commits_loaded: false,
            loading_more: false,
            simplified_graph,
            full_commits_cache: None,
            simplified_commits_cache: None,
            debug,
            last_frame_time_ms: 0,
            dirty: true,
            theme: Theme::default(),
        }
    }
}
