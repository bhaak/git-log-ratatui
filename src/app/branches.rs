use crate::app::state::AppState;
use crate::domain::BranchScope;
use crate::tree;
use crate::view::TreeItem;

pub(crate) fn rebuild_branch_tree(state: &mut AppState) {
    let local_section_key = "__local__";
    let remote_section_key = "__remote__";
    let tags_section_key = "__tags__";

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

    let tags_item = TreeItem {
        name: "Tags".to_string(),
        depth: 0,
        expandable: true,
        expanded: tags_expanded,
        is_branch: false,
        full_path: String::new(),
        tree_prefix: String::new(),
        key: tags_section_key.to_string(),
    };

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

    // Local branches section
    if !local_names.is_empty() {
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
            let branch_items = tree::flatten_tree(&local_root, 0, &state.branch.expanded_nodes);
            items.extend(branch_items);
        }
    }

    // Remote branches section
    if !remote_names.is_empty() {
        items.push(remote_item);

        // Build and sort remote tree
        let mut remote_root = tree::build_branch_tree(&remote_names);
        tree::sort_tree(&mut remote_root);

        if remote_expanded {
            let branch_items = tree::flatten_tree(&remote_root, 0, &state.branch.expanded_nodes);
            items.extend(branch_items);
        }
    }

    // Tags section (only shown when scope is All)
    if state.branch.branch_scope == BranchScope::All && !state.branch.all_branches.tags.is_empty() {
        items.push(tags_item);

        let mut tag_root = tree::build_branch_tree(&state.branch.all_branches.tags);
        tree::sort_tree(&mut tag_root);

        if tags_expanded {
            let mut tag_items = tree::flatten_tree(&tag_root, 0, &state.branch.expanded_nodes);
            // Prefix tag full_paths with refs/tags/ to avoid collisions with same-named branches
            for item in &mut tag_items {
                if item.is_branch {
                    item.full_path = format!("refs/tags/{}", item.full_path);
                }
            }
            items.extend(tag_items);
        }
    }

    state.branch.branch_tree = items;
}
