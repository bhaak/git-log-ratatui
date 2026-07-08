use crate::app::search;
use crate::app::App;
use crate::config::Config;
use crate::view::CommitRow as Commit;
use crate::view::TreeItem;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::app::state::AppState;
use crate::app::workers;
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
    assert_eq!(search::visible_to_filtered(&app.state, 0), 0);
    assert_eq!(search::visible_to_filtered(&app.state, 1), 2);
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
            last_commit_date: None,
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
            last_commit_date: None,
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
            last_commit_date: None,
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

    for cmd in crate::app::input::mouse_click(&app, 19, 5) {
        crate::app::input::execute_command(&mut app, cmd);
    }

    assert_eq!(
        app.state.branch.branch_index, prev_branch_index,
        "branch_index should NOT change on scrollbar click"
    );

    for cmd in crate::app::input::mouse_click(&app, 5, 3) {
        crate::app::input::execute_command(&mut app, cmd);
    }
    assert_eq!(
        app.state.branch.branch_index, 2,
        "branch_index should change on content click"
    );
}
