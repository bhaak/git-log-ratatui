use std::collections::HashMap;
use std::path::Path;

use git_graph::graph::GitGraph;

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
    ) -> Result<Vec<Commit>, String> {
        let decoration_map = self.build_decoration_map()?;

        let settings = create_graph_settings(scope);

        let start_point = branch.map(|b| b.to_string());

        // GitGraph::new() takes ownership of Repository, so open a fresh handle
        let repo = git2::Repository::open(Path::new(&self.repo_path))
            .map_err(|e| format!("Failed to open repository for git-graph: {}", e))?;

        let graph = GitGraph::new(repo, &settings, start_point, None)
            .map_err(|e| format!("git-graph error: {}", e))?;

        let mut commits = build_commits_from_graph(&graph);
        self.enrich_commits(&mut commits, &decoration_map);
        Ok(commits)
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

        Ok(map)
    }

    /// Fill in author, date, subject, merge status, and decorations from git2.
    fn enrich_commits(
        &self,
        commits: &mut [Commit],
        decoration_map: &HashMap<git2::Oid, Vec<Decoration>>,
    ) {
        for commit in commits.iter_mut() {
            if commit.graph_only || commit.hash.is_empty() {
                continue;
            }

            let oid = match git2::Oid::from_str(&commit.hash) {
                Ok(oid) => oid,
                Err(_) => continue,
            };
            let git_commit = match self.repo.find_commit(oid) {
                Ok(c) => c,
                Err(_) => continue,
            };

            commit.author = git_commit.author().name().unwrap_or("").to_string();
            commit.date = time_to_string(git_commit.time());
            commit.subject = git_commit.summary().unwrap_or("").to_string();
            commit.merge = git_commit.parent_count() > 1;

            if let Some(decos) = decoration_map.get(&oid) {
                let mut sorted: Vec<Decoration> = decos.clone();
                sorted.sort_by_key(|d| d.kind.priority());
                commit.decorations = sorted;
            }
        }

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
            .fetch_commits(None, BranchScope::All)
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
}
