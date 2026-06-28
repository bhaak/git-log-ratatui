use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use crate::models::*;

/// Wrapper around git2 for core git operations plus `git log --graph` shelling.
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
            subject: commit
                .summary()
                .unwrap_or("")
                .to_string(),
            parents: commit
                .parent_ids()
                .map(|id| id.to_string())
                .collect(),
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
        let diff = self
            .repo
            .diff_tree_to_tree(
                parent_tree.as_ref(),
                Some(&tree),
                Some(&mut diff_opts),
            )
            .map_err(|e| format!("Diff error: {}", e))?;

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
                    file_entries.push(FileEntry {
                        name: path.to_string_lossy().to_string(),
                        diff_line: diff_lines.len(),
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

    /// Fetch commits: shells `git log --graph --format=%H` for graph visualization,
    /// then enriches each commit with author/date/subject/merge/decorations from git2.
    pub fn fetch_commits(
        &self,
        branch: Option<&str>,
        scope: BranchScope,
    ) -> Result<Vec<Commit>, String> {
        let decoration_map = self.build_decoration_map()?;

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.repo_path)
            .arg("log")
            .arg("--graph")
            .arg("--format=%H");

        if let Some(branch) = branch {
            cmd.arg(branch);
        } else {
            match scope {
                BranchScope::All => {
                    cmd.arg("--all");
                }
                BranchScope::Local => {
                    cmd.arg("--branches");
                }
                BranchScope::Remote => {
                    cmd.arg("--remotes");
                }
            }
        }

        let output = cmd
            .output()
            .map_err(|e| format!("git log error: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "git log failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let mut commits = parse_git_log_graph(&text);
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
            let target_oid = match r.target().or_else(|| {
                r.resolve().ok().and_then(|resolved| resolved.target())
            }) {
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

            map.entry(target_oid)
                .or_default()
                .push(Decoration {
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
                sorted.sort_by_key(|d| decoration_priority(&d.kind));
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

fn decoration_priority(kind: &DecorationKind) -> u8 {
    match kind {
        DecorationKind::Head => 0,
        DecorationKind::LocalBranch => 1,
        DecorationKind::Tag => 2,
        DecorationKind::RemoteBranch => 3,
    }
}

/// Append a single diff line from git2's `diff.print()` callback to `diff_lines`.
/// Handles the origin encoding: '+'/'-'/' ' get their prefix prepended,
/// 'F'/'H' (file/hunk headers) are split on newlines and pushed each, others trimmed.
fn append_diff_line(diff_lines: &mut Vec<String>, origin: char, content: &str) {
    match origin {
        '+' | '-' | ' ' => {
            diff_lines.push(format!("{}{}", origin, content.trim_end_matches('\n')));
        }
        'F' | 'H' => {
            for s in content.split('\n') {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    diff_lines.push(trimmed.to_string());
                }
            }
        }
        _ => {
            let trimmed = content.trim_end_matches('\n');
            if !trimmed.is_empty() {
                diff_lines.push(trimmed.to_string());
            }
        }
    }
}

fn time_to_string(time: git2::Time) -> String {
    let mut ts = time.seconds();
    let days = ts / 86400;
    ts %= 86400;
    let hour = ts / 3600;
    ts %= 3600;
    let minute = ts / 60;

    // Convert days since epoch to year/month/day
    // Algorithm: start from 1970-01-01
    let (year, month, day) = days_to_ymd(days);

    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month, day, hour, minute)
}

fn time_to_string_with_seconds(time: git2::Time) -> String {
    let second = time.seconds() % 60;
    format!("{}:{:02}", time_to_string(time), second)
}

fn days_to_ymd(mut days: i64) -> (i64, u32, u32) {
    // Days since 1970-01-01
    let mut year = 1970i64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let month_days = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u32;
    for &md in month_days.iter() {
        if days < md as i64 {
            break;
        }
        days -= md as i64;
        month += 1;
    }
    (year, month, (days + 1) as u32)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Parse `git log --graph --format=%H` output into minimal Commit structs.
/// Graph-only lines (no hash) get `graph_only = true`.
/// Author, date, subject, merge, decorations are filled later by git2 enrichment.
fn parse_git_log_graph(text: &str) -> Vec<Commit> {
    let mut commits = Vec::new();

    for line in text.lines() {
        let graph = extract_graph(line);
        let hash = line[graph.len()..].trim().to_string();
        let graph_only = hash.is_empty();

        commits.push(Commit {
            hash,
            graph,
            graph_only,
            author: String::new(),
            date: String::new(),
            subject: String::new(),
            merge: false,
            decorations: Vec::new(),
            deco_line: 0,
        });
    }

    commits
}

/// Extract the graph prefix from a git log line.
fn extract_graph(line: &str) -> String {
    let mut graph = String::new();
    for ch in line.chars() {
        if matches!(ch, ' ' | '*' | '|' | '/' | '\\' | '_') {
            graph.push(ch);
        } else {
            break;
        }
    }
    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_graph_simple() {
        let graph = extract_graph("| * some text");
        assert_eq!(graph, "| * ");
    }

    #[test]
    fn test_extract_graph_no_graph() {
        let graph = extract_graph("commit text");
        assert_eq!(graph, "");
    }

    #[test]
    fn test_parse_git_log_graph() {
        let text = "* abc123def\n| * \n| * 456789abc\n";
        let commits = parse_git_log_graph(text);
        assert_eq!(commits.len(), 3);
        assert_eq!(commits[0].hash, "abc123def");
        assert!(!commits[0].graph_only);
        assert_eq!(commits[1].hash, "");
        assert!(commits[1].graph_only);
        assert_eq!(commits[2].hash, "456789abc");
        assert!(!commits[2].graph_only);
    }

    #[test]
    fn test_parse_git_log_graph_empty() {
        let commits = parse_git_log_graph("");
        assert!(commits.is_empty());
    }

    #[test]
    fn test_decoration_priority() {
        assert_eq!(decoration_priority(&DecorationKind::Head), 0);
        assert_eq!(decoration_priority(&DecorationKind::LocalBranch), 1);
        assert_eq!(decoration_priority(&DecorationKind::Tag), 2);
        assert_eq!(decoration_priority(&DecorationKind::RemoteBranch), 3);
    }

    #[test]
    fn test_days_to_ymd_epoch() {
        let (year, month, day) = days_to_ymd(0);
        assert_eq!(year, 1970);
        assert_eq!(month, 1);
        assert_eq!(day, 1);
    }

    #[test]
    fn test_days_to_ymd_2024_01_01() {
        // 2024-01-01 is 19724 days after 1970-01-01
        let (year, month, day) = days_to_ymd(19723);
        assert_eq!(year, 2024);
        assert_eq!(month, 1);
        assert_eq!(day, 1);
    }

    #[test]
    fn test_days_to_ymd_2024_12_31() {
        // End of leap year 2024
        let (year, month, day) = days_to_ymd(19723 + 365);
        assert_eq!(year, 2024);
        assert_eq!(month, 12);
        assert_eq!(day, 31);
    }

    #[test]
    fn test_time_to_string_epoch() {
        let time = git2::Time::new(0, 0);
        assert_eq!(time_to_string(time), "1970-01-01 00:00");
        assert_eq!(time_to_string_with_seconds(time), "1970-01-01 00:00:00");
    }

    #[test]
    fn test_is_leap() {
        assert!(is_leap(2024));
        assert!(!is_leap(2023));
        assert!(is_leap(2000)); // divisible by 400
        assert!(!is_leap(1900)); // divisible by 100 but not 400
    }

    // --- Diff line parsing tests ---

    #[test]
    fn test_append_diff_line_addition() {
        let mut lines = Vec::new();
        append_diff_line(&mut lines, '+', "new line\n");
        assert_eq!(lines, vec!["+new line"]);
    }

    #[test]
    fn test_append_diff_line_deletion() {
        let mut lines = Vec::new();
        append_diff_line(&mut lines, '-', "old line\n");
        assert_eq!(lines, vec!["-old line"]);
    }

    #[test]
    fn test_append_diff_line_context() {
        let mut lines = Vec::new();
        append_diff_line(&mut lines, ' ', "unchanged\n");
        assert_eq!(lines, vec![" unchanged"]);
    }

    #[test]
    fn test_append_diff_line_file_header() {
        let mut lines = Vec::new();
        append_diff_line(&mut lines, 'F', "diff --git a/foo.txt b/foo.txt\nindex abc..def\n--- a/foo.txt\n+++ b/foo.txt\n");
        assert_eq!(
            lines,
            vec![
                "diff --git a/foo.txt b/foo.txt",
                "index abc..def",
                "--- a/foo.txt",
                "+++ b/foo.txt",
            ]
        );
    }

    #[test]
    fn test_append_diff_line_hunk_header() {
        let mut lines = Vec::new();
        append_diff_line(&mut lines, 'H', "@@ -1,3 +1,4 @@\n");
        assert_eq!(lines, vec!["@@ -1,3 +1,4 @@"]);
    }

    #[test]
    fn test_append_diff_line_empty_skipped() {
        let mut lines = Vec::new();
        append_diff_line(&mut lines, 'F', "\n");
        assert!(lines.is_empty());
    }
}
