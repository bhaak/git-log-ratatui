use crate::app::branches::rebuild_branch_tree;
use crate::app::state::AppState;
use crate::config::Config;
use crate::domain::{BranchData, BranchEntry, BranchScope, StashEntry};

fn make_empty_state() -> AppState {
    let config = Config::default();
    AppState::new(".".to_string(), &config, false, false)
}

fn make_state_with_data(data: BranchData, scope: BranchScope) -> AppState {
    let mut state = make_empty_state();
    state.branch.all_branches = data;
    state.branch.branches_loaded = true;
    state.branch.branch_scope = scope;
    state
}

fn make_branch_entry(name: &str, is_remote: bool, date: &str, epoch_days: i64) -> BranchEntry {
    BranchEntry {
        name: name.to_string(),
        is_remote,
        last_commit_date: Some(date.to_string()),
        epoch_days: Some(epoch_days),
    }
}

#[test]
fn test_rebuild_branch_tree_empty_state() {
    let mut state = make_empty_state();
    rebuild_branch_tree(&mut state);
    assert!(state.branch.branch_tree.is_empty());
}

#[test]
fn test_rebuild_branch_tree_only_local_branches() {
    let mut state = make_state_with_data(
        BranchData {
            default_branch: Some("main".to_string()),
            entries: vec![
                make_branch_entry("main", false, "2024-01-01 12:00", 20000),
                make_branch_entry("feature/login", false, "2024-01-02 12:00", 20001),
            ],
            tags: vec![],
            tags_dates: vec![],
            stashes: vec![],
        },
        BranchScope::All,
    );

    rebuild_branch_tree(&mut state);
    let tree = &state.branch.branch_tree;

    assert!(!tree.is_empty(), "tree should not be empty");

    // Section header
    assert_eq!(tree[0].name, "Local Branches");
    assert_eq!(tree[0].depth, 0);
    assert!(!tree[0].is_branch);
    assert_eq!(tree[0].key, "__local__");

    // Local branch items exist
    let branch_paths: Vec<&str> = tree
        .iter()
        .filter(|i| i.is_branch)
        .map(|i| i.full_path.as_str())
        .collect();
    assert!(branch_paths.contains(&"main"));
    assert!(branch_paths.contains(&"feature/login"));

    // Default branch should be first
    let first_branch = tree.iter().find(|i| i.is_branch).unwrap();
    assert_eq!(first_branch.full_path, "main");
}

#[test]
fn test_rebuild_branch_tree_local_branches_with_dates() {
    let mut state = make_state_with_data(
        BranchData {
            default_branch: None,
            entries: vec![make_branch_entry("dev", false, "2024-06-01 10:00", 21234)],
            tags: vec![],
            tags_dates: vec![],
            stashes: vec![],
        },
        BranchScope::All,
    );

    rebuild_branch_tree(&mut state);
    let tree = &state.branch.branch_tree;

    let branch = tree.iter().find(|i| i.full_path == "dev").unwrap();
    assert_eq!(
        branch.last_commit_date,
        Some("2024-06-01 10:00".to_string())
    );
    assert_eq!(branch.epoch_days, Some(21234));
}

#[test]
fn test_rebuild_branch_tree_only_remote_branches() {
    let mut state = make_state_with_data(
        BranchData {
            default_branch: None,
            entries: vec![
                make_branch_entry("origin/main", true, "2024-01-01 12:00", 20000),
                make_branch_entry("origin/feature/foo", true, "2024-01-02 12:00", 20001),
            ],
            tags: vec![],
            tags_dates: vec![],
            stashes: vec![],
        },
        BranchScope::All,
    );

    rebuild_branch_tree(&mut state);
    let tree = &state.branch.branch_tree;

    assert_eq!(tree[0].name, "Remote Branches");
    assert_eq!(tree[0].key, "__remote__");

    let branch_paths: Vec<&str> = tree
        .iter()
        .filter(|i| i.is_branch)
        .map(|i| i.full_path.as_str())
        .collect();
    assert!(branch_paths.contains(&"origin/main"));
    assert!(branch_paths.contains(&"origin/feature/foo"));
}

#[test]
fn test_rebuild_branch_tree_tags() {
    let mut state = make_state_with_data(
        BranchData {
            default_branch: None,
            entries: vec![],
            tags: vec!["v1.0".to_string(), "v1.1".to_string()],
            tags_dates: vec![
                Some(("2024-03-01 08:00".to_string(), 20100)),
                Some(("2024-04-01 08:00".to_string(), 20131)),
            ],
            stashes: vec![],
        },
        BranchScope::All,
    );

    rebuild_branch_tree(&mut state);
    let tree = &state.branch.branch_tree;

    assert_eq!(tree[0].name, "Tags");
    assert_eq!(tree[0].key, "__tags__");

    let tag_v10 = tree
        .iter()
        .find(|i| i.full_path == "refs/tags/v1.0")
        .unwrap();
    assert_eq!(
        tag_v10.last_commit_date,
        Some("2024-03-01 08:00".to_string())
    );
    assert_eq!(tag_v10.epoch_days, Some(20100));

    let tag_v11 = tree
        .iter()
        .find(|i| i.full_path == "refs/tags/v1.1")
        .unwrap();
    assert_eq!(
        tag_v11.last_commit_date,
        Some("2024-04-01 08:00".to_string())
    );
    assert_eq!(tag_v11.epoch_days, Some(20131));
}

