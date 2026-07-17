use std::collections::{HashMap, HashSet};

use git2::Oid;
use git_graph::graph::GitGraph;

use crate::theme::Theme;
use crate::view::CommitRow;

pub fn build_commits_from_graph(graph: &GitGraph) -> Vec<CommitRow> {
    let mut commits = Vec::with_capacity(graph.commits.len());
    let (branch_starts, branch_ends, num_cols) = compute_branch_boundaries(graph);
    let child_cols = build_child_cols(graph);

    let mut active = vec![false; num_cols];
    let mut max_active_col: usize = 0;
    let mut start_ptr: usize = 0;
    let mut end_ptr: usize = 0;

    for (i, info) in graph.commits.iter().enumerate() {
        let current_col = info
            .branch_trace
            .and_then(|t| graph.all_branches[t].visual.column)
            .unwrap_or(0);

        while start_ptr < branch_starts.len() && branch_starts[start_ptr].0 <= i {
            let col = branch_starts[start_ptr].1;
            active[col] = true;
            max_active_col = max_active_col.max(col);
            start_ptr += 1;
        }

        while end_ptr < branch_ends.len() && branch_ends[end_ptr].0 < i {
            let col = branch_ends[end_ptr].1;
            active[col] = false;
            end_ptr += 1;
        }

        if !active[max_active_col] {
            max_active_col = active.iter().rposition(|&a| a).unwrap_or(0);
        }

        max_active_col = max_active_col.max(current_col);

        let mut fork_start_cols = Vec::new();
        if let Some(cols) = child_cols.get(&info.oid) {
            if cols.len() >= 2 {
                for &child_col in cols {
                    if child_col != current_col {
                        fork_start_cols.push(child_col);
                        if child_col > max_active_col {
                            max_active_col = child_col;
                        }
                    }
                }
            }
        }

        let mut merge_parent_cols = Vec::new();

        if info.is_merge {
            merge_parent_cols = (0..2)
                .filter_map(|p| {
                    info.parents[p].and_then(|oid| {
                        graph.indices.get(&oid).and_then(|&idx| {
                            graph.commits[idx]
                                .branch_trace
                                .and_then(|t| graph.all_branches[t].visual.column)
                        })
                    })
                })
                .filter(|&c| c != current_col)
                .collect();
        }

        for &pc in &merge_parent_cols {
            if pc > max_active_col {
                max_active_col = pc;
            }
        }

        let (line, colors) = build_graph_line(
            &active,
            &merge_parent_cols,
            &fork_start_cols,
            current_col,
            info.is_merge,
            max_active_col,
        );

        commits.push(CommitRow {
            hash: info.oid.to_string(),
            graph: line,
            graph_colors: colors,
            graph_only: false,
            author: String::new(),
            date: String::new(),
            subject: String::new(),
            merge: info.is_merge,
            decorations: Vec::new(),
            deco_line: 0,
            epoch_days: 0,
        });
    }

    commits
}

/// Collect branch start/end positions and maximum column count from the graph.
#[allow(clippy::type_complexity)]
fn compute_branch_boundaries(
    graph: &GitGraph,
) -> (Vec<(usize, usize)>, Vec<(usize, usize)>, usize) {
    let num_cols = graph
        .all_branches
        .iter()
        .filter_map(|b| b.visual.column)
        .max()
        .map(|c| c + 1)
        .unwrap_or(1);

    let mut branch_starts: Vec<(usize, usize)> = Vec::new();
    let mut branch_ends: Vec<(usize, usize)> = Vec::new();
    for branch in graph.all_branches.iter() {
        let Some(col) = branch.visual.column else {
            continue;
        };
        let (Some(start), Some(end)) = branch.range else {
            continue;
        };
        branch_starts.push((start, col));
        branch_ends.push((end, col));
    }
    branch_starts.sort_unstable_by_key(|&(s, _)| s);
    branch_ends.sort_unstable_by_key(|&(e, _)| e);

    (branch_starts, branch_ends, num_cols)
}

