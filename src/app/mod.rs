use std::time::Instant;

use ratatui::widgets::TableState;
use tracing::{debug, error};

use crate::config::Config;
use crate::error::AppError;

use crate::workers;
use crate::workers::{
    BranchCommand, BranchResult, BranchWorker, CommitCommand, CommitResult, CommitWorker,
    DiffCommand, DiffResult, DiffWorker,
};
use state::AppState;

pub mod branches;
pub mod cache;
pub mod commands;
mod input;
mod render;
pub mod search;
pub mod state;
mod viewport;

pub(crate) const PAGE_SIZE: usize = 10;
const INITIAL_COMMIT_LIMIT: usize = 5000;
pub(crate) const COMMIT_LIMIT_INCREMENT: usize = 5000;
pub(crate) const POLL_INTERVAL_DEFAULT: u8 = 10;
pub(crate) const POLL_INTERVAL_MAX: u8 = 200;
pub(crate) const POLL_BACKOFF_STEP: u8 = 10;

pub struct App {
    pub state: AppState,
    branch_worker: BranchWorker,
    commit_worker: CommitWorker,
    diff_worker: DiffWorker,
}

impl App {
    pub fn new(
        repo_path: String,
        config: &Config,
        cli_simplified: bool,
        cli_debug: bool,
    ) -> Result<Self, AppError> {
        let branch_worker = workers::new_branch_worker(&repo_path)?;
        let commit_worker = workers::new_commit_worker(&repo_path)?;
        let diff_worker = workers::new_diff_worker(&repo_path)?;

        let simplified_graph = cli_simplified || config.behavior.simplified_graph_default;
        let debug = cli_debug || config.behavior.debug_default;

        Ok(App {
            state: AppState::new(repo_path, config, simplified_graph, debug),
            branch_worker,
            commit_worker,
            diff_worker,
        })
    }

    pub fn run(
        &mut self,
        terminal: &mut ratatui::Terminal<impl ratatui::backend::Backend>,
    ) -> Result<(), AppError> {
        self.request_branches();
        self.request_commits(None);

        loop {
            let frame_start = if self.state.debug {
                Some(Instant::now())
            } else {
                None
            };
            if self.process_git_results() {
                self.state.ui.dirty = true;
            }
            if self.state.ui.dirty {
                let draw_result = terminal.draw(|frame| render::render(self, frame));
                if let Err(e) = draw_result {
                    return Err(format!("Render error: {}", e).into());
                }
                if let Some(start) = frame_start {
                    self.state.last_frame_time_ms = start.elapsed().as_millis() as u64;
                }
                self.state.ui.dirty = false;
            }
            match input::handle_event(self)? {
                input::EventOutcome::Quit => break,
                input::EventOutcome::Continue => {}
            }
        }

        Ok(())
    }

