pub mod branches;
pub mod commits;
pub mod decorations;
pub mod diff;
pub mod graph_conv;

use std::path::Path;

use tracing::info;

use crate::error::AppError;

/// Wrapper around git2 for core git operations with git-graph integration.
pub struct GitRepository {
    repo: git2::Repository,
    repo_path: String,
}

impl GitRepository {
    /// Open a git repository at the given path.
    pub fn open(path: &str) -> Result<Self, AppError> {
        let repo = git2::Repository::open(Path::new(path))?;
        info!("Repository opened at {}", path);
        Ok(GitRepository {
            repo,
            repo_path: path.to_string(),
        })
    }

    /// Detect the repository's default branch by checking remote HEAD first,
    /// then falling back to the local HEAD.
    pub fn detect_default_branch(&self) -> Option<String> {
        if let Ok(r) = self.repo.find_reference("refs/remotes/origin/HEAD") {
            if let Some(target) = r.symbolic_target() {
                if let Some(name) = target.strip_prefix("refs/remotes/origin/") {
                    return Some(name.to_string());
                }
            }
        }
        if let Ok(head) = self.repo.head() {
            if head.is_branch() {
                return head.shorthand().map(|s| s.to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_commits_on_current_repo() {
        let git = GitRepository::open(".").expect("failed to open repository");
        let commits = git
            .fetch_commits(None, crate::models::BranchScope::All, None)
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

        run(&["checkout", "-b", "feature/test"]);
        std::fs::write(tmp.join("file"), "feature").unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "feature commit"]);
        let feat_hash = String::from_utf8(run(&["rev-parse", "--short", "HEAD"]).stdout)
            .unwrap()
            .trim()
            .to_string();

        run(&["checkout", "main"]);
        std::fs::write(tmp.join("file"), "main after branch").unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "main commit"]);

        let git = GitRepository::open(&tmp.to_string_lossy()).unwrap();
        let commits = git
            .fetch_commits(
                Some("feature/test"),
                crate::models::BranchScope::Local,
                None,
            )
            .expect("fetch_commits for feature/test failed");

        assert!(!commits.is_empty(), "should have commits for feature/test");

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

        let commits_simple = git
            .fetch_commits_simplified(
                Some("feature/test"),
                crate::models::BranchScope::Local,
                None,
            )
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

        let main_commits = git
            .fetch_commits_simplified(Some("main"), crate::models::BranchScope::Local, None)
            .expect("fetch_commits_simplified for main failed");
        assert!(
            !main_commits.is_empty(),
            "should have commits for main (simplified)"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
