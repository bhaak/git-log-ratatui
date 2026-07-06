use std::sync::Arc;

use crate::config::Config;
use crate::state::*;
use crate::theme::Theme;

/// Pure application state -- all data fields without workers or rendering logic.
/// Lives inside App as `App.state`, accessible via `App::state()` / `App::state_mut()`.
pub struct AppState {
    pub repo_path: String,
    pub debug: bool,
    pub last_frame_time_ms: u64,
    pub theme: Arc<Theme>,
    pub branch: BranchState,
    pub commit: CommitTableState,
    pub diff: DiffState,
    pub search: SearchState,
    pub ui: UiState,
}

impl AppState {
    /// Create default application state.
    /// Does not spawn workers -- that happens in App::new().
    pub fn new(repo_path: String, config: &Config, simplified_graph: bool, debug: bool) -> Self {
        AppState {
            repo_path,
            debug,
            last_frame_time_ms: 0,
            theme: Arc::new(Theme::default()),
            branch: BranchState::new(),
            commit: CommitTableState::new(simplified_graph, config.behavior.commit_batch_size),
            diff: DiffState::new(),
            search: SearchState::new(),
            ui: UiState::new(
                config.layout.branch_width_pct,
                config.layout.diff_height_pct,
                config.behavior.poll_min_ms,
            ),
        }
    }
}
