//! Main event loop and profiling mode.
//!
//! The interactive `run()` method drives the terminal event loop:
//! poll workers, render the UI if dirty, and dispatch input events.
//! The `run_profile()` method is a non-interactive benchmark mode.

use std::time::Instant;

use crate::error::AppError;

use crate::workers::{BranchResult, CommitResult, DiffResult};

use super::render;
use super::App;

impl App {
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
            if let Some(expiry) = self.state.ui.status_expiry {
                if expiry <= Instant::now() {
                    self.state.ui.status_message = None;
                    self.state.ui.status_expiry = None;
                    self.state.ui.dirty = true;
                }
            }
            if self.state.ui.dirty {
                if self.state.ui.needs_terminal_reset {
                    let _ = terminal.clear();
                    self.state.ui.needs_terminal_reset = false;
                }
                let draw_result = terminal.draw(|frame| render::render(self, frame));
                if let Err(e) = draw_result {
                    return Err(format!("Render error: {}", e).into());
                }
                if let Some(start) = frame_start {
                    let frame_dur = start.elapsed();
                    self.state.last_frame_time_ms = frame_dur.as_millis() as u64;
                    self.state.metrics.record_frame(frame_dur);
                }
                self.state.ui.dirty = false;
            }
            match super::input::handle_event(self)? {
                super::input::EventOutcome::Quit => break,
                super::input::EventOutcome::Continue => {}
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
}
