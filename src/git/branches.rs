use std::time::Instant;

use crate::domain::{BranchData, BranchEntry, BranchScope, StashEntry};
use crate::error::AppError;
use crate::time_format::time_to_string;

use super::GitRepository;

impl GitRepository {
    /// Fetch all branch entries for the given scope, including branch type info
    /// and the full tag list. Tags are always returned regardless of scope.
    pub fn fetch_branches(&self, scope: BranchScope) -> Result<BranchData, AppError> {
        let t0 = Instant::now();
        let default_branch = self.detect_default_branch();
        let mut entries = Vec::new();

        let collect = |entries: &mut Vec<BranchEntry>,
                       names: Vec<(String, Option<(String, i64)>)>,
                       is_remote: bool| {
            for (name, date_data) in names {
                let (date_str, epoch_days) = date_data
                    .map(|(s, d)| (Some(s), Some(d)))
                    .unwrap_or((None, None));
                entries.push(BranchEntry {
                    name,
                    is_remote,
                    last_commit_date: date_str,
                    epoch_days,
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
        let stashes = self.fetch_stashes();

        let elapsed = t0.elapsed();
        tracing::debug!(
            "branches: {} entries, {} tags, {} stashes in {}ms",
            entries.len(),
            tags.len(),
            stashes.len(),
            elapsed.as_millis()
        );

        Ok(BranchData {
            default_branch,
            entries,
            tags,
            tags_dates,
            stashes,
        })
    }

    /// Fetch tag names with their last commit dates and epoch days.
    #[allow(clippy::type_complexity)]
    fn fetch_tags_with_dates(&self) -> Result<(Vec<String>, Vec<Option<(String, i64)>>), AppError> {
        let tag_names = self.repo.tag_names(None)?;
        let mut tags = Vec::new();
        let mut dates = Vec::new();
        for name in tag_names.iter().flatten() {
            tags.push(name.to_string());
            let date_data = self
                .repo
                .find_reference(&format!("refs/tags/{name}"))
                .ok()
                .and_then(|r| {
                    r.peel_to_commit()
                        .ok()
                        .map(|c| (time_to_string(c.time()), c.time().seconds() / 86400))
                });
            dates.push(date_data);
        }
        Ok((tags, dates))
    }

    /// Fetch stash entries: index, message, OID, and timestamps.
    fn fetch_stashes(&self) -> Vec<StashEntry> {
        use std::path::Path;
        let mut stashes = Vec::new();
        let repo_path = self.repo_path.clone();
        if let Ok(mut repo) = git2::Repository::open(Path::new(&repo_path)) {
            let mut raw: Vec<(usize, String, git2::Oid)> = Vec::new();
            let _ = repo.stash_foreach(|index, message, &oid| {
                raw.push((index, message.to_string(), oid));
                true
            });
            for (index, message, oid) in raw {
                let commit = repo.find_commit(oid).ok();
                let epoch_days = commit.as_ref().map(|c| c.time().seconds() / 86400);
                let last_commit_date = commit.as_ref().map(|c| time_to_string(c.time()));
                stashes.push(StashEntry {
                    index,
                    message,
                    oid: oid.to_string(),
                    last_commit_date,
                    epoch_days,
                });
            }
        }
        stashes
    }

    #[allow(clippy::type_complexity)]
    fn list_branches(
        &self,
        filter: Option<git2::BranchType>,
    ) -> Result<Vec<(String, Option<(String, i64)>)>, AppError> {
        let mut branches = Vec::new();
        let iter = self.repo.branches(filter)?;

        for branch_result in iter {
            let (branch, _branch_type) = branch_result?;
            if let Ok(Some(name)) = branch.name() {
                let name = name.to_string();
                if !name.contains("HEAD") {
                    let date_data = branch
                        .get()
                        .peel_to_commit()
                        .ok()
                        .map(|c| (time_to_string(c.time()), c.time().seconds() / 86400));
                    branches.push((name, date_data));
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
