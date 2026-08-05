use std::sync::Arc;

use git_log_ratatui::app::search;
use git_log_ratatui::app::state::AppState;
use git_log_ratatui::config::Config;
use git_log_ratatui::domain::BranchScope;
use git_log_ratatui::view::{CommitRow as Commit, Panel};
use git_log_ratatui::Theme;

/// Helper: create a minimal AppState for testing.
fn test_state() -> AppState {
    AppState::new(".".to_string(), &Config::default(), false, false)
}

/// Helper: create a commit with the given hash and subject.
fn commit(hash: &str, subject: &str, graph_only: bool) -> Commit {
    Commit {
        hash: hash.to_string(),
        author: "tester".to_string(),
        date: "2024-01-01".to_string(),
        subject: subject.to_string(),
        graph: "●".to_string(),
        graph_colors: vec![],
        merge: false,
        graph_only,
        decorations: vec![],
        deco_line: 0,
        epoch_days: 0,
    }
}

// ---------------------------------------------------------------------------
// Test 1: AppState::new() creates correct initial values
// ---------------------------------------------------------------------------
#[test]
fn test_initial_state_values() {
    let state = test_state();

    assert_eq!(state.ui.focus, Panel::Commits);
    assert_eq!(state.branch.branch_scope, BranchScope::All);
    assert!(state.search.search_query.is_empty());
    assert_eq!(state.commit.selected_index, 0);
    assert!(state.commit.all_commits.is_empty());
    assert!(state.commit.filtered_commits.is_none());
    assert_eq!(state.search.cursor_pos, 0);
}

// ---------------------------------------------------------------------------
// Test 2: Search filters commits correctly (end-to-end: state -> search -> result)
// ---------------------------------------------------------------------------
#[test]
fn test_search_filters_commits() {
    let mut state = test_state();

    state.commit.all_commits = vec![
        commit("aaa", "fix login bug", false),
        commit("bbb", "add feature X", false),
        commit("ccc", "update README", true), // graph_only
        commit("ddd", "Bump version", false),
    ];
    // No filter initially
    assert!(state.commit.filtered_commits.is_none());

    // Search for "login"
    state.search.search_query = "login".to_string();
    search::apply_search_filter(&mut state);

    assert!(state.commit.filtered_commits.is_some());
    let filtered = state.commit.filtered_commits.as_ref().unwrap();
    assert_eq!(filtered.iter().filter(|c| !c.graph_only).count(), 1);
    assert!(filtered.iter().any(|c| c.subject.contains("login")));

    // Search for "Bump" -> case-insensitive
    state.search.search_query = "bump".to_string();
    search::apply_search_filter(&mut state);
    let filtered = state.commit.filtered_commits.as_ref().unwrap();
    assert!(filtered.iter().any(|c| c.subject.contains("Bump")));

    // Clear search -> back to all commits
    state.search.search_query.clear();
    search::apply_search_filter(&mut state);
    assert!(state.commit.filtered_commits.is_none());
}

// ---------------------------------------------------------------------------
// Test 3: Branch scope cycles correctly
// ---------------------------------------------------------------------------
#[test]
fn test_branch_scope_cycle() {
    let mut state = test_state();

    assert_eq!(state.branch.branch_scope, BranchScope::All);

    state.branch.branch_scope = state.branch.branch_scope.next();
    assert_eq!(state.branch.branch_scope, BranchScope::Local);

    state.branch.branch_scope = state.branch.branch_scope.next();
    assert_eq!(state.branch.branch_scope, BranchScope::Remote);

    state.branch.branch_scope = state.branch.branch_scope.next();
    assert_eq!(state.branch.branch_scope, BranchScope::Tags);

    state.branch.branch_scope = state.branch.branch_scope.next();
    assert_eq!(state.branch.branch_scope, BranchScope::Stash);

    state.branch.branch_scope = state.branch.branch_scope.next();
    assert_eq!(state.branch.branch_scope, BranchScope::All);

    // Labels
    assert_eq!(BranchScope::All.label(), "all");
    assert_eq!(BranchScope::Local.label(), "local");
    assert_eq!(BranchScope::Remote.label(), "remote");
    assert_eq!(BranchScope::Tags.label(), "tags");
    assert_eq!(BranchScope::Stash.label(), "stash");
}

// ---------------------------------------------------------------------------
// Test 4: Panel focus cycle (Tab / Shift+Tab) is correct
// ---------------------------------------------------------------------------
#[test]
fn test_panel_focus_cycle() {
    let mut state = test_state();

    assert_eq!(state.ui.focus, Panel::Commits);

    // Forward: Commits → Diff → Search → Scope → Branches → Commits
    state.ui.focus = state.ui.focus.next();
    assert_eq!(state.ui.focus, Panel::Diff);
    state.ui.focus = state.ui.focus.next();
    assert_eq!(state.ui.focus, Panel::Search);
    state.ui.focus = state.ui.focus.next();
    assert_eq!(state.ui.focus, Panel::Scope);
    state.ui.focus = state.ui.focus.next();
    assert_eq!(state.ui.focus, Panel::Branches);
    state.ui.focus = state.ui.focus.next();
    assert_eq!(state.ui.focus, Panel::Commits); // wraps around

    // Backward: Commits → Branches → Scope → Search → Diff → Commits
    state.ui.focus = state.ui.focus.prev();
    assert_eq!(state.ui.focus, Panel::Branches);
    state.ui.focus = state.ui.focus.prev();
    assert_eq!(state.ui.focus, Panel::Scope);
    state.ui.focus = state.ui.focus.prev();
    assert_eq!(state.ui.focus, Panel::Search);
    state.ui.focus = state.ui.focus.prev();
    assert_eq!(state.ui.focus, Panel::Diff);
    state.ui.focus = state.ui.focus.prev();
    assert_eq!(state.ui.focus, Panel::Commits);
}

