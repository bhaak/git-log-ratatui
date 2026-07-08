use crate::domain::{Commit, Decoration};

/// A commit row enriched with graph rendering data for the UI.
/// Replaces the old `models::Commit` type.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitRow {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub subject: String,
    /// Unicode graph line from git-graph (already box-drawing characters).
    pub graph: String,
    /// Lane color index per character position (same length as graph).
    /// 255 = no lane (space). Otherwise lane_index % LANE_COLORS.len() picks the color.
    pub graph_colors: Vec<u8>,
    /// True if this is a merge commit (has multiple parents).
    pub merge: bool,
    /// True if this row is a graph-only continuation line (no commit).
    pub graph_only: bool,
    /// Branch/tag/HEAD decorations resolved via git2 references.
    pub decorations: Vec<Decoration>,
    /// Index of the first row in the expanded table for this decoration block.
    pub deco_line: usize,
}

impl CommitRow {
    /// Build a CommitRow from a domain Commit by applying graph rendering data.
    #[allow(dead_code)]
    pub fn from_commit(
        commit: Commit,
        graph: String,
        graph_colors: Vec<u8>,
        graph_only: bool,
        deco_line: usize,
    ) -> Self {
        Self {
            hash: commit.hash,
            author: commit.author,
            date: commit.date,
            subject: commit.subject,
            merge: commit.merge,
            decorations: commit.decorations,
            graph,
            graph_colors,
            graph_only,
            deco_line,
        }
    }
}
