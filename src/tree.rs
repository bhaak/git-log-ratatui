use std::collections::BTreeMap;

use crate::models::*;

/// Build a hierarchical tree from a flat list of branch names.
pub fn build_branch_tree(branches: &[String]) -> BranchNode {
    let mut root = BranchNode {
        name: String::new(),
        full_path: String::new(),
        children: Vec::new(),
    };

    for branch_name in branches {
        let parts: Vec<&str> = branch_name.split('/').collect();
        insert_into_tree(&mut root, &parts, branch_name);
    }

    root
}

fn insert_into_tree(node: &mut BranchNode, parts: &[&str], full_path: &str) {
    if parts.is_empty() {
        return;
    }

    let name = parts[0].to_string();
    let child_idx = node.children.iter().position(|c| c.name == name);

    if parts.len() == 1 {
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
        if let Some(idx) = child_idx {
            insert_into_tree(&mut node.children[idx], &parts[1..], full_path);
        } else {
            let mut new_node = BranchNode {
                name,
                full_path: String::new(),
                children: Vec::new(),
            };
            insert_into_tree(&mut new_node, &parts[1..], full_path);
            node.children.push(new_node);
        }
    }
}

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

/// Public entry point for flattening — starts with empty ancestor chain.
pub fn flatten_tree(
    node: &BranchNode,
    depth: usize,
    expanded: &BTreeMap<String, bool>,
) -> Vec<TreeItem> {
    flatten_tree_inner(node, depth, expanded, &[])
}

/// Recursive flattening that tracks which ancestors were the last child in their sibling group.
fn flatten_tree_inner(
    node: &BranchNode,
    depth: usize,
    expanded: &BTreeMap<String, bool>,
    ancestors_last: &[bool],
) -> Vec<TreeItem> {
    let mut items = Vec::new();
    let child_count = node.children.len();

    for (i, child) in node.children.iter().enumerate() {
        let is_last = i == child_count - 1;

        let expandable = !child.children.is_empty();

        // Use name-based key for expanded lookups (works for branch nodes and directory nodes)
        let node_key = if child.full_path.is_empty() {
            format!("{}/", child.name)
        } else {
            child.full_path.clone()
        };

        let is_expanded = expanded.get(&node_key).copied().unwrap_or(expandable);

        let marker = if expandable {
            if is_expanded {
                "\u{25BC}"
            } else {
                "\u{25B6}"
            }
        } else {
            "  "
        };

        // Build connector prefix from ancestor chain
        let prefix = build_tree_prefix(ancestors_last);
        let connector = if is_last {
            "\u{2514}\u{2500}"
        } else {
            "\u{251C}\u{2500}"
        };

        let display_name = if child.full_path.is_empty() || expandable {
            format!("{}{}{} {}", prefix, connector, marker, child.name)
        } else {
            format!("{}{}{}{}", prefix, connector, marker, child.name)
        };

        items.push(TreeItem {
            name: display_name,
            depth: depth + 1,
            expandable,
            expanded: is_expanded,
            is_branch: !child.full_path.is_empty(),
            full_path: child.full_path.clone(),
            tree_prefix: format!("{}{}", prefix, connector),
            key: node_key.clone(),
        });

        if is_expanded {
            let mut child_ancestors = ancestors_last.to_vec();
            child_ancestors.push(is_last);
            let child_items = flatten_tree_inner(child, depth + 1, expanded, &child_ancestors);
            items.extend(child_items);
        }
    }

    items
}

/// Build an indentation prefix showing ancestor lines.
/// Uses `│  ` for non-last ancestors and `   ` for last-child ancestors.
fn build_tree_prefix(ancestors_last: &[bool]) -> String {
    ancestors_last
        .iter()
        .map(|&is_last| if is_last { "   " } else { "\u{2502}  " })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_simple_tree() {
        let branches = vec![
            "main".to_string(),
            "feature/login".to_string(),
            "feature/signup".to_string(),
        ];
        let mut root = build_branch_tree(&branches);
        sort_tree(&mut root);

        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].name, "main");
        assert_eq!(root.children[1].name, "feature");

        let feature = &root.children[1];
        assert_eq!(feature.children.len(), 2);
    }

    #[test]
    fn test_flatten_collapsed() {
        let branches = vec!["main".to_string(), "feature/login".to_string()];
        let mut root = build_branch_tree(&branches);
        sort_tree(&mut root);

        let mut expanded: BTreeMap<String, bool> = BTreeMap::new();
        expanded.insert("feature/".to_string(), false);

        let items = flatten_tree(&root, 0, &expanded);

        assert_eq!(items.len(), 2);
        assert!(items[0].name.contains("main"));
        assert!(!items[0].expandable);
        // "feature" directory node (no trailing slash in display)
        assert!(!items[1].name.contains("feature/"));
        assert!(items[1].name.contains("feature"));
        assert!(items[1].expandable);
        assert!(!items[1].expanded);
    }

    #[test]
    fn test_flatten_expanded() {
        let branches = vec!["main".to_string(), "feature/login".to_string()];
        let mut root = build_branch_tree(&branches);
        sort_tree(&mut root);

        let expanded: BTreeMap<String, bool> = BTreeMap::new();
        let items = flatten_tree(&root, 0, &expanded);

        assert_eq!(items.len(), 3);
        assert!(items[0].name.contains("main"));
        assert_eq!(items[2].depth, 2);
        assert!(items[2].is_branch);
    }

    #[test]
    fn test_sort_tree_main_master_first() {
        let branches = vec![
            "develop".to_string(),
            "master".to_string(),
            "main".to_string(),
            "feature".to_string(),
        ];
        let mut root = build_branch_tree(&branches);
        sort_tree(&mut root);

        assert_eq!(root.children[0].name, "main");
        assert_eq!(root.children[1].name, "master");
        assert_eq!(root.children[2].name, "develop");
        assert_eq!(root.children[3].name, "feature");
    }

    #[test]
    fn test_empty_tree() {
        let branches: Vec<String> = Vec::new();
        let mut root = build_branch_tree(&branches);
        sort_tree(&mut root);
        let expanded = BTreeMap::new();
        let items = flatten_tree(&root, 0, &expanded);
        assert!(items.is_empty());
    }

    #[test]
    fn test_branch_with_child_branches_is_both_branch_and_expandable() {
        // When a branch name is also a prefix (e.g. "feature" and "feature/login"),
        // the "feature" node should be both a branch AND expandable.
        let branches = vec!["feature".to_string(), "feature/login".to_string()];
        let mut root = build_branch_tree(&branches);
        sort_tree(&mut root);

        let expanded = BTreeMap::new();
        let items = flatten_tree(&root, 0, &expanded);

        // "feature" item should exist
        let feature_item = items
            .iter()
            .find(|i| i.full_path == "feature")
            .expect("feature branch item not found");

        assert!(
            feature_item.is_branch,
            "feature should be marked as a branch"
        );
        assert!(
            feature_item.expandable,
            "feature should be expandable since it has child branches"
        );
        // "feature/login" should also exist under it
        let login_item = items
            .iter()
            .find(|i| i.full_path == "feature/login")
            .expect("feature/login branch item not found");
        assert!(login_item.is_branch);
        assert!(!login_item.expandable);
    }
}
