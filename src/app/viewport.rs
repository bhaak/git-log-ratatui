use crate::app::state::AppState;
use crate::app::COMMIT_LIMIT_INCREMENT;
use crate::workers::{CommitCommand, CommitWorker};

/// Raise the commit limit and reload, appending older commits.
/// Triggered when the user scrolls near the end of the loaded commits.
pub(crate) fn request_more_commits(state: &mut AppState, commit_worker: &CommitWorker) {
    state.commits_loaded = false;
    state.loading_more = true;
    state.commit_limit = state.commit_limit.saturating_add(COMMIT_LIMIT_INCREMENT);
    if !commit_worker.send(CommitCommand::FetchCommits {
        branch: state.selected_branch.clone(),
        scope: state.branch_scope,
        limit: Some(state.commit_limit),
        simplified: state.simplified_graph,
    }) {
        state.status_message = Some("Commit worker disconnected — restart required".to_string());
    }
}