    /// Non-interactive profiling mode: load branches, commits, and diff repeatedly, print timings.
    pub fn run_profile(&mut self, iterations: u32) -> Result<(), AppError> {
        let total = Instant::now();
        let mut branch_total = 0u128;
        let mut commit_total = 0u128;
        let mut diff_total = 0u128;

        for i in 0..iterations {
            // Phase 1: Load branches
            let t0 = Instant::now();
            self.request_branches();
            match self.branch_worker.recv() {
                Some(BranchResult::Branches(branches)) => {
                    self.state.branch.all_branches = branches;
                }
                Some(BranchResult::Error(e)) => return Err(e),
                None => return Err("Branch worker disconnected".into()),
            }
            branch_total += t0.elapsed().as_millis();

            // Phase 2: Load commits with graph
            let t0 = Instant::now();
            self.request_commits(None);
            match self.commit_worker.recv() {
                Some(CommitResult::Commits(commits)) => {
                    self.state.commit.all_commits = commits;
                }
                Some(CommitResult::Error(e)) => return Err(e),
                None => return Err("Commit worker disconnected".into()),
            }
            commit_total += t0.elapsed().as_millis();

            // Phase 3: Load diff for first commit (if any)
            let first_hash = self
                .state
                .commit
                .all_commits
                .first()
                .map(|c| c.hash.clone());
            if let Some(hash) = first_hash {
                let t0 = Instant::now();
                self.request_diff(&hash);
                match self.diff_worker.recv() {
                    Some(DiffResult::Diff {
                        commit_info,
                        diff_lines,
                        file_entries,
                    }) => {
                        self.state.diff.commit_info = Some(*commit_info);
                        self.state.diff.diff_lines = diff_lines;
                        self.state.diff.file_entries = file_entries;
                    }
                    Some(DiffResult::Error(e)) => return Err(e),
                    None => return Err("Diff worker disconnected".into()),
                }
                diff_total += t0.elapsed().as_millis();
            }

            if i == 0 {
                println!(
                    "Total branches: {}",
                    self.state.branch.all_branches.entries.len()
                );
                println!("Total commits:  {}", self.state.commit.all_commits.len());
            }
        }

        let total_ms = total.elapsed().as_millis();
        let n = iterations as u128;

        println!(
            "Iterations: {iterations} | avg (ms): branches={} commits={} diff={} total={total_ms}",
            branch_total / n,
            commit_total / n,
            diff_total / n
        );

        Ok(())
    }

    // --- Worker communication (per-window threads) ---

    fn request_branches(&mut self) {
        debug!(
            "Requesting branches (scope={:?})",
            self.state.branch.branch_scope
        );
        self.state.branch.branches_loaded = false;
        self.branch_worker.send(BranchCommand::FetchBranches {
            scope: self.state.branch.branch_scope,
        });
    }

    fn request_commits(&mut self, branch: Option<String>) {
        debug!("Requesting commits (branch={:?})", branch);
        self.state.commit.commits_loaded = false;
        self.state.commit.all_commits.clear();
        self.state.commit.filtered_commits = None;
        self.state.commit.visible_to_commit.clear();
        self.state.commit.selected_index = 0;
        self.state.diff.commit_info = None;
        self.state.diff.diff_lines = Vec::new();
        self.state.diff.file_entries = Vec::new();
        self.state.diff.last_selected_hash = None;
        self.state.commit.table_state = TableState::default();
        self.state.commit.commit_limit = INITIAL_COMMIT_LIMIT;
        self.state.commit.all_commits_loaded = false;
        self.state.commit.loading_more = false;
        self.state.branch.selected_branch = branch.clone();
        self.state.ui.status_message = Some("Loading commits...".to_string());
        self.state.commit.cache.invalidate();
        if !self.commit_worker.send(CommitCommand::FetchCommits {
            branch,
            scope: self.state.branch.branch_scope,
            limit: Some(self.state.commit.commit_limit),
            simplified: self.state.commit.simplified_graph,
        }) {
            self.state.ui.status_message =
                Some("Commit worker disconnected — restart required".to_string());
        }
    }

    /// Toggle between full (git-graph) and simplified (git2 revwalk) graphs.
    /// Saves the current commit list to a cache so re-toggling is instant.
    fn toggle_simplified_graph(&mut self) {
        if self.state.commit.simplified_graph {
            self.state
                .commit
                .cache
                .set(true, self.state.commit.all_commits.clone());
        } else {
            self.state
                .commit
                .cache
                .set(false, self.state.commit.all_commits.clone());
        }

        self.state.commit.simplified_graph = !self.state.commit.simplified_graph;

        if let Some(cached) = self
            .state
            .commit
            .cache
            .get(self.state.commit.simplified_graph)
        {
            self.state.commit.all_commits = cached;
            self.state.commit.commits_loaded = true;
            search::apply_search_filter(&mut self.state);
            return;
        }

        // Cache miss: fetch from the worker thread.
        let branch = self.state.branch.selected_branch.clone();
        self.request_commits(branch);
    }

    fn request_diff(&mut self, hash: &str) {
        self.state.diff.diff_pending = true;
        self.diff_worker.send(DiffCommand::FetchDiff {
            hash: hash.to_string(),
        });
    }

