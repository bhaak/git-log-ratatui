/// A commit entry with pure git metadata (no UI/graph data).
/// Exists as a semantic domain type; CommitRow in the view layer
/// extends this with rendering-specific fields.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct Commit {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub subject: String,
    /// True if this is a merge commit (has multiple parents).
    pub merge: bool,
    /// Branch/tag/HEAD decorations resolved via git2 references.
    pub decorations: Vec<Decoration>,
}

/// A single decoration on a commit (HEAD, branch, tag).
#[derive(Debug, Clone, PartialEq)]
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

impl DecorationKind {
    /// Sort priority for decoration display (lower = first).
    pub fn priority(&self) -> u8 {
        match self {
            DecorationKind::Head => 0,
            DecorationKind::LocalBranch => 1,
            DecorationKind::Tag => 2,
            DecorationKind::RemoteBranch => 3,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_info_default() {
        let info = CommitInfo::default();
        assert!(info.subject.is_empty());
        assert!(info.hash.is_empty());
        assert!(info.parents.is_empty());
    }
}
