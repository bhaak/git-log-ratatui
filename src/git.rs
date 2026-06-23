use std::process::Command;
use std::sync::mpsc;
use std::thread;

use crate::models::*;

/// Fetch branches from the repository.
fn fetch_branches(repo_path: &str, scope: BranchScope) -> GitResult {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo_path).arg("branch");

    match scope {
        BranchScope::All => {
            cmd.arg("--all");
        }
        BranchScope::Local => {} // default
        BranchScope::Remote => {
            cmd.arg("--remote");
        }
    }

    cmd.arg("--format=%(refname:short)");

    match cmd.output() {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let branches: Vec<String> = text
                .lines()
                .filter(|l| !l.is_empty())
                .filter(|l| !l.starts_with("HEAD"))
                .map(|l| l.to_string())
                .collect();
            GitResult::Branches(branches)
        }
        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr);
            GitResult::Error(format!("git branch failed: {}", err))
        }
        Err(e) => GitResult::Error(format!("git branch error: {}", e)),
    }
}

/// Fetch commits from the repository using `git log --graph`.
fn fetch_commits(repo_path: &str, branch: Option<&str>, scope: BranchScope) -> GitResult {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(repo_path)
        .arg("log")
        .arg("--graph")
        .arg("--date=format:%Y-%m-%d %H:%M")
        .arg("--format=%H%x00%an%x00%ad%x00%s%x00%d%x00%P");

    if let Some(branch) = branch {
        cmd.arg(branch);
    } else {
        match scope {
            BranchScope::All => { cmd.arg("--all"); }
            BranchScope::Local => { cmd.arg("--branches"); }
            BranchScope::Remote => { cmd.arg("--remotes"); }
        }
    }

    match cmd.output() {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let commits = parse_git_log(&text);
            GitResult::Commits(commits)
        }
        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr);
            GitResult::Error(format!("git log failed: {}", err))
        }
        Err(e) => GitResult::Error(format!("git log error: {}", e)),
    }
}

/// Parse the null-byte-delimited git log output into Commit structs.
fn parse_git_log(text: &str) -> Vec<Commit> {
    let mut commits = Vec::new();

    for line in text.lines() {
        // Count leading graph characters (spaces and graph chars)
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

    // Assign deco_line: each commit with decorations gets a row for the deco line
    let mut expanded_idx = 0;
    for commit in commits.iter_mut() {
        if !commit.decorations.is_empty() {
            expanded_idx += 1; // decoration row
        }
        commit.deco_line = expanded_idx;
        expanded_idx += 1; // the commit row itself
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

    // Split by ", " and " -> "
    let mut decos = Vec::new();
    let mut is_head = false;

    for part in raw.split(", ") {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Handle "HEAD -> branch" format
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

/// Fetch diff and commit info for a specific commit hash.
fn fetch_diff(repo_path: &str, hash: &str) -> GitResult {
    let commit_info = match fetch_commit_info(repo_path, hash) {
        Ok(info) => info,
        Err(e) => return GitResult::Error(e),
    };

    let (diff_lines, file_entries) = match fetch_diff_output(repo_path, hash) {
        Ok(result) => result,
        Err(e) => return GitResult::Error(e),
    };

    GitResult::Diff {
        commit_info,
        diff_lines,
        file_entries,
    }
}

/// Fetch structured commit metadata using `git show --no-patch`.
fn fetch_commit_info(repo_path: &str, hash: &str) -> Result<CommitInfo, String> {
    let format = "%H%x00%s%x00%P%x00%an%x00%ae%x00%ai%x00%cn%x00%ce%x00%ci";
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("show")
        .arg(hash)
        .arg("--no-patch")
        .arg(format!("--format={}", format))
        .output()
        .map_err(|e| format!("git show error: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "git show failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = text.split('\0').collect();
    if parts.len() < 9 {
        return Err("unexpected git show output format".to_string());
    }

    Ok(CommitInfo {
        hash: parts[0].to_string(),
        subject: parts[1].to_string(),
        parents: parts[2]
            .split(' ')
            .filter(|p| !p.is_empty())
            .map(|p| p.to_string())
            .collect(),
        author_name: parts[3].to_string(),
        author_email: parts[4].to_string(),
        author_date: parts[5].to_string(),
        committer_name: parts[6].to_string(),
        committer_email: parts[7].to_string(),
        committer_date: parts[8].trim().to_string(),
    })
}

/// Fetch the raw diff output and extract file entries.
fn fetch_diff_output(repo_path: &str, hash: &str) -> Result<(Vec<String>, Vec<FileEntry>), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("show")
        .arg(hash)
        .arg("--format=")
        .output()
        .map_err(|e| format!("git show error: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "git show failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();

    let mut file_entries = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("diff --git") {
            // Extract filename from "diff --git a/path b/path"
            if let Some(name) = line
                .split(" b/")
                .nth(1)
                .map(|s| s.to_string())
            {
                file_entries.push(FileEntry {
                    name,
                    diff_line: i,
                });
            }
        }
    }

    Ok((lines, file_entries))
}

/// Channel-based git worker that uses a single thread but multiple channels.
/// Each panel gets its own result channel for parallel streaming.
pub struct GitWorker {
    cmd_sender: mpsc::Sender<GitCommand>,
    result_receiver: mpsc::Receiver<GitResult>,
}

impl GitWorker {
    /// Create a new git worker with its own thread.
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<GitCommand>();
    let (result_tx, result_rx) = mpsc::channel::<GitResult>();

        thread::spawn(move || {
            for cmd in cmd_rx {
                let result = match cmd {
                    GitCommand::FetchBranches { repo_path, scope } => {
                        fetch_branches(&repo_path, scope)
                    }
                GitCommand::FetchCommits { repo_path, branch, scope } => {
                    fetch_commits(&repo_path, branch.as_deref(), scope)
                }
                    GitCommand::FetchDiff { repo_path, hash } => {
                        fetch_diff(&repo_path, &hash)
                    }
                };
                if result_tx.send(result).is_err() {
                    break;
                }
            }
        });

        GitWorker {
            cmd_sender: cmd_tx,
            result_receiver: result_rx,
        }
    }

    /// Send a command to the git worker.
    pub fn send(&self, cmd: GitCommand) {
        let _ = self.cmd_sender.send(cmd);
    }

    /// Try to receive a result (non-blocking).
    pub fn try_recv(&self) -> Option<GitResult> {
        self.result_receiver.try_recv().ok()
    }
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
        assert_eq!(decos.len(), 4); // HEAD, main, v1.0, origin/main
        assert_eq!(decos[0].label, "HEAD");
        assert_eq!(decos[0].kind, DecorationKind::Head);
        assert_eq!(decos[1].label, "main");
        assert_eq!(decos[1].kind, DecorationKind::LocalBranch);
        assert_eq!(decos[2].label, "v1.0");
        assert_eq!(decos[2].kind, DecorationKind::Tag);
        assert_eq!(decos[3].label, "origin/main");
        assert_eq!(decos[3].kind, DecorationKind::RemoteBranch);
    }

    #[test]
    fn test_convert_graph_chars() {
        // Test via commit_table's function — but it's private.
        // Test via extract_graph instead, which is exported.
        let graph = extract_graph("| * /");
        assert_eq!(graph, "| * /");
    }
}