    /// Poll all three worker channels for results (non-blocking, parallel streams).
    /// Returns true if any worker produced data that requires a redraw.
    fn process_git_results(&mut self) -> bool {
        let mut changed = false;
        changed |= self.poll_branch_results();
        changed |= self.poll_commit_results();
        changed |= self.poll_diff_results();
        changed
    }

    fn poll_branch_results(&mut self) -> bool {
        let mut changed = false;
        while let Some(result) = self.branch_worker.try_recv() {
            changed = true;
            match result {
                BranchResult::Branches(branches) => {
                    self.state.branch.all_branches = branches;
                    branches::rebuild_branch_tree(&mut self.state);
                    self.state.branch.branches_loaded = true;
                }
                BranchResult::Error(err) => {
                    error!("Branch worker error: {}", err);
                    self.state.ui.status_message = Some(err.to_string());
                }
            }
        }
        changed
    }

    fn poll_commit_results(&mut self) -> bool {
        let mut changed = false;
        while let Some(result) = self.commit_worker.try_recv() {
            changed = true;
            match result {
                CommitResult::Commits(commits) => {
                    let prev_len = self.state.commit.all_commits.len();
                    self.state.commit.all_commits = commits;
                    if self.state.commit.loading_more {
                        self.state.commit.loading_more = false;
                        if self.state.commit.all_commits.len() <= prev_len {
                            self.state.commit.all_commits_loaded = true;
                        }
                        search::reapply_filter_preserving_selection(&mut self.state);
                    } else {
                        search::apply_search_filter(&mut self.state);
                    }
                    self.state.commit.commits_loaded = true;
                    self.state.ui.status_message = None;
                    self.state.commit.cache.set(
                        self.state.commit.simplified_graph,
                        self.state.commit.all_commits.clone(),
                    );
                    if !self
                        .state
                        .commit
                        .filtered_commits
                        .as_deref()
                        .unwrap_or(&self.state.commit.all_commits)
                        .is_empty()
                        && self.state.diff.commit_info.is_none()
                    {
                        let hash = self
                            .state
                            .commit
                            .filtered_commits
                            .as_deref()
                            .unwrap_or(&self.state.commit.all_commits)[0]
                            .hash
                            .clone();
                        if !hash.is_empty() {
                            self.request_diff(&hash);
                        }
                    }
                }
                CommitResult::Error(err) => {
                    error!("Commit worker error: {}", err);
                    self.state.ui.status_message = Some(err.to_string());
                }
            }
        }
        changed
    }

