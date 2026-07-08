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
