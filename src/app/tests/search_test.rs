use crate::app::search::{
    build_visible_mapping, clamp_selection, filtered_to_visible,
    reapply_filter_preserving_selection, visible_count, visible_to_filtered,
};
use crate::app::state::AppState;
use crate::config::Config;

fn test_state() -> AppState {
    AppState::new(".".to_string(), &Config::default(), false, false)
}

#[test]
fn test_visible_count_empty() {
    let state = test_state();
    assert_eq!(visible_count(&state), 0);
}

#[test]
fn test_visible_count_only_graph_only() {
    let mut state = test_state();
    let g = crate::view::CommitRow::from_commit(
        crate::domain::Commit {
            hash: "a".into(),
            author: String::new(),
            date: String::new(),
            subject: String::new(),
            merge: false,
            decorations: vec![],
        },
        "".into(),
        vec![],
        true,
        0,
    );
    state.commit.all_commits = vec![g];
    build_visible_mapping(&mut state);
    assert_eq!(visible_count(&state), 0);
}

#[test]
fn test_visible_count_with_real_commits() {
    let mut state = test_state();
    let c1 = crate::view::CommitRow::from_commit(
        crate::domain::Commit {
            hash: "a".into(),
            author: String::new(),
            date: String::new(),
            subject: "first".into(),
            merge: false,
            decorations: vec![],
        },
        "".into(),
        vec![],
        false,
        0,
    );
    let c2 = crate::view::CommitRow::from_commit(
        crate::domain::Commit {
            hash: "b".into(),
            author: String::new(),
            date: String::new(),
            subject: "second".into(),
            merge: false,
            decorations: vec![],
        },
        "".into(),
        vec![],
        false,
        0,
    );
    state.commit.all_commits = vec![c1, c2];
    build_visible_mapping(&mut state);
    assert_eq!(visible_count(&state), 2);
}

#[test]
fn test_visible_to_filtered_in_range() {
    let mut state = test_state();
    state.commit.visible_to_commit = vec![1, 3, 5];
    assert_eq!(visible_to_filtered(&state, 0), 1);
    assert_eq!(visible_to_filtered(&state, 1), 3);
    assert_eq!(visible_to_filtered(&state, 2), 5);
}

#[test]
fn test_visible_to_filtered_out_of_bounds_returns_zero() {
    let mut state = test_state();
    state.commit.visible_to_commit = vec![1];
    assert_eq!(visible_to_filtered(&state, 5), 0);
}

#[test]
fn test_filtered_to_visible_found() {
    let mut state = test_state();
    state.commit.visible_to_commit = vec![1, 3, 5];
    assert_eq!(filtered_to_visible(&state, 3), Some(1));
}

#[test]
fn test_filtered_to_visible_not_found() {
    let mut state = test_state();
    state.commit.visible_to_commit = vec![1, 3, 5];
    assert_eq!(filtered_to_visible(&state, 2), None);
}

#[test]
fn test_filtered_to_visible_empty_mapping() {
    let state = test_state();
    assert_eq!(filtered_to_visible(&state, 0), None);
}

#[test]
fn test_clamp_selection_within_range() {
    let mut state = test_state();
    state.commit.visible_to_commit = vec![0, 1, 2];
    state.commit.selected_index = 1;
    clamp_selection(&mut state);
    assert_eq!(state.commit.selected_index, 1);
}

#[test]
fn test_clamp_selection_beyond_range() {
    let mut state = test_state();
    state.commit.visible_to_commit = vec![0, 1, 2];
    state.commit.selected_index = 42;
    clamp_selection(&mut state);
    assert_eq!(state.commit.selected_index, 2);
}

#[test]
fn test_clamp_selection_empty_visible() {
    let mut state = test_state();
    state.commit.selected_index = 5;
    clamp_selection(&mut state);
    assert_eq!(state.commit.selected_index, 0);
}

#[test]
fn test_reapply_filter_preserves_selection() {
    let mut state = test_state();
    let c = crate::view::CommitRow::from_commit(
        crate::domain::Commit {
            hash: "a".into(),
            author: String::new(),
            date: String::new(),
            subject: "test".into(),
            merge: false,
            decorations: vec![],
        },
        "".into(),
        vec![],
        false,
        0,
    );
    state.commit.all_commits = vec![c];
    state.commit.selected_index = 0;
    build_visible_mapping(&mut state);
    reapply_filter_preserving_selection(&mut state);
    assert_eq!(state.commit.selected_index, 0);
}