#[test]
fn test_rebuild_branch_tree_stashes() {
    let mut state = make_state_with_data(
        BranchData {
            default_branch: None,
            entries: vec![],
            tags: vec![],
            tags_dates: vec![],
            stashes: vec![
                StashEntry {
                    index: 0,
                    message: "WIP login".to_string(),
                    oid: "abc123".to_string(),
                    last_commit_date: Some("2024-05-01 09:00".to_string()),
                    epoch_days: Some(20200),
                },
                StashEntry {
                    index: 1,
                    message: "WIP signup".to_string(),
                    oid: "def456".to_string(),
                    last_commit_date: Some("2024-05-02 09:00".to_string()),
                    epoch_days: Some(20201),
                },
            ],
        },
        BranchScope::All,
    );

    rebuild_branch_tree(&mut state);
    let tree = &state.branch.branch_tree;

    assert_eq!(tree[0].name, "Stashes");
    assert_eq!(tree[0].key, "__stashes__");

    let stash_items: Vec<_> = tree
        .iter()
        .filter(|i| i.full_path.starts_with("stash@"))
        .collect();
    assert_eq!(stash_items.len(), 2);
    assert_eq!(stash_items[0].full_path, "stash@{0}");
    assert_eq!(stash_items[0].key, "abc123");
    assert_eq!(
        stash_items[0].last_commit_date,
        Some("2024-05-01 09:00".to_string())
    );
    assert_eq!(stash_items[1].full_path, "stash@{1}");
    assert_eq!(stash_items[1].key, "def456");
}

#[test]
fn test_rebuild_branch_tree_scope_local_only() {
    let mut state = make_state_with_data(
        BranchData {
            default_branch: None,
            entries: vec![
                make_branch_entry("main", false, "2024-01-01 12:00", 20000),
                make_branch_entry("origin/main", true, "2024-01-01 12:00", 20000),
            ],
            tags: vec!["v1.0".to_string()],
            tags_dates: vec![Some(("2024-03-01 08:00".to_string(), 20100))],
            stashes: vec![StashEntry {
                index: 0,
                message: String::new(),
                oid: "abc".to_string(),
                last_commit_date: None,
                epoch_days: None,
            }],
        },
        BranchScope::Local,
    );

    rebuild_branch_tree(&mut state);
    let tree = &state.branch.branch_tree;

    assert!(tree.iter().any(|i| i.key == "__local__"));
    assert!(!tree.iter().any(|i| i.key == "__remote__"));
    assert!(!tree.iter().any(|i| i.key == "__tags__"));
    assert!(!tree.iter().any(|i| i.key == "__stashes__"));
    assert!(tree.iter().any(|i| i.full_path == "main"));
}

#[test]
fn test_rebuild_branch_tree_scope_remote_only() {
    let mut state = make_state_with_data(
        BranchData {
            default_branch: None,
            entries: vec![
                make_branch_entry("main", false, "2024-01-01 12:00", 20000),
                make_branch_entry("origin/main", true, "2024-01-01 12:00", 20000),
            ],
            tags: vec![],
            tags_dates: vec![],
            stashes: vec![],
        },
        BranchScope::Remote,
    );

    rebuild_branch_tree(&mut state);
    let tree = &state.branch.branch_tree;

    assert!(!tree.iter().any(|i| i.key == "__local__"));
    assert!(tree.iter().any(|i| i.key == "__remote__"));
    assert!(tree.iter().any(|i| i.full_path == "origin/main"));
}

#[test]
fn test_rebuild_branch_tree_scope_tags_only() {
    let mut state = make_state_with_data(
        BranchData {
            default_branch: None,
            entries: vec![make_branch_entry("main", false, "2024-01-01 12:00", 20000)],
            tags: vec!["v1.0".to_string()],
            tags_dates: vec![Some(("2024-03-01 08:00".to_string(), 20100))],
            stashes: vec![],
        },
        BranchScope::Tags,
    );

    rebuild_branch_tree(&mut state);
    let tree = &state.branch.branch_tree;

    assert!(!tree.iter().any(|i| i.key == "__local__"));
    assert!(!tree.iter().any(|i| i.key == "__remote__"));
    assert!(tree.iter().any(|i| i.key == "__tags__"));
    assert!(!tree.iter().any(|i| i.key == "__stashes__"));
    assert!(tree.iter().any(|i| i.full_path == "refs/tags/v1.0"));
}

