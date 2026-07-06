use crate::app::state::AppState;

pub fn apply_search_filter(state: &mut AppState) {
    if state.search_query.is_empty() {
        state.filtered_commits = None;
    } else {
        let q = state.search_query.to_lowercase();
        state.filtered_commits = Some(
            state
                .all_commits
                .iter()
                .filter(|c| {
                    c.graph_only
                        || c.hash.to_lowercase().contains(&q)
                        || c.author.to_lowercase().contains(&q)
                        || c.date.to_lowercase().contains(&q)
                        || c.subject.to_lowercase().contains(&q)
                })
                .cloned()
                .collect(),
        );
    }
    build_visible_mapping(state);
    state.selected_index = 0;
    clamp_selection(state);
}

pub fn reapply_filter_preserving_selection(state: &mut AppState) {
    let prev = state.selected_index;
    apply_search_filter(state);
    state.selected_index = prev;
    clamp_selection(state);
}

pub fn build_visible_mapping(state: &mut AppState) {
    let commits = state
        .filtered_commits
        .as_deref()
        .unwrap_or(&state.all_commits);
    state.visible_to_commit = (0..commits.len())
        .filter(|&i| !commits[i].graph_only)
        .collect();
}

pub fn visible_count(state: &AppState) -> usize {
    state.visible_to_commit.len()
}

pub fn visible_to_filtered(state: &AppState, visible_idx: usize) -> usize {
    state
        .visible_to_commit
        .get(visible_idx)
        .copied()
        .unwrap_or(0)
}

pub fn filtered_to_visible(state: &AppState, filtered_idx: usize) -> Option<usize> {
    state
        .visible_to_commit
        .iter()
        .position(|&i| i == filtered_idx)
}

pub fn clamp_selection(state: &mut AppState) {
    if visible_count(state) > 0 {
        state.selected_index = state
            .selected_index
            .min(visible_count(state).saturating_sub(1));
    } else {
        state.selected_index = 0;
    }
}
