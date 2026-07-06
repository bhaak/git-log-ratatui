use crate::app::state::AppState;
use crate::app::COMMIT_LIMIT_INCREMENT;
use crate::workers::{CommitCommand, CommitWorker};

/// Raise the commit limit and reload, appending older commits.
/// Triggered when the user scrolls near the end of the loaded commits.
pub(crate) fn request_more_commits(state: &mut AppState, commit_worker: &CommitWorker) {
    state.commit.commits_loaded = false;
    state.commit.loading_more = true;
    state.commit.commit_limit = state
        .commit
        .commit_limit
        .saturating_add(COMMIT_LIMIT_INCREMENT);
    if !commit_worker.send(CommitCommand::FetchCommits {
        branch: state.branch.selected_branch.clone(),
        scope: state.branch.branch_scope,
        limit: Some(state.commit.commit_limit),
        simplified: state.commit.simplified_graph,
    }) {
        state.ui.status_message = Some("Commit worker disconnected — restart required".to_string());
    }
}
