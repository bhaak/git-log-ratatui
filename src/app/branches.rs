use std::collections::HashMap;

use crate::app::state::AppState;
use crate::domain::BranchScope;
use crate::tree;
use crate::view::TreeItem;

pub(crate) fn rebuild_branch_tree(state: &mut AppState) {
    let local_section_key = "__local__";
    let remote_section_key = "__remote__";
    let tags_section_key = "__tags__";
    let stashes_section_key = "__stashes__";

    let local_expanded = state
        .branch
        .expanded_nodes
        .get(local_section_key)
        .copied()
        .unwrap_or(true);
    let remote_expanded = state
        .branch
        .expanded_nodes
        .get(remote_section_key)
        .copied()
        .unwrap_or(true);
    let tags_expanded = state
        .branch
        .expanded_nodes
        .get(tags_section_key)
        .copied()
        .unwrap_or(true);
    let stashes_expanded = state
        .branch
        .expanded_nodes
        .get(stashes_section_key)
        .copied()
        .unwrap_or(true);

    let local_item = TreeItem {
        name: "Local Branches".to_string(),
        depth: 0,
        expandable: true,
        expanded: local_expanded,
        is_branch: false,
        full_path: String::new(),
        tree_prefix: String::new(),
        key: local_section_key.to_string(),
        last_commit_date: None,
        epoch_days: None,
    };

    let remote_item = TreeItem {
        name: "Remote Branches".to_string(),
        depth: 0,
        expandable: true,
        expanded: remote_expanded,
        is_branch: false,
        full_path: String::new(),
        tree_prefix: String::new(),
        key: remote_section_key.to_string(),
        last_commit_date: None,
        epoch_days: None,
    };

    let tags_item = TreeItem {
        name: "Tags".to_string(),
        depth: 0,
        expandable: true,
        expanded: tags_expanded,
        is_branch: false,
        full_path: String::new(),
        tree_prefix: String::new(),
        key: tags_section_key.to_string(),
        last_commit_date: None,
        epoch_days: None,
    };

    let stashes_item = TreeItem {
        name: "Stashes".to_string(),
        depth: 0,
        expandable: true,
        expanded: stashes_expanded,
        is_branch: false,
        full_path: String::new(),
        tree_prefix: String::new(),
        key: stashes_section_key.to_string(),
        last_commit_date: None,
        epoch_days: None,
    };

    // Build name → date lookup maps
    let date_map: HashMap<String, Option<String>> = state
        .branch
        .all_branches
        .entries
        .iter()
        .map(|e| (e.name.clone(), e.last_commit_date.clone()))
        .collect();

    // Build name → epoch_days lookup map for staleness coloring
    let epoch_map: HashMap<String, Option<i64>> = state
        .branch
        .all_branches
        .entries
        .iter()
        .map(|e| (e.name.clone(), e.epoch_days))
        .collect();

    let tag_date_map: HashMap<&str, Option<String>> = state
        .branch
        .all_branches
        .tags
        .iter()
        .zip(state.branch.all_branches.tags_dates.iter())
        .map(|(t, d)| (t.as_str(), d.as_ref().map(|(s, _)| s.clone())))
        .collect();

    let tag_epoch_map: HashMap<&str, Option<i64>> = state
        .branch
        .all_branches
        .tags
        .iter()
        .zip(state.branch.all_branches.tags_dates.iter())
        .map(|(t, d)| (t.as_str(), d.as_ref().map(|(_, e)| *e)))
        .collect();

    // Separate local and remote branch names
    let local_names: Vec<String> = state
        .branch
        .all_branches
        .entries
        .iter()
        .filter(|e| !e.is_remote)
        .map(|e| e.name.clone())
        .collect();

    let remote_names: Vec<String> = state
        .branch
        .all_branches
        .entries
        .iter()
        .filter(|e| e.is_remote)
        .map(|e| e.name.clone())
        .collect();

    let mut items = Vec::new();

    let show_local = matches!(
        state.branch.branch_scope,
        BranchScope::All | BranchScope::Local
    );
    let show_remote = matches!(
        state.branch.branch_scope,
        BranchScope::All | BranchScope::Remote
    );
    let show_tags = matches!(
        state.branch.branch_scope,
        BranchScope::All | BranchScope::Tags
    );
    let show_stashes = matches!(
        state.branch.branch_scope,
        BranchScope::All | BranchScope::Stash
    );

    // Local branches section (All or Local scope)
    if show_local && !local_names.is_empty() {
        items.push(local_item);

        // Build and sort local tree
        let mut local_root = tree::build_branch_tree(&local_names);
        tree::sort_tree(&mut local_root);

        // Move default branch to the front of local children
        if let Some(ref default) = state.branch.all_branches.default_branch {
            if let Some(pos) = local_root.children.iter().position(|c| c.name == *default) {
                let default_child = local_root.children.remove(pos);
                local_root.children.insert(0, default_child);
            }
        }

        if local_expanded {
            let mut branch_items = tree::flatten_tree(&local_root, 0, &state.branch.expanded_nodes);
            for item in &mut branch_items {
                item.last_commit_date = date_map.get(&item.full_path).and_then(|d| d.clone());
                item.epoch_days = epoch_map.get(&item.full_path).and_then(|d| *d);
            }
            items.extend(branch_items);
        }
    }

    // Remote branches section (All or Remote scope)
    if show_remote && !remote_names.is_empty() {
        items.push(remote_item);

        // Build and sort remote tree
        let mut remote_root = tree::build_branch_tree(&remote_names);
        tree::sort_tree(&mut remote_root);

        if remote_expanded {
            let mut branch_items =
                tree::flatten_tree(&remote_root, 0, &state.branch.expanded_nodes);
            for item in &mut branch_items {
                item.last_commit_date = date_map.get(&item.full_path).and_then(|d| d.clone());
                item.epoch_days = epoch_map.get(&item.full_path).and_then(|d| *d);
            }
            items.extend(branch_items);
        }
    }

    // Tags section (All or Tags scope)
    if show_tags && !state.branch.all_branches.tags.is_empty() {
        items.push(tags_item);

        let mut tag_root = tree::build_branch_tree(&state.branch.all_branches.tags);
        tree::sort_tree(&mut tag_root);

        if tags_expanded {
            let mut tag_items = tree::flatten_tree(&tag_root, 0, &state.branch.expanded_nodes);
            // Prefix tag full_paths with refs/tags/ to avoid collisions and set dates
            for item in &mut tag_items {
                if item.is_branch {
                    let tag_name = item.full_path.clone();
                    item.full_path = format!("refs/tags/{}", tag_name);
                }
                item.last_commit_date = tag_date_map
                    .get(
                        item.full_path
                            .strip_prefix("refs/tags/")
                            .unwrap_or(&item.full_path),
                    )
                    .and_then(|d| d.clone());
                item.epoch_days = tag_epoch_map
                    .get(
                        item.full_path
                            .strip_prefix("refs/tags/")
                            .unwrap_or(&item.full_path),
                    )
                    .and_then(|d| *d);
            }
            items.extend(tag_items);
        }
    }

    // Stashes section (All or Stash scope)
    if show_stashes && !state.branch.all_branches.stashes.is_empty() {
        items.push(stashes_item);

        if stashes_expanded {
            let stash_count = state.branch.all_branches.stashes.len();
            for (i, stash) in state.branch.all_branches.stashes.iter().enumerate() {
                let is_last = i == stash_count - 1;
                let connector = if is_last {
                    "\u{2514}\u{2500}"
                } else {
                    "\u{251C}\u{2500}"
                };
                let stash_name = if stash.message.is_empty() {
                    format!("stash@{{{}}}", stash.index)
                } else {
                    format!("stash@{{{}}}: {}", stash.index, stash.message)
                };
                let display_name = format!("{}  {}", connector, stash_name);
                items.push(TreeItem {
                    name: display_name,
                    depth: 1,
                    expandable: false,
                    expanded: false,
                    is_branch: true,
                    full_path: format!("stash@{{{}}}", stash.index),
                    tree_prefix: format!("{} ", connector),
                    key: stash.oid.clone(),
                    last_commit_date: stash.last_commit_date.clone(),
                    epoch_days: stash.epoch_days,
                });
            }
        }
    }

    state.branch.branch_tree = items;
}
