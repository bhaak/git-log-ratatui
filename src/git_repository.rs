use std::collections::HashMap;
use std::path::Path;

use git_graph::graph::GitGraph;
use rayon::prelude::*;

use crate::diff_format::append_diff_line;
use crate::graph::{build_commits_from_graph, create_graph_settings};
use crate::models::*;
use crate::time_format::{time_to_string, time_to_string_with_seconds};

/// Wrapper around git2 for core git operations with git-graph integration.
pub struct GitRepository {
    repo: git2::Repository,
    repo_path: String,
}

impl GitRepository {
    /// Open a git repository at the given path.
    pub fn open(path: &str) -> Result<Self, String> {
        let repo = git2::Repository::open(Path::new(path))
            .map_err(|e| format!("Failed to open repository: {}", e))?;
        Ok(GitRepository {
            repo,
            repo_path: path.to_string(),
        })
    }

    /// Fetch all branch names for the given scope.
    pub fn fetch_branches(&self, scope: BranchScope) -> Result<Vec<String>, String> {
        let mut branches = Vec::new();

        match scope {
            BranchScope::All => {
                branches.extend(self.list_branches(Some(git2::BranchType::Local))?);
                branches.extend(self.list_branches(Some(git2::BranchType::Remote))?);
            }
            BranchScope::Local => {
                branches = self.list_branches(Some(git2::BranchType::Local))?;
            }
            BranchScope::Remote => {
                branches = self.list_branches(Some(git2::BranchType::Remote))?;
            }
        }

        branches.sort();
        // Deduplicate by name (local and remote branches may have same name)
        branches.dedup();

        Ok(branches)
    }

    fn list_branches(&self, filter: Option<git2::BranchType>) -> Result<Vec<String>, String> {
        let mut branches = Vec::new();
        let iter = self
            .repo
            .branches(filter)
            .map_err(|e| format!("Failed to list branches: {}", e))?;

        for branch_result in iter {
            let (branch, _branch_type) =
                branch_result.map_err(|e| format!("Branch iteration error: {}", e))?;
            if let Ok(Some(name)) = branch.name() {
                let name = name.to_string();
                if !name.contains("HEAD") {
                    branches.push(name);
                }
            }
        }
        Ok(branches)
    }

    /// Fetch structured commit metadata using git2.
    pub fn fetch_commit_info(&self, hash: &str) -> Result<CommitInfo, String> {
        let oid = git2::Oid::from_str(hash)
            .map_err(|e| format!("Invalid commit hash '{}': {}", hash, e))?;
        let commit = self
            .repo
            .find_commit(oid)
            .map_err(|e| format!("Commit not found: {}", e))?;

        let time = commit.time();
        let author_date = time_to_string_with_seconds(time);
        let author_name = commit.author().name().unwrap_or("").to_string();
        let author_email = commit.author().email().unwrap_or("").to_string();

        let committer = commit.committer();
        let committer_time = committer.when();
        let committer_name = committer.name().unwrap_or("").to_string();
        let committer_email = committer.email().unwrap_or("").to_string();
        let committer_date = if committer_name == author_name && committer_email == author_email {
            String::new()
        } else {
            time_to_string_with_seconds(committer_time)
        };

        Ok(CommitInfo {
            hash: hash.to_string(),
            subject: commit.summary().unwrap_or("").to_string(),
            parents: commit.parent_ids().map(|id| id.to_string()).collect(),
            author_name,
            author_email,
            author_date,
            committer_name,
            committer_email,
            committer_date,
        })
    }

