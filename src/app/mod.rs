use std::ops::{Deref, DerefMut};
use std::time::Instant;

use ratatui::widgets::TableState;

use crate::error::AppError;

use crate::workers;
use crate::workers::{
    BranchCommand, BranchResult, BranchWorker, CommitCommand, CommitResult, CommitWorker,
    DiffCommand, DiffResult, DiffWorker,
};
use state::AppState;

pub(crate) mod branches;
mod input;
mod render;
pub(crate) mod search;
pub(crate) mod state;
mod viewport;

pub(crate) const PAGE_SIZE: usize = 10;
/// Number of commits loaded at startup. Keeps first paint fast on large repos.
const INITIAL_COMMIT_LIMIT: usize = 5000;
/// How many additional commits to load when scrolling near the end.
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

impl Deref for App {
    type Target = AppState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for App {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl App {
    pub fn new(repo_path: String, simplified_graph: bool, debug: bool) -> Result<Self, AppError> {
        let branch_worker = workers::new_branch_worker(&repo_path)?;
        let commit_worker = workers::new_commit_worker(&repo_path)?;
        let diff_worker = workers::new_diff_worker(&repo_path)?;

        Ok(App {
            state: AppState::new(repo_path, simplified_graph, debug),
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
            let frame_start = if self.debug {
                Some(Instant::now())
            } else {
                None
            };
            if self.process_git_results() {
                self.dirty = true;
            }
            if self.dirty {
                let draw_result = terminal.draw(|frame| render::render(self, frame));
                if let Err(e) = draw_result {
                    return Err(format!("Render error: {}", e).into());
                }
                if let Some(start) = frame_start {
                    self.last_frame_time_ms = start.elapsed().as_millis() as u64;
                }
                self.dirty = false;
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
                    self.all_branches = branches;
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
                    self.all_commits = commits;
                }
                Some(CommitResult::Error(e)) => return Err(e),
                None => return Err("Commit worker disconnected".into()),
            }
            commit_total += t0.elapsed().as_millis();

            // Phase 3: Load diff for first commit (if any)
            let first_hash = self.all_commits.first().map(|c| c.hash.clone());
            if let Some(hash) = first_hash {
                let t0 = Instant::now();
                self.request_diff(&hash);
                match self.diff_worker.recv() {
                    Some(DiffResult::Diff {
                        commit_info,
                        diff_lines,
                        file_entries,
                    }) => {
                        self.commit_info = Some(*commit_info);
                        self.diff_lines = diff_lines;
                        self.file_entries = file_entries;
                    }
                    Some(DiffResult::Error(e)) => return Err(e),
                    None => return Err("Diff worker disconnected".into()),
                }
                diff_total += t0.elapsed().as_millis();
            }

            if i == 0 {
                println!("Total branches: {}", self.all_branches.entries.len());
                println!("Total commits:  {}", self.all_commits.len());
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
        self.branches_loaded = false;
        self.branch_worker.send(BranchCommand::FetchBranches {
            scope: self.branch_scope,
        });
    }

    fn request_commits(&mut self, branch: Option<String>) {
        self.commits_loaded = false;
        // Clear stale commit data immediately so the old branch's commits are
        // not shown while the new branch's data is loading. Also reset
        // selection to the first commit (tip of the branch) and clear diff
        // data so the old branch's metadata isn't shown.
        self.all_commits.clear();
        self.filtered_commits = None;
        self.visible_to_commit.clear();
        self.selected_index = 0;
        self.commit_info = None;
        self.diff_lines = Vec::new();
        self.file_entries = Vec::new();
        self.last_selected_hash = None;
        self.table_state = TableState::default();
        // Fresh load (startup, branch change, scope change): reset the window.
        self.commit_limit = INITIAL_COMMIT_LIMIT;
        self.all_commits_loaded = false;
        self.loading_more = false;
        self.selected_branch = branch.clone();
        self.status_message = Some("Loading commits...".to_string());
        // Branch / scope changed: the other mode's cache is now stale.
        self.full_commits_cache = None;
        self.simplified_commits_cache = None;
        if !self.commit_worker.send(CommitCommand::FetchCommits {
            branch,
            scope: self.branch_scope,
            limit: Some(self.commit_limit),
            simplified: self.simplified_graph,
        }) {
            self.status_message = Some("Commit worker disconnected — restart required".to_string());
        }
    }

    /// Toggle between full (git-graph) and simplified (git2 revwalk) graphs.
    /// Saves the current commit list to a cache so re-toggling is instant.
    fn toggle_simplified_graph(&mut self) {
        // Save current commits into the cache for the old mode.
        if self.simplified_graph {
            self.simplified_commits_cache = Some(self.all_commits.clone());
        } else {
            self.full_commits_cache = Some(self.all_commits.clone());
        }

        self.simplified_graph = !self.simplified_graph;

        // Restore from cache if the target mode was previously loaded.
        if self.simplified_graph {
            if let Some(cached) = self.simplified_commits_cache.take() {
                self.all_commits = cached;
                self.commits_loaded = true;
                search::apply_search_filter(&mut self.state);
                return;
            }
        } else if let Some(cached) = self.full_commits_cache.take() {
            self.all_commits = cached;
            self.commits_loaded = true;
            search::apply_search_filter(&mut self.state);
            return;
        }

        // Cache miss: fetch from the worker thread.
        self.request_commits(self.selected_branch.clone());
    }

    fn request_diff(&mut self, hash: &str) {
        self.diff_pending = true;
        self.diff_worker.send(DiffCommand::FetchDiff {
            hash: hash.to_string(),
        });
    }

    /// Poll all three worker channels for results (non-blocking, parallel streams).
    /// Returns true if any worker produced data that requires a redraw.
    fn process_git_results(&mut self) -> bool {
        let mut changed = false;

        // Poll branch worker
        while let Some(result) = self.branch_worker.try_recv() {
            changed = true;
            match result {
                BranchResult::Branches(branches) => {
                    self.all_branches = branches;
                    branches::rebuild_branch_tree(&mut self.state);
                    self.branches_loaded = true;
                }
                BranchResult::Error(err) => {
                    self.status_message = Some(err.to_string());
                }
            }
        }

        // Poll commit worker
        while let Some(result) = self.commit_worker.try_recv() {
            changed = true;
            match result {
                CommitResult::Commits(commits) => {
                    let prev_len = self.all_commits.len();
                    self.all_commits = commits;
                    if self.loading_more {
                        self.loading_more = false;
                        if self.all_commits.len() <= prev_len {
                            self.all_commits_loaded = true;
                        }
                        search::reapply_filter_preserving_selection(&mut self.state);
                    } else {
                        search::apply_search_filter(&mut self.state);
                    }
                    self.commits_loaded = true;
                    self.status_message = None;
                    if self.simplified_graph {
                        self.simplified_commits_cache = Some(self.all_commits.clone());
                    } else {
                        self.full_commits_cache = Some(self.all_commits.clone());
                    }
                    if !self
                        .filtered_commits
                        .as_deref()
                        .unwrap_or(&self.all_commits)
                        .is_empty()
                        && self.commit_info.is_none()
                    {
                        let hash = self
                            .filtered_commits
                            .as_deref()
                            .unwrap_or(&self.all_commits)[0]
                            .hash
                            .clone();
                        if !hash.is_empty() {
                            self.request_diff(&hash);
                        }
                    }
                }
                CommitResult::Error(err) => {
                    self.status_message = Some(err.to_string());
                }
            }
        }

        // Poll diff worker
        while let Some(result) = self.diff_worker.try_recv() {
            changed = true;
            match result {
                DiffResult::Diff {
                    commit_info,
                    diff_lines,
                    file_entries,
                } => {
                    self.commit_info = Some(*commit_info);
                    self.diff_lines = diff_lines;
                    self.file_entries = file_entries;
                    self.diff_scroll = 0;
                    self.selected_file_index = 0;
                    self.diff_pending = false;
                }
                DiffResult::Error(err) => {
                    self.status_message = Some(err.to_string());
                }
            }
        }

        changed
    }

    // --- Branch tree ---

    // --- Search ---
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
        App {
            state: AppState::new(".".to_string(), false, false),
            branch_worker: workers::new_branch_worker(".").unwrap(),
            commit_worker: workers::new_commit_worker(".").unwrap(),
            diff_worker: workers::new_diff_worker(".").unwrap(),
        }
    }

    #[test]
    fn test_search_filter_empty_query() {
        let mut app = test_app();
        app.all_commits = vec![Commit {
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
        app.search_query.clear();
        search::apply_search_filter(&mut app.state);
        assert_eq!(
            app.filtered_commits
                .as_deref()
                .unwrap_or(&app.all_commits)
                .len(),
            1
        );
    }

    #[test]
    fn test_search_filter_by_subject() {
        let mut app = test_app();
        app.all_commits = vec![
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
        app.search_query = "bug".into();
        search::apply_search_filter(&mut app.state);
        assert_eq!(app.filtered_commits.as_deref().unwrap().len(), 1);
        assert_eq!(app.filtered_commits.as_deref().unwrap()[0].hash, "abc");
    }

    #[test]
    fn test_search_filter_case_insensitive() {
        let mut app = test_app();
        app.all_commits = vec![Commit {
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
        app.search_query = "bug".into();
        search::apply_search_filter(&mut app.state);
        assert_eq!(app.filtered_commits.as_deref().unwrap().len(), 1);
    }

    #[test]
    fn test_visible_mapping_skips_graph_only() {
        let mut app = test_app();
        app.filtered_commits = Some(vec![
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
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_clamp_selection_in_range() {
        let mut app = test_app();
        app.filtered_commits = Some(vec![Commit {
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
        app.selected_index = 0;
        search::clamp_selection(&mut app.state);
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_branch_click_with_scroll_offset() {
        let mut app = test_app();

        // Simulate a scrolled branch tree — the list widget offset is 20,
        // meaning 20 items are scrolled off the top.
        app.branch_tree = (0..50)
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
        app.branch_list_offset = 20;

        // Simulate a click at the 5th visible row (terminal row = panel_y + BORDER_OVERHEAD + 5).
        // Panel is at y=0, BORDER_OVERHEAD=1, so clicking terminal row 6 should give
        // rel_row=5, and actual_index = 5 + 20 = 25.
        let panel_y = 0u16;
        let click_row = 6u16; // panel_y=0, BORDER_OVERHEAD=1 → rel_row = 6-0-1 = 5
        let rel_row = (click_row
            .saturating_sub(panel_y)
            .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
        let actual_index = rel_row + app.branch_list_offset;

        assert_eq!(rel_row, 5);
        assert_eq!(actual_index, 25);
        assert!(actual_index < app.branch_tree.len());
        assert_eq!(app.branch_tree[actual_index].full_path, "branch_25");
    }

    #[test]
    fn test_branch_click_without_scroll() {
        let mut app = test_app();

        app.branch_tree = (0..10)
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
        app.branch_list_offset = 0;

        let panel_y = 0u16;
        let click_row = 3u16; // panel_y=0, BORDER_OVERHEAD=1 → rel_row = 3-0-1 = 2
        let rel_row = (click_row
            .saturating_sub(panel_y)
            .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
        let actual_index = rel_row + app.branch_list_offset;

        assert_eq!(rel_row, 2);
        assert_eq!(actual_index, 2);
        assert_eq!(app.branch_tree[actual_index].full_path, "branch_2");
    }

    #[test]
    fn test_app_new_returns_ok() {
        let result = App::new(".".to_string(), false, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_scrollbar_click_does_not_change_selection() {
        let mut app = test_app();

        // Use a 100-wide terminal. With DEFAULT_BRANCH_PCT=20, branch gets 20 cols.
        // Scrollbar is at col=19 (rightmost column of branch area).
        app.last_size = Some((100, 30));
        app.branch_tree = (0..50)
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

        // Simulate a prior render that set up the scrollbar state so
        // click_to_index returns Some for scrollbar clicks.
        {
            use ratatui::{backend::TestBackend, Terminal};
            let mut terminal = Terminal::new(TestBackend::new(2, 10)).unwrap();
            terminal
                .draw(|f| {
                    app.branch_scrollbar.render(
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

        let prev_branch_index = app.branch_index;

        // Click on the branch panel scrollbar column (rightmost of 20-col area = col 19)
        input::handle_mouse_click(&mut app, 19, 5);

        assert_eq!(
            app.branch_index, prev_branch_index,
            "branch_index should NOT change on scrollbar click"
        );

        // Click on content area (col=5, row=3) should change branch_index
        input::handle_mouse_click(&mut app, 5, 3);
        // BORDER_OVERHEAD=1, branch area y=0 → rel_row = 3 - 0 - 1 = 2
        assert_eq!(
            app.branch_index, 2,
            "branch_index should change on content click"
        );
    }
}
