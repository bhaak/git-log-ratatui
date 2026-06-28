use std::fmt;

/// A commit entry produced by the git-graph crate, enriched with git2 metadata.
#[derive(Debug, Clone)]
pub struct Commit {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub subject: String,
    /// Unicode graph line from git-graph (already box-drawing characters).
    pub graph: String,
    /// True if this is a merge commit (has multiple parents).
    pub merge: bool,
    /// True if this row is a graph-only continuation line (no commit).
    pub graph_only: bool,
    /// Branch/tag/HEAD decorations resolved via git2 references.
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

/// Structured metadata for a single commit (from git2).
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
#[allow(dead_code)]
pub struct TreeItem {
    pub name: String,
    pub depth: usize,
    pub expandable: bool,
    pub expanded: bool,
    pub is_branch: bool,
    pub full_path: String,
    pub tree_prefix: String,
    /// Unique key for this node in the expanded/collapsed map.
    pub key: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_scope_next_all_to_local() {
        assert_eq!(BranchScope::All.next(), BranchScope::Local);
    }

    #[test]
    fn test_branch_scope_next_local_to_remote() {
        assert_eq!(BranchScope::Local.next(), BranchScope::Remote);
    }

    #[test]
    fn test_branch_scope_next_remote_to_all() {
        assert_eq!(BranchScope::Remote.next(), BranchScope::All);
    }

    #[test]
    fn test_branch_scope_label() {
        assert_eq!(BranchScope::All.label(), "all");
        assert_eq!(BranchScope::Local.label(), "local");
        assert_eq!(BranchScope::Remote.label(), "remote");
    }

    #[test]
    fn test_panel_next_cycles_forward() {
        assert_eq!(Panel::Branches.next(), Panel::Search);
        assert_eq!(Panel::Search.next(), Panel::Scope);
        assert_eq!(Panel::Scope.next(), Panel::Commits);
        assert_eq!(Panel::Commits.next(), Panel::Diff);
        assert_eq!(Panel::Diff.next(), Panel::Branches);
    }

    #[test]
    fn test_panel_prev_cycles_backward() {
        assert_eq!(Panel::Branches.prev(), Panel::Diff);
        assert_eq!(Panel::Diff.prev(), Panel::Commits);
        assert_eq!(Panel::Commits.prev(), Panel::Scope);
        assert_eq!(Panel::Scope.prev(), Panel::Search);
        assert_eq!(Panel::Search.prev(), Panel::Branches);
    }

    #[test]
    fn test_panel_display() {
        assert_eq!(Panel::Branches.to_string(), "Branches");
        assert_eq!(Panel::Search.to_string(), "Search");
        assert_eq!(Panel::Scope.to_string(), "Scope");
        assert_eq!(Panel::Commits.to_string(), "Commits");
        assert_eq!(Panel::Diff.to_string(), "Diff");
    }

    #[test]
    fn test_commit_info_default() {
        let info = CommitInfo::default();
        assert!(info.subject.is_empty());
        assert!(info.hash.is_empty());
        assert!(info.parents.is_empty());
    }
}
