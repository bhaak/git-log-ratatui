use crate::error::AppError;
use crate::models::*;

use super::GitRepository;

impl GitRepository {
    /// Fetch all branch entries for the given scope, including branch type info
    /// and the full tag list. Tags are always returned regardless of scope.
    pub fn fetch_branches(&self, scope: BranchScope) -> Result<BranchData, AppError> {
        let default_branch = self.detect_default_branch();
        let mut entries = Vec::new();

        let collect = |entries: &mut Vec<BranchEntry>, names: Vec<String>, is_remote: bool| {
            for name in names {
                entries.push(BranchEntry { name, is_remote });
            }
        };

        match scope {
            BranchScope::All => {
                collect(
                    &mut entries,
                    self.list_branches(Some(git2::BranchType::Local))?,
                    false,
                );
                collect(
                    &mut entries,
                    self.list_branches(Some(git2::BranchType::Remote))?,
                    true,
                );
            }
            BranchScope::Local => {
                collect(
                    &mut entries,
                    self.list_branches(Some(git2::BranchType::Local))?,
                    false,
                );
            }
            BranchScope::Remote => {
                collect(
                    &mut entries,
                    self.list_branches(Some(git2::BranchType::Remote))?,
                    true,
                );
            }
        }

        let tags = self.fetch_tags()?;

        Ok(BranchData {
            default_branch,
            entries,
            tags,
        })
    }

    /// Fetch all tag names from the repository.
    fn fetch_tags(&self) -> Result<Vec<String>, AppError> {
        let tag_names = self.repo.tag_names(None)?;
        let mut tags = Vec::new();
        for name in tag_names.iter().flatten() {
            tags.push(name.to_string());
        }
        Ok(tags)
    }

    fn list_branches(&self, filter: Option<git2::BranchType>) -> Result<Vec<String>, AppError> {
        let mut branches = Vec::new();
        let iter = self.repo.branches(filter)?;

        for branch_result in iter {
            let (branch, _branch_type) = branch_result?;
            if let Ok(Some(name)) = branch.name() {
                let name = name.to_string();
                if !name.contains("HEAD") {
                    branches.push(name);
                }
            }
        }
        Ok(branches)
    }

    /// Resolve a shorthand name to a full git reference.
    ///
    /// Tries `refs/heads/`, then `refs/remotes/`, then `refs/tags/`.
    /// If the name already starts with `refs/`, looks it up directly.
    pub fn resolve_branch_ref_name(&self, branch: &str) -> Option<String> {
        if let Ok(r) = self.repo.find_reference(&format!("refs/heads/{}", branch)) {
            return r.name().map(|n| n.to_string());
        }
        if let Ok(r) = self
            .repo
            .find_reference(&format!("refs/remotes/{}", branch))
        {
            return r.name().map(|n| n.to_string());
        }
        if let Ok(r) = self.repo.find_reference(&format!("refs/tags/{}", branch)) {
            return r.name().map(|n| n.to_string());
        }
        if branch.starts_with("refs/") {
            if let Ok(r) = self.repo.find_reference(branch) {
                return r.name().map(|n| n.to_string());
            }
        }
        None
    }
}
