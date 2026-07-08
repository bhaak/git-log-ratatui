use crate::diff_format::append_diff_line;
use crate::domain::{CommitInfo, FileEntry};
use crate::error::AppError;
use crate::time_format::time_to_string_with_seconds;

use super::GitRepository;

impl GitRepository {
    /// Fetch structured commit metadata using git2.
    pub fn fetch_commit_info(&self, hash: &str) -> Result<CommitInfo, AppError> {
        let oid = git2::Oid::from_str(hash)?;
        let commit = self.repo.find_commit(oid)?;

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
    pub fn fetch_diff(&self, hash: &str) -> Result<(Vec<String>, Vec<FileEntry>), AppError> {
        let oid = git2::Oid::from_str(hash)?;
        let commit = self.repo.find_commit(oid)?;
        let tree = commit.tree()?;

        let parent_tree = if commit.parent_count() > 0 {
            commit.parent(0).ok().and_then(|p| p.tree().ok())
        } else {
            None
        };

        let mut diff_opts = git2::DiffOptions::new();
        let mut diff =
            self.repo
                .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))?;

        let mut find_opts = git2::DiffFindOptions::new();
        find_opts.renames(true);
        diff.find_similar(Some(&mut find_opts))?;

        let mut diff_lines = Vec::new();
        let mut file_entries = Vec::new();
        let mut last_file_id: Option<git2::Oid> = None;

        diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
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
        })?;

        Ok((diff_lines, file_entries))
    }
}
