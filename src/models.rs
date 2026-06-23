use std::fmt;

/// Represents a parsed commit from `git log`.
#[derive(Debug, Clone)]
pub struct Commit {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub subject: String,
    /// Raw graph line from `git log --graph` (ASCII chars).
    pub graph: String,
    /// True if this is a merge commit (has multiple parents).
    pub merge: bool,
    /// True if this row is a graph-only continuation line (no actual commit).
    pub graph_only: bool,
    /// Decorations parsed from the `%d` field (e.g. "HEAD -> main, tag: v1.0").
    pub decorations: Vec<Decoration>,
    /// Index of the first row in the expanded table for this decoration block.
    pub deco_line: usize,
}

/// A single decoration on a commit (HEAD, branch, tag).
#[derive(Debug, Clone)]
pub struct Decoration {
    pub label: String,
    pub kind: DecorationKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DecorationKind {
    Head,
    Tag,
    LocalBranch,
    RemoteBranch,
}

/// Structured metadata for a single commit (from `git show`).
#[derive(Debug, Clone, Default)]
pub struct CommitInfo {
    pub subject: String,
    pub hash: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_date: String,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_date: String,
}

/// A node in the hierarchical branch tree.
#[derive(Debug, Clone)]
pub struct BranchNode {
    pub name: String,
    pub full_path: String,
    pub children: Vec<BranchNode>,
}

/// A flattened item in the branch tree display list.
#[derive(Debug, Clone)]
pub struct TreeItem {
    pub name: String,
    pub depth: usize,
    pub expandable: bool,
    pub expanded: bool,
    pub is_branch: bool,
    pub full_path: String,
    pub tree_prefix: String,
}

/// The scope of branches to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchScope {
    All,
    Local,
    Remote,
}

impl BranchScope {
    pub fn next(self) -> Self {
        match self {
            BranchScope::All => BranchScope::Local,
            BranchScope::Local => BranchScope::Remote,
            BranchScope::Remote => BranchScope::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BranchScope::All => "all",
            BranchScope::Local => "local",
            BranchScope::Remote => "remote",
        }
    }
}

/// The currently focused UI panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Branches,
    Search,
    Scope,
    Commits,
    Diff,
}

impl Panel {
    pub const ALL: [Panel; 5] = [
        Panel::Branches,
        Panel::Search,
        Panel::Scope,
        Panel::Commits,
        Panel::Diff,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&p| p == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let idx = Self::ALL.iter().position(|&p| p == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

impl fmt::Display for Panel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Panel::Branches => write!(f, "Branches"),
            Panel::Search => write!(f, "Search"),
            Panel::Scope => write!(f, "Scope"),
            Panel::Commits => write!(f, "Commits"),
            Panel::Diff => write!(f, "Diff"),
        }
    }
}

/// A file changed in a commit, with its diff line offset.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub diff_line: usize,
}

/// A message sent from the main thread to a git worker thread.
#[derive(Debug, Clone)]
pub enum GitCommand {
    /// Fetch all branches for the given scope.
    FetchBranches { repo_path: String, scope: BranchScope },
    /// Fetch commits for the given branch (None = all branches).
    FetchCommits {
        repo_path: String,
        branch: Option<String>,
    },
    /// Fetch diff and commit info for a specific commit hash.
    FetchDiff {
        repo_path: String,
        hash: String,
    },
}

/// A result returned from a git worker thread to the main thread.
#[derive(Debug, Clone)]
pub enum GitResult {
    Branches(Vec<String>),
    Commits(Vec<Commit>),
    Diff {
        commit_info: CommitInfo,
        diff_lines: Vec<String>,
        file_entries: Vec<FileEntry>,
    },
    Error(String),
}
