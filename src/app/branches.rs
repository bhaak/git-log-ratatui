use crate::app::state::AppState;
use crate::models::*;
use crate::tree;

pub(crate) fn rebuild_branch_tree(state: &mut AppState) {
    let local_section_key = "__local__";
    let remote_section_key = "__remote__";

    let local_expanded = state
        .expanded_nodes
        .get(local_section_key)
        .copied()
        .unwrap_or(true);
    let remote_expanded = state
        .expanded_nodes
        .get(remote_section_key)
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
    };

    // Separate local and remote branch names
    let local_names: Vec<String> = state
        .all_branches
        .entries
        .iter()
        .filter(|e| !e.is_remote)
        .map(|e| e.name.clone())
        .collect();

    let remote_names: Vec<String> = state
        .all_branches
        .entries
        .iter()
        .filter(|e| e.is_remote)
        .map(|e| e.name.clone())
        .collect();

    let mut items = vec![local_item];

    // Build and sort local tree
    let mut local_root = tree::build_branch_tree(&local_names);
    tree::sort_tree(&mut local_root);

    // Move default branch to the front of local children
    if let Some(ref default) = state.all_branches.default_branch {
        if let Some(pos) = local_root.children.iter().position(|c| c.name == *default) {
            let default_child = local_root.children.remove(pos);
            local_root.children.insert(0, default_child);
        }
    }

    if local_expanded {
        let branch_items = tree::flatten_tree(&local_root, 0, &state.expanded_nodes);
        items.extend(branch_items);
    }

    items.push(remote_item);

    // Build and sort remote tree
    let mut remote_root = tree::build_branch_tree(&remote_names);
    tree::sort_tree(&mut remote_root);

    if remote_expanded {
        let branch_items = tree::flatten_tree(&remote_root, 0, &state.expanded_nodes);
        items.extend(branch_items);
    }

    state.branch_tree = items;
}
