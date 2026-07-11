use crate::config::Config;
use crate::error::AppError;

use crate::workers;
use crate::workers::{BranchWorker, CommitWorker, DiffWorker};
use state::AppState;

pub mod branches;
pub mod cache;
pub mod commands;
mod input;
pub mod metrics;
mod render;
mod runner;
pub mod search;
pub mod state;
mod viewport;
mod worker_mgr;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

pub(crate) const PAGE_SIZE: usize = 10;
pub(super) const INITIAL_COMMIT_LIMIT: usize = 5000;
pub(crate) const COMMIT_LIMIT_INCREMENT: usize = 5000;
pub(crate) const POLL_INTERVAL_DEFAULT: u8 = 10;
pub(crate) const POLL_INTERVAL_MAX: u8 = 200;
pub(crate) const POLL_BACKOFF_STEP: u8 = 10;

pub struct App {
    pub state: AppState,
    pub(crate) branch_worker: BranchWorker,
    pub(crate) commit_worker: CommitWorker,
    pub(crate) diff_worker: DiffWorker,
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
}