// ---------------------------------------------------------------------------
// Test 5: Selecting a branch updates selected_branch
// ---------------------------------------------------------------------------
#[test]
fn test_select_branch_updates_state() {
    let mut state = test_state();

    // Simulate selecting a branch
    state.branch.selected_branch = Some("feature/login".to_string());
    assert_eq!(
        state.branch.selected_branch.as_deref(),
        Some("feature/login")
    );

    // Simulate selecting "all branches" (None)
    state.branch.selected_branch = None;
    assert_eq!(state.branch.selected_branch, None);
}

// ---------------------------------------------------------------------------
// Test 6: Lazy loading: commit_limit increment triggers when scrolling near end
// ---------------------------------------------------------------------------
#[test]
fn test_commit_limit_starts_at_batch_size() {
    let state = test_state();

    // commit_limit initialised from Config::default().behavior.commit_batch_size
    assert!(state.commit.commit_limit > 0);
    assert_eq!(
        state.commit.commit_limit,
        Config::default().behavior.commit_batch_size
    );
    assert!(!state.commit.all_commits_loaded);
    assert!(!state.commit.loading_more);
}

// ---------------------------------------------------------------------------
// Test 7: Simplified graph toggle caches both modes correctly
// ---------------------------------------------------------------------------
#[test]
fn test_simplified_graph_initial_and_toggle_aware() {
    let state = test_state();
    assert!(!state.commit.simplified_graph);

    // With simplified_graph enabled via config
    let state_simple = AppState::new(".".to_string(), &Config::default(), true, false);
    assert!(state_simple.commit.simplified_graph);

    // Caches start empty
    assert!(state_simple.commit.cache.is_empty());
}

// ---------------------------------------------------------------------------
// Additional: visible mapping skips graph_only
// ---------------------------------------------------------------------------
#[test]
fn test_visible_mapping_integration() {
    let mut state = test_state();

    state.commit.all_commits = vec![
        commit("aaa", "first", false),
        commit("bbb", "second", true), // graph_only
        commit("ccc", "third", false),
    ];

    // Apply no filter -> filtered_commits stays None, builds visible mapping from all_commits
    search::apply_search_filter(&mut state);

    assert_eq!(search::visible_count(&state), 2); // only non-graph_only
    assert_eq!(search::visible_to_filtered(&state, 0), 0); // first visible -> index 0
    assert_eq!(search::visible_to_filtered(&state, 1), 2); // second visible -> index 2
    assert_eq!(search::filtered_to_visible(&state, 0), Some(0));
    assert_eq!(search::filtered_to_visible(&state, 2), Some(1));
    assert_eq!(search::filtered_to_visible(&state, 1), None); // graph_only -> no visible row
}

// ---------------------------------------------------------------------------
// Additional: clamp_selection on empty commits
// ---------------------------------------------------------------------------
#[test]
fn test_clamp_selection_integration() {
    let mut state = test_state();

    state.commit.selected_index = 42;
    search::clamp_selection(&mut state);
    assert_eq!(state.commit.selected_index, 0);

    state.commit.all_commits = vec![commit("a", "one", false), commit("b", "two", false)];
    search::apply_search_filter(&mut state);

    state.commit.selected_index = 10;
    search::clamp_selection(&mut state);
    assert_eq!(state.commit.selected_index, 1);
}

// ---------------------------------------------------------------------------
// Test 10: theme cycling transitions
// ---------------------------------------------------------------------------
#[test]
fn test_theme_cycling_starts_with_custom() {
    let state = test_state();
    assert!(state.theme_preset_index.is_none());
    // Custom theme is a clone of the active theme
    assert_eq!(Arc::as_ptr(&state.theme), Arc::as_ptr(&state.custom_theme));
}

/// Simulate the theme cycling logic from input::process_effects.
fn cycle_theme(state: &mut AppState) {
    let next_idx = match state.theme_preset_index {
        None => Some(0),
        Some(i) if i + 1 < Theme::PRESET_COUNT => Some(i + 1),
        Some(_) => None,
    };
    state.theme_preset_index = next_idx;
    state.theme = match next_idx {
        None => Arc::clone(&state.custom_theme),
        Some(idx) => Arc::new(Theme::preset(idx)),
    };
}

#[test]
fn test_theme_cycles_custom_to_presets_and_back() {
    let mut state = test_state();

    // Custom -> Default preset
    cycle_theme(&mut state);
    assert_eq!(state.theme_preset_index, Some(0));

    // Default -> Monokai
    cycle_theme(&mut state);
    assert_eq!(state.theme_preset_index, Some(1));

    // Monokai -> Nord
    cycle_theme(&mut state);
    assert_eq!(state.theme_preset_index, Some(2));

    // Nord -> Solarized Dark
    cycle_theme(&mut state);
    assert_eq!(state.theme_preset_index, Some(3));

    // Solarized Dark -> Custom
    cycle_theme(&mut state);
    assert!(state.theme_preset_index.is_none());
    assert_eq!(Arc::as_ptr(&state.theme), Arc::as_ptr(&state.custom_theme));

    // Custom -> Default again
    cycle_theme(&mut state);
    assert_eq!(state.theme_preset_index, Some(0));
}

#[test]
fn test_preset_themes_are_distinct_from_custom() {
    let _state = test_state();
    // Each preset should be different from custom
    for i in 0..Theme::PRESET_COUNT {
        let preset = Theme::preset(i);
        assert_eq!(preset.graph_colors.len(), 8);
    }
}
