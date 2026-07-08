use crate::domain::{BranchData, BranchEntry, BranchScope};
use crate::error::AppError;
use crate::time_format::time_to_string;

use super::GitRepository;

impl GitRepository {
    /// Fetch all branch entries for the given scope, including branch type info
    /// and the full tag list. Tags are always returned regardless of scope.
    pub fn fetch_branches(&self, scope: BranchScope) -> Result<BranchData, AppError> {
        let default_branch = self.detect_default_branch();
        let mut entries = Vec::new();

        let collect = |entries: &mut Vec<BranchEntry>,
                       names: Vec<(String, Option<String>)>,
                       is_remote: bool| {
            for (name, date) in names {
                entries.push(BranchEntry {
                    name,
                    is_remote,
                    last_commit_date: date,
                });
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

        let (tags, tags_dates) = self.fetch_tags_with_dates()?;

        Ok(BranchData {
            default_branch,
            entries,
            tags,
            tags_dates,
        })
    }

    /// Fetch tag names with their last commit dates.
    fn fetch_tags_with_dates(&self) -> Result<(Vec<String>, Vec<Option<String>>), AppError> {
        let tag_names = self.repo.tag_names(None)?;
        let mut tags = Vec::new();
        let mut dates = Vec::new();
        for name in tag_names.iter().flatten() {
            tags.push(name.to_string());
            let date = self
                .repo
                .find_reference(&format!("refs/tags/{name}"))
                .ok()
                .and_then(|r| r.peel_to_commit().ok().map(|c| time_to_string(c.time())));
            dates.push(date);
        }
        Ok((tags, dates))
    }

    fn list_branches(
        &self,
        filter: Option<git2::BranchType>,
    ) -> Result<Vec<(String, Option<String>)>, AppError> {
        let mut branches = Vec::new();
        let iter = self.repo.branches(filter)?;

        for branch_result in iter {
            let (branch, _branch_type) = branch_result?;
            if let Ok(Some(name)) = branch.name() {
                let name = name.to_string();
                if !name.contains("HEAD") {
                    let date = branch
                        .get()
                        .peel_to_commit()
                        .ok()
                        .map(|c| time_to_string(c.time()));
                    branches.push((name, date));
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
