use std::collections::{BTreeMap, HashMap};

use crate::app::state::AppState;
use crate::domain::{BranchEntry, BranchScope};
use crate::state::BranchState;
use crate::tree;
use crate::view::TreeItem;

const LOCAL_KEY: &str = "__local__";
const REMOTE_KEY: &str = "__remote__";
const TAGS_KEY: &str = "__tags__";
const STASHES_KEY: &str = "__stashes__";

pub(crate) fn rebuild_branch_tree(state: &mut AppState) {
    let (date_map, epoch_map) = build_branch_maps(&state.branch.all_branches.entries);
    let (local_names, remote_names) = split_local_remote(&state.branch.all_branches.entries);
    let (tag_date_map, tag_epoch_map) = build_tag_maps(
        &state.branch.all_branches.tags,
        &state.branch.all_branches.tags_dates,
    );

    let mut items = Vec::new();

    push_local_section(
        &mut items,
        &state.branch,
        &date_map,
        &epoch_map,
        &local_names,
    );
    push_remote_section(
        &mut items,
        &state.branch,
        &date_map,
        &epoch_map,
        &remote_names,
    );
    push_tag_section(&mut items, &state.branch, &tag_date_map, &tag_epoch_map);
    push_stash_section(&mut items, &state.branch);

    state.branch.branch_tree = items;
}

fn scope_shows(scope: BranchScope, filter: BranchScope) -> bool {
    scope == BranchScope::All || scope == filter
}

fn expanded_or(nodes: &BTreeMap<String, bool>, key: &str, default: bool) -> bool {
    nodes.get(key).copied().unwrap_or(default)
}

fn section_header(name: &str, key: &str, expanded: bool) -> TreeItem {
    TreeItem {
        name: name.to_string(),
        depth: 0,
        expandable: true,
        expanded,
        is_branch: false,
        full_path: String::new(),
        tree_prefix: String::new(),
        key: key.to_string(),
        last_commit_date: None,
        epoch_days: None,
    }
}

fn build_branch_maps(
    entries: &[BranchEntry],
) -> (
    HashMap<String, Option<String>>,
    HashMap<String, Option<i64>>,
) {
    let mut date_map = HashMap::with_capacity(entries.len());
    let mut epoch_map = HashMap::with_capacity(entries.len());
    for e in entries {
        date_map.insert(e.name.clone(), e.last_commit_date.clone());
        epoch_map.insert(e.name.clone(), e.epoch_days);
    }
    (date_map, epoch_map)
}

fn build_tag_maps(
    tags: &[String],
    tags_dates: &[Option<(String, i64)>],
) -> (
    HashMap<String, Option<String>>,
    HashMap<String, Option<i64>>,
) {
    let mut date_map = HashMap::new();
    let mut epoch_map = HashMap::new();
    for (t, d) in tags.iter().zip(tags_dates.iter()) {
        let (date, epoch) = match d {
            Some((s, e)) => (Some(s.clone()), Some(*e)),
            None => (None, None),
        };
        date_map.insert(t.clone(), date);
        epoch_map.insert(t.clone(), epoch);
    }
    (date_map, epoch_map)
}

fn split_local_remote(entries: &[BranchEntry]) -> (Vec<String>, Vec<String>) {
    let mut local = Vec::new();
    let mut remote = Vec::new();
    for e in entries {
        if e.is_remote {
            remote.push(e.name.clone());
        } else {
            local.push(e.name.clone());
        }
    }
    (local, remote)
}

fn push_local_section(
    items: &mut Vec<TreeItem>,
    state: &BranchState,
    date_map: &HashMap<String, Option<String>>,
    epoch_map: &HashMap<String, Option<i64>>,
    local_names: &[String],
) {
    if !scope_shows(state.branch_scope, BranchScope::Local) || local_names.is_empty() {
        return;
    }

    let expanded = expanded_or(&state.expanded_nodes, LOCAL_KEY, true);
    items.push(section_header("Local Branches", LOCAL_KEY, expanded));

    let mut root = tree::build_branch_tree(local_names);
    tree::sort_tree(&mut root);

    if let Some(ref default) = state.all_branches.default_branch {
        if let Some(pos) = root.children.iter().position(|c| c.name == *default) {
            let default_child = root.children.remove(pos);
            root.children.insert(0, default_child);
        }
    }

    if expanded {
        push_branch_children(items, &root, &state.expanded_nodes, date_map, epoch_map);
    }
}

fn push_remote_section(
    items: &mut Vec<TreeItem>,
    state: &BranchState,
    date_map: &HashMap<String, Option<String>>,
    epoch_map: &HashMap<String, Option<i64>>,
    remote_names: &[String],
) {
    if !scope_shows(state.branch_scope, BranchScope::Remote) || remote_names.is_empty() {
        return;
    }

    let expanded = expanded_or(&state.expanded_nodes, REMOTE_KEY, true);
    items.push(section_header("Remote Branches", REMOTE_KEY, expanded));

    let mut root = tree::build_branch_tree(remote_names);
    tree::sort_tree(&mut root);

    if expanded {
        push_branch_children(items, &root, &state.expanded_nodes, date_map, epoch_map);
    }
}

fn push_branch_children(
    items: &mut Vec<TreeItem>,
    root: &crate::domain::BranchNode,
    expanded_nodes: &BTreeMap<String, bool>,
    date_map: &HashMap<String, Option<String>>,
    epoch_map: &HashMap<String, Option<i64>>,
) {
    let mut branch_items = tree::flatten_tree(root, 0, expanded_nodes);
    for item in &mut branch_items {
        item.last_commit_date = date_map.get(&item.full_path).and_then(|d| d.clone());
        item.epoch_days = epoch_map.get(&item.full_path).and_then(|d| *d);
    }
    items.extend(branch_items);
}

fn push_tag_section(
    items: &mut Vec<TreeItem>,
    state: &BranchState,
    tag_date_map: &HashMap<String, Option<String>>,
    tag_epoch_map: &HashMap<String, Option<i64>>,
) {
    if !scope_shows(state.branch_scope, BranchScope::Tags) || state.all_branches.tags.is_empty() {
        return;
    }

    let expanded = expanded_or(&state.expanded_nodes, TAGS_KEY, true);
    items.push(section_header("Tags", TAGS_KEY, expanded));

    if !expanded {
        return;
    }

    let mut tag_root = tree::build_branch_tree(&state.all_branches.tags);
    tree::sort_tree(&mut tag_root);

    let mut tag_items = tree::flatten_tree(&tag_root, 0, &state.expanded_nodes);
    for item in &mut tag_items {
        if item.is_branch {
            let tag_name = item.full_path.clone();
            item.full_path = format!("refs/tags/{}", tag_name);
        }
        let lookup_key = item
            .full_path
            .strip_prefix("refs/tags/")
            .unwrap_or(&item.full_path);
        item.last_commit_date = tag_date_map.get(lookup_key).and_then(|d| d.clone());
        item.epoch_days = tag_epoch_map.get(lookup_key).and_then(|d| *d);
    }
    items.extend(tag_items);
}

fn push_stash_section(items: &mut Vec<TreeItem>, state: &BranchState) {
    if !scope_shows(state.branch_scope, BranchScope::Stash) || state.all_branches.stashes.is_empty()
    {
        return;
    }

    let expanded = expanded_or(&state.expanded_nodes, STASHES_KEY, true);
    items.push(section_header("Stashes", STASHES_KEY, expanded));

    if !expanded {
        return;
    }

    let stash_count = state.all_branches.stashes.len();
    for (i, stash) in state.all_branches.stashes.iter().enumerate() {
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