/// Build a map from commit OID to set of child column indices for fork detection.
fn build_child_cols(graph: &GitGraph) -> HashMap<Oid, HashSet<usize>> {
    let mut child_cols: HashMap<Oid, HashSet<usize>> = HashMap::with_capacity(graph.commits.len());
    for info in graph.commits.iter() {
        let current_col = info
            .branch_trace
            .and_then(|t| graph.all_branches[t].visual.column)
            .unwrap_or(0);
        for parent in info.parents.iter().flatten() {
            child_cols.entry(*parent).or_default().insert(current_col);
        }
    }
    child_cols
}

/// Build the graph line string and lane color vector for one commit row.
/// Each column gets a box-drawing character based on the current state of
/// active lanes, merge parents, and fork starts.
fn build_graph_line(
    active: &[bool],
    merge_parent_cols: &[usize],
    fork_start_cols: &[usize],
    current_col: usize,
    is_merge: bool,
    max_active_col: usize,
) -> (String, Vec<u8>) {
    let cap = max_active_col + 1;
    let mut line = String::with_capacity(cap);
    let mut colors = Vec::with_capacity(cap);
    for (col, &is_active) in active.iter().enumerate().take(cap) {
        let is_merge_parent = merge_parent_cols.contains(&col);
        let is_fork_start = fork_start_cols.contains(&col);

        if is_active || is_merge_parent || is_fork_start {
            let lane_color = (col % Theme::default().graph_colors.len()) as u8;
            if col == current_col {
                line.push(if is_merge { '\u{25CB}' } else { '\u{25CF}' });
            } else if is_merge_parent {
                if col < current_col {
                    line.push('\u{256D}');
                } else {
                    line.push('\u{256E}');
                }
            } else if is_fork_start {
                if col < current_col {
                    line.push('\u{2570}');
                } else {
                    line.push('\u{256F}');
                }
            } else {
                line.push('\u{2502}');
            }
            colors.push(lane_color);
        } else {
            line.push(' ');
            colors.push(255);
        }
    }
    (line, colors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::BranchScope;
    use crate::graph::create_graph_settings;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("git-log-ratatui-{name}-{id}"))
    }

    struct TempRepo {
        dir: std::path::PathBuf,
        _repo: git2::Repository,
    }

    impl TempRepo {
        fn new() -> Self {
            let dir = unique_temp_dir("test");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let repo = git2::Repository::init(&dir).unwrap();

            let mut cfg = repo.config().unwrap();
            cfg.set_str("user.name", "Test").unwrap();
            cfg.set_str("user.email", "test@test.com").unwrap();

            let tr = TempRepo { dir, _repo: repo };
            // git-graph with start_point=None looks for refs/heads/master.
            // Ensure a master branch exists so graph tests work regardless of
            // the system default branch name.
            tr.root_commit("refs/heads/master");
            tr
        }

        fn repo(&self) -> &git2::Repository {
            &self._repo
        }

        fn root_commit(&self, refname: &str) -> git2::Oid {
            let sig = git2::Signature::now("Test", "test@test.com").unwrap();
            let tree_oid = {
                let blob_oid = self._repo.blob(b"test content").unwrap();
                let mut builder = self._repo.treebuilder(None).unwrap();
                builder.insert("test.txt", blob_oid, 0o100644).unwrap();
                builder.write().unwrap()
            };
            let tree = self._repo.find_tree(tree_oid).unwrap();
            self._repo
                .commit(Some(refname), &sig, &sig, "root", &tree, &[])
                .unwrap()
        }

        fn child_commit(&self, refname: &str, message: &str, parent_oid: git2::Oid) -> git2::Oid {
            let sig = git2::Signature::now("Test", "test@test.com").unwrap();
            let parent = self._repo.find_commit(parent_oid).unwrap();
            let tree = parent.tree().unwrap();
            self._repo
                .commit(Some(refname), &sig, &sig, message, &tree, &[&parent])
                .unwrap()
        }

        fn merge_commit(
            &self,
            refname: &str,
            message: &str,
            p1: git2::Oid,
            p2: git2::Oid,
        ) -> git2::Oid {
            let sig = git2::Signature::now("Test", "test@test.com").unwrap();
            let parent1 = self._repo.find_commit(p1).unwrap();
            let parent2 = self._repo.find_commit(p2).unwrap();
            let tree = parent1.tree().unwrap();
            self._repo
                .commit(
                    Some(refname),
                    &sig,
                    &sig,
                    message,
                    &tree,
                    &[&parent1, &parent2],
                )
                .unwrap()
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn build_graph(repo: git2::Repository) -> GitGraph {
        let settings = create_graph_settings(BranchScope::All);
        GitGraph::new(repo, settings, None, None).expect("GitGraph::new failed")
    }

    #[test]
    fn test_linear_history_graph_lines() {
        let tr = TempRepo::new();
        let a = tr.root_commit("refs/heads/main");
        let b = tr.child_commit("refs/heads/main", "B", a);
        let c = tr.child_commit("refs/heads/main", "C", b);

        let repo = git2::Repository::open(Path::new(&tr.dir)).unwrap();
        let graph = build_graph(repo);
        let commits = build_commits_from_graph(&graph);

        assert_eq!(commits.len(), 3, "expected 3 commits");

        for (i, commit) in commits.iter().enumerate() {
            assert!(
                commit.graph.contains('\u{25CF}'),
                "commit {i} should contain ●, got: {:?}",
                commit.graph
            );
            assert!(
                !commit.graph_only,
                "no commit should be graph_only in linear history"
            );
        }

        let oids: Vec<String> = commits.iter().map(|c| c.hash.clone()).collect();
        assert!(oids.contains(&c.to_string()));
        assert!(oids.contains(&b.to_string()));
        assert!(oids.contains(&a.to_string()));
    }

    #[test]
    fn test_merge_commit_has_merge_marker() {
        let tr = TempRepo::new();
        let a = tr.root_commit("refs/heads/main");
        let b = tr.child_commit("refs/heads/main", "B", a);

        let commit_a = tr.repo().find_commit(a).unwrap();
        tr.repo().branch("feature/test", &commit_a, false).unwrap();

        let c = tr.child_commit("refs/heads/feature/test", "C", a);

        let d = tr.merge_commit("refs/heads/main", "Merge", b, c);

        let repo = git2::Repository::open(Path::new(&tr.dir)).unwrap();
        let graph = build_graph(repo);
        let commits = build_commits_from_graph(&graph);

        assert!(
            commits.len() >= 3,
            "expected at least 3 commits, got {}",
            commits.len()
        );

        let merge_commit = commits
            .iter()
            .find(|c| c.hash == d.to_string())
            .expect("merge commit D not found");
        assert!(
            merge_commit.graph.contains('\u{25CB}'),
            "merge commit should contain ○, got: {:?}",
            merge_commit.graph
        );
        assert!(merge_commit.merge, "merge flag should be true");

        let has_connector = commits.iter().any(|c| {
            c.graph.contains('\u{256D}')
                || c.graph.contains('\u{256E}')
                || c.graph.contains('\u{2570}')
                || c.graph.contains('\u{256F}')
        });
        assert!(
            has_connector,
            "expected at least one graph connector in merge history"
        );
    }

    #[test]
    fn test_commits_have_unique_hashes() {
        let tr = TempRepo::new();
        let a = tr.root_commit("refs/heads/main");
        tr.child_commit("refs/heads/main", "B", a);

        let repo = git2::Repository::open(Path::new(&tr.dir)).unwrap();
        let graph = build_graph(repo);
        let commits = build_commits_from_graph(&graph);

        let mut seen = std::collections::HashSet::new();
        for commit in &commits {
            assert!(seen.insert(&commit.hash), "duplicate hash: {}", commit.hash);
        }
    }

    #[test]
    fn test_current_repo_produces_graph() {
        let repo =
            git2::Repository::open(Path::new(".")).expect("failed to open current repository");
        let graph = build_graph(repo);
        let commits = build_commits_from_graph(&graph);

        assert!(!commits.is_empty(), "expected at least one commit");
        let has_graph = commits.iter().any(|c| !c.graph.trim().is_empty());
        assert!(has_graph, "expected at least one commit with a graph line");
        let all_have_hashes = commits.iter().all(|c| !c.hash.is_empty());
        assert!(all_have_hashes, "all commits should have hashes");
    }

    #[test]
    fn test_empty_graph_produces_empty_vec() {
        let dir = unique_temp_dir("empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = git2::Repository::init(&dir).unwrap();

        let result = GitGraph::new(repo, create_graph_settings(BranchScope::All), None, None);
        if let Ok(graph) = result {
            let commits = build_commits_from_graph(&graph);
            assert!(commits.is_empty(), "empty repo should produce no commits");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