#[test]
fn test_rebuild_branch_tree_scope_stash_only() {
    let mut state = make_state_with_data(
        BranchData {
            default_branch: None,
            entries: vec![make_branch_entry("main", false, "2024-01-01 12:00", 20000)],
            tags: vec![],
            tags_dates: vec![],
            stashes: vec![StashEntry {
                index: 0,
                message: String::new(),
                oid: "abc".to_string(),
                last_commit_date: None,
                epoch_days: None,
            }],
        },
        BranchScope::Stash,
    );

    rebuild_branch_tree(&mut state);
    let tree = &state.branch.branch_tree;

    assert!(!tree.iter().any(|i| i.key == "__local__"));
    assert!(!tree.iter().any(|i| i.key == "__remote__"));
    assert!(!tree.iter().any(|i| i.key == "__tags__"));
    assert!(tree.iter().any(|i| i.key == "__stashes__"));
    assert!(tree.iter().any(|i| i.full_path == "stash@{0}"));
}

#[test]
fn test_rebuild_branch_tree_collapsed_section_shows_only_header() {
    let mut state = make_state_with_data(
        BranchData {
            default_branch: None,
            entries: vec![make_branch_entry("main", false, "2024-01-01 12:00", 20000)],
            tags: vec!["v1.0".to_string()],
            tags_dates: vec![Some(("2024-03-01 08:00".to_string(), 20100))],
            stashes: vec![StashEntry {
                index: 0,
                message: "test".to_string(),
                oid: "abc".to_string(),
                last_commit_date: None,
                epoch_days: None,
            }],
        },
        BranchScope::All,
    );

    // Collapse all sections
    state
        .branch
        .expanded_nodes
        .insert("__local__".to_string(), false);
    state
        .branch
        .expanded_nodes
        .insert("__tags__".to_string(), false);
    state
        .branch
        .expanded_nodes
        .insert("__stashes__".to_string(), false);

    rebuild_branch_tree(&mut state);
    let tree = &state.branch.branch_tree;

    // Only section headers, no branch/tag/stash children
    assert!(tree.iter().all(|i| i.depth == 0 && !i.is_branch));
    assert_eq!(tree.len(), 3); // local header, tags header, stashes header
}

#[test]
fn test_rebuild_branch_tree_preserves_expanded_state() {
    let mut state = make_state_with_data(
        BranchData {
            default_branch: None,
            entries: vec![make_branch_entry("main", false, "2024-01-01 12:00", 20000)],
            tags: vec![],
            tags_dates: vec![],
            stashes: vec![],
        },
        BranchScope::All,
    );

    // Explicitly expand local section
    state
        .branch
        .expanded_nodes
        .insert("__local__".to_string(), true);

    rebuild_branch_tree(&mut state);
    let tree = &state.branch.branch_tree;

    let local_header = tree.iter().find(|i| i.key == "__local__").unwrap();
    assert!(local_header.expanded);
}

#[test]
fn test_rebuild_branch_tree_default_branch_first() {
    let mut state = make_state_with_data(
        BranchData {
            default_branch: Some("main".to_string()),
            entries: vec![
                make_branch_entry("develop", false, "2024-01-01 12:00", 20000),
                make_branch_entry("main", false, "2024-01-01 12:00", 20000),
                make_branch_entry("feature/x", false, "2024-01-01 12:00", 20000),
            ],
            tags: vec![],
            tags_dates: vec![],
            stashes: vec![],
        },
        BranchScope::All,
    );

    rebuild_branch_tree(&mut state);
    let tree = &state.branch.branch_tree;

    let branch_paths: Vec<&str> = tree
        .iter()
        .filter(|i| i.is_branch && i.depth == 1)
        .map(|i| i.full_path.as_str())
        .collect();

    // main should be first in the list of direct children
    assert_eq!(branch_paths[0], "main");
}

#[test]
fn test_rebuild_branch_tree_empty_section_not_shown() {
    // No remote branches, so remote section should not appear
    let mut state = make_state_with_data(
        BranchData {
            default_branch: None,
            entries: vec![make_branch_entry("main", false, "2024-01-01 12:00", 20000)],
            tags: vec![],
            tags_dates: vec![],
            stashes: vec![],
        },
        BranchScope::All,
    );

    rebuild_branch_tree(&mut state);
    let tree = &state.branch.branch_tree;

    assert!(!tree.iter().any(|i| i.key == "__remote__"));
    assert!(!tree.iter().any(|i| i.key == "__tags__"));
    assert!(!tree.iter().any(|i| i.key == "__stashes__"));
    assert!(tree.iter().any(|i| i.key == "__local__"));
}
