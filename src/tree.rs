/// Branch tree building logic.
/// Constructs a hierarchical tree from flat branch names
/// and flattens it into a display list with Unicode tree connectors.

use crate::models::*;
use std::collections::BTreeMap;

/// Build a hierarchical tree from a flat list of branch names.
pub fn build_branch_tree(branches: &[String]) -> BranchNode {
    let mut root = BranchNode {
        name: "".to_string(),
        full_path: "".to_string(),
        children: Vec::new(),
    };

    for branch_name in branches {
        let parts: Vec<&str> = branch_name.split('/').collect();
        insert_into_tree(&mut root, &parts, branch_name);
    }

    root
}

/// Insert a branch path into the tree, creating intermediate nodes as needed.
fn insert_into_tree(node: &mut BranchNode, parts: &[&str], full_path: &str) {
    if parts.is_empty() {
        return;
    }

    let name = parts[0].to_string();

    // Find or create the child node
    let child_idx = node.children.iter().position(|c| c.name == name);

    if parts.len() == 1 {
        // Leaf node (actual branch)
        if let Some(idx) = child_idx {
            node.children[idx].full_path = full_path.to_string();
        } else {
            node.children.push(BranchNode {
                name,
                full_path: full_path.to_string(),
                children: Vec::new(),
            });
        }
    } else {
        // Intermediate node
        if let Some(idx) = child_idx {
            insert_into_tree(&mut node.children[idx], &parts[1..], full_path);
        } else {
            let mut new_node = BranchNode {
                name,
                full_path: "".to_string(),
                children: Vec::new(),
            };
            insert_into_tree(&mut new_node, &parts[1..], full_path);
            node.children.push(new_node);
        }
    }
}

/// Sort tree children: main/master first, then alphabetical.
pub fn sort_tree(node: &mut BranchNode) {
    node.children.sort_by(|a, b| {
        let a_prio = priority_order(&a.name);
        let b_prio = priority_order(&b.name);
        a_prio
            .cmp(&b_prio)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    for child in &mut node.children {
        sort_tree(child);
    }
}

fn priority_order(name: &str) -> u8 {
    match name.to_lowercase().as_str() {
        "main" => 0,
        "master" => 1,
        _ => 2,
    }
}

/// Flatten the tree into a display list, respecting expanded state.
pub fn flatten_tree(
    node: &BranchNode,
    depth: usize,
    expanded: &BTreeMap<String, bool>,
) -> Vec<TreeItem> {
    let mut items = Vec::new();

    // Build the tree prefix
    let prefix = if depth == 0 {
        String::new()
    } else {
        build_tree_prefix(depth)
    };

    let expandable = !node.children.is_empty();
    let is_expanded = expanded
        .get(&node.full_path)
        .copied()
        .unwrap_or(expandable);

    let marker = if expandable {
        if is_expanded { "▼" } else { "▶" }
    } else {
        "  "
    };

    // Show display name
    let display_name = if depth == 0 {
        String::new()
    } else if node.full_path.is_empty() {
        format!("{}{} {}/", prefix, marker, node.name)
    } else {
        format!("{}{} {}", prefix, marker, node.name)
    };

    if depth > 0 {
        items.push(TreeItem {
            name: display_name,
            depth,
            expandable,
            expanded: is_expanded,
            is_branch: !node.full_path.is_empty(),
            full_path: node.full_path.clone(),
            tree_prefix: prefix.clone(),
        });
    }

    // Recurse into children if expanded
    if is_expanded {
        for child in &node.children {
            items.extend(flatten_tree(child, depth + 1, expanded));
        }
    }

    items
}

/// Build a tree connector prefix for the given depth.
/// Uses Unicode box-drawing characters (│, ├, └).
fn build_tree_prefix(depth: usize) -> String {
    if depth == 1 {
        return String::new();
    }

    let mut prefix = String::new();
    // We need to know which ancestors are the "last child" to draw correctly.
    // For simplicity, we use "│  " for all ancestors.
    for _ in 1..depth {
        prefix.push_str("│  ");
    }
    prefix
}

/// Build a full tree prefix with correct connector for the last child.
pub fn build_tree_item_prefix(path: &str, full_tree: &[TreeItem]) -> String {
    // Find all ancestors and determine if each is the last child in its group
    let parts: Vec<&str> = path.split('/').collect();
    let mut prefix = String::new();

    for d in 1..parts.len() {
        let ancestor_path: String = parts[..d].join("/");
        // Check if this ancestor is the last child among its siblings
        let is_last = is_last_child_in_group(full_tree, &ancestor_path, d);
        if is_last {
            prefix.push_str("   ");
        } else {
            prefix.push_str("│  ");
        }
    }

    // Determine connector for the current item
    let is_last = is_last_child_in_group(full_tree, path, parts.len());
    prefix.push_str(if is_last { "└─" } else { "├─" });

    prefix
}

/// Check if a path is the last child in its sibling group at the given depth.
fn is_last_child_in_group(items: &[TreeItem], path: &str, depth: usize) -> bool {
    // Find all items at this depth sharing the same prefix
    let parent_path = {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() <= 1 {
            return true;
        }
        parts[..parts.len() - 1].join("/")
    };

    let siblings: Vec<&TreeItem> = items
        .iter()
        .filter(|item| {
            item.depth == depth
                && item.full_path.starts_with(&parent_path)
                && item
                    .full_path
                    .split('/')
                    .count()
                    == path.split('/').count()
        })
        .collect();

    if siblings.is_empty() {
        return true;
    }

    siblings.last().map(|s| s.full_path == path).unwrap_or(true)
}
