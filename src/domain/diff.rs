/// A file changed in a commit, with its diff line offset.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub diff_line: usize,
    /// File change status: '+' added, '-' deleted, '~' modified, '→' renamed, '?' unknown
    pub status: char,
    /// Previous path for renamed/copied files
    pub old_name: Option<String>,
    /// Number of lines added in this file
    pub lines_added: usize,
    /// Number of lines removed in this file
    pub lines_removed: usize,
}