    /// Fetch diff for a commit using git2.
    pub fn fetch_diff(&self, hash: &str) -> Result<(Vec<String>, Vec<FileEntry>), String> {
        let oid = git2::Oid::from_str(hash)
            .map_err(|e| format!("Invalid commit hash '{}': {}", hash, e))?;
        let commit = self
            .repo
            .find_commit(oid)
            .map_err(|e| format!("Commit not found: {}", e))?;
        let tree = commit
            .tree()
            .map_err(|e| format!("Failed to get tree: {}", e))?;

        let parent_tree = if commit.parent_count() > 0 {
            commit.parent(0).ok().and_then(|p| p.tree().ok())
        } else {
            None
        };

        let mut diff_opts = git2::DiffOptions::new();
        let mut diff = self
            .repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))
            .map_err(|e| format!("Diff error: {}", e))?;

        let mut find_opts = git2::DiffFindOptions::new();
        find_opts.renames(true);
        diff.find_similar(Some(&mut find_opts))
            .map_err(|e| format!("Rename detection error: {}", e))?;

        let mut diff_lines = Vec::new();
        let mut file_entries = Vec::new();
        let mut last_file_id: Option<git2::Oid> = None;

        diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
            // Detect file boundary: git2 calls the callback per-file sequentially,
            // so when the delta id changes, we've started a new file.
            let delta_id = delta.new_file().id();
            if last_file_id != Some(delta_id) {
                last_file_id = Some(delta_id);
                if let Some(path) = delta.new_file().path() {
                    let status = match delta.status() {
                        git2::Delta::Added => '+',
                        git2::Delta::Deleted => '-',
                        git2::Delta::Modified | git2::Delta::Typechange => '~',
                        git2::Delta::Renamed | git2::Delta::Copied => '→',
                        _ => '?',
                    };
                    let old_path = delta
                        .old_file()
                        .path()
                        .map(|p| p.to_string_lossy().to_string());
                    let new_path = path.to_string_lossy().to_string();
                    let old_name = old_path.filter(|o| o != &new_path);
                    file_entries.push(FileEntry {
                        name: path.to_string_lossy().to_string(),
                        diff_line: diff_lines.len(),
                        status,
                        old_name,
                    });
                }
            }

            append_diff_line(
                &mut diff_lines,
                line.origin(),
                &String::from_utf8_lossy(line.content()),
            );
            true
        })
        .map_err(|e| format!("Diff print error: {}", e))?;

        Ok((diff_lines, file_entries))
    }

    /// Fetch commits using git-graph to generate the graph visualization,
    /// then enriches each commit with author/date/subject/merge/decorations from git2.
    pub fn fetch_commits(
        &self,
        branch: Option<&str>,
        scope: BranchScope,
        limit: Option<usize>,
    ) -> Result<Vec<Commit>, String> {
        let decoration_map = self.build_decoration_map()?;

        let settings = create_graph_settings(scope);

        let start_point = branch.map(|b| b.to_string());

        // GitGraph::new() takes ownership of Repository, so open a fresh handle
        let repo = git2::Repository::open(Path::new(&self.repo_path))
            .map_err(|e| format!("Failed to open repository for git-graph: {}", e))?;

        // `limit` caps the revwalk to the newest N commits. Because the walk is
        // topological + time sorted, the first N commits are stable as N grows,
        // so raising the limit only appends older commits.
        let graph = GitGraph::new(repo, settings, start_point, limit)
            .map_err(|e| format!("git-graph error: {}", e))?;

        let mut commits = build_commits_from_graph(&graph);
        self.enrich_commits(&mut commits, &decoration_map);
        Ok(commits)
    }

    /// Fast-path fetch that skips git-graph entirely. Walks commits with git2
    /// Revwalk and assigns lane colors based on which branch tip each commit
    /// belongs to. Much faster for the simplified graph view.
    pub fn fetch_commits_simplified(
        &self,
        branch: Option<&str>,
        scope: BranchScope,
        limit: Option<usize>,
    ) -> Result<Vec<Commit>, String> {
        let decoration_map = self.build_decoration_map()?;
        let branch_tip_colors = self.build_branch_tip_colors(scope)?;

        let mut revwalk = self
            .repo
            .revwalk()
            .map_err(|e| format!("Failed to create revwalk: {}", e))?;

        revwalk
            .set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)
            .map_err(|e| format!("Failed to set revwalk sorting: {}", e))?;

        if let Some(b) = branch {
            let refname = self
                .resolve_branch_ref_name(b)
                .unwrap_or_else(|| format!("refs/heads/{}", b));
            revwalk
                .push_ref(&refname)
                .map_err(|e| format!("Failed to push branch ref '{}': {}", b, e))?;
        } else {
            self.push_scope_refs(&mut revwalk, scope)?;
        }

        let mut commits: Vec<Commit> = Vec::new();
        for oid_result in revwalk {
            let oid = oid_result.map_err(|e| format!("Revwalk error: {}", e))?;
            if let Some(max) = limit {
                if commits.len() >= max {
                    break;
                }
            }

            let lane = branch_tip_colors.get(&oid).copied().unwrap_or(255);
            let graph = if lane == 255 {
                // Find merge status from git2 for graph char
                let merge = self
                    .repo
                    .find_commit(oid)
                    .map(|c| c.parent_count() > 1)
                    .unwrap_or(false);
                let ch = if merge { '○' } else { '●' };
                ch.to_string()
            } else {
                '●'.to_string()
            };

            commits.push(Commit {
                hash: oid.to_string(),
                graph,
                graph_colors: vec![lane],
                graph_only: false,
                author: String::new(),
                date: String::new(),
                subject: String::new(),
                merge: false,
                decorations: Vec::new(),
                deco_line: 0,
            });
        }

        self.enrich_commits(&mut commits, &decoration_map);
        Ok(commits)
    }

    /// Resolve a shorthand branch name (e.g. "main" or "origin/main") to a full
    /// git reference name (e.g. "refs/heads/main" or "refs/remotes/origin/main").
    fn resolve_branch_ref_name(&self, branch: &str) -> Option<String> {
        // Try local branch
        if let Ok(r) = self.repo.find_reference(&format!("refs/heads/{}", branch)) {
            return r.name().map(|n| n.to_string());
        }
        // Try remote branch
        if let Ok(r) = self
            .repo
            .find_reference(&format!("refs/remotes/{}", branch))
        {
            return r.name().map(|n| n.to_string());
        }
        // If branch already contains a ref prefix, try it directly
        if branch.starts_with("refs/") {
            if let Ok(r) = self.repo.find_reference(branch) {
                return r.name().map(|n| n.to_string());
            }
        }
        None
    }
    fn push_scope_refs(
        &self,
        revwalk: &mut git2::Revwalk,
        scope: BranchScope,
    ) -> Result<(), String> {
        let refs = self
            .repo
            .references()
            .map_err(|e| format!("Failed to list references: {}", e))?;

        for r in refs {
            let r = r.map_err(|e| format!("Ref error: {}", e))?;
            if !r.is_branch() && !r.is_tag() {
                continue;
            }
            if scope == BranchScope::Local && r.is_remote() {
                continue;
            }
            if scope == BranchScope::Remote && !r.is_remote() {
                continue;
            }
            if r.is_tag() {
                continue; // skip tags for commit walk
            }
            if let Some(name) = r.name() {
                revwalk
                    .push_ref(name)
                    .map_err(|e| format!("Failed to push ref '{}': {}", name, e))?;
            }
        }
        Ok(())
    }

    /// Build a mapping from branch tip OID to lane color index.
    /// Each unique branch gets a color from LANE_COLORS based on its name hash.
    fn build_branch_tip_colors(
        &self,
        scope: BranchScope,
    ) -> Result<HashMap<git2::Oid, u8>, String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut tips: HashMap<git2::Oid, u8> = HashMap::new();
        let refs = self
            .repo
            .references()
            .map_err(|e| format!("Failed to list references: {}", e))?;

        for r in refs {
            let r = r.map_err(|e| format!("Ref error: {}", e))?;
            if !r.is_branch() {
                continue;
            }
            if scope == BranchScope::Local && r.is_remote() {
                continue;
            }
            if scope == BranchScope::Remote && !r.is_remote() {
                continue;
            }

            let target_oid = match r.target() {
                Some(oid) => oid,
                None => continue,
            };

            let name = r.shorthand().unwrap_or("");
            if name.is_empty() {
                continue;
            }

            let mut hasher = DefaultHasher::new();
            name.hash(&mut hasher);
            let color_idx = (hasher.finish() % crate::graph::LANE_COLORS.len() as u64) as u8;

            tips.entry(target_oid).or_insert(color_idx);
        }

        Ok(tips)
    }

    /// Build a map from commit Oid to decorations by iterating all references.
    fn build_decoration_map(&self) -> Result<HashMap<git2::Oid, Vec<Decoration>>, String> {
        let mut map: HashMap<git2::Oid, Vec<Decoration>> = HashMap::new();
        let refs = self
            .repo
            .references()
            .map_err(|e| format!("Failed to list references: {}", e))?;

        for r in refs {
            let r = match r {
                Ok(r) => r,
                Err(_) => continue,
            };
            let target_oid = match r
                .target()
                .or_else(|| r.resolve().ok().and_then(|resolved| resolved.target()))
            {
                Some(oid) => oid,
                None => continue,
            };
            let shorthand = r.shorthand().unwrap_or("").to_string();
            if shorthand.is_empty() {
                continue;
            }

            let kind = if r.is_tag() {
                DecorationKind::Tag
            } else if r.is_remote() {
                DecorationKind::RemoteBranch
            } else if r.is_branch() {
                DecorationKind::LocalBranch
            } else {
                // HEAD reference or other symbolic ref
                DecorationKind::Head
            };

            map.entry(target_oid).or_default().push(Decoration {
                label: shorthand,
                kind,
            });
        }

        // Sort every decoration list once by priority, so enrich_commits can
        // simply clone without re-sorting per commit.
        for decos in map.values_mut() {
            decos.sort_unstable_by_key(|d| d.kind.priority());
        }

        Ok(map)
    }

    /// Fill in author, date, subject, merge status, and decorations from git2.
    fn enrich_commits(
        &self,
        commits: &mut [Commit],
        decoration_map: &HashMap<git2::Oid, Vec<Decoration>>,
    ) {
        let repo_path = self.repo_path.clone();

        // Open one git2::Repository per rayon worker thread; find_commit is read-only.
        commits.par_iter_mut().for_each_init(
            || {
                git2::Repository::open(Path::new(&repo_path))
                    .expect("Failed to open repository for enrich_commits")
            },
            |repo, commit| {
                if commit.graph_only || commit.hash.is_empty() {
                    return;
                }

                let oid = match git2::Oid::from_str(&commit.hash) {
                    Ok(oid) => oid,
                    Err(_) => return,
                };
                let git_commit = match repo.find_commit(oid) {
                    Ok(c) => c,
                    Err(_) => return,
                };

                commit.author = git_commit.author().name().unwrap_or("").to_string();
                commit.date = time_to_string(git_commit.time());
                commit.subject = git_commit.summary().unwrap_or("").to_string();
                commit.merge = git_commit.parent_count() > 1;

                if let Some(decos) = decoration_map.get(&oid) {
                    commit.decorations = decos.clone();
                }
            },
        );

        let mut expanded_idx = 0;
        for commit in commits.iter_mut() {
            if !commit.decorations.is_empty() {
                expanded_idx += 1;
            }
            commit.deco_line = expanded_idx;
            expanded_idx += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_commits_on_current_repo() {
        let git = GitRepository::open(".").expect("failed to open repository");
        let commits = git
            .fetch_commits(None, BranchScope::All, None)
            .expect("fetch_commits failed");
        assert!(!commits.is_empty(), "expected at least one commit");

        let has_graph = commits.iter().any(|c| !c.graph.trim().is_empty());
        assert!(
            has_graph,
            "expected at least one commit with non-empty graph line"
        );

        let all_have_hashes = commits.iter().all(|c| !c.hash.is_empty());
        assert!(
            all_have_hashes,
            "expected all commits to have hashes in linear history"
        );
    }

    #[test]
    fn test_fetch_commits_for_branch_with_shorthand_name() {
        use std::process::Command;

        let tmp = std::env::temp_dir().join("git-test-branch-head");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let run = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(&tmp)
                .args(args)
                .output()
                .expect("git command failed")
        };

        run(&["init"]);
        run(&["config", "user.name", "Test"]);
        run(&["config", "user.email", "test@test.com"]);
        std::fs::write(tmp.join("file"), "content").unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "root"]);

        // Create feature branch from root
        run(&["checkout", "-b", "feature/test"]);
        std::fs::write(tmp.join("file"), "feature").unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "feature commit"]);
        let feat_hash = String::from_utf8(run(&["rev-parse", "--short", "HEAD"]).stdout)
            .unwrap()
            .trim()
            .to_string();

        // Commit on main after branching
        run(&["checkout", "main"]);
        std::fs::write(tmp.join("file"), "main after branch").unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "main commit"]);

        // Fetch commits for feature/test using shorthand name (full graph)
        let git = GitRepository::open(&tmp.to_string_lossy()).unwrap();
        let commits = git
            .fetch_commits(Some("feature/test"), BranchScope::Local, None)
            .expect("fetch_commits for feature/test failed");

        assert!(!commits.is_empty(), "should have commits for feature/test");

        // The first non-empty-hash commit should be the branch HEAD
        let first = commits
            .iter()
            .find(|c| !c.hash.is_empty())
            .expect("should have at least one real commit");

        let short = &first.hash[..first.hash.len().min(7)];
        assert_eq!(
            short, feat_hash,
            "first commit should be feature/test HEAD ({}) but got ({})",
            feat_hash, short
        );

        // Also test with simplified path (shorthand resolution)
        let commits_simple = git
            .fetch_commits_simplified(Some("feature/test"), BranchScope::Local, None)
            .expect("fetch_commits_simplified for feature/test failed");
        assert!(
            !commits_simple.is_empty(),
            "should have commits for feature/test (simplified)"
        );
        let first_simple = commits_simple
            .iter()
            .find(|c| !c.hash.is_empty())
            .expect("should have at least one real commit");
        let short_simple = &first_simple.hash[..first_simple.hash.len().min(7)];
        assert_eq!(
            short_simple, feat_hash,
            "first commit (simplified) should be feature/test HEAD ({}) but got ({})",
            feat_hash, short_simple
        );

        // Test with "main" shorthand and simplified path
        let main_commits = git
            .fetch_commits_simplified(Some("main"), BranchScope::Local, None)
            .expect("fetch_commits_simplified for main failed");
        assert!(
            !main_commits.is_empty(),
            "should have commits for main (simplified)"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
