use std::collections::{HashMap, HashSet};

use git2::Oid;
use git_graph::graph::GitGraph;
use git_graph::print::format::CommitFormat;
use git_graph::settings::{
    BranchOrder, BranchSettings, BranchSettingsDef, Characters, MergePatterns, Settings,
};

use crate::models::{BranchScope, Commit};

pub fn create_graph_settings(scope: BranchScope) -> Settings {
    let include_remote = match scope {
        BranchScope::All | BranchScope::Remote => true,
        BranchScope::Local => false,
    };

    Settings {
        reverse_commit_order: false,
        debug: false,
        compact: false,
        colored: false,
        include_remote,
        format: CommitFormat::OneLine,
        wrapping: None,
        characters: Characters::thin(),
        branch_order: BranchOrder::ShortestFirst(true),
        branches: BranchSettings::from(BranchSettingsDef::simple())
            .expect("simple branching model is valid"),
        merge_patterns: MergePatterns::default(),
    }
}

pub fn build_commits_from_graph(graph: &GitGraph) -> Vec<Commit> {
    let mut commits = Vec::new();

    let num_cols = graph
        .all_branches
        .iter()
        .filter_map(|b| b.visual.column)
        .max()
        .map(|c| c + 1)
        .unwrap_or(1);

    // Build a map from parent OID to the set of child columns.
    // Used to detect fork points: if any child is in a different column,
    // the parent commit is a fork point.
    let mut child_cols: HashMap<Oid, HashSet<usize>> = HashMap::new();
    for info in graph.commits.iter() {
        let current_col = info
            .branch_trace
            .and_then(|t| graph.all_branches[t].visual.column)
            .unwrap_or(0);
        for parent in info.parents.iter().flatten() {
            child_cols.entry(*parent).or_default().insert(current_col);
        }
    }

    for (i, info) in graph.commits.iter().enumerate() {
        let current_col = info
            .branch_trace
            .and_then(|t| graph.all_branches[t].visual.column)
            .unwrap_or(0);

        // Determine which branches are active at this commit index
        let mut active = vec![false; num_cols];
        let mut max_active_col = current_col;
        for branch in graph.all_branches.iter() {
            let col = match branch.visual.column {
                Some(c) => c,
                None => continue,
            };
            let (Some(start), Some(end)) = branch.range else {
                continue;
            };
            if start <= i && i <= end {
                active[col] = true;
                if col > max_active_col {
                    max_active_col = col;
                }
            }
        }

        // Find fork points: commits with children in at least two different columns
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

        // Find parent columns that differ from current_col
        // For merges: draw top corners (╭ ╮) — horizontal lines merging down
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

        // Ensure parent columns are included in the graph line
        for &pc in &merge_parent_cols {
            if pc > max_active_col {
                max_active_col = pc;
            }
        }

        // Build graph line string — draw corners at parent columns
        let mut line = String::with_capacity(max_active_col + 1);
        for (col, &is_active) in active.iter().enumerate().take(max_active_col + 1) {
            let is_merge_parent = merge_parent_cols.contains(&col);
            let is_fork_start = fork_start_cols.contains(&col);

            if is_active || is_merge_parent || is_fork_start {
                if col == current_col {
                    line.push(if info.is_merge {
                        '\u{25CB}'
                    } else {
                        '\u{25CF}'
                    });
                    // ○ merge, ● normal
                } else if is_merge_parent {
                    // Merge connector: horizontal lines curving down
                    if col < current_col {
                        line.push('\u{256D}'); // ╭ parent to the left, curve from right
                    } else {
                        line.push('\u{256E}'); // ╮ parent to the right, curve from left
                    }
                } else if is_fork_start {
                    // New branch starting here: vertical line curving toward child branch
                    if col < current_col {
                        line.push('\u{2570}'); // ╰ new branch left, curve from right
                    } else {
                        line.push('\u{256F}'); // ╯ new branch right, curve from left
                    }
                } else {
                    line.push('\u{2502}'); // │ branch continuation
                }
            } else {
                line.push(' ');
            }
        }

        commits.push(Commit {
            hash: info.oid.to_string(),
            graph: line,
            graph_only: false,
            author: String::new(),
            date: String::new(),
            subject: String::new(),
            merge: info.is_merge,
            decorations: Vec::new(),
            deco_line: 0,
        });
    }

    commits
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("git-log-ratatui-{name}-{id}"))
    }

    // --- create_graph_settings tests ---

    #[test]
    fn test_create_graph_settings_all_scope() {
        let settings = create_graph_settings(BranchScope::All);
        assert!(settings.include_remote);
        assert_eq!(settings.reverse_commit_order, false);
        assert_eq!(settings.debug, false);
        assert_eq!(settings.compact, false);
        assert_eq!(settings.colored, false);
    }

    #[test]
    fn test_create_graph_settings_local_scope() {
        let settings = create_graph_settings(BranchScope::Local);
        assert!(!settings.include_remote);
    }

    #[test]
    fn test_create_graph_settings_remote_scope() {
        let settings = create_graph_settings(BranchScope::Remote);
        assert!(settings.include_remote);
    }

    // --- build_commits_from_graph tests (integration with temp repo) ---

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

            // Configure git user for commits
            let mut cfg = repo.config().unwrap();
            cfg.set_str("user.name", "Test").unwrap();
            cfg.set_str("user.email", "test@test.com").unwrap();

            TempRepo { dir, _repo: repo }
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

    /// Build a GitGraph from a repository with default settings.
    fn build_graph(repo: git2::Repository) -> GitGraph {
        let settings = create_graph_settings(BranchScope::All);
        GitGraph::new(repo, &settings, None, None).expect("GitGraph::new failed")
    }

    #[test]
    fn test_linear_history_graph_lines() {
        let tr = TempRepo::new();
        let a = tr.root_commit("refs/heads/main");
        let b = tr.child_commit("refs/heads/main", "B", a);
        let c = tr.child_commit("refs/heads/main", "C", b);

        // Re-open repo for GraphGraph (it takes ownership)
        let repo = git2::Repository::open(Path::new(&tr.dir)).unwrap();
        let graph = build_graph(repo);
        let commits = build_commits_from_graph(&graph);

        assert_eq!(commits.len(), 3, "expected 3 commits");

        // All commits on a single linear branch should have a simple graph "●" (bullet)
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

        // Create feature branch at A
        let commit_a = tr.repo().find_commit(a).unwrap();
        tr.repo().branch("feature/test", &commit_a, false).unwrap();

        // Create commit C on feature
        let c = tr.child_commit("refs/heads/feature/test", "C", a);

        // Merge feature into main (commit D has parents B and C)
        let d = tr.merge_commit("refs/heads/main", "Merge", b, c);

        let repo = git2::Repository::open(Path::new(&tr.dir)).unwrap();
        let graph = build_graph(repo);
        let commits = build_commits_from_graph(&graph);

        assert!(
            commits.len() >= 3,
            "expected at least 3 commits, got {}",
            commits.len()
        );

        // The merge commit (D) should have a merge marker ○
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

        // At least one commit (the merge or its ancestor) should have a merge connector
        let has_connector = commits.iter().any(|c| {
            c.graph.contains('\u{256D}')
                || c.graph.contains('\u{256E}') // ╭ or ╮
                || c.graph.contains('\u{2570}')
                || c.graph.contains('\u{256F}') // ╰ or ╯
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

        let result = GitGraph::new(repo, &create_graph_settings(BranchScope::All), None, None);
        match result {
            Ok(graph) => {
                let commits = build_commits_from_graph(&graph);
                assert!(commits.is_empty(), "empty repo should produce no commits");
            }
            Err(_) => {
                // Expected: no commits to walk
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
