/// A node in the hierarchical branch tree.
#[derive(Debug, Clone)]
pub struct BranchNode {
    pub name: String,
    pub full_path: String,
    pub children: Vec<BranchNode>,
}

/// The scope of branches to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchScope {
    All,
    Local,
    Remote,
    Tags,
    Stash,
}

impl BranchScope {
    pub fn next(self) -> Self {
        match self {
            BranchScope::All => BranchScope::Local,
            BranchScope::Local => BranchScope::Remote,
            BranchScope::Remote => BranchScope::Tags,
            BranchScope::Tags => BranchScope::Stash,
            BranchScope::Stash => BranchScope::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BranchScope::All => "all",
            BranchScope::Local => "local",
            BranchScope::Remote => "remote",
            BranchScope::Tags => "tags",
            BranchScope::Stash => "stash",
        }
    }
}

/// A single branch entry with its type information.
#[derive(Debug, Clone)]
pub struct BranchEntry {
    pub name: String,
    pub is_remote: bool,
    /// Last commit date as "YYYY-MM-DD HH:MM", None if unresolvable.
    pub last_commit_date: Option<String>,
    /// Pre-computed Unix epoch days (seconds/86400), None if unresolvable.
    pub epoch_days: Option<i64>,
}

/// A single stash entry with its metadata.
#[derive(Debug, Clone)]
pub struct StashEntry {
    pub index: usize,
    pub message: String,
    pub oid: String,
    pub last_commit_date: Option<String>,
    pub epoch_days: Option<i64>,
}

/// Complete branch data returned from the git repository.
#[derive(Debug, Clone)]
pub struct BranchData {
    pub default_branch: Option<String>,
    pub entries: Vec<BranchEntry>,
    /// Tag names from the repository (always fetched, regardless of scope).
    pub tags: Vec<String>,
    /// Last commit date for each tag (same index as `tags`).
    /// Each entry is `(date_string, epoch_days)`.
    pub tags_dates: Vec<Option<(String, i64)>>,
    /// Stash entries from the repository (always fetched).
    pub stashes: Vec<StashEntry>,
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
    fn test_branch_scope_next_remote_to_tags() {
        assert_eq!(BranchScope::Remote.next(), BranchScope::Tags);
    }

    #[test]
    fn test_branch_scope_next_tags_to_stash() {
        assert_eq!(BranchScope::Tags.next(), BranchScope::Stash);
    }

    #[test]
    fn test_branch_scope_next_stash_to_all() {
        assert_eq!(BranchScope::Stash.next(), BranchScope::All);
    }

    #[test]
    fn test_branch_scope_label() {
        assert_eq!(BranchScope::All.label(), "all");
        assert_eq!(BranchScope::Local.label(), "local");
        assert_eq!(BranchScope::Remote.label(), "remote");
        assert_eq!(BranchScope::Tags.label(), "tags");
        assert_eq!(BranchScope::Stash.label(), "stash");
    }
}
