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
    /// Last commit date as "YYYY-MM-DD HH:MM" for staleness coloring, None if unavailable.
    pub last_commit_date: Option<String>,
    /// Pre-computed epoch days for the last commit date, used for staleness coloring
    /// in the render path to avoid repeated string parsing.
    pub epoch_days: Option<i64>,
}