    fn poll_diff_results(&mut self) -> bool {
        let mut changed = false;
        while let Some(result) = self.diff_worker.try_recv() {
            changed = true;
            match result {
                DiffResult::Diff {
                    commit_info,
                    diff_lines,
                    file_entries,
                } => {
                    self.state.diff.commit_info = Some(*commit_info);
                    self.state.diff.diff_lines = diff_lines;
                    self.state.diff.file_entries = file_entries;
                    self.state.diff.diff_scroll = 0;
                    self.state.diff.selected_file_index = 0;
                    self.state.diff.diff_pending = false;
                }
                DiffResult::Error(err) => {
                    error!("Diff worker error: {}", err);
                    self.state.ui.status_message = Some(err.to_string());
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::search;
    use super::*;
    use crate::models::*;
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    use crate::ui;

    /// Helper to build a minimal App for testing pure logic functions.
    fn test_app() -> App {
        let config = Config::default();
        App {
            state: AppState::new(".".to_string(), &config, false, false),
            branch_worker: workers::new_branch_worker(".").unwrap(),
            commit_worker: workers::new_commit_worker(".").unwrap(),
            diff_worker: workers::new_diff_worker(".").unwrap(),
        }
    }

    #[test]
    fn test_search_filter_empty_query() {
        let mut app = test_app();
        app.state.commit.all_commits = vec![Commit {
            hash: "abc".into(),
            author: "alice".into(),
            date: "2024-01-01".into(),
            subject: "fix bug".into(),
            graph: "*".into(),
            graph_colors: vec![],
            merge: false,
            graph_only: false,
            decorations: vec![],
            deco_line: 0,
        }];
        app.state.search.search_query.clear();
        search::apply_search_filter(&mut app.state);
        assert_eq!(
            app.state
                .commit
                .filtered_commits
                .as_deref()
                .unwrap_or(&app.state.commit.all_commits)
                .len(),
            1
        );
    }

    #[test]
    fn test_search_filter_by_subject() {
        let mut app = test_app();
        app.state.commit.all_commits = vec![
            Commit {
                hash: "abc".into(),
                author: "alice".into(),
                date: "2024-01-01".into(),
                subject: "fix bug".into(),
                graph: "*".into(),
                graph_colors: vec![],
                merge: false,
                graph_only: false,
                decorations: vec![],
                deco_line: 0,
            },
            Commit {
                hash: "def".into(),
                author: "bob".into(),
                date: "2024-01-02".into(),
                subject: "add feature".into(),
                graph: "*".into(),
                graph_colors: vec![],
                merge: false,
                graph_only: false,
                decorations: vec![],
                deco_line: 0,
            },
        ];
        app.state.search.search_query = "bug".into();
        search::apply_search_filter(&mut app.state);
        assert_eq!(
            app.state.commit.filtered_commits.as_deref().unwrap().len(),
            1
        );
        assert_eq!(
            app.state.commit.filtered_commits.as_deref().unwrap()[0].hash,
            "abc"
        );
    }

    #[test]
    fn test_search_filter_case_insensitive() {
        let mut app = test_app();
        app.state.commit.all_commits = vec![Commit {
            hash: "abc".into(),
            author: "ALICE".into(),
            date: "2024-01-01".into(),
            subject: "Fix Bug".into(),
            graph: "*".into(),
            graph_colors: vec![],
            merge: false,
            graph_only: false,
            decorations: vec![],
            deco_line: 0,
        }];
        app.state.search.search_query = "bug".into();
        search::apply_search_filter(&mut app.state);
        assert_eq!(
            app.state.commit.filtered_commits.as_deref().unwrap().len(),
            1
        );
    }

    #[test]
    fn test_visible_mapping_skips_graph_only() {
        let mut app = test_app();
        app.state.commit.filtered_commits = Some(vec![
            Commit {
                hash: "abc".into(),
                author: "a".into(),
                date: "d".into(),
                subject: "s".into(),
                graph: "*".into(),
                graph_colors: vec![],
                merge: false,
                graph_only: false,
                decorations: vec![],
                deco_line: 0,
            },
            Commit {
                hash: "".into(),
                author: "".into(),
                date: "".into(),
                subject: "".into(),
                graph: "|".into(),
                graph_colors: vec![],
                merge: false,
                graph_only: true,
                decorations: vec![],
                deco_line: 0,
            },
            Commit {
                hash: "def".into(),
                author: "b".into(),
                date: "d".into(),
                subject: "s".into(),
                graph: "*".into(),
                graph_colors: vec![],
                merge: false,
                graph_only: false,
                decorations: vec![],
                deco_line: 0,
            },
        ]);
        search::build_visible_mapping(&mut app.state);
        assert_eq!(search::visible_count(&app.state), 2);
        assert_eq!(search::visible_to_filtered(&app.state, 0), 0); // first commit
        assert_eq!(search::visible_to_filtered(&app.state, 1), 2); // third commit
    }

    #[test]
    fn test_clamp_selection_empty() {
        let mut app = test_app();
        search::clamp_selection(&mut app.state);
        assert_eq!(app.state.commit.selected_index, 0);
    }

    #[test]
    fn test_clamp_selection_in_range() {
        let mut app = test_app();
        app.state.commit.filtered_commits = Some(vec![Commit {
            hash: "abc".into(),
            author: "a".into(),
            date: "d".into(),
            subject: "s".into(),
            graph: "*".into(),
            graph_colors: vec![],
            merge: false,
            graph_only: false,
            decorations: vec![],
            deco_line: 0,
        }]);
        search::build_visible_mapping(&mut app.state);
        app.state.commit.selected_index = 0;
        search::clamp_selection(&mut app.state);
        assert_eq!(app.state.commit.selected_index, 0);
    }

    #[test]
    fn test_branch_click_with_scroll_offset() {
        let mut app = test_app();

        app.state.branch.branch_tree = (0..50)
            .map(|i| TreeItem {
                name: format!("branch_{}", i),
                key: format!("branch_{}", i),
                depth: 0,
                is_branch: true,
                full_path: format!("branch_{}", i),
                expanded: false,
                expandable: false,
                tree_prefix: String::new(),
            })
            .collect();
        app.state.branch.branch_list_offset = 20;

        let panel_y = 0u16;
        let click_row = 6u16;
        let rel_row = (click_row
            .saturating_sub(panel_y)
            .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
        let actual_index = rel_row + app.state.branch.branch_list_offset;

        assert_eq!(rel_row, 5);
        assert_eq!(actual_index, 25);
        assert!(actual_index < app.state.branch.branch_tree.len());
        assert_eq!(
            app.state.branch.branch_tree[actual_index].full_path,
            "branch_25"
        );
    }

    #[test]
    fn test_branch_click_without_scroll() {
        let mut app = test_app();

        app.state.branch.branch_tree = (0..10)
            .map(|i| TreeItem {
                name: format!("branch_{}", i),
                key: format!("branch_{}", i),
                depth: 0,
                is_branch: true,
                full_path: format!("branch_{}", i),
                expanded: false,
                expandable: false,
                tree_prefix: String::new(),
            })
            .collect();
        app.state.branch.branch_list_offset = 0;

        let panel_y = 0u16;
        let click_row = 3u16;
        let rel_row = (click_row
            .saturating_sub(panel_y)
            .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
        let actual_index = rel_row + app.state.branch.branch_list_offset;

        assert_eq!(rel_row, 2);
        assert_eq!(actual_index, 2);
        assert_eq!(
            app.state.branch.branch_tree[actual_index].full_path,
            "branch_2"
        );
    }

    #[test]
    fn test_app_new_returns_ok() {
        let config = Config::default();
        let result = App::new(".".to_string(), &config, false, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_scrollbar_click_does_not_change_selection() {
        let mut app = test_app();

        app.state.ui.last_size = Some((100, 30));
        app.state.branch.branch_tree = (0..50)
            .map(|i| TreeItem {
                name: format!("branch_{}", i),
                key: format!("branch_{}", i),
                depth: 0,
                is_branch: true,
                full_path: format!("branch_{}", i),
                expanded: false,
                expandable: false,
                tree_prefix: String::new(),
            })
            .collect();

        {
            use ratatui::{backend::TestBackend, Terminal};
            let mut terminal = Terminal::new(TestBackend::new(2, 10)).unwrap();
            terminal
                .draw(|f| {
                    app.state.branch.scrollbar.render(
                        f,
                        Rect::new(1, 0, 1, 10),
                        50,
                        5,
                        0,
                        Style::default(),
                    );
                })
                .unwrap();
        }

        let prev_branch_index = app.state.branch.branch_index;

        // Click on scrollbar column (x=19, which is far right) — should NOT change selection
        for cmd in input::mouse_click(&app, 19, 5) {
            input::execute_command(&mut app, cmd);
        }

        assert_eq!(
            app.state.branch.branch_index, prev_branch_index,
            "branch_index should NOT change on scrollbar click"
        );

        // Click on content area (x=5) — SHOULD change selection
        for cmd in input::mouse_click(&app, 5, 3) {
            input::execute_command(&mut app, cmd);
        }
        assert_eq!(
            app.state.branch.branch_index, 2,
            "branch_index should change on content click"
        );
    }
}
