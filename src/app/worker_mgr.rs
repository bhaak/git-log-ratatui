//! Worker communication methods for the App.
//!
//! Each UI panel gets its own background thread (branch, commit, diff)
//! following the project convention. This module handles sending commands
//! to workers and polling their results via non-blocking mpsc channels.

use std::time::Instant;

use ratatui::widgets::TableState;
use tracing::{debug, error};

use crate::workers::{
    BranchCommand, BranchResult, CommitCommand, CommitResult, DiffCommand, DiffResult,
};

use super::branches;
use super::search;
use super::App;
use super::INITIAL_COMMIT_LIMIT;

impl App {
    // --- Worker communication (per-window threads) ---

    pub(crate) fn request_branches(&mut self) {
        debug!(
            "Requesting branches (scope={:?})",
            self.state.branch.branch_scope
        );
        self.state.branch.branches_loaded = false;
        self.state.metrics.branch_request_time = Some(Instant::now());
        self.branch_worker.send(BranchCommand::FetchBranches {
            scope: self.state.branch.branch_scope,
        });
    }

    pub(crate) fn request_commits(&mut self, branch: Option<String>) {
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
        self.state.metrics.commit_request_time = Some(Instant::now());
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
    pub(crate) fn toggle_simplified_graph(&mut self) {
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

    pub(crate) fn request_diff(&mut self, hash: &str) {
        self.state.diff.diff_pending = true;
        self.state.metrics.diff_request_time = Some(Instant::now());
        self.diff_worker.send(DiffCommand::FetchDiff {
            hash: hash.to_string(),
        });
    }

    /// Poll all three worker channels for results (non-blocking, parallel streams).
    /// Returns true if any worker produced data that requires a redraw.
    pub(crate) fn process_git_results(&mut self) -> bool {
        let t0 = Instant::now();
        let mut changed = false;
        changed |= self.poll_branch_results();
        changed |= self.poll_commit_results();
        changed |= self.poll_diff_results();
        self.state.metrics.last_poll = Some(t0.elapsed());
        changed
    }

    fn poll_branch_results(&mut self) -> bool {
        let mut changed = false;
        while let Some(result) = self.branch_worker.try_recv() {
            changed = true;
            if let Some(t) = self.state.metrics.branch_request_time.take() {
                self.state.metrics.last_branch_fetch = Some(t.elapsed());
            }
            match result {
                BranchResult::Branches(branches) => {
                    self.state.metrics.last_branch_count = branches.entries.len();
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
            if let Some(t) = self.state.metrics.commit_request_time.take() {
                self.state.metrics.last_commit_fetch = Some(t.elapsed());
            }
            match result {
                CommitResult::Commits(commits) => {
                    self.state.metrics.last_commit_count = commits.len() as u64;
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
            if let Some(t) = self.state.metrics.diff_request_time.take() {
                self.state.metrics.last_diff = Some(t.elapsed());
            }
            match result {
                DiffResult::Diff {
                    commit_info,
                    diff_lines,
                    file_entries,
                } => {
                    self.state.metrics.last_diff_lines = diff_lines.len();
                    self.state.metrics.last_diff_files = file_entries.len();
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
