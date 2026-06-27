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
        let author_date = time_to_string(time);

        let committer = commit.committer();
        let committer_time = commit.committer().when();
        let committer_date = if commit.committer().name() == commit.author().name()
            && commit.committer().email() == commit.author().email()
        {
            String::new()
        } else {
            time_to_string(committer_time)
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
            author_name: commit.author().name().unwrap_or("").to_string(),
            author_email: commit.author().email().unwrap_or("").to_string(),
            author_date,
            committer_name: committer.name().unwrap_or("").to_string(),
            committer_email: committer.email().unwrap_or("").to_string(),
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

        let _ = diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            let origin = line.origin();
            let content = String::from_utf8_lossy(line.content());
            let display = match origin {
                '+' | '-' | ' ' => format!("{}{}", origin, content),
                'F' => {
                    // File header
                    let header = content.trim_end_matches('\n');
                    diff_lines.push(format!("diff --git a/{} b/{}", header, header));
                    return true;
                }
                'H' => {
                    // Header
                    let h = content.trim();
                    if !h.is_empty() {
                        diff_lines.push(h.to_string());
                    }
                    return true;
                }
                _ => {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        trimmed.to_string()
                    } else {
                        return true;
                    }
                }
            };
            diff_lines.push(display);
            true
        })
        .map_err(|e| format!("Diff print error: {}", e))?;

        // Extract file entries from the diff lines
        for (i, line) in diff_lines.iter().enumerate() {
            if line.starts_with("diff --git") {
                if let Some(name) = line.split(" b/").nth(1).map(|s| s.to_string()) {
                    file_entries.push(FileEntry {
                        name,
                        diff_line: i,
                    });
                }
            }
        }

        Ok((diff_lines, file_entries))
    }

    /// Fetch commits with `git log --graph` (shells out, git2 cannot render graphs).
    pub fn fetch_commits(
        &self,
        branch: Option<&str>,
        scope: BranchScope,
    ) -> Result<Vec<Commit>, String> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.repo_path)
            .arg("log")
            .arg("--graph")
            .arg("--date=format:%Y-%m-%d %H:%M")
            .arg("--format=%H%x00%an%x00%ad%x00%s%x00%d%x00%P");

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
        let commits = parse_git_log(&text);
        Ok(commits)
    }
}

fn time_to_string(time: git2::Time) -> String {
    let total_seconds = time.seconds() as i64;
    // Convert Unix timestamp to year/month/day/hour/minute
    let mut ts = total_seconds;
    // Days since epoch
    let days = ts / 86400;
    ts = ts % 86400;
    let hour = ts / 3600;
    ts = ts % 3600;
    let minute = ts / 60;

    // Convert days since epoch to year/month/day
    // Algorithm: start from 1970-01-01
    let (year, month, day) = days_to_ymd(days);

    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month, day, hour, minute)
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

/// Parse the null-byte-delimited git log output into Commit structs.
fn parse_git_log(text: &str) -> Vec<Commit> {
    let mut commits = Vec::new();

    for line in text.lines() {
        let graph = extract_graph(line);
        let content = &line[graph.len()..];

        let parts: Vec<&str> = content.split('\0').collect();
        if parts.len() < 6 {
            continue;
        }

        let hash = parts[0].to_string();
        let author = parts[1].to_string();
        let date = parts[2].to_string();
        let subject = parts[3].to_string();
        let decorations_raw = parts[4].trim().trim_matches(|c| c == '(' || c == ')');
        let parents_raw = parts[5].trim();

        let decorations = parse_decorations(decorations_raw);
        let merge = parents_raw.split(' ').filter(|p| !p.is_empty()).count() > 1;
        let graph_only = hash.is_empty() || hash == " ";

        commits.push(Commit {
            hash,
            author,
            date,
            subject,
            graph,
            merge,
            graph_only,
            decorations,
            deco_line: 0,
        });
    }

    let mut expanded_idx = 0;
    for commit in commits.iter_mut() {
        if !commit.decorations.is_empty() {
            expanded_idx += 1;
        }
        commit.deco_line = expanded_idx;
        expanded_idx += 1;
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

/// Parse the decorations field (ref names) into a list of decorations.
fn parse_decorations(raw: &str) -> Vec<Decoration> {
    if raw.is_empty() {
        return Vec::new();
    }

    let mut decos = Vec::new();
    let mut is_head = false;

    for part in raw.split(", ") {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("HEAD") {
            is_head = true;
            if let Some(branch) = trimmed.strip_prefix("HEAD -> ") {
                decos.push(Decoration {
                    label: branch.to_string(),
                    kind: DecorationKind::LocalBranch,
                });
            }
        } else if let Some(name) = trimmed.strip_prefix("tag: ") {
            decos.push(Decoration {
                label: name.to_string(),
                kind: DecorationKind::Tag,
            });
        } else if trimmed.starts_with("origin/") {
            decos.push(Decoration {
                label: trimmed.to_string(),
                kind: DecorationKind::RemoteBranch,
            });
        } else {
            decos.push(Decoration {
                label: trimmed.to_string(),
                kind: DecorationKind::LocalBranch,
            });
        }
    }

    if is_head {
        decos.insert(
            0,
            Decoration {
                label: "HEAD".to_string(),
                kind: DecorationKind::Head,
            },
        );
    }

    decos
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
    fn test_parse_decorations_head_to_main() {
        let decos = parse_decorations("HEAD -> main");
        assert_eq!(decos.len(), 2);
        assert_eq!(decos[0].label, "HEAD");
        assert_eq!(decos[0].kind, DecorationKind::Head);
        assert_eq!(decos[1].label, "main");
        assert_eq!(decos[1].kind, DecorationKind::LocalBranch);
    }

    #[test]
    fn test_parse_decorations_tag() {
        let decos = parse_decorations("tag: v1.0");
        assert_eq!(decos.len(), 1);
        assert_eq!(decos[0].label, "v1.0");
        assert_eq!(decos[0].kind, DecorationKind::Tag);
    }

    #[test]
    fn test_parse_decorations_remote() {
        let decos = parse_decorations("origin/main");
        assert_eq!(decos.len(), 1);
        assert_eq!(decos[0].label, "origin/main");
        assert_eq!(decos[0].kind, DecorationKind::RemoteBranch);
    }

    #[test]
    fn test_parse_decorations_empty() {
        let decos = parse_decorations("");
        assert!(decos.is_empty());
    }

    #[test]
    fn test_parse_decorations_multiple() {
        let decos = parse_decorations("HEAD -> main, tag: v1.0, origin/main");
        assert_eq!(decos.len(), 4);
        assert_eq!(decos[0].label, "HEAD");
        assert_eq!(decos[0].kind, DecorationKind::Head);
        assert_eq!(decos[1].label, "main");
        assert_eq!(decos[1].kind, DecorationKind::LocalBranch);
        assert_eq!(decos[2].label, "v1.0");
        assert_eq!(decos[2].kind, DecorationKind::Tag);
        assert_eq!(decos[3].label, "origin/main");
        assert_eq!(decos[3].kind, DecorationKind::RemoteBranch);
    }
}
